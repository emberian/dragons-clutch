//! Frame-bounded streaming decoders for the dense order page.
//!
//! The buffered [`OrderPageAccount`] decoder returns a whole page **by value**:
//! [`MAX_ORDERS_PER_PAGE`] slots of [`ORDER_SLOT_BYTES`] each, plus the header.
//! That is measured at an estimated 8,640-byte SBF call frame against a
//! 4,096-byte maximum, so no on-chain instruction can call it at all.  This
//! module is the same set of refusals, arranged so that no function ever holds
//! more than one slot: the header is decoded on its own, the slot array is
//! walked one [`ORDER_SLOT_BYTES`] slot at a time, and the page digest is
//! folded into a streamed SHA-256 exactly as
//! [`OrderPageAccount::recomputed_page_digest`] already does.
//!
//! # On-chain consumers use only this path
//!
//! [`OrderPageAccount::decode`], [`OrderPageAccount::decode_on_grid`], and
//! [`crate::verify_page_set`] are **host-only**.  They are unchanged, they stay
//! the golden reference every equivalence test is written against, and no
//! instruction compiled for `target_os = "solana"` may call them.  An on-chain
//! consumer reads a page with [`verify_page`] (or [`verify_page_on_grid`]),
//! walks its orders with [`OrderSlotCursor`], and closes a frozen page set with
//! [`verify_page_set`] or [`epoch_binds_page_set`].
//!
//! # Equivalence contract
//!
//! For **any** byte input:
//!
//! - [`verify_page`] returns exactly what [`OrderPageAccount::decode`] returns:
//!   the identical [`CodecError`], or the same page's header fields.
//! - [`verify_page_on_grid`] stands in the same relation to
//!   [`OrderPageAccount::decode_on_grid`].
//! - [`verify_page_set`] returns exactly what decoding every page in index
//!   order and then calling [`crate::verify_page_set`] returns — including
//!   which of several faults is reported first — with one stated exception: a
//!   set of more than [`MAX_ORDER_PAGES`] page slices is refused with
//!   [`CodecError::InvalidCount`] before any page bytes are read, because no
//!   such set could be a book whatever its pages hold.
//! - [`epoch_binds_page_set`] stands in the same relation to
//!   [`EpochAccount::binds_page_set`].
//!
//! Equivalence is a property of the whole verdict, not of every helper.  The
//! header-local [`OrderPageHeader::validate_shape`] deliberately decides only
//! what the first [`ORDER_PAGE_HEADER_BYTES`] bytes can decide; on a page whose
//! slot array is canonical it agrees with [`verify_page`], and on one whose
//! slots are not it may report a header fault the page verdict would have
//! reported later.  It is a cheap precondition, never the page's verdict.

use super::{
    account_len, account_version, canonical_order_set_id, check_hash, decode_slot, CodecError,
    EpochAccount, EpochId, Hash32, MarketId, OrderPageAccount, OrderSlot, PriceGridAccount, Reader,
    Result, Sha256, EPOCH_PHASE_OPEN, MAX_ORDERS_PER_PAGE, MAX_ORDER_PAGES, MAX_PORTFOLIO_ORDERS,
    ORDER_PAGE_DOMAIN, ORDER_PAGE_TAG, ORDER_SLOT_BYTES,
};

/// Bytes of an order page before its slot array: the fixed page header.
///
/// This is the only part of a page a streaming decoder ever materializes.
pub const ORDER_PAGE_HEADER_BYTES: usize =
    account_len::ORDER_PAGE - (MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES);

const _: () = assert!(ORDER_PAGE_HEADER_BYTES == 235);
const _: () = assert!(
    ORDER_PAGE_HEADER_BYTES + (MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES) == account_len::ORDER_PAGE
);

/// The header half of an [`OrderPageAccount`]: every field except the slots.
///
/// These are exactly the fields the cross-page closure checks read, which is
/// why a page set can be verified without any page ever being materialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderPageHeader {
    /// Market identity.
    pub market: MarketId,
    /// Epoch identity.
    pub epoch: EpochId,
    /// Set-wide order-set digest; zero until the page set is frozen.
    pub order_set: Hash32,
    /// Digest of this page's position and record bytes.
    pub page_digest: Hash32,
    /// This page's lowest order id; zero exactly when the page is empty.
    pub first_order_id: Hash32,
    /// This page's highest order id; zero exactly when the page is empty.
    pub last_order_id: Hash32,
    /// The previous page's `last_order_id`; zero exactly on page zero.
    pub prev_page_last_order_id: Hash32,
    /// Zero-based page index.
    pub page_index: u16,
    /// Total frozen page count.
    pub page_count: u16,
    /// Orders across the whole page set; zero until the set is frozen.
    pub set_order_count: u16,
    /// Number of populated records.
    pub order_count: u8,
    /// Freeze state: 0 open, 1 frozen.
    pub frozen: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl OrderPageHeader {
    /// An all-zero header, used only to initialize a fixed page-set array.
    const ZEROED: Self = Self {
        market: Hash32::ZERO,
        epoch: Hash32::ZERO,
        order_set: Hash32::ZERO,
        page_digest: Hash32::ZERO,
        first_order_id: Hash32::ZERO,
        last_order_id: Hash32::ZERO,
        prev_page_last_order_id: Hash32::ZERO,
        page_index: 0,
        page_count: 0,
        set_order_count: 0,
        order_count: 0,
        frozen: 0,
        stored_bump: 0,
    };

    /// Read the fixed [`ORDER_PAGE_HEADER_BYTES`] header, materializing no slot.
    ///
    /// The account's **exact** [`account_len::ORDER_PAGE`] length, tag, and
    /// schema version are still required — a header is only meaningful as the
    /// head of a whole page — but nothing beyond the header is read, so this
    /// costs one header struct of stack and no slot at all.
    ///
    /// This decides nothing about the page's contents: it does not validate the
    /// header's own shape (see [`OrderPageHeader::validate_shape`]) and it
    /// cannot check the stored digest, which is a fact about the slot bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            ORDER_PAGE_TAG,
            account_version::ORDER_PAGE,
            account_len::ORDER_PAGE,
        )?;
        Ok(Self {
            market: r.hash()?,
            epoch: r.hash()?,
            order_set: r.hash()?,
            page_digest: r.hash()?,
            first_order_id: r.hash()?,
            last_order_id: r.hash()?,
            prev_page_last_order_id: r.hash()?,
            page_index: r.u16()?,
            page_count: r.u16()?,
            set_order_count: r.u16()?,
            order_count: r.u8()?,
            frozen: r.u8()?,
            stored_bump: r.u8()?,
        })
    }

    /// This page's header fields, from an already-decoded page.
    ///
    /// The buffered decoder stays the golden reference, so this is how a host
    /// test states "the streaming path saw the same page".
    pub fn of_page(page: &OrderPageAccount) -> Self {
        Self {
            market: page.market,
            epoch: page.epoch,
            order_set: page.order_set,
            page_digest: page.page_digest,
            first_order_id: page.first_order_id,
            last_order_id: page.last_order_id,
            prev_page_last_order_id: page.prev_page_last_order_id,
            page_index: page.page_index,
            page_count: page.page_count,
            set_order_count: page.set_order_count,
            order_count: page.order_count,
            frozen: page.frozen,
            stored_bump: page.stored_bump,
        }
    }

    /// Every check the header can make on its own.
    ///
    /// Identities, the freeze flag, the page and order counts, the empty-page
    /// range rule, the predecessor link, and the frozen-set commitments — all
    /// of which read header bytes only.  It is a precondition a caller may
    /// apply before spending compute on the slot array, **not** the page's
    /// verdict: it cannot see a record, an order-id chain, or the page digest,
    /// so a page that passes it may still be refused by [`verify_page`].
    pub fn validate_shape(&self) -> Result<()> {
        self.validate_head()?;
        self.validate_empty_range()?;
        self.validate_link()?;
        self.validate_commitments()
    }

    /// Identities, freeze flag, and counts — the checks a page makes first.
    fn validate_head(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.epoch)?;
        if self.frozen > 1 {
            return Err(CodecError::InvalidEnum);
        }
        if self.page_count == 0
            || self.page_count as usize > MAX_ORDER_PAGES
            || self.page_index >= self.page_count
            || self.order_count as usize > MAX_ORDERS_PER_PAGE
            || (self.frozen == 1 && self.order_count == 0)
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }

    /// The half of the stored-range rule that needs no slot: an empty page
    /// stores a zero range.  The other half — that a populated page's stored
    /// range is exactly its first and last record — is a slot fact.
    fn validate_empty_range(&self) -> Result<()> {
        if self.order_count == 0
            && (self.first_order_id != Hash32::ZERO || self.last_order_id != Hash32::ZERO)
        {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Page zero opens the chain; every later page links to its predecessor.
    fn validate_link(&self) -> Result<()> {
        if self.page_index == 0 {
            if self.prev_page_last_order_id != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else {
            if self.prev_page_last_order_id == Hash32::ZERO {
                return Err(CodecError::ZeroIdentity);
            }
            if self.order_count > 0 && self.first_order_id.0 <= self.prev_page_last_order_id.0 {
                return Err(CodecError::NonCanonicalIdentity);
            }
        }
        Ok(())
    }

    /// Set-wide commitments exist exactly while the set is frozen.
    fn validate_commitments(&self) -> Result<()> {
        if self.frozen != 1 {
            if self.order_set != Hash32::ZERO || self.set_order_count != 0 {
                return Err(CodecError::NonCanonicalPadding);
            }
            return Ok(());
        }
        check_hash(self.order_set)?;
        if self.set_order_count < self.order_count as u16
            || self.set_order_count as usize > super::MAX_EPOCH_ORDERS
        {
            return Err(CodecError::InvalidCount);
        }
        let full_pages = self.page_count as usize - 1;
        let low = full_pages * MAX_ORDERS_PER_PAGE;
        if self.page_index as usize + 1 == self.page_count as usize {
            // The last page closes the count exactly.
            if self.set_order_count as usize != low + self.order_count as usize {
                return Err(CodecError::MismatchedBinding);
            }
        } else {
            // Every non-final page of a frozen set is dense.
            if self.order_count as usize != MAX_ORDERS_PER_PAGE {
                return Err(CodecError::InvalidCount);
            }
            if self.set_order_count as usize <= low
                || self.set_order_count as usize > low + MAX_ORDERS_PER_PAGE
            {
                return Err(CodecError::MismatchedBinding);
            }
        }
        Ok(())
    }
}

/// A cursor that decodes one order slot at a time from a page's raw bytes.
///
/// The cursor holds the page bytes, the populated-record count it read from the
/// header, its position, and the previous record's order id.  Each step decodes
/// exactly one [`ORDER_SLOT_BYTES`] slot — kind byte, that kind's exact body,
/// canonical zero padding to the common width — so the walk costs one slot of
/// stack no matter how many slots a page holds.  Across steps it enforces the
/// page's own record rules: a slot below `order_count` must hold a valid record
/// whose order id is nonzero and strictly above its predecessor, and a slot at
/// or above `order_count` must be canonical padding.
///
/// A step that refuses fuses the cursor: every later step is `None`.
///
/// The cursor reads `order_count` from the header without deciding whether that
/// count is itself canonical — that is [`verify_page`]'s verdict, and a caller
/// normally runs it first.  A cursor opened over a page with an over-large
/// count walks a *stricter* array than the page describes (every slot must then
/// hold a record), never a laxer one, so skipping the page verdict can only
/// refuse more.
///
/// ```
/// use clutch_solana_layout::{account_len, stream, OrderSlot};
/// # use clutch_solana_layout::*;
/// # let market = Hash32::from_bytes([1; 32]);
/// # let epoch = canonical_epoch_id(market, 4);
/// # let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
/// # orders[0] = OrderSlot::Single(OrderRecord {
/// #     owner: Hash32::from_bytes([20; 32]),
/// #     order_id: Hash32::from_bytes([7; 32]),
/// #     outcome: 0, side: 0, quantity: 10, limit: 2_500,
/// #     minimum_fill: 0, flags: 0, generation: 1,
/// # });
/// # let mut page = OrderPageAccount {
/// #     market, epoch,
/// #     order_set: Hash32::ZERO, page_digest: Hash32::ZERO,
/// #     first_order_id: Hash32::from_bytes([7; 32]),
/// #     last_order_id: Hash32::from_bytes([7; 32]),
/// #     prev_page_last_order_id: Hash32::ZERO,
/// #     page_index: 0, page_count: 1, set_order_count: 0,
/// #     order_count: 1, frozen: 0, stored_bump: 5, orders,
/// # };
/// # page.page_digest = page.recomputed_page_digest().unwrap();
/// # let mut bytes = [0u8; account_len::ORDER_PAGE];
/// # page.encode(&mut bytes).unwrap();
/// // `bytes` is one encoded order page.
/// let header = stream::verify_page(&bytes).unwrap();
/// assert_eq!(header.order_count, 1);
///
/// let mut cursor = stream::OrderSlotCursor::new(&bytes).unwrap();
/// let mut records = 0;
/// while let Some(slot) = cursor.next_slot() {
///     match slot.unwrap() {
///         OrderSlot::Empty => {}
///         _ => records += 1,
///     }
/// }
/// assert_eq!(records, 1);
/// ```
#[derive(Clone, Debug)]
pub struct OrderSlotCursor<'a> {
    input: &'a [u8],
    order_count: u8,
    index: usize,
    previous: Hash32,
}

impl<'a> OrderSlotCursor<'a> {
    /// Open a cursor over one page's slot array.
    ///
    /// The page's tag, schema version, and exact length are checked, and its
    /// header is read to learn `order_count`; no slot is decoded yet.
    pub fn new(input: &'a [u8]) -> Result<Self> {
        let header = OrderPageHeader::decode(input)?;
        Ok(Self::over(input, header.order_count))
    }

    /// Position a cursor with an already-read `order_count`.  Private: the
    /// count must come from this very buffer's header, which
    /// [`OrderSlotCursor::new`] guarantees and a caller could not.
    fn over(input: &'a [u8], order_count: u8) -> Self {
        Self {
            input,
            order_count,
            index: 0,
            previous: Hash32::ZERO,
        }
    }

    /// Index of the slot the next step will read.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Slots left to read.
    pub fn remaining(&self) -> usize {
        MAX_ORDERS_PER_PAGE - self.index
    }

    /// Decode the next slot, or `None` once the array is exhausted or a step
    /// has refused.
    pub fn next_slot(&mut self) -> Option<Result<OrderSlot>> {
        if self.index >= MAX_ORDERS_PER_PAGE {
            return None;
        }
        let index = self.index;
        let step = self.step(index);
        // A refusal fuses the cursor: a page whose slot array is not canonical
        // has no later slots to speak of.
        self.index = if step.is_ok() {
            index + 1
        } else {
            MAX_ORDERS_PER_PAGE
        };
        Some(step)
    }

    fn step(&mut self, index: usize) -> Result<OrderSlot> {
        let start = ORDER_PAGE_HEADER_BYTES + (index * ORDER_SLOT_BYTES);
        let mut r = Reader::at(self.input, start);
        let slot = decode_slot(&mut r)?;
        if index < self.order_count as usize {
            slot.validate()?;
            let id = slot.order_id();
            if id == Hash32::ZERO || (self.previous != Hash32::ZERO && id.0 <= self.previous.0) {
                return Err(CodecError::NonCanonicalIdentity);
            }
            self.previous = id;
        } else if slot != OrderSlot::Empty {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(slot)
    }
}

impl Iterator for OrderSlotCursor<'_> {
    type Item = Result<OrderSlot>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_slot()
    }
}

/// Recompute one page's digest from its raw bytes, streaming its slots.
///
/// The value equals [`OrderPageAccount::recomputed_page_digest`] on the same
/// page and [`crate::canonical_page_digest`] over the concatenated slot bytes.
/// Every slot is decoded structurally as it is folded, so a page whose slot
/// array is not canonical has no digest rather than a digest over junk.
pub fn streamed_page_digest(input: &[u8]) -> Result<Hash32> {
    let header = OrderPageHeader::decode(input)?;
    fold_page_digest(input, &header)
}

/// Fold the page preimage into a streamed SHA-256, one slot at a time.
///
/// `header` must have been decoded from `input`, which is what makes the
/// slot-array offsets below in range.  Each slot is decoded before its bytes
/// are folded, so this performs exactly the structural half of the buffered
/// decoder's slot pass — kind byte, exact body width, canonical padding — in
/// the same order, and refuses with the same error.  Hashing the raw bytes is
/// identical to hashing a re-encoded slot precisely because a slot that decodes
/// has exactly one encoding.
fn fold_page_digest(input: &[u8], header: &OrderPageHeader) -> Result<Hash32> {
    let mut h = Sha256::new();
    h.update(ORDER_PAGE_DOMAIN);
    h.update(&header.market.0);
    h.update(&header.epoch.0);
    h.update(&header.page_index.to_le_bytes());
    h.update(&[header.order_count]);
    let mut i = 0;
    while i < MAX_ORDERS_PER_PAGE {
        let start = ORDER_PAGE_HEADER_BYTES + (i * ORDER_SLOT_BYTES);
        let mut r = Reader::at(input, start);
        decode_slot(&mut r)?;
        h.update(&input[start..start + ORDER_SLOT_BYTES]);
        i += 1;
    }
    Ok(Hash32(h.finish()))
}

/// Verify one order page from its raw bytes and return its header.
///
/// Every refusal of [`OrderPageAccount::decode`], in the same order, at a
/// bounded frame: the header is read on its own, the slot array is swept once
/// structurally while the page digest is folded, the header's own shape is
/// checked, the slot array is walked once more for record semantics and the
/// order-id chain, and the stored digest is compared last.
///
/// The returned header is every page field except the slots; a caller that
/// needs the records opens an [`OrderSlotCursor`] over the same bytes.
pub fn verify_page(input: &[u8]) -> Result<OrderPageHeader> {
    verify_page_folding(input).map(|(header, _)| header)
}

/// [`verify_page`], also returning the page's portfolio-record count.
///
/// The count is a slot fold the cross-page bound needs and no header carries,
/// so folding it here is what keeps [`verify_page_set`] to one pass over the
/// slot bytes.
fn verify_page_folding(input: &[u8]) -> Result<(OrderPageHeader, usize)> {
    let header = OrderPageHeader::decode(input)?;
    // Pass one: the structural slot sweep the buffered decoder performs before
    // any semantic check, folded into the digest as it goes.
    let digest = fold_page_digest(input, &header)?;
    header.validate_head()?;
    // Pass two: record semantics, the order-id chain, the padding rule, and the
    // stored range — one slot of stack at a time.
    let mut cursor = OrderSlotCursor::over(input, header.order_count);
    let mut portfolios = 0usize;
    let mut first = Hash32::ZERO;
    let mut last = Hash32::ZERO;
    let mut index = 0usize;
    while let Some(step) = cursor.next_slot() {
        let slot = step?;
        if index < header.order_count as usize {
            if index == 0 {
                first = slot.order_id();
            }
            last = slot.order_id();
            if slot.is_portfolio() {
                portfolios += 1;
            }
        }
        index += 1;
    }
    // One page cannot hold more portfolio records than the whole frozen book
    // admits; the set-wide sum is checked in [`verify_page_set`].
    if portfolios > MAX_PORTFOLIO_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    if header.first_order_id != first || header.last_order_id != last {
        return Err(CodecError::MismatchedBinding);
    }
    header.validate_link()?;
    header.validate_commitments()?;
    if header.page_digest != digest {
        return Err(CodecError::MismatchedBinding);
    }
    Ok((header, portfolios))
}

/// Verify one order page and apply every check that needs the frozen grid.
///
/// The streaming counterpart of [`OrderPageAccount::decode_on_grid`]: a
/// single-Egg record's `limit` must be an exact member of the tick vector, and
/// a portfolio record's per-lot value and bound must be representable on the
/// frozen price scale.  The grid is walked one slot at a time.
pub fn verify_page_on_grid(input: &[u8], grid: &PriceGridAccount) -> Result<OrderPageHeader> {
    let header = verify_page(input)?;
    grid.validate()?;
    let mut cursor = OrderSlotCursor::over(input, header.order_count);
    let mut index = 0usize;
    while index < header.order_count as usize {
        let slot = match cursor.next_slot() {
            Some(step) => step?,
            // Unreachable: `order_count` is at most `MAX_ORDERS_PER_PAGE` and
            // `verify_page` already accepted the array.  Stated, not assumed.
            None => return Err(CodecError::Truncated),
        };
        match slot {
            OrderSlot::Single(o) => {
                grid.tick_of(o.limit)?;
            }
            OrderSlot::Portfolio(p) => p.validate_on_scale(grid.price_scale)?,
            OrderSlot::Empty => return Err(CodecError::ZeroIdentity),
        }
        index += 1;
    }
    Ok(header)
}

/// Verify cross-page order range, uniqueness, and closure over raw page bytes.
///
/// The streaming counterpart of [`crate::verify_page_set`]: the pages are given
/// as their account bytes in page-index order, each is verified with
/// [`verify_page`], and the cross-page checks then run over the page **headers**
/// alone.  That is the whole point — every cross-page fact (index, market,
/// epoch, order-set digest, set order count, stored range, predecessor link,
/// page digest) is a header field, and the only slot facts the closure needs
/// are per-page folds already computed while each page was verified.  Four
/// headers cost under a kilobyte of stack; four pages would cost fifteen.
///
/// On success the recomputed set-wide order-set digest is returned.
pub fn verify_page_set(pages: &[&[u8]]) -> Result<Hash32> {
    // A set of more than `MAX_ORDER_PAGES` pages could not be one book whatever
    // its pages hold, and no fixed header array could hold it either.
    if pages.is_empty() || pages.len() > MAX_ORDER_PAGES {
        return Err(CodecError::InvalidCount);
    }
    let mut headers = [OrderPageHeader::ZEROED; MAX_ORDER_PAGES];
    let mut portfolios = 0usize;
    let mut i = 0;
    while i < pages.len() {
        let (header, page_portfolios) = verify_page_folding(pages[i])?;
        headers[i] = header;
        portfolios += page_portfolios;
        i += 1;
    }
    close_page_set(&headers[..pages.len()], portfolios)
}

/// The cross-page half of the closure, over verified page headers.
fn close_page_set(headers: &[OrderPageHeader], portfolios: usize) -> Result<Hash32> {
    let head = &headers[0];
    if head.page_count as usize != headers.len() {
        return Err(CodecError::InvalidCount);
    }
    let mut digests = [Hash32::ZERO; MAX_ORDER_PAGES];
    let mut total: u16 = 0;
    let mut i = 0;
    while i < headers.len() {
        let page = &headers[i];
        if page.frozen != 1 {
            return Err(CodecError::MismatchedBinding);
        }
        if page.page_index as usize != i
            || page.page_count != head.page_count
            || page.market != head.market
            || page.epoch != head.epoch
            || page.order_set != head.order_set
            || page.set_order_count != head.set_order_count
        {
            return Err(CodecError::MismatchedBinding);
        }
        if i == 0 {
            if page.prev_page_last_order_id != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else if page.prev_page_last_order_id != headers[i - 1].last_order_id {
            return Err(CodecError::NonCanonicalIdentity);
        }
        total = total
            .checked_add(page.order_count as u16)
            .ok_or(CodecError::ArithmeticOverflow)?;
        digests[i] = page.page_digest;
        i += 1;
    }
    if total != head.set_order_count {
        return Err(CodecError::MismatchedBinding);
    }
    if portfolios > MAX_PORTFOLIO_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    let order_set = canonical_order_set_id(
        head.market,
        head.epoch,
        head.page_count,
        head.set_order_count,
        &digests[..headers.len()],
    );
    if order_set != head.order_set {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(order_set)
}

/// Tie a verified page set, given as raw page bytes, to its epoch.
///
/// The streaming counterpart of [`EpochAccount::binds_page_set`].  The epoch
/// owns the market's actual outcome width, which no page can bound below
/// [`crate::MAX_OUTCOMES`], so after the set closes, every record is walked
/// once more — one slot of stack at a time — to refuse a single-Egg outcome at
/// or above, or a portfolio `active_len` above, that width.
pub fn epoch_binds_page_set(epoch: &EpochAccount, pages: &[&[u8]]) -> Result<()> {
    epoch.validate()?;
    if epoch.phase == EPOCH_PHASE_OPEN {
        return Err(CodecError::MismatchedBinding);
    }
    let order_set = verify_page_set(pages)?;
    let head = OrderPageHeader::decode(pages[0])?;
    let tail = OrderPageHeader::decode(pages[pages.len() - 1])?;
    if order_set != epoch.order_set
        || head.market != epoch.market
        || head.epoch != epoch.epoch
        || head.page_count != epoch.page_count
        || head.set_order_count != epoch.order_count
        || head.first_order_id != epoch.first_order_id
        || tail.last_order_id != epoch.last_order_id
    {
        return Err(CodecError::MismatchedBinding);
    }
    let mut i = 0;
    while i < pages.len() {
        let order_count = OrderPageHeader::decode(pages[i])?.order_count;
        let mut cursor = OrderSlotCursor::over(pages[i], order_count);
        let mut index = 0usize;
        while index < order_count as usize {
            let slot = match cursor.next_slot() {
                Some(step) => step?,
                // Unreachable after `verify_page_set`; stated, not assumed.
                None => return Err(CodecError::Truncated),
            };
            match slot {
                OrderSlot::Single(o) => {
                    if o.outcome >= epoch.outcome_count {
                        return Err(CodecError::MismatchedBinding);
                    }
                }
                OrderSlot::Portfolio(p) => {
                    if p.active_len > epoch.outcome_count {
                        return Err(CodecError::MismatchedBinding);
                    }
                }
                OrderSlot::Empty => return Err(CodecError::ZeroIdentity),
            }
            index += 1;
        }
        i += 1;
    }
    Ok(())
}
