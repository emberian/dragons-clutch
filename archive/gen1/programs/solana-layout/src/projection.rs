//! Page→order projection: the host half of the layout→relation seam.
//!
//! `docs/implementation/STREAMING_RELATION_DESIGN.md` §10 specifies the feed
//! the streaming verifier consumes; the v4 page addendum in
//! `docs/implementation/SOLANA_LAYOUT.md` amends two of its items, and this
//! module implements the amended spec exactly:
//!
//! * **Order ids are stored ranks.**  A v4 record's persisted `order_id` is
//!   already `canonical_order_id(rank)` — its own slot position — so the only
//!   work left here is the **live-rank renumbering that skips retirements**.
//!   The relation's `canonical_order_id` is the record's one-based position
//!   among the *live* records of the canonical page-set walk, which equals the
//!   stored rank exactly when the set holds no tombstone.
//! * **Expiry is per-order and persisted.**  `expiry_epoch` is copied
//!   verbatim.  The horizon is *checked* where the epoch index is
//!   authenticated — [`crate::EpochAccount::binds_page_set`] and
//!   [`crate::stream::epoch_binds_page_set`] refuse a frozen set holding a
//!   live record already past it — and the relation checks it again at
//!   admission against its own domain.  This module refuses neither; it is a
//!   mapping, not a horizon authority.
//!
//! The field-by-field contract is the `PortfolioOrderV1` mapping table in
//! SOLANA_LAYOUT.md (and its single-Egg counterpart in §10): every persisted
//! field verbatim, `side` `0`/`1` → [`Side::Buy`]/[`Side::Sell`], flags bit 0
//! → [`PartialPolicy::AllOrNone`].  The two coordinates no record persists are
//! the two parameters: the live rank, and the owner tag produced by
//! [`OwnerInterner`] — first-appearance interning over the same walk, which is
//! what makes the tag bijective into `0..count` **by construction** (the
//! property §10 notes nothing else proves).  The program's later obligation —
//! refusing an epoch whose frozen `owner_count` differs from the interning
//! count at pass-1 end — belongs to the pass driver, not to this module.
//!
//! What a slot projects to:
//!
//! | slot | projection |
//! | --- | --- |
//! | [`OrderSlot::Single`] | `Some(OrderV1::SingleEgg(..))`, one live rank consumed |
//! | [`OrderSlot::Portfolio`] | `Some(OrderV1::Portfolio(..))`, one live rank consumed |
//! | [`OrderSlot::Tombstone`] | `None` — skipped, **no live rank consumed**, owner not interned |
//! | [`OrderSlot::Empty`] | refused — inside the populated prefix an empty slot is a missing order, not padding |
//!
//! The index vocabulary this projection fixes — relation order ids are
//! one-based live ranks; [`crate::clearing::CandidateFeedAccount`] fill
//! indices and [`crate::clearing::LegRef::Order`] indices are the same live
//! ranks zero-based — is documented at those types and pinned by the host
//! differential in `tests/projection_stream.rs`.
//!
//! Nothing here verifies anything.  The page digests authenticate the bytes,
//! the epoch binding authenticates width and horizon, and `clutch-batch` owns
//! every admission, eligibility, fill, conservation, and pairing question.  A
//! projected order is not an admissible order.

use clutch_batch::relation_v1::{OrderV1, PortfolioOrderV1, SingleEggOrderV1};
use clutch_batch::{PartialPolicy, Side};

use super::{
    check_hash, CodecError, Hash32, OrderRecord, OrderSlot, OwnerId, PortfolioRecord, Result,
    MAX_EPOCH_ORDERS, MAX_OUTCOMES, MAX_PORTFOLIO_ORDERS,
};

// The projection is a straight field map, so the two crates' bounds must be
// the same numbers.  Stated here so a drift is a compile error, not a
// truncation.
const _: () = assert!(MAX_OUTCOMES == clutch_batch::relation_v1::MAX_OUTCOMES);
const _: () = assert!(MAX_EPOCH_ORDERS == clutch_batch::MAX_ORDERS);
const _: () = assert!(MAX_PORTFOLIO_ORDERS == clutch_batch::relation_v1::MAX_PORTFOLIO_ORDERS);

/// First-appearance owner interning over one canonical page-set walk.
///
/// §10's `owner: u16` coordinate: the feed tags each order with the index of
/// its owner's first appearance among the **live** records of the walk, which
/// makes the tag set exactly `0..count` and the tag↔owner map a bijection *by
/// construction* — same owner, same tag; distinct owners, distinct tags; no
/// gap.  `EpochAccount.owner_count` records the distinct-owner count of the
/// same walk at freeze, and the pass driver refuses the epoch when the two
/// disagree; that comparison is the driver's, not this table's.
///
/// The table is [`MAX_EPOCH_ORDERS`] owners wide — a book of 64 slots cannot
/// have more distinct owners than orders — and a 65th distinct owner refuses
/// with [`CodecError::InvalidCount`], mirroring the relation's own
/// `TooManyOrders` bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerInterner {
    /// Interned owners, first appearance first; only `..count` is meaningful.
    owners: [OwnerId; MAX_EPOCH_ORDERS],
    /// Distinct owners interned so far.
    count: u16,
}

impl OwnerInterner {
    /// The empty table.  A `const` so a checkpoint can hold one statically.
    pub const NEW: Self = Self {
        owners: [Hash32::ZERO; MAX_EPOCH_ORDERS],
        count: 0,
    };

    /// A fresh, empty table.
    pub fn new() -> Self {
        Self::NEW
    }

    /// Distinct owners interned so far; tags handed out are exactly `0..count`.
    pub fn count(&self) -> u16 {
        self.count
    }

    /// The interned owners, first appearance first.
    pub fn owners(&self) -> &[OwnerId] {
        &self.owners[..self.count as usize]
    }

    /// The tag `owner` already carries, without interning it.
    pub fn tag_of(&self, owner: OwnerId) -> Option<u16> {
        let mut i = 0usize;
        while i < self.count as usize {
            if self.owners[i] == owner {
                return Some(i as u16);
            }
            i += 1;
        }
        None
    }

    /// Crate-internal raw access for the in-place region reader.
    ///
    /// `clutch-solana-layout`'s clearing plane restores a persisted table
    /// without materializing a second 2,050-byte value on any call frame
    /// (`read_owner_interner_into`); the rules it must uphold are exactly
    /// [`OwnerInterner::restore`]'s, which stays the by-value oracle.
    pub(crate) fn raw_parts_mut(&mut self) -> (&mut [OwnerId; MAX_EPOCH_ORDERS], &mut u16) {
        (&mut self.owners, &mut self.count)
    }

    /// Reassemble a persisted table from its raw region image.
    ///
    /// The read half of the checkpoint's layout-owned interning region
    /// ([`crate::clearing::read_owner_interner`]): a bounded count, no zero
    /// owner below it, canonical zero padding at and beyond it.  Distinctness
    /// below the count is by construction of [`OwnerInterner::intern`] — the
    /// only writer of the region — and is deliberately not an all-pairs
    /// re-verification here; see the region's own documentation.
    pub fn restore(owners: [OwnerId; MAX_EPOCH_ORDERS], count: u16) -> Result<Self> {
        if count as usize > MAX_EPOCH_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        let mut i = 0usize;
        while i < MAX_EPOCH_ORDERS {
            if i < count as usize {
                check_hash(owners[i])?;
            } else if owners[i] != Hash32::ZERO {
                return Err(CodecError::NonCanonicalPadding);
            }
            i += 1;
        }
        Ok(Self { owners, count })
    }

    /// Tag `owner`, interning it on first appearance.
    ///
    /// Idempotent: a later pass over the same walk reproduces the same tags
    /// from the same table.  Refuses the zero owner — the all-zero identity is
    /// reserved for "no owner" everywhere in this crate — and refuses a 65th
    /// distinct owner with [`CodecError::InvalidCount`].
    pub fn intern(&mut self, owner: OwnerId) -> Result<u16> {
        check_hash(owner)?;
        if let Some(tag) = self.tag_of(owner) {
            return Ok(tag);
        }
        if self.count as usize >= MAX_EPOCH_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        let tag = self.count;
        self.owners[tag as usize] = owner;
        self.count += 1;
        Ok(tag)
    }
}

impl Default for OwnerInterner {
    fn default() -> Self {
        Self::NEW
    }
}

/// A live rank is one-based — the zero id stays reserved for "no order" — and
/// no walk over at most [`MAX_EPOCH_ORDERS`] slots can number past the slots.
fn check_live_rank(live_rank: u64) -> Result<()> {
    if live_rank == 0 {
        return Err(CodecError::ZeroIdentity);
    }
    if live_rank > MAX_EPOCH_ORDERS as u64 {
        return Err(CodecError::InvalidCount);
    }
    Ok(())
}

/// Refuse the enum bytes exactly as the record validators do, so the
/// projection is total over arbitrary records rather than over a precondition.
fn check_side_and_flags(side: u8, flags: u8) -> Result<()> {
    if side > 1 || flags & !1 != 0 {
        return Err(CodecError::InvalidEnum);
    }
    Ok(())
}

fn partial_policy(flags: u8) -> PartialPolicy {
    if flags & 1 != 0 {
        PartialPolicy::AllOrNone
    } else {
        PartialPolicy::Allow
    }
}

fn side(side: u8) -> Side {
    if side == 0 {
        Side::Buy
    } else {
        Side::Sell
    }
}

/// Project one single-Egg record onto the relation's order type.
///
/// Pure, and exactly the §10 single-Egg mapping table: every persisted field
/// verbatim (`limit` is the relation's `limit_price`), `side` `0`/`1` →
/// [`Side`], flags bit 0 → [`PartialPolicy`], `canonical_order_id =
/// live_rank`.  `live_rank` is the record's **one-based** position among the
/// live records of the canonical page-set walk — not its stored slot rank on a
/// set that has any retirement — and `owner_tag` is [`OwnerInterner::intern`]'s
/// image of the 32-byte owner.  Neither coordinate is judged against a domain
/// here: `owner < owner_count`, expiry, and every economic bound stay
/// `clutch-batch`'s.
///
/// Refusals are representation facts only: a zero or over-[`MAX_EPOCH_ORDERS`]
/// live rank, a side byte past `1`, a reserved flag bit.
pub fn project_single(
    record: &OrderRecord,
    live_rank: u64,
    owner_tag: u16,
) -> Result<SingleEggOrderV1> {
    check_live_rank(live_rank)?;
    check_side_and_flags(record.side, record.flags)?;
    Ok(SingleEggOrderV1 {
        canonical_order_id: live_rank,
        owner: owner_tag,
        outcome: record.outcome,
        side: side(record.side),
        quantity: record.quantity,
        limit_price: record.limit,
        minimum_fill: record.minimum_fill,
        partial_policy: partial_policy(record.flags),
        expiry_epoch: record.expiry_epoch,
    })
}

/// Project one portfolio record onto the relation's order type.
///
/// Pure, and exactly the `PortfolioOrderV1` mapping table in SOLANA_LAYOUT.md:
/// `coefficients` verbatim including canonical zero padding, `active_len`,
/// `lots`, `limit_collateral_per_lot`, `minimum_fill_lots`, and `expiry_epoch`
/// verbatim; `side` `0`/`1` → [`Side`]; flags bit 0 → [`PartialPolicy`];
/// `canonical_order_id = live_rank`, one-based as in [`project_single`];
/// `owner_tag` the interner's image of the 32-byte owner.  `generation` is
/// replay protection for the placing instruction and has no relation field.
///
/// Refusals are the same representation facts as [`project_single`]'s.
pub fn project_portfolio(
    record: &PortfolioRecord,
    live_rank: u64,
    owner_tag: u16,
) -> Result<PortfolioOrderV1> {
    check_live_rank(live_rank)?;
    check_side_and_flags(record.side, record.flags)?;
    Ok(PortfolioOrderV1 {
        canonical_order_id: live_rank,
        owner: owner_tag,
        side: side(record.side),
        coefficients: record.coefficients,
        active_len: record.active_len,
        lots: record.lots,
        limit_collateral_per_lot: record.limit_collateral_per_lot,
        minimum_fill_lots: record.minimum_fill_lots,
        partial_policy: partial_policy(record.flags),
        expiry_epoch: record.expiry_epoch,
    })
}

/// Project one populated slot of the canonical walk, interning its owner.
///
/// The per-slot step of the §10 walk, over slots **inside the populated
/// prefix** (index below the page's `order_count`; a cursor's slots at or
/// beyond it are canonical padding and are never offered to the projection):
///
/// * a live record projects to `Some` — its owner is interned (first
///   appearance takes the next tag) and the record maps under
///   [`project_single`] / [`project_portfolio`] at `live_rank`;
/// * a [`TombstoneRecord`](crate::TombstoneRecord) projects to `None`: the
///   retirement is skipped, **`live_rank` is not consumed** — the caller
///   passes the same rank to the next slot — and the retired owner is not
///   interned, because a retired record is never fed and must not mint a tag;
/// * an [`OrderSlot::Empty`] refuses with [`CodecError::ZeroIdentity`]: inside
///   the populated prefix an empty slot is a missing order, exactly the
///   reading [`OrderSlot::validate`] and the epoch binding walks give it.
///
/// `live_rank` is the one-based rank the next live record will take, i.e. one
/// more than the live records already projected; the caller owns the counter
/// and increments it exactly when this returns `Ok(Some(_))`.  On a refusal
/// the walk is over and the interner's contents are no longer meaningful.
pub fn project_slot(
    slot: &OrderSlot,
    live_rank: u64,
    owners: &mut OwnerInterner,
) -> Result<Option<OrderV1>> {
    match slot {
        OrderSlot::Empty => Err(CodecError::ZeroIdentity),
        OrderSlot::Tombstone(_) => Ok(None),
        OrderSlot::Single(record) => {
            let tag = owners.intern(record.owner)?;
            Ok(Some(OrderV1::SingleEgg(project_single(
                record, live_rank, tag,
            )?)))
        }
        OrderSlot::Portfolio(record) => {
            let tag = owners.intern(record.owner)?;
            Ok(Some(OrderV1::Portfolio(project_portfolio(
                record, live_rank, tag,
            )?)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{canonical_order_id, TombstoneRecord};

    fn owner(byte: u8) -> OwnerId {
        Hash32::from_bytes([byte; 32])
    }

    fn single_record(owner_byte: u8, rank: u64) -> OrderRecord {
        OrderRecord {
            owner: owner(owner_byte),
            order_id: canonical_order_id(rank),
            outcome: 1,
            side: 0,
            quantity: 10,
            limit: 2_500,
            minimum_fill: 2,
            flags: 0,
            generation: 1,
            expiry_epoch: 9,
        }
    }

    fn portfolio_record(owner_byte: u8, rank: u64) -> PortfolioRecord {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = 3;
        coefficients[1] = 1;
        PortfolioRecord {
            owner: owner(owner_byte),
            order_id: canonical_order_id(rank),
            side: 1,
            active_len: 2,
            flags: 1,
            coefficients,
            lots: 5,
            limit_collateral_per_lot: 9_000,
            minimum_fill_lots: 5,
            generation: 1,
            expiry_epoch: 7,
        }
    }

    #[test]
    fn single_mapping_is_field_for_field() {
        let record = single_record(20, 3);
        let projected = project_single(&record, 2, 4).unwrap();
        assert_eq!(projected.canonical_order_id, 2); // live rank, not stored rank
        assert_eq!(projected.owner, 4);
        assert_eq!(projected.outcome, 1);
        assert_eq!(projected.side, Side::Buy);
        assert_eq!(projected.quantity, 10);
        assert_eq!(projected.limit_price, 2_500);
        assert_eq!(projected.minimum_fill, 2);
        assert_eq!(projected.partial_policy, PartialPolicy::Allow);
        assert_eq!(projected.expiry_epoch, 9);
    }

    #[test]
    fn portfolio_mapping_is_field_for_field() {
        let record = portfolio_record(21, 6);
        let projected = project_portfolio(&record, 5, 0).unwrap();
        assert_eq!(projected.canonical_order_id, 5);
        assert_eq!(projected.owner, 0);
        assert_eq!(projected.side, Side::Sell);
        assert_eq!(projected.coefficients, record.coefficients);
        assert_eq!(projected.active_len, 2);
        assert_eq!(projected.lots, 5);
        assert_eq!(projected.limit_collateral_per_lot, 9_000);
        assert_eq!(projected.minimum_fill_lots, 5);
        assert_eq!(projected.partial_policy, PartialPolicy::AllOrNone);
        assert_eq!(projected.expiry_epoch, 7);
    }

    #[test]
    fn ranks_sides_and_flags_outside_the_representation_refuse() {
        let record = single_record(20, 1);
        assert_eq!(project_single(&record, 0, 0), Err(CodecError::ZeroIdentity));
        assert_eq!(
            project_single(&record, MAX_EPOCH_ORDERS as u64 + 1, 0),
            Err(CodecError::InvalidCount)
        );
        let mut bad_side = record;
        bad_side.side = 2;
        assert_eq!(
            project_single(&bad_side, 1, 0),
            Err(CodecError::InvalidEnum)
        );
        let mut bad_flags = record;
        bad_flags.flags = 2;
        assert_eq!(
            project_single(&bad_flags, 1, 0),
            Err(CodecError::InvalidEnum)
        );
        let mut portfolio = portfolio_record(21, 1);
        portfolio.side = 3;
        assert_eq!(
            project_portfolio(&portfolio, 1, 0),
            Err(CodecError::InvalidEnum)
        );
    }

    #[test]
    fn slot_projection_skips_tombstones_and_refuses_padding() {
        let mut owners = OwnerInterner::new();
        let tombstone = OrderSlot::Tombstone(TombstoneRecord {
            order_id: canonical_order_id(1),
            owner: owner(20),
            retired_generation: 1,
            generation: 2,
        });
        // Skipped: no order, no rank consumed, no tag minted.
        assert_eq!(project_slot(&tombstone, 1, &mut owners), Ok(None));
        assert_eq!(owners.count(), 0);
        // The same live rank then lands on the next live record.
        let live = OrderSlot::Single(single_record(20, 2));
        let projected = project_slot(&live, 1, &mut owners).unwrap().unwrap();
        assert_eq!(projected.id(), 1);
        assert_eq!(owners.count(), 1);
        // Empty inside the populated prefix is a missing order.
        assert_eq!(
            project_slot(&OrderSlot::Empty, 2, &mut owners),
            Err(CodecError::ZeroIdentity)
        );
    }

    #[test]
    fn interner_is_a_bijection_by_construction() {
        let mut owners = OwnerInterner::new();
        // First appearances take 0, 1, 2, ...; repeats return their own tag.
        assert_eq!(owners.intern(owner(1)), Ok(0));
        assert_eq!(owners.intern(owner(2)), Ok(1));
        assert_eq!(owners.intern(owner(1)), Ok(0));
        assert_eq!(owners.intern(owner(3)), Ok(2));
        assert_eq!(owners.count(), 3);
        assert_eq!(owners.tag_of(owner(2)), Some(1));
        assert_eq!(owners.tag_of(owner(9)), None);
        assert_eq!(owners.owners(), &[owner(1), owner(2), owner(3)]);
    }

    #[test]
    fn the_sixty_fifth_distinct_owner_refuses() {
        let mut owners = OwnerInterner::new();
        let mut i = 0u16;
        while (i as usize) < MAX_EPOCH_ORDERS {
            assert_eq!(owners.intern(owner(i as u8 + 1)), Ok(i));
            i += 1;
        }
        assert_eq!(owners.count(), MAX_EPOCH_ORDERS as u16);
        // A 65th *distinct* owner refuses; a repeat still answers its own tag.
        assert_eq!(
            owners.intern(Hash32::from_bytes([0xEE; 32])),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(owners.intern(owner(64)), Ok(63));
        // The zero owner is no identity at any fill level.
        assert_eq!(
            OwnerInterner::new().intern(Hash32::ZERO),
            Err(CodecError::ZeroIdentity)
        );
    }
}
