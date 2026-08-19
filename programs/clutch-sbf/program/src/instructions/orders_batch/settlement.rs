//! Fail-closed settlement preflight and narrow coupled consumption seam.
//!
//! This module deliberately stops before `relation_v1_stream`.  It closes the
//! joins which can be checked without inventing an on-chain policy preimage,
//! truncating a 32-byte identity to `u64`, or interpreting the opaque
//! `ClearWorkV1` bytes as a Rust value.  A successful [`verify_preflight`]
//! therefore means only that one submitted candidate feed, one checkpoint
//! header, and the complete frozen page set are mutually bound.  It is not a
//! candidate-verification or settlement verdict.
//!
//! After an external lifecycle has selected that candidate and frozen an exact
//! receipt, [`prepare_direct_full_slice`] admits one intentionally small live
//! subset: a zero-fee, exact-divisibility, same-page, full-fill direct pair of
//! single-Egg reservations.  This does not create the missing selection or
//! entitlement authority; it makes their eventual output consumable without
//! making an order page authority over assets.

use clutch_solana_layout::{
    canonical_order_id,
    clearing::{
        fill_at, slice_at, verify_candidate_feed, verify_clear_work, CandidateFeedHeader,
        ClearWorkHeader, LegRef, CLEAR_WORK_STATUS_COMPLETE,
    },
    reservation::{
        ReservationAccount, ReservationPlan, RESERVATION_STATE_ACTIVE, RESERVATION_STATE_CONSUMED,
    },
    stream, CandidateRecord, CodecError, EpochAccount, Hash32, OrderSlot, PositionAccount,
    SettlementReceiptAccount, CANDIDATE_STATUS_SELECTED, CANDIDATE_STATUS_SUBMITTED,
    EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES,
    RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED, RECEIPT_FLAG_SLICE_EXHAUSTED,
    RECEIPT_LEG_DIRECT,
};

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};

/// The facts discharged by the byte-level preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreflightFacts {
    /// Market identity shared by every input.
    pub market: Hash32,
    /// Frozen epoch identity shared by every input.
    pub epoch: Hash32,
    /// Submitted candidate identity shared by record, feed, and checkpoint.
    pub candidate: Hash32,
    /// Recomputed frozen page-set identity.
    pub order_set: Hash32,
    /// Total populated slots, including retirements.
    pub slot_count: u16,
    /// Live relation orders.  Today this necessarily equals `slot_count`:
    /// [`CandidateRecord::binds_epoch`] still owns the unresolved tombstone
    /// cardinality join and refuses every other value.
    pub live_order_count: u16,
    /// Complete frozen page count.
    pub page_count: u16,
    /// Page named by the `SettlePage` wire and the checkpoint cursor.
    pub page_cursor: u16,
}

/// Borrowed inputs to one structural settlement preflight.
///
/// Grouped so an eventual account-plane adapter has one typed handoff rather
/// than eight positional arguments which could be silently transposed.
pub(super) struct PreflightInput<'a> {
    pub epoch_bytes: &'a [u8],
    pub candidate_bytes: &'a [u8],
    pub feed_bytes: &'a [u8],
    pub clear_work_bytes: &'a [u8],
    pub pages: &'a [&'a [u8]],
    pub intent_market: Hash32,
    pub intent_epoch: Hash32,
    pub intent_page: u16,
}

/// Verify every presently representable settlement binding.
///
/// The order matters.  Each input first passes its owning codec.  The complete
/// page set is then recomputed and bound to the epoch before any candidate or
/// checkpoint claim is trusted.  No account is mutated.
// The production instruction cannot call this until the missing account-init
// and stable-body joins land.  Keeping the executable preflight compiled (and
// tested) is intentional; it is not dormant settlement success.
#[allow(dead_code)]
pub(super) fn verify_preflight(input: &PreflightInput<'_>) -> Result<PreflightFacts, CodecError> {
    let epoch = EpochAccount::decode(input.epoch_bytes)?;
    if epoch.phase != EPOCH_PHASE_FROZEN
        || epoch.market != input.intent_market
        || epoch.epoch != input.intent_epoch
        || input.intent_page >= epoch.page_count
    {
        return Err(CodecError::MismatchedBinding);
    }

    // Inclusion is not inferred from a page carrying the same `order_set`.
    // Every page is present, verified, and folded into that identity here.
    stream::epoch_binds_page_set(&epoch, input.pages)?;

    let mut live_order_count = 0u16;
    let mut page_index = 0usize;
    while page_index < input.pages.len() {
        let header = stream::OrderPageHeader::decode(input.pages[page_index])?;
        live_order_count = live_order_count
            .checked_add(u16::from(header.live_count()))
            .ok_or(CodecError::ArithmeticOverflow)?;
        page_index += 1;
    }

    let candidate = CandidateRecord::decode(input.candidate_bytes)?;
    if candidate.status != CANDIDATE_STATUS_SUBMITTED {
        return Err(CodecError::MismatchedBinding);
    }
    /* This intentionally calls the semantic owner's current binding rather
     * than weakening it locally.  Since `epoch.order_count` includes
     * tombstones and a candidate feed skips them, a cancelled book stops here
     * until the layout schema owns an exact live cardinality. */
    candidate.binds_epoch(&epoch)?;
    if u16::from(candidate.order_len) != live_order_count {
        return Err(CodecError::MismatchedBinding);
    }

    let feed = verify_candidate_feed(input.feed_bytes)?;
    bind_feed(&feed, &candidate, &epoch)?;

    let clear = verify_clear_work(input.clear_work_bytes)?;
    bind_checkpoint(&clear, &candidate, &epoch, input.intent_page)?;

    Ok(PreflightFacts {
        market: epoch.market,
        epoch: epoch.epoch,
        candidate: candidate.candidate,
        order_set: epoch.order_set,
        slot_count: epoch.order_count,
        live_order_count,
        page_count: epoch.page_count,
        page_cursor: input.intent_page,
    })
}

/// Bind the solver-written feed to the candidate record and frozen set.
#[allow(dead_code)]
fn bind_feed(
    feed: &CandidateFeedHeader,
    candidate: &CandidateRecord,
    epoch: &EpochAccount,
) -> Result<(), CodecError> {
    if feed.candidate != candidate.candidate
        || feed.epoch != candidate.epoch
        || feed.market != candidate.market
        || feed.order_set != epoch.order_set
        || feed.prices != candidate.prices
        || feed.virtual_split != candidate.virtual_split
        || feed.virtual_merge != candidate.virtual_merge
        || feed.honored_aon_mask != candidate.honored_aon_mask
        || feed.weighted_direct_volume != candidate.weighted_direct_volume
        || feed.limit_surplus_price_units != candidate.limit_surplus_price_units
        || feed.churn != candidate.churn
        || feed.distinct_owners != candidate.distinct_owners
        || feed.order_len != candidate.order_len
        || feed.outcome_count != candidate.outcome_count
    {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(())
}

/// Bind the checkpoint's layout-owned header without interpreting its body.
#[allow(dead_code)]
fn bind_checkpoint(
    clear: &ClearWorkHeader,
    candidate: &CandidateRecord,
    epoch: &EpochAccount,
    intent_page: u16,
) -> Result<(), CodecError> {
    if clear.market != candidate.market
        || clear.epoch != candidate.epoch
        || clear.candidate != candidate.candidate
        || clear.page_cursor != intent_page
        || clear.status == CLEAR_WORK_STATUS_COMPLETE
    {
        return Err(CodecError::MismatchedBinding);
    }
    // An open checkpoint is canonically unbound by its codec.  Once bound, it
    // must remain on this exact frozen set.  The body remains opaque in both
    // cases and no relation step is attempted here.
    if clear.order_set != Hash32::ZERO && clear.order_set != epoch.order_set {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Narrow coupled consumption seam                                           */
/* ------------------------------------------------------------------------ */

/// The exact post-state scalars for the smallest honest settlement slice.
///
/// This deliberately represents only a full, direct, one-to-one fill between
/// two single-Egg orders.  No field can describe a partial slice, portfolio,
/// virtual leg, fee, or rounded consideration, so those cases cannot drift
/// into the implementation by an unchecked branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectFullSlicePlan {
    /// Candidate-feed slice consumed by this transition.
    pub slice_index: u16,
    /// Shared Egg outcome.
    pub outcome: u8,
    /// Exact Egg quantity transferred from the sell reservation to the buyer.
    pub quantity: u64,
    /// Exact collateral atoms transferred from buyer cash to seller cash.
    pub consideration_atoms: u64,
    /// Buy reservation envelope released from the Position's reserved subset.
    pub buyer_reserved_release: u64,
}

/// Immutable inputs to one direct full-slice settlement.
///
/// `buy_order` and `sell_order` must be read from the frozen page selected by
/// the candidate slice.  The account-plane wrapper proves that provenance;
/// this pure seam independently proves every economic and identity join.
pub(super) struct DirectFullSliceInput<'a> {
    pub epoch: &'a EpochAccount,
    pub candidate: &'a CandidateRecord,
    pub candidate_feed: &'a [u8],
    pub page_index: u16,
    pub slice_index: u16,
    pub buy_order: &'a OrderSlot,
    pub sell_order: &'a OrderSlot,
    pub buyer_position: &'a PositionAccount,
    pub seller_position: &'a PositionAccount,
    pub buyer_reservation: &'a ReservationAccount,
    pub seller_reservation: &'a ReservationAccount,
    pub receipt: &'a SettlementReceiptAccount,
}

/// Verify one already-selected, already-entitled direct full slice.
///
/// The authority chain is candidate feed -> exact pairing slice -> two frozen
/// order definitions -> two exact ACTIVE reservations -> one pre-frozen
/// receipt.  The page is evidence for the order definitions, never value
/// authority.  This function writes nothing and is therefore the atomic
/// precondition for [`apply_direct_full_slice`].
#[inline(never)]
pub(super) fn prepare_direct_full_slice(
    input: &DirectFullSliceInput<'_>,
) -> Outcome<DirectFullSlicePlan> {
    input.epoch.validate()?;
    input.candidate.validate()?;
    input.candidate.binds_epoch(input.epoch)?;
    require(
        input.epoch.phase == EPOCH_PHASE_CLEARED
            && input.candidate.status == CANDIDATE_STATUS_SELECTED,
        ClutchError::NotActive,
    )?;

    let feed = verify_candidate_feed(input.candidate_feed)?;
    bind_feed(&feed, input.candidate, input.epoch)?;
    let slice = slice_at(input.candidate_feed, &feed, input.slice_index)?;
    let (buy_index, sell_index) = match (slice.buy_ref, slice.sell_ref) {
        (LegRef::Order(buy), LegRef::Order(sell)) => (buy, sell),
        _ => return Err(CodecError::InvalidEnum.into()),
    };
    require(buy_index != sell_index, ClutchError::MismatchedState)?;

    // V1 consumption is same-page.  Order ids are positional, so binding the
    // feed indices to canonical ids makes an index/order substitution a red
    // check rather than an assumption.
    let buy_page = usize::from(buy_index) / MAX_ORDERS_PER_PAGE;
    let sell_page = usize::from(sell_index) / MAX_ORDERS_PER_PAGE;
    require(
        buy_page == usize::from(input.page_index) && sell_page == usize::from(input.page_index),
        ClutchError::MismatchedState,
    )?;

    let (buy, sell) = match (input.buy_order, input.sell_order) {
        (OrderSlot::Single(buy), OrderSlot::Single(sell)) => (buy, sell),
        _ => return Err(CodecError::InvalidEnum.into()),
    };
    buy.validate()?;
    sell.validate()?;
    require(
        buy.side == 0
            && sell.side == 1
            && buy.owner != sell.owner
            && buy.outcome == sell.outcome
            && buy.outcome == slice.outcome
            && buy.order_id == canonical_order_id(u64::from(buy_index) + 1)
            && sell.order_id == canonical_order_id(u64::from(sell_index) + 1),
        ClutchError::MismatchedState,
    )?;

    // This seam is intentionally full-fill and one-to-one.  A candidate that
    // split either order across receipts needs cumulative entitlement state;
    // accepting it here would double-spend an ACTIVE reservation.
    let buy_fill = fill_at(input.candidate_feed, &feed, buy_index)?;
    let sell_fill = fill_at(input.candidate_feed, &feed, sell_index)?;
    require(
        slice.quantity != 0
            && slice.quantity == buy_fill
            && slice.quantity == sell_fill
            && slice.quantity == buy.quantity
            && slice.quantity == sell.quantity,
        ClutchError::MismatchedState,
    )?;

    let price = input.candidate.prices[usize::from(slice.outcome)];
    require(
        price <= buy.limit && price >= sell.limit,
        ClutchError::MismatchedState,
    )?;
    let consideration_price_units = u128::from(slice.quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(input.epoch.price_scale != 0, ClutchError::MismatchedState)?;
    let scale = u128::from(input.epoch.price_scale);
    require(
        consideration_price_units % scale == 0,
        ClutchError::MismatchedState,
    )?;
    let consideration_atoms = u64::try_from(consideration_price_units / scale)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;

    validate_position_pair(input, buy, sell)?;
    validate_reservation(
        input.buyer_reservation,
        input.epoch,
        input.page_index,
        input.buy_order,
        input.buyer_position,
    )?;
    validate_reservation(
        input.seller_reservation,
        input.epoch,
        input.page_index,
        input.sell_order,
        input.seller_position,
    )?;
    // Fees need a frozen policy preimage and a named recipient.  Until that
    // exists, only a signed zero-fee envelope can cross this seam.
    require(
        input.buyer_reservation.max_fee_atoms == 0 && input.seller_reservation.max_fee_atoms == 0,
        ClutchError::AuthorizationUnavailable,
    )?;

    let buy_plan = ReservationPlan::for_order(
        input.buy_order,
        input.epoch.outcome_count,
        input.epoch.price_scale,
        0,
    )?;
    let sell_plan = ReservationPlan::for_order(
        input.sell_order,
        input.epoch.outcome_count,
        input.epoch.price_scale,
        0,
    )?;
    require(
        buy_plan.cash_atoms == input.buyer_reservation.initial_cash_atoms
            && buy_plan.internal == input.buyer_reservation.initial_internal
            && buy_plan.order_kind == input.buyer_reservation.order_kind
            && buy_plan.side == input.buyer_reservation.side
            && sell_plan.cash_atoms == input.seller_reservation.initial_cash_atoms
            && sell_plan.internal == input.seller_reservation.initial_internal
            && sell_plan.order_kind == input.seller_reservation.order_kind
            && sell_plan.side == input.seller_reservation.side
            && input.buyer_reservation.remaining_cash_atoms >= consideration_atoms
            && input.seller_reservation.remaining_internal[usize::from(slice.outcome)]
                == slice.quantity,
        ClutchError::MismatchedState,
    )?;

    input.receipt.validate()?;
    input.receipt.binds_candidate(input.candidate)?;
    let expected_sequence = u64::from(input.slice_index)
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        input.receipt.leg_kind == RECEIPT_LEG_DIRECT
            && input.receipt.buy_order_id == buy.order_id
            && input.receipt.sell_order_id == sell.order_id
            && input.receipt.slice_index == input.slice_index
            && input.receipt.outcome == slice.outcome
            && input.receipt.quantity == slice.quantity
            && input.receipt.price == price
            && input.receipt.consideration_price_units == consideration_price_units
            && input.receipt.sequence == expected_sequence
            && input.receipt.settled_quantity == 0
            && input.receipt.consumed_flags == 0,
        ClutchError::MismatchedState,
    )?;

    // Decide every post-state arithmetic operation before any caller writes.
    input
        .buyer_position
        .cash_atoms
        .checked_sub(consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    input
        .buyer_position
        .reserved_cash_atoms
        .checked_sub(input.buyer_reservation.remaining_cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    input.buyer_position.internal[usize::from(slice.outcome)]
        .checked_add(slice.quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    input
        .seller_position
        .cash_atoms
        .checked_add(consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    Ok(DirectFullSlicePlan {
        slice_index: input.slice_index,
        outcome: slice.outcome,
        quantity: slice.quantity,
        consideration_atoms,
        buyer_reserved_release: input.buyer_reservation.remaining_cash_atoms,
    })
}

#[inline(never)]
fn validate_position_pair(
    input: &DirectFullSliceInput<'_>,
    buy: &clutch_solana_layout::OrderRecord,
    sell: &clutch_solana_layout::OrderRecord,
) -> Outcome<()> {
    input.buyer_position.validate()?;
    input.seller_position.validate()?;
    require(
        input.buyer_position.close_state == 0
            && input.seller_position.close_state == 0
            && input.buyer_position.market == input.epoch.market
            && input.seller_position.market == input.epoch.market
            && input.buyer_position.owner == buy.owner
            && input.seller_position.owner == sell.owner,
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn validate_reservation(
    reservation: &ReservationAccount,
    epoch: &EpochAccount,
    page_index: u16,
    order: &OrderSlot,
    position: &PositionAccount,
) -> Outcome<()> {
    reservation.validate()?;
    require(
        reservation.state == RESERVATION_STATE_ACTIVE
            && reservation.market == epoch.market
            && reservation.epoch == epoch.epoch
            && reservation.owner == order.owner()
            && reservation.owner == position.owner
            && reservation.order_id == order.order_id()
            && reservation.position_generation == position.generation
            && reservation.order_generation == order.generation()
            && reservation.page_index == page_index
            && reservation.terms == epoch.terms
            && reservation.price_grid == epoch.price_grid
            && reservation.policy == epoch.policy
            && reservation.outcome_count == epoch.outcome_count,
        ClutchError::MismatchedState,
    )
}

/// Commit an already-verified direct full slice with no remaining fallible
/// operation.
///
/// Callers must obtain `plan` from [`prepare_direct_full_slice`] over these
/// exact prestates.  The account-plane wrapper does that and writes all five
/// accounts in one Solana instruction; a runtime refusal therefore rolls the
/// whole transaction back.
pub(super) fn apply_direct_full_slice(
    buyer_position: &mut PositionAccount,
    seller_position: &mut PositionAccount,
    buyer_reservation: &mut ReservationAccount,
    seller_reservation: &mut ReservationAccount,
    receipt: &mut SettlementReceiptAccount,
    plan: DirectFullSlicePlan,
) {
    let outcome = usize::from(plan.outcome);
    buyer_position.cash_atoms -= plan.consideration_atoms;
    buyer_position.reserved_cash_atoms -= plan.buyer_reserved_release;
    buyer_position.internal[outcome] += plan.quantity;
    seller_position.cash_atoms += plan.consideration_atoms;

    for reservation in [buyer_reservation, seller_reservation] {
        reservation.remaining_cash_atoms = 0;
        reservation.remaining_internal = [0; MAX_OUTCOMES];
        reservation.release_generation = 0;
        reservation.state = RESERVATION_STATE_CONSUMED;
    }
    receipt.settled_quantity = receipt.quantity;
    receipt.consumed_flags =
        RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED;
}

/// Read the two order definitions named by one direct slice from one frozen
/// page.
///
/// V1 deliberately refuses tombstones and cross-page pairs.  The first rule
/// keeps relation indices identical to positional page slots; the second keeps
/// the account list fixed.  The page is checked against the selected epoch but
/// remains evidence only: the candidate slice and reservations authorize the
/// transfer.
#[inline(never)]
pub(super) fn load_same_page_direct_orders(
    page_bytes: &[u8],
    feed_bytes: &[u8],
    epoch: &EpochAccount,
    page_index: u16,
    slice_index: u16,
) -> Outcome<(OrderSlot, OrderSlot)> {
    let header = stream::verify_page(page_bytes)?;
    require(
        header.frozen == 1
            && header.tombstone_count == 0
            && header.market == epoch.market
            && header.epoch == epoch.epoch
            && header.order_set == epoch.order_set
            && header.set_order_count == epoch.order_count
            && header.page_count == epoch.page_count
            && header.page_index == page_index,
        ClutchError::MismatchedState,
    )?;
    let feed = CandidateFeedHeader::decode(feed_bytes)?;
    let slice = slice_at(feed_bytes, &feed, slice_index)?;
    let (buy_index, sell_index) = match (slice.buy_ref, slice.sell_ref) {
        (LegRef::Order(buy), LegRef::Order(sell)) => (buy, sell),
        _ => return Err(CodecError::InvalidEnum.into()),
    };
    let base = usize::from(page_index)
        .checked_mul(MAX_ORDERS_PER_PAGE)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let buy_global = usize::from(buy_index);
    let sell_global = usize::from(sell_index);
    require(
        buy_global >= base
            && buy_global < base + MAX_ORDERS_PER_PAGE
            && sell_global >= base
            && sell_global < base + MAX_ORDERS_PER_PAGE,
        ClutchError::MismatchedState,
    )?;
    let buy_local = buy_global - base;
    let sell_local = sell_global - base;
    let mut buy = OrderSlot::Empty;
    let mut sell = OrderSlot::Empty;
    let mut cursor = stream::OrderSlotCursor::new(page_bytes)?;
    let mut index = 0usize;
    while let Some(slot) = cursor.next_slot() {
        let slot = slot?;
        if index == buy_local {
            buy = slot;
        }
        if index == sell_local {
            sell = slot;
        }
        index += 1;
    }
    require(
        buy.is_live() && sell.is_live(),
        ClutchError::MismatchedState,
    )?;
    Ok((buy, sell))
}

/// Ranked prerequisites which keep the on-chain relation transition closed.
///
/// Order is dependency order, not severity.  A caller must not skip an earlier
/// item by fabricating the fact needed by a later one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Historical full-lifecycle STOP ledger, exercised by unit tests.
pub(super) enum SettlementBlocker {
    /// Candidate-set closure/selection is not yet an on-chain transition.
    CandidateSelection,
    /// CandidateFeed/ClearWork have no permissionless authenticated lifecycle.
    CandidateFeedInitialization,
    /// Receipts/pot are not created as complete pre-resolution entitlements.
    EntitlementFreeze,
    /// The frozen book lacks a complete reservation-set commitment.
    ReservationSetClosure,
    /// Partial/multi-slice orders need cumulative per-order consumption state.
    PartialFillLedger,
    /// Nonzero fees need a policy preimage and exact recipient.
    FrozenPolicyFees,
    /// Virtual split/merge legs need a funded FinalPot transition.
    VirtualPot,
    /// No terminal sweep proves every reservation/receipt/pot is empty once.
    TerminalClosure,
}

/// The exact dependency order of the remaining settlement work.
#[allow(dead_code)] // Historical full-lifecycle STOP ledger, exercised by unit tests.
pub(super) const SETTLEMENT_BLOCKERS: [SettlementBlocker; 8] = [
    SettlementBlocker::CandidateSelection,
    SettlementBlocker::CandidateFeedInitialization,
    SettlementBlocker::EntitlementFreeze,
    SettlementBlocker::ReservationSetClosure,
    SettlementBlocker::PartialFillLedger,
    SettlementBlocker::FrozenPolicyFees,
    SettlementBlocker::VirtualPot,
    SettlementBlocker::TerminalClosure,
];

/// A tiny executable state machine for the part which is safe today.
///
/// It can bind one byte-verified preflight, idempotently.  It cannot enter a
/// relation-verified or entitlement phase: [`advance_relation`] returns the
/// first ranked blocker without changing a byte.  This makes the current STOP
/// an executable property instead of a comment beside a success-shaped API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FailClosedCheckpoint {
    bound: Option<PreflightFacts>,
}

#[allow(dead_code)] // The narrow live seam starts after this preflight checkpoint.
impl FailClosedCheckpoint {
    pub const NEW: Self = Self { bound: None };

    /// Bind once; an exact replay is idempotent and a conflicting replay fails.
    #[allow(dead_code)]
    pub fn bind(&mut self, facts: PreflightFacts) -> Result<(), CodecError> {
        match self.bound {
            None => {
                self.bound = Some(facts);
                Ok(())
            }
            Some(before) if before == facts => Ok(()),
            Some(_) => Err(CodecError::MismatchedBinding),
        }
    }

    /// Refuse the first relation step and leave the checkpoint unchanged.
    pub fn advance_relation(&mut self) -> Result<(), SettlementBlocker> {
        // An unbound invocation cannot even claim the first prerequisite was
        // reached; the same blocker is returned because the production wire
        // has no new error allocation for this research checkpoint.
        Err(SETTLEMENT_BLOCKERS[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        clearing::{
            bind_order_set, init_candidate_feed, init_clear_work, write_fill, write_slice_at,
            CandidateFeedHeader, PairingSlice, CANDIDATE_FEED_FLAG_SLICES_DECLARED,
        },
        reservation::ReservationPlan,
        stream::{append_slot, frozen_set_commitment, init_page, seal_page},
        CandidateRecord, OrderRecord, OrderSlot, PositionAccount, SettlementReceiptAccount,
        CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED, MAX_OUTCOMES, RECEIPT_LEG_DIRECT,
        RELATION_VERSION,
    };

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn order(rank: u64, owner: u8) -> OrderRecord {
        OrderRecord {
            owner: h(owner),
            order_id: canonical_order_id(rank),
            outcome: (rank as u8 - 1) & 1,
            side: (rank as u8 - 1) & 1,
            quantity: 10,
            limit: 5_000,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 9,
        }
    }

    struct Fixture {
        epoch: [u8; account_len::EPOCH],
        candidate: [u8; account_len::CANDIDATE],
        feed: [u8; account_len::CANDIDATE_FEED],
        work: [u8; account_len::CLEAR_WORK],
        page: [u8; account_len::ORDER_PAGE],
        market: Hash32,
        epoch_id: Hash32,
    }

    fn fixture() -> Fixture {
        let market = h(1);
        let epoch_id = canonical_epoch_id(market, 4);
        let mut page = [0; account_len::ORDER_PAGE];
        init_page(&mut page, market, epoch_id, 0, 1, 5).unwrap();
        append_slot(&mut page, OrderSlot::Single(order(1, 0x20))).unwrap();
        append_slot(&mut page, OrderSlot::Single(order(2, 0x21))).unwrap();
        let (order_set, slot_count) = frozen_set_commitment(&[&page]).unwrap();
        seal_page(&mut page, order_set, slot_count).unwrap();

        let epoch_account = EpochAccount {
            epoch: epoch_id,
            market,
            book: h(2),
            terms: h(3),
            price_grid: h(4),
            policy: h(5),
            order_set,
            first_order_id: canonical_order_id(1),
            last_order_id: canonical_order_id(2),
            epoch_index: 4,
            relation_version: RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: 7,
            owner_count: 2,
            page_count: 1,
            order_count: 2,
            outcome_count: 2,
            phase: EPOCH_PHASE_FROZEN,
            stored_bump: 6,
            flags: 0,
        };
        let mut epoch = [0; account_len::EPOCH];
        epoch_account.encode(&mut epoch).unwrap();

        let prices = {
            let mut values = [0; MAX_OUTCOMES];
            values[0] = 5_000;
            values[1] = 5_000;
            values
        };
        let mut candidate_account = CandidateRecord {
            candidate: Hash32::ZERO,
            epoch: epoch_id,
            market,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 20,
            limit_surplus_price_units: 0,
            churn: 0,
            submitted_slot: 99,
            distinct_owners: 2,
            order_len: 2,
            outcome_count: 2,
            status: CANDIDATE_STATUS_SUBMITTED,
            stored_bump: 7,
            flags: 0,
        };
        candidate_account.candidate = candidate_account.recomputed_candidate_digest().unwrap();
        let mut candidate = [0; account_len::CANDIDATE];
        candidate_account.encode(&mut candidate).unwrap();

        let feed_header = CandidateFeedHeader {
            candidate: candidate_account.candidate,
            epoch: epoch_id,
            market,
            order_set,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 20,
            limit_surplus_price_units: 0,
            claimed_digest: 123,
            churn: 0,
            declared_slices: 0,
            distinct_owners: 2,
            order_len: 2,
            outcome_count: 2,
            stored_bump: 8,
            flags: 0,
        };
        let mut feed = [0; account_len::CANDIDATE_FEED];
        init_candidate_feed(&mut feed, &feed_header).unwrap();

        let mut work = [0; account_len::CLEAR_WORK];
        init_clear_work(&mut work, market, epoch_id, candidate_account.candidate, 9).unwrap();

        Fixture {
            epoch,
            candidate,
            feed,
            work,
            page,
            market,
            epoch_id,
        }
    }

    fn preflight(f: &Fixture) -> Result<PreflightFacts, CodecError> {
        verify_preflight(&PreflightInput {
            epoch_bytes: &f.epoch,
            candidate_bytes: &f.candidate,
            feed_bytes: &f.feed,
            clear_work_bytes: &f.work,
            pages: &[&f.page],
            intent_market: f.market,
            intent_epoch: f.epoch_id,
            intent_page: 0,
        })
    }

    #[test]
    fn complete_page_set_candidate_feed_and_checkpoint_bind() {
        let f = fixture();
        let facts = preflight(&f).unwrap();
        assert_eq!(facts.market, f.market);
        assert_eq!(facts.epoch, f.epoch_id);
        assert_eq!(facts.slot_count, 2);
        assert_eq!(facts.live_order_count, 2);
        assert_eq!(facts.page_count, 1);
        assert_eq!(facts.page_cursor, 0);
    }

    #[test]
    fn page_inclusion_candidate_and_checkpoint_tampering_refuse() {
        let f = fixture();

        let mut wrong_page = f.page;
        let last = wrong_page.len() - 1;
        wrong_page[last] ^= 1;
        assert_eq!(
            verify_preflight(&PreflightInput {
                epoch_bytes: &f.epoch,
                candidate_bytes: &f.candidate,
                feed_bytes: &f.feed,
                clear_work_bytes: &f.work,
                pages: &[&wrong_page],
                intent_market: f.market,
                intent_epoch: f.epoch_id,
                intent_page: 0,
            }),
            Err(CodecError::NonCanonicalPadding)
        );

        let mut wrong_feed = f.feed;
        // Reframe a valid feed against a different order set; the header is
        // internally valid but cannot bind this epoch.
        let mut header = CandidateFeedHeader::decode(&wrong_feed).unwrap();
        header.order_set = h(0xaa);
        init_candidate_feed(&mut wrong_feed, &header).unwrap();
        assert_eq!(
            verify_preflight(&PreflightInput {
                epoch_bytes: &f.epoch,
                candidate_bytes: &f.candidate,
                feed_bytes: &wrong_feed,
                clear_work_bytes: &f.work,
                pages: &[&f.page],
                intent_market: f.market,
                intent_epoch: f.epoch_id,
                intent_page: 0,
            }),
            Err(CodecError::MismatchedBinding)
        );

        let mut wrong_work = [0; account_len::CLEAR_WORK];
        let candidate = CandidateRecord::decode(&f.candidate).unwrap();
        init_clear_work(
            &mut wrong_work,
            f.market,
            f.epoch_id,
            candidate.candidate,
            9,
        )
        .unwrap();
        bind_order_set(&mut wrong_work, h(0xbb), 1).unwrap();
        assert_eq!(
            verify_preflight(&PreflightInput {
                epoch_bytes: &f.epoch,
                candidate_bytes: &f.candidate,
                feed_bytes: &f.feed,
                clear_work_bytes: &wrong_work,
                pages: &[&f.page],
                intent_market: f.market,
                intent_epoch: f.epoch_id,
                intent_page: 0,
            }),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn a_selected_candidate_cannot_reenter_verification() {
        let mut f = fixture();
        let mut candidate = CandidateRecord::decode(&f.candidate).unwrap();
        candidate.status = CANDIDATE_STATUS_SELECTED;
        candidate.encode(&mut f.candidate).unwrap();
        assert_eq!(preflight(&f), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn tombstone_cardinality_stays_a_fail_closed_semantic_owner_stop() {
        let mut f = fixture();
        // The frozen page is rebuilt with one retirement.  Epoch order_count is
        // still two populated slots; the relation feed has one live order.
        let mut page = [0; account_len::ORDER_PAGE];
        init_page(&mut page, f.market, f.epoch_id, 0, 1, 5).unwrap();
        let first = order(1, 0x20);
        append_slot(&mut page, OrderSlot::Single(first)).unwrap();
        append_slot(&mut page, OrderSlot::Single(order(2, 0x21))).unwrap();
        clutch_solana_layout::stream::write_tombstone(&mut page, first.order_id, first.owner, 2)
            .unwrap();
        let (order_set, slots) = frozen_set_commitment(&[&page]).unwrap();
        seal_page(&mut page, order_set, slots).unwrap();
        f.page = page;

        let mut epoch = EpochAccount::decode(&f.epoch).unwrap();
        epoch.order_set = order_set;
        epoch.encode(&mut f.epoch).unwrap();

        let mut candidate = CandidateRecord::decode(&f.candidate).unwrap();
        candidate.order_len = 1;
        candidate.candidate = candidate.recomputed_candidate_digest().unwrap();
        candidate.encode(&mut f.candidate).unwrap();
        // Rebind otherwise-valid feed/work to the live candidate; the epoch's
        // semantic owner still refuses `1 != 2`, before an adapter can invent a
        // cardinality convention.
        let mut feed = CandidateFeedHeader::decode(&f.feed).unwrap();
        feed.order_len = 1;
        feed.order_set = order_set;
        feed.candidate = feed.recomputed_candidate_digest().unwrap();
        init_candidate_feed(&mut f.feed, &feed).unwrap();
        init_clear_work(&mut f.work, f.market, f.epoch_id, feed.candidate, 9).unwrap();

        assert_eq!(preflight(&f), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn fail_closed_checkpoint_is_idempotent_and_refusal_atomic() {
        let facts = preflight(&fixture()).unwrap();
        let mut checkpoint = FailClosedCheckpoint::NEW;
        checkpoint.bind(facts).unwrap();
        let after_first = checkpoint;
        checkpoint.bind(facts).unwrap();
        assert_eq!(checkpoint, after_first, "exact replay is idempotent");

        let mut conflicting = facts;
        conflicting.page_cursor = 1;
        assert_eq!(
            checkpoint.bind(conflicting),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(checkpoint, after_first, "conflict writes nothing");

        assert_eq!(
            checkpoint.advance_relation(),
            Err(SettlementBlocker::CandidateSelection)
        );
        assert_eq!(checkpoint, after_first, "blocked advance writes nothing");
    }

    #[test]
    fn ranked_blockers_keep_relation_and_entitlement_phases_unreachable() {
        assert_eq!(SETTLEMENT_BLOCKERS.len(), 8);
        assert_eq!(
            SETTLEMENT_BLOCKERS[0],
            SettlementBlocker::CandidateSelection
        );
        assert_eq!(
            SETTLEMENT_BLOCKERS[1],
            SettlementBlocker::CandidateFeedInitialization
        );
        assert_eq!(SETTLEMENT_BLOCKERS[7], SettlementBlocker::TerminalClosure);
    }

    struct DirectFixture {
        epoch: EpochAccount,
        candidate: CandidateRecord,
        feed: [u8; account_len::CANDIDATE_FEED],
        buy: OrderSlot,
        sell: OrderSlot,
        buyer_position: PositionAccount,
        seller_position: PositionAccount,
        buyer_reservation: ReservationAccount,
        seller_reservation: ReservationAccount,
        receipt: SettlementReceiptAccount,
    }

    impl DirectFixture {
        fn input(&self) -> DirectFullSliceInput<'_> {
            DirectFullSliceInput {
                epoch: &self.epoch,
                candidate: &self.candidate,
                candidate_feed: &self.feed,
                page_index: 0,
                slice_index: 0,
                buy_order: &self.buy,
                sell_order: &self.sell,
                buyer_position: &self.buyer_position,
                seller_position: &self.seller_position,
                buyer_reservation: &self.buyer_reservation,
                seller_reservation: &self.seller_reservation,
                receipt: &self.receipt,
            }
        }
    }

    fn direct_fixture() -> DirectFixture {
        let market = h(0x31);
        let epoch_id = canonical_epoch_id(market, 7);
        let buy_owner = h(0x41);
        let sell_owner = h(0x42);
        let terms = h(0x51);
        let grid = h(0x52);
        let policy = h(0x53);
        let order_set = h(0x54);
        let buy = OrderSlot::Single(OrderRecord {
            owner: buy_owner,
            order_id: canonical_order_id(1),
            outcome: 0,
            side: 0,
            quantity: 4,
            limit: 5_000,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 7,
        });
        let sell = OrderSlot::Single(OrderRecord {
            owner: sell_owner,
            order_id: canonical_order_id(2),
            outcome: 0,
            side: 1,
            quantity: 4,
            limit: 5_000,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 7,
        });
        let epoch = EpochAccount {
            epoch: epoch_id,
            market,
            book: h(0x55),
            terms,
            price_grid: grid,
            policy,
            order_set,
            first_order_id: canonical_order_id(1),
            last_order_id: canonical_order_id(2),
            epoch_index: 7,
            relation_version: RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: 9,
            owner_count: 2,
            page_count: 1,
            order_count: 2,
            outcome_count: 2,
            phase: EPOCH_PHASE_CLEARED,
            stored_bump: 3,
            flags: 0,
        };
        let mut prices = [0; MAX_OUTCOMES];
        prices[0] = 5_000;
        prices[1] = 5_000;
        let mut candidate = CandidateRecord {
            candidate: Hash32::ZERO,
            epoch: epoch_id,
            market,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 8,
            limit_surplus_price_units: 0,
            churn: 0,
            submitted_slot: 80,
            distinct_owners: 2,
            order_len: 2,
            outcome_count: 2,
            status: CANDIDATE_STATUS_SELECTED,
            stored_bump: 4,
            flags: 0,
        };
        candidate.candidate = candidate.recomputed_candidate_digest().unwrap();
        let header = CandidateFeedHeader {
            candidate: candidate.candidate,
            epoch: epoch_id,
            market,
            order_set,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 8,
            limit_surplus_price_units: 0,
            claimed_digest: 11,
            churn: 0,
            declared_slices: 1,
            distinct_owners: 2,
            order_len: 2,
            outcome_count: 2,
            stored_bump: 5,
            flags: CANDIDATE_FEED_FLAG_SLICES_DECLARED,
        };
        let mut feed = [0; account_len::CANDIDATE_FEED];
        init_candidate_feed(&mut feed, &header).unwrap();
        write_fill(&mut feed, 0, 4).unwrap();
        write_fill(&mut feed, 1, 4).unwrap();
        write_slice_at(
            &mut feed,
            0,
            &PairingSlice {
                buy_ref: LegRef::Order(0),
                sell_ref: LegRef::Order(1),
                outcome: 0,
                quantity: 4,
            },
        )
        .unwrap();

        let buyer_position = PositionAccount {
            market,
            owner: buy_owner,
            generation: 0,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 10,
            reserved_cash_atoms: 2,
            stored_bump: 6,
            close_state: 0,
        };
        let seller_position = PositionAccount {
            market,
            owner: sell_owner,
            generation: 0,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: 7,
            close_state: 0,
        };
        let buy_plan = ReservationPlan::for_order(&buy, 2, 10_000, 0).unwrap();
        let sell_plan = ReservationPlan::for_order(&sell, 2, 10_000, 0).unwrap();
        let buyer_reservation = ReservationAccount::active(
            market,
            epoch_id,
            buy_owner,
            buy.order_id(),
            grid,
            terms,
            policy,
            0,
            buy.generation(),
            0,
            8,
            buy_plan,
        )
        .unwrap();
        let seller_reservation = ReservationAccount::active(
            market,
            epoch_id,
            sell_owner,
            sell.order_id(),
            grid,
            terms,
            policy,
            0,
            sell.generation(),
            0,
            9,
            sell_plan,
        )
        .unwrap();
        let receipt = SettlementReceiptAccount {
            epoch: epoch_id,
            market,
            candidate: candidate.candidate,
            buy_order_id: buy.order_id(),
            sell_order_id: sell.order_id(),
            consideration_price_units: 20_000,
            quantity: 4,
            settled_quantity: 0,
            price: 5_000,
            sequence: 1,
            slice_index: 0,
            outcome: 0,
            leg_kind: RECEIPT_LEG_DIRECT,
            consumed_flags: 0,
            stored_bump: 10,
            flags: 0,
        };
        DirectFixture {
            epoch,
            candidate,
            feed,
            buy,
            sell,
            buyer_position,
            seller_position,
            buyer_reservation,
            seller_reservation,
            receipt,
        }
    }

    #[test]
    fn direct_full_slice_couples_both_assets_and_releases_residual_once() {
        let mut f = direct_fixture();
        let plan = prepare_direct_full_slice(&f.input()).unwrap();
        assert_eq!(plan.consideration_atoms, 2);
        let cash_before = f.buyer_position.cash_atoms + f.seller_position.cash_atoms;
        let claims_before = f.buyer_position.internal[0]
            + f.seller_position.internal[0]
            + f.seller_reservation.remaining_internal[0];
        apply_direct_full_slice(
            &mut f.buyer_position,
            &mut f.seller_position,
            &mut f.buyer_reservation,
            &mut f.seller_reservation,
            &mut f.receipt,
            plan,
        );
        assert_eq!(f.buyer_position.cash_atoms, 8);
        assert_eq!(f.buyer_position.reserved_cash_atoms, 0);
        assert_eq!(f.buyer_position.internal[0], 4);
        assert_eq!(f.seller_position.cash_atoms, 2);
        assert!(f.buyer_reservation.remaining_is_zero());
        assert!(f.seller_reservation.remaining_is_zero());
        assert_eq!(f.buyer_reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(f.seller_reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(f.receipt.settled_quantity, 4);
        assert_eq!(
            f.receipt.consumed_flags,
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED
        );
        assert_eq!(
            f.buyer_position.cash_atoms + f.seller_position.cash_atoms,
            cash_before
        );
        assert_eq!(
            f.buyer_position.internal[0] + f.seller_position.internal[0],
            claims_before
        );
        assert_eq!(
            prepare_direct_full_slice(&f.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn stale_partial_cross_outcome_and_fee_cases_refuse_without_a_write() {
        let mut stale = direct_fixture();
        stale.candidate.status = CANDIDATE_STATUS_SUBMITTED;
        let before = stale.buyer_position;
        assert_eq!(
            prepare_direct_full_slice(&stale.input()),
            Err(Refusal::Adapter(ClutchError::NotActive))
        );
        assert_eq!(stale.buyer_position, before);

        let mut cross = direct_fixture();
        if let OrderSlot::Single(ref mut sell) = cross.sell {
            sell.outcome = 1;
        }
        let before = cross.seller_reservation;
        assert_eq!(
            prepare_direct_full_slice(&cross.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(cross.seller_reservation, before);

        let mut partial = direct_fixture();
        write_slice_at(
            &mut partial.feed,
            0,
            &PairingSlice {
                buy_ref: LegRef::Order(0),
                sell_ref: LegRef::Order(1),
                outcome: 0,
                quantity: 2,
            },
        )
        .unwrap();
        let before = partial.receipt;
        assert_eq!(
            prepare_direct_full_slice(&partial.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(partial.receipt, before);

        let mut fee = direct_fixture();
        fee.buyer_reservation.max_fee_atoms = 1;
        fee.buyer_reservation.initial_cash_atoms = 3;
        fee.buyer_reservation.remaining_cash_atoms = 3;
        fee.buyer_position.reserved_cash_atoms = 3;
        fee.buyer_reservation.validate().unwrap();
        assert_eq!(
            prepare_direct_full_slice(&fee.input()),
            Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
        );
    }
}
