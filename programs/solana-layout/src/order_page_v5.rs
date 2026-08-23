//! Canonical General OrderPage V5 codec and frame-bounded reader.
//!
//! V5 owns `(tag 8, version 5)` and exactly 4,140 bytes.  Bytes `0..4012`
//! retain the complete V4 field and slot sequence; bytes `4012..4140` are a
//! positional `[u64; 16]` table containing the authenticated Position
//! generation used by each live order.  This is a fresh schema: neither this
//! decoder nor its streaming reader accepts V4.
//!
//! The generation at index `i` belongs only to slot `i`.  A single or
//! portfolio order requires a nonzero generation.  Empty and tombstone slots
//! require zero.  No Position id, Reservation id, or parallel owner binding is
//! persisted here; those identities remain account-metadata joins in the
//! adapter.
//!
//! The exact leaf transcript is `SHA256(page_domain || market || epoch ||
//! page_index_le || [order_count, tombstone_count] || slot[0] || ... ||
//! slot[15] || position_generation[0]_le || ... ||
//! position_generation[15]_le)`.  The set transcript is `SHA256(set_domain ||
//! market || epoch || page_count_le || set_order_count_le || page_digest[0] ||
//! ...)`, in page-index order.

use super::{
    account_len, account_version, check_hash, decode_slot, encode_slot, order_id_rank,
    page_base_rank, projection::OwnerInterner, put_header, stream, CodecError, EpochId, Hash32,
    MarketId, OrderPageAccount, OrderSlot, Reader, Result, Sha256, Writer, MAX_ORDERS_PER_PAGE,
    MAX_ORDER_PAGES, MAX_OUTCOMES, MAX_PORTFOLIO_ORDERS, ORDER_PAGE_TAG, ORDER_SLOT_BYTES,
};

/// Canonical General OrderPage discriminator.
pub const ORDER_PAGE_V5_TAG: u8 = ORDER_PAGE_TAG;
/// Canonical General OrderPage schema version.
pub const ORDER_PAGE_V5_VERSION: u8 = account_version::ORDER_PAGE_V5;
/// Exact canonical General OrderPage account width.
pub const ORDER_PAGE_V5_BYTES: usize = account_len::ORDER_PAGE_V5;
/// Exact bytes after the two-byte tag/version envelope.
pub const ORDER_PAGE_V5_BODY_BYTES: usize = ORDER_PAGE_V5_BYTES - 2;
/// V5 bytes before the first order slot.
pub const ORDER_PAGE_V5_HEADER_BYTES: usize = 236;
/// Offset of the trailing Position-generation table.
pub const ORDER_PAGE_V5_GENERATION_TAIL_OFFSET: usize = account_len::ORDER_PAGE;
/// Exact width of the trailing Position-generation table.
pub const ORDER_PAGE_V5_GENERATION_TAIL_BYTES: usize = MAX_ORDERS_PER_PAGE * 8;

/// Fresh V5 page-commitment domain.
pub const ORDER_PAGE_V5_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/order-page/v5";
/// Fresh V5 page-set/order-set commitment domain.
pub const ORDER_SET_V5_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/order-set/v5";

const _: () = assert!(ORDER_PAGE_TAG == 8);
const _: () = assert!(account_version::ORDER_PAGE_V5 == 5);
const _: () = assert!(account_len::ORDER_PAGE == 4012);
const _: () = assert!(account_len::ORDER_PAGE_V5 == 4140);
const _: () = assert!(ORDER_PAGE_V5_BODY_BYTES == 4138);
const _: () = assert!(ORDER_PAGE_V5_HEADER_BYTES + MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES == 4012);
const _: () = assert!(
    ORDER_PAGE_V5_GENERATION_TAIL_OFFSET + ORDER_PAGE_V5_GENERATION_TAIL_BYTES
        == account_len::ORDER_PAGE_V5
);

fn validate_slot_generation(slot: OrderSlot, generation: u64) -> Result<()> {
    match slot {
        OrderSlot::Single(_) | OrderSlot::Portfolio(_) if generation == 0 => {
            Err(CodecError::ZeroValue)
        }
        OrderSlot::Single(_) | OrderSlot::Portfolio(_) => Ok(()),
        OrderSlot::Empty | OrderSlot::Tombstone(_) if generation != 0 => {
            Err(CodecError::NonCanonicalPadding)
        }
        OrderSlot::Empty | OrderSlot::Tombstone(_) => Ok(()),
    }
}

fn page_digest_from_canonical_parts(
    market: MarketId,
    epoch: EpochId,
    page_index: u16,
    order_count: u8,
    tombstone_count: u8,
    slots: &[u8],
    position_generations: &[u64; MAX_ORDERS_PER_PAGE],
) -> Result<Hash32> {
    check_hash(market)?;
    check_hash(epoch)?;
    if order_count as usize > MAX_ORDERS_PER_PAGE || tombstone_count > order_count {
        return Err(CodecError::InvalidCount);
    }
    if slots.len() != MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES {
        return Err(CodecError::InvalidCount);
    }
    let mut h = Sha256::new();
    h.update(ORDER_PAGE_V5_DIGEST_DOMAIN);
    h.update(&market.0);
    h.update(&epoch.0);
    h.update(&page_index.to_le_bytes());
    h.update(&[order_count, tombstone_count]);
    let mut i = 0;
    while i < MAX_ORDERS_PER_PAGE {
        let start = i * ORDER_SLOT_BYTES;
        let mut r = Reader::at(slots, start);
        let slot = decode_slot(&mut r)?;
        validate_slot_generation(slot, position_generations[i])?;
        h.update(&slots[start..start + ORDER_SLOT_BYTES]);
        i += 1;
    }
    i = 0;
    while i < MAX_ORDERS_PER_PAGE {
        h.update(&position_generations[i].to_le_bytes());
        i += 1;
    }
    Ok(Hash32(h.finish()))
}

/// Hash one exact V5 slot array and its positional Position generations.
///
/// `slots` must be exactly sixteen canonical 236-byte slot encodings.  The
/// generation route rule is checked before any digest is returned.
pub fn canonical_page_digest_v5(
    market: MarketId,
    epoch: EpochId,
    page_index: u16,
    order_count: u8,
    tombstone_count: u8,
    slots: &[u8],
    position_generations: &[u64; MAX_ORDERS_PER_PAGE],
) -> Result<Hash32> {
    page_digest_from_canonical_parts(
        market,
        epoch,
        page_index,
        order_count,
        tombstone_count,
        slots,
        position_generations,
    )
}

/// Derive the V5 order-set id from the ordered V5 page-digest sequence.
///
/// The fresh set domain prevents a V4 page-digest list from being interpreted
/// as a V5 set, while each V5 leaf transitively commits its generation table.
pub fn canonical_order_set_id_v5(
    market: MarketId,
    epoch: EpochId,
    page_count: u16,
    set_order_count: u16,
    page_digests: &[Hash32],
) -> Result<Hash32> {
    check_hash(market)?;
    check_hash(epoch)?;
    if page_count == 0
        || page_count as usize > MAX_ORDER_PAGES
        || page_digests.len() != page_count as usize
    {
        return Err(CodecError::InvalidCount);
    }
    let mut h = Sha256::new();
    h.update(ORDER_SET_V5_DIGEST_DOMAIN);
    h.update(&market.0);
    h.update(&epoch.0);
    h.update(&page_count.to_le_bytes());
    h.update(&set_order_count.to_le_bytes());
    let mut i = 0;
    while i < page_digests.len() {
        check_hash(page_digests[i])?;
        h.update(&page_digests[i].0);
        i += 1;
    }
    Ok(Hash32(h.finish()))
}

/// Buffered host representation of one canonical V5 page.
///
/// `page` is the complete historical V4 semantic field and slot sequence, but
/// its `page_digest` and `order_set` fields contain V5 commitments.  The
/// trailing table is a distinct field so no caller can accidentally treat an
/// order's replay generation as its Position generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderPageAccountV5 {
    /// Exact historical V4 semantic prefix and slot sequence.
    pub page: OrderPageAccount,
    /// Position generation authenticated for each slot by index.
    pub position_generations: [u64; MAX_ORDERS_PER_PAGE],
}

impl OrderPageAccountV5 {
    /// Live records on this page: populated slots minus tombstones.
    pub const fn live_count(&self) -> u8 {
        self.page.live_count()
    }

    /// Recompute the V5 page commitment without buffering a page.
    pub fn recomputed_page_digest(&self) -> Result<Hash32> {
        let mut h = Sha256::new();
        h.update(ORDER_PAGE_V5_DIGEST_DOMAIN);
        h.update(&self.page.market.0);
        h.update(&self.page.epoch.0);
        h.update(&self.page.page_index.to_le_bytes());
        h.update(&[self.page.order_count, self.page.tombstone_count]);
        let mut slot_bytes = [0; ORDER_SLOT_BYTES];
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            validate_slot_generation(self.page.orders[i], self.position_generations[i])?;
            let mut w = Writer::new(&mut slot_bytes);
            encode_slot(&mut w, self.page.orders[i])?;
            h.update(&slot_bytes);
            i += 1;
        }
        i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            h.update(&self.position_generations[i].to_le_bytes());
            i += 1;
        }
        Ok(Hash32(h.finish()))
    }

    /// Validate the complete V4 semantic prefix under V5 commitments and the
    /// positional generation route.
    pub fn validate(&self) -> Result<()> {
        // V4 remains the one semantic owner of its historical header and slot
        // rules.  Give that validator its own V4 leaf commitment only for the
        // duration of the shape check; neither V4 bytes nor a V4 commitment is
        // accepted by the V5 decoder.
        let mut historical = self.page;
        historical.page_digest = historical.recomputed_page_digest()?;
        historical.validate()?;
        let digest = self.recomputed_page_digest()?;
        if digest != self.page.page_digest {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exactly [`account_len::ORDER_PAGE_V5`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < account_len::ORDER_PAGE_V5 {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, ORDER_PAGE_TAG, account_version::ORDER_PAGE_V5)?;
        w.hash(self.page.market)?;
        w.hash(self.page.epoch)?;
        w.hash(self.page.order_set)?;
        w.hash(self.page.page_digest)?;
        w.hash(self.page.first_order_id)?;
        w.hash(self.page.last_order_id)?;
        w.hash(self.page.prev_page_last_order_id)?;
        w.u16(self.page.page_index)?;
        w.u16(self.page.page_count)?;
        w.u16(self.page.set_order_count)?;
        w.u8(self.page.order_count)?;
        w.u8(self.page.tombstone_count)?;
        w.u8(self.page.frozen)?;
        w.u8(self.page.stored_bump)?;
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            encode_slot(&mut w, self.page.orders[i])?;
            i += 1;
        }
        i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            w.u64(self.position_generations[i])?;
            i += 1;
        }
        Ok(w.at)
    }

    /// Decode exact tag-8/version-5 bytes and reject every V4 buffer.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            ORDER_PAGE_TAG,
            account_version::ORDER_PAGE_V5,
            account_len::ORDER_PAGE_V5,
        )?;
        let market = r.hash()?;
        let epoch = r.hash()?;
        let order_set = r.hash()?;
        let page_digest = r.hash()?;
        let first_order_id = r.hash()?;
        let last_order_id = r.hash()?;
        let prev_page_last_order_id = r.hash()?;
        let page_index = r.u16()?;
        let page_count = r.u16()?;
        let set_order_count = r.u16()?;
        let order_count = r.u8()?;
        let tombstone_count = r.u8()?;
        let frozen = r.u8()?;
        let stored_bump = r.u8()?;
        let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            orders[i] = decode_slot(&mut r)?;
            i += 1;
        }
        let mut position_generations = [0; MAX_ORDERS_PER_PAGE];
        i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            position_generations[i] = r.u64()?;
            i += 1;
        }
        r.done()?;
        let value = Self {
            page: OrderPageAccount {
                market,
                epoch,
                order_set,
                page_digest,
                first_order_id,
                last_order_id,
                prev_page_last_order_id,
                page_index,
                page_count,
                set_order_count,
                order_count,
                tombstone_count,
                frozen,
                stored_bump,
                orders,
            },
            position_generations,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Verify a buffered frozen V5 page set and return its V5 order-set id.
pub fn verify_page_set_v5(pages: &[OrderPageAccountV5]) -> Result<Hash32> {
    if pages.is_empty() || pages.len() > MAX_ORDER_PAGES {
        return Err(CodecError::InvalidCount);
    }
    let head = &pages[0].page;
    if head.page_count as usize != pages.len() {
        return Err(CodecError::InvalidCount);
    }
    let mut digests = [Hash32::ZERO; MAX_ORDER_PAGES];
    let mut total = 0u16;
    let mut live = 0u16;
    let mut portfolios = 0usize;
    let mut i = 0;
    while i < pages.len() {
        pages[i].validate()?;
        let page = &pages[i].page;
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
        } else if page.prev_page_last_order_id != pages[i - 1].page.last_order_id {
            return Err(CodecError::NonCanonicalIdentity);
        }
        total = total
            .checked_add(page.order_count as u16)
            .ok_or(CodecError::ArithmeticOverflow)?;
        live = live
            .checked_add(page.live_count() as u16)
            .ok_or(CodecError::ArithmeticOverflow)?;
        let mut j = 0;
        while j < page.order_count as usize {
            if page.orders[j].is_portfolio() {
                portfolios += 1;
            }
            j += 1;
        }
        digests[i] = page.page_digest;
        i += 1;
    }
    if total != head.set_order_count {
        return Err(CodecError::MismatchedBinding);
    }
    if live == 0 || portfolios > MAX_PORTFOLIO_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    let order_set = canonical_order_set_id_v5(
        head.market,
        head.epoch,
        head.page_count,
        head.set_order_count,
        &digests[..pages.len()],
    )?;
    if order_set != head.order_set {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(order_set)
}

/// Header-only projection of an OrderPage V5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderPageHeaderV5 {
    /// Market identity.
    pub market: MarketId,
    /// Epoch identity.
    pub epoch: EpochId,
    /// V5 set-wide order-set digest, zero while open.
    pub order_set: Hash32,
    /// V5 digest of this page, including the generation tail.
    pub page_digest: Hash32,
    /// Lowest populated order id, or zero for an empty page.
    pub first_order_id: Hash32,
    /// Highest populated order id, or zero for an empty page.
    pub last_order_id: Hash32,
    /// Previous page's last order id, or zero on page zero.
    pub prev_page_last_order_id: Hash32,
    /// Zero-based page index.
    pub page_index: u16,
    /// Number of pages in the set.
    pub page_count: u16,
    /// Populated slots across the frozen set, zero while open.
    pub set_order_count: u16,
    /// Populated slots on this page.
    pub order_count: u8,
    /// Tombstones among the populated slots.
    pub tombstone_count: u8,
    /// Freeze flag, zero or one.
    pub frozen: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl OrderPageHeaderV5 {
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
        tombstone_count: 0,
        frozen: 0,
        stored_bump: 0,
    };

    /// Decode only the fixed header after enforcing V5 tag, version, and exact
    /// 4,140-byte account length.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut r = Reader::new(
            input,
            ORDER_PAGE_TAG,
            account_version::ORDER_PAGE_V5,
            account_len::ORDER_PAGE_V5,
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
            tombstone_count: r.u8()?,
            frozen: r.u8()?,
            stored_bump: r.u8()?,
        })
    }

    /// Header projection of a buffered V5 page.
    pub fn of_page(page: &OrderPageAccountV5) -> Self {
        Self {
            market: page.page.market,
            epoch: page.page.epoch,
            order_set: page.page.order_set,
            page_digest: page.page.page_digest,
            first_order_id: page.page.first_order_id,
            last_order_id: page.page.last_order_id,
            prev_page_last_order_id: page.page.prev_page_last_order_id,
            page_index: page.page.page_index,
            page_count: page.page.page_count,
            set_order_count: page.page.set_order_count,
            order_count: page.page.order_count,
            tombstone_count: page.page.tombstone_count,
            frozen: page.page.frozen,
            stored_bump: page.page.stored_bump,
        }
    }

    /// Populated slots that remain live.
    pub const fn live_count(&self) -> u8 {
        self.order_count.saturating_sub(self.tombstone_count)
    }

    fn historical_header(&self) -> stream::OrderPageHeader {
        stream::OrderPageHeader {
            market: self.market,
            epoch: self.epoch,
            order_set: self.order_set,
            page_digest: self.page_digest,
            first_order_id: self.first_order_id,
            last_order_id: self.last_order_id,
            prev_page_last_order_id: self.prev_page_last_order_id,
            page_index: self.page_index,
            page_count: self.page_count,
            set_order_count: self.set_order_count,
            order_count: self.order_count,
            tombstone_count: self.tombstone_count,
            frozen: self.frozen,
            stored_bump: self.stored_bump,
        }
    }

    /// Validate every header-local historical page invariant.
    pub fn validate_shape(&self) -> Result<()> {
        self.historical_header().validate_shape()
    }
}

/// One cursor result: a canonical slot and its same-index Position generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedOrderSlotV5 {
    /// Zero-based slot coordinate within the page.
    pub slot_index: u8,
    /// Canonically decoded slot.
    pub slot: OrderSlot,
    /// Same-index Position generation; nonzero exactly for live slots.
    pub position_generation: u64,
}

/// Frame-bounded cursor over V5 slots and their trailing generation entries.
///
/// One step materializes one slot and one `u64`; it never buffers the page or
/// the complete generation table.  A refusal fuses the cursor.
#[derive(Clone, Debug)]
pub struct OrderSlotCursorV5<'a> {
    input: &'a [u8],
    order_count: u8,
    index: usize,
    base: u64,
}

impl<'a> OrderSlotCursorV5<'a> {
    /// Open a cursor after enforcing the V5 account envelope.
    pub fn new(input: &'a [u8]) -> Result<Self> {
        let header = OrderPageHeaderV5::decode(input)?;
        Ok(Self::over(input, header.order_count, header.page_index))
    }

    fn over(input: &'a [u8], order_count: u8, page_index: u16) -> Self {
        Self {
            input,
            order_count,
            index: 0,
            base: page_base_rank(page_index),
        }
    }

    /// Index the next step will read.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Slots remaining before the fixed array is exhausted.
    pub const fn remaining(&self) -> usize {
        MAX_ORDERS_PER_PAGE - self.index
    }

    /// Decode and verify the next slot/generation pair.
    pub fn next_slot(&mut self) -> Option<Result<VerifiedOrderSlotV5>> {
        if self.index >= MAX_ORDERS_PER_PAGE {
            return None;
        }
        let index = self.index;
        let step = self.step(index);
        self.index = if step.is_ok() {
            index + 1
        } else {
            MAX_ORDERS_PER_PAGE
        };
        Some(step)
    }

    fn step(&self, index: usize) -> Result<VerifiedOrderSlotV5> {
        let slot_start = ORDER_PAGE_V5_HEADER_BYTES + index * ORDER_SLOT_BYTES;
        let mut slot_reader = Reader::at(self.input, slot_start);
        let slot = decode_slot(&mut slot_reader)?;
        let generation_start = ORDER_PAGE_V5_GENERATION_TAIL_OFFSET + index * 8;
        let mut generation_reader = Reader::at(self.input, generation_start);
        let position_generation = generation_reader.u64()?;
        validate_slot_generation(slot, position_generation)?;
        if index < self.order_count as usize {
            slot.validate()?;
            if order_id_rank(slot.order_id())? != self.base + index as u64 + 1 {
                return Err(CodecError::NonCanonicalIdentity);
            }
        } else if slot != OrderSlot::Empty {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(VerifiedOrderSlotV5 {
            slot_index: index as u8,
            slot,
            position_generation,
        })
    }
}

impl Iterator for OrderSlotCursorV5<'_> {
    type Item = Result<VerifiedOrderSlotV5>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_slot()
    }
}

#[cfg(not(target_os = "solana"))]
fn fold_page_digest_v5(input: &[u8], header: &OrderPageHeaderV5) -> Result<Hash32> {
    let mut h = Sha256::new();
    h.update(ORDER_PAGE_V5_DIGEST_DOMAIN);
    h.update(&header.market.0);
    h.update(&header.epoch.0);
    h.update(&header.page_index.to_le_bytes());
    h.update(&[header.order_count, header.tombstone_count]);
    let mut i = 0;
    while i < MAX_ORDERS_PER_PAGE {
        let slot_start = ORDER_PAGE_V5_HEADER_BYTES + i * ORDER_SLOT_BYTES;
        let mut slot_reader = Reader::at(input, slot_start);
        let slot = decode_slot(&mut slot_reader)?;
        let generation_start = ORDER_PAGE_V5_GENERATION_TAIL_OFFSET + i * 8;
        let mut generation_reader = Reader::at(input, generation_start);
        validate_slot_generation(slot, generation_reader.u64()?)?;
        h.update(&input[slot_start..slot_start + ORDER_SLOT_BYTES]);
        i += 1;
    }
    h.update(&input[ORDER_PAGE_V5_GENERATION_TAIL_OFFSET..account_len::ORDER_PAGE_V5]);
    Ok(Hash32(h.finish()))
}

#[cfg(target_os = "solana")]
fn fold_page_digest_v5(input: &[u8], header: &OrderPageHeaderV5) -> Result<Hash32> {
    let page_index = header.page_index.to_le_bytes();
    let counts = [header.order_count, header.tombstone_count];
    let mut preimage: [&[u8]; 6 + MAX_ORDERS_PER_PAGE] = [&[]; 6 + MAX_ORDERS_PER_PAGE];
    preimage[0] = ORDER_PAGE_V5_DIGEST_DOMAIN;
    preimage[1] = &header.market.0;
    preimage[2] = &header.epoch.0;
    preimage[3] = &page_index;
    preimage[4] = &counts;
    let mut i = 0;
    while i < MAX_ORDERS_PER_PAGE {
        let slot_start = ORDER_PAGE_V5_HEADER_BYTES + i * ORDER_SLOT_BYTES;
        let mut slot_reader = Reader::at(input, slot_start);
        let slot = decode_slot(&mut slot_reader)?;
        let generation_start = ORDER_PAGE_V5_GENERATION_TAIL_OFFSET + i * 8;
        let mut generation_reader = Reader::at(input, generation_start);
        validate_slot_generation(slot, generation_reader.u64()?)?;
        preimage[5 + i] = &input[slot_start..slot_start + ORDER_SLOT_BYTES];
        i += 1;
    }
    preimage[5 + MAX_ORDERS_PER_PAGE] =
        &input[ORDER_PAGE_V5_GENERATION_TAIL_OFFSET..account_len::ORDER_PAGE_V5];
    Ok(Hash32(solana_sha256_hasher::hashv(&preimage).to_bytes()))
}

/// Recompute the V5 page digest from raw bytes with a frame-bounded fold.
pub fn streamed_page_digest_v5(input: &[u8]) -> Result<Hash32> {
    let header = OrderPageHeaderV5::decode(input)?;
    fold_page_digest_v5(input, &header)
}

fn verify_page_folding_v5(input: &[u8]) -> Result<(OrderPageHeaderV5, usize)> {
    verify_page_folding_v5_observing(input, |_| Ok(()))
}

/// Verify one exact V5 page while observing each validated populated slot.
///
/// The observer runs inside the semantic cursor pass that page verification
/// already requires. It never sees padding and cannot replace the stored V5
/// digest, slot-generation, record, range, or portfolio checks.
fn verify_page_folding_v5_observing<F>(
    input: &[u8],
    mut observe: F,
) -> Result<(OrderPageHeaderV5, usize)>
where
    F: FnMut(&VerifiedOrderSlotV5) -> Result<()>,
{
    let header = OrderPageHeaderV5::decode(input)?;
    let digest = fold_page_digest_v5(input, &header)?;
    header.validate_shape()?;
    let mut cursor = OrderSlotCursorV5::over(input, header.order_count, header.page_index);
    let mut portfolios = 0usize;
    let mut tombstones = 0u8;
    let mut first = Hash32::ZERO;
    let mut last = Hash32::ZERO;
    let mut index = 0usize;
    while let Some(step) = cursor.next_slot() {
        let verified = step?;
        if index < header.order_count as usize {
            observe(&verified)?;
            if index == 0 {
                first = verified.slot.order_id();
            }
            last = verified.slot.order_id();
            if verified.slot.is_portfolio() {
                portfolios += 1;
            }
            if verified.slot.is_tombstone() {
                tombstones += 1;
            }
        }
        index += 1;
    }
    if tombstones != header.tombstone_count
        || first != header.first_order_id
        || last != header.last_order_id
    {
        return Err(CodecError::MismatchedBinding);
    }
    if portfolios > MAX_PORTFOLIO_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    if digest != header.page_digest {
        return Err(CodecError::MismatchedBinding);
    }
    Ok((header, portfolios))
}

/// Verify one raw V5 page without materializing the 4,140-byte account.
pub fn verify_page_v5(input: &[u8]) -> Result<OrderPageHeaderV5> {
    verify_page_folding_v5(input).map(|(header, _)| header)
}

/// Immutable Epoch-owned context for one V5 freeze traversal.
///
/// Market, Epoch, outcome width, and horizon come from authenticated root
/// accounts in the adapter. Page-supplied copies are checked against this
/// context and never become parallel authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezePageSetContextV5 {
    market: MarketId,
    epoch: EpochId,
    outcome_count: u8,
    epoch_index: u64,
}

impl FreezePageSetContextV5 {
    /// Construct one checked context from authenticated root facts.
    pub fn new(
        market: MarketId,
        epoch: EpochId,
        outcome_count: u8,
        epoch_index: u64,
    ) -> Result<Self> {
        check_hash(market)?;
        check_hash(epoch)?;
        if !(2..=u8::try_from(MAX_OUTCOMES).map_err(|_| CodecError::InvalidCount)?)
            .contains(&outcome_count)
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(Self {
            market,
            epoch,
            outcome_count,
            epoch_index,
        })
    }

    /// Authenticated General MarketRuntime identity.
    pub const fn market(&self) -> MarketId {
        self.market
    }

    /// Authenticated General Epoch PDA identity.
    pub const fn epoch(&self) -> EpochId {
        self.epoch
    }
}

/// Exact facts from one complete authenticated open OrderPage V5 traversal.
///
/// Fields are private so no caller can construct a commitment, cardinality,
/// owner, horizon, or Position-generation summary. The retained bounded
/// headers support both exact PDA authentication and cheap post-seal checks;
/// no V5 page body is traversed again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezePageSetPrestateV5 {
    context: FreezePageSetContextV5,
    headers: [OrderPageHeaderV5; MAX_ORDER_PAGES],
    page_count: u16,
    order_set: Hash32,
    populated_order_count: u16,
    live_order_count: u16,
    owner_count: u16,
    position_generation_count: u16,
}

impl FreezePageSetPrestateV5 {
    /// V5 order-set identity committing every exact page body and Position
    /// generation tail.
    pub const fn order_set(&self) -> Hash32 {
        self.order_set
    }

    /// Exact number of pages authenticated in canonical index order.
    pub const fn page_count(&self) -> u16 {
        self.page_count
    }

    /// Populated slots, including tombstones.
    pub const fn populated_order_count(&self) -> u16 {
        self.populated_order_count
    }

    /// Live RelationV2 orders, excluding tombstones.
    pub const fn live_order_count(&self) -> u16 {
        self.live_order_count
    }

    /// Exact number of distinct live owners.
    ///
    /// Owner identities remain private interner scratch. Only cardinality is
    /// exposed, preserving the owner-blind RelationV2/ScoreV2-Q policy.
    pub const fn owner_count(&self) -> u16 {
        self.owner_count
    }

    /// Live slots whose exact nonzero Position generation was authenticated.
    pub const fn position_generation_count(&self) -> u16 {
        self.position_generation_count
    }

    /// Return one authenticated page header for exact metadata/PDA checks.
    pub fn header(&self, page_index: u16) -> Result<OrderPageHeaderV5> {
        if page_index >= self.page_count {
            return Err(CodecError::InvalidCount);
        }
        Ok(self.headers[usize::from(page_index)])
    }

    /// Compare a V5 seal result to its exact authenticated open header.
    ///
    /// Freeze may alter only `frozen`, `order_set`, and `set_order_count`.
    /// Every generation-bearing body byte stays transitively committed by the
    /// unchanged V5 page digest.
    pub fn binds_sealed_header(
        &self,
        page_index: u16,
        sealed: &OrderPageHeaderV5,
    ) -> Result<()> {
        let mut expected = self.header(page_index)?;
        expected.frozen = 1;
        expected.order_set = self.order_set;
        expected.set_order_count = self.populated_order_count;
        if *sealed != expected {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Confirm the supplied root context is exactly the one traversed.
    pub fn binds_context(&self, context: FreezePageSetContextV5) -> Result<()> {
        if context != self.context {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }
}

/// Authenticate one nonempty open V5 book and derive every freeze fact once.
///
/// This is a prestate operation: no byte is mutated. Each exact 4,140-byte
/// page is digest-verified, its slot and Position-generation tail are decoded
/// canonically, owners are interned only for live records, and width/expiry
/// are checked against the authenticated Epoch context in the same semantic
/// cursor sweep. Cross-page density, links, counts, and the V5 set commitment
/// are then closed over the retained headers.
///
/// `owners` is bounded scratch and must be empty. It is not a caller summary;
/// the page bodies are its only input. A failed traversal has no persisted
/// effect and therefore cannot partially freeze a book.
pub fn freeze_page_set_prestate_v5(
    context: FreezePageSetContextV5,
    pages: &[&[u8]],
    owners: &mut OwnerInterner,
) -> Result<FreezePageSetPrestateV5> {
    if pages.is_empty() || pages.len() > MAX_ORDER_PAGES {
        return Err(CodecError::InvalidCount);
    }
    if owners.count() != 0 {
        return Err(CodecError::MismatchedBinding);
    }
    let page_count = u16::try_from(pages.len()).map_err(|_| CodecError::InvalidCount)?;
    let mut headers = [OrderPageHeaderV5::ZEROED; MAX_ORDER_PAGES];
    let mut digests = [Hash32::ZERO; MAX_ORDER_PAGES];
    let mut populated = 0u16;
    let mut live = 0u16;
    let mut generation_bound = 0u16;
    let mut portfolios = 0usize;
    let mut horizon_valid = true;
    let mut index = 0usize;
    while index < pages.len() {
        let (header, page_portfolios) =
            verify_page_folding_v5_observing(pages[index], |verified| {
                match verified.slot {
                    OrderSlot::Single(order) => {
                        owners.intern(order.owner)?;
                        generation_bound = generation_bound
                            .checked_add(1)
                            .ok_or(CodecError::ArithmeticOverflow)?;
                        if verified.position_generation == 0
                            || order.outcome >= context.outcome_count
                            || order.expiry_epoch < context.epoch_index
                        {
                            horizon_valid = false;
                        }
                    }
                    OrderSlot::Portfolio(order) => {
                        owners.intern(order.owner)?;
                        generation_bound = generation_bound
                            .checked_add(1)
                            .ok_or(CodecError::ArithmeticOverflow)?;
                        if verified.position_generation == 0
                            || order.active_len > context.outcome_count
                            || order.expiry_epoch < context.epoch_index
                        {
                            horizon_valid = false;
                        }
                    }
                    OrderSlot::Tombstone(_) => {
                        if verified.position_generation != 0 {
                            return Err(CodecError::NonCanonicalPadding);
                        }
                    }
                    OrderSlot::Empty => return Err(CodecError::ZeroIdentity),
                }
                Ok(())
            })?;
        let expected_index = u16::try_from(index).map_err(|_| CodecError::InvalidCount)?;
        if header.frozen != 0
            || header.page_index != expected_index
            || header.page_count != page_count
            || header.market != context.market
            || header.epoch != context.epoch
        {
            return Err(CodecError::MismatchedBinding);
        }
        if index == 0 {
            if header.prev_page_last_order_id != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
        } else if header.prev_page_last_order_id != headers[index - 1].last_order_id {
            return Err(CodecError::NonCanonicalIdentity);
        }
        if index + 1 < pages.len() {
            if usize::from(header.order_count) != MAX_ORDERS_PER_PAGE {
                return Err(CodecError::InvalidCount);
            }
        } else if header.order_count == 0 {
            return Err(CodecError::InvalidCount);
        }
        populated = populated
            .checked_add(u16::from(header.order_count))
            .ok_or(CodecError::ArithmeticOverflow)?;
        live = live
            .checked_add(u16::from(header.live_count()))
            .ok_or(CodecError::ArithmeticOverflow)?;
        portfolios = portfolios
            .checked_add(page_portfolios)
            .ok_or(CodecError::ArithmeticOverflow)?;
        digests[index] = header.page_digest;
        headers[index] = header;
        index += 1;
    }
    if live == 0 || portfolios > MAX_PORTFOLIO_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    if !horizon_valid || generation_bound != live {
        return Err(CodecError::MismatchedBinding);
    }
    let order_set = canonical_order_set_id_v5(
        context.market,
        context.epoch,
        page_count,
        populated,
        &digests[..pages.len()],
    )?;
    Ok(FreezePageSetPrestateV5 {
        context,
        headers,
        page_count,
        order_set,
        populated_order_count: populated,
        live_order_count: live,
        owner_count: owners.count(),
        position_generation_count: generation_bound,
    })
}

/// Stamp one authenticated open V5 page with its set-wide commitment.
///
/// The V5 page digest already commits the complete slot and Position-
/// generation body. Freeze changes exactly the three header fields excluded
/// from that leaf preimage, validates the resulting frozen shape, and returns
/// the header for [`FreezePageSetPrestateV5::binds_sealed_header`].
pub fn seal_page_v5(
    page: &mut [u8],
    order_set: Hash32,
    set_order_count: u16,
) -> Result<OrderPageHeaderV5> {
    let mut header = OrderPageHeaderV5::decode(page)?;
    if header.frozen != 0 {
        return Err(CodecError::MismatchedBinding);
    }
    header.frozen = 1;
    header.order_set = order_set;
    header.set_order_count = set_order_count;
    header.validate_shape()?;
    write_header_v5(page, &header)?;
    Ok(header)
}

/// Write exactly the fixed V5 header, preserving all body and generation bytes.
fn write_header_v5(page: &mut [u8], header: &OrderPageHeaderV5) -> Result<()> {
    if page.len() != ORDER_PAGE_V5_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    let mut writer = Writer::new(&mut page[..ORDER_PAGE_V5_HEADER_BYTES]);
    put_header(
        &mut writer,
        ORDER_PAGE_TAG,
        account_version::ORDER_PAGE_V5,
    )?;
    writer.hash(header.market)?;
    writer.hash(header.epoch)?;
    writer.hash(header.order_set)?;
    writer.hash(header.page_digest)?;
    writer.hash(header.first_order_id)?;
    writer.hash(header.last_order_id)?;
    writer.hash(header.prev_page_last_order_id)?;
    writer.u16(header.page_index)?;
    writer.u16(header.page_count)?;
    writer.u16(header.set_order_count)?;
    writer.u8(header.order_count)?;
    writer.u8(header.tombstone_count)?;
    writer.u8(header.frozen)?;
    writer.u8(header.stored_bump)
}

/// Verify a frozen V5 page set from raw account slices.
pub fn verify_page_set_v5_streaming(pages: &[&[u8]]) -> Result<Hash32> {
    if pages.is_empty() || pages.len() > MAX_ORDER_PAGES {
        return Err(CodecError::InvalidCount);
    }
    let mut headers = [OrderPageHeaderV5::ZEROED; MAX_ORDER_PAGES];
    let mut portfolios = 0usize;
    let mut i = 0;
    while i < pages.len() {
        let (header, count) = verify_page_folding_v5(pages[i])?;
        headers[i] = header;
        portfolios += count;
        i += 1;
    }
    close_page_set_v5(&headers[..pages.len()], portfolios)
}

fn close_page_set_v5(headers: &[OrderPageHeaderV5], portfolios: usize) -> Result<Hash32> {
    let head = &headers[0];
    if head.page_count as usize != headers.len() {
        return Err(CodecError::InvalidCount);
    }
    let mut digests = [Hash32::ZERO; MAX_ORDER_PAGES];
    let mut total = 0u16;
    let mut live = 0u16;
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
        live = live
            .checked_add(page.live_count() as u16)
            .ok_or(CodecError::ArithmeticOverflow)?;
        digests[i] = page.page_digest;
        i += 1;
    }
    if total != head.set_order_count {
        return Err(CodecError::MismatchedBinding);
    }
    if live == 0 || portfolios > MAX_PORTFOLIO_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    let order_set = canonical_order_set_id_v5(
        head.market,
        head.epoch,
        head.page_count,
        head.set_order_count,
        &digests[..headers.len()],
    )?;
    if order_set != head.order_set {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(order_set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical_order_id, OrderRecord};

    fn open_page(position_generation: u64) -> OrderPageAccountV5 {
        let order_id = canonical_order_id(1);
        let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
        orders[0] = OrderSlot::Single(OrderRecord {
            owner: Hash32::from_bytes([3; 32]),
            order_id,
            outcome: 0,
            side: 0,
            quantity: 7,
            limit: 4,
            minimum_fill: 1,
            flags: 0,
            generation: 9,
            expiry_epoch: 12,
        });
        let mut value = OrderPageAccountV5 {
            page: OrderPageAccount {
                market: Hash32::from_bytes([1; 32]),
                epoch: Hash32::from_bytes([2; 32]),
                order_set: Hash32::ZERO,
                page_digest: Hash32::ZERO,
                first_order_id: order_id,
                last_order_id: order_id,
                prev_page_last_order_id: Hash32::ZERO,
                page_index: 0,
                page_count: 1,
                set_order_count: 0,
                order_count: 1,
                tombstone_count: 0,
                frozen: 0,
                stored_bump: 5,
                orders,
            },
            position_generations: [0; MAX_ORDERS_PER_PAGE],
        };
        value.position_generations[0] = position_generation;
        if position_generation != 0 {
            value.page.page_digest = value.recomputed_page_digest().unwrap();
        }
        value
    }

    fn encoded(value: &OrderPageAccountV5) -> [u8; ORDER_PAGE_V5_BYTES] {
        let mut bytes = [0; ORDER_PAGE_V5_BYTES];
        assert_eq!(value.encode(&mut bytes), Ok(ORDER_PAGE_V5_BYTES));
        bytes
    }

    fn frozen_page() -> OrderPageAccountV5 {
        let mut page = open_page(41);
        page.page.frozen = 1;
        page.page.set_order_count = 1;
        page.page.order_set = Hash32::from_bytes([8; 32]);
        page.page.page_digest = page.recomputed_page_digest().unwrap();
        page.page.order_set = canonical_order_set_id_v5(
            page.page.market,
            page.page.epoch,
            1,
            1,
            &[page.page.page_digest],
        )
        .unwrap();
        page
    }

    #[test]
    fn v5_envelope_and_width_are_exact_and_v4_is_not_reinterpreted() {
        let page = open_page(41);
        let bytes = encoded(&page);
        assert_eq!(bytes[0], 8);
        assert_eq!(bytes[1], 5);
        assert_eq!(bytes.len(), 4140);
        assert_eq!(OrderPageAccountV5::decode(&bytes), Ok(page));
        assert_eq!(
            OrderPageAccountV5::decode(&bytes[..ORDER_PAGE_V5_BYTES - 1]),
            Err(CodecError::Truncated)
        );
        assert_eq!(OrderPageAccount::decode(&bytes), Err(CodecError::TrailingBytes));

        let mut wrong_version = bytes;
        wrong_version[1] = account_version::ORDER_PAGE;
        assert_eq!(
            OrderPageAccountV5::decode(&wrong_version),
            Err(CodecError::WrongVersion)
        );

        let mut historical = page.page;
        historical.page_digest = historical.recomputed_page_digest().unwrap();
        let mut v4 = [0; account_len::ORDER_PAGE];
        historical.encode(&mut v4).unwrap();
        assert_eq!(OrderPageAccountV5::decode(&v4), Err(CodecError::Truncated));
    }

    #[test]
    fn generation_route_is_exact_for_live_retired_and_empty_slots() {
        let live_zero = open_page(0);
        assert_eq!(live_zero.validate(), Err(CodecError::ZeroValue));

        let mut padding_nonzero = open_page(41);
        padding_nonzero.position_generations[1] = 1;
        assert_eq!(
            padding_nonzero.recomputed_page_digest(),
            Err(CodecError::NonCanonicalPadding)
        );

        let mut tombstone = open_page(41);
        tombstone.page.orders[0] = OrderSlot::Tombstone(crate::TombstoneRecord {
            order_id: canonical_order_id(1),
            owner: Hash32::from_bytes([3; 32]),
            retired_generation: 9,
            generation: 10,
        });
        tombstone.page.tombstone_count = 1;
        assert_eq!(
            tombstone.recomputed_page_digest(),
            Err(CodecError::NonCanonicalPadding)
        );
        tombstone.position_generations[0] = 0;
        tombstone.page.page_digest = tombstone.recomputed_page_digest().unwrap();
        assert_eq!(tombstone.validate(), Ok(()));
    }

    #[test]
    fn cursor_streams_slot_and_same_index_position_generation() {
        let page = open_page(41);
        let bytes = encoded(&page);
        assert_eq!(verify_page_v5(&bytes), Ok(OrderPageHeaderV5::of_page(&page)));
        assert_eq!(streamed_page_digest_v5(&bytes), Ok(page.page.page_digest));

        let mut cursor = OrderSlotCursorV5::new(&bytes).unwrap();
        let first = cursor.next_slot().unwrap().unwrap();
        assert_eq!(first.slot_index, 0);
        assert_eq!(first.slot, page.page.orders[0]);
        assert_eq!(first.position_generation, 41);
        let padding = cursor.next_slot().unwrap().unwrap();
        assert_eq!(padding.slot_index, 1);
        assert_eq!(padding.slot, OrderSlot::Empty);
        assert_eq!(padding.position_generation, 0);
    }

    #[test]
    fn generation_tail_changes_both_v5_commitment_layers() {
        let page = open_page(41);
        let mut changed = page;
        changed.position_generations[0] = 42;
        let changed_digest = changed.recomputed_page_digest().unwrap();
        assert_ne!(page.page.page_digest, changed_digest);
        let before_set = canonical_order_set_id_v5(
            page.page.market,
            page.page.epoch,
            1,
            1,
            &[page.page.page_digest],
        )
        .unwrap();
        let after_set = canonical_order_set_id_v5(
            page.page.market,
            page.page.epoch,
            1,
            1,
            &[changed_digest],
        )
        .unwrap();
        assert_ne!(before_set, after_set);

        let mut tampered = encoded(&page);
        tampered[ORDER_PAGE_V5_GENERATION_TAIL_OFFSET] = 42;
        assert_eq!(verify_page_v5(&tampered), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn buffered_and_streaming_frozen_set_closure_agree() {
        let page = frozen_page();
        let bytes = encoded(&page);
        assert_eq!(verify_page_set_v5(&[page]), Ok(page.page.order_set));
        assert_eq!(
            verify_page_set_v5_streaming(&[&bytes]),
            Ok(page.page.order_set)
        );
    }

    #[test]
    fn one_v5_freeze_traversal_owns_commitment_owner_horizon_and_generation_facts() {
        let page = open_page(41);
        let mut bytes = encoded(&page);
        let before = bytes;
        let context =
            FreezePageSetContextV5::new(page.page.market, page.page.epoch, 2, 12).unwrap();
        let mut owners = OwnerInterner::new();
        let facts = freeze_page_set_prestate_v5(context, &[&bytes], &mut owners).unwrap();

        assert_eq!(facts.page_count(), 1);
        assert_eq!(facts.populated_order_count(), 1);
        assert_eq!(facts.live_order_count(), 1);
        assert_eq!(facts.owner_count(), 1);
        assert_eq!(facts.position_generation_count(), 1);
        assert_eq!(facts.header(0).unwrap().stored_bump, page.page.stored_bump);
        assert_eq!(facts.binds_context(context), Ok(()));
        assert_eq!(bytes, before, "prestate authentication is read-only");

        let sealed = seal_page_v5(
            &mut bytes,
            facts.order_set(),
            facts.populated_order_count(),
        )
        .unwrap();
        assert_eq!(facts.binds_sealed_header(0, &sealed), Ok(()));
        assert_eq!(
            &bytes[ORDER_PAGE_V5_HEADER_BYTES..],
            &before[ORDER_PAGE_V5_HEADER_BYTES..],
            "the seal cannot alter a slot or Position generation",
        );
        assert_eq!(
            verify_page_set_v5_streaming(&[&bytes]),
            Ok(facts.order_set())
        );
    }

    #[test]
    fn v5_freeze_prestate_refuses_width_horizon_tail_and_caller_scratch_faults() {
        let page = open_page(41);
        let bytes = encoded(&page);
        let context =
            FreezePageSetContextV5::new(page.page.market, page.page.epoch, 2, 12).unwrap();

        let stale =
            FreezePageSetContextV5::new(page.page.market, page.page.epoch, 2, 13).unwrap();
        assert_eq!(
            freeze_page_set_prestate_v5(stale, &[&bytes], &mut OwnerInterner::new()),
            Err(CodecError::MismatchedBinding)
        );

        let mut too_wide = page;
        too_wide.page.orders[0] = match too_wide.page.orders[0] {
            OrderSlot::Single(order) => OrderSlot::Single(OrderRecord {
                outcome: 2,
                ..order
            }),
            _ => unreachable!(),
        };
        too_wide.page.page_digest = too_wide.recomputed_page_digest().unwrap();
        let too_wide_bytes = encoded(&too_wide);
        assert_eq!(
            freeze_page_set_prestate_v5(
                context,
                &[&too_wide_bytes],
                &mut OwnerInterner::new(),
            ),
            Err(CodecError::MismatchedBinding)
        );

        let mut tampered_tail = bytes;
        tampered_tail[ORDER_PAGE_V5_GENERATION_TAIL_OFFSET] ^= 1;
        assert_eq!(
            freeze_page_set_prestate_v5(
                context,
                &[&tampered_tail],
                &mut OwnerInterner::new(),
            ),
            Err(CodecError::MismatchedBinding)
        );

        let mut prefilled = OwnerInterner::new();
        prefilled.intern(Hash32::from_bytes([99; 32])).unwrap();
        assert_eq!(
            freeze_page_set_prestate_v5(context, &[&bytes], &mut prefilled),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn v5_freeze_header_receipt_refuses_any_postseal_drift() {
        let page = open_page(41);
        let mut bytes = encoded(&page);
        let context =
            FreezePageSetContextV5::new(page.page.market, page.page.epoch, 2, 12).unwrap();
        let facts =
            freeze_page_set_prestate_v5(context, &[&bytes], &mut OwnerInterner::new()).unwrap();
        let mut sealed = seal_page_v5(
            &mut bytes,
            facts.order_set(),
            facts.populated_order_count(),
        )
        .unwrap();
        sealed.stored_bump ^= 1;
        assert_eq!(
            facts.binds_sealed_header(0, &sealed),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            facts.binds_sealed_header(1, &sealed),
            Err(CodecError::InvalidCount)
        );
    }

    #[test]
    fn raw_page_digest_api_requires_exact_canonical_slots_and_tail() {
        let page = open_page(41);
        let bytes = encoded(&page);
        let slots = &bytes[ORDER_PAGE_V5_HEADER_BYTES..ORDER_PAGE_V5_GENERATION_TAIL_OFFSET];
        assert_eq!(
            canonical_page_digest_v5(
                page.page.market,
                page.page.epoch,
                page.page.page_index,
                page.page.order_count,
                page.page.tombstone_count,
                slots,
                &page.position_generations,
            ),
            Ok(page.page.page_digest)
        );
        assert_eq!(
            canonical_page_digest_v5(
                page.page.market,
                page.page.epoch,
                page.page.page_index,
                page.page.order_count,
                page.page.tombstone_count,
                &slots[..slots.len() - 1],
                &page.position_generations,
            ),
            Err(CodecError::InvalidCount)
        );
    }
}
