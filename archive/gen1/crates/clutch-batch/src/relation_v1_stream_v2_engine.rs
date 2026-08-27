//! Complete relation execution over borrowed active-width V2 bytes.
//!
//! This is kept beside the V2 codec so the fixed-layout V1 checkpoint remains
//! a migration/differential oracle, never the runtime representation.

use super::*;
use crate::relation_v1::{
    mask_bit, scaled_reservation, AllocationPolicyV1, AonPolicyV1, BasisDescriptorV1, DigestFoldV1,
    EligibilityV1, ErrorV1, FeeBaseV1, LegRefV1, OrderV1, PairingSliceV1, PairingWitnessPolicyV1,
    RelationDomainV1, RoundingBoundaryV1, ScoreV1, SelfCrossPolicyV1, SummaryV1, MAX_OUTCOMES,
};
use crate::relation_v1_stream::{FeedErrorV1, FeedStatusV1, StreamCandidateV1};
use crate::{seeded_rank, DustPolicy, PartialPolicy, Side, MAX_ORDERS};

const CLASS_STRICT: u8 = 0;
const CLASS_MARGINAL: u8 = 1;

const FLAG_ACTIVE: u8 = 1;
const FLAG_FORCED: u8 = 1 << 1;
const FLAG_HONORED: u8 = 1 << 2;
const FLAG_POOL: u8 = 1 << 3;
const FLAG_STRICT_FULL: u8 = 1 << 4;

const M00_DOMAIN: u8 = 0;
const M01_ADMIT: u8 = 1;
const M03_SELF_CROSS: u8 = 3;
const M04_LEN: u8 = 4;
const M05_PRICES: u8 = 5;
const M06_CLASSIFY: u8 = 6;
const M07_WITNESS_FILLS: u8 = 7;
const M08_CHURN: u8 = 8;
const M09_FLOWS: u8 = 9;
const M10_CONSERVATION: u8 = 10;
const M11_CANONICAL: u8 = 11;
const M12_PAIRING: u8 = 12;
const M13_SETTLE: u8 = 13;
const M14_SCORE: u8 = 14;
const V0_COMPLETE_MAJOR: u8 = M05_PRICES;

const V3_STEP_AGGREGATE: u16 = 0;
const V3_STEP_VIRTUAL: u16 = 1;
const V3_STEP_AON_AGG: u16 = 2;
const V3_STEP_FORCED: u16 = 3;
const V3_STEP_STRICT: u16 = 4;
const V3_STEP_BUY_CAST: u16 = 5;
const V3_STEP_BUY_POOL: u16 = 6;
const V3_STEP_BUY_DUST: u16 = 7;
const V3_STEP_SELL_CAST: u16 = 8;
const V3_STEP_SELL_POOL: u16 = 9;
const V3_STEP_SELL_DUST: u16 = 10;
const V3_STEP_FLOW_CAST: u16 = 11;
const V3_BLOCK_OBLIGATION: u16 = MAX_OUTCOMES as u16 + 1;
const V3_BLOCK_EQUALITY: u16 = MAX_OUTCOMES as u16 + 2;

const fn pos(major: u8, a: u16, b: u16, c: u16, site: u8) -> u64 {
    ((major as u64) << 56)
        | ((a as u64) << 40)
        | ((b as u64) << 24)
        | ((c as u64) << 8)
        | (site as u64)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PoolV2 {
    total: u128,
    count: u16,
    target: u64,
    floor_sum: u64,
    ready: bool,
    dust_rejected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PoolRowV2 {
    remainder: u128,
    rank: u64,
    id: u64,
    floor: u64,
    effective: u64,
    minimum: u64,
    pool: u8,
    extra: bool,
    aon: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutcomeAggV2 {
    demand: u128,
    supply: u128,
    forced_buy: u128,
    forced_sell: u128,
    forced_aon_buy: u128,
    forced_aon_sell: u128,
    strict_buy: u128,
    strict_sell: u128,
}

#[derive(Clone, Copy)]
struct PassSteps {
    net_assign: bool,
    accumulate: bool,
    floor: bool,
}

#[derive(Clone, Copy)]
struct EngineStateV2 {
    phase: u8,
    pass: u8,
    order_passes: u8,
    slices_after_pass: u8,
    slices_expected: bool,
    check_claims: bool,
    cursor: u16,
    slice_cursor: u16,
    order_count: u16,
    latch_set: bool,
    latch_position: u64,
    latch_error: ErrorV1,
    fold: DigestFoldV1,
    sealed_fold: DigestFoldV1,
    digest: DigestFoldV1,
    previous_id: u64,
    portfolio_count: u8,
    domain: RelationDomainV1,
    cand: StreamCandidateV1,
    owner_slots: u16,
    side_buy_bits: u64,
    opening_reserved_cash: u128,
    netting_cancelled_cash: u128,
    consideration: u128,
    seller_credit: u128,
    limit_surplus: u128,
    debit_atoms: u128,
    credit_atoms: u128,
    rounding_pot: u128,
    summary: SummaryV1,
    summary_valid: bool,
}

/// Complete V1-equivalent feed engine backed by exact-width V2 account bytes.
///
/// Public mutating calls persist scalar state before returning, including on a
/// protocol error. Array and ledger writes land directly in the borrowed body.
/// No method constructs or allocates a `ClearWorkV1`.
pub struct ClearWorkFeedV2<'a> {
    bytes: &'a mut [u8],
    widths: ClearWorkWidthsV2,
    state: EngineStateV2,
}

impl<'a> ClearWorkFeedV2<'a> {
    /// Open hostile account bytes only after the V2 structural validator.
    pub fn open(bytes: &'a mut [u8], widths: ClearWorkWidthsV2) -> Result<Self, ClearWorkFaultV2> {
        validate_clear_work_v2(bytes, widths)?;
        let state = decode_state(bytes, widths)?;
        Ok(Self {
            bytes,
            widths,
            state,
        })
    }

    /// Initialize and open a canonical idle body without V1 expansion.
    pub fn initialize(
        bytes: &'a mut [u8],
        widths: ClearWorkWidthsV2,
    ) -> Result<Self, ClearWorkFaultV2> {
        initialize_clear_work_v2_idle(bytes, widths)?;
        Self::open(bytes, widths)
    }

    pub fn begin(
        &mut self,
        domain: &RelationDomainV1,
        candidate: &StreamCandidateV1,
        strict_claims: bool,
    ) -> Result<FeedStatusV1, FeedErrorV1> {
        self.begin_with_basis(domain, candidate, strict_claims, BasisDescriptorV1::UNGATED)
    }

    pub fn begin_with_basis(
        &mut self,
        domain: &RelationDomainV1,
        candidate: &StreamCandidateV1,
        strict_claims: bool,
        basis: BasisDescriptorV1,
    ) -> Result<FeedStatusV1, FeedErrorV1> {
        let result = self.begin_inner(domain, candidate, strict_claims, basis);
        self.persist();
        result
    }

    pub fn status(&self) -> FeedStatusV1 {
        match self.state.phase {
            PHASE_ORDERS => FeedStatusV1::NeedOrders {
                pass: self.state.pass,
            },
            PHASE_SLICES => FeedStatusV1::NeedSlices,
            _ => FeedStatusV1::Complete,
        }
    }

    pub fn verdict(&self) -> Option<Result<&SummaryV1, ErrorV1>> {
        if self.state.phase != PHASE_COMPLETE {
            return None;
        }
        if self.state.latch_set {
            Some(Err(self.state.latch_error))
        } else if self.state.summary_valid {
            Some(Ok(&self.state.summary))
        } else {
            None
        }
    }

    pub fn consumed_fold(&self) -> u128 {
        self.state.sealed_fold.digest()
    }

    pub fn is_idle(&self) -> bool {
        self.state.phase == PHASE_IDLE
    }

    pub fn is_poisoned(&self) -> bool {
        self.state.phase == PHASE_POISONED
    }

    pub fn orders_consumed(&self) -> u16 {
        self.state.cursor
    }

    pub fn slices_consumed(&self) -> u16 {
        self.state.slice_cursor
    }

    pub fn push_order(&mut self, order: &OrderV1, fill: u64) -> Result<FeedStatusV1, FeedErrorV1> {
        let result = self.push_order_inner(order, fill);
        self.persist();
        result
    }

    pub fn push_slice(&mut self, slice: &PairingSliceV1) -> Result<FeedStatusV1, FeedErrorV1> {
        let result = self.push_slice_inner(slice);
        self.persist();
        result
    }

    pub fn end_pass(&mut self) -> Result<FeedStatusV1, FeedErrorV1> {
        let result = self.end_pass_inner();
        self.persist();
        result
    }

    fn reset(&mut self) {
        // Length and widths were validated at construction; this cannot fail.
        let result = initialize_clear_work_v2_idle(self.bytes, self.widths);
        debug_assert!(result.is_ok());
        self.state = decode_state(self.bytes, self.widths).unwrap_or_else(|_| unreachable!());
    }

    fn persist(&mut self) {
        encode_state(self.bytes, self.widths, &self.state);
    }

    fn latch(&mut self, position: u64, error: ErrorV1) {
        if !self.state.latch_set || position < self.state.latch_position {
            self.state.latch_set = true;
            self.state.latch_position = position;
            self.state.latch_error = error;
        }
    }

    fn outcomes(&self) -> usize {
        self.state.domain.outcome_count as usize
    }

    fn imbalance(&self) -> i128 {
        self.state.cand.virtual_split as i128 - self.state.cand.virtual_merge as i128
    }

    fn slices_declared(&self) -> u16 {
        self.state.cand.declared_slices.unwrap_or_default()
    }

    fn slice_checks_live(&self) -> bool {
        self.state.slices_expected
            && self.state.domain.policy.pairing_witness == PairingWitnessPolicyV1::ExplicitSlices
            && (self.slices_declared() as usize) <= crate::relation_v1::MAX_SLICES
    }

    // Execution methods follow below. Keeping them on this byte-backed type
    // makes accidental calls into ClearWorkV1 impossible in the runtime path.

    fn begin_inner(
        &mut self,
        domain: &RelationDomainV1,
        candidate: &StreamCandidateV1,
        strict_claims: bool,
        basis: BasisDescriptorV1,
    ) -> Result<FeedStatusV1, FeedErrorV1> {
        self.reset();
        self.state.check_claims = strict_claims;
        self.state.domain = *domain;
        self.state.cand = *candidate;
        self.state.order_passes = match domain.policy.self_cross {
            SelfCrossPolicyV1::NetAtAdmission => 3,
            SelfCrossPolicyV1::RefuseOverlap | SelfCrossPolicyV1::AllowGateAtPairing => 2,
        };
        self.state.slices_expected = candidate.declared_slices.is_some();
        self.state.slices_after_pass = self.state.order_passes - 1;

        if let Err(error) = domain.validate() {
            self.latch(pos(M00_DOMAIN, 0, 0, 0, 0), error);
            self.state.phase = PHASE_COMPLETE;
            return Ok(FeedStatusV1::Complete);
        }
        // Active storage widths come from authenticated frozen state, never
        // from the candidate claim. A mismatch is unreachable in the adapter
        // envelope; keep it a non-verdict poison if a host caller violates it.
        if domain.outcome_count != self.widths.outcomes
            || domain.owner_count != self.widths.owners as u16
        {
            self.state.phase = PHASE_POISONED;
            return Err(FeedErrorV1::ResumeFoldMismatch);
        }
        if let Err(error) = crate::relation_v1::validate_prices(domain, &candidate.prices) {
            self.latch(pos(M05_PRICES, 0, 0, 0, 0), error);
        }
        if let Err(error) =
            crate::relation_v1::validate_price_moment_cone(domain, basis, &candidate.prices)
        {
            self.latch(pos(M05_PRICES, 1, 0, 0, 0), error);
        }
        let witnessed = domain.policy.aon == AonPolicyV1::WitnessedHonoredMask;
        if !witnessed && candidate.honored_aon_mask != 0 {
            self.latch(
                pos(M07_WITNESS_FILLS, 0, 0, 0, 0),
                ErrorV1::AonMaskNotApplicable,
            );
        }
        if candidate.virtual_split != 0 && candidate.virtual_merge != 0 {
            self.latch(pos(M08_CHURN, 0, 0, 0, 0), ErrorV1::ChurnNotCanonical);
        }
        match (domain.policy.pairing_witness, self.state.slices_expected) {
            (PairingWitnessPolicyV1::RecomputedConstructor, true) => self.latch(
                pos(M12_PAIRING, 2, 0, 0, 0),
                ErrorV1::PairingWitnessNotAdmitted,
            ),
            (PairingWitnessPolicyV1::ExplicitSlices, false) => {
                self.latch(pos(M12_PAIRING, 2, 0, 0, 0), ErrorV1::PairingWitnessMissing)
            }
            _ => {}
        }
        if self.state.slices_expected
            && (self.slices_declared() as usize) > crate::relation_v1::MAX_SLICES
        {
            self.latch(pos(M12_PAIRING, 3, 0, 0, 0), ErrorV1::SliceSumMismatch);
        }
        self.state.digest.feed_head(
            domain,
            candidate.order_len,
            &candidate.prices,
            candidate.virtual_split,
            candidate.virtual_merge,
        );
        self.state.phase = PHASE_ORDERS;
        self.state.pass = 1;
        Ok(FeedStatusV1::NeedOrders { pass: 1 })
    }

    fn push_order_inner(
        &mut self,
        order: &OrderV1,
        fill: u64,
    ) -> Result<FeedStatusV1, FeedErrorV1> {
        match self.state.phase {
            PHASE_ORDERS => {}
            PHASE_IDLE | PHASE_POISONED => return Err(FeedErrorV1::NotInProgress),
            PHASE_SLICES => return Err(FeedErrorV1::WrongPhase),
            _ => return Err(FeedErrorV1::FeedComplete),
        }
        if self.state.pass == 1 {
            if self.state.cursor as usize >= self.widths.orders as usize {
                let error = if self.widths.orders as usize == MAX_ORDERS {
                    ErrorV1::TooManyOrders
                } else {
                    ErrorV1::CandidateMismatch
                };
                let major = if error == ErrorV1::TooManyOrders {
                    M01_ADMIT
                } else {
                    M04_LEN
                };
                self.latch(pos(major, self.state.cursor, 0, 0, 0), error);
                self.state.phase = PHASE_COMPLETE;
                return Ok(FeedStatusV1::Complete);
            }
        } else if self.state.cursor >= self.state.order_count {
            return Err(FeedErrorV1::TooManyPushes);
        }
        let index = self.state.cursor as usize;
        self.state.cursor += 1;
        self.fold_order(order, fill);
        if self.state.pass == 1 {
            self.state.digest.feed(fill);
            if let Some(status) = self.admit_order(index, order)? {
                return Ok(status);
            }
        }
        let steps = self.pass_steps();
        if steps.net_assign {
            self.net_assign_order(index, order);
        }
        if steps.accumulate {
            self.accumulate_order(index, order, fill);
        }
        if steps.floor {
            self.floor_order(index, order, fill);
        }
        Ok(self.status())
    }

    fn push_slice_inner(&mut self, slice: &PairingSliceV1) -> Result<FeedStatusV1, FeedErrorV1> {
        match self.state.phase {
            PHASE_SLICES => {}
            PHASE_IDLE | PHASE_POISONED => return Err(FeedErrorV1::NotInProgress),
            PHASE_ORDERS => return Err(FeedErrorV1::WrongPhase),
            _ => return Err(FeedErrorV1::FeedComplete),
        }
        if self.state.slice_cursor >= self.slices_declared() {
            return Err(FeedErrorV1::TooManyPushes);
        }
        let k = self.state.slice_cursor;
        self.state.slice_cursor += 1;
        self.state.digest.feed_slice(slice);
        self.check_slice(k, slice);
        Ok(self.status())
    }

    fn end_pass_inner(&mut self) -> Result<FeedStatusV1, FeedErrorV1> {
        match self.state.phase {
            PHASE_ORDERS => {}
            PHASE_SLICES => {
                if self.state.slice_cursor != self.slices_declared() {
                    self.state.phase = PHASE_POISONED;
                    return Err(FeedErrorV1::ResumeFoldMismatch);
                }
                self.state.phase = PHASE_ORDERS;
                self.state.pass += 1;
                return Ok(self.status());
            }
            PHASE_IDLE | PHASE_POISONED => return Err(FeedErrorV1::NotInProgress),
            _ => return Err(FeedErrorV1::FeedComplete),
        }
        if self.state.pass == 1 {
            self.state.order_count = self.state.cursor;
            self.state.sealed_fold = self.state.fold;
        } else if self.state.cursor != self.state.order_count
            || self.state.fold != self.state.sealed_fold
        {
            self.state.phase = PHASE_POISONED;
            return Err(FeedErrorV1::ResumeFoldMismatch);
        }
        self.state.fold = DigestFoldV1::NEW;
        self.state.cursor = 0;

        if self.state.pass == 1 {
            self.finalize_pass_one();
        }
        let steps = self.pass_steps();
        if steps.accumulate {
            self.finalize_accumulate();
        }
        let netting = self.state.domain.policy.self_cross == SelfCrossPolicyV1::NetAtAdmission;
        let v0_complete = if netting {
            self.state.pass >= 2
        } else {
            self.state.pass >= 1
        };
        if v0_complete
            && self.state.latch_set
            && (self.state.latch_position >> 56) as u8 <= V0_COMPLETE_MAJOR
        {
            self.state.phase = PHASE_COMPLETE;
            return Ok(FeedStatusV1::Complete);
        }
        if steps.floor {
            self.finalize_floor();
            self.state.phase = PHASE_COMPLETE;
            return Ok(FeedStatusV1::Complete);
        }
        if self.state.pass == self.state.slices_after_pass && self.slice_checks_live() {
            let n = self.widths.orders as usize;
            let o = self.widths.outcomes as usize;
            let mut i = 0usize;
            while i < n {
                let mut outcome = 0usize;
                while outcome < o {
                    self.set_matrix(MatrixU64V2::ScratchBuy, i, outcome, 0);
                    outcome += 1;
                }
                i += 1;
            }
            self.state.digest.feed(self.slices_declared() as u64);
            self.state.phase = PHASE_SLICES;
            return Ok(FeedStatusV1::NeedSlices);
        }
        self.state.pass += 1;
        Ok(self.status())
    }

    fn check_slice(&mut self, k: u16, slice: &PairingSliceV1) {
        let outcomes = self.outcomes();
        let count = self.state.order_count as usize;
        let fault = pos(M12_PAIRING, 3, 1, k, 0);
        if slice.quantity == 0 || slice.outcome as usize >= outcomes {
            self.latch(fault, ErrorV1::SliceNotExecutable);
            return;
        }
        let outcome = slice.outcome as usize;
        let buy_owner = match slice.buy_ref {
            LegRefV1::Order(index) => {
                let index = index as usize;
                if index >= count
                    || self.state.side_buy_bits & (1u64 << index) == 0
                    || self.touch(index) & (1u16 << outcome) == 0
                {
                    self.latch(fault, ErrorV1::SliceNotExecutable);
                    return;
                }
                let covered = self.matrix(MatrixU64V2::ScratchBuy, index, outcome);
                match covered.checked_add(slice.quantity) {
                    Some(sum) => self.set_matrix(MatrixU64V2::ScratchBuy, index, outcome, sum),
                    None => self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow),
                }
                Some(self.owner_slot(index))
            }
            LegRefV1::Merge => {
                let used = self.slice_used(true, outcome);
                match used.checked_add(slice.quantity) {
                    Some(sum) => self.set_slice_used(true, outcome, sum),
                    None => self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow),
                }
                None
            }
            LegRefV1::Split => {
                self.latch(fault, ErrorV1::SliceNotExecutable);
                return;
            }
        };
        let sell_owner = match slice.sell_ref {
            LegRefV1::Order(index) => {
                let index = index as usize;
                if index >= count
                    || self.state.side_buy_bits & (1u64 << index) != 0
                    || self.touch(index) & (1u16 << outcome) == 0
                {
                    self.latch(fault, ErrorV1::SliceNotExecutable);
                    return;
                }
                let covered = self.matrix(MatrixU64V2::ScratchBuy, index, outcome);
                match covered.checked_add(slice.quantity) {
                    Some(sum) => self.set_matrix(MatrixU64V2::ScratchBuy, index, outcome, sum),
                    None => self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow),
                }
                Some(self.owner_slot(index))
            }
            LegRefV1::Split => {
                let used = self.slice_used(false, outcome);
                match used.checked_add(slice.quantity) {
                    Some(sum) => self.set_slice_used(false, outcome, sum),
                    None => self.latch(pos(M12_PAIRING, 3, 1, k, 1), ErrorV1::ArithmeticOverflow),
                }
                None
            }
            LegRefV1::Merge => {
                self.latch(fault, ErrorV1::SliceNotExecutable);
                return;
            }
        };
        match (buy_owner, sell_owner) {
            (None, None) => self.latch(fault, ErrorV1::SliceNotExecutable),
            (Some(buy), Some(sell)) if buy == sell => {
                self.latch(fault, ErrorV1::SliceNotExecutable)
            }
            _ => {}
        }
    }

    fn finalize_pass_one(&mut self) {
        let outcomes = self.outcomes();
        let mut j = self.state.order_count as usize;
        while j < MAX_ORDERS {
            self.state.digest.feed(0);
            j += 1;
        }
        self.state.digest.feed(self.state.cand.honored_aon_mask);

        match self.state.domain.policy.self_cross {
            SelfCrossPolicyV1::AllowGateAtPairing => {}
            SelfCrossPolicyV1::RefuseOverlap => {
                let mut outcome = 0usize;
                while outcome < outcomes {
                    let mut slot = 0usize;
                    while slot < self.state.owner_slots as usize {
                        if self.matrix(MatrixU64V2::ScratchBuy, slot, outcome) != 0
                            && self.matrix(MatrixU64V2::ScratchSell, slot, outcome) != 0
                        {
                            self.latch(
                                pos(M03_SELF_CROSS, outcome as u16, slot as u16, 1, 0),
                                ErrorV1::SelfCrossRefused,
                            );
                        }
                        slot += 1;
                    }
                    outcome += 1;
                }
            }
            SelfCrossPolicyV1::NetAtAdmission => {
                let mut outcome = 0usize;
                while outcome < outcomes {
                    let mut slot = 0usize;
                    while slot < self.state.owner_slots as usize {
                        let buy_total = self.matrix(MatrixU64V2::ScratchBuy, slot, outcome);
                        let sell_total = self.matrix(MatrixU64V2::ScratchSell, slot, outcome);
                        if buy_total != 0 && sell_total != 0 {
                            if self.cell_portfolio(slot) & (1u16 << outcome) != 0 {
                                self.latch(
                                    pos(M03_SELF_CROSS, outcome as u16, slot as u16, 1, 0),
                                    ErrorV1::SelfCrossRefused,
                                );
                            }
                            let netted = if buy_total < sell_total {
                                buy_total
                            } else {
                                sell_total
                            };
                            self.set_matrix(MatrixU64V2::ScratchBuy, slot, outcome, netted);
                            self.set_matrix(MatrixU64V2::ScratchSell, slot, outcome, netted);
                        } else {
                            self.set_matrix(MatrixU64V2::ScratchBuy, slot, outcome, 0);
                            self.set_matrix(MatrixU64V2::ScratchSell, slot, outcome, 0);
                        }
                        slot += 1;
                    }
                    outcome += 1;
                }
            }
        }

        if self.state.order_count != self.state.cand.order_len as u16 {
            self.latch(pos(M04_LEN, 0, 0, 0, 0), ErrorV1::CandidateMismatch);
        }
        let mut i = self.state.order_count as usize;
        while i < MAX_ORDERS {
            if mask_bit(self.state.cand.honored_aon_mask, i) {
                self.latch(
                    pos(M07_WITNESS_FILLS, 1, i as u16, 0, 0),
                    ErrorV1::AonMaskNotApplicable,
                );
            }
            i += 1;
        }
    }

    fn finalize_accumulate(&mut self) {
        let outcomes = self.outcomes();
        let imbalance = self.imbalance();
        let allocation_a =
            self.state.domain.policy.allocation == AllocationPolicyV1::PricePriorityMarginalProRata;

        let mut outcome = 0usize;
        while outcome < outcomes {
            let left =
                self.flow(OutcomeFlowV2::Buy, outcome) + self.state.cand.virtual_merge as u128;
            let right =
                self.flow(OutcomeFlowV2::Sell, outcome) + self.state.cand.virtual_split as u128;
            if left != right {
                self.latch(
                    pos(M10_CONSERVATION, outcome as u16, 0, 0, 0),
                    ErrorV1::OutcomeConservationMismatch,
                );
            }
            outcome += 1;
        }

        let mut i = 0usize;
        while i < outcomes {
            let block = 1 + i as u16;
            let agg = self.agg(i);
            let supply_plus = agg.supply as i128 + imbalance;
            let executed_buy_signed = if (agg.demand as i128) < supply_plus {
                agg.demand as i128
            } else {
                supply_plus
            };
            let executed_sell_signed = executed_buy_signed - imbalance;
            if executed_buy_signed < 0 || executed_sell_signed < 0 {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_VIRTUAL, 0, 0),
                    ErrorV1::InfeasibleVirtualLeg,
                );
                i += 1;
                continue;
            }
            let executed_buy = executed_buy_signed as u128;
            let executed_sell = executed_sell_signed as u128;
            if executed_buy < agg.forced_aon_buy || executed_sell < agg.forced_aon_sell {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_AON_AGG, 0, 0),
                    ErrorV1::AonMaskDishonored,
                );
            }
            if executed_buy < agg.forced_buy || executed_sell < agg.forced_sell {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_FORCED, 0, 0),
                    ErrorV1::StrictUnderfill,
                );
                i += 1;
                continue;
            }
            if allocation_a
                && (executed_buy < agg.forced_buy + agg.strict_buy
                    || executed_sell < agg.forced_sell + agg.strict_sell)
            {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_STRICT, 0, 0),
                    ErrorV1::StrictUnderfill,
                );
                i += 1;
                continue;
            }
            self.fix_pool_target(
                i,
                Side::Buy,
                executed_buy - agg.forced_buy,
                if allocation_a { agg.strict_buy } else { 0 },
                block,
                V3_STEP_BUY_CAST,
                V3_STEP_BUY_POOL,
            );
            self.fix_pool_target(
                i,
                Side::Sell,
                executed_sell - agg.forced_sell,
                if allocation_a { agg.strict_sell } else { 0 },
                block,
                V3_STEP_SELL_CAST,
                V3_STEP_SELL_POOL,
            );
            if u64::try_from(executed_buy).is_err() || u64::try_from(executed_sell).is_err() {
                self.latch(
                    pos(M11_CANONICAL, block, V3_STEP_FLOW_CAST, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                );
            }
            i += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fix_pool_target(
        &mut self,
        outcome: usize,
        side: Side,
        target_less_forced: u128,
        strict: u128,
        block: u16,
        cast_step: u16,
        pool_step: u16,
    ) {
        let index = pool_index(outcome, side);
        if u64::try_from(target_less_forced).is_err() {
            self.latch(
                pos(M11_CANONICAL, block, cast_step, 0, 0),
                ErrorV1::ArithmeticOverflow,
            );
            return;
        }
        let target = target_less_forced - strict;
        let mut pool = self.pool(index);
        if target == 0 {
            pool.target = 0;
            pool.ready = true;
            self.set_pool(index, pool);
            return;
        }
        if pool.count == 0 || pool.total < target {
            self.latch(
                pos(M11_CANONICAL, block, pool_step, 0, 0),
                ErrorV1::ConservationFailure,
            );
            return;
        }
        pool.target = target as u64;
        pool.ready = true;
        self.set_pool(index, pool);
    }

    fn finalize_floor(&mut self) {
        self.finalize_dust();
        self.finalize_feasibility();
        self.finalize_slice_sums();
        self.finalize_settle();
        self.finalize_score();
    }

    fn finalize_dust(&mut self) {
        let outcomes = self.outcomes();
        let reject = self.state.domain.policy.dust == DustPolicy::Reject;
        let mut i = 0usize;
        while i < outcomes {
            let mut s = 0usize;
            while s < 2 {
                let side = if s == 0 { Side::Buy } else { Side::Sell };
                let index = pool_index(i, side);
                let mut pool = self.pool(index);
                if pool.ready && pool.target != 0 {
                    let dust = pool.target.saturating_sub(pool.floor_sum);
                    if dust != 0 && reject {
                        let step = if s == 0 {
                            V3_STEP_BUY_DUST
                        } else {
                            V3_STEP_SELL_DUST
                        };
                        self.latch(
                            pos(M11_CANONICAL, 1 + i as u16, step, 0, 0),
                            ErrorV1::DustRejected,
                        );
                        pool.dust_rejected = true;
                        self.set_pool(index, pool);
                    }
                }
                s += 1;
            }
            i += 1;
        }

        let count = self.state.order_count as usize;
        let mismatch = pos(M11_CANONICAL, V3_BLOCK_EQUALITY, 0, 0, 0);
        let mut j = 0usize;
        while j < count {
            let row = self.key(j);
            if row.pool == POOL_NONE {
                j += 1;
                continue;
            }
            let pool = self.pool(row.pool as usize);
            if !pool.ready || pool.dust_rejected {
                j += 1;
                continue;
            }
            let dust = pool.target.saturating_sub(pool.floor_sum) as usize;
            let mut better = 0usize;
            let mut k = 0usize;
            while k < count {
                if k != j {
                    let other = self.key(k);
                    if other.pool == row.pool && key_beats(&other, &row) {
                        better += 1;
                    }
                }
                k += 1;
            }
            let member = better < dust;
            if member != row.extra {
                self.latch(mismatch, ErrorV1::CandidateMismatch);
            }
            let derived = if member {
                row.floor.saturating_add(1)
            } else {
                row.floor
            };
            if derived != 0 {
                if row.aon && derived != row.effective {
                    self.latch(
                        pos(M11_CANONICAL, V3_BLOCK_OBLIGATION, j as u16, 0, 0),
                        ErrorV1::AllOrNoneViolation,
                    );
                }
                if derived < row.minimum {
                    self.latch(
                        pos(M11_CANONICAL, V3_BLOCK_OBLIGATION, j as u16, 0, 1),
                        ErrorV1::MinimumFillViolation,
                    );
                }
            }
            j += 1;
        }
    }

    fn finalize_feasibility(&mut self) {
        let outcomes = self.outcomes();
        let merge = self.state.cand.virtual_merge;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let flow = self.flow(OutcomeFlowV2::Buy, outcome);
            let flow = if flow > u64::MAX as u128 {
                u64::MAX
            } else {
                flow as u64
            };
            let total_flow = match flow.checked_add(merge) {
                Some(sum) => sum,
                None => {
                    self.latch(
                        pos(M12_PAIRING, 1, outcome as u16, 0, 0),
                        ErrorV1::ArithmeticOverflow,
                    );
                    outcome += 1;
                    continue;
                }
            };
            let mut slot = 0usize;
            while slot < self.state.owner_slots as usize {
                match self
                    .part(true, slot, outcome)
                    .checked_add(self.part(false, slot, outcome))
                {
                    Some(part) if part > total_flow => self.latch(
                        pos(M12_PAIRING, 1, outcome as u16, slot as u16, 1),
                        ErrorV1::PairingInfeasible {
                            outcome: outcome as u8,
                            owner: self.owner(slot),
                        },
                    ),
                    Some(_) => {}
                    None => self.latch(
                        pos(M12_PAIRING, 1, outcome as u16, slot as u16, 0),
                        ErrorV1::ArithmeticOverflow,
                    ),
                }
                slot += 1;
            }
            outcome += 1;
        }
    }

    fn finalize_slice_sums(&mut self) {
        if !self.slice_checks_live() {
            return;
        }
        let outcomes = self.outcomes();
        let mut outcome = 0usize;
        while outcome < outcomes {
            if self.slice_used(false, outcome) != self.state.cand.virtual_split
                || self.slice_used(true, outcome) != self.state.cand.virtual_merge
            {
                self.latch(pos(M12_PAIRING, 5, 0, 0, 0), ErrorV1::SliceSumMismatch);
            }
            outcome += 1;
        }
    }

    #[inline(never)]
    fn finalize_composite_numerators(&mut self, outcomes: usize) {
        let FeeBaseV1::CompositeDispersionFloor {
            dispersion_bps,
            floor_range_bps,
        } = self.state.domain.policy.fee_base
        else {
            return;
        };
        if dispersion_bps == 0 && floor_range_bps == 0 {
            return;
        }
        let mut slot = 0usize;
        while slot < self.state.owner_slots as usize {
            // The owning quote API accepts fixed canonical arrays. These two
            // small locals replace an unaligned cast from compact account data.
            let mut participation = [0u64; MAX_OUTCOMES];
            let mut outcome = 0usize;
            while outcome < outcomes {
                participation[outcome] = self.part(true, slot, outcome);
                outcome += 1;
            }
            match crate::relation_v1::composite_fee_quote(
                &participation,
                &self.state.cand.prices,
                outcomes,
                self.state.domain.price_scale,
                dispersion_bps,
                floor_range_bps,
                0,
            ) {
                Ok(quote) => {
                    self.set_owner_units_value(OwnerUnitsV2::FeeBps, slot, quote.exact_numerator)
                }
                Err(error) => self.latch(pos(M13_SETTLE, 3, slot as u16, 0, 4), error),
            }
            slot += 1;
        }
    }

    fn finalize_settle(&mut self) {
        let outcomes = self.outcomes();
        let scale = self.state.domain.price_scale as u128;
        let mut flow_consideration = 0u128;
        let mut flow_credit = 0u128;
        let mut fee_total = 0u128;
        let mut fee_carry = 0u128;
        let mut cash_refund = 0u128;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let price = self.state.cand.prices[outcome] as u128;
            match self
                .flow(OutcomeFlowV2::Buy, outcome)
                .checked_mul(price)
                .and_then(|term| flow_consideration.checked_add(term))
            {
                Some(sum) => flow_consideration = sum,
                None => self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            match self
                .flow(OutcomeFlowV2::Sell, outcome)
                .checked_mul(price)
                .and_then(|term| flow_credit.checked_add(term))
            {
                Some(sum) => flow_credit = sum,
                None => self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 1),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            let opening = self.ledger_egg(0, outcome);
            let cancelled = self.ledger_egg(1, outcome);
            let filled = self.ledger_egg(2, outcome);
            let refund = opening
                .checked_sub(cancelled)
                .and_then(|value| value.checked_sub(filled));
            let refund = match refund {
                Some(refund) => refund,
                None => {
                    self.latch(
                        pos(M13_SETTLE, 1, outcome as u16, 0, 2),
                        ErrorV1::ConservationFailure,
                    );
                    0
                }
            };
            self.state.summary.unfilled_refund_egg[outcome] = refund;
            if (opening as u128) != filled as u128 + cancelled as u128 + refund as u128 {
                self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 3),
                    ErrorV1::ConservationFailure,
                );
            }
            if self.flow(OutcomeFlowV2::Sell, outcome) != filled as u128 {
                self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 4),
                    ErrorV1::ConservationFailure,
                );
            }
            let egg_out =
                self.flow(OutcomeFlowV2::Sell, outcome) + self.state.cand.virtual_split as u128;
            let egg_in =
                self.flow(OutcomeFlowV2::Buy, outcome) + self.state.cand.virtual_merge as u128;
            if egg_out != egg_in {
                self.latch(
                    pos(M13_SETTLE, 1, outcome as u16, 0, 5),
                    ErrorV1::ConservationFailure,
                );
            }
            outcome += 1;
        }
        if flow_consideration != self.state.consideration || flow_credit != self.state.seller_credit
        {
            self.latch(pos(M13_SETTLE, 2, 0, 0, 0), ErrorV1::ConsiderationMismatch);
        }

        self.finalize_composite_numerators(outcomes);
        let denominator = match crate::relation_v1::fee_denominator_of(&self.state.domain) {
            Ok(value) => value,
            Err(error) => {
                self.latch(pos(M13_SETTLE, 3, u16::MAX, 0, 1), error);
                crate::relation_v1::FEE_BPS_DENOMINATOR as u128
            }
        };
        let quotient_is_atoms = crate::relation_v1::fee_quotient_is_atoms(&self.state.domain);
        let mut fee_quotient_total = 0u128;
        let mut fee_bps_total = 0u128;
        let mut slot = 0usize;
        while slot < self.state.owner_slots as usize {
            let fee_units = self.owner_units_value(OwnerUnitsV2::FeeBps, slot);
            let quotient = fee_units / denominator;
            let owed = match crate::relation_v1::fee_owed_price_units(
                quotient,
                quotient_is_atoms,
                scale,
            ) {
                Ok(value) => value,
                Err(error) => {
                    self.latch(pos(M13_SETTLE, 3, slot as u16, 0, 5), error);
                    0
                }
            };
            fee_carry += fee_units % denominator;
            fee_quotient_total += quotient;
            match fee_total.checked_add(owed) {
                Some(sum) => fee_total = sum,
                None => self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            let debit = self.owner_units_value(OwnerUnitsV2::Debit, slot);
            match debit.checked_add(owed) {
                Some(sum) => self.set_owner_units_value(OwnerUnitsV2::Debit, slot, sum),
                None => self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 1),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            let debit = self.owner_units_value(OwnerUnitsV2::Debit, slot);
            let reserved = self.owner_units_value(OwnerUnitsV2::Reserved, slot);
            if debit > reserved {
                self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 2),
                    ErrorV1::FeePayerUnfunded,
                );
            }
            cash_refund += reserved.saturating_sub(debit);
            match fee_bps_total.checked_add(fee_units) {
                Some(sum) => fee_bps_total = sum,
                None => self.latch(
                    pos(M13_SETTLE, 3, slot as u16, 0, 3),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            slot += 1;
        }
        if fee_quotient_total * denominator + fee_carry != fee_bps_total {
            self.latch(pos(M13_SETTLE, 3, u16::MAX, 0, 0), ErrorV1::FeeMismatch);
        }

        match self.state.domain.policy.rounding {
            RoundingBoundaryV1::ReceiptFloor => {
                let mut slot = 0usize;
                while slot < self.state.owner_slots as usize {
                    let fee_units = self.owner_units_value(OwnerUnitsV2::FeeBps, slot);
                    let fee_units = match crate::relation_v1::fee_owed_price_units(
                        fee_units / denominator,
                        quotient_is_atoms,
                        scale,
                    ) {
                        Ok(value) => value,
                        Err(error) => {
                            self.latch(pos(M13_SETTLE, 4, slot as u16, 0, 0), error);
                            0
                        }
                    };
                    if fee_units != 0 {
                        let atoms = fee_units.div_ceil(scale);
                        self.state.debit_atoms += atoms;
                        self.state.rounding_pot += atoms * scale - fee_units;
                    }
                    slot += 1;
                }
            }
            RoundingBoundaryV1::TerminalOwnerFloor | RoundingBoundaryV1::None => {
                let mut slot = 0usize;
                while slot < self.state.owner_slots as usize {
                    let debit = self.owner_units_value(OwnerUnitsV2::Debit, slot);
                    if debit != 0 {
                        let atoms = debit.div_ceil(scale);
                        self.state.debit_atoms += atoms;
                        self.state.rounding_pot += atoms * scale - debit;
                    }
                    let credit = self.owner_units_value(OwnerUnitsV2::Credit, slot);
                    if credit != 0 {
                        let atoms = credit / scale;
                        self.state.credit_atoms += atoms;
                        self.state.rounding_pot += credit - atoms * scale;
                    }
                    slot += 1;
                }
            }
        }
        if self.state.domain.policy.rounding == RoundingBoundaryV1::None
            && self.state.rounding_pot != 0
        {
            self.latch(pos(M13_SETTLE, 5, 0, 0, 0), ErrorV1::RemainderRequired);
        }

        let split_cost = match (self.state.cand.virtual_split as u128).checked_mul(scale) {
            Some(value) => value,
            None => {
                self.latch(pos(M13_SETTLE, 6, 0, 0, 0), ErrorV1::ArithmeticOverflow);
                0
            }
        };
        let merge_proceeds = match (self.state.cand.virtual_merge as u128).checked_mul(scale) {
            Some(value) => value,
            None => {
                self.latch(pos(M13_SETTLE, 6, 0, 0, 1), ErrorV1::ArithmeticOverflow);
                0
            }
        };
        let conservation_left = self.state.consideration.checked_add(merge_proceeds);
        let conservation_right = self.state.seller_credit.checked_add(split_cost);
        if conservation_left.is_none() || conservation_right.is_none() {
            self.latch(pos(M13_SETTLE, 6, 0, 0, 2), ErrorV1::ArithmeticOverflow);
        } else if conservation_left != conservation_right {
            self.latch(pos(M13_SETTLE, 6, 0, 0, 2), ErrorV1::ConservationFailure);
        }
        let cash_out = self
            .state
            .consideration
            .checked_add(fee_total)
            .and_then(|sum| sum.checked_add(cash_refund))
            .and_then(|sum| sum.checked_add(self.state.netting_cancelled_cash));
        match cash_out {
            Some(cash_out) if self.state.opening_reserved_cash != cash_out => {
                self.latch(pos(M13_SETTLE, 6, 0, 0, 3), ErrorV1::ConservationFailure)
            }
            None => self.latch(pos(M13_SETTLE, 6, 0, 0, 3), ErrorV1::ArithmeticOverflow),
            _ => {}
        }

        self.state.summary.fee_price_units = fee_total;
        self.state.summary.fee_carry_bps_units = fee_carry;
        self.state.summary.cash_refund_price_units = cash_refund;
        self.state.summary.split_cost_price_units = split_cost;
        self.state.summary.merge_proceeds_price_units = merge_proceeds;
    }

    fn finalize_score(&mut self) {
        let outcomes = self.outcomes();
        let scale = self.state.domain.price_scale as i128;
        let sigma = self.state.cand.virtual_split;
        let mu = self.state.cand.virtual_merge;
        let mut weighted = 0i128;
        let mut overlap_total = 0u64;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let flow = self.flow(OutcomeFlowV2::Buy, outcome);
            let flow = if flow > u64::MAX as u128 {
                u64::MAX
            } else {
                flow as u64
            };
            let total_flow = match flow.checked_add(mu) {
                Some(sum) => sum,
                None => {
                    self.latch(
                        pos(M14_SCORE, 0, outcome as u16, 0, 0),
                        ErrorV1::ArithmeticOverflow,
                    );
                    outcome += 1;
                    continue;
                }
            };
            let direct = match total_flow
                .checked_sub(sigma)
                .and_then(|value| value.checked_sub(mu))
            {
                Some(direct) => direct,
                None => {
                    self.latch(
                        pos(M14_SCORE, 0, outcome as u16, 0, 1),
                        ErrorV1::ArithmeticOverflow,
                    );
                    outcome += 1;
                    continue;
                }
            };
            let mut overlap = 0u64;
            let mut slot = 0usize;
            while slot < self.state.owner_slots as usize {
                let buy = self.part(true, slot, outcome);
                let sell = self.part(false, slot, outcome);
                let cell = if buy < sell { buy } else { sell };
                match overlap.checked_add(cell) {
                    Some(sum) => overlap = sum,
                    None => self.latch(
                        pos(M14_SCORE, 0, outcome as u16, slot as u16, 2),
                        ErrorV1::ArithmeticOverflow,
                    ),
                }
                slot += 1;
            }
            match overlap_total.checked_add(overlap) {
                Some(sum) => overlap_total = sum,
                None => self.latch(
                    pos(M14_SCORE, 0, outcome as u16, 0, 3),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            let price = self.state.cand.prices[outcome] as i128;
            let weighted_term = price
                .checked_mul(scale - price)
                .and_then(|weight| weight.checked_mul(direct as i128 - overlap as i128));
            match weighted_term.and_then(|term| weighted.checked_add(term)) {
                Some(sum) => weighted = sum,
                None => self.latch(
                    pos(M14_SCORE, 0, outcome as u16, 0, 4),
                    ErrorV1::ArithmeticOverflow,
                ),
            }
            self.state.summary.buy_flow[outcome] = flow;
            let sell_flow = self.flow(OutcomeFlowV2::Sell, outcome);
            self.state.summary.sell_flow[outcome] = if sell_flow > u64::MAX as u128 {
                u64::MAX
            } else {
                sell_flow as u64
            };
            self.state.summary.total_flow[outcome] = total_flow;
            self.state.summary.direct_flow[outcome] = direct;
            outcome += 1;
        }
        let mut owners = 0u16;
        let mut slot = 0usize;
        while slot < self.state.owner_slots as usize {
            let mut participates = false;
            let mut i = 0usize;
            while i < outcomes {
                if self.part(true, slot, i) != 0 || self.part(false, slot, i) != 0 {
                    participates = true;
                }
                i += 1;
            }
            if participates {
                owners += 1;
            }
            slot += 1;
        }
        let churn = match sigma.checked_add(mu) {
            Some(churn) => churn,
            None => {
                self.latch(
                    pos(M14_SCORE, 0, u16::MAX, 0, 0),
                    ErrorV1::ArithmeticOverflow,
                );
                0
            }
        };
        let digest = self.state.digest.digest();
        let score = ScoreV1 {
            weighted_direct_volume: weighted,
            limit_surplus_price_units: self.state.limit_surplus,
            distinct_owners: owners,
            churn,
            digest,
        };
        if self.state.check_claims {
            if self.state.cand.claimed_score != score {
                self.latch(pos(M14_SCORE, 1, 0, 0, 0), ErrorV1::ScoreMismatch);
            }
            if self.state.cand.canonical_candidate_digest != digest {
                self.latch(pos(M14_SCORE, 2, 0, 0, 0), ErrorV1::DigestMismatch);
            }
        }

        self.state.summary.outcome_count = self.state.domain.outcome_count;
        self.state.summary.virtual_split = sigma;
        self.state.summary.virtual_merge = mu;
        let mut outcome = 0usize;
        while outcome < outcomes {
            self.state.summary.opening_reserved_egg[outcome] = self.ledger_egg(0, outcome);
            self.state.summary.netting_cancelled_egg[outcome] = self.ledger_egg(1, outcome);
            outcome += 1;
        }
        self.state.summary.opening_reserved_cash_price_units = self.state.opening_reserved_cash;
        self.state.summary.buyer_consideration_price_units = self.state.consideration;
        self.state.summary.seller_credit_price_units = self.state.seller_credit;
        self.state.summary.rounding_pot_price_units = self.state.rounding_pot;
        self.state.summary.debit_atoms = self.state.debit_atoms;
        self.state.summary.credit_atoms = self.state.credit_atoms;
        self.state.summary.distinct_participating_owners = owners;
        self.state.summary.self_overlap_volume = overlap_total;
        self.state.summary.score = score;
        self.state.summary.candidate_digest = digest;
        self.state.summary_valid = !self.state.latch_set;
    }

    fn pass_steps(&self) -> PassSteps {
        let netting = self.state.domain.policy.self_cross == SelfCrossPolicyV1::NetAtAdmission;
        if netting {
            PassSteps {
                net_assign: self.state.pass == 2,
                accumulate: self.state.pass == 2,
                floor: self.state.pass == 3,
            }
        } else {
            PassSteps {
                net_assign: false,
                accumulate: self.state.pass == 1,
                floor: self.state.pass == 2,
            }
        }
    }

    fn fold_order(&mut self, order: &OrderV1, fill: u64) {
        match order {
            OrderV1::SingleEgg(o) => {
                self.state.fold.feed(1);
                self.state.fold.feed(o.canonical_order_id);
                self.state.fold.feed(o.owner as u64);
                self.state.fold.feed(o.outcome as u64);
                self.state.fold.feed(match o.side {
                    Side::Buy => 0,
                    Side::Sell => 1,
                });
                self.state.fold.feed(o.quantity);
                self.state.fold.feed(o.limit_price);
                self.state.fold.feed(o.minimum_fill);
                self.state.fold.feed(match o.partial_policy {
                    PartialPolicy::Allow => 0,
                    PartialPolicy::AllOrNone => 1,
                });
                self.state.fold.feed(o.expiry_epoch);
            }
            OrderV1::Portfolio(o) => {
                self.state.fold.feed(2);
                self.state.fold.feed(o.canonical_order_id);
                self.state.fold.feed(o.owner as u64);
                self.state.fold.feed(match o.side {
                    Side::Buy => 0,
                    Side::Sell => 1,
                });
                let mut i = 0usize;
                while i < MAX_OUTCOMES {
                    self.state.fold.feed(o.coefficients[i]);
                    i += 1;
                }
                self.state.fold.feed(o.active_len as u64);
                self.state.fold.feed(o.lots);
                self.state.fold.feed(o.limit_collateral_per_lot);
                self.state.fold.feed(o.minimum_fill_lots);
                self.state.fold.feed(match o.partial_policy {
                    PartialPolicy::Allow => 0,
                    PartialPolicy::AllOrNone => 1,
                });
                self.state.fold.feed(o.expiry_epoch);
            }
        }
        self.state.fold.feed(fill);
    }

    fn admit_order(
        &mut self,
        index: usize,
        order: &OrderV1,
    ) -> Result<Option<FeedStatusV1>, FeedErrorV1> {
        let domain = self.state.domain;
        let outcomes = self.outcomes();
        let refuse = |work: &mut Self, error: ErrorV1| {
            work.latch(pos(M01_ADMIT, index as u16, 0, 0, 0), error);
            work.state.phase = PHASE_COMPLETE;
            Some(FeedStatusV1::Complete)
        };
        if order.id() == 0 || order.id() <= self.state.previous_id {
            return Ok(refuse(self, ErrorV1::NonCanonicalOrderOrder));
        }
        self.state.previous_id = order.id();
        if order.owner() >= domain.owner_count {
            return Ok(refuse(self, ErrorV1::InvalidOwner));
        }
        if order.expiry_epoch() < domain.epoch {
            return Ok(refuse(self, ErrorV1::ExpiredOrder));
        }
        match order {
            OrderV1::SingleEgg(o) => {
                if o.outcome as usize >= outcomes {
                    return Ok(refuse(self, ErrorV1::InvalidOutcome));
                }
                if o.quantity == 0 {
                    return Ok(refuse(self, ErrorV1::InvalidQuantity));
                }
                if o.minimum_fill > o.quantity {
                    return Ok(refuse(self, ErrorV1::InvalidMinimumFill));
                }
                if o.limit_price > domain.price_scale {
                    return Ok(refuse(self, ErrorV1::PriceOutOfRange));
                }
            }
            OrderV1::Portfolio(o) => {
                self.state.portfolio_count += 1;
                if self.state.portfolio_count as usize > crate::relation_v1::MAX_PORTFOLIO_ORDERS {
                    return Ok(refuse(self, ErrorV1::TooManyPortfolios));
                }
                if o.active_len == 0 || o.active_len as usize > outcomes {
                    return Ok(refuse(self, ErrorV1::InvalidOutcome));
                }
                if o.lots == 0 {
                    return Ok(refuse(self, ErrorV1::InvalidQuantity));
                }
                if o.minimum_fill_lots > o.lots {
                    return Ok(refuse(self, ErrorV1::InvalidMinimumFill));
                }
                let mut j = 0usize;
                let mut nonzero = false;
                while j < o.active_len as usize {
                    if o.coefficients[j] != 0 {
                        nonzero = true;
                    }
                    j += 1;
                }
                if !nonzero {
                    return Ok(refuse(self, ErrorV1::InvalidQuantity));
                }
                while j < MAX_OUTCOMES {
                    if o.coefficients[j] != 0 {
                        return Ok(refuse(self, ErrorV1::NonCanonicalPadding));
                    }
                    j += 1;
                }
                let mut value = 0u128;
                let mut k = 0usize;
                while k < o.active_len as usize {
                    let term =
                        match (o.coefficients[k] as u128).checked_mul(domain.price_scale as u128) {
                            Some(term) => term,
                            None => return Ok(refuse(self, ErrorV1::ArithmeticOverflow)),
                        };
                    value = match value.checked_add(term) {
                        Some(value) => value,
                        None => return Ok(refuse(self, ErrorV1::ArithmeticOverflow)),
                    };
                    k += 1;
                }
                if (o.lots as u128).checked_mul(value).is_none() {
                    return Ok(refuse(self, ErrorV1::ArithmeticOverflow));
                }
            }
        }
        if order.partial_policy() == PartialPolicy::AllOrNone
            && order.minimum_fill() != order.quantity()
        {
            return Ok(refuse(self, ErrorV1::InvalidMinimumFill));
        }
        if domain.policy.aon == AonPolicyV1::RefuseAdmission {
            if order.partial_policy() == PartialPolicy::AllOrNone {
                return Ok(refuse(self, ErrorV1::AonNotAdmitted));
            }
            if order.minimum_fill() > 1 {
                return Ok(refuse(self, ErrorV1::MinimumFillNotAdmitted));
            }
        }
        if order.reservation_price_units(domain.price_scale).is_err() {
            return Ok(refuse(self, ErrorV1::ArithmeticOverflow));
        }

        let owner = order.owner();
        let mut slot = usize::MAX;
        let mut s = 0usize;
        while s < self.state.owner_slots as usize {
            if self.owner(s) == owner {
                slot = s;
                break;
            }
            s += 1;
        }
        if slot == usize::MAX {
            slot = self.state.owner_slots as usize;
            if slot >= self.widths.owners as usize {
                self.latch(pos(M01_ADMIT, index as u16, 0, 0, 0), ErrorV1::InvalidOwner);
                self.state.phase = PHASE_COMPLETE;
                return Ok(Some(FeedStatusV1::Complete));
            }
            self.set_owner(slot, owner);
            self.state.owner_slots += 1;
        }
        self.set_owner_slot(index, slot as u16);

        if order.side() == Side::Buy {
            self.state.side_buy_bits |= 1u64 << index;
        }
        let mut touch = 0u16;
        let mut outcome = 0usize;
        while outcome < outcomes {
            if order.touches(outcome as u8) {
                touch |= 1u16 << outcome;
            }
            outcome += 1;
        }
        self.set_touch(index, touch);

        match domain.policy.self_cross {
            SelfCrossPolicyV1::AllowGateAtPairing => {}
            SelfCrossPolicyV1::RefuseOverlap => {
                let mut i = 0usize;
                while i < outcomes {
                    if touch & (1u16 << i) != 0 {
                        let matrix = if order.side() == Side::Buy {
                            MatrixU64V2::ScratchBuy
                        } else {
                            MatrixU64V2::ScratchSell
                        };
                        let value = self.matrix(matrix, slot, i).saturating_add(1);
                        self.set_matrix(matrix, slot, i, value);
                    }
                    i += 1;
                }
            }
            SelfCrossPolicyV1::NetAtAdmission => {
                let units = order.quantity();
                let portfolio = matches!(order, OrderV1::Portfolio(_));
                let mut i = 0usize;
                while i < outcomes {
                    if touch & (1u16 << i) != 0 {
                        if portfolio {
                            let value = self.cell_portfolio(slot) | (1u16 << i);
                            self.set_cell_portfolio(slot, value);
                        }
                        let matrix = if order.side() == Side::Buy {
                            MatrixU64V2::ScratchBuy
                        } else {
                            MatrixU64V2::ScratchSell
                        };
                        let cell = self.matrix(matrix, slot, i);
                        match cell.checked_add(units) {
                            Some(sum) => self.set_matrix(matrix, slot, i, sum),
                            None => self.latch(
                                pos(M03_SELF_CROSS, i as u16, slot as u16, 0, 0),
                                ErrorV1::ArithmeticOverflow,
                            ),
                        }
                    }
                    i += 1;
                }
            }
        }
        Ok(None)
    }

    fn net_assign_order(&mut self, index: usize, order: &OrderV1) {
        let outcomes = self.outcomes();
        let slot = self.owner_slot(index) as usize;
        let mut i = 0usize;
        while i < outcomes {
            if self.touch(index) & (1u16 << i) == 0 {
                i += 1;
                continue;
            }
            let matrix = if order.side() == Side::Buy {
                MatrixU64V2::ScratchBuy
            } else {
                MatrixU64V2::ScratchSell
            };
            let cell = self.matrix(matrix, slot, i);
            let available = order.quantity().saturating_sub(self.cancelled(index));
            let take = if available < cell { available } else { cell };
            if take != 0 {
                if order.partial_policy() == PartialPolicy::AllOrNone && take != available {
                    self.latch(
                        pos(M03_SELF_CROSS, i as u16, slot as u16, 2, 0),
                        ErrorV1::SelfCrossRefused,
                    );
                }
                self.set_matrix(matrix, slot, i, cell - take);
                self.set_cancelled(index, self.cancelled(index) + take);
            }
            i += 1;
        }
    }

    fn accumulate_order(&mut self, index: usize, order: &OrderV1, fill: u64) {
        let domain = self.state.domain;
        let outcomes = self.outcomes();
        let slot = self.owner_slot(index) as usize;
        let effective = order.quantity().saturating_sub(self.cancelled(index));
        let minimum = order.minimum_fill();
        let effective_minimum = if minimum > effective {
            effective
        } else {
            minimum
        };

        let class = if effective == 0 {
            CLASS_INELIGIBLE
        } else {
            match crate::relation_v1::classify_order(&domain, order, &self.state.cand.prices) {
                Ok(EligibilityV1::Strict) => CLASS_STRICT,
                Ok(EligibilityV1::Marginal) => CLASS_MARGINAL,
                Ok(EligibilityV1::Ineligible) => CLASS_INELIGIBLE,
                Err(error) => {
                    self.latch(pos(M06_CLASSIFY, index as u16, 0, 0, 0), error);
                    CLASS_INELIGIBLE
                }
            }
        };
        self.set_class(index, class);

        let witnessed = domain.policy.aon == AonPolicyV1::WitnessedHonoredMask;
        let honored_bit = mask_bit(self.state.cand.honored_aon_mask, index);
        let witness_fault = if fill > effective {
            Some(ErrorV1::FillExceedsQuantity)
        } else if fill != 0 && class == CLASS_INELIGIBLE {
            Some(ErrorV1::IneligibleFill)
        } else if witnessed && honored_bit && !order.carries_minimum_obligation() {
            Some(ErrorV1::AonMaskNotApplicable)
        } else if witnessed && honored_bit && (class == CLASS_INELIGIBLE || fill != effective) {
            Some(ErrorV1::AonMaskDishonored)
        } else if witnessed && !honored_bit && order.carries_minimum_obligation() && fill != 0 {
            Some(ErrorV1::AonMaskLeak)
        } else if order.partial_policy() == PartialPolicy::AllOrNone
            && fill != 0
            && fill != effective
        {
            Some(ErrorV1::AllOrNoneViolation)
        } else if fill != 0 && fill < effective_minimum {
            Some(ErrorV1::MinimumFillViolation)
        } else {
            None
        };
        if let Some(error) = witness_fault {
            self.latch(pos(M07_WITNESS_FILLS, 2, index as u16, 0, 0), error);
        }

        let obligated = witnessed && order.carries_minimum_obligation();
        let portfolio = matches!(order, OrderV1::Portfolio(_));
        if honored_bit && !witnessed {
            self.latch(
                pos(M11_CANONICAL, 0, index as u16, 0, 0),
                ErrorV1::AonMaskNotApplicable,
            );
        }
        if honored_bit && witnessed && !order.carries_minimum_obligation() {
            self.latch(
                pos(M11_CANONICAL, 0, index as u16, 0, 1),
                ErrorV1::AonMaskNotApplicable,
            );
        }
        if honored_bit && (class == CLASS_INELIGIBLE || effective == 0) {
            self.latch(
                pos(M11_CANONICAL, 0, index as u16, 0, 2),
                ErrorV1::AonMaskDishonored,
            );
        }
        let active = class != CLASS_INELIGIBLE
            && effective != 0
            && (!obligated || honored_bit)
            && (!portfolio || honored_bit || class == CLASS_STRICT);
        let forced = active && (honored_bit || portfolio);
        let mut flags = 0u8;
        if active {
            flags |= FLAG_ACTIVE;
        }
        if forced {
            flags |= FLAG_FORCED;
        }
        if honored_bit {
            flags |= FLAG_HONORED;
        }

        let allocation_a =
            domain.policy.allocation == AllocationPolicyV1::PricePriorityMarginalProRata;
        let mut outcome = 0usize;
        while outcome < outcomes {
            if fill != 0 {
                match order.leg_quantity(outcome as u8, fill) {
                    Ok(leg) if leg != 0 => {
                        let buy = order.side() == Side::Buy;
                        let side = if buy {
                            OutcomeFlowV2::Buy
                        } else {
                            OutcomeFlowV2::Sell
                        };
                        let flow = self.flow(side, outcome);
                        let widened = flow + leg as u128;
                        if flow <= u64::MAX as u128 && widened > u64::MAX as u128 {
                            self.latch(pos(M09_FLOWS, 0, 0, 0, 0), ErrorV1::ArithmeticOverflow);
                        }
                        self.set_flow(side, outcome, widened);
                        let cell = self.part(buy, slot, outcome);
                        let cell = match cell.checked_add(leg) {
                            Some(sum) => sum,
                            None => {
                                self.latch(
                                    pos(M12_PAIRING, 0, 0, 0, 0),
                                    ErrorV1::ArithmeticOverflow,
                                );
                                u64::MAX
                            }
                        };
                        self.set_part(buy, slot, outcome, cell);
                    }
                    Ok(_) => {}
                    Err(error) => self.latch(pos(M09_FLOWS, 0, 0, 0, 0), error),
                }
            }
            if active {
                match order.leg_quantity(outcome as u8, effective) {
                    Ok(leg) if leg != 0 => {
                        let leg = leg as u128;
                        let mut agg = self.agg(outcome);
                        let buy = order.side() == Side::Buy;
                        if buy {
                            agg.demand += leg;
                        } else {
                            agg.supply += leg;
                        }
                        if forced {
                            if buy {
                                agg.forced_buy += leg;
                            } else {
                                agg.forced_sell += leg;
                            }
                            if honored_bit {
                                if buy {
                                    agg.forced_aon_buy += leg;
                                } else {
                                    agg.forced_aon_sell += leg;
                                }
                            }
                        } else if class == CLASS_STRICT {
                            if buy {
                                agg.strict_buy += leg;
                            } else {
                                agg.strict_sell += leg;
                            }
                        }
                        self.set_agg(outcome, agg);
                    }
                    Ok(_) => {}
                    Err(error) => self.latch(
                        pos(
                            M11_CANONICAL,
                            1 + outcome as u16,
                            V3_STEP_AGGREGATE,
                            index as u16,
                            0,
                        ),
                        error,
                    ),
                }
            }
            outcome += 1;
        }

        if let OrderV1::SingleEgg(o) = order {
            let participant = active && !forced && (o.outcome as usize) < outcomes;
            let pooled = participant && (!allocation_a || class == CLASS_MARGINAL);
            let strict_full = participant && allocation_a && class == CLASS_STRICT;
            if pooled {
                flags |= FLAG_POOL;
                let pool_index = pool_index(o.outcome as usize, o.side);
                let mut pool = self.pool(pool_index);
                pool.total += effective as u128;
                pool.count += 1;
                self.set_pool(pool_index, pool);
            }
            if strict_full {
                flags |= FLAG_STRICT_FULL;
            }
        }
        self.set_flags(index, flags);
        self.settle_order(index, order, fill, effective, slot);
    }

    fn settle_order(
        &mut self,
        index: usize,
        order: &OrderV1,
        fill: u64,
        effective: u64,
        slot: usize,
    ) {
        let domain = self.state.domain;
        let scale = domain.price_scale as u128;
        let outcomes = self.outcomes();
        let cancelled = self.cancelled(index);
        let mut site = 0u8;
        macro_rules! settle_latch {
            ($error:expr) => {{
                self.latch(pos(M13_SETTLE, 0, index as u16, 0, site), $error);
            }};
        }
        macro_rules! add_state {
            ($field:ident, $value:expr) => {{
                match self.state.$field.checked_add($value) {
                    Some(sum) => self.state.$field = sum,
                    None => settle_latch!(ErrorV1::ArithmeticOverflow),
                }
                site = site.saturating_add(1);
            }};
        }
        macro_rules! add_owner {
            ($column:expr, $value:expr) => {{
                let current = self.owner_units_value($column, slot);
                match current.checked_add($value) {
                    Some(sum) => self.set_owner_units_value($column, slot, sum),
                    None => settle_latch!(ErrorV1::ArithmeticOverflow),
                }
                site = site.saturating_add(1);
            }};
        }
        let full_reservation = match order.reservation_price_units(domain.price_scale) {
            Ok(value) => value,
            Err(error) => {
                settle_latch!(error);
                return;
            }
        };
        let effective_reservation = match order.side() {
            Side::Buy => match scaled_reservation(order, effective, domain.price_scale) {
                Ok(value) => value,
                Err(error) => {
                    settle_latch!(error);
                    return;
                }
            },
            Side::Sell => 0,
        };
        add_state!(opening_reserved_cash, full_reservation);
        add_state!(
            netting_cancelled_cash,
            full_reservation - effective_reservation
        );
        add_owner!(OwnerUnitsV2::Reserved, effective_reservation);

        let mut order_value = 0u128;
        let mut outcome = 0usize;
        while outcome < outcomes {
            let reserved_leg = match order.leg_quantity(outcome as u8, order.quantity()) {
                Ok(leg) => leg,
                Err(error) => {
                    settle_latch!(error);
                    return;
                }
            };
            let cancelled_leg = match order.leg_quantity(outcome as u8, cancelled) {
                Ok(leg) => leg,
                Err(error) => {
                    settle_latch!(error);
                    return;
                }
            };
            let filled_leg = match order.leg_quantity(outcome as u8, fill) {
                Ok(leg) => leg,
                Err(error) => {
                    settle_latch!(error);
                    return;
                }
            };
            if order.side() == Side::Sell {
                for (array, leg) in [(0, reserved_leg), (1, cancelled_leg), (2, filled_leg)] {
                    let current = self.ledger_egg(array, outcome);
                    match current.checked_add(leg) {
                        Some(sum) => self.set_ledger_egg(array, outcome, sum),
                        None => settle_latch!(ErrorV1::ArithmeticOverflow),
                    }
                    site = site.saturating_add(1);
                }
            }
            if fill != 0 {
                let value = match (filled_leg as u128)
                    .checked_mul(self.state.cand.prices[outcome] as u128)
                {
                    Some(value) => value,
                    None => {
                        settle_latch!(ErrorV1::ArithmeticOverflow);
                        return;
                    }
                };
                match order_value.checked_add(value) {
                    Some(sum) => order_value = sum,
                    None => settle_latch!(ErrorV1::ArithmeticOverflow),
                }
                site = site.saturating_add(1);
                if domain.policy.rounding == RoundingBoundaryV1::ReceiptFloor && value != 0 {
                    match order.side() {
                        Side::Buy => {
                            let atoms = value.div_ceil(scale);
                            add_state!(debit_atoms, atoms);
                            add_state!(rounding_pot, atoms * scale - value);
                        }
                        Side::Sell => {
                            let atoms = value / scale;
                            add_state!(credit_atoms, atoms);
                            add_state!(rounding_pot, value - atoms * scale);
                        }
                    }
                }
            }
            outcome += 1;
        }

        if fill != 0 {
            match order.side() {
                Side::Buy => {
                    add_state!(consideration, order_value);
                    add_owner!(OwnerUnitsV2::Debit, order_value);
                    let limit = match scaled_reservation(order, fill, domain.price_scale) {
                        Ok(value) => value,
                        Err(error) => {
                            settle_latch!(error);
                            return;
                        }
                    };
                    if limit < order_value {
                        settle_latch!(ErrorV1::ConsiderationMismatch);
                        return;
                    }
                    add_state!(limit_surplus, limit - order_value);
                    match domain.policy.fee_base {
                        FeeBaseV1::None => {}
                        FeeBaseV1::FlatNotional { bps } => {
                            match order_value.checked_mul(bps as u128) {
                                Some(term) => add_owner!(OwnerUnitsV2::FeeBps, term),
                                None => settle_latch!(ErrorV1::ArithmeticOverflow),
                            }
                        }
                        FeeBaseV1::CompositeDispersionFloor { .. } => {}
                    }
                }
                Side::Sell => {
                    add_state!(seller_credit, order_value);
                    add_owner!(OwnerUnitsV2::Credit, order_value);
                    let limit = match scaled_reservation(order, fill, domain.price_scale) {
                        Ok(value) => value,
                        Err(error) => {
                            settle_latch!(error);
                            return;
                        }
                    };
                    if order_value < limit {
                        settle_latch!(ErrorV1::ConsiderationMismatch);
                        return;
                    }
                    add_state!(limit_surplus, order_value - limit);
                }
            }
        }
        let _ = site;
    }

    fn floor_order(&mut self, index: usize, order: &OrderV1, fill: u64) {
        let outcomes = self.outcomes();
        let flags = self.flags(index);
        let effective = order.quantity().saturating_sub(self.cancelled(index));
        let minimum = order.minimum_fill();
        let effective_minimum = if minimum > effective {
            effective
        } else {
            minimum
        };
        let mismatch = pos(M11_CANONICAL, V3_BLOCK_EQUALITY, 0, 0, 0);

        if flags & FLAG_POOL != 0 {
            if let OrderV1::SingleEgg(o) = order {
                let pool_index = pool_index(o.outcome as usize, o.side);
                let mut pool = self.pool(pool_index);
                if pool.ready && pool.target != 0 {
                    let product = (effective as u128) * (pool.target as u128);
                    let floor = (product / pool.total) as u64;
                    let remainder = product % pool.total;
                    if fill != floor && fill != floor.saturating_add(1) {
                        self.latch(mismatch, ErrorV1::CandidateMismatch);
                    }
                    self.set_key(
                        index,
                        PoolRowV2 {
                            remainder,
                            rank: seeded_rank(order.id(), self.state.domain.remainder_seed),
                            id: order.id(),
                            floor,
                            effective,
                            minimum: effective_minimum,
                            pool: pool_index as u8,
                            extra: fill != floor && fill == floor.saturating_add(1),
                            aon: order.partial_policy() == PartialPolicy::AllOrNone,
                        },
                    );
                    pool.floor_sum = pool.floor_sum.saturating_add(floor);
                    self.set_pool(pool_index, pool);
                } else if pool.ready && fill != 0 {
                    self.latch(mismatch, ErrorV1::CandidateMismatch);
                }
            }
        } else if flags & (FLAG_FORCED | FLAG_STRICT_FULL) != 0 {
            if fill != effective {
                self.latch(mismatch, ErrorV1::CandidateMismatch);
            }
        } else if fill != 0 {
            self.latch(mismatch, ErrorV1::CandidateMismatch);
        }

        if self.slice_checks_live() {
            let mut outcome = 0usize;
            while outcome < outcomes {
                let leg = match order.leg_quantity(outcome as u8, fill) {
                    Ok(leg) => leg,
                    Err(error) => {
                        self.latch(pos(M09_FLOWS, 0, 0, 0, 0), error);
                        0
                    }
                };
                if self.matrix(MatrixU64V2::ScratchBuy, index, outcome) != leg {
                    self.latch(pos(M12_PAIRING, 4, 0, 0, 0), ErrorV1::SliceSumMismatch);
                }
                outcome += 1;
            }
        }
    }

    // --- byte-backed active arrays ---------------------------------------

    fn owner(&self, index: usize) -> u16 {
        let at = self.control_offsets().0 + 2 * index;
        read_u16(self.bytes, at)
    }

    fn set_owner(&mut self, index: usize, value: u16) {
        let at = self.control_offsets().0 + 2 * index;
        write_u16(self.bytes, at, value);
    }

    fn order_offsets(&self) -> (usize, usize, usize, usize, usize, usize, usize) {
        let n = self.widths.orders as usize;
        let start = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Orders)
            .offset;
        let side = start + 2 * n;
        let touch = side + 8;
        let classes = touch + 2 * n;
        let flags = classes + n;
        let cancelled = flags + n;
        let keys = cancelled + 8 * n;
        (start, side, touch, classes, flags, cancelled, keys)
    }

    fn owner_slot(&self, index: usize) -> u16 {
        read_u16(self.bytes, self.order_offsets().0 + 2 * index)
    }

    fn set_owner_slot(&mut self, index: usize, value: u16) {
        write_u16(self.bytes, self.order_offsets().0 + 2 * index, value);
    }

    fn touch(&self, index: usize) -> u16 {
        read_u16(self.bytes, self.order_offsets().2 + 2 * index)
    }

    fn set_touch(&mut self, index: usize, value: u16) {
        write_u16(self.bytes, self.order_offsets().2 + 2 * index, value);
    }

    fn set_class(&mut self, index: usize, value: u8) {
        let at = self.order_offsets().3 + index;
        self.bytes[at] = value;
    }

    fn flags(&self, index: usize) -> u8 {
        self.bytes[self.order_offsets().4 + index]
    }

    fn set_flags(&mut self, index: usize, value: u8) {
        let at = self.order_offsets().4 + index;
        self.bytes[at] = value;
    }

    fn cancelled(&self, index: usize) -> u64 {
        read_u64(self.bytes, self.order_offsets().5 + 8 * index)
    }

    fn set_cancelled(&mut self, index: usize, value: u64) {
        write_u64(self.bytes, self.order_offsets().5 + 8 * index, value);
    }

    fn key(&self, index: usize) -> PoolRowV2 {
        let at = self.order_offsets().6 + 59 * index;
        PoolRowV2 {
            remainder: read_u128(self.bytes, at),
            rank: read_u64(self.bytes, at + 16),
            id: read_u64(self.bytes, at + 24),
            floor: read_u64(self.bytes, at + 32),
            effective: read_u64(self.bytes, at + 40),
            minimum: read_u64(self.bytes, at + 48),
            pool: self.bytes[at + 56],
            extra: self.bytes[at + 57] != 0,
            aon: self.bytes[at + 58] != 0,
        }
    }

    fn set_key(&mut self, index: usize, value: PoolRowV2) {
        let at = self.order_offsets().6 + 59 * index;
        write_u128(self.bytes, at, value.remainder);
        write_u64(self.bytes, at + 16, value.rank);
        write_u64(self.bytes, at + 24, value.id);
        write_u64(self.bytes, at + 32, value.floor);
        write_u64(self.bytes, at + 40, value.effective);
        write_u64(self.bytes, at + 48, value.minimum);
        self.bytes[at + 56] = value.pool;
        self.bytes[at + 57] = value.extra as u8;
        self.bytes[at + 58] = value.aon as u8;
    }

    fn matrix(&self, matrix: MatrixU64V2, row: usize, outcome: usize) -> u64 {
        let at = matrix_offset(
            LayoutV2::new(self.widths),
            self.widths,
            matrix,
            row,
            outcome,
        )
        .unwrap_or_else(|_| unreachable!());
        read_u64(self.bytes, at)
    }

    fn set_matrix(&mut self, matrix: MatrixU64V2, row: usize, outcome: usize, value: u64) {
        let at = matrix_offset(
            LayoutV2::new(self.widths),
            self.widths,
            matrix,
            row,
            outcome,
        )
        .unwrap_or_else(|_| unreachable!());
        write_u64(self.bytes, at, value);
    }

    fn cell_portfolio(&self, owner: usize) -> u16 {
        let n = self.widths.orders as usize;
        let o = self.widths.outcomes as usize;
        let start = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Scratch)
            .offset
            + 16 * n * o;
        read_u16(self.bytes, start + 2 * owner)
    }

    fn set_cell_portfolio(&mut self, owner: usize, value: u16) {
        let n = self.widths.orders as usize;
        let o = self.widths.outcomes as usize;
        let start = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Scratch)
            .offset
            + 16 * n * o;
        write_u16(self.bytes, start + 2 * owner, value);
    }

    fn flow(&self, side: OutcomeFlowV2, outcome: usize) -> u128 {
        let at = outcome_flow_offset(LayoutV2::new(self.widths), self.widths, side, outcome)
            .unwrap_or_else(|_| unreachable!());
        read_u128(self.bytes, at)
    }

    fn set_flow(&mut self, side: OutcomeFlowV2, outcome: usize, value: u128) {
        let at = outcome_flow_offset(LayoutV2::new(self.widths), self.widths, side, outcome)
            .unwrap_or_else(|_| unreachable!());
        write_u128(self.bytes, at, value);
    }

    fn part(&self, buy: bool, owner: usize, outcome: usize) -> u64 {
        self.matrix(
            if buy {
                MatrixU64V2::ParticipationBuy
            } else {
                MatrixU64V2::ParticipationSell
            },
            owner,
            outcome,
        )
    }

    fn set_part(&mut self, buy: bool, owner: usize, outcome: usize, value: u64) {
        self.set_matrix(
            if buy {
                MatrixU64V2::ParticipationBuy
            } else {
                MatrixU64V2::ParticipationSell
            },
            owner,
            outcome,
            value,
        );
    }

    fn agg(&self, outcome: usize) -> OutcomeAggV2 {
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Pools)
            .offset
            + 128 * outcome;
        OutcomeAggV2 {
            demand: read_u128(self.bytes, at),
            supply: read_u128(self.bytes, at + 16),
            forced_buy: read_u128(self.bytes, at + 32),
            forced_sell: read_u128(self.bytes, at + 48),
            forced_aon_buy: read_u128(self.bytes, at + 64),
            forced_aon_sell: read_u128(self.bytes, at + 80),
            strict_buy: read_u128(self.bytes, at + 96),
            strict_sell: read_u128(self.bytes, at + 112),
        }
    }

    fn set_agg(&mut self, outcome: usize, value: OutcomeAggV2) {
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Pools)
            .offset
            + 128 * outcome;
        for (offset, field) in [
            (0, value.demand),
            (16, value.supply),
            (32, value.forced_buy),
            (48, value.forced_sell),
            (64, value.forced_aon_buy),
            (80, value.forced_aon_sell),
            (96, value.strict_buy),
            (112, value.strict_sell),
        ] {
            write_u128(self.bytes, at + offset, field);
        }
    }

    fn pool(&self, index: usize) -> PoolV2 {
        let o = self.widths.outcomes as usize;
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Pools)
            .offset
            + 128 * o
            + 36 * index;
        PoolV2 {
            total: read_u128(self.bytes, at),
            count: read_u16(self.bytes, at + 16),
            target: read_u64(self.bytes, at + 18),
            floor_sum: read_u64(self.bytes, at + 26),
            ready: self.bytes[at + 34] != 0,
            dust_rejected: self.bytes[at + 35] != 0,
        }
    }

    fn set_pool(&mut self, index: usize, value: PoolV2) {
        let o = self.widths.outcomes as usize;
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Pools)
            .offset
            + 128 * o
            + 36 * index;
        write_u128(self.bytes, at, value.total);
        write_u16(self.bytes, at + 16, value.count);
        write_u64(self.bytes, at + 18, value.target);
        write_u64(self.bytes, at + 26, value.floor_sum);
        self.bytes[at + 34] = value.ready as u8;
        self.bytes[at + 35] = value.dust_rejected as u8;
    }

    fn owner_units_value(&self, column: OwnerUnitsV2, owner: usize) -> u128 {
        let at = owner_units_offset(LayoutV2::new(self.widths), self.widths, column, owner)
            .unwrap_or_else(|_| unreachable!());
        read_u128(self.bytes, at)
    }

    fn set_owner_units_value(&mut self, column: OwnerUnitsV2, owner: usize, value: u128) {
        let at = owner_units_offset(LayoutV2::new(self.widths), self.widths, column, owner)
            .unwrap_or_else(|_| unreachable!());
        write_u128(self.bytes, at, value);
    }

    fn ledger_egg(&self, array: usize, outcome: usize) -> u64 {
        let u = self.widths.owners as usize;
        let o = self.widths.outcomes as usize;
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Ledger)
            .offset
            + 64 * u
            + 8 * (array * o + outcome);
        read_u64(self.bytes, at)
    }

    fn set_ledger_egg(&mut self, array: usize, outcome: usize, value: u64) {
        let u = self.widths.owners as usize;
        let o = self.widths.outcomes as usize;
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Ledger)
            .offset
            + 64 * u
            + 8 * (array * o + outcome);
        write_u64(self.bytes, at, value);
    }

    fn slice_used(&self, merge: bool, outcome: usize) -> u64 {
        let o = self.widths.outcomes as usize;
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Slices)
            .offset
            + 8 * ((merge as usize) * o + outcome);
        read_u64(self.bytes, at)
    }

    fn set_slice_used(&mut self, merge: bool, outcome: usize, value: u64) {
        let o = self.widths.outcomes as usize;
        let at = LayoutV2::new(self.widths)
            .span(ClearWorkRegionV2::Slices)
            .offset
            + 8 * ((merge as usize) * o + outcome);
        write_u64(self.bytes, at, value);
    }

    fn control_offsets(&self) -> (usize, usize) {
        let o = self.widths.outcomes as usize;
        let u = self.widths.owners as usize;
        let owners = 161 + 8 * o + V1_CAND_TAIL_BYTES;
        (owners, owners + 2 * u)
    }
}

fn decode_state(
    input: &[u8],
    widths: ClearWorkWidthsV2,
) -> Result<EngineStateV2, ClearWorkFaultV2> {
    let o = widths.outcomes as usize;
    let u = widths.owners as usize;
    let candidate_tail = 161 + 8 * o;
    let owner_slots_at = candidate_tail + V1_CAND_TAIL_BYTES + 2 * u;
    let ledger = LayoutV2::new(widths).span(ClearWorkRegionV2::Ledger);
    let cash = ledger.offset + 64 * u + 24 * o;
    Ok(EngineStateV2 {
        phase: input[0],
        pass: input[1],
        order_passes: input[2],
        slices_after_pass: input[3],
        slices_expected: input[4] != 0,
        check_claims: input[5] != 0,
        cursor: read_u16(input, 6),
        slice_cursor: read_u16(input, 8),
        order_count: read_u16(input, 10),
        latch_set: input[12] != 0,
        latch_position: read_u64(input, 13),
        latch_error: decode_error(&input[21..25])?,
        fold: read_digest_fold(input, 25),
        sealed_fold: read_digest_fold(input, 41),
        digest: read_digest_fold(input, 57),
        previous_id: read_u64(input, 73),
        portfolio_count: input[81],
        domain: decode_domain(input)?,
        cand: decode_candidate(input, widths)?,
        owner_slots: read_u16(input, owner_slots_at),
        side_buy_bits: read_u64(
            input,
            LayoutV2::new(widths).span(ClearWorkRegionV2::Orders).offset
                + 2 * widths.orders as usize,
        ),
        opening_reserved_cash: read_u128(input, cash),
        netting_cancelled_cash: read_u128(input, cash + 16),
        consideration: read_u128(input, cash + 32),
        seller_credit: read_u128(input, cash + 48),
        limit_surplus: read_u128(input, cash + 64),
        debit_atoms: read_u128(input, cash + 80),
        credit_atoms: read_u128(input, cash + 96),
        rounding_pot: read_u128(input, cash + 112),
        summary: decode_summary(input, widths),
        summary_valid: input[LayoutV2::new(widths).span(ClearWorkRegionV2::Summary).end() - 1] != 0,
    })
}

fn encode_state(output: &mut [u8], widths: ClearWorkWidthsV2, state: &EngineStateV2) {
    let o = widths.outcomes as usize;
    let u = widths.owners as usize;
    output[0] = state.phase;
    output[1] = state.pass;
    output[2] = state.order_passes;
    output[3] = state.slices_after_pass;
    output[4] = state.slices_expected as u8;
    output[5] = state.check_claims as u8;
    write_u16(output, 6, state.cursor);
    write_u16(output, 8, state.slice_cursor);
    write_u16(output, 10, state.order_count);
    output[12] = state.latch_set as u8;
    write_u64(output, 13, state.latch_position);
    encode_error(&mut output[21..25], state.latch_error);
    write_digest_fold(output, 25, state.fold);
    write_digest_fold(output, 41, state.sealed_fold);
    write_digest_fold(output, 57, state.digest);
    write_u64(output, 73, state.previous_id);
    output[81] = state.portfolio_count;
    encode_domain(output, &state.domain);
    encode_candidate(output, widths, &state.cand);
    let owner_slots_at = 161 + 8 * o + V1_CAND_TAIL_BYTES + 2 * u;
    write_u16(output, owner_slots_at, state.owner_slots);
    let side =
        LayoutV2::new(widths).span(ClearWorkRegionV2::Orders).offset + 2 * widths.orders as usize;
    write_u64(output, side, state.side_buy_bits);
    let ledger = LayoutV2::new(widths).span(ClearWorkRegionV2::Ledger);
    let cash = ledger.offset + 64 * u + 24 * o;
    for (offset, value) in [
        (0, state.opening_reserved_cash),
        (16, state.netting_cancelled_cash),
        (32, state.consideration),
        (48, state.seller_credit),
        (64, state.limit_surplus),
        (80, state.debit_atoms),
        (96, state.credit_atoms),
        (112, state.rounding_pot),
    ] {
        write_u128(output, cash + offset, value);
    }
    encode_summary(output, widths, &state.summary);
    let valid = LayoutV2::new(widths).span(ClearWorkRegionV2::Summary).end() - 1;
    output[valid] = state.summary_valid as u8;
}

fn decode_domain(input: &[u8]) -> Result<RelationDomainV1, ClearWorkFaultV2> {
    Ok(RelationDomainV1 {
        relation_version: read_u32(input, 82),
        market_id: read_u64(input, 86),
        book_id: read_u64(input, 94),
        epoch: read_u64(input, 102),
        policy_id: read_u64(input, 110),
        order_set_id: read_u64(input, 118),
        outcome_count: input[126],
        owner_count: read_u16(input, 127),
        price_scale: read_u64(input, 129),
        remainder_seed: read_u64(input, 137),
        policy: decode_policy_v1(&input[145..160]).map_err(ClearWorkFaultV2::V1Codec)?,
    })
}

fn encode_domain(output: &mut [u8], domain: &RelationDomainV1) {
    write_u32(output, 82, domain.relation_version);
    write_u64(output, 86, domain.market_id);
    write_u64(output, 94, domain.book_id);
    write_u64(output, 102, domain.epoch);
    write_u64(output, 110, domain.policy_id);
    write_u64(output, 118, domain.order_set_id);
    output[126] = domain.outcome_count;
    write_u16(output, 127, domain.owner_count);
    write_u64(output, 129, domain.price_scale);
    write_u64(output, 137, domain.remainder_seed);
    let result = crate::relation_v1_stream::encode_policy_v1(&domain.policy, &mut output[145..160]);
    debug_assert!(result.is_ok());
}

fn decode_candidate(
    input: &[u8],
    widths: ClearWorkWidthsV2,
) -> Result<StreamCandidateV1, ClearWorkFaultV2> {
    let o = widths.outcomes as usize;
    let mut prices = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < o {
        prices[i] = read_u64(input, 161 + 8 * i);
        i += 1;
    }
    let tail = 161 + 8 * o;
    let declared = match input[tail + 98] {
        0 => None,
        1 => Some(read_u16(input, tail + 99)),
        _ => return Err(ClearWorkFaultV2::InvalidSliceDeclaration),
    };
    Ok(StreamCandidateV1 {
        order_len: input[160],
        prices,
        virtual_split: read_u64(input, tail),
        virtual_merge: read_u64(input, tail + 8),
        honored_aon_mask: read_u64(input, tail + 16),
        claimed_score: decode_score(input, tail + 24),
        canonical_candidate_digest: read_u128(input, tail + 82),
        declared_slices: declared,
    })
}

fn encode_candidate(output: &mut [u8], widths: ClearWorkWidthsV2, candidate: &StreamCandidateV1) {
    let o = widths.outcomes as usize;
    output[160] = candidate.order_len;
    let mut i = 0usize;
    while i < o {
        write_u64(output, 161 + 8 * i, candidate.prices[i]);
        i += 1;
    }
    let tail = 161 + 8 * o;
    write_u64(output, tail, candidate.virtual_split);
    write_u64(output, tail + 8, candidate.virtual_merge);
    write_u64(output, tail + 16, candidate.honored_aon_mask);
    encode_score(output, tail + 24, &candidate.claimed_score);
    write_u128(output, tail + 82, candidate.canonical_candidate_digest);
    match candidate.declared_slices {
        None => {
            output[tail + 98] = 0;
            write_u16(output, tail + 99, 0);
        }
        Some(count) => {
            output[tail + 98] = 1;
            write_u16(output, tail + 99, count);
        }
    }
}

fn decode_summary(input: &[u8], widths: ClearWorkWidthsV2) -> SummaryV1 {
    let o = widths.outcomes as usize;
    let at = LayoutV2::new(widths)
        .span(ClearWorkRegionV2::Summary)
        .offset;
    let mut summary = summary_zero();
    summary.outcome_count = input[at];
    let mut cursor = at + 1;
    decode_active_egg(input, &mut cursor, o, &mut summary.buy_flow);
    decode_active_egg(input, &mut cursor, o, &mut summary.sell_flow);
    decode_active_egg(input, &mut cursor, o, &mut summary.total_flow);
    decode_active_egg(input, &mut cursor, o, &mut summary.direct_flow);
    summary.virtual_split = read_advance_u64(input, &mut cursor);
    summary.virtual_merge = read_advance_u64(input, &mut cursor);
    decode_active_egg(input, &mut cursor, o, &mut summary.opening_reserved_egg);
    decode_active_egg(input, &mut cursor, o, &mut summary.unfilled_refund_egg);
    decode_active_egg(input, &mut cursor, o, &mut summary.netting_cancelled_egg);
    summary.opening_reserved_cash_price_units = read_advance_u128(input, &mut cursor);
    summary.buyer_consideration_price_units = read_advance_u128(input, &mut cursor);
    summary.seller_credit_price_units = read_advance_u128(input, &mut cursor);
    summary.split_cost_price_units = read_advance_u128(input, &mut cursor);
    summary.merge_proceeds_price_units = read_advance_u128(input, &mut cursor);
    summary.fee_price_units = read_advance_u128(input, &mut cursor);
    summary.fee_carry_bps_units = read_advance_u128(input, &mut cursor);
    summary.cash_refund_price_units = read_advance_u128(input, &mut cursor);
    summary.rounding_pot_price_units = read_advance_u128(input, &mut cursor);
    summary.debit_atoms = read_advance_u128(input, &mut cursor);
    summary.credit_atoms = read_advance_u128(input, &mut cursor);
    summary.distinct_participating_owners = read_u16(input, cursor);
    cursor += 2;
    summary.self_overlap_volume = read_advance_u64(input, &mut cursor);
    summary.score = decode_score(input, cursor);
    cursor += 58;
    summary.candidate_digest = read_u128(input, cursor);
    summary
}

fn encode_summary(output: &mut [u8], widths: ClearWorkWidthsV2, summary: &SummaryV1) {
    let o = widths.outcomes as usize;
    let at = LayoutV2::new(widths)
        .span(ClearWorkRegionV2::Summary)
        .offset;
    output[at] = summary.outcome_count;
    let mut cursor = at + 1;
    encode_active_egg(output, &mut cursor, o, &summary.buy_flow);
    encode_active_egg(output, &mut cursor, o, &summary.sell_flow);
    encode_active_egg(output, &mut cursor, o, &summary.total_flow);
    encode_active_egg(output, &mut cursor, o, &summary.direct_flow);
    write_advance_u64(output, &mut cursor, summary.virtual_split);
    write_advance_u64(output, &mut cursor, summary.virtual_merge);
    encode_active_egg(output, &mut cursor, o, &summary.opening_reserved_egg);
    encode_active_egg(output, &mut cursor, o, &summary.unfilled_refund_egg);
    encode_active_egg(output, &mut cursor, o, &summary.netting_cancelled_egg);
    for value in [
        summary.opening_reserved_cash_price_units,
        summary.buyer_consideration_price_units,
        summary.seller_credit_price_units,
        summary.split_cost_price_units,
        summary.merge_proceeds_price_units,
        summary.fee_price_units,
        summary.fee_carry_bps_units,
        summary.cash_refund_price_units,
        summary.rounding_pot_price_units,
        summary.debit_atoms,
        summary.credit_atoms,
    ] {
        write_advance_u128(output, &mut cursor, value);
    }
    write_u16(output, cursor, summary.distinct_participating_owners);
    cursor += 2;
    write_advance_u64(output, &mut cursor, summary.self_overlap_volume);
    encode_score(output, cursor, &summary.score);
    cursor += 58;
    write_u128(output, cursor, summary.candidate_digest);
}

fn summary_zero() -> SummaryV1 {
    SummaryV1 {
        outcome_count: 0,
        buy_flow: [0; MAX_OUTCOMES],
        sell_flow: [0; MAX_OUTCOMES],
        total_flow: [0; MAX_OUTCOMES],
        direct_flow: [0; MAX_OUTCOMES],
        virtual_split: 0,
        virtual_merge: 0,
        opening_reserved_egg: [0; MAX_OUTCOMES],
        unfilled_refund_egg: [0; MAX_OUTCOMES],
        netting_cancelled_egg: [0; MAX_OUTCOMES],
        opening_reserved_cash_price_units: 0,
        buyer_consideration_price_units: 0,
        seller_credit_price_units: 0,
        split_cost_price_units: 0,
        merge_proceeds_price_units: 0,
        fee_price_units: 0,
        fee_carry_bps_units: 0,
        cash_refund_price_units: 0,
        rounding_pot_price_units: 0,
        debit_atoms: 0,
        credit_atoms: 0,
        distinct_participating_owners: 0,
        self_overlap_volume: 0,
        score: ScoreV1::ZERO,
        candidate_digest: 0,
    }
}

fn decode_score(input: &[u8], at: usize) -> ScoreV1 {
    ScoreV1 {
        weighted_direct_volume: read_i128(input, at),
        limit_surplus_price_units: read_u128(input, at + 16),
        distinct_owners: read_u16(input, at + 32),
        churn: read_u64(input, at + 34),
        digest: read_u128(input, at + 42),
    }
}

fn encode_score(output: &mut [u8], at: usize, score: &ScoreV1) {
    write_i128(output, at, score.weighted_direct_volume);
    write_u128(output, at + 16, score.limit_surplus_price_units);
    write_u16(output, at + 32, score.distinct_owners);
    write_u64(output, at + 34, score.churn);
    write_u128(output, at + 42, score.digest);
}

fn read_digest_fold(input: &[u8], at: usize) -> DigestFoldV1 {
    DigestFoldV1::from_words(read_u64(input, at), read_u64(input, at + 8))
}

fn write_digest_fold(output: &mut [u8], at: usize, fold: DigestFoldV1) {
    let (high, low) = fold.words();
    write_u64(output, at, high);
    write_u64(output, at + 8, low);
}

fn decode_active_egg(
    input: &[u8],
    cursor: &mut usize,
    outcomes: usize,
    target: &mut [u64; MAX_OUTCOMES],
) {
    let mut i = 0usize;
    while i < outcomes {
        target[i] = read_advance_u64(input, cursor);
        i += 1;
    }
}

fn encode_active_egg(
    output: &mut [u8],
    cursor: &mut usize,
    outcomes: usize,
    source: &[u64; MAX_OUTCOMES],
) {
    let mut i = 0usize;
    while i < outcomes {
        write_advance_u64(output, cursor, source[i]);
        i += 1;
    }
}

fn read_advance_u64(input: &[u8], cursor: &mut usize) -> u64 {
    let value = read_u64(input, *cursor);
    *cursor += 8;
    value
}

fn read_advance_u128(input: &[u8], cursor: &mut usize) -> u128 {
    let value = read_u128(input, *cursor);
    *cursor += 16;
    value
}

fn write_advance_u64(output: &mut [u8], cursor: &mut usize, value: u64) {
    write_u64(output, *cursor, value);
    *cursor += 8;
}

fn write_advance_u128(output: &mut [u8], cursor: &mut usize, value: u128) {
    write_u128(output, *cursor, value);
    *cursor += 16;
}

fn read_u32(input: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([input[at], input[at + 1], input[at + 2], input[at + 3]])
}

fn read_i128(input: &[u8], at: usize) -> i128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&input[at..at + 16]);
    i128::from_le_bytes(bytes)
}

fn write_u16(output: &mut [u8], at: usize, value: u16) {
    output[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut [u8], at: usize, value: u32) {
    output[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_i128(output: &mut [u8], at: usize, value: i128) {
    output[at..at + 16].copy_from_slice(&value.to_le_bytes());
}

fn decode_error(input: &[u8]) -> Result<ErrorV1, ClearWorkFaultV2> {
    let code = input[0];
    let outcome = input[1];
    let owner = read_u16(input, 2);
    Ok(match code {
        0 => ErrorV1::UnknownRelationVersion,
        1 => ErrorV1::InvalidPriceScale,
        2 => ErrorV1::PolicyVariantUnimplemented,
        3 => ErrorV1::InvalidOwner,
        4 => ErrorV1::InvalidOutcome,
        5 => ErrorV1::InvalidQuantity,
        6 => ErrorV1::InvalidMinimumFill,
        7 => ErrorV1::NonCanonicalOrderOrder,
        8 => ErrorV1::NonCanonicalPadding,
        9 => ErrorV1::AonNotAdmitted,
        10 => ErrorV1::MinimumFillNotAdmitted,
        11 => ErrorV1::SelfCrossRefused,
        12 => ErrorV1::ExpiredOrder,
        13 => ErrorV1::TooManyOrders,
        14 => ErrorV1::TooManyPortfolios,
        15 => ErrorV1::SimplexSumMismatch,
        16 => ErrorV1::PriceOutOfRange,
        17 => ErrorV1::IneligibleFill,
        18 => ErrorV1::CandidateMismatch,
        19 => ErrorV1::StrictUnderfill,
        20 => ErrorV1::FillExceedsQuantity,
        21 => ErrorV1::MinimumFillViolation,
        22 => ErrorV1::AllOrNoneViolation,
        23 => ErrorV1::AonMaskDishonored,
        24 => ErrorV1::AonMaskLeak,
        25 => ErrorV1::AonMaskNotApplicable,
        26 => ErrorV1::DustRejected,
        27 => ErrorV1::OutcomeConservationMismatch,
        28 => ErrorV1::ChurnNotCanonical,
        29 => ErrorV1::InfeasibleVirtualLeg,
        30 => ErrorV1::PairingInfeasible { outcome, owner },
        31 => ErrorV1::SliceNotExecutable,
        32 => ErrorV1::SliceSumMismatch,
        33 => ErrorV1::PairingWitnessNotAdmitted,
        34 => ErrorV1::PairingWitnessMissing,
        35 => ErrorV1::ConstructorStalled,
        36 => ErrorV1::SliceCapacityExceeded,
        37 => ErrorV1::ConsiderationMismatch,
        38 => ErrorV1::RemainderRequired,
        39 => ErrorV1::FeeMismatch,
        40 => ErrorV1::FeePayerUnfunded,
        41 => ErrorV1::ConservationFailure,
        42 => ErrorV1::ScoreMismatch,
        43 => ErrorV1::DigestMismatch,
        44 => ErrorV1::ArithmeticOverflow,
        45 => ErrorV1::NoValidCandidate,
        46 => ErrorV1::SearchBudgetExceeded,
        47 => ErrorV1::PriceOutsideMomentCone { outcome },
        _ => return Err(ClearWorkFaultV2::InvalidErrorCode),
    })
}

fn encode_error(output: &mut [u8], error: ErrorV1) {
    let (code, outcome, owner) = match error {
        ErrorV1::PairingInfeasible { outcome, owner } => (30, outcome, owner),
        ErrorV1::PriceOutsideMomentCone { outcome } => (47, outcome, 0),
        other => (error_code(other), 0, 0),
    };
    output[0] = code;
    output[1] = outcome;
    write_u16(output, 2, owner);
}

fn error_code(error: ErrorV1) -> u8 {
    match error {
        ErrorV1::UnknownRelationVersion => 0,
        ErrorV1::InvalidPriceScale => 1,
        ErrorV1::PolicyVariantUnimplemented => 2,
        ErrorV1::InvalidOwner => 3,
        ErrorV1::InvalidOutcome => 4,
        ErrorV1::InvalidQuantity => 5,
        ErrorV1::InvalidMinimumFill => 6,
        ErrorV1::NonCanonicalOrderOrder => 7,
        ErrorV1::NonCanonicalPadding => 8,
        ErrorV1::AonNotAdmitted => 9,
        ErrorV1::MinimumFillNotAdmitted => 10,
        ErrorV1::SelfCrossRefused => 11,
        ErrorV1::ExpiredOrder => 12,
        ErrorV1::TooManyOrders => 13,
        ErrorV1::TooManyPortfolios => 14,
        ErrorV1::SimplexSumMismatch => 15,
        ErrorV1::PriceOutOfRange => 16,
        ErrorV1::IneligibleFill => 17,
        ErrorV1::CandidateMismatch => 18,
        ErrorV1::StrictUnderfill => 19,
        ErrorV1::FillExceedsQuantity => 20,
        ErrorV1::MinimumFillViolation => 21,
        ErrorV1::AllOrNoneViolation => 22,
        ErrorV1::AonMaskDishonored => 23,
        ErrorV1::AonMaskLeak => 24,
        ErrorV1::AonMaskNotApplicable => 25,
        ErrorV1::DustRejected => 26,
        ErrorV1::OutcomeConservationMismatch => 27,
        ErrorV1::ChurnNotCanonical => 28,
        ErrorV1::InfeasibleVirtualLeg => 29,
        ErrorV1::PairingInfeasible { .. } => 30,
        ErrorV1::SliceNotExecutable => 31,
        ErrorV1::SliceSumMismatch => 32,
        ErrorV1::PairingWitnessNotAdmitted => 33,
        ErrorV1::PairingWitnessMissing => 34,
        ErrorV1::ConstructorStalled => 35,
        ErrorV1::SliceCapacityExceeded => 36,
        ErrorV1::ConsiderationMismatch => 37,
        ErrorV1::RemainderRequired => 38,
        ErrorV1::FeeMismatch => 39,
        ErrorV1::FeePayerUnfunded => 40,
        ErrorV1::ConservationFailure => 41,
        ErrorV1::ScoreMismatch => 42,
        ErrorV1::DigestMismatch => 43,
        ErrorV1::ArithmeticOverflow => 44,
        ErrorV1::NoValidCandidate => 45,
        ErrorV1::SearchBudgetExceeded => 46,
        ErrorV1::PriceOutsideMomentCone { .. } => 47,
    }
}

fn pool_index(outcome: usize, side: Side) -> usize {
    outcome * 2
        + match side {
            Side::Buy => 0,
            Side::Sell => 1,
        }
}

fn key_beats(a: &PoolRowV2, b: &PoolRowV2) -> bool {
    a.remainder > b.remainder
        || (a.remainder == b.remainder && (a.rank < b.rank || (a.rank == b.rank && a.id < b.id)))
}
