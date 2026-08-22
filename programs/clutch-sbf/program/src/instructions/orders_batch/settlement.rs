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
        ReservationAccount, ReservationPlan, RESERVATION_STATE_ACTIVE, RESERVATION_STATE_CONSUMED,
        RESERVATION_STATE_ENTITLED,
    },
    stream, CandidateRecord, CodecError, EpochAccount, FinalPotAccount, Hash32, OrderSlot,
    PositionAccount, PriceGridAccount, SettlementReceiptAccount, CANDIDATE_STATUS_SELECTED,
    CANDIDATE_STATUS_SUBMITTED, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, MAX_OUTCOMES,
    ORDER_KIND_SINGLE, POT_PHASE_OPEN, RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT, RECEIPT_LEG_SPLIT,
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
/// leg or a fee, so those cannot drift in by an unchecked branch.
///
/// The two atom legs are **separate numbers**.  Under the frozen
/// `TerminalOwnerFloor` boundary a payer's whole-order value rounds *up* to
/// collateral atoms and a payee's rounds *down*, so a book whose order values
/// are not multiples of the price scale debits strictly more than it credits.
/// The difference is [`Self::residue_price_units`], and it is not a fee, not a
/// transfer, and not held by any account: it is the conversion slack the
/// relation names `rounding_pot`, realized here by simply never crediting it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::instructions) struct EntitledSliceConsumptionPlan {
    /// Shared Egg outcome.
    pub(in crate::instructions) outcome: u8,
    /// Exact Egg quantity transferred from the sell reservation to the buyer.
    pub(in crate::instructions) quantity: u64,
    /// Collateral atoms debited from the buy end on this slice.
    pub(in crate::instructions) buyer_debit_atoms: u64,
    /// Collateral atoms credited to the sell end on this slice.
    pub(in crate::instructions) seller_credit_atoms: u64,
    /// Price units this slice leaves unallocated, drawn from the epoch pot's
    /// verified expectation.
    ///
    /// Nonzero only on a *completing* end whose whole-order value is not a
    /// multiple of the price scale: the payer's round-up excess and the
    /// payee's round-down shortfall, each realized exactly once, at the one
    /// slice that finishes that order.
    pub(in crate::instructions) residue_price_units: u128,
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
    /// The pot's exact cash after this slice, in price units, on an epoch
    /// whose selected candidate carries a virtual split; `None` leaves the
    /// field untouched, which is every churn-free epoch.
    ///
    /// The pot's cash is not a custody account and not a fee: it is the
    /// running total of collateral this epoch has debited but not credited,
    /// which sits unallocated in the market's Hoard pool exactly as the
    /// rounding residue does.  Its closed form is derived in
    /// [`pot_cash_after`], and its terminal value is `sigma * price_scale` —
    /// the collateral that backs the virtual split's mint.
    pub(in crate::instructions) pot_cash_after: Option<u128>,
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
///
/// `pot` is the epoch's [`FinalPotAccount`], present exactly when the slice
/// may realize rounding residue.  It is never a source or a sink of value: it
/// carries the *verified* residue expectation the relation computed, and each
/// completing end draws its own share down.  The pot reaching zero when the
/// last receipt consumes is the whole-plane statement that the runtime's
/// per-order conversions summed to the relation's per-owner `rounding_pot`.
pub(super) struct EntitledSliceConsumptionInput<'a> {
    pub epoch: &'a EpochAccount,
    pub candidate: &'a CandidateRecord,
    pub buyer_position: &'a PositionAccount,
    pub seller_position: &'a PositionAccount,
    pub buyer_reservation: &'a ReservationAccount,
    pub seller_reservation: &'a ReservationAccount,
    pub receipt: &'a SettlementReceiptAccount,
    pub pot: Option<&'a FinalPotAccount>,
}

/// One single-Egg order's cumulative value in price units.
///
/// A single-Egg order lives on exactly one outcome at one frozen price, so
/// `units * price` is a total function of its consumed ledger and the whole
/// per-slice conversion telescopes through it: the atoms a slice moves are the
/// difference of two conversions of *cumulative* values, never a conversion of
/// the slice's own value.  That is what makes the sum over an order's slices
/// equal the single conversion the relation performs, exactly.
///
/// A portfolio order spans several prices, so its value is not a function of
/// `units` alone; this seam keeps requiring per-slice exactness for those ends,
/// which makes the telescoping vacuous and the residue zero.
fn cumulative_value_price_units(units: u64, price: u64) -> u128 {
    // Two `u64` factors always fit a `u128` product.
    u128::from(units) * u128::from(price)
}

/// The pot's cash after one slice moved `debit` atoms in and `credit` atoms
/// out and realized `residue` price units — the closed form the whole virtual
/// join rests on.
///
/// Write `V_e` for one end's *cumulative* exact consumed value in price units
/// and `S` for the price scale.  Every consumed slice adds `q * p` to its buy
/// end and the same `q * p` to its sell end; a virtual-split slice has no sell
/// end, so summing over the slices consumed so far,
///
/// ```text
///   sum_buys V  -  sum_sells V  =  split value consumed so far.
/// ```
///
/// [`convert_leg`] debits a payer `ceil(V/S)` atoms and credits a payee
/// `floor(V/S)`, realizing the gap `r_e` exactly once, at the slice that
/// completes that end.  Therefore
///
/// ```text
///   pot_cash  =  sum (debit*S - credit*S - residue)
///             =  (sum_buys V - sum_sells V)  +  sum over still-open ends r_e
/// ```
///
/// and three facts follow, each of which this seam relies on:
///
/// * **Non-negative at every step**, because both terms are — so the pot is
///   never asked to pay out value it does not hold, and no float is needed.
/// * **Exactly `sigma * S` when the last receipt consumes**, because every
///   still-open end has closed and every split slice has paid: the split
///   serves `sigma` on every outcome (`relation_v1.rs:3830-3832`) and prices
///   lie on the scaled simplex, so `sum_i sigma * p_i = sigma * S`.
/// * **Zero at the freeze**, which is why the freeze cannot mint and this
///   seam can.
///
/// The individual step may still be negative — a slice whose payer crosses no
/// atom boundary while its payee does — so the arithmetic is checked and an
/// underflow refuses rather than wrapping.
fn pot_cash_after(
    pot_cash_price_units: u128,
    debit_atoms: u64,
    credit_atoms: u64,
    residue_price_units: u128,
    scale: u128,
) -> Outcome<u128> {
    let credited = u128::from(credit_atoms)
        .checked_mul(scale)
        .and_then(|value| value.checked_add(residue_price_units))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    u128::from(debit_atoms)
        .checked_mul(scale)
        .and_then(|value| pot_cash_price_units.checked_add(value))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
        .checked_sub(credited)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))
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
    // `consideration_price_units == quantity * price`.  Whether that value
    // converts to whole collateral atoms is *not* required here any more —
    // the conversion is per order, not per slice (see
    // [`cumulative_value_price_units`]) — but a portfolio end still needs it,
    // because its cumulative value is not a function of its consumed units.
    require(input.epoch.price_scale != 0, ClutchError::MismatchedState)?;
    let scale = u128::from(input.epoch.price_scale);
    let slice_is_exact = input
        .receipt
        .consideration_price_units
        .is_multiple_of(scale);

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

    /* The terminal-owner conversion, realized per end.  Each end's atoms are
     * the difference of two conversions of *cumulative* order value, so an
     * order's slices sum to exactly one conversion of its whole value: the
     * payer's rounds up, the payee's rounds down, and the gap is the residue
     * the completing slice hands to the pot. */
    let price = input.receipt.price;
    let (buyer_debit_atoms, buyer_residue) = convert_leg(LegConversion {
        side: ConversionSide::Payer,
        order_kind: input.buyer_reservation.order_kind,
        consumed_before: input.buyer_reservation.consumed_units,
        consumed_after: buyer_consumed,
        entitled_units: input.buyer_reservation.entitled_units,
        completes: buyer_completes,
        price,
        scale,
        slice_is_exact,
        slice_price_units: input.receipt.consideration_price_units,
    })?;
    let (seller_credit_atoms, seller_residue) = convert_leg(LegConversion {
        side: ConversionSide::Payee,
        order_kind: input.seller_reservation.order_kind,
        consumed_before: input.seller_reservation.consumed_units,
        consumed_after: seller_consumed,
        entitled_units: input.seller_reservation.entitled_units,
        completes: seller_completes,
        price,
        scale,
        slice_is_exact,
        slice_price_units: input.receipt.consideration_price_units,
    })?;
    let residue_price_units = buyer_residue
        .checked_add(seller_residue)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    /* The residue is drawn from the epoch pot's *verified* expectation, so a
     * runtime conversion the relation did not predict cannot land: the pot
     * refuses to go negative, and reaching zero exactly once every receipt has
     * consumed is the whole-plane closure.
     *
     * On an epoch whose selected candidate carries a virtual split the pot is
     * mandatory on *every* slice, direct ones included, because its cash
     * ledger only closes at `sigma * price_scale` when every slice has fed it
     * — see [`pot_cash_after`].  A churn-free epoch keeps the sealed rule
     * exactly: the pot is presented only when this slice realizes residue,
     * and its cash field is never written. */
    let churned = input.candidate.virtual_split != 0;
    let pot_cash_after = if churned || residue_price_units != 0 {
        let pot = input
            .pot
            .ok_or(Refusal::Adapter(ClutchError::AccountCount))?;
        pot.binds_candidate(input.candidate)?;
        require(
            pot.rounding_pot_price_units >= residue_price_units,
            ClutchError::AggregateClosureMismatch,
        )?;
        if churned {
            Some(pot_cash_after(
                pot.pot_cash_price_units,
                buyer_debit_atoms,
                seller_credit_atoms,
                residue_price_units,
                scale,
            )?)
        } else {
            None
        }
    } else {
        if let Some(pot) = input.pot {
            pot.binds_candidate(input.candidate)?;
        }
        None
    };

    // The buy envelope is cash-only and must cover this slice's debit; the
    // sell envelope must hold at least the transferred quantity on this
    // receipt's outcome.
    require(
        input.buyer_reservation.remaining_internal == [0; MAX_OUTCOMES]
            && input.buyer_reservation.remaining_cash_atoms >= buyer_debit_atoms
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
        input.buyer_reservation.remaining_cash_atoms - buyer_debit_atoms
    } else {
        0
    };

    // Decide every post-state arithmetic operation before any caller writes.
    let buyer_reserved_release = buyer_debit_atoms
        .checked_add(buyer_release_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    input
        .buyer_position
        .cash_atoms
        .checked_sub(buyer_debit_atoms)
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
        .checked_add(seller_credit_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    input.seller_position.internal[outcome]
        .checked_add(seller_remainder)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    Ok(EntitledSliceConsumptionPlan {
        outcome: input.receipt.outcome,
        quantity: input.receipt.quantity,
        buyer_debit_atoms,
        seller_credit_atoms,
        residue_price_units,
        buyer_completes,
        buyer_release_atoms,
        seller_completes,
        seller_remainder,
        pot_cash_after,
    })
}

/// Which way one end's whole-order value converts to collateral atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConversionSide {
    /// A buy end: the relation rounds a payer's owed value **up**, so the
    /// excess above the exact value is what the payer leaves behind.
    Payer,
    /// A sell end: the relation rounds a payee's owed value **down**, so the
    /// shortfall below the exact value is what the payee never receives.
    Payee,
}

/// One end's conversion coordinates for a single slice.
struct LegConversion {
    side: ConversionSide,
    order_kind: u8,
    consumed_before: u64,
    consumed_after: u64,
    entitled_units: u64,
    completes: bool,
    price: u64,
    scale: u128,
    slice_is_exact: bool,
    slice_price_units: u128,
}

/// Convert one end's slice into collateral atoms, plus the residue it realizes.
///
/// The realization of `RoundingBoundaryV1::TerminalOwnerFloor`
/// (`crates/clutch-batch/src/relation_v1.rs:2482-2497`): a payer's atoms are
/// `debit_units.div_ceil(scale)` and the pot takes `atoms * scale -
/// debit_units`; a payee's are `credit_units / scale` and the pot takes
/// `credit_units - atoms * scale`.  Both are conversions of the **whole
/// order's** value, so the per-slice numbers here are differences of
/// cumulative conversions and telescope to exactly that one conversion.
///
/// A portfolio end keeps the older per-slice rule — its cumulative value is
/// not a function of its consumed units — which forces per-slice exactness and
/// therefore zero residue.
#[inline(never)]
fn convert_leg(leg: LegConversion) -> Outcome<(u64, u128)> {
    if leg.order_kind != ORDER_KIND_SINGLE {
        require(leg.slice_is_exact, ClutchError::MismatchedState)?;
        let atoms = u64::try_from(leg.slice_price_units / leg.scale)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        return Ok((atoms, 0));
    }
    let before = cumulative_value_price_units(leg.consumed_before, leg.price);
    let after = cumulative_value_price_units(leg.consumed_after, leg.price);
    let (converted_before, converted_after) = match leg.side {
        ConversionSide::Payer => (before.div_ceil(leg.scale), after.div_ceil(leg.scale)),
        ConversionSide::Payee => (before / leg.scale, after / leg.scale),
    };
    let atoms = u64::try_from(
        converted_after
            .checked_sub(converted_before)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    if !leg.completes {
        return Ok((atoms, 0));
    }
    /* Completion: `after` is the whole order's value, so the residue is this
     * end's entire contribution to the epoch's rounding pot, realized once. */
    let total = cumulative_value_price_units(leg.entitled_units, leg.price);
    require(total == after, ClutchError::AggregateClosureMismatch)?;
    let residue = match leg.side {
        ConversionSide::Payer => converted_after
            .checked_mul(leg.scale)
            .and_then(|value| value.checked_sub(total))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ConversionSide::Payee => total
            .checked_sub(
                converted_after
                    .checked_mul(leg.scale)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            )
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    };
    Ok((atoms, residue))
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
///
/// The cash halves of that invariant are two numbers, not one, whenever the
/// terminal-owner conversion is inexact: the buy end is debited its
/// round-*up* and the sell end credited its round-*down*, and the difference
/// is drawn from the pot's verified residue expectation.  Nothing custodies
/// it — the atoms are simply never credited, so they stay unallocated in the
/// market's collateral pool — and the pot's field is the record of how much
/// the epoch is still expected to leave there.
#[allow(clippy::too_many_arguments)] // one argument per written account
pub(in crate::instructions) fn apply_entitled_slice_consumption(
    buyer_position: &mut PositionAccount,
    seller_position: &mut PositionAccount,
    buyer_reservation: &mut ReservationAccount,
    seller_reservation: &mut ReservationAccount,
    receipt: &mut SettlementReceiptAccount,
    pot: Option<&mut FinalPotAccount>,
    plan: EntitledSliceConsumptionPlan,
) {
    let outcome = usize::from(plan.outcome);
    buyer_position.cash_atoms -= plan.buyer_debit_atoms;
    buyer_position.reserved_cash_atoms -= plan.buyer_debit_atoms + plan.buyer_release_atoms;
    buyer_position.internal[outcome] += plan.quantity;
    seller_position.cash_atoms += plan.seller_credit_atoms;
    seller_position.internal[outcome] += plan.seller_remainder;
    if let Some(pot) = pot {
        pot.rounding_pot_price_units -= plan.residue_price_units;
        if let Some(cash) = plan.pot_cash_after {
            pot.pot_cash_price_units = cash;
        }
    }

    buyer_reservation.consumed_units += plan.quantity;
    buyer_reservation.remaining_cash_atoms -= plan.buyer_debit_atoms + plan.buyer_release_atoms;
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

/* ------------------------------------------------------------------------ */
/* The virtual join: one real end, the epoch pot on the other                */
/* ------------------------------------------------------------------------ */

/// Which half of one virtual slice this consumption is.
///
/// A virtual-split receipt consumes in **two** phases and the order is not a
/// convention: it is the only order in which the pot is never short.
///
/// * [`VirtualPhase::Pay`] moves the buy end's cumulative debit exactly as an
///   ordinary slice does, but there is no seller — so the whole
///   `debit * S - residue` lands in the pot's cash and the buyer receives
///   nothing yet.
/// * [`VirtualPhase::Deliver`] moves `quantity` Egg atoms out of the pot's
///   inventory into the buy end's Position, minting the inventory first when
///   the pot does not yet hold any.
///
/// The `SettlementReceiptAccount` consumption flags have always been three
/// separate bits; this is the first seam that sets them at two different
/// times, and it is why they are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::instructions) enum VirtualPhase {
    /// The buy end pays; the pot's cash rises.
    Pay,
    /// The pot mints if it must, then delivers.
    Deliver,
}

/// Immutable inputs to one virtual slice consumption.
pub(super) struct VirtualSliceConsumptionInput<'a> {
    pub epoch: &'a EpochAccount,
    pub candidate: &'a CandidateRecord,
    pub position: &'a PositionAccount,
    pub reservation: &'a ReservationAccount,
    pub receipt: &'a SettlementReceiptAccount,
    pub pot: &'a FinalPotAccount,
}

/// The exact post-state scalars for consuming one virtual slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::instructions) struct VirtualSliceConsumptionPlan {
    /// Which half this is.
    pub(in crate::instructions) phase: VirtualPhase,
    /// Shared Egg outcome.
    pub(in crate::instructions) outcome: u8,
    /// Egg atoms this slice moves.
    pub(in crate::instructions) quantity: u64,
    /// Collateral atoms debited from the buy end; `Pay` only.
    pub(in crate::instructions) debit_atoms: u64,
    /// Buy envelope released above the consideration, at completion only.
    pub(in crate::instructions) release_atoms: u64,
    /// Whether this slice is the buy end's last.
    pub(in crate::instructions) completes: bool,
    /// Price units this slice leaves unallocated; `Pay` only.
    pub(in crate::instructions) residue_price_units: u128,
    /// The pot's rounding expectation after this slice.
    pub(in crate::instructions) rounding_after: u128,
    /// The pot's cash after this slice, *before* any mint is paid for.
    pub(in crate::instructions) pot_cash_after: u128,
    /// Complete sets this consumption must mint before it can deliver; zero
    /// unless this is the one `Deliver` that finds the pot with no inventory.
    ///
    /// The caller pays for it by calling `split::pooled_set_transition`, which
    /// is the only route to `HoardAccount::collateral_atoms`, the kernel
    /// aggregate and the supply ledger, with every CLO-DELTA-V1 obligation
    /// intact.
    pub(in crate::instructions) mint_sets: u64,
    /// The pot's cash once the mint above has been paid for.
    pub(in crate::instructions) pot_cash_after_mint: u128,
}

/// Verify one already-entitled virtual slice consumption.
///
/// Writes nothing, and is therefore the atomic precondition for
/// [`apply_virtual_slice_consumption`] plus — on the one minting `Deliver` —
/// the `split::pooled_set_transition` call the account plane makes between
/// them.
#[inline(never)]
pub(super) fn prepare_virtual_slice_consumption(
    input: &VirtualSliceConsumptionInput<'_>,
) -> Outcome<VirtualSliceConsumptionPlan> {
    input.epoch.validate()?;
    input.candidate.validate()?;
    input
        .candidate
        .binds_epoch(input.epoch, u16::from(input.candidate.order_len))?;
    require(
        input.epoch.phase == EPOCH_PHASE_CLEARED
            && input.candidate.status == CANDIDATE_STATUS_SELECTED,
        ClutchError::NotActive,
    )?;
    /* The one direction the pot can fund.  A merge would have refused at the
     * entitlement freeze; restating it here is what keeps a hand-built
     * account list from reaching the burn half through this seam. */
    let sigma = input.candidate.virtual_split;
    require(
        sigma != 0 && input.candidate.virtual_merge == 0,
        ClutchError::NotYetImplemented,
    )?;

    input.receipt.validate()?;
    input.receipt.binds_candidate(input.candidate)?;
    let expected_sequence = u64::from(input.receipt.slice_index)
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        input.receipt.leg_kind == RECEIPT_LEG_SPLIT
            && input.receipt.sell_order_id == Hash32::ZERO
            && input.receipt.sequence == expected_sequence
            && input.receipt.quantity != 0,
        ClutchError::MismatchedState,
    )?;
    let outcome = usize::from(input.receipt.outcome);
    require(
        input.receipt.outcome < input.epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;
    require(input.epoch.price_scale != 0, ClutchError::MismatchedState)?;
    let scale = u128::from(input.epoch.price_scale);

    input.pot.binds_candidate(input.candidate)?;
    require(
        input.pot.phase == POT_PHASE_OPEN,
        ClutchError::MismatchedState,
    )?;

    /* The phase is read off the receipt's own flags, so it is a fact of
     * persisted state and never a caller's claim, and each flag latches
     * exactly once. */
    let phase = match input.receipt.consumed_flags {
        0 => VirtualPhase::Pay,
        RECEIPT_FLAG_BUY_CONSUMED => VirtualPhase::Deliver,
        _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };

    match phase {
        VirtualPhase::Pay => {
            require(
                input.receipt.settled_quantity == 0,
                ClutchError::MismatchedState,
            )?;
            validate_entitled_reservation(
                input.reservation,
                input.epoch,
                input.position,
                input.receipt.buy_order_id,
                0,
            )?;
            require(
                input.reservation.max_fee_atoms == 0,
                ClutchError::AuthorizationUnavailable,
            )?;
            let consumed = input
                .reservation
                .consumed_units
                .checked_add(input.receipt.quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            require(
                consumed <= input.reservation.entitled_units,
                ClutchError::AggregateClosureMismatch,
            )?;
            let completes = consumed == input.reservation.entitled_units;
            let (debit_atoms, residue_price_units) = convert_leg(LegConversion {
                side: ConversionSide::Payer,
                order_kind: input.reservation.order_kind,
                consumed_before: input.reservation.consumed_units,
                consumed_after: consumed,
                entitled_units: input.reservation.entitled_units,
                completes,
                price: input.receipt.price,
                scale,
                slice_is_exact: input
                    .receipt
                    .consideration_price_units
                    .is_multiple_of(scale),
                slice_price_units: input.receipt.consideration_price_units,
            })?;
            require(
                input.reservation.remaining_internal == [0; MAX_OUTCOMES]
                    && input.reservation.remaining_cash_atoms >= debit_atoms,
                ClutchError::MismatchedState,
            )?;
            require(
                input.pot.rounding_pot_price_units >= residue_price_units,
                ClutchError::AggregateClosureMismatch,
            )?;
            let release_atoms = if completes {
                input.reservation.remaining_cash_atoms - debit_atoms
            } else {
                0
            };
            let reserved_release = debit_atoms
                .checked_add(release_atoms)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            input
                .position
                .cash_atoms
                .checked_sub(debit_atoms)
                .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
            input
                .position
                .reserved_cash_atoms
                .checked_sub(reserved_release)
                .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
            /* The pot's cash rises by the whole debit less the realized
             * residue: with no seller there is nothing to credit, which is
             * exactly the `credit = 0` case of [`pot_cash_after`]. */
            let cash = pot_cash_after(
                input.pot.pot_cash_price_units,
                debit_atoms,
                0,
                residue_price_units,
                scale,
            )?;
            Ok(VirtualSliceConsumptionPlan {
                phase,
                outcome: input.receipt.outcome,
                quantity: input.receipt.quantity,
                debit_atoms,
                release_atoms,
                completes,
                residue_price_units,
                rounding_after: input.pot.rounding_pot_price_units - residue_price_units,
                pot_cash_after: cash,
                mint_sets: 0,
                pot_cash_after_mint: cash,
            })
        }
        VirtualPhase::Deliver => {
            require(
                input.receipt.settled_quantity == 0,
                ClutchError::MismatchedState,
            )?;
            /* The delivery's destination is bound the only way it can be:
             * through the reservation that names both this receipt's buy
             * order and this Position's owner.  The reservation is CONSUMED
             * once its last slice has paid, so both live states are admitted
             * — nothing is written to it here. */
            require(
                (input.reservation.state == RESERVATION_STATE_ENTITLED
                    || input.reservation.state == RESERVATION_STATE_CONSUMED)
                    && input.reservation.side == 0
                    && input.reservation.entitled_units != 0
                    && input.reservation.market == input.epoch.market
                    && input.reservation.epoch == input.epoch.epoch
                    && input.reservation.owner == input.position.owner
                    && input.reservation.order_id == input.receipt.buy_order_id
                    && input.reservation.position_generation == input.position.generation
                    && input.reservation.outcome_count == input.epoch.outcome_count
                    && input.position.close_state == 0
                    && input.position.market == input.epoch.market,
                ClutchError::MismatchedState,
            )?;
            input.reservation.validate()?;
            input.position.validate()?;

            /* The mint, and the exact condition under which it happens.
             *
             * The pot's inventory is all zero in exactly two reachable
             * states: before the mint, and after every split slice has been
             * delivered.  The split serves `sigma` on every outcome
             * (`relation_v1.rs:3830-3832`), so deliveries total `sigma` per
             * outcome and an all-zero inventory after the mint means every
             * split receipt is already exhausted — no `Deliver` can run.  A
             * second mint is therefore unreachable, and the cash test below
             * is a funding requirement rather than the uniqueness argument.
             *
             * `sigma * price_scale` is what the pot must hold: one collateral
             * atom per complete set, because prices lie on the scaled simplex
             * and the split's cost is `sigma * price_scale`
             * (`relation_v1.rs:2749-2757`).  Those atoms are already inside
             * the Hoard's token account, debited from the buyers and credited
             * to nobody, which is what makes the mint *backed* rather than
             * created. */
            let empty = input.pot.pot_internal == [0; MAX_OUTCOMES];
            let mint_sets = if empty { sigma } else { 0 };
            let cost = u128::from(mint_sets)
                .checked_mul(scale)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            let pot_cash_after_mint = input
                .pot
                .pot_cash_price_units
                .checked_sub(cost)
                .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
            let held = if empty {
                sigma
            } else {
                input.pot.pot_internal[outcome]
            };
            require(
                held >= input.receipt.quantity,
                ClutchError::AggregateClosureMismatch,
            )?;
            input.position.internal[outcome]
                .checked_add(input.receipt.quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            Ok(VirtualSliceConsumptionPlan {
                phase,
                outcome: input.receipt.outcome,
                quantity: input.receipt.quantity,
                debit_atoms: 0,
                release_atoms: 0,
                completes: false,
                residue_price_units: 0,
                rounding_after: input.pot.rounding_pot_price_units,
                pot_cash_after: input.pot.pot_cash_price_units,
                mint_sets,
                pot_cash_after_mint,
            })
        }
    }
}

/// Commit an already-verified virtual slice consumption.
///
/// On a minting `Deliver` the caller must have run
/// `split::pooled_set_transition` over `pot.pot_internal` *before* this, so
/// the inventory this moves out of already exists and is already backed.
pub(in crate::instructions) fn apply_virtual_slice_consumption(
    position: &mut PositionAccount,
    reservation: &mut ReservationAccount,
    receipt: &mut SettlementReceiptAccount,
    pot: &mut FinalPotAccount,
    plan: VirtualSliceConsumptionPlan,
) {
    let outcome = usize::from(plan.outcome);
    match plan.phase {
        VirtualPhase::Pay => {
            position.cash_atoms -= plan.debit_atoms;
            position.reserved_cash_atoms -= plan.debit_atoms + plan.release_atoms;
            reservation.consumed_units += plan.quantity;
            reservation.remaining_cash_atoms -= plan.debit_atoms + plan.release_atoms;
            if plan.completes {
                reservation.state = RESERVATION_STATE_CONSUMED;
            }
            pot.rounding_pot_price_units = plan.rounding_after;
            pot.pot_cash_price_units = plan.pot_cash_after;
            receipt.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED;
        }
        VirtualPhase::Deliver => {
            position.internal[outcome] += plan.quantity;
            pot.pot_internal[outcome] -= plan.quantity;
            pot.pot_cash_price_units = plan.pot_cash_after_mint;
            receipt.settled_quantity = receipt.quantity;
            receipt.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED
                | RECEIPT_FLAG_SELL_CONSUMED
                | RECEIPT_FLAG_SLICE_EXHAUSTED;
        }
    }
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
    /// A verified summary carrying a nonzero `rounding_pot` needs a
    /// consumption path that realizes remainder atoms.
    ///
    /// Retired: the pot carries the verified expectation and each completing
    /// order draws its own share down.
    RoundingPotRealization,
    /// Virtual split/merge legs need a funded FinalPot transition.
    ///
    /// Retired in the split direction: the pot pays, then mints, then
    /// delivers.
    VirtualPot,
    /// No terminal sweep proves every reservation/receipt/pot is empty once.
    TerminalClosure,
    /// A verified summary carrying `mu` needs a payee credit that can be
    /// deferred past its own Egg delivery.
    ///
    /// The pot's cash identity (`pot_cash_after`) is `-(merge value
    /// consumed) + pending gaps` before the burn: strictly negative once a
    /// merge slice pays, because the pot must credit a payee before it holds
    /// the complete sets whose burn would fund the credit.  The order that
    /// works is deliver-every-leg, burn, then pay — and that separates a
    /// sell end's Egg delivery from its ledger advance, which needs a
    /// `paid_units` term the v2 `ReservationAccount` schema does not carry.
    VirtualMergeCredit,
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
///   every *owner* sum does stayed refused and re-filed under the rounding
///   family — retired by the row below.
/// * `RoundingPotRealization` — the VirtualPot wave's rounding half: the
///   freeze no longer refuses a nonzero verified `rounding_pot`, it *records*
///   it.  Under `TerminalOwnerFloor` (`relation_v1.rs:2482-2497`) the pot is
///   funded by nobody outside the book — it is the payers' round-up excess
///   plus the payees' round-down shortfall — so the runtime realizes it by
///   converting each end's *cumulative* order value (up for a payer, down for
///   a payee) and simply never crediting the gap, which leaves exactly
///   `rounding_pot / price_scale` collateral atoms unallocated in the market's
///   pool.  The pot account holds no value; it holds the verified
///   expectation, every completing order draws its own share down, and its
///   reaching zero when the last receipt consumes is the whole-plane closure
///   `CloseGeneralPot` already requires.  Slices need no longer convert at
///   all, retiring the partial-fill wave's recorded residual.  Recorded
///   residual of *this* row: the relation converts per **owner**, summing an
///   owner's orders before rounding, while a reservation can only carry its
///   own order — so an epoch in which any participating owner holds two
///   filled orders *and* any order value is inexact refuses at `EntitleSlice`
///   rather than minting receipts whose residues could not sum to the
///   verified pot.  The coincidence is checked, not assumed:
///   `distinct_owners == filled_order_count`.
/// * `VirtualPot` — this wave, in the one direction the pot can fund without
///   ever holding an unbacked claim.  The derivation, because it *refutes*
///   the shape the row was filed under.  Write `V_e` for one end's cumulative
///   exact consumed value in price units: every slice adds `q * p` to its buy
///   end and the same to its sell end, and a split slice has no sell end, so
///   `sum_buys V - sum_sells V` is the split value consumed so far.  With
///   [`convert_leg`] debiting `ceil(V/S)` and crediting `floor(V/S)`, the
///   pot's cash is identically `(sum_buys V - sum_sells V) + sum of the
///   pending gaps of still-open ends` ([`pot_cash_after`]).  **At the freeze
///   that is zero**, so the freeze cannot mint: raising
///   `HoardAccount::collateral_atoms` by `sigma` there would attribute the
///   same Hoard token atoms twice, once as complete-set backing and once as
///   the buyers' `reserved_cash_atoms` — the unbacked mint the join must not
///   perform.  A lazy per-slice mint deadlocks for a second reason: the
///   kernel mints *complete sets* (`clutch_kernel::MarketState::split`, and
///   `required_collateral_for` is `max_i total_supply[i]`), so delivering `q`
///   atoms of one outcome costs `q * S` and earns only `q * p_i < q * S`.
///   The realizable order is therefore **pay, then mint, then deliver**: a
///   virtual receipt latches `BUY_CONSUMED` when its buy end pays, and
///   `SELL_CONSUMED | SLICE_EXHAUSTED` when the pot delivers.  Once every
///   split slice has paid the pot holds exactly `sigma * price_scale` price
///   units — `sum_i sigma * p_i` on the scaled simplex — of collateral that
///   is already inside the Hoard's token account and attributed to nobody, so
///   `split::pooled_set_transition` *reclassifies* it rather than creating
///   it, under the identical kernel step, ledger delta, internal bound,
///   two-term closure, collateral cap and Hoard mirror `Intent::Split` runs.
///   Deliveries total `sigma` per outcome (`relation_v1.rs:3830-3832`), so
///   the pot ends exactly empty on both terms and `CloseGeneralPot` is
///   unchanged.  Recorded residual: the merge direction, re-filed as the row
///   below rather than left inside this one.
#[allow(dead_code)] // Executable record; the ledger test pins it.
pub(super) const RETIRED_SETTLEMENT_BLOCKERS: [SettlementBlocker; 9] = [
    SettlementBlocker::FrozenPolicyPreimage,
    SettlementBlocker::FullWidthRelationDomain,
    SettlementBlocker::CandidateWindowClosure,
    SettlementBlocker::EntitlementFreeze,
    SettlementBlocker::GeneralReservationSetClosure,
    SettlementBlocker::TerminalClosure,
    SettlementBlocker::PartialFillLedger,
    SettlementBlocker::RoundingPotRealization,
    SettlementBlocker::VirtualPot,
];

/// The exact dependency order of the remaining settlement work.
///
/// * `VirtualMergeCredit` — the freeze accepts a verified summary carrying
///   `sigma` and still refuses one carrying `mu`, and the root cause is
///   **not** a settlement-seam width nor the mint authority the retired
///   `VirtualPot` row named.  The mint authority now exists
///   (`split::pooled_set_transition`, reached by the virtual `SettlePage`
///   shape) and the burn would use the same primitive in the other
///   direction.  What is missing is a *cash order*.  The pot's cash identity
///   ([`pot_cash_after`]) is `(sum_buys V - sum_sells V) + pending gaps`, and
///   a merge slice adds to `sum_sells V` alone — so the moment a merge slice
///   is credited the first term is negative, and it is negative by exactly
///   the value of the sets the pot has not burned yet.  The pot would have to
///   pay a payee before it holds the complete sets whose burn funds the
///   payment, which is the unbacked case in the other direction: releasing
///   `mu` collateral atoms the market has not yet burned claims for.
///   The order that closes is deliver-every-merge-leg, burn `mu`, then pay —
///   `merge_used[i] == mu` on every outcome (`relation_v1.rs:3830-3832`)
///   guarantees the pot reaches a whole `mu` complete sets — but it separates
///   a sell end's Egg delivery from its ledger advance, and
///   [`convert_leg`]'s payee credit is a difference of conversions of
///   `consumed_units`.  Splitting those needs a `paid_units` term the v2
///   `ReservationAccount` schema does not carry, and adding one is a
///   reservation-codec change rather than a settlement-seam change.  Ranked
///   as the schema change it is.
#[allow(dead_code)] // Executable record; the ledger test pins it.
pub(super) const SETTLEMENT_BLOCKERS: [SettlementBlocker; 1] =
    [SettlementBlocker::VirtualMergeCredit];

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        clearing::{bind_order_set, init_candidate_feed, init_clear_work, CandidateFeedHeader},
        reservation::ReservationPlan,
        stream::{append_slot, frozen_set_commitment, init_page, seal_page},
        CandidateRecord, OrderRecord, OrderSlot, PositionAccount, SettlementReceiptAccount,
        CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED, MAX_OUTCOMES, POT_PHASE_OPEN,
        RECEIPT_LEG_DIRECT, RELATION_VERSION,
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
            basis_degree: 1,
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
        // the standing tail is exactly the honest remainder.  TerminalClosure
        // retired with the tag-60..67 close wave; PartialFillLedger with the
        // reservation-v2 ledger and the per-slice seams; RoundingPotRealization
        // with the cumulative per-order conversion and the pot's drawn-down
        // expectation; VirtualPot with the pay-then-mint-then-deliver order,
        // whose mint runs through `split::pooled_set_transition`.  What stands
        // is narrower than the row it came out of and is *not* the mint
        // authority: `VirtualMergeCredit` is a reservation-schema change.
        assert_eq!(RETIRED_SETTLEMENT_BLOCKERS.len(), 9);
        assert_eq!(SETTLEMENT_BLOCKERS.len(), 1);
        assert_eq!(
            RETIRED_SETTLEMENT_BLOCKERS[8],
            SettlementBlocker::VirtualPot
        );
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
        assert_eq!(SETTLEMENT_BLOCKERS, [SettlementBlocker::VirtualMergeCredit]);
        let all = [
            SettlementBlocker::FrozenPolicyPreimage,
            SettlementBlocker::FullWidthRelationDomain,
            SettlementBlocker::CandidateWindowClosure,
            SettlementBlocker::EntitlementFreeze,
            SettlementBlocker::GeneralReservationSetClosure,
            SettlementBlocker::PartialFillLedger,
            SettlementBlocker::RoundingPotRealization,
            SettlementBlocker::VirtualPot,
            SettlementBlocker::TerminalClosure,
            SettlementBlocker::VirtualMergeCredit,
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
                pot: None,
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
            basis_degree: 1,
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

    /* -------------------------------------------------------------------- */
    /* The virtual join, on the relation's own verified sigma = 1 book       */
    /* -------------------------------------------------------------------- */

    /// The book `relation_v1_tests.rs:439-457` verifies, at this seam's scale.
    ///
    /// Two outcomes at `price_scale` 10 000, prices `[5000, 5000]`; buys `A`
    /// and `B` for two atoms each on outcomes 0 and 1 at limit 10 000; sells
    /// `C` and `D` for two atoms each at limit 5000, filled one each.  The
    /// canonical candidate is `sigma = 1` with fills `[2, 2, 1, 1]`, and the
    /// relation's own numbers are
    ///
    /// ```text
    ///   consideration  20 000 = seller_credit 10 000 + split_cost 10 000
    ///   debit_atoms 2, credit_atoms 0, rounding_pot 10 000
    ///   (2 - 0) * 10 000  ==  sigma * 10 000 + 10 000        (V8 closure)
    /// ```
    ///
    /// The fixture is the state **mid-walk**: `A`'s direct slice against `C`
    /// has consumed, so the pot already holds `C`'s round-down of 5000 as
    /// cash and half its rounding expectation is discharged.
    struct VirtualFixture {
        epoch: EpochAccount,
        candidate: CandidateRecord,
        position: PositionAccount,
        reservation: ReservationAccount,
        receipt: SettlementReceiptAccount,
        pot: FinalPotAccount,
    }

    impl VirtualFixture {
        fn input(&self) -> VirtualSliceConsumptionInput<'_> {
            VirtualSliceConsumptionInput {
                epoch: &self.epoch,
                candidate: &self.candidate,
                position: &self.position,
                reservation: &self.reservation,
                receipt: &self.receipt,
                pot: &self.pot,
            }
        }
    }

    fn virtual_fixture() -> VirtualFixture {
        let market = h(0x31);
        let epoch_id = canonical_epoch_id(market, 7);
        let buy_owner = h(0x41);
        let terms = h(0x51);
        let grid = h(0x52);
        let policy = h(0x53);
        let buy = OrderSlot::Single(OrderRecord {
            owner: buy_owner,
            order_id: canonical_order_id(1),
            outcome: 0,
            side: 0,
            quantity: 2,
            limit: 10_000,
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
            order_set: h(0x54),
            first_order_id: canonical_order_id(1),
            last_order_id: canonical_order_id(4),
            epoch_index: 7,
            relation_version: RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: 9,
            owner_count: 4,
            page_count: 1,
            order_count: 4,
            outcome_count: 2,
            basis_degree: 1,
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
            virtual_split: 1,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 4,
            limit_surplus_price_units: 0,
            score_digest: Hash32([0x5d; 32]),
            churn: 1,
            submitted_slot: 80,
            distinct_owners: 4,
            order_len: 4,
            outcome_count: 2,
            status: CANDIDATE_STATUS_SELECTED,
            stored_bump: 4,
            flags: 0,
        };
        candidate.candidate = candidate.recomputed_candidate_digest().unwrap();

        // `A` reserved two atoms at its limit and has already paid one on its
        // direct slice against `C`; one atom of price improvement is left.
        let position = PositionAccount {
            market,
            owner: buy_owner,
            generation: 0,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 9,
            reserved_cash_atoms: 1,
            stored_bump: 6,
            close_state: 0,
        };
        let plan = ReservationPlan::for_order(&buy, 2, 10_000, 0).unwrap();
        assert_eq!(plan.cash_atoms, 2);
        let mut reservation = ReservationAccount::active(
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
            plan,
        )
        .unwrap();
        reservation = reservation.entitled(2).unwrap();
        reservation.consumed_units = 1;
        reservation.remaining_cash_atoms = 1;

        let receipt = SettlementReceiptAccount {
            epoch: epoch_id,
            market,
            candidate: candidate.candidate,
            buy_order_id: buy.order_id(),
            sell_order_id: Hash32::ZERO,
            consideration_price_units: 5_000,
            quantity: 1,
            settled_quantity: 0,
            price: 5_000,
            sequence: 2,
            slice_index: 1,
            outcome: 0,
            leg_kind: RECEIPT_LEG_SPLIT,
            consumed_flags: 0,
            stored_bump: 10,
            flags: 0,
        };
        let pot = FinalPotAccount {
            epoch: epoch_id,
            market,
            candidate: candidate.candidate,
            pot_internal: [0; MAX_OUTCOMES],
            // `C`'s completing direct slice paid one whole atom in and
            // realized 5000 of the 10 000 the relation expects.
            pot_cash_price_units: 5_000,
            rounding_pot_price_units: 5_000,
            outcome_count: 2,
            phase: POT_PHASE_OPEN,
            stored_bump: 11,
            flags: 0,
        };
        VirtualFixture {
            epoch,
            candidate,
            position,
            reservation,
            receipt,
            pot,
        }
    }

    #[test]
    fn the_pot_cash_closed_form_is_debit_less_credit_less_residue() {
        // The identity every other virtual test rests on, stated directly.
        assert_eq!(pot_cash_after(5_000, 1, 0, 5_000, 10_000).unwrap(), 10_000);
        assert_eq!(pot_cash_after(0, 0, 0, 0, 10_000).unwrap(), 0);
        // A slice whose payer crosses no atom boundary while its payee does
        // takes value *out* of the pot, which is legal while it stays solvent
        // and refuses the moment it would not.
        assert_eq!(pot_cash_after(10_000, 0, 1, 0, 10_000).unwrap(), 0);
        assert_eq!(
            pot_cash_after(0, 0, 1, 0, 10_000),
            Err(Refusal::Adapter(ClutchError::AggregateClosureMismatch))
        );
    }

    #[test]
    fn a_virtual_pay_moves_the_buy_end_alone_and_never_delivers() {
        let fixture = virtual_fixture();
        let plan = prepare_virtual_slice_consumption(&fixture.input()).unwrap();
        assert_eq!(plan.phase, VirtualPhase::Pay);
        // `A`'s whole order is worth exactly one atom and it paid that atom on
        // its direct slice, so this slice debits nothing and releases the
        // price improvement.
        assert_eq!(plan.debit_atoms, 0);
        assert_eq!(plan.release_atoms, 1);
        assert!(plan.completes);
        assert_eq!(plan.residue_price_units, 0);
        assert_eq!(plan.pot_cash_after, 5_000);
        assert_eq!(plan.mint_sets, 0);

        let mut position = fixture.position;
        let mut reservation = fixture.reservation;
        let mut receipt = fixture.receipt;
        let mut pot = fixture.pot;
        apply_virtual_slice_consumption(
            &mut position,
            &mut reservation,
            &mut receipt,
            &mut pot,
            plan,
        );
        // Paid, not delivered: the buy end holds no new claim yet, and the
        // receipt latches exactly one of its three consumption bits.
        assert_eq!(position.internal, [0; MAX_OUTCOMES]);
        assert_eq!(position.reserved_cash_atoms, 0);
        assert_eq!(position.cash_atoms, 9);
        assert_eq!(reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(receipt.consumed_flags, RECEIPT_FLAG_BUY_CONSUMED);
        assert_eq!(receipt.settled_quantity, 0);
        assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
        position.validate().unwrap();
        reservation.validate().unwrap();
        receipt.validate().unwrap();
        pot.validate().unwrap();
    }

    #[test]
    fn a_virtual_deliver_mints_exactly_the_candidates_sigma_once_the_pot_is_funded() {
        let mut fixture = virtual_fixture();
        fixture.receipt.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED;
        // Every split slice has paid: the pot holds `sigma * price_scale`.
        fixture.pot.pot_cash_price_units = 10_000;
        fixture.pot.rounding_pot_price_units = 0;
        fixture.reservation.state = RESERVATION_STATE_CONSUMED;
        fixture.reservation.consumed_units = 2;
        fixture.reservation.remaining_cash_atoms = 0;
        fixture.position.reserved_cash_atoms = 0;

        let plan = prepare_virtual_slice_consumption(&fixture.input()).unwrap();
        assert_eq!(plan.phase, VirtualPhase::Deliver);
        assert_eq!(plan.mint_sets, fixture.candidate.virtual_split);
        // The mint is paid for out of collateral the pot already holds.
        assert_eq!(plan.pot_cash_after_mint, 0);

        let mut position = fixture.position;
        let mut reservation = fixture.reservation;
        let mut receipt = fixture.receipt;
        let mut pot = fixture.pot;
        // Stand in for `split::pooled_set_transition`, which is the only
        // thing allowed to write this vector on a real account plane.
        pot.pot_internal[0] = 1;
        pot.pot_internal[1] = 1;
        apply_virtual_slice_consumption(
            &mut position,
            &mut reservation,
            &mut receipt,
            &mut pot,
            plan,
        );
        assert_eq!(position.internal[0], 1);
        assert_eq!(pot.pot_internal[0], 0);
        assert_eq!(pot.pot_internal[1], 1);
        assert_eq!(pot.pot_cash_price_units, 0);
        assert_eq!(
            receipt.consumed_flags,
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED
        );
        assert_eq!(receipt.settled_quantity, receipt.quantity);
        pot.validate().unwrap();
    }

    #[test]
    fn a_virtual_deliver_before_the_whole_book_has_paid_refuses() {
        let mut fixture = virtual_fixture();
        fixture.receipt.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED;
        fixture.reservation.state = RESERVATION_STATE_CONSUMED;
        fixture.reservation.consumed_units = 2;
        fixture.reservation.remaining_cash_atoms = 0;
        fixture.position.reserved_cash_atoms = 0;
        // The pot holds 5000 of the 10 000 the mint costs: the seam refuses
        // rather than minting a set the market has not been paid for.
        assert_eq!(
            prepare_virtual_slice_consumption(&fixture.input()),
            Err(Refusal::Adapter(ClutchError::AggregateClosureMismatch))
        );
    }

    #[test]
    fn a_second_mint_is_unreachable_because_a_drained_pot_holds_no_cash() {
        // The state after the last delivery: inventory empty again, and the
        // cash spent on the one mint.  Any further `Deliver` would have to
        // re-mint, and cannot.
        let mut fixture = virtual_fixture();
        fixture.receipt.consumed_flags = RECEIPT_FLAG_BUY_CONSUMED;
        fixture.reservation.state = RESERVATION_STATE_CONSUMED;
        fixture.reservation.consumed_units = 2;
        fixture.reservation.remaining_cash_atoms = 0;
        fixture.position.reserved_cash_atoms = 0;
        fixture.pot.pot_cash_price_units = 0;
        fixture.pot.rounding_pot_price_units = 0;
        assert_eq!(
            prepare_virtual_slice_consumption(&fixture.input()),
            Err(Refusal::Adapter(ClutchError::AggregateClosureMismatch))
        );
    }

    #[test]
    fn the_virtual_seam_refuses_a_merge_candidate_and_a_churn_free_one() {
        let mut fixture = virtual_fixture();
        fixture.candidate.virtual_split = 0;
        fixture.candidate.virtual_merge = 1;
        fixture.candidate.churn = 1;
        fixture.candidate.candidate = fixture.candidate.recomputed_candidate_digest().unwrap();
        fixture.receipt.candidate = fixture.candidate.candidate;
        fixture.pot.candidate = fixture.candidate.candidate;
        assert_eq!(
            prepare_virtual_slice_consumption(&fixture.input()),
            Err(Refusal::Adapter(ClutchError::NotYetImplemented))
        );

        let mut zero = virtual_fixture();
        zero.candidate.virtual_split = 0;
        zero.candidate.churn = 0;
        zero.candidate.candidate = zero.candidate.recomputed_candidate_digest().unwrap();
        zero.receipt.candidate = zero.candidate.candidate;
        zero.pot.candidate = zero.candidate.candidate;
        assert_eq!(
            prepare_virtual_slice_consumption(&zero.input()),
            Err(Refusal::Adapter(ClutchError::NotYetImplemented))
        );
    }

    #[test]
    fn a_forged_consumption_flag_has_no_phase_and_refuses() {
        // Only two forgeries are reachable at all: the frozen receipt codec
        // ties `SLICE_EXHAUSTED` to `settled_quantity == quantity`, so every
        // flag word carrying it over an unsettled receipt is already a codec
        // fault.  That tie is also what makes the two-phase latch legal —
        // `Pay` leaves the quantity unsettled and sets only `BUY_CONSUMED`.
        for flags in [
            RECEIPT_FLAG_SELL_CONSUMED,
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED,
        ] {
            let mut fixture = virtual_fixture();
            fixture.receipt.consumed_flags = flags;
            assert_eq!(
                prepare_virtual_slice_consumption(&fixture.input()),
                Err(Refusal::Adapter(ClutchError::MismatchedState)),
                "flags {flags}"
            );
        }
        for flags in [
            RECEIPT_FLAG_SLICE_EXHAUSTED,
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED,
        ] {
            let mut fixture = virtual_fixture();
            fixture.receipt.consumed_flags = flags;
            assert_eq!(
                prepare_virtual_slice_consumption(&fixture.input()),
                Err(Refusal::Codec(CodecError::InvalidEnum)),
                "flags {flags}"
            );
        }
    }

    #[test]
    fn a_direct_receipt_never_crosses_the_virtual_seam_and_the_reverse() {
        let mut fixture = virtual_fixture();
        fixture.receipt.leg_kind = RECEIPT_LEG_DIRECT;
        fixture.receipt.sell_order_id = canonical_order_id(3);
        assert_eq!(
            prepare_virtual_slice_consumption(&fixture.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );

        // And the direct seam refuses a split receipt: its `leg_kind` gate
        // was already there, and this pins that the two seams stay disjoint.
        let mut direct = direct_fixture();
        direct.receipt.leg_kind = RECEIPT_LEG_SPLIT;
        direct.receipt.sell_order_id = Hash32::ZERO;
        assert_eq!(
            prepare_entitled_slice_consumption(&direct.input()),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn a_churned_epoch_makes_the_pot_mandatory_on_every_direct_slice() {
        // The pot's cash only closes at `sigma * price_scale` when every
        // slice feeds it, so a churned epoch may not settle a direct slice
        // with the seven-account shape.
        let mut fixture = direct_fixture();
        fixture.candidate.virtual_split = 1;
        fixture.candidate.churn = 1;
        fixture.candidate.candidate = fixture.candidate.recomputed_candidate_digest().unwrap();
        fixture.receipt.candidate = fixture.candidate.candidate;
        assert_eq!(
            prepare_entitled_slice_consumption(&fixture.input()),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
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
        assert_eq!(plan.buyer_debit_atoms, 2);
        assert_eq!(plan.seller_credit_atoms, 2);
        assert_eq!(plan.residue_price_units, 0);
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
            None,
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
        let initial_cash: u64 = f
            .buyer_reservations
            .iter()
            .map(|r| r.initial_cash_atoms)
            .sum();
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
                pot: None,
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
                None,
                plan,
            );
            f.buyers[at].validate().unwrap();
            f.seller_position.validate().unwrap();
            f.buyer_reservations[at].validate().unwrap();
            f.seller_reservation.validate().unwrap();
            consumed_atoms += plan.buyer_debit_atoms;
            assert_eq!(plan.seller_credit_atoms, plan.buyer_debit_atoms);
            assert_eq!(plan.residue_price_units, 0);
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
            assert_eq!(f.buyer_reservations[at].state, RESERVATION_STATE_CONSUMED);
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
                pot: None,
            }),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    /// One inexactly convertible pair: five Eggs at half the price scale, so
    /// each end's whole-order value is `25_000` price units against a scale of
    /// `10_000`.  The payer converts up to three atoms, the payee down to two,
    /// and the epoch leaves exactly one atom — `10_000` price units — where no
    /// owner is entitled to it.
    fn inexact_fixture() -> (DirectFixture, FinalPotAccount) {
        let mut f = direct_fixture();
        for slot in [&mut f.buy, &mut f.sell] {
            if let OrderSlot::Single(order) = slot {
                order.quantity = 5;
            }
        }
        let buy_plan = ReservationPlan::for_order(&f.buy, 2, 10_000, 0).unwrap();
        let sell_plan = ReservationPlan::for_order(&f.sell, 2, 10_000, 0).unwrap();
        // The buy reserves the round-up of its whole limit value, which is
        // exactly what its round-up consideration costs: no release remains.
        assert_eq!(buy_plan.cash_atoms, 3);
        f.buyer_position.reserved_cash_atoms = buy_plan.cash_atoms;
        f.buyer_reservation = ReservationAccount::active(
            f.epoch.market,
            f.epoch.epoch,
            f.buyer_position.owner,
            f.buy.order_id(),
            f.epoch.price_grid,
            f.epoch.terms,
            f.epoch.policy,
            0,
            f.buy.generation(),
            0,
            8,
            buy_plan,
        )
        .unwrap()
        .entitled(5)
        .unwrap();
        f.seller_reservation = ReservationAccount::active(
            f.epoch.market,
            f.epoch.epoch,
            f.seller_position.owner,
            f.sell.order_id(),
            f.epoch.price_grid,
            f.epoch.terms,
            f.epoch.policy,
            0,
            f.sell.generation(),
            0,
            9,
            sell_plan,
        )
        .unwrap()
        .entitled(5)
        .unwrap();
        f.receipt.quantity = 5;
        f.receipt.consideration_price_units = 5 * 5_000;
        f.receipt.validate().unwrap();
        let pot = FinalPotAccount {
            epoch: f.epoch.epoch,
            market: f.epoch.market,
            candidate: f.candidate.candidate,
            pot_internal: [0; MAX_OUTCOMES],
            pot_cash_price_units: 0,
            rounding_pot_price_units: 10_000,
            outcome_count: f.epoch.outcome_count,
            phase: POT_PHASE_OPEN,
            stored_bump: 5,
            flags: 0,
        };
        pot.validate().unwrap();
        (f, pot)
    }

    #[test]
    fn an_inexact_pair_converts_per_order_and_drains_the_pot_to_empty() {
        let (mut f, mut pot) = inexact_fixture();
        let cash_before = f.buyer_position.cash_atoms + f.seller_position.cash_atoms;
        let plan = prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
            pot: Some(&pot),
            ..f.input()
        })
        .unwrap();
        // The frozen boundary, realized: the payer rounds up, the payee rounds
        // down, and the gap is the pot's whole expectation.
        assert_eq!(plan.buyer_debit_atoms, 3);
        assert_eq!(plan.seller_credit_atoms, 2);
        assert_eq!(plan.residue_price_units, 10_000);
        assert!(plan.buyer_completes && plan.seller_completes);
        assert_eq!(plan.buyer_release_atoms, 0);

        apply_entitled_slice_consumption(
            &mut f.buyer_position,
            &mut f.seller_position,
            &mut f.buyer_reservation,
            &mut f.seller_reservation,
            &mut f.receipt,
            Some(&mut pot),
            plan,
        );
        f.buyer_position.validate().unwrap();
        f.seller_position.validate().unwrap();
        f.buyer_reservation.validate().unwrap();
        f.seller_reservation.validate().unwrap();
        f.receipt.validate().unwrap();
        pot.validate().unwrap();

        // The pot is empty, so the epoch's pot can close.
        assert_eq!(pot.rounding_pot_price_units, 0);
        assert_eq!(pot.pot_cash_price_units, 0);
        assert_eq!(pot.pot_internal, [0; MAX_OUTCOMES]);
        // Nothing custodies the residue: the epoch's owners simply hold one
        // atom less than they started with, which is `rounding_pot / scale`.
        let cash_after = f.buyer_position.cash_atoms + f.seller_position.cash_atoms;
        assert_eq!(cash_before - cash_after, 1);
        assert_eq!(u128::from(cash_before - cash_after) * 10_000, 10_000);
        assert_eq!(f.buyer_position.internal[0], 5);
        assert_eq!(f.buyer_reservation.state, RESERVATION_STATE_CONSUMED);
        assert_eq!(f.seller_reservation.state, RESERVATION_STATE_CONSUMED);
        assert!(f.seller_reservation.remaining_is_zero());
    }

    #[test]
    fn realizing_residue_without_a_pot_or_beyond_its_expectation_refuses() {
        let (f, pot) = inexact_fixture();
        // No pot presented: the seam will not realize residue it cannot draw.
        assert_eq!(
            prepare_entitled_slice_consumption(&f.input()),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
        // A pot whose verified expectation is short of what this slice would
        // realize refuses rather than going negative.
        let mut short = pot;
        short.rounding_pot_price_units = 9_999;
        assert_eq!(
            prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
                pot: Some(&short),
                ..f.input()
            }),
            Err(Refusal::Adapter(ClutchError::AggregateClosureMismatch))
        );
        // A stranger's pot refuses at the binding, not at the arithmetic.
        let mut stranger = pot;
        stranger.candidate = h(0x77);
        assert!(
            prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
                pot: Some(&stranger),
                ..f.input()
            })
            .is_err()
        );
    }

    #[test]
    fn an_inexact_slice_of_an_exactly_convertible_order_realizes_no_residue() {
        // The residue the partial-fill wave recorded as a standing refusal:
        // slices that do not convert while every order sum does.  The
        // conversion is cumulative, so the atoms telescope and the pot is
        // never touched.
        let mut f = partial_fixture();
        f.receipts[0].quantity = 5;
        f.receipts[0].consideration_price_units = 5 * 5_000;
        f.receipts[0].validate().unwrap();
        let plan = prepare_entitled_slice_consumption(&EntitledSliceConsumptionInput {
            epoch: &f.epoch,
            candidate: &f.candidate,
            buyer_position: &f.buyers[0],
            seller_position: &f.seller_position,
            buyer_reservation: &f.buyer_reservations[0],
            seller_reservation: &f.seller_reservation,
            receipt: &f.receipts[0],
            pot: None,
        })
        .unwrap();
        assert_eq!(plan.buyer_debit_atoms, 3);
        assert_eq!(plan.seller_credit_atoms, 2);
        assert_eq!(plan.residue_price_units, 0);
        assert!(!plan.buyer_completes && !plan.seller_completes);
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
                pot: None,
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
                None,
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
            pot: None,
        })
        .unwrap();
        apply_entitled_slice_consumption(
            &mut f.buyers[0],
            &mut f.seller_position,
            &mut f.buyer_reservations[0],
            &mut f.seller_reservation,
            &mut f.receipts[0],
            None,
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
            pot: None,
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
