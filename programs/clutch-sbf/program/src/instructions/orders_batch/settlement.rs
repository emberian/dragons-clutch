//! Settlement preflight, the generalized entitled consumption seam, and the
//! truthful blocker ledger.
//!
//! Since T2-8 the selection and entitlement authorities are live: the walk
//! (tags 51-53) verifies, `FinalizeSelection` (57) selects, and the
//! entitlement freeze (58-59) creates the per-slice
//! [`SettlementReceiptAccount`] entitlements from the SELECTED candidate's
//! feed against the complete digest-verified page set, flipping both
//! referenced reservations `ACTIVE → ENTITLED`.  Consumption therefore no
//! longer re-derives page provenance: [`prepare_entitled_direct_slice`]
//! consumes one entitled receipt against its two `ENTITLED` single-Egg
//! reservations — the receipt is the one-shot latch, the reservations carry
//! the exact frozen envelope, and every economic move is the V2 seam's exact
//! math (full fill, zero fee, exactly divisible consideration) generalized to
//! any frozen general book.  Portfolio full pairs consume through
//! `clutch_solana_layout::portfolio_settlement::{prepare,apply}_full_pair` in
//! the account-plane wrapper, not here.
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

/// The exact post-state scalars for one entitled direct slice.
///
/// This deliberately represents only a full, direct, one-to-one fill between
/// two single-Egg orders.  No field can describe a partial slice, a portfolio
/// leg (those consume through the layout crate's full-pair seam), a virtual
/// leg, a fee, or a rounded consideration, so those cases cannot drift into
/// the implementation by an unchecked branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::instructions) struct EntitledDirectSlicePlan {
    /// Shared Egg outcome.
    pub(in crate::instructions) outcome: u8,
    /// Exact Egg quantity transferred from the sell reservation to the buyer.
    pub(in crate::instructions) quantity: u64,
    /// Exact collateral atoms transferred from buyer cash to seller cash.
    pub(in crate::instructions) consideration_atoms: u64,
    /// Buy reservation envelope released from the Position's reserved subset.
    ///
    /// The buyer's unspent `release - consideration` turns back into free
    /// cash implicitly, which is the buyer-side price-improvement refund.
    pub(in crate::instructions) buyer_reserved_release: u64,
    /// Seller Egg atoms above the transferred quantity, refunded to the
    /// seller's Position — the V3 Settle precedent's seller-remainder leg
    /// (structurally zero for a full-fill single, stated as the formula).
    pub(in crate::instructions) seller_remainder: u64,
}

/// Immutable inputs to one entitled direct-slice settlement.
///
/// No page and no feed: the entitlement freeze (tags 58-59) already verified
/// this receipt's slice against the complete digest-verified frozen page set
/// — full fills, one slice per single order, price inside both limits, exact
/// divisibility — and stamped both reservations `ENTITLED`.  What remains
/// here is the exact one-shot consumption: the receipt is the latch, the
/// reservations carry the frozen envelope, and every economic move is
/// checked against them.
pub(super) struct EntitledDirectSliceInput<'a> {
    pub epoch: &'a EpochAccount,
    pub candidate: &'a CandidateRecord,
    pub buyer_position: &'a PositionAccount,
    pub seller_position: &'a PositionAccount,
    pub buyer_reservation: &'a ReservationAccount,
    pub seller_reservation: &'a ReservationAccount,
    pub receipt: &'a SettlementReceiptAccount,
}

/// Verify one already-selected, already-entitled direct slice.
///
/// The authority chain is CLEARED epoch -> SELECTED candidate -> entitled
/// receipt -> two exact `ENTITLED` reservations.  This function writes
/// nothing and is therefore the atomic precondition for
/// [`apply_entitled_direct_slice`].
#[inline(never)]
pub(super) fn prepare_entitled_direct_slice(
    input: &EntitledDirectSliceInput<'_>,
) -> Outcome<EntitledDirectSlicePlan> {
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

    // The buy envelope is cash-only and must cover the consideration; the
    // sell envelope holds Egg on exactly the receipt's outcome and nothing
    // stray, and its atoms above the transferred quantity refund to the
    // seller (the V3 Settle precedent's remainder leg).
    require(
        input.buyer_reservation.remaining_internal == [0; MAX_OUTCOMES]
            && input.buyer_reservation.remaining_cash_atoms >= consideration_atoms
            && input.seller_reservation.remaining_cash_atoms == 0,
        ClutchError::MismatchedState,
    )?;
    let mut stray = 0usize;
    while stray < MAX_OUTCOMES {
        require(
            stray == outcome || input.seller_reservation.remaining_internal[stray] == 0,
            ClutchError::MismatchedState,
        )?;
        stray += 1;
    }
    let seller_remainder = input.seller_reservation.remaining_internal[outcome]
        .checked_sub(input.receipt.quantity)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;

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

    Ok(EntitledDirectSlicePlan {
        outcome: input.receipt.outcome,
        quantity: input.receipt.quantity,
        consideration_atoms,
        buyer_reserved_release: input.buyer_reservation.remaining_cash_atoms,
        seller_remainder,
    })
}

/// One `ENTITLED` single-Egg reservation, bound to its Position, the epoch's
/// frozen coordinates, and the receipt's order id — untouched envelope and
/// zero fee, exactly as the entitlement freeze left it.
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
            && reservation.order_kind == ORDER_KIND_SINGLE
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
            && reservation.remaining_cash_atoms == reservation.initial_cash_atoms
            && reservation.remaining_internal == reservation.initial_internal
            && position.close_state == 0
            && position.market == epoch.market,
        ClutchError::MismatchedState,
    )
}

/// Commit an already-verified entitled direct slice with no remaining
/// fallible operation.
///
/// Callers must obtain `plan` from [`prepare_entitled_direct_slice`] over
/// these exact prestates.  The account-plane wrapper does that and writes all
/// five accounts in one Solana instruction; a runtime refusal therefore rolls
/// the whole transaction back.  Both consumed reservations keep their
/// `initial_*` envelope intact — the persisted CONSUMED account *is* the
/// archive of the exact consumed amounts, standing until the TerminalClosure
/// blocker retires.
pub(in crate::instructions) fn apply_entitled_direct_slice(
    buyer_position: &mut PositionAccount,
    seller_position: &mut PositionAccount,
    buyer_reservation: &mut ReservationAccount,
    seller_reservation: &mut ReservationAccount,
    receipt: &mut SettlementReceiptAccount,
    plan: EntitledDirectSlicePlan,
) {
    let outcome = usize::from(plan.outcome);
    buyer_position.cash_atoms -= plan.consideration_atoms;
    buyer_position.reserved_cash_atoms -= plan.buyer_reserved_release;
    buyer_position.internal[outcome] += plan.quantity;
    seller_position.cash_atoms += plan.consideration_atoms;
    seller_position.internal[outcome] += plan.seller_remainder;

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
#[allow(dead_code)] // Executable record; the ledger test pins it.
pub(super) const RETIRED_SETTLEMENT_BLOCKERS: [SettlementBlocker; 5] = [
    SettlementBlocker::FrozenPolicyPreimage,
    SettlementBlocker::FullWidthRelationDomain,
    SettlementBlocker::CandidateWindowClosure,
    SettlementBlocker::EntitlementFreeze,
    SettlementBlocker::GeneralReservationSetClosure,
];

/// The exact dependency order of the remaining settlement work.
///
/// * `PartialFillLedger` — the entitlement freeze refuses any slice that is
///   not a full one-to-one fill of both its ends (`NotYetImplemented`).
/// * `VirtualPot` — the freeze refuses any verified summary carrying
///   `virtual_split`/`virtual_merge`, and — same family — any summary whose
///   rounding pot is nonzero, because the exact-only consumption seam cannot
///   fund one.
/// * `TerminalClosure` — nothing yet proves every reservation, receipt, and
///   pot consumed exactly once and reclaims their rent; consumed reservations
///   persist as their own archive, and a lapsed epoch's ACTIVE reservations
///   stand under the same open row (release requires OPEN, and no
///   post-freeze/lapse release or expiry path exists yet).
#[allow(dead_code)] // Executable record; the ledger test pins it.
pub(super) const SETTLEMENT_BLOCKERS: [SettlementBlocker; 3] = [
    SettlementBlocker::PartialFillLedger,
    SettlementBlocker::VirtualPot,
    SettlementBlocker::TerminalClosure,
];

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
        // the standing tail is exactly the honest remainder: partial fills,
        // virtual pots, and terminal closure.
        assert_eq!(RETIRED_SETTLEMENT_BLOCKERS.len(), 5);
        assert_eq!(SETTLEMENT_BLOCKERS.len(), 3);
        assert_eq!(
            RETIRED_SETTLEMENT_BLOCKERS[3],
            SettlementBlocker::EntitlementFreeze
        );
        assert_eq!(
            SETTLEMENT_BLOCKERS,
            [
                SettlementBlocker::PartialFillLedger,
                SettlementBlocker::VirtualPot,
                SettlementBlocker::TerminalClosure
            ]
        );
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
        fn input(&self) -> EntitledDirectSliceInput<'_> {
            EntitledDirectSliceInput {
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
        // The entitlement freeze's poststate: the untouched envelope, ENTITLED.
        buyer_reservation.state = RESERVATION_STATE_ENTITLED;
        seller_reservation.state = RESERVATION_STATE_ENTITLED;
        buyer_reservation.validate().unwrap();
        seller_reservation.validate().unwrap();
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
        let plan = prepare_entitled_direct_slice(&f.input()).unwrap();
        assert_eq!(plan.consideration_atoms, 2);
        assert_eq!(plan.seller_remainder, 0);
        let cash_before = f.buyer_position.cash_atoms + f.seller_position.cash_atoms;
        let claims_before = f.buyer_position.internal[0]
            + f.seller_position.internal[0]
            + f.seller_reservation.remaining_internal[0];
        let buyer_initial = f.buyer_reservation.initial_cash_atoms;
        apply_entitled_direct_slice(
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
            prepare_entitled_direct_slice(&f.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn stale_unentitled_cross_outcome_and_fee_cases_refuse_without_a_write() {
        let mut stale = direct_fixture();
        stale.candidate.status = CANDIDATE_STATUS_SUBMITTED;
        // v3: an unselected record carries no verified tie digest.
        stale.candidate.score_digest = Hash32::ZERO;
        let before = stale.buyer_position;
        assert_eq!(
            prepare_entitled_direct_slice(&stale.input()),
            Err(Refusal::Adapter(ClutchError::NotActive))
        );
        assert_eq!(stale.buyer_position, before);

        // A reservation the freeze never entitled cannot be consumed.
        let mut unentitled = direct_fixture();
        unentitled.buyer_reservation.state = RESERVATION_STATE_ACTIVE;
        let before = unentitled.buyer_reservation;
        assert_eq!(
            prepare_entitled_direct_slice(&unentitled.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(unentitled.buyer_reservation, before);

        // A receipt naming an outcome the sell envelope does not hold refuses
        // on the stray-compartment sweep.
        let mut cross = direct_fixture();
        cross.receipt.outcome = 1;
        let before = cross.seller_reservation;
        assert_eq!(
            prepare_entitled_direct_slice(&cross.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(cross.seller_reservation, before);

        // A receipt claiming more than the entitled envelope refuses.
        let mut over = direct_fixture();
        over.receipt.quantity = 6;
        over.receipt.consideration_price_units = 30_000;
        let before = over.receipt;
        assert_eq!(
            prepare_entitled_direct_slice(&over.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(over.receipt, before);

        let mut fee = direct_fixture();
        fee.buyer_reservation.max_fee_atoms = 1;
        fee.buyer_reservation.initial_cash_atoms = 3;
        fee.buyer_reservation.remaining_cash_atoms = 3;
        fee.buyer_position.reserved_cash_atoms = 3;
        fee.buyer_reservation.validate().unwrap();
        assert_eq!(
            prepare_entitled_direct_slice(&fee.input()),
            Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
        );
    }
}
