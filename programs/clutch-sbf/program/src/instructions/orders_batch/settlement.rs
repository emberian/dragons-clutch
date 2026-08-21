//! Settlement preflight, the generalized entitled consumption seam, and the
//! truthful blocker ledger.
//!
//! Since T2-8 the selection and entitlement authorities are live: the walk
//! (tags 51-53) verifies, `FinalizeSelection` (57) selects, and the
//! entitlement freeze (58-59) creates the per-slice
//! [`SettlementReceiptAccount`] entitlements from the SELECTED candidate's
//! feed against the complete digest-verified page set, flipping both
//! referenced reservations `ACTIVE → ENTITLED` and stamping each with its
//! whole order's entitled total.  Consumption therefore no longer re-derives
//! page provenance: [`prepare_entitled_slice_consumption`] consumes one
//! entitled receipt against its two stamped `ENTITLED` reservations — the
//! receipt is the one-shot latch, the reservations carry the exact frozen
//! envelope *and* the cumulative per-order ledger, and every economic move is
//! the V2 seam's exact math (zero fee, exactly divisible consideration)
//! generalized from "full one-to-one fills only" to per-slice consumption with
//! per-order completion.  A full fill is now the one-slice special case.
//! Exclusive portfolio full pairs keep consuming atomically through
//! `clutch_solana_layout::portfolio_settlement::{prepare,apply}_full_pair` in
//! the account-plane wrapper, not here; their reservations carry no ledger and
//! this seam refuses them.
//!
//! [`verify_preflight`] is the historical byte-level preflight kept compiled
//! and tested: it binds one submitted candidate feed, one checkpoint header,
//! and the complete frozen page set, and is not a settlement verdict.
//! [`prepare_direct_submission`] constructs the exact `SUBMITTED`
//! candidate/feed proposal for the narrow two-order book; it stops before
//! relation verification, selection, Epoch phase change, or receipt freeze.

use clutch_solana_layout::{
    canonical_order_set_id,
    clearing::{
        verify_candidate_feed, verify_clear_work, CandidateFeedHeader, ClearWorkHeader, LegRef,
        PairingSlice, CANDIDATE_FEED_FLAG_SLICES_DECLARED, CLEAR_WORK_STATUS_COMPLETE,
    },
    reservation::{
        ReservationAccount, ReservationPlan, RESERVATION_STATE_ACTIVE,
        RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED,
    },
    stream, CandidateRecord, CodecError, EpochAccount, Hash32, OrderSlot, PositionAccount,
    PriceGridAccount, SettlementReceiptAccount, CANDIDATE_STATUS_SELECTED,
    CANDIDATE_STATUS_SUBMITTED, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, MAX_OUTCOMES,
    ORDER_KIND_SINGLE, RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT,
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
    /// Live relation orders: a fold over the digest-verified page headers of
    /// the complete frozen set, which [`CandidateRecord::binds_epoch`] binds
    /// exactly to the candidate's `order_len`.
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

    // These are the exact bytes the set binding above just digest-verified,
    // so the header fold below is [`CandidateRecord::binds_epoch`]'s caller
    // contract for `live_order_count` discharged, not a claim re-read.
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
    candidate.binds_epoch(&epoch, live_order_count)?;

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
/* Deterministic narrow candidate submission                                */
/* ------------------------------------------------------------------------ */

/// The complete bytes, except PDA bumps, of one narrow submitted candidate.
///
/// Score fields and the relation's 128-bit digest are deliberately zero.  The
/// frozen Epoch does not carry the `FrozenPolicyV1` preimage or the specified
/// Hash32-to-relation-domain mapping needed to recompute them.  Consequently
/// this is a proposal with status `SUBMITTED`, never a verification result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectSubmissionPlan {
    /// Candidate record written to the canonical Candidate PDA.
    pub candidate: CandidateRecord,
    /// Header written to the canonical CandidateFeed PDA.
    pub feed: CandidateFeedHeader,
    /// Fill at live order index zero.
    pub fill_zero: u64,
    /// Fill at live order index one.
    pub fill_one: u64,
    /// The only explicit direct pairing slice.
    pub slice: PairingSlice,
}

impl DirectSubmissionPlan {
    /// Bind account-local PDA bumps after the content identity has selected
    /// both addresses. Bumps are outside both content digests.
    pub fn bind_bumps(&mut self, candidate_bump: u8, feed_bump: u8) {
        self.candidate.stored_bump = candidate_bump;
        self.feed.stored_bump = feed_bump;
    }
}

/// Immutable inputs to the deterministic two-order submission constructor.
pub(super) struct DirectSubmissionInput<'a> {
    pub epoch: &'a EpochAccount,
    pub grid: &'a PriceGridAccount,
    pub page_bytes: &'a [u8],
    pub page_index: u16,
    pub reservation_zero: &'a ReservationAccount,
    pub reservation_one: &'a ReservationAccount,
    pub submitted_slot: u64,
}

/// Construct one exact, funded direct candidate from an authenticated page.
///
/// This accepts only a one-page, two-order, two-outcome book with two distinct
/// owners, opposite single-Egg sides, equal quantities and equal interior
/// limits, no minimum-fill/AON condition, no tombstone, no fee headroom, and
/// two untouched ACTIVE reservations.  Those restrictions make the proposal's
/// coordinates, fills, and single explicit slice a total function of frozen
/// state. They do *not* make it the best valid submitted candidate.
#[inline(never)]
pub(super) fn prepare_direct_submission(
    input: &DirectSubmissionInput<'_>,
) -> Outcome<DirectSubmissionPlan> {
    input.epoch.validate()?;
    input.grid.validate()?;
    require(
        input.epoch.phase == EPOCH_PHASE_FROZEN
            && input.epoch.page_count == 1
            && input.epoch.order_count == 2
            && input.epoch.owner_count == 2
            && input.epoch.outcome_count == 2
            && input.page_index == 0
            && input.grid.grid == input.epoch.price_grid
            && input.grid.price_scale == input.epoch.price_scale,
        ClutchError::MismatchedState,
    )?;

    let header = stream::verify_page_on_grid(input.page_bytes, input.grid)?;
    // One page is the complete frozen set in this narrow constructor. The page
    // digest was just recomputed from every slot; fold that verified digest
    // directly rather than running the general multi-page verifier a second
    // time over the same bytes.
    let recomputed_order_set =
        canonical_order_set_id(header.market, header.epoch, 1, 2, &[header.page_digest]);
    require(
        header.frozen == 1
            && header.page_index == 0
            && header.page_count == 1
            && header.order_count == 2
            && header.tombstone_count == 0
            && header.set_order_count == 2
            && header.market == input.epoch.market
            && header.epoch == input.epoch.epoch
            && header.order_set == input.epoch.order_set
            && recomputed_order_set == input.epoch.order_set
            && header.first_order_id == input.epoch.first_order_id
            && header.last_order_id == input.epoch.last_order_id,
        ClutchError::MismatchedState,
    )?;

    let mut cursor = stream::OrderSlotCursor::new(input.page_bytes)?;
    let zero = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let one = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let (order_zero, order_one) = match (zero, one) {
        (OrderSlot::Single(zero), OrderSlot::Single(one)) => (zero, one),
        _ => return Err(CodecError::InvalidEnum.into()),
    };
    order_zero.validate()?;
    order_one.validate()?;
    require(
        order_zero.side != order_one.side
            && order_zero.owner != order_one.owner
            && order_zero.outcome == order_one.outcome
            && order_zero.quantity == order_one.quantity
            && order_zero.limit == order_one.limit
            && order_zero.limit != 0
            && order_zero.limit < input.epoch.price_scale
            && order_zero.minimum_fill == 0
            && order_one.minimum_fill == 0
            && order_zero.flags == 0
            && order_one.flags == 0
            && order_zero.expiry_epoch >= input.epoch.epoch_index
            && order_one.expiry_epoch >= input.epoch.epoch_index,
        ClutchError::MismatchedState,
    )?;

    // Both active outcome prices must be members of the frozen grid. The
    // traded outcome's price is the common limit; simplex closure determines
    // the other and leaves no price coordinate to the submitter.
    let complement = input
        .epoch
        .price_scale
        .checked_sub(order_zero.limit)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    input.grid.tick_of(order_zero.limit)?;
    input.grid.tick_of(complement)?;

    validate_submission_reservation(
        input.reservation_zero,
        input.epoch,
        input.page_index,
        &OrderSlot::Single(order_zero),
    )?;
    validate_submission_reservation(
        input.reservation_one,
        input.epoch,
        input.page_index,
        &OrderSlot::Single(order_one),
    )?;

    let mut prices = [0u64; MAX_OUTCOMES];
    prices[usize::from(order_zero.outcome)] = order_zero.limit;
    prices[1usize - usize::from(order_zero.outcome)] = complement;
    let mut candidate = CandidateRecord {
        candidate: Hash32::ZERO,
        epoch: input.epoch.epoch,
        market: input.epoch.market,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: 0,
        // A submission carries no verified tie digest.
        score_digest: Hash32::ZERO,
        // Unverified claims. See the type-level comment above.
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        churn: 0,
        submitted_slot: input.submitted_slot,
        distinct_owners: 0,
        order_len: 2,
        outcome_count: 2,
        status: CANDIDATE_STATUS_SUBMITTED,
        stored_bump: 0,
        flags: 0,
    };
    candidate.candidate = candidate.recomputed_candidate_digest()?;
    candidate.validate()?;
    // `header` was digest-verified above and its recomputed one-page order-set
    // fold is the epoch's, so its live count is the whole frozen set's.
    candidate.binds_epoch(input.epoch, u16::from(header.live_count()))?;

    let feed = CandidateFeedHeader {
        candidate: candidate.candidate,
        epoch: candidate.epoch,
        market: candidate.market,
        order_set: input.epoch.order_set,
        prices,
        virtual_split: 0,
        virtual_merge: 0,
        honored_aon_mask: 0,
        weighted_direct_volume: 0,
        limit_surplus_price_units: 0,
        claimed_digest: 0,
        churn: 0,
        declared_slices: 1,
        distinct_owners: 0,
        order_len: 2,
        outcome_count: 2,
        stored_bump: 0,
        flags: CANDIDATE_FEED_FLAG_SLICES_DECLARED,
    };
    feed.validate()?;
    bind_feed(&feed, &candidate, input.epoch)?;

    let (buy_index, sell_index) = if order_zero.side == 0 { (0, 1) } else { (1, 0) };
    let slice = PairingSlice {
        buy_ref: LegRef::Order(buy_index),
        sell_ref: LegRef::Order(sell_index),
        outcome: order_zero.outcome,
        quantity: order_zero.quantity,
    };
    slice.validate(2, 2)?;
    Ok(DirectSubmissionPlan {
        candidate,
        feed,
        fill_zero: order_zero.quantity,
        fill_one: order_one.quantity,
        slice,
    })
}

#[inline(never)]
fn validate_submission_reservation(
    reservation: &ReservationAccount,
    epoch: &EpochAccount,
    page_index: u16,
    order: &OrderSlot,
) -> Outcome<()> {
    reservation.validate()?;
    let plan = ReservationPlan::for_order(order, epoch.outcome_count, epoch.price_scale, 0)?;
    require(
        reservation.state == RESERVATION_STATE_ACTIVE
            && reservation.market == epoch.market
            && reservation.epoch == epoch.epoch
            && reservation.owner == order.owner()
            && reservation.order_id == order.order_id()
            && reservation.order_generation == order.generation()
            && reservation.page_index == page_index
            && reservation.terms == epoch.terms
            && reservation.price_grid == epoch.price_grid
            && reservation.policy == epoch.policy
            && reservation.outcome_count == epoch.outcome_count
            && reservation.max_fee_atoms == 0
            && reservation.release_generation == 0
            && reservation.initial_cash_atoms == plan.cash_atoms
            && reservation.remaining_cash_atoms == plan.cash_atoms
            && reservation.initial_internal == plan.internal
            && reservation.remaining_internal == plan.internal
            && reservation.order_kind == plan.order_kind
            && reservation.side == plan.side,
        ClutchError::MismatchedState,
    )
}

/* ------------------------------------------------------------------------ */
/* Generalized entitled consumption seam (T2-8)                              */
/* ------------------------------------------------------------------------ */

/// The exact post-state scalars for consuming one entitled slice.
///
/// One slice, not one order: both ends carry a cumulative per-order ledger, so
/// a slice moves its own quantity and *completes* an end only when that end's
/// `consumed_units` reaches its stamped `entitled_units`.  Completion is
/// decided independently per end, which is what lets a buy finish while its
/// counterparty still has slices outstanding.  No field can describe a virtual
/// leg, a fee, or a rounded consideration, so those cannot drift in by an
/// unchecked branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::instructions) struct EntitledSliceConsumptionPlan {
    /// Shared Egg outcome.
    pub(in crate::instructions) outcome: u8,
    /// Exact Egg quantity transferred from the sell reservation to the buyer.
    pub(in crate::instructions) quantity: u64,
    /// Exact collateral atoms transferred from buyer cash to seller cash.
    pub(in crate::instructions) consideration_atoms: u64,
    /// Whether this slice is the buy end's last: `consumed + quantity` reaches
    /// the stamped total.
    pub(in crate::instructions) buyer_completes: bool,
    /// Buy envelope released *above* the consideration, at completion only.
    ///
    /// The buyer's unspent remainder turns back into free cash implicitly:
    /// price improvement and the unfilled refund in one number, paid once.
    pub(in crate::instructions) buyer_release_atoms: u64,
    /// Whether this slice is the sell end's last.
    pub(in crate::instructions) seller_completes: bool,
    /// Seller Egg atoms above the transferred quantity, refunded to the
    /// seller's Position at completion — the V3 Settle precedent's
    /// seller-remainder leg, now the unfilled refund of a partial sell.
    pub(in crate::instructions) seller_remainder: u64,
}

/// Immutable inputs to one entitled slice consumption.
///
/// No page and no feed: the entitlement freeze (tags 58-59) already verified
/// this receipt's slice against the complete digest-verified frozen page set
/// — real orders on both ends, the per-order totals equal to the verified
/// fills, price inside the per-outcome limits, exact divisibility — and
/// stamped both reservations `ENTITLED` with their whole orders' totals.  What
/// remains here is the exact one-shot consumption of this one slice: the
/// receipt is the latch, the reservations carry the frozen envelope and the
/// cumulative ledger, and every economic move is checked against them.
pub(super) struct EntitledSliceConsumptionInput<'a> {
    pub epoch: &'a EpochAccount,
    pub candidate: &'a CandidateRecord,
    pub buyer_position: &'a PositionAccount,
    pub seller_position: &'a PositionAccount,
    pub buyer_reservation: &'a ReservationAccount,
    pub seller_reservation: &'a ReservationAccount,
    pub receipt: &'a SettlementReceiptAccount,
}

/// Verify one already-selected, already-entitled slice consumption.
///
/// The authority chain is CLEARED epoch -> SELECTED candidate -> entitled
/// receipt -> two stamped `ENTITLED` reservations.  This function writes
/// nothing and is therefore the atomic precondition for
/// [`apply_entitled_slice_consumption`].  Every fallible step happens here;
/// the apply is total.
#[inline(never)]
pub(super) fn prepare_entitled_slice_consumption(
    input: &EntitledSliceConsumptionInput<'_>,
) -> Outcome<EntitledSliceConsumptionPlan> {
    input.epoch.validate()?;
    input.candidate.validate()?;
    /* The exact live-cardinality half of `binds_epoch` was discharged at the
     * entitlement freeze, where the complete page set is present; here the
     * candidate's own stamped `order_len` re-checks only the identity and
     * width bindings. */
    input
        .candidate
        .binds_epoch(input.epoch, u16::from(input.candidate.order_len))?;
    require(
        input.epoch.phase == EPOCH_PHASE_CLEARED
            && input.candidate.status == CANDIDATE_STATUS_SELECTED,
        ClutchError::NotActive,
    )?;

    input.receipt.validate()?;
    input.receipt.binds_candidate(input.candidate)?;
    let expected_sequence = u64::from(input.receipt.slice_index)
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        input.receipt.leg_kind == RECEIPT_LEG_DIRECT
            && input.receipt.sequence == expected_sequence
            && input.receipt.settled_quantity == 0
            && input.receipt.consumed_flags == 0
            && input.receipt.quantity != 0,
        ClutchError::MismatchedState,
    )?;
    let outcome = usize::from(input.receipt.outcome);
    require(
        input.receipt.outcome < input.epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;

    // The exact consideration: the codec already pins
    // `consideration_price_units == quantity * price`; the seam adds the
    // exact-divisibility rule (verified once at the freeze, re-required here
    // because the atoms move here).
    require(input.epoch.price_scale != 0, ClutchError::MismatchedState)?;
    let scale = u128::from(input.epoch.price_scale);
    require(
        input.receipt.consideration_price_units.is_multiple_of(scale),
        ClutchError::MismatchedState,
    )?;
    let consideration_atoms = u64::try_from(input.receipt.consideration_price_units / scale)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;

    validate_entitled_reservation(
        input.buyer_reservation,
        input.epoch,
        input.buyer_position,
        input.receipt.buy_order_id,
        0,
    )?;
    validate_entitled_reservation(
        input.seller_reservation,
        input.epoch,
        input.seller_position,
        input.receipt.sell_order_id,
        1,
    )?;
    require(
        input.buyer_position.owner != input.seller_position.owner,
        ClutchError::MismatchedState,
    )?;
    // Fees need a frozen fee base and a named recipient.  Until that exists,
    // only a signed zero-fee envelope can cross this seam.
    require(
        input.buyer_reservation.max_fee_atoms == 0 && input.seller_reservation.max_fee_atoms == 0,
        ClutchError::AuthorizationUnavailable,
    )?;

    /* The per-order ledger bound, both ends: this slice's quantity must fit
     * inside what is left of each end's stamped entitled total, and reaching
     * that total is exactly completion. */
    let buyer_consumed = input
        .buyer_reservation
        .consumed_units
        .checked_add(input.receipt.quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let seller_consumed = input
        .seller_reservation
        .consumed_units
        .checked_add(input.receipt.quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        buyer_consumed <= input.buyer_reservation.entitled_units
            && seller_consumed <= input.seller_reservation.entitled_units,
        ClutchError::AggregateClosureMismatch,
    )?;
    let buyer_completes = buyer_consumed == input.buyer_reservation.entitled_units;
    let seller_completes = seller_consumed == input.seller_reservation.entitled_units;

    // The buy envelope is cash-only and must cover the consideration; the
    // sell envelope must hold at least the transferred quantity on this
    // receipt's outcome.
    require(
        input.buyer_reservation.remaining_internal == [0; MAX_OUTCOMES]
            && input.buyer_reservation.remaining_cash_atoms >= consideration_atoms
            && input.seller_reservation.remaining_cash_atoms == 0
            && input.seller_reservation.remaining_internal[outcome] >= input.receipt.quantity,
        ClutchError::MismatchedState,
    )?;
    /* A single-Egg sell holds exactly one compartment, and a completing sell
     * of any family must have drained every *other* compartment already: the
     * refund this slice pays is the whole remaining envelope, and it is paid
     * on this receipt's outcome.  A residue on another compartment at
     * completion is refused rather than stranded — under the frozen
     * `StrictWholeOrder` portfolio policy it cannot arise, and stating it is
     * how it stays that way. */
    if input.seller_reservation.order_kind == ORDER_KIND_SINGLE || seller_completes {
        let mut stray = 0usize;
        while stray < MAX_OUTCOMES {
            require(
                stray == outcome || input.seller_reservation.remaining_internal[stray] == 0,
                ClutchError::MismatchedState,
            )?;
            stray += 1;
        }
    }
    let seller_remainder = if seller_completes {
        input.seller_reservation.remaining_internal[outcome] - input.receipt.quantity
    } else {
        0
    };
    let buyer_release_atoms = if buyer_completes {
        input.buyer_reservation.remaining_cash_atoms - consideration_atoms
    } else {
        0
    };

    // Decide every post-state arithmetic operation before any caller writes.
    let buyer_reserved_release = consideration_atoms
        .checked_add(buyer_release_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    input
        .buyer_position
        .cash_atoms
        .checked_sub(consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    input
        .buyer_position
        .reserved_cash_atoms
        .checked_sub(buyer_reserved_release)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    input.buyer_position.internal[outcome]
        .checked_add(input.receipt.quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    input
        .seller_position
        .cash_atoms
        .checked_add(consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    input.seller_position.internal[outcome]
        .checked_add(seller_remainder)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    Ok(EntitledSliceConsumptionPlan {
        outcome: input.receipt.outcome,
        quantity: input.receipt.quantity,
        consideration_atoms,
        buyer_completes,
        buyer_release_atoms,
        seller_completes,
        seller_remainder,
    })
}

/// One stamped `ENTITLED` reservation, bound to its Position, the epoch's
/// frozen coordinates, and the receipt's order id — zero fee, exactly as the
/// entitlement freeze left it.
///
/// The envelope is deliberately *not* required untouched: an earlier slice of
/// the same order may already have been consumed, which is the whole point of
/// the cumulative ledger.  A nonzero `entitled_units` is what marks this a
/// per-slice entitlement; an unstamped reservation is the atomic portfolio
/// full pair's, and it consumes through its own seam or not at all.
#[inline(never)]
fn validate_entitled_reservation(
    reservation: &ReservationAccount,
    epoch: &EpochAccount,
    position: &PositionAccount,
    order_id: Hash32,
    side: u8,
) -> Outcome<()> {
    reservation.validate()?;
    position.validate()?;
    require(
        reservation.state == RESERVATION_STATE_ENTITLED
            && reservation.entitled_units != 0
            && reservation.side == side
            && reservation.market == epoch.market
            && reservation.epoch == epoch.epoch
            && reservation.owner == position.owner
            && reservation.order_id == order_id
            && reservation.position_generation == position.generation
            && reservation.terms == epoch.terms
            && reservation.price_grid == epoch.price_grid
            && reservation.policy == epoch.policy
            && reservation.outcome_count == epoch.outcome_count
            && reservation.release_generation == 0
            && position.close_state == 0
            && position.market == epoch.market,
        ClutchError::MismatchedState,
    )
}

/// Commit an already-verified slice consumption with no remaining fallible
/// operation.
///
/// Callers must obtain `plan` from [`prepare_entitled_slice_consumption`] over
/// these exact prestates.  The account-plane wrapper does that and writes all
/// five accounts in one Solana instruction; a runtime refusal therefore rolls
/// the whole transaction back.
///
/// The invariant this holds, per cash and per outcome, at every transaction
/// boundary: `initial = consumed-so-far + remaining + released`, with
/// `released` zero until that end completes.  A completing end pays out its
/// exact remainder — the buy end's price improvement and unfilled refund in
/// one number, the sell end's unfilled Egg vector — and takes `CONSUMED`,
/// which is exactly `consumed_units == entitled_units`.  A consumed
/// reservation keeps its `initial_*` envelope intact: the persisted account
/// *is* the archive of the exact consumed amounts, until
/// `CloseGeneralReservation` (tag 62) reclaims its rent once the page closed.
pub(in crate::instructions) fn apply_entitled_slice_consumption(
    buyer_position: &mut PositionAccount,
    seller_position: &mut PositionAccount,
    buyer_reservation: &mut ReservationAccount,
    seller_reservation: &mut ReservationAccount,
    receipt: &mut SettlementReceiptAccount,
    plan: EntitledSliceConsumptionPlan,
) {
    let outcome = usize::from(plan.outcome);
    buyer_position.cash_atoms -= plan.consideration_atoms;
    buyer_position.reserved_cash_atoms -= plan.consideration_atoms + plan.buyer_release_atoms;
    buyer_position.internal[outcome] += plan.quantity;
    seller_position.cash_atoms += plan.consideration_atoms;
    seller_position.internal[outcome] += plan.seller_remainder;

    buyer_reservation.consumed_units += plan.quantity;
    buyer_reservation.remaining_cash_atoms -=
        plan.consideration_atoms + plan.buyer_release_atoms;
    if plan.buyer_completes {
        buyer_reservation.state = RESERVATION_STATE_CONSUMED;
    }

    seller_reservation.consumed_units += plan.quantity;
    seller_reservation.remaining_internal[outcome] -= plan.quantity;
    if plan.seller_completes {
        seller_reservation.remaining_internal[outcome] -= plan.seller_remainder;
        seller_reservation.state = RESERVATION_STATE_CONSUMED;
    }

    receipt.settled_quantity = receipt.quantity;
    receipt.consumed_flags =
        RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED;
}

/// The full-lifecycle prerequisite ledger, kept truthful as rows retire.
///
/// Order is dependency order, not severity.  A caller must not skip an
/// earlier item by fabricating the fact needed by a later one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // The ledger is the executable record; tests pin both halves.
pub(super) enum SettlementBlocker {
    /// Epoch.policy is only an opaque digest; no account transports and
    /// validates every `FrozenPolicyV1` selector and fee parameter.
    FrozenPolicyPreimage,
    /// Relation V1 consumes five `u64` identity tags while this account plane
    /// owns full Hash32 identities. No lossless bridge is specified.
    FullWidthRelationDomain,
    /// A canonical submission is not a closed proposal window. Selection needs
    /// a deadline and a complete immutable commitment to all admitted candidates.
    CandidateWindowClosure,
    /// Receipts/pot are not created as complete pre-resolution entitlements.
    EntitlementFreeze,
    /// General frozen books lack a complete reservation-set commitment. The
    /// exact one-page/two-live-order submission discharges this locally only.
    GeneralReservationSetClosure,
    /// Partial/multi-slice orders need cumulative per-order consumption state.
    ///
    /// Retired: `ReservationAccount` v2 *is* that state.
    PartialFillLedger,
    /// Virtual split/merge legs need a funded FinalPot transition.
    VirtualPot,
    /// No terminal sweep proves every reservation/receipt/pot is empty once.
    TerminalClosure,
}

/// The retired rows, in the order Tier 2 discharged them.
///
/// * `FrozenPolicyPreimage` — T2-5: the pinned `GENERAL_CLEARING_POLICY_V1`
///   artifact, digest-bound to `epoch.policy` and re-derived by the walk.
/// * `FullWidthRelationDomain` — T2-5/T2-6: the zero-sentinel streamed domain
///   plus the full-width tie digest recomputed at close and selection.
/// * `CandidateWindowClosure` — T2-7: deadline-closed bounded registry and
///   `FinalizeSelection`.
/// * `EntitlementFreeze` — T2-8: `FreezeEntitlement` (58) funds the pot from
///   the verified summary and `EntitleSlice` (59) creates the per-slice
///   receipt entitlements, `ACTIVE → ENTITLED`.
/// * `GeneralReservationSetClosure` — T2-6 join 1: the pass-1 walk sweeps
///   every live order's exact ACTIVE reservation before binding.
/// * `TerminalClosure` — the R4-unlocked close wave (tags 60-67,
///   [`super::terminal_closure`]): owner-signed post-terminal release for
///   lapsed and zero-fill reservations, and the dependency-ordered rent
///   closes — receipt, reservation archive, page, pot, candidate pair,
///   checkpoint, then the epoch root — each refusing before economic zero,
///   paying exactly the recorded principal to the exact recorded payer
///   (general funding ledger; the reservation's stored owner), and burning
///   every surplus at the frozen incinerator.  Recorded residuals: an
///   account created without its optional funding ledger keeps the
///   unowned-refund blocker and stands forever, and an abandoned ACTIVE
///   reservation holds its page — and so the epoch root — open at recorded
///   rent cost.
/// * `PartialFillLedger` — the PartialFillLedger wave: `ReservationAccount`
///   schema v2 carries the per-order cumulative ledger (`entitled_units`
///   stamped once from the digest-verified feed, monotone `consumed_units`),
///   `EntitleSlice` routes by shape instead of refusing — per-slice for a
///   fragmented or partially filled single pair, a mixed single/portfolio
///   pair, and a portfolio end with several counterparties; atomic, verbatim,
///   for an exclusive portfolio full pair — and `SettlePage`'s seven-account
///   shape consumes one slice at a time, completing each end independently
///   when its stamped total is reached and releasing its exact remainder
///   once.  No new account family, no sibling policy profile, and the frozen
///   `GENERAL_CLEARING_POLICY_V1` digest unchanged: the model plane already
///   admitted partial fills and the runtime seams were the only refusal
///   sites.  Recorded residual: a witness whose *slices* do not convert while
///   every *owner* sum does stays refused and re-files under `VirtualPot`.
#[allow(dead_code)] // Executable record; the ledger test pins it.
pub(super) const RETIRED_SETTLEMENT_BLOCKERS: [SettlementBlocker; 7] = [
    SettlementBlocker::FrozenPolicyPreimage,
    SettlementBlocker::FullWidthRelationDomain,
    SettlementBlocker::CandidateWindowClosure,
    SettlementBlocker::EntitlementFreeze,
    SettlementBlocker::GeneralReservationSetClosure,
    SettlementBlocker::TerminalClosure,
    SettlementBlocker::PartialFillLedger,
];

/// The exact dependency order of the remaining settlement work.
///
/// * `VirtualPot` — the freeze refuses any verified summary carrying
///   `virtual_split`/`virtual_merge`, and — same family — any summary whose
///   rounding pot is nonzero, because the exact-only consumption seam cannot
///   fund one.  Since the PartialFillLedger wave this row also carries the
///   per-slice rounding residue: a witness whose slices do not convert
///   exactly refuses at `EntitleSlice`, even when every per-owner sum is
///   whole, because realizing it needs a funded pot rather than a wider seam.
#[allow(dead_code)] // Executable record; the ledger test pins it.
pub(super) const SETTLEMENT_BLOCKERS: [SettlementBlocker; 1] =
    [SettlementBlocker::VirtualPot];

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        clearing::{bind_order_set, init_candidate_feed, init_clear_work, CandidateFeedHeader},
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
            score_digest: Hash32::ZERO,
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
        // v3: selection carries the verified tie digest with it.
        candidate.score_digest = Hash32([0x5d; 32]);
        candidate.encode(&mut f.candidate).unwrap();
        assert_eq!(preflight(&f), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn a_cancelled_book_binds_its_live_count_and_refuses_the_slot_claim() {
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

        // The untouched fixture candidate claims the populated-slot count.
        // On the cancelled book that is one more order than the relation is
        // ever fed, and the layout binding refuses it.
        assert_eq!(preflight(&f), Err(CodecError::MismatchedBinding));

        // The candidate naming the exact live cardinality binds; preflight
        // recomputes that count from the digest-verified page header rather
        // than reading any claim.
        let mut candidate = CandidateRecord::decode(&f.candidate).unwrap();
        candidate.order_len = 1;
        candidate.candidate = candidate.recomputed_candidate_digest().unwrap();
        candidate.encode(&mut f.candidate).unwrap();
        let mut feed = CandidateFeedHeader::decode(&f.feed).unwrap();
        feed.order_len = 1;
        feed.order_set = order_set;
        feed.candidate = feed.recomputed_candidate_digest().unwrap();
        init_candidate_feed(&mut f.feed, &feed).unwrap();
        init_clear_work(&mut f.work, f.market, f.epoch_id, feed.candidate, 9).unwrap();

        let facts = preflight(&f).unwrap();
        assert_eq!(facts.slot_count, 2);
        assert_eq!(facts.live_order_count, 1);
    }

    #[test]
    fn the_blocker_ledger_records_the_retired_prefix_and_the_standing_tail() {
        // Every original row appears exactly once, retired or standing, and
        // the standing tail is exactly the honest remainder: virtual pots and
        // the per-slice rounding residue re-filed under them.  TerminalClosure
        // retired with the tag-60..67 close wave; PartialFillLedger retired
        // with the reservation-v2 ledger and the per-slice seams.
        assert_eq!(RETIRED_SETTLEMENT_BLOCKERS.len(), 7);
        assert_eq!(SETTLEMENT_BLOCKERS.len(), 1);
        assert_eq!(
            RETIRED_SETTLEMENT_BLOCKERS[3],
            SettlementBlocker::EntitlementFreeze
        );
        assert_eq!(
            RETIRED_SETTLEMENT_BLOCKERS[5],
            SettlementBlocker::TerminalClosure
        );
        assert_eq!(
            RETIRED_SETTLEMENT_BLOCKERS[6],
            SettlementBlocker::PartialFillLedger
        );
        assert_eq!(SETTLEMENT_BLOCKERS, [SettlementBlocker::VirtualPot]);
        let all = [
            SettlementBlocker::FrozenPolicyPreimage,
            SettlementBlocker::FullWidthRelationDomain,
            SettlementBlocker::CandidateWindowClosure,
            SettlementBlocker::EntitlementFreeze,
            SettlementBlocker::GeneralReservationSetClosure,
            SettlementBlocker::PartialFillLedger,
            SettlementBlocker::VirtualPot,
            SettlementBlocker::TerminalClosure,
        ];
        for blocker in all {
            let retired = RETIRED_SETTLEMENT_BLOCKERS.contains(&blocker);
            let standing = SETTLEMENT_BLOCKERS.contains(&blocker);
            assert!(retired != standing, "{blocker:?} must be exactly one");
        }
    }

    struct DirectFixture {
        epoch: EpochAccount,
        candidate: CandidateRecord,
        buy: OrderSlot,
        sell: OrderSlot,
        buyer_position: PositionAccount,
        seller_position: PositionAccount,
        buyer_reservation: ReservationAccount,
        seller_reservation: ReservationAccount,
        receipt: SettlementReceiptAccount,
    }

    impl DirectFixture {
        fn input(&self) -> EntitledSliceConsumptionInput<'_> {
            EntitledSliceConsumptionInput {
                epoch: &self.epoch,
                candidate: &self.candidate,
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
            // v3: a SELECTED record carries a verified tie digest.
            score_digest: Hash32([0x5d; 32]),
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
        let mut buyer_reservation = ReservationAccount::active(
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
        let mut seller_reservation = ReservationAccount::active(
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
        // The entitlement freeze's poststate: the untouched envelope,
        // ENTITLED, each end stamped with its whole order's entitled total.
        // Both orders are wholly filled by one slice here, so the total is
        // the slice quantity and this one slice completes both ends.
        buyer_reservation = buyer_reservation.entitled(4).unwrap();
        seller_reservation = seller_reservation.entitled(4).unwrap();
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
            buy,
            sell,
            buyer_position,
            seller_position,
            buyer_reservation,
            seller_reservation,
            receipt,
        }
    }

    struct SubmissionFixture {
        epoch: EpochAccount,
        grid: PriceGridAccount,
        page: [u8; account_len::ORDER_PAGE],
        reservation_zero: ReservationAccount,
        reservation_one: ReservationAccount,
    }

    impl SubmissionFixture {
        fn input(&self) -> DirectSubmissionInput<'_> {
            DirectSubmissionInput {
                epoch: &self.epoch,
                grid: &self.grid,
                page_bytes: &self.page,
                page_index: 0,
                reservation_zero: &self.reservation_zero,
                reservation_one: &self.reservation_one,
                submitted_slot: 77,
            }
        }
    }

    fn submission_fixture() -> SubmissionFixture {
        let direct = direct_fixture();
        let mut page = [0; account_len::ORDER_PAGE];
        init_page(&mut page, direct.epoch.market, direct.epoch.epoch, 0, 1, 5).unwrap();
        append_slot(&mut page, direct.buy).unwrap();
        append_slot(&mut page, direct.sell).unwrap();
        let (order_set, count) = frozen_set_commitment(&[&page]).unwrap();
        seal_page(&mut page, order_set, count).unwrap();

        let mut ticks = [0; clutch_solana_layout::MAX_GRID_TICKS];
        ticks[0] = 1_000;
        ticks[1] = 5_000;
        let mut grid = PriceGridAccount {
            grid: Hash32::ZERO,
            realm: h(0x61),
            price_scale: direct.epoch.price_scale,
            tick_count: 2,
            ticks,
            stored_bump: 4,
            flags: 0,
        };
        grid.grid = grid.recomputed_grid_id().unwrap();
        let mut epoch = direct.epoch;
        epoch.phase = EPOCH_PHASE_FROZEN;
        epoch.order_set = order_set;
        epoch.price_grid = grid.grid;

        let zero_plan = ReservationPlan::for_order(&direct.buy, 2, 10_000, 0).unwrap();
        let one_plan = ReservationPlan::for_order(&direct.sell, 2, 10_000, 0).unwrap();
        let reservation_zero = ReservationAccount::active(
            epoch.market,
            epoch.epoch,
            direct.buy.owner(),
            direct.buy.order_id(),
            grid.grid,
            epoch.terms,
            epoch.policy,
            0,
            direct.buy.generation(),
            0,
            8,
            zero_plan,
        )
        .unwrap();
        let reservation_one = ReservationAccount::active(
            epoch.market,
            epoch.epoch,
            direct.sell.owner(),
            direct.sell.order_id(),
            grid.grid,
            epoch.terms,
            epoch.policy,
            0,
            direct.sell.generation(),
            0,
            9,
            one_plan,
        )
        .unwrap();
        SubmissionFixture {
            epoch,
            grid,
            page,
            reservation_zero,
            reservation_one,
        }
    }

    #[test]
    fn narrow_submission_is_deterministic_funded_and_stays_unverified() {
        let f = submission_fixture();
        let first = prepare_direct_submission(&f.input()).unwrap();
        let second = prepare_direct_submission(&f.input()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.candidate.status, CANDIDATE_STATUS_SUBMITTED);
        assert_eq!(first.candidate.weighted_direct_volume, 0);
        assert_eq!(first.candidate.distinct_owners, 0);
        assert_eq!(first.feed.claimed_digest, 0);
        assert_eq!(first.fill_zero, 4);
        assert_eq!(first.fill_one, 4);
        assert_eq!(first.slice.buy_ref, LegRef::Order(0));
        assert_eq!(first.slice.sell_ref, LegRef::Order(1));
        assert_eq!(first.slice.quantity, 4);
        assert_eq!(f.epoch.phase, EPOCH_PHASE_FROZEN);
    }

    #[test]
    fn submission_refuses_reservation_substitution_and_policy_shaped_orders() {
        let f = submission_fixture();
        let swapped = DirectSubmissionInput {
            reservation_zero: &f.reservation_one,
            reservation_one: &f.reservation_zero,
            ..f.input()
        };
        assert_eq!(
            prepare_direct_submission(&swapped),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );

        let mut page = f.page;
        let mut cursor = stream::OrderSlotCursor::new(&page).unwrap();
        let mut first = cursor.next_slot().unwrap().unwrap();
        if let OrderSlot::Single(ref mut order) = first {
            order.minimum_fill = order.quantity;
            order.flags = 1;
        }
        // Rebuild a valid frozen set so the refusal is the constructor's
        // explicit policy stop, not corrupt page framing.
        init_page(&mut page, f.epoch.market, f.epoch.epoch, 0, 1, 5).unwrap();
        append_slot(&mut page, first).unwrap();
        let second = OrderSlot::Single(OrderRecord {
            owner: h(0x42),
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
        append_slot(&mut page, second).unwrap();
        let (order_set, count) = frozen_set_commitment(&[&page]).unwrap();
        seal_page(&mut page, order_set, count).unwrap();
        let mut epoch = f.epoch;
        epoch.order_set = order_set;
        let policy_shaped = DirectSubmissionInput {
            epoch: &epoch,
            page_bytes: &page,
            ..f.input()
        };
        assert_eq!(
            prepare_direct_submission(&policy_shaped),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn entitled_direct_slice_couples_both_assets_and_releases_residual_once() {
        let mut f = direct_fixture();
        let plan = prepare_entitled_slice_consumption(&f.input()).unwrap();
        assert_eq!(plan.consideration_atoms, 2);
        assert_eq!(plan.seller_remainder, 0);
        let cash_before = f.buyer_position.cash_atoms + f.seller_position.cash_atoms;
        let claims_before = f.buyer_position.internal[0]
            + f.seller_position.internal[0]
            + f.seller_reservation.remaining_internal[0];
        let buyer_initial = f.buyer_reservation.initial_cash_atoms;
        apply_entitled_slice_consumption(
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
        // The consumed reservation is its own archive: the initial envelope
        // survives, so the exact consumed amounts stay readable.
        assert_eq!(f.buyer_reservation.initial_cash_atoms, buyer_initial);
        assert_eq!(f.seller_reservation.initial_internal[0], 4);
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
        // Double consumption refuses on the exhausted receipt and the
        // CONSUMED reservations alike.
        assert_eq!(
            prepare_entitled_slice_consumption(&f.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    /// A seller of twelve, filled ten across two buyers of six and four.
    ///
    /// The whole point of the ledger in one fixture: the sell end survives the
    /// first consumption `ENTITLED` with a drawn-down envelope, completes on
    /// the second, and returns its exact two-Egg remainder once.
    struct PartialFixture {
        epoch: EpochAccount,
        candidate: CandidateRecord,
        seller_position: PositionAccount,
        seller_reservation: ReservationAccount,
        buyers: [PositionAccount; 2],
        buyer_reservations: [ReservationAccount; 2],
        receipts: [SettlementReceiptAccount; 2],
    }

    fn partial_fixture() -> PartialFixture {
        let base = direct_fixture();
        let epoch = base.epoch;
        let candidate = base.candidate;
        let market = epoch.market;
        let sell_owner = h(0x42);

        let sell = OrderSlot::Single(OrderRecord {
            owner: sell_owner,
            order_id: canonical_order_id(3),
            outcome: 0,
            side: 1,
            quantity: 12,
            limit: 5_000,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 7,
        });
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
        let seller_reservation = ReservationAccount::active(
            market,
            epoch.epoch,
            sell_owner,
            sell.order_id(),
            epoch.price_grid,
            epoch.terms,
            epoch.policy,
            0,
            sell.generation(),
            0,
            9,
            ReservationPlan::for_order(&sell, 2, 10_000, 0).unwrap(),
        )
        .unwrap()
        // Filled ten of twelve: the stamp is the fill, not the order size.
        .entitled(10)
        .unwrap();

        let mut buyers = [seller_position; 2];
        let mut buyer_reservations = [seller_reservation; 2];
        let mut receipts = [base.receipt; 2];
        for (at, quantity) in [(0usize, 6u64), (1, 4)] {
            let owner = h(0x50 + at as u8);
            let buy = OrderSlot::Single(OrderRecord {
                owner,
                order_id: canonical_order_id(1 + at as u64),
                outcome: 0,
                side: 0,
                quantity,
                // Above the clearing price on purpose: a completing buy then
                // has a real price-improvement refund to release.
                limit: 6_000,
                minimum_fill: 0,
                flags: 0,
                generation: 1,
                expiry_epoch: 7,
            });
            let plan = ReservationPlan::for_order(&buy, 2, 10_000, 0).unwrap();
            buyers[at] = PositionAccount {
                market,
                owner,
                generation: 0,
                internal: [0; MAX_OUTCOMES],
                cash_atoms: 100,
                reserved_cash_atoms: plan.cash_atoms,
                stored_bump: 6,
                close_state: 0,
            };
            buyer_reservations[at] = ReservationAccount::active(
                market,
                epoch.epoch,
                owner,
                buy.order_id(),
                epoch.price_grid,
                epoch.terms,
                epoch.policy,
                0,
                buy.generation(),
                0,
                8,
                plan,
            )
            .unwrap()
            .entitled(quantity)
            .unwrap();
            receipts[at] = SettlementReceiptAccount {
                buy_order_id: buy.order_id(),
                sell_order_id: sell.order_id(),
                consideration_price_units: u128::from(quantity) * 5_000,
                quantity,
                sequence: at as u64 + 1,
                slice_index: at as u16,
                ..base.receipt
            };
        }
        PartialFixture {
            epoch,
            candidate,
            seller_position,
            seller_reservation,
            buyers,
            buyer_reservations,
            receipts,
        }
    }

    #[test]
    fn a_partial_sell_consumes_slice_by_slice_and_returns_its_remainder_once() {
        let mut f = partial_fixture();
        let initial_internal = f.seller_reservation.initial_internal[0];
        let initial_cash: u64 = f.buyer_reservations.iter().map(|r| r.initial_cash_atoms).sum();
        let mut consumed_atoms = 0u64;
        let mut consumed_units = 0u64;

        for at in 0..2usize {
            let plan = prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
                epoch: &f.epoch,
                candidate: &f.candidate,
                buyer_position: &f.buyers[at],
                seller_position: &f.seller_position,
                buyer_reservation: &f.buyer_reservations[at],
                seller_reservation: &f.seller_reservation,
                receipt: &f.receipts[at],
            })
            .unwrap();
            // Every buy end completes on its own single slice; the sell end
            // only on the second.
            assert!(plan.buyer_completes);
            assert_eq!(plan.seller_completes, at == 1);
            assert_eq!(plan.seller_remainder, if at == 1 { 2 } else { 0 });
            // Reserved four (six) atoms, paid three (two): the completing buy
            // releases the one-atom difference back into free cash.
            assert_eq!(plan.buyer_release_atoms, 1);
            apply_entitled_slice_consumption(
                &mut f.buyers[at],
                &mut f.seller_position,
                &mut f.buyer_reservations[at],
                &mut f.seller_reservation,
                &mut f.receipts[at],
                plan,
            );
            f.buyers[at].validate().unwrap();
            f.seller_position.validate().unwrap();
            f.buyer_reservations[at].validate().unwrap();
            f.seller_reservation.validate().unwrap();
            consumed_atoms += plan.consideration_atoms;
            consumed_units += plan.quantity;

            // The pinned invariant at this transaction boundary, per outcome:
            // initial = consumed-so-far + remaining + released.
            let released = if plan.seller_completes { 2 } else { 0 };
            assert_eq!(
                initial_internal,
                consumed_units + f.seller_reservation.remaining_internal[0] + released
            );
            assert_eq!(f.seller_reservation.consumed_units, consumed_units);
            assert_eq!(
                f.seller_reservation.state,
                if at == 1 {
                    RESERVATION_STATE_CONSUMED
                } else {
                    RESERVATION_STATE_ENTITLED
                }
            );
            assert_eq!(
                f.buyer_reservations[at].state,
                RESERVATION_STATE_CONSUMED
            );
        }

        // Ten Eggs went to the buyers, two came back, twelve are accounted
        // for; the seller's cash is exactly the two considerations.
        assert_eq!(f.buyers[0].internal[0], 6);
        assert_eq!(f.buyers[1].internal[0], 4);
        assert_eq!(f.seller_position.internal[0], 2);
        assert_eq!(consumed_units, 10);
        assert_eq!(consumed_atoms, 5);
        assert_eq!(f.seller_position.cash_atoms, 5);
        assert_eq!(f.buyers[0].cash_atoms + f.buyers[1].cash_atoms, 195);
        assert_eq!(initial_cash, 7);
        assert!(f.seller_reservation.remaining_is_zero());
        for buyer in &f.buyers {
            assert_eq!(buyer.reserved_cash_atoms, 0);
        }
        // Replay on the exhausted receipts refuses.
        assert_eq!(
            prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
                epoch: &f.epoch,
                candidate: &f.candidate,
                buyer_position: &f.buyers[0],
                seller_position: &f.seller_position,
                buyer_reservation: &f.buyer_reservations[0],
                seller_reservation: &f.seller_reservation,
                receipt: &f.receipts[0],
            }),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn a_receipt_whose_consideration_does_not_convert_refuses_at_consumption() {
        // The entitlement seam already refuses an inexact slice, so a receipt
        // like this cannot be minted; the consumption seam re-requires it
        // anyway, because this is where the atoms actually move.
        let mut f = partial_fixture();
        f.receipts[0].quantity = 5;
        f.receipts[0].consideration_price_units = 5 * 5_000;
        f.receipts[0].validate().unwrap();
        assert_eq!(
            prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
                epoch: &f.epoch,
                candidate: &f.candidate,
                buyer_position: &f.buyers[0],
                seller_position: &f.seller_position,
                buyer_reservation: &f.buyer_reservations[0],
                seller_reservation: &f.seller_reservation,
                receipt: &f.receipts[0],
            }),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn a_forged_stamp_cannot_widen_a_partial_orders_admission() {
        // The sell end is stamped ten.  A forged stamp of twelve — the order
        // size rather than the fill — would let the two receipts consume the
        // whole envelope with no remainder returned.  Both the codec and the
        // seam refuse the ledger it would need.
        let mut f = partial_fixture();
        let mut forged = f.seller_reservation;
        forged.entitled_units = 12;
        // The forgery is internally consistent bytes; what refuses it is the
        // entitlement seam's recomputation, and here the closure: consuming
        // the two entitled slices can never reach the forged total, so the
        // sell end can never complete and its remainder never returns.
        assert_eq!(forged.validate(), Ok(()));
        f.seller_reservation = forged;
        for at in 0..2usize {
            let plan = prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
                epoch: &f.epoch,
                candidate: &f.candidate,
                buyer_position: &f.buyers[at],
                seller_position: &f.seller_position,
                buyer_reservation: &f.buyer_reservations[at],
                seller_reservation: &f.seller_reservation,
                receipt: &f.receipts[at],
            })
            .unwrap();
            assert!(!plan.seller_completes);
            assert_eq!(plan.seller_remainder, 0);
            apply_entitled_slice_consumption(
                &mut f.buyers[at],
                &mut f.seller_position,
                &mut f.buyer_reservations[at],
                &mut f.seller_reservation,
                &mut f.receipts[at],
                plan,
            );
        }
        // The forged stamp strands the remainder inside the reservation
        // rather than paying it out: the invariant holds, and the account is
        // still ENTITLED with two Eggs it can never release.
        assert_eq!(f.seller_reservation.state, RESERVATION_STATE_ENTITLED);
        assert_eq!(f.seller_reservation.remaining_internal[0], 2);
        assert_eq!(f.seller_position.internal[0], 0);
        assert_eq!(f.seller_reservation.consumed_units, 10);
    }

    #[test]
    fn a_slice_consumed_before_its_sibling_is_entitled_leaves_the_sibling_consumable() {
        // Out-of-order: slice one is consumed while slice two has no receipt
        // yet.  The sell end's envelope is drawn down, so the later
        // entitlement touch must not require an untouched envelope — and the
        // later consumption must still work off the same stamp.
        let mut f = partial_fixture();
        let plan = prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
            epoch: &f.epoch,
            candidate: &f.candidate,
            buyer_position: &f.buyers[0],
            seller_position: &f.seller_position,
            buyer_reservation: &f.buyer_reservations[0],
            seller_reservation: &f.seller_reservation,
            receipt: &f.receipts[0],
        })
        .unwrap();
        apply_entitled_slice_consumption(
            &mut f.buyers[0],
            &mut f.seller_position,
            &mut f.buyer_reservations[0],
            &mut f.seller_reservation,
            &mut f.receipts[0],
            plan,
        );
        assert_ne!(
            f.seller_reservation.remaining_internal,
            f.seller_reservation.initial_internal
        );
        // The stamp is unchanged, so the later slice's entitlement touch
        // re-derives it and agrees.
        assert_eq!(f.seller_reservation.requires_stamp(10), Ok(()));
        // ...and the later slice consumes off the drawn-down envelope.
        let plan = prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
            epoch: &f.epoch,
            candidate: &f.candidate,
            buyer_position: &f.buyers[1],
            seller_position: &f.seller_position,
            buyer_reservation: &f.buyer_reservations[1],
            seller_reservation: &f.seller_reservation,
            receipt: &f.receipts[1],
        })
        .unwrap();
        assert!(plan.seller_completes);
        assert_eq!(plan.seller_remainder, 2);
    }

    #[test]
    fn stale_unentitled_cross_outcome_and_fee_cases_refuse_without_a_write() {
        let mut stale = direct_fixture();
        stale.candidate.status = CANDIDATE_STATUS_SUBMITTED;
        // v3: an unselected record carries no verified tie digest.
        stale.candidate.score_digest = Hash32::ZERO;
        let before = stale.buyer_position;
        assert_eq!(
            prepare_entitled_slice_consumption(&stale.input()),
            Err(Refusal::Adapter(ClutchError::NotActive))
        );
        assert_eq!(stale.buyer_position, before);

        // A reservation the freeze never entitled cannot be consumed: it
        // carries no stamp, so it is ACTIVE and the seam refuses it.
        let mut unentitled = direct_fixture();
        unentitled.buyer_reservation.state = RESERVATION_STATE_ACTIVE;
        unentitled.buyer_reservation.entitled_units = 0;
        unentitled.buyer_reservation.validate().unwrap();
        let before = unentitled.buyer_reservation;
        assert_eq!(
            prepare_entitled_slice_consumption(&unentitled.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(unentitled.buyer_reservation, before);

        // An ENTITLED reservation with no stamp is the atomic portfolio full
        // pair's; this seam is not its consumer.
        let mut unstamped = direct_fixture();
        unstamped.buyer_reservation.entitled_units = 0;
        unstamped.buyer_reservation.validate().unwrap();
        assert_eq!(
            prepare_entitled_slice_consumption(&unstamped.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );

        // A receipt naming an outcome the sell envelope does not hold refuses
        // on the stray-compartment sweep.
        let mut cross = direct_fixture();
        cross.receipt.outcome = 1;
        let before = cross.seller_reservation;
        assert_eq!(
            prepare_entitled_slice_consumption(&cross.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(cross.seller_reservation, before);

        // A receipt claiming more than the stamped total refuses on the
        // ledger bound, before any envelope is touched.
        let mut over = direct_fixture();
        over.receipt.quantity = 6;
        over.receipt.consideration_price_units = 30_000;
        let before = over.receipt;
        assert_eq!(
            prepare_entitled_slice_consumption(&over.input()),
            Err(Refusal::Adapter(ClutchError::AggregateClosureMismatch))
        );
        assert_eq!(over.receipt, before);

        let mut fee = direct_fixture();
        fee.buyer_reservation.max_fee_atoms = 1;
        fee.buyer_reservation.initial_cash_atoms = 3;
        fee.buyer_reservation.remaining_cash_atoms = 3;
        fee.buyer_position.reserved_cash_atoms = 3;
        fee.buyer_reservation.validate().unwrap();
        assert_eq!(
            prepare_entitled_slice_consumption(&fee.input()),
            Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
        );
    }
}
