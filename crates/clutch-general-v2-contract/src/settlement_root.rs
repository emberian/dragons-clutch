// SPDX-License-Identifier: AGPL-3.0-or-later

//! Counted candidate-scoped owner for the General V2 settlement graph.
//!
//! This account replaces the legacy SelectedCandidate handoff whose settlement
//! children were not counted. It owns immutable selection identity, the exact expected
//! settlement-child cardinalities, disjoint child-rent compartments, and all
//! live-child counters. No method in this module moves cash, Eggs, fees, rent,
//! or lamports; an adapter must compose those physical transitions atomically
//! with the returned root successor.

use clutch_owner_settlement::{
    AuthenticatedFinalPotV1, SettlementCashPotExpectationV1, SettlementCashPotV1,
    VirtualCashDirectionV1, VirtualInventoryStateV1, VirtualReceiptKindV1, MAX_OUTCOMES,
};

use crate::{
    AdmissionNodeStatusV1, AdmissionNodeV4AccountV1, CandidateFeedHeaderV2,
    CandidateWindowV5AccountV1, CodecError, DeletableRentOwnerV1, FirstAdmittedTieV1,
    GeneralEpochPhaseV1, GeneralEpochV6AccountV1, Id32, MarketBindingV2, Reader,
    SettlementCandidateKindV1, Sha256BackendV1, Writer, FINAL_POT_ACCOUNT_BYTES,
    FINAL_POT_ACCOUNT_TAG, FINAL_POT_ACCOUNT_VERSION, ID_BYTES, SCORE_V2_Q_RANK_CAPACITY,
};

/// Fresh centrally reserved SettlementRoot account discriminator.
pub const SETTLEMENT_ROOT_ACCOUNT_TAG: u8 = 0xa9;
/// First SettlementRoot schema version.
pub const SETTLEMENT_ROOT_ACCOUNT_VERSION: u8 = 1;
/// Exact fixed width of [`SettlementRootV1AccountV1`].
pub const SETTLEMENT_ROOT_ACCOUNT_BYTES: usize = 980;
/// Fresh candidate-scoped root PDA seed domain.
pub const SETTLEMENT_ROOT_SEED_DOMAIN_V1: &[u8] = b"general-settlement-root:v1";
/// Full-account terminal semantic-ID domain.
pub const SETTLEMENT_ROOT_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-settlement-root-data/v1";

const IDENTITY_COUNT: usize = 19;

/// Canonical root PDA tuple: Epoch PDA plus final SettlementCandidateId.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementRootSeedTupleV1 {
    epoch: [u8; 32],
    candidate: [u8; 32],
}

impl SettlementRootSeedTupleV1 {
    /// Construct only from two distinct live identities.
    pub fn new(epoch: Id32, candidate: Id32) -> Result<Self, CodecError> {
        require_live(epoch)?;
        require_live(candidate)?;
        if epoch == candidate {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(Self {
            epoch: epoch.bytes(),
            candidate: candidate.bytes(),
        })
    }

    /// First seed.
    pub const fn domain(&self) -> &'static [u8] {
        SETTLEMENT_ROOT_SEED_DOMAIN_V1
    }

    /// Second seed: the authenticated Epoch PDA.
    pub const fn epoch(&self) -> &[u8; 32] {
        &self.epoch
    }

    /// Third seed: the stable final candidate identity.
    pub const fn candidate(&self) -> &[u8; 32] {
        &self.candidate
    }
}

/// Optional child-rent compartment whose absence is canonical all-zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptionalSettlementRentV1 {
    /// Payer/refund owner, absent only with zero amounts.
    pub payer: Id32,
    /// Exact refundable principal.
    pub refundable_principal: u64,
    /// Hostile prefund routed only to the neutral sink.
    pub donation_floor: u64,
}

impl OptionalSettlementRentV1 {
    /// Canonical absent compartment.
    pub const ABSENT: Self = Self {
        payer: Id32::ZERO,
        refundable_principal: 0,
        donation_floor: 0,
    };

    /// Promote one required rent owner into the optional encoding.
    pub fn present(rent: DeletableRentOwnerV1) -> Result<Self, CodecError> {
        rent.validate()?;
        Ok(Self {
            payer: rent.payer,
            refundable_principal: rent.refundable_principal,
            donation_floor: rent.donation_floor,
        })
    }

    /// Decode its semantic presence.
    pub fn get(self) -> Result<Option<DeletableRentOwnerV1>, CodecError> {
        if self == Self::ABSENT {
            return Ok(None);
        }
        let rent = DeletableRentOwnerV1 {
            payer: self.payer,
            refundable_principal: self.refundable_principal,
            donation_floor: self.donation_floor,
        };
        rent.validate()?;
        Ok(Some(rent))
    }
}

/// Lifecycle of one expected singleton settlement child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementRootChildStateV1 {
    /// This direction has no such child.
    Absent = 0,
    /// The root owns its identity/rent, but creation awaits a checked transition.
    ExpectedUncreated = 1,
    /// The exact child account is live.
    Live = 2,
    /// The exact child was retired under its terminal semantic owner.
    Retired = 3,
}

impl SettlementRootChildStateV1 {
    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Absent),
            1 => Ok(Self::ExpectedUncreated),
            2 => Ok(Self::Live),
            3 => Ok(Self::Retired),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// Root lifecycle independent of any one settlement child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SettlementRootPhaseV1 {
    /// Action 24 is creating the exact receipt/row/reservation graph.
    Materializing = 0,
    /// Materialization is complete and value/accounting transitions may run.
    Settling = 1,
    /// Value movement is terminal; dependency-ordered retirement may run.
    Retiring = 2,
    /// Every expected child was observed and every live child retired.
    Terminal = 3,
}

impl SettlementRootPhaseV1 {
    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Materializing),
            1 => Ok(Self::Settling),
            2 => Ok(Self::Retiring),
            3 => Ok(Self::Terminal),
            _ => Err(CodecError::InvalidState),
        }
    }
}

/// Exhaustive expected/admitted/live settlement child cardinalities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementRootChildCountsV1 {
    /// One receipt per selected slice.
    pub expected_receipts: u16,
    /// Receipts whose exact V4 accounts were created.
    pub admitted_receipts: u16,
    /// Created V4 receipts not yet dependency-ordered closed.
    pub live_receipts: u16,
    /// Distinct participating owner rows.
    pub expected_owner_rows: u16,
    /// Owner rows created at their first canonical slice.
    pub admitted_owner_rows: u16,
    /// Owner rows not yet terminally closed.
    pub live_owner_rows: u16,
    /// Every active frozen order Reservation, filled or unfilled.
    pub expected_reservations: u16,
    /// Distinct filled Reservations expected to be adopted by action 24.
    pub expected_filled_reservations: u16,
    /// Filled Reservations brought under this root by action 24.
    pub admitted_reservations: u16,
    /// Filled adopted Reservations not yet terminally closed.
    pub live_reservations: u16,
    /// Unfilled Reservations atomically released and closed by action 41.
    pub released_unfilled_reservations: u16,
    /// Owners whose row/Position/pot/Replay action-38 transition completed.
    pub completed_owner_finalizations: u16,
    /// Fee-bearing rent-owned `0x83/4` receipts not yet terminally consumed.
    /// Zero-fee owners instead retain exact action-38 GEN1 Replay evidence.
    pub live_fee_finalizations: u16,
    /// Dealer children admitted by the selected route.
    pub expected_dealer_children: u16,
    /// Dealer children admitted under the typed root hook.
    pub admitted_dealer_children: u16,
    /// Dealer children not yet terminally retired.
    pub live_dealer_children: u16,
    /// Merge receipts that require the later action-40 paid latch.
    pub expected_merge_payments: u16,
    /// Merge receipts materialized as exact V4 children.
    pub admitted_merge_payments: u16,
    /// Merge payment latches completed by action 40.
    pub completed_merge_payments: u16,
}

impl SettlementRootChildCountsV1 {
    /// Validate every admitted/live/completed bound.
    pub fn validate(self) -> Result<(), CodecError> {
        for (expected, admitted, live) in [
            (
                self.expected_receipts,
                self.admitted_receipts,
                self.live_receipts,
            ),
            (
                self.expected_owner_rows,
                self.admitted_owner_rows,
                self.live_owner_rows,
            ),
            (
                self.expected_owner_rows,
                self.completed_owner_finalizations,
                self.live_fee_finalizations,
            ),
            (
                self.expected_dealer_children,
                self.admitted_dealer_children,
                self.live_dealer_children,
            ),
        ] {
            if admitted > expected || live > admitted {
                return Err(CodecError::InvalidCount);
            }
        }
        let expected_unfilled = self
            .expected_reservations
            .checked_sub(self.expected_filled_reservations)
            .ok_or(CodecError::InvalidCount)?;
        if self.admitted_reservations > self.expected_filled_reservations
            || self.live_reservations > self.admitted_reservations
            || self.released_unfilled_reservations > expected_unfilled
            || self.admitted_merge_payments > self.expected_merge_payments
            || self.completed_merge_payments > self.admitted_merge_payments
        {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }

    fn terminal(self) -> bool {
        let Some(expected_unfilled) = self
            .expected_reservations
            .checked_sub(self.expected_filled_reservations)
        else {
            return false;
        };
        self.admitted_receipts == self.expected_receipts
            && self.live_receipts == 0
            && self.admitted_owner_rows == self.expected_owner_rows
            && self.live_owner_rows == 0
            && self.admitted_reservations == self.expected_filled_reservations
            && self.live_reservations == 0
            && self.released_unfilled_reservations == expected_unfilled
            && self.completed_owner_finalizations == self.expected_owner_rows
            && self.live_fee_finalizations == 0
            && self.admitted_dealer_children == self.expected_dealer_children
            && self.live_dealer_children == 0
            && self.admitted_merge_payments == self.expected_merge_payments
            && self.completed_merge_payments == self.expected_merge_payments
    }
}

/// Counted immutable selection plus mutable settlement dependency graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementRootV1AccountV1 {
    epoch: Id32,
    market: Id32,
    market_instance_v2_id: Id32,
    market_binding: Id32,
    window: Id32,
    source_admission_node: Id32,
    retained_feed: Id32,
    order_set: Id32,
    settlement_candidate_id: Id32,
    candidate_bundle_digest: Id32,
    settlement_witness_digest: Id32,
    owner_order_set_digest: Id32,
    cost_certificate_id: Id32,
    batch_policy_id: Id32,
    score_policy_id: Id32,
    fee_record: Id32,
    settlement_cash_pot: Id32,
    final_pot: Id32,
    solver_reward_destination: Id32,
    rank_key: [u8; SCORE_V2_Q_RANK_CAPACITY],
    root_rent: DeletableRentOwnerV1,
    cash_pot_rent: DeletableRentOwnerV1,
    final_pot_rent: OptionalSettlementRentV1,
    price_scale: u64,
    consideration_debit_atoms: u64,
    seller_credit_atoms: u64,
    selected_fee_atoms: u64,
    virtual_cash_atoms: u64,
    rounding_pot_price_units: u128,
    epoch_generation: u64,
    selected_ordinal: u64,
    selected_slot: u64,
    counts: SettlementRootChildCountsV1,
    outcome_count: u8,
    order_count: u8,
    virtual_cash_direction: VirtualCashDirectionV1,
    phase: SettlementRootPhaseV1,
    cash_pot_state: SettlementRootChildStateV1,
    final_pot_state: SettlementRootChildStateV1,
    retained_feed_state: SettlementRootChildStateV1,
    fee_record_state: SettlementRootChildStateV1,
    stored_bump: u8,
    cash_pot_bump: u8,
    final_pot_bump: u8,
    flags: u8,
}

impl SettlementRootV1AccountV1 {
    /// Parent counted Epoch PDA.
    pub const fn epoch(&self) -> Id32 {
        self.epoch
    }

    /// Actual General MarketRuntime PDA.
    pub const fn market(&self) -> Id32 {
        self.market
    }

    /// Exact immutable MarketBinding V2 account.
    pub const fn market_binding(&self) -> Id32 {
        self.market_binding
    }

    /// Exact finalized Window V5 account.
    pub const fn window(&self) -> Id32 {
        self.window
    }

    /// Historical winning AdmissionNode V4 identity.
    pub const fn source_admission_node(&self) -> Id32 {
        self.source_admission_node
    }

    /// Full Product occurrence MarketInstanceV2 identity.
    pub const fn market_instance_v2_id(&self) -> Id32 {
        self.market_instance_v2_id
    }

    /// Nonzero Epoch generation inherited by every child.
    pub const fn epoch_generation(&self) -> u64 {
        self.epoch_generation
    }

    /// Window-assigned one-based admission ordinal retained after Node close.
    pub const fn selected_ordinal(&self) -> u64 {
        self.selected_ordinal
    }

    /// Stable final SettlementCandidateId.
    pub const fn settlement_candidate_id(&self) -> Id32 {
        self.settlement_candidate_id
    }

    /// Retained sealed Feed identity.
    pub const fn retained_feed(&self) -> Id32 {
        self.retained_feed
    }

    /// Frozen order-set identity inherited from the selected Feed.
    pub const fn order_set(&self) -> Id32 {
        self.order_set
    }

    /// Exact selected candidate bundle digest.
    pub const fn candidate_bundle_digest(&self) -> Id32 {
        self.candidate_bundle_digest
    }

    /// Exact V4 receipt/row/reservation/fee/Dealer count owner.
    pub const fn counts(&self) -> SettlementRootChildCountsV1 {
        self.counts
    }

    /// Immutable complete owner/order-set identity.
    pub const fn owner_order_set_digest(&self) -> Id32 {
        self.owner_order_set_digest
    }

    /// Immutable cost certificate selected by the 96-byte rank.
    pub const fn cost_certificate_id(&self) -> Id32 {
        self.cost_certificate_id
    }

    /// Immutable batch policy used by the selected candidate certificate.
    pub const fn batch_policy_id(&self) -> Id32 {
        self.batch_policy_id
    }

    /// Immutable score policy used by the selected candidate certificate.
    pub const fn score_policy_id(&self) -> Id32 {
        self.score_policy_id
    }

    /// Canonical relation/settlement witness retained by FinalPot.
    pub const fn settlement_witness_digest(&self) -> Id32 {
        self.settlement_witness_digest
    }

    /// Exact selected cost-aware rank.
    pub const fn rank_key(&self) -> &[u8; SCORE_V2_Q_RANK_CAPACITY] {
        &self.rank_key
    }

    /// Active market outcome width.
    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }

    /// Every active frozen order counted by this settlement root.
    pub const fn order_count(&self) -> u8 {
        self.order_count
    }

    /// Immutable virtual-cash direction.
    pub const fn virtual_cash_direction(&self) -> VirtualCashDirectionV1 {
        self.virtual_cash_direction
    }

    /// Exact selected virtual complete-set quantity/cash principal.
    pub const fn virtual_cash_atoms(&self) -> u64 {
        self.virtual_cash_atoms
    }

    /// Current root lifecycle.
    pub const fn phase(&self) -> SettlementRootPhaseV1 {
        self.phase
    }

    /// Exact root rent/refund/donation owner.
    pub const fn root_rent(&self) -> DeletableRentOwnerV1 {
        self.root_rent
    }

    /// Replace only the root rent compartment for the reserved indexed-root
    /// in-place upgrade. The sibling module derives the exact principal top-up
    /// and observed donation; no external caller can mint this rewrite.
    pub(super) fn with_indexed_root_rent(
        &self,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        rent.validate()?;
        if self.phase != SettlementRootPhaseV1::Materializing
            || rent.payer != self.root_rent.payer
            || rent.refundable_principal < self.root_rent.refundable_principal
            || rent.donation_floor < self.root_rent.donation_floor
        {
            return Err(CodecError::InvalidState);
        }
        let mut value = *self;
        value.root_rent = rent;
        value.validate()?;
        Ok(value)
    }

    /// Stored root PDA bump.
    pub const fn stored_bump(&self) -> u8 {
        self.stored_bump
    }

    /// Expected buyer-first cash-pot PDA.
    pub const fn settlement_cash_pot(&self) -> Id32 {
        self.settlement_cash_pot
    }

    /// Cash-pot creation state, including merge's expected-uncreated state.
    pub const fn cash_pot_state(&self) -> SettlementRootChildStateV1 {
        self.cash_pot_state
    }

    /// Exact cash-pot rent/refund/donation owner.
    pub const fn cash_pot_rent(&self) -> DeletableRentOwnerV1 {
        self.cash_pot_rent
    }

    /// Canonical cash-pot PDA bump.
    pub const fn cash_pot_bump(&self) -> u8 {
        self.cash_pot_bump
    }

    /// Reconstruct the exact verifier-derived immutable cash expectation.
    pub fn cash_pot_expectation(&self) -> Result<SettlementCashPotExpectationV1, CodecError> {
        let value = SettlementCashPotExpectationV1 {
            market: self.market.bytes(),
            epoch: self.epoch.bytes(),
            candidate: self.settlement_candidate_id.bytes(),
            owner_order_set_digest: self.owner_order_set_digest.bytes(),
            fee_record: self.fee_record.bytes(),
            price_scale: self.price_scale,
            owner_count: self.counts.expected_owner_rows,
            consideration_debit_atoms: self.consideration_debit_atoms,
            seller_credit_atoms: self.seller_credit_atoms,
            selected_fee_atoms: self.selected_fee_atoms,
            rounding_pot_price_units: self.rounding_pot_price_units,
            virtual_cash_direction: self.virtual_cash_direction,
            virtual_cash_atoms: self.virtual_cash_atoms,
        };
        value.validate().map_err(|_| CodecError::InvalidState)?;
        Ok(value)
    }

    /// Expected FinalPot PDA, absent only for the nonvirtual direction.
    pub const fn final_pot(&self) -> Id32 {
        self.final_pot
    }

    /// FinalPot singleton lifecycle.
    pub const fn final_pot_state(&self) -> SettlementRootChildStateV1 {
        self.final_pot_state
    }

    /// Selected composite-fee record, zero only for the zero-fee route.
    pub const fn fee_record(&self) -> Id32 {
        self.fee_record
    }

    /// Selected fee-record lifecycle.
    pub const fn fee_record_state(&self) -> SettlementRootChildStateV1 {
        self.fee_record_state
    }

    /// Retained Feed lifecycle.
    pub const fn retained_feed_state(&self) -> SettlementRootChildStateV1 {
        self.retained_feed_state
    }

    /// Selection/finalization slot.
    pub const fn selected_slot(&self) -> u64 {
        self.selected_slot
    }

    /// Exact optional FinalPot rent owner.
    pub fn final_pot_rent(&self) -> Result<Option<DeletableRentOwnerV1>, CodecError> {
        self.final_pot_rent.get()
    }

    /// Validate immutable selection, directional singleton states, all counts,
    /// and the terminal partition.
    pub fn validate(&self) -> Result<(), CodecError> {
        let identities = self.identities();
        let mut index = 0usize;
        while index < identities.len() {
            let optional = index == 15 || index == 17;
            if identities[index].is_zero() && !optional {
                return Err(CodecError::ZeroIdentity);
            }
            index += 1;
        }
        // Only physical accounts are pairwise distinct. Direct selection
        // intentionally makes final candidate == base RelationV2 candidate,
        // and other independently checked content identities may also match.
        let physical = [
            self.epoch,
            self.market,
            self.market_binding,
            self.window,
            self.source_admission_node,
            self.retained_feed,
            self.settlement_cash_pot,
            self.final_pot,
        ];
        let mut left = 0usize;
        while left < physical.len() {
            let mut right = left + 1;
            while !physical[left].is_zero() && right < physical.len() {
                if physical[left] == physical[right] {
                    return Err(CodecError::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        self.root_rent.validate()?;
        self.cash_pot_rent.validate()?;
        self.counts.validate()?;
        self.cash_pot_expectation()?;
        if self.epoch_generation == 0
            || self.selected_slot == 0
            || self.selected_ordinal == 0
            || self.price_scale == 0
            || !(2..=16).contains(&self.outcome_count)
            || self.order_count == 0
            || self.counts.expected_receipts == 0
            || self.counts.expected_owner_rows == 0
            || self.counts.expected_reservations == 0
            || self.counts.expected_reservations != u16::from(self.order_count)
            || self.counts.expected_filled_reservations == 0
            || self.counts.expected_filled_reservations > self.counts.expected_reservations
            || self.counts.expected_owner_rows > self.counts.expected_filled_reservations
            || self.flags != 0
            || self.retained_feed_state == SettlementRootChildStateV1::Absent
            || self.retained_feed_state == SettlementRootChildStateV1::ExpectedUncreated
            || self.cash_pot_state == SettlementRootChildStateV1::Absent
        {
            return Err(CodecError::InvalidState);
        }
        validate_selected_rank(
            &self.rank_key,
            self.settlement_candidate_id,
            self.selected_ordinal,
        )?;
        let fee_present = !self.fee_record.is_zero();
        if fee_present != (self.selected_fee_atoms != 0)
            || fee_present != (self.fee_record_state != SettlementRootChildStateV1::Absent)
            || (!fee_present && self.fee_record_state != SettlementRootChildStateV1::Absent)
            || (!fee_present && self.counts.live_fee_finalizations != 0)
        {
            return Err(CodecError::InvalidState);
        }
        match self.virtual_cash_direction {
            VirtualCashDirectionV1::None => {
                if self.virtual_cash_atoms != 0
                    || !self.final_pot.is_zero()
                    || self.final_pot_state != SettlementRootChildStateV1::Absent
                    || self.final_pot_rent.get()?.is_some()
                    || self.cash_pot_state == SettlementRootChildStateV1::ExpectedUncreated
                    || self.counts.expected_merge_payments != 0
                {
                    return Err(CodecError::InvalidState);
                }
            }
            VirtualCashDirectionV1::Split => {
                if self.virtual_cash_atoms == 0
                    || self.final_pot.is_zero()
                    || self.final_pot_state == SettlementRootChildStateV1::Absent
                    || self.final_pot_state == SettlementRootChildStateV1::ExpectedUncreated
                    || self.final_pot_rent.get()?.is_none()
                    || self.cash_pot_state == SettlementRootChildStateV1::ExpectedUncreated
                    || self.counts.expected_merge_payments != 0
                {
                    return Err(CodecError::InvalidState);
                }
            }
            VirtualCashDirectionV1::Merge => {
                if self.virtual_cash_atoms == 0
                    || self.final_pot.is_zero()
                    || self.final_pot_state == SettlementRootChildStateV1::Absent
                    || self.final_pot_state == SettlementRootChildStateV1::ExpectedUncreated
                    || self.final_pot_rent.get()?.is_none()
                    || self.counts.expected_merge_payments == 0
                    || self.cash_pot_state == SettlementRootChildStateV1::Absent
                {
                    return Err(CodecError::InvalidState);
                }
            }
        }
        let materialization_complete = self.materialization_complete();
        match self.phase {
            SettlementRootPhaseV1::Materializing => {
                if materialization_complete
                    || self.counts.live_receipts != self.counts.admitted_receipts
                    || self.counts.completed_owner_finalizations != 0
                    || self.counts.live_fee_finalizations != 0
                {
                    return Err(CodecError::InvalidState);
                }
            }
            SettlementRootPhaseV1::Settling => {
                if !materialization_complete {
                    return Err(CodecError::InvalidState);
                }
            }
            SettlementRootPhaseV1::Retiring => {
                if !self.settlement_children_fully_accounted()
                    || self.retained_feed_state != SettlementRootChildStateV1::Live
                    || !matches!(
                        self.cash_pot_state,
                        SettlementRootChildStateV1::Live | SettlementRootChildStateV1::Retired
                    )
                    || !matches!(
                        self.fee_record_state,
                        SettlementRootChildStateV1::Absent
                            | SettlementRootChildStateV1::Live
                            | SettlementRootChildStateV1::Retired
                    )
                {
                    return Err(CodecError::InvalidState);
                }
            }
            SettlementRootPhaseV1::Terminal => {
                if !self.counts.terminal()
                    || self.cash_pot_state != SettlementRootChildStateV1::Retired
                    || !matches!(
                        self.final_pot_state,
                        SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Retired
                    )
                    || self.retained_feed_state != SettlementRootChildStateV1::Retired
                    || !matches!(
                        self.fee_record_state,
                        SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Retired
                    )
                {
                    return Err(CodecError::InvalidState);
                }
            }
        }
        Ok(())
    }

    fn materialization_complete(&self) -> bool {
        let Some(expected_unfilled) = self
            .counts
            .expected_reservations
            .checked_sub(self.counts.expected_filled_reservations)
        else {
            return false;
        };
        self.counts.admitted_receipts == self.counts.expected_receipts
            && self.counts.admitted_owner_rows == self.counts.expected_owner_rows
            && self.counts.admitted_reservations == self.counts.expected_filled_reservations
            && self.counts.released_unfilled_reservations == expected_unfilled
            && self.counts.admitted_dealer_children == self.counts.expected_dealer_children
            && self.counts.admitted_merge_payments == self.counts.expected_merge_payments
    }

    /// Stable aggregate facts established before any singleton retirement.
    ///
    /// The live adapter remains responsible for authenticating the terminal
    /// semantic state of each exact Receipt, owner row, Reservation, and fee
    /// finalization child before invoking its narrow close transition. Once
    /// every such close has been counted, these facts remain invariant for the
    /// whole `Retiring` phase.
    fn settlement_children_fully_accounted(&self) -> bool {
        self.materialization_complete()
            && self.counts.live_receipts == 0
            && self.counts.live_owner_rows == 0
            && self.counts.live_reservations == 0
            && self.counts.completed_owner_finalizations == self.counts.expected_owner_rows
            && self.counts.live_fee_finalizations == 0
            && self.counts.completed_merge_payments == self.counts.expected_merge_payments
    }

    /// Admit the unique opaque Dealer attachment selected by a CoveredDealer
    /// candidate.
    ///
    /// The root owns only the exhaustive child count. The adapter must create
    /// and authenticate the Dealer-owned child account in the same rollback
    /// domain as this successor write; no caller-provided child DTO enters the
    /// General account body.
    pub fn admit_dealer_child(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Materializing
            || self.counts.expected_dealer_children != 1
            || self.counts.admitted_dealer_children != 0
            || self.counts.live_dealer_children != 0
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.admitted_dealer_children = 1;
        next.counts.live_dealer_children = 1;
        if next.materialization_complete() {
            next.phase = SettlementRootPhaseV1::Settling;
        }
        next.validate()?;
        Ok(next)
    }

    /// Retire the unique authenticated Dealer attachment after its Dealer
    /// Lease lifecycle is terminal.
    ///
    /// The adapter must authenticate the exact child PDA, immutable candidate
    /// binding, rent close, and Dealer terminal capability before applying
    /// this aggregate count transition.
    pub fn retire_dealer_child(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Retiring
            || self.counts.expected_dealer_children != 1
            || self.counts.admitted_dealer_children != 1
            || self.counts.live_dealer_children != 1
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.live_dealer_children = 0;
        next.validate()?;
        Ok(next)
    }

    /// Encode the exact 980-byte root.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let mut writer = Writer::exact(output, SETTLEMENT_ROOT_ACCOUNT_BYTES)?;
        writer.u8(SETTLEMENT_ROOT_ACCOUNT_TAG)?;
        writer.u8(SETTLEMENT_ROOT_ACCOUNT_VERSION)?;
        for identity in self.identities() {
            writer.bytes(&identity.bytes())?;
        }
        writer.bytes(&self.rank_key)?;
        write_rent(&mut writer, self.root_rent)?;
        write_rent(&mut writer, self.cash_pot_rent)?;
        write_optional_rent(&mut writer, self.final_pot_rent)?;
        for value in [
            self.price_scale,
            self.consideration_debit_atoms,
            self.seller_credit_atoms,
            self.selected_fee_atoms,
            self.virtual_cash_atoms,
        ] {
            writer.u64(value)?;
        }
        writer.u128(self.rounding_pot_price_units)?;
        writer.u64(self.epoch_generation)?;
        writer.u64(self.selected_ordinal)?;
        writer.u64(self.selected_slot)?;
        write_counts(&mut writer, self.counts)?;
        for value in [
            self.outcome_count,
            self.order_count,
            direction_byte(self.virtual_cash_direction),
            self.phase as u8,
            self.cash_pot_state as u8,
            self.final_pot_state as u8,
            self.retained_feed_state as u8,
            self.fee_record_state as u8,
            self.stored_bump,
            self.cash_pot_bump,
            self.final_pot_bump,
            self.flags,
        ] {
            writer.u8(value)?;
        }
        writer.finish()
    }

    /// Content identity of the exact authenticated root prestate.
    ///
    /// This is the same account-key-bound transcript used by terminal
    /// projection, but it makes no terminality claim. Read-only adapters use
    /// it to bind a private page-set capability to the precise root bytes they
    /// authenticated before an atomic successor write.
    pub fn data_id<B: Sha256BackendV1>(
        &self,
        backend: &B,
        root_account: Id32,
    ) -> Result<Id32, CodecError> {
        self.validate()?;
        require_live(root_account)?;
        let mut bytes = [0u8; SETTLEMENT_ROOT_ACCOUNT_BYTES];
        self.encode(&mut bytes)?;
        Id32::new(backend.sha256(&[
            SETTLEMENT_ROOT_DATA_ID_DOMAIN_V1,
            &root_account.bytes(),
            &bytes,
        ]))
    }

    /// Decode one hostile root account and rerun every invariant.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, SETTLEMENT_ROOT_ACCOUNT_BYTES)?;
        if reader.u8()? != SETTLEMENT_ROOT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != SETTLEMENT_ROOT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let mut identities = [Id32::ZERO; IDENTITY_COUNT];
        let mut at = 0usize;
        while at < identities.len() {
            identities[at] = Id32::from_bytes(reader.array()?);
            at += 1;
        }
        let value = Self {
            epoch: identities[0],
            market: identities[1],
            market_instance_v2_id: identities[2],
            market_binding: identities[3],
            window: identities[4],
            source_admission_node: identities[5],
            retained_feed: identities[6],
            order_set: identities[7],
            settlement_candidate_id: identities[8],
            candidate_bundle_digest: identities[9],
            settlement_witness_digest: identities[10],
            owner_order_set_digest: identities[11],
            cost_certificate_id: identities[12],
            batch_policy_id: identities[13],
            score_policy_id: identities[14],
            fee_record: identities[15],
            settlement_cash_pot: identities[16],
            final_pot: identities[17],
            solver_reward_destination: identities[18],
            rank_key: reader.array()?,
            root_rent: read_rent(&mut reader)?,
            cash_pot_rent: read_rent(&mut reader)?,
            final_pot_rent: read_optional_rent(&mut reader)?,
            price_scale: reader.u64()?,
            consideration_debit_atoms: reader.u64()?,
            seller_credit_atoms: reader.u64()?,
            selected_fee_atoms: reader.u64()?,
            virtual_cash_atoms: reader.u64()?,
            rounding_pot_price_units: reader.u128()?,
            epoch_generation: reader.u64()?,
            selected_ordinal: reader.u64()?,
            selected_slot: reader.u64()?,
            counts: read_counts(&mut reader)?,
            outcome_count: reader.u8()?,
            order_count: reader.u8()?,
            virtual_cash_direction: decode_direction(reader.u8()?)?,
            phase: SettlementRootPhaseV1::decode(reader.u8()?)?,
            cash_pot_state: SettlementRootChildStateV1::decode(reader.u8()?)?,
            final_pot_state: SettlementRootChildStateV1::decode(reader.u8()?)?,
            retained_feed_state: SettlementRootChildStateV1::decode(reader.u8()?)?,
            fee_record_state: SettlementRootChildStateV1::decode(reader.u8()?)?,
            stored_bump: reader.u8()?,
            cash_pot_bump: reader.u8()?,
            final_pot_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }

    /// Apply one exact action-24 child-creation delta.
    /// Structural action-24 successor. The live adapter must reauthenticate
    /// the exact root account/PDA/owner and compose every child write.
    pub fn admit_materialization_delta(
        &self,
        owner_rows_created: u8,
        filled_reservations_admitted: u8,
        merge_receipt: bool,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Materializing
            || owner_rows_created > 2
            || filled_reservations_admitted > 2
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.admitted_receipts = checked_add(next.counts.admitted_receipts, 1)?;
        next.counts.live_receipts = checked_add(next.counts.live_receipts, 1)?;
        next.counts.admitted_owner_rows = checked_add(
            next.counts.admitted_owner_rows,
            u16::from(owner_rows_created),
        )?;
        next.counts.live_owner_rows =
            checked_add(next.counts.live_owner_rows, u16::from(owner_rows_created))?;
        next.counts.admitted_reservations = checked_add(
            next.counts.admitted_reservations,
            u16::from(filled_reservations_admitted),
        )?;
        next.counts.live_reservations = checked_add(
            next.counts.live_reservations,
            u16::from(filled_reservations_admitted),
        )?;
        if merge_receipt {
            next.counts.admitted_merge_payments =
                checked_add(next.counts.admitted_merge_payments, 1)?;
        }
        if next.materialization_complete() {
            next.phase = SettlementRootPhaseV1::Settling;
        }
        next.validate()?;
        Ok(next)
    }

    /// Record one action-41 zero-fill Reservation release.
    ///
    /// This structural successor does not authenticate the frozen page/order,
    /// sealed Feed zero fill, Reservation, PositionV3, GEN1 Replay, rent owner,
    /// value return, or account close. The live adapter must compose all of
    /// those exact facts atomically before writing the returned root bytes.
    pub fn release_unfilled_reservation(&self) -> Result<Self, CodecError> {
        self.validate()?;
        let expected_unfilled = self
            .counts
            .expected_reservations
            .checked_sub(self.counts.expected_filled_reservations)
            .ok_or(CodecError::InvalidCount)?;
        if self.phase != SettlementRootPhaseV1::Materializing
            || self.counts.released_unfilled_reservations >= expected_unfilled
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.released_unfilled_reservations =
            checked_add(next.counts.released_unfilled_reservations, 1)?;
        if next.materialization_complete() {
            next.phase = SettlementRootPhaseV1::Settling;
        }
        next.validate()?;
        Ok(next)
    }

    fn activate_merge_cash_pot(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.virtual_cash_direction != VirtualCashDirectionV1::Merge
            || self.cash_pot_state != SettlementRootChildStateV1::ExpectedUncreated
            || self.phase != SettlementRootPhaseV1::Settling
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.cash_pot_state = SettlementRootChildStateV1::Live;
        next.validate()?;
        Ok(next)
    }

    /// Record one complete action-38 owner transition. `fee_receipt_created`
    /// must be true exactly for fee-bearing roots; zero-fee roots rely on the
    /// action-38 row/pot GEN1 Replay evidence and create no phantom fee PDA.
    /// This structural successor authenticates no outer account.
    pub fn complete_owner_finalization(
        &self,
        fee_receipt_created: bool,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Settling
            || fee_receipt_created != !self.fee_record.is_zero()
            || self.counts.completed_owner_finalizations >= self.counts.expected_owner_rows
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.completed_owner_finalizations =
            checked_add(next.counts.completed_owner_finalizations, 1)?;
        if fee_receipt_created {
            next.counts.live_fee_finalizations =
                checked_add(next.counts.live_fee_finalizations, 1)?;
        }
        next.validate()?;
        Ok(next)
    }

    /// Action 40 completes one distinct merge paid latch.
    /// Structural action-40 count successor. This does not authenticate the
    /// Receipt/Reservation/fee/Replay write set. Fee-bearing action 40 must
    /// consume exact rent-owned `0x83/4`; zero-fee action 40 must authenticate current
    /// action-38 Replay evidence committing the finalized row and exact
    /// cash-pot poststate after action 38 authenticated action 37.
    pub fn complete_merge_payment(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.virtual_cash_direction != VirtualCashDirectionV1::Merge
            || self.phase != SettlementRootPhaseV1::Settling
            || self.cash_pot_state != SettlementRootChildStateV1::Live
            || self.counts.completed_merge_payments >= self.counts.admitted_merge_payments
            || self.counts.completed_merge_payments >= self.counts.expected_merge_payments
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.completed_merge_payments =
            checked_add(next.counts.completed_merge_payments, 1)?;
        next.validate()?;
        Ok(next)
    }

    /// Count exactly one terminal Receipt account closed by its authenticated
    /// settlement route.
    ///
    /// This transition proves no Receipt semantics or rent movement by itself.
    /// The adapter must authenticate one exact fully accounted and delivered
    /// Receipt and compose its close atomically with this successor write.
    pub fn retire_one_receipt(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Settling || self.counts.live_receipts == 0 {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.live_receipts = next
            .counts
            .live_receipts
            .checked_sub(1)
            .ok_or(CodecError::InvalidCount)?;
        next.validate()?;
        Ok(next)
    }

    /// Count exactly one finalized owner-row account closed by its
    /// authenticated owner-finalization route.
    ///
    /// The number of retired rows can never overtake the number of completed
    /// owner finalizations, even if an adapter presents closures out of order.
    pub fn retire_one_owner_row(&self) -> Result<Self, CodecError> {
        self.validate()?;
        let retired = self
            .counts
            .admitted_owner_rows
            .checked_sub(self.counts.live_owner_rows)
            .ok_or(CodecError::InvalidCount)?;
        if self.phase != SettlementRootPhaseV1::Settling
            || self.counts.live_owner_rows == 0
            || retired >= self.counts.completed_owner_finalizations
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.live_owner_rows = next
            .counts
            .live_owner_rows
            .checked_sub(1)
            .ok_or(CodecError::InvalidCount)?;
        next.validate()?;
        Ok(next)
    }

    /// Count exactly one filled Reservation closed after its authenticated
    /// route proves the Reservation terminal.
    pub fn retire_one_reservation(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Settling || self.counts.live_reservations == 0 {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.live_reservations = next
            .counts
            .live_reservations
            .checked_sub(1)
            .ok_or(CodecError::InvalidCount)?;
        next.validate()?;
        Ok(next)
    }

    /// Count exactly one rent-owned fee-finalization child consumed by its
    /// authenticated fee route.
    ///
    /// Zero-fee roots never acquire these children and therefore cannot invoke
    /// this transition.
    pub fn retire_one_fee_finalization(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Settling
            || self.fee_record_state != SettlementRootChildStateV1::Live
            || self.counts.live_fee_finalizations == 0
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.live_fee_finalizations = next
            .counts
            .live_fee_finalizations
            .checked_sub(1)
            .ok_or(CodecError::InvalidCount)?;
        next.validate()?;
        Ok(next)
    }

    /// Begin dependency-ordered singleton retirement after every settlement
    /// child has been fully delivered, accounted, finalized, and closed.
    ///
    /// No caller-selected count delta enters this transition. Dealer children
    /// and the singleton cash/final/fee records remain live so their exact
    /// terminal accounts can be authenticated and retired afterward.
    pub fn begin_retiring(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Settling
            || !self.settlement_children_fully_accounted()
            || self.counts.live_dealer_children != self.counts.admitted_dealer_children
            || self.cash_pot_state != SettlementRootChildStateV1::Live
            || self.retained_feed_state != SettlementRootChildStateV1::Live
            || !matches!(
                self.final_pot_state,
                SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Live
            )
            || !matches!(
                self.fee_record_state,
                SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Live
            )
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.phase = SettlementRootPhaseV1::Retiring;
        next.validate()?;
        Ok(next)
    }

    /// Retire the authenticated settlement cash pot after settlement enters
    /// its dependency-ordered retirement phase.
    pub fn retire_cash_pot(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Retiring
            || self.cash_pot_state != SettlementRootChildStateV1::Live
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.cash_pot_state = SettlementRootChildStateV1::Retired;
        next.validate()?;
        Ok(next)
    }

    /// Retire the authenticated FinalPot for exactly one Split or Merge root.
    /// Nonvirtual roots have no FinalPot and cannot invoke this transition.
    pub fn retire_final_pot(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Retiring
            || self.virtual_cash_direction == VirtualCashDirectionV1::None
            || self.final_pot_state != SettlementRootChildStateV1::Live
            || self.final_pot_rent.get()?.is_none()
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.final_pot_state = SettlementRootChildStateV1::Retired;
        next.validate()?;
        Ok(next)
    }

    /// Retire the authenticated selected fee record after every per-owner fee
    /// finalization child has already been consumed.
    pub fn retire_fee_record(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if self.phase != SettlementRootPhaseV1::Retiring
            || self.fee_record_state != SettlementRootChildStateV1::Live
            || self.counts.live_fee_finalizations != 0
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.fee_record_state = SettlementRootChildStateV1::Retired;
        next.validate()?;
        Ok(next)
    }

    /// Retire one exact coefficient-portfolio pair's complete archive set.
    ///
    /// This is the sole aggregate count transition for action 44. The caller
    /// cannot choose a subset: `receipt_count` must equal the root's entire
    /// admitted/live Receipt set, and the root must own exactly two filled,
    /// live Reservations. The live composer must additionally authenticate
    /// every committed Receipt V5 sibling, both consumed Reservation V9
    /// accounts, both Position/GEN1 child decrements, and every exact rent
    /// transfer before writing this successor.
    ///
    /// This transition deliberately remains `Settling` and leaves owner rows,
    /// the cash pot, and the retained Feed live. Exact owner-row closes must
    /// finish before [`Self::begin_retiring`]; the private portfolio terminal
    /// receipt is never authority to skip that dependency order or promote
    /// this successor directly to `Retiring` or `Terminal`.
    pub fn retire_portfolio_pair_archives(
        &self,
        receipt_count: u8,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        let receipts = u16::from(receipt_count);
        if self.phase != SettlementRootPhaseV1::Settling
            || receipt_count == 0
            || self.virtual_cash_direction != VirtualCashDirectionV1::None
            || self.counts.expected_receipts != receipts
            || self.counts.admitted_receipts != receipts
            || self.counts.live_receipts != receipts
            || self.counts.expected_filled_reservations != 2
            || self.counts.admitted_reservations != 2
            || self.counts.live_reservations != 2
            || self.counts.completed_owner_finalizations != self.counts.expected_owner_rows
            || self.counts.live_fee_finalizations != 0
            || self.counts.expected_dealer_children != 0
            || self.counts.admitted_dealer_children != 0
            || self.counts.live_dealer_children != 0
            || self.counts.expected_merge_payments != 0
            || self.counts.admitted_merge_payments != 0
            || self.counts.completed_merge_payments != 0
        {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.counts.live_receipts = next
            .counts
            .live_receipts
            .checked_sub(receipts)
            .ok_or(CodecError::InvalidCount)?;
        next.counts.live_reservations = next
            .counts
            .live_reservations
            .checked_sub(2)
            .ok_or(CodecError::InvalidCount)?;
        next.validate()?;
        Ok(next)
    }

    /// Whether every non-Feed liability is discharged while the exact
    /// retained Feed remains available for its final authenticated readers.
    pub(super) fn at_retained_feed_retirement_frontier(&self) -> bool {
        let Some(expected_unfilled) = self
            .counts
            .expected_reservations
            .checked_sub(self.counts.expected_filled_reservations)
        else {
            return false;
        };
        self.phase == SettlementRootPhaseV1::Retiring
            && self.counts.admitted_receipts == self.counts.expected_receipts
            && self.counts.live_receipts == 0
            && self.counts.admitted_owner_rows == self.counts.expected_owner_rows
            && self.counts.live_owner_rows == 0
            && self.counts.admitted_reservations == self.counts.expected_filled_reservations
            && self.counts.live_reservations == 0
            && self.counts.released_unfilled_reservations == expected_unfilled
            && self.counts.completed_owner_finalizations == self.counts.expected_owner_rows
            && self.counts.live_fee_finalizations == 0
            && self.counts.admitted_dealer_children == self.counts.expected_dealer_children
            && self.counts.live_dealer_children == 0
            && self.counts.admitted_merge_payments == self.counts.expected_merge_payments
            && self.counts.completed_merge_payments == self.counts.expected_merge_payments
            && self.cash_pot_state == SettlementRootChildStateV1::Retired
            && matches!(
                self.final_pot_state,
                SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Retired
            )
            && self.retained_feed_state == SettlementRootChildStateV1::Live
            && matches!(
                self.fee_record_state,
                SettlementRootChildStateV1::Absent | SettlementRootChildStateV1::Retired
            )
    }

    /// Structural Feed-retirement successor used only after the adapter closes
    /// the exact retained Feed in the same rollback domain.
    pub(super) fn retire_retained_feed_and_finish(&self) -> Result<Self, CodecError> {
        self.validate()?;
        if !self.at_retained_feed_retirement_frontier() {
            return Err(CodecError::InvalidState);
        }
        let mut next = *self;
        next.retained_feed_state = SettlementRootChildStateV1::Retired;
        next.phase = SettlementRootPhaseV1::Terminal;
        next.validate()?;
        Ok(next)
    }

    /// Derive a structural full-account terminal receipt only after every
    /// expected child is observed and retired. This is not a capability: the
    /// retirement adapter must reauthenticate the exact root PDA, program
    /// owner, body bytes, generation, and occurrence join before promotion.
    pub fn terminal_projection<B: Sha256BackendV1>(
        &self,
        backend: &B,
        root_account: Id32,
    ) -> Result<SettlementRootTerminalProjectionV1, CodecError> {
        self.validate()?;
        require_live(root_account)?;
        if self.phase != SettlementRootPhaseV1::Terminal
            || self
                .identities()
                .iter()
                .any(|identity| *identity == root_account)
        {
            return Err(CodecError::InvalidState);
        }
        let receipt = self.data_id(backend, root_account)?;
        Ok(SettlementRootTerminalProjectionV1 {
            root_account,
            market_instance_v2_id: self.market_instance_v2_id,
            market: self.market,
            epoch: self.epoch,
            epoch_generation: self.epoch_generation,
            settlement_candidate_id: self.settlement_candidate_id,
            terminal_receipt_id: receipt,
        })
    }

    fn identities(&self) -> [Id32; IDENTITY_COUNT] {
        [
            self.epoch,
            self.market,
            self.market_instance_v2_id,
            self.market_binding,
            self.window,
            self.source_admission_node,
            self.retained_feed,
            self.order_set,
            self.settlement_candidate_id,
            self.candidate_bundle_digest,
            self.settlement_witness_digest,
            self.owner_order_set_digest,
            self.cost_certificate_id,
            self.batch_policy_id,
            self.score_policy_id,
            self.fee_record,
            self.settlement_cash_pot,
            self.final_pot,
            self.solver_reward_destination,
        ]
    }
}

/// Exact action-39 pure initialization inputs.
#[derive(Clone, Copy, Debug)]
pub struct InitializeSettlementRootV1<'a> {
    /// New counted root PDA.
    pub root_account: Id32,
    /// Stored root PDA bump reproduced by the adapter.
    pub root_bump: u8,
    /// Actual immutable Epoch PDA.
    pub epoch_account: Id32,
    /// Exact MarketBinding V2 account.
    pub market_binding_account: Id32,
    /// Exact Window V5 account.
    pub window_account: Id32,
    /// Exact retained sealed CandidateFeed V2 account.
    pub retained_feed_account: Id32,
    /// Parent Epoch generation.
    pub epoch_generation: u64,
    /// Full Product MarketInstance V2 identity.
    pub market_instance_v2_id: Id32,
    /// Exact frozen Epoch prestate whose unique selected child becomes this root.
    pub epoch: &'a GeneralEpochV6AccountV1,
    /// Exact immutable MarketBinding V2 body.
    pub market: &'a MarketBindingV2,
    /// Exact pre-finalization Window V5 body.
    pub window: &'a CandidateWindowV5AccountV1,
    /// Exact winning AdmissionNode V4 body.
    pub node: &'a AdmissionNodeV4AccountV1,
    /// Exact retained sealed CandidateFeed V2 header.
    pub feed: &'a CandidateFeedHeaderV2,
    /// Current finalization slot.
    pub current_slot: u64,
    /// Complete verifier-derived owner/order and fee expectation.
    pub owner_order_set_digest: Id32,
    /// Exact candidate-wide owner cash expectation.
    pub cash_expectation: SettlementCashPotExpectationV1,
    /// Distinct selected Reservations derived from the complete frozen book.
    pub expected_reservations: u16,
    /// Distinct filled Reservations adopted by action 24; the complement is
    /// released and co-closed one at a time by action 41.
    pub expected_filled_reservations: u16,
    /// Number of merge receipts requiring action-40 payment latches.
    pub expected_merge_payments: u16,
    /// Canonical cash-pot PDA, including merge's not-yet-created identity.
    pub settlement_cash_pot: Id32,
    /// Stored cash-pot PDA bump.
    pub cash_pot_bump: u8,
    /// Canonical FinalPot PDA, zero only for the nonvirtual direction.
    pub final_pot: Id32,
    /// Stored FinalPot PDA bump, zero only when FinalPot is absent.
    pub final_pot_bump: u8,
    /// Root account's exact rent/refund/donation owner.
    pub root_rent: DeletableRentOwnerV1,
    /// Cash-pot account's exact rent/refund/donation owner.
    pub cash_pot_rent: DeletableRentOwnerV1,
    /// Optional FinalPot rent/refund/donation owner.
    pub final_pot_rent: OptionalSettlementRentV1,
}

/// Exact action-39 poststate and singleton creation projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializeSettlementRootPlanV1 {
    epoch: GeneralEpochV6AccountV1,
    root: SettlementRootV1AccountV1,
    window: CandidateWindowV5AccountV1,
    cash_pot: Option<SettlementCashPotV1>,
    final_pot: Option<SettlementFinalPotInitializationV1>,
}

impl InitializeSettlementRootPlanV1 {
    /// Finalized Epoch successor counting this unique SettlementRoot.
    pub const fn epoch(&self) -> &GeneralEpochV6AccountV1 {
        &self.epoch
    }

    /// Newly counted SettlementRoot.
    pub const fn root(&self) -> &SettlementRootV1AccountV1 {
        &self.root
    }

    /// Finalized Window successor pointing only at the counted root.
    pub const fn window(&self) -> &CandidateWindowV5AccountV1 {
        &self.window
    }

    /// Exact cash-pot body created now for None/Split; Merge stays absent.
    pub const fn cash_pot(&self) -> Option<SettlementCashPotV1> {
        self.cash_pot
    }

    /// Exact FinalPot creation facts for Split/Merge; absent for None.
    pub const fn final_pot(&self) -> Option<SettlementFinalPotInitializationV1> {
        self.final_pot
    }
}

/// Exact action-39 FinalPot initialization. This is a creation poststate, not
/// a caller-authenticated existing-account projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementFinalPotInitializationV1 {
    account: Id32,
    market: Id32,
    epoch: Id32,
    candidate: Id32,
    owner_order_set_digest: Id32,
    settlement_witness_digest: Id32,
    kind: VirtualReceiptKindV1,
    authorized_complete_set_atoms: u64,
    outcome_count: u8,
    rent: DeletableRentOwnerV1,
    stored_bump: u8,
}

impl SettlementFinalPotInitializationV1 {
    /// Exact FinalPot account to create.
    pub const fn account(&self) -> Id32 {
        self.account
    }
    /// General MarketRuntime identity.
    pub const fn market(&self) -> Id32 {
        self.market
    }
    /// Counted Epoch identity.
    pub const fn epoch(&self) -> Id32 {
        self.epoch
    }
    /// Final candidate identity.
    pub const fn candidate(&self) -> Id32 {
        self.candidate
    }
    /// Immutable owner/order-set digest.
    pub const fn owner_order_set_digest(&self) -> Id32 {
        self.owner_order_set_digest
    }
    /// Checked relation/settlement witness.
    pub const fn settlement_witness_digest(&self) -> Id32 {
        self.settlement_witness_digest
    }
    /// Split or Merge; None has no initialization.
    pub const fn kind(&self) -> VirtualReceiptKindV1 {
        self.kind
    }
    /// Exact selected complete-set quantity.
    pub const fn authorized_complete_set_atoms(&self) -> u64 {
        self.authorized_complete_set_atoms
    }
    /// Active outcome count.
    pub const fn outcome_count(&self) -> u8 {
        self.outcome_count
    }
    /// Exact FinalPot rent owner.
    pub const fn rent(&self) -> DeletableRentOwnerV1 {
        self.rent
    }
    /// Stored FinalPot PDA bump.
    pub const fn stored_bump(&self) -> u8 {
        self.stored_bump
    }

    /// Encode the exact fresh FinalPot outer without consulting a historical
    /// SelectedCandidate account. The action-39 root plan is the sole budget
    /// authority and fixes every semantic field in this postimage.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        let semantic = AuthenticatedFinalPotV1 {
            account: self.account.bytes(),
            market: self.market.bytes(),
            epoch: self.epoch.bytes(),
            candidate: self.candidate.bytes(),
            owner_order_set_digest: self.owner_order_set_digest.bytes(),
            relation_witness_digest: self.settlement_witness_digest.bytes(),
            cash_principal_atoms: 0,
            internal_claims: [0; MAX_OUTCOMES],
            inventory_kind: self.kind,
            authorized_complete_set_atoms: self.authorized_complete_set_atoms,
            processed_complete_set_atoms: 0,
            inventory_transition_sequence: 0,
            inventory_state: VirtualInventoryStateV1::Open,
            outcome_count: self.outcome_count,
            phase: 0,
            writable: true,
            selected_budget_authenticated: true,
        };
        let body = semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, FINAL_POT_ACCOUNT_BYTES)?;
        writer.u8(FINAL_POT_ACCOUNT_TAG)?;
        writer.u8(FINAL_POT_ACCOUNT_VERSION)?;
        writer.bytes(&body)?;
        writer.u8(self.stored_bump)?;
        writer.u8(0)?;
        writer.finish()
    }
}

/// Exact action-37 sole merge cash-pot creation poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivateMergeCashPotPlanV1 {
    root: SettlementRootV1AccountV1,
    cash_pot_account: Id32,
    cash_pot: SettlementCashPotV1,
    rent: DeletableRentOwnerV1,
    stored_bump: u8,
}

impl ActivateMergeCashPotPlanV1 {
    /// Root successor latching the singleton cash pot live.
    pub const fn root(&self) -> &SettlementRootV1AccountV1 {
        &self.root
    }
    /// Exact sole cash-pot PDA.
    pub const fn cash_pot_account(&self) -> Id32 {
        self.cash_pot_account
    }
    /// Canonical opening-merge cash-pot body.
    pub const fn cash_pot(&self) -> SettlementCashPotV1 {
        self.cash_pot
    }
    /// Exact rent/refund/donation owner.
    pub const fn rent(&self) -> DeletableRentOwnerV1 {
        self.rent
    }
    /// Stored cash-pot PDA bump.
    pub const fn stored_bump(&self) -> u8 {
        self.stored_bump
    }
}

/// Atomically derive the only permitted merge cash-pot creation. The adapter
/// must compose this root write, account creation, exact rent funding, and the
/// terminal inventory transition in one instruction.
pub fn prepare_activate_merge_cash_pot_v1(
    root: &SettlementRootV1AccountV1,
) -> Result<ActivateMergeCashPotPlanV1, CodecError> {
    let successor = root.activate_merge_cash_pot()?;
    let expectation = root.cash_pot_expectation()?;
    let cash_pot = SettlementCashPotV1::new(expectation).map_err(|_| CodecError::InvalidState)?;
    Ok(ActivateMergeCashPotPlanV1 {
        root: successor,
        cash_pot_account: root.settlement_cash_pot,
        cash_pot,
        rent: root.cash_pot_rent,
        stored_bump: root.cash_pot_bump,
    })
}

/// Create the counted root and derive singleton states without accepting
/// caller expectation bytes.
pub fn initialize_settlement_root_v1(
    request: InitializeSettlementRootV1<'_>,
) -> Result<InitializeSettlementRootPlanV1, CodecError> {
    request.epoch.validate()?;
    request.market.validate()?;
    request.window.validate()?;
    request.node.validate()?;
    request.feed.validate(true)?;
    request.root_rent.validate()?;
    request.cash_pot_rent.validate()?;
    request
        .cash_expectation
        .validate()
        .map_err(|_| CodecError::InvalidState)?;
    for identity in [
        request.root_account,
        request.epoch_account,
        request.market_binding_account,
        request.window_account,
        request.retained_feed_account,
        request.market_instance_v2_id,
        request.owner_order_set_digest,
        request.settlement_cash_pot,
    ] {
        require_live(identity)?;
    }
    let window = request.window.base();
    let node = request.node.base();
    let market = request.market.base();
    let direction = request.cash_expectation.virtual_cash_direction;
    if request.epoch_generation == 0
        || request.epoch.phase != GeneralEpochPhaseV1::Frozen
        || request.epoch.selected_candidate_count != 0
        || request.epoch.generation != request.epoch_generation
        || request.epoch.market_binding != request.market_binding_account
        || request.epoch.market_runtime != market.market
        || request.epoch.market_instance_v2_id != request.market_instance_v2_id
        || request.epoch.window != request.window_account
        || request.epoch.order_set != request.feed.order_set
        || request.current_slot < window.verification_closes_slot
        || request.current_slot == 0
        || request.feed.order_count == 0
        || request.feed.slice_count == 0
        || !matches!(
            request.feed.candidate_kind,
            SettlementCandidateKindV1::Direct | SettlementCandidateKindV1::CoveredDealer
        )
        || window.finalized_slot != 0
        || window.valid_verdict_count == 0
        || window
            .revealed_count
            .checked_add(window.expired_commitment_count)
            .ok_or(CodecError::ArithmeticOverflow)?
            != window.admitted_count
        || window
            .verdict_count
            .checked_add(window.expired_unverified_count)
            .ok_or(CodecError::ArithmeticOverflow)?
            != window.revealed_count
        || window.epoch != request.epoch_account
        || window.epoch_generation != request.epoch_generation
        || window.market != market.market
        || window.relation_policy_id != market.relation_policy_id
        || window.admission_policy_id != market.admission_policy_id
        || window.score_policy_id != market.score_policy_id
        || node.epoch != request.epoch_account
        || node.epoch_generation != request.epoch_generation
        || node.market != market.market
        || node.relation_policy_id != market.relation_policy_id
        || node.admission_policy_id != market.admission_policy_id
        || node.score_policy_id != market.score_policy_id
        || node.status != AdmissionNodeStatusV1::VerifiedValid
        || node.candidate_kind != request.feed.candidate_kind
        || request.node.cost_certificate_id().is_zero()
        || window.best_candidate_node != node.node
        || window.best_settlement_candidate_id != node.settlement_candidate_id
        || window.best_rank_key != node.rank_key
        || window.best_ordinal != node.ordinal
        || market.market_instance_v2_id != request.market_instance_v2_id
        || request.feed.epoch != request.epoch_account
        || request.feed.epoch_generation != request.epoch_generation
        || request.feed.market != market.market
        || request.feed.node != node.node
        || request.feed.settlement_candidate_id != node.settlement_candidate_id
        || request.feed.base_relation_candidate_id != node.base_relation_candidate_id
        || request.feed.settlement_witness_digest != node.settlement_witness_digest
        || request.feed.relation_policy_id != market.relation_policy_id
        || request.feed.price_measure_policy_v1_id != market.price_measure_policy_v1_id
        || request.feed.native_claim_basis_id != market.native_claim_basis_id
        || request.feed.outcome_count != market.outcome_count
        || request.cash_expectation.market != market.market.bytes()
        || request.cash_expectation.epoch != request.epoch_account.bytes()
        || request.cash_expectation.candidate != node.settlement_candidate_id.bytes()
        || request.cash_expectation.owner_order_set_digest != request.owner_order_set_digest.bytes()
        || request.cash_expectation.price_scale != market.price_scale
        || request.cash_expectation.owner_count == 0
        || request.cash_expectation.owner_count > u16::from(request.feed.order_count)
        || request.expected_reservations != u16::from(request.feed.order_count)
        || request.expected_filled_reservations == 0
        || request.expected_filled_reservations > request.expected_reservations
        || request.cash_expectation.owner_count > request.expected_filled_reservations
        || request.expected_merge_payments > request.feed.slice_count
        || (direction == VirtualCashDirectionV1::Merge) != (request.expected_merge_payments != 0)
    {
        return Err(CodecError::MismatchedBinding);
    }
    let final_pot_present = direction != VirtualCashDirectionV1::None;
    if final_pot_present
        != (!request.final_pot.is_zero() && request.final_pot_rent.get()?.is_some())
        || (!final_pot_present && request.final_pot_bump != 0)
    {
        return Err(CodecError::MismatchedBinding);
    }
    let virtual_kind = match direction {
        VirtualCashDirectionV1::None => {
            if request.feed.virtual_split != 0 || request.feed.virtual_merge != 0 {
                return Err(CodecError::MismatchedBinding);
            }
            None
        }
        VirtualCashDirectionV1::Split => {
            if request.feed.virtual_split == 0
                || request.feed.virtual_merge != 0
                || request.feed.virtual_split != request.cash_expectation.virtual_cash_atoms
            {
                return Err(CodecError::MismatchedBinding);
            }
            Some(VirtualReceiptKindV1::Split)
        }
        VirtualCashDirectionV1::Merge => {
            if request.feed.virtual_merge == 0
                || request.feed.virtual_split != 0
                || request.feed.virtual_merge != request.cash_expectation.virtual_cash_atoms
            {
                return Err(CodecError::MismatchedBinding);
            }
            Some(VirtualReceiptKindV1::Merge)
        }
    };
    let fee_record = Id32::from_bytes(request.cash_expectation.fee_record);
    let root = SettlementRootV1AccountV1 {
        epoch: request.epoch_account,
        market: market.market,
        market_instance_v2_id: request.market_instance_v2_id,
        market_binding: request.market_binding_account,
        window: request.window_account,
        source_admission_node: node.node,
        retained_feed: request.retained_feed_account,
        order_set: request.feed.order_set,
        settlement_candidate_id: node.settlement_candidate_id,
        candidate_bundle_digest: node.candidate_bundle_digest,
        settlement_witness_digest: node.settlement_witness_digest,
        owner_order_set_digest: request.owner_order_set_digest,
        cost_certificate_id: request.node.cost_certificate_id(),
        batch_policy_id: request.market.batch_policy_id(),
        score_policy_id: market.score_policy_id,
        fee_record,
        settlement_cash_pot: request.settlement_cash_pot,
        final_pot: request.final_pot,
        solver_reward_destination: node.solver_reward_destination,
        rank_key: node.rank_key,
        root_rent: request.root_rent,
        cash_pot_rent: request.cash_pot_rent,
        final_pot_rent: request.final_pot_rent,
        price_scale: request.cash_expectation.price_scale,
        consideration_debit_atoms: request.cash_expectation.consideration_debit_atoms,
        seller_credit_atoms: request.cash_expectation.seller_credit_atoms,
        selected_fee_atoms: request.cash_expectation.selected_fee_atoms,
        virtual_cash_atoms: request.cash_expectation.virtual_cash_atoms,
        rounding_pot_price_units: request.cash_expectation.rounding_pot_price_units,
        epoch_generation: request.epoch_generation,
        selected_ordinal: node.ordinal,
        selected_slot: request.current_slot,
        counts: SettlementRootChildCountsV1 {
            expected_receipts: request.feed.slice_count,
            admitted_receipts: 0,
            live_receipts: 0,
            expected_owner_rows: request.cash_expectation.owner_count,
            admitted_owner_rows: 0,
            live_owner_rows: 0,
            expected_reservations: request.expected_reservations,
            expected_filled_reservations: request.expected_filled_reservations,
            admitted_reservations: 0,
            live_reservations: 0,
            released_unfilled_reservations: 0,
            completed_owner_finalizations: 0,
            live_fee_finalizations: 0,
            expected_dealer_children: u16::from(
                request.feed.candidate_kind == SettlementCandidateKindV1::CoveredDealer,
            ),
            admitted_dealer_children: 0,
            live_dealer_children: 0,
            expected_merge_payments: request.expected_merge_payments,
            admitted_merge_payments: 0,
            completed_merge_payments: 0,
        },
        outcome_count: request.feed.outcome_count,
        order_count: request.feed.order_count,
        virtual_cash_direction: direction,
        phase: SettlementRootPhaseV1::Materializing,
        cash_pot_state: if direction == VirtualCashDirectionV1::Merge {
            SettlementRootChildStateV1::ExpectedUncreated
        } else {
            SettlementRootChildStateV1::Live
        },
        final_pot_state: if final_pot_present {
            SettlementRootChildStateV1::Live
        } else {
            SettlementRootChildStateV1::Absent
        },
        retained_feed_state: SettlementRootChildStateV1::Live,
        fee_record_state: if fee_record.is_zero() {
            SettlementRootChildStateV1::Absent
        } else {
            SettlementRootChildStateV1::Live
        },
        stored_bump: request.root_bump,
        cash_pot_bump: request.cash_pot_bump,
        final_pot_bump: if final_pot_present {
            request.final_pot_bump
        } else {
            0
        },
        flags: 0,
    };
    root.validate()?;
    let epoch = finalize_epoch_for_settlement_root(request.epoch)?;
    let mut window_after = *window;
    window_after.finalized_slot = request.current_slot;
    window_after.selected_candidate_artifact = request.root_account;
    window_after.best_candidate_node = Id32::ZERO;
    window_after.best_settlement_candidate_id = Id32::ZERO;
    window_after.best_rank_key = [0; SCORE_V2_Q_RANK_CAPACITY];
    window_after.best_ordinal = 0;
    let window = CandidateWindowV5AccountV1::new(window_after)?;
    let cash_pot = if direction == VirtualCashDirectionV1::Merge {
        None
    } else {
        Some(
            SettlementCashPotV1::new(request.cash_expectation)
                .map_err(|_| CodecError::InvalidState)?,
        )
    };
    let final_pot = match virtual_kind {
        None => None,
        Some(kind) => Some(SettlementFinalPotInitializationV1 {
            account: request.final_pot,
            market: market.market,
            epoch: request.epoch_account,
            candidate: node.settlement_candidate_id,
            owner_order_set_digest: request.owner_order_set_digest,
            settlement_witness_digest: node.settlement_witness_digest,
            kind,
            authorized_complete_set_atoms: request.cash_expectation.virtual_cash_atoms,
            outcome_count: request.feed.outcome_count,
            rent: request
                .final_pot_rent
                .get()?
                .ok_or(CodecError::InvalidState)?,
            stored_bump: request.final_pot_bump,
        }),
    };
    Ok(InitializeSettlementRootPlanV1 {
        epoch,
        root,
        window,
        cash_pot,
        final_pot,
    })
}

fn finalize_epoch_for_settlement_root(
    epoch: &GeneralEpochV6AccountV1,
) -> Result<GeneralEpochV6AccountV1, CodecError> {
    epoch.validate()?;
    if epoch.phase != GeneralEpochPhaseV1::Frozen || epoch.selected_candidate_count != 0 {
        return Err(CodecError::MismatchedBinding);
    }
    let successor = GeneralEpochV6AccountV1 {
        selected_candidate_count: 1,
        phase: GeneralEpochPhaseV1::Finalized,
        ..*epoch
    };
    successor.validate()?;
    Ok(successor)
}

/// Structural zero-liability handoff for the Product occurrence adapter.
///
/// This pure projection deliberately carries no account-owner/PDA boolean and
/// grants no close authority by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementRootTerminalProjectionV1 {
    root_account: Id32,
    market_instance_v2_id: Id32,
    market: Id32,
    epoch: Id32,
    epoch_generation: u64,
    settlement_candidate_id: Id32,
    terminal_receipt_id: Id32,
}

impl SettlementRootTerminalProjectionV1 {
    /// Exact root account being consumed.
    pub const fn root_account(&self) -> Id32 {
        self.root_account
    }
    /// Full Product occurrence identity.
    pub const fn market_instance_v2_id(&self) -> Id32 {
        self.market_instance_v2_id
    }
    /// Actual General MarketRuntime PDA.
    pub const fn market(&self) -> Id32 {
        self.market
    }
    /// Counted General Epoch PDA.
    pub const fn epoch(&self) -> Id32 {
        self.epoch
    }
    /// Exact nonzero General generation.
    pub const fn epoch_generation(&self) -> u64 {
        self.epoch_generation
    }
    /// Stable final candidate identity.
    pub const fn settlement_candidate_id(&self) -> Id32 {
        self.settlement_candidate_id
    }
    /// Content identity of the exact terminal root account.
    pub const fn terminal_receipt_id(&self) -> Id32 {
        self.terminal_receipt_id
    }
}

fn require_live(identity: Id32) -> Result<(), CodecError> {
    if identity.is_zero() {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn checked_add(value: u16, delta: u16) -> Result<u16, CodecError> {
    value
        .checked_add(delta)
        .ok_or(CodecError::ArithmeticOverflow)
}

fn validate_selected_rank(
    rank: &[u8; SCORE_V2_Q_RANK_CAPACITY],
    candidate: Id32,
    ordinal: u64,
) -> Result<(), CodecError> {
    let candidate_bytes = candidate.bytes();
    let ordinal_bytes = FirstAdmittedTieV1 { ordinal }.coordinate()?;
    let mut index = 0usize;
    while index < ID_BYTES {
        if rank[32 + index] != !candidate_bytes[index] || rank[64 + index] != !ordinal_bytes[index]
        {
            return Err(CodecError::MismatchedBinding);
        }
        index += 1;
    }
    Ok(())
}

fn direction_byte(direction: VirtualCashDirectionV1) -> u8 {
    match direction {
        VirtualCashDirectionV1::None => 0,
        VirtualCashDirectionV1::Split => 1,
        VirtualCashDirectionV1::Merge => 2,
    }
}

fn decode_direction(value: u8) -> Result<VirtualCashDirectionV1, CodecError> {
    match value {
        0 => Ok(VirtualCashDirectionV1::None),
        1 => Ok(VirtualCashDirectionV1::Split),
        2 => Ok(VirtualCashDirectionV1::Merge),
        _ => Err(CodecError::InvalidState),
    }
}

fn write_rent(writer: &mut Writer<'_>, rent: DeletableRentOwnerV1) -> Result<(), CodecError> {
    writer.bytes(&rent.payer.bytes())?;
    writer.u64(rent.refundable_principal)?;
    writer.u64(rent.donation_floor)
}

fn read_rent(reader: &mut Reader<'_>) -> Result<DeletableRentOwnerV1, CodecError> {
    let value = DeletableRentOwnerV1 {
        payer: Id32::from_bytes(reader.array()?),
        refundable_principal: reader.u64()?,
        donation_floor: reader.u64()?,
    };
    value.validate()?;
    Ok(value)
}

fn write_optional_rent(
    writer: &mut Writer<'_>,
    rent: OptionalSettlementRentV1,
) -> Result<(), CodecError> {
    rent.get()?;
    writer.bytes(&rent.payer.bytes())?;
    writer.u64(rent.refundable_principal)?;
    writer.u64(rent.donation_floor)
}

fn read_optional_rent(reader: &mut Reader<'_>) -> Result<OptionalSettlementRentV1, CodecError> {
    let value = OptionalSettlementRentV1 {
        payer: Id32::from_bytes(reader.array()?),
        refundable_principal: reader.u64()?,
        donation_floor: reader.u64()?,
    };
    value.get()?;
    Ok(value)
}

fn write_counts(
    writer: &mut Writer<'_>,
    counts: SettlementRootChildCountsV1,
) -> Result<(), CodecError> {
    counts.validate()?;
    for value in [
        counts.expected_receipts,
        counts.admitted_receipts,
        counts.live_receipts,
        counts.expected_owner_rows,
        counts.admitted_owner_rows,
        counts.live_owner_rows,
        counts.expected_reservations,
        counts.expected_filled_reservations,
        counts.admitted_reservations,
        counts.live_reservations,
        counts.released_unfilled_reservations,
        counts.completed_owner_finalizations,
        counts.live_fee_finalizations,
        counts.expected_dealer_children,
        counts.admitted_dealer_children,
        counts.live_dealer_children,
        counts.expected_merge_payments,
        counts.admitted_merge_payments,
        counts.completed_merge_payments,
    ] {
        writer.u16(value)?;
    }
    Ok(())
}

fn read_counts(reader: &mut Reader<'_>) -> Result<SettlementRootChildCountsV1, CodecError> {
    let value = SettlementRootChildCountsV1 {
        expected_receipts: reader.u16()?,
        admitted_receipts: reader.u16()?,
        live_receipts: reader.u16()?,
        expected_owner_rows: reader.u16()?,
        admitted_owner_rows: reader.u16()?,
        live_owner_rows: reader.u16()?,
        expected_reservations: reader.u16()?,
        expected_filled_reservations: reader.u16()?,
        admitted_reservations: reader.u16()?,
        live_reservations: reader.u16()?,
        released_unfilled_reservations: reader.u16()?,
        completed_owner_finalizations: reader.u16()?,
        live_fee_finalizations: reader.u16()?,
        expected_dealer_children: reader.u16()?,
        admitted_dealer_children: reader.u16()?,
        live_dealer_children: reader.u16()?,
        expected_merge_payments: reader.u16()?,
        admitted_merge_payments: reader.u16()?,
        completed_merge_payments: reader.u16()?,
    };
    value.validate()?;
    Ok(value)
}

const _: () = assert!(IDENTITY_COUNT == 19);
const _: () = assert!(SETTLEMENT_ROOT_ACCOUNT_BYTES == 980);

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn id(byte: u8) -> Id32 {
        Id32::from_bytes([byte; ID_BYTES])
    }

    fn frozen_epoch() -> GeneralEpochV6AccountV1 {
        GeneralEpochV6AccountV1 {
            market_binding: id(1),
            market_runtime: id(2),
            market_instance_v2_id: id(3),
            economic_domain: id(4),
            window: id(5),
            budget: id(6),
            order_set: id(7),
            epoch_index: 8,
            generation: 9,
            freeze_deadline_slot: 10,
            frozen_slot: 10,
            candidate_bundle_count: 1,
            work_count: 0,
            selected_candidate_count: 0,
            rent: DeletableRentOwnerV1 {
                payer: id(11),
                refundable_principal: 12,
                donation_floor: 0,
            },
            phase: GeneralEpochPhaseV1::Frozen,
            stored_bump: 13,
            flags: 0,
        }
    }

    pub(crate) fn portfolio_settling_root() -> SettlementRootV1AccountV1 {
        let candidate = id(9);
        let ordinal = 1u64;
        let mut rank_key = [0u8; SCORE_V2_Q_RANK_CAPACITY];
        let candidate_bytes = candidate.bytes();
        let ordinal_bytes = FirstAdmittedTieV1 { ordinal }.coordinate().unwrap();
        let mut index = 0usize;
        while index < ID_BYTES {
            rank_key[32 + index] = !candidate_bytes[index];
            rank_key[64 + index] = !ordinal_bytes[index];
            index += 1;
        }
        SettlementRootV1AccountV1 {
            epoch: id(1),
            market: id(2),
            market_instance_v2_id: id(3),
            market_binding: id(4),
            window: id(5),
            source_admission_node: id(6),
            retained_feed: id(7),
            order_set: id(8),
            settlement_candidate_id: candidate,
            candidate_bundle_digest: id(10),
            settlement_witness_digest: id(11),
            owner_order_set_digest: id(12),
            cost_certificate_id: id(13),
            batch_policy_id: id(14),
            score_policy_id: id(15),
            fee_record: Id32::ZERO,
            settlement_cash_pot: id(16),
            final_pot: Id32::ZERO,
            solver_reward_destination: id(17),
            rank_key,
            root_rent: DeletableRentOwnerV1 {
                payer: id(18),
                refundable_principal: 100,
                donation_floor: 0,
            },
            cash_pot_rent: DeletableRentOwnerV1 {
                payer: id(19),
                refundable_principal: 100,
                donation_floor: 0,
            },
            final_pot_rent: OptionalSettlementRentV1::ABSENT,
            price_scale: 10_000,
            consideration_debit_atoms: 10,
            seller_credit_atoms: 10,
            selected_fee_atoms: 0,
            virtual_cash_atoms: 0,
            rounding_pot_price_units: 0,
            epoch_generation: 1,
            selected_ordinal: ordinal,
            selected_slot: 1,
            counts: SettlementRootChildCountsV1 {
                expected_receipts: 2,
                admitted_receipts: 2,
                live_receipts: 2,
                expected_owner_rows: 2,
                admitted_owner_rows: 2,
                live_owner_rows: 2,
                expected_reservations: 2,
                expected_filled_reservations: 2,
                admitted_reservations: 2,
                live_reservations: 2,
                released_unfilled_reservations: 0,
                completed_owner_finalizations: 2,
                live_fee_finalizations: 0,
                expected_dealer_children: 0,
                admitted_dealer_children: 0,
                live_dealer_children: 0,
                expected_merge_payments: 0,
                admitted_merge_payments: 0,
                completed_merge_payments: 0,
            },
            outcome_count: 2,
            order_count: 2,
            virtual_cash_direction: VirtualCashDirectionV1::None,
            phase: SettlementRootPhaseV1::Settling,
            cash_pot_state: SettlementRootChildStateV1::Live,
            final_pot_state: SettlementRootChildStateV1::Absent,
            retained_feed_state: SettlementRootChildStateV1::Live,
            fee_record_state: SettlementRootChildStateV1::Absent,
            stored_bump: 1,
            cash_pot_bump: 2,
            final_pot_bump: 0,
            flags: 0,
        }
    }

    pub(crate) fn pre_feed_terminal_frontier_root() -> SettlementRootV1AccountV1 {
        let root = portfolio_settling_root()
            .retire_portfolio_pair_archives(2)
            .unwrap();
        let root = root.retire_one_owner_row().unwrap();
        let root = root.retire_one_owner_row().unwrap();
        let root = root.begin_retiring().unwrap();
        let root = root.retire_cash_pot().unwrap();
        root.validate().unwrap();
        root
    }

    pub(crate) fn terminal_root() -> SettlementRootV1AccountV1 {
        let mut root = pre_feed_terminal_frontier_root();
        root.retained_feed_state = SettlementRootChildStateV1::Retired;
        root.phase = SettlementRootPhaseV1::Terminal;
        root.validate().unwrap();
        root
    }

    pub(crate) fn materializing_root() -> SettlementRootV1AccountV1 {
        let mut root = portfolio_settling_root();
        root.counts.admitted_receipts = 0;
        root.counts.live_receipts = 0;
        root.counts.admitted_owner_rows = 0;
        root.counts.live_owner_rows = 0;
        root.counts.admitted_reservations = 0;
        root.counts.live_reservations = 0;
        root.counts.completed_owner_finalizations = 0;
        root.phase = SettlementRootPhaseV1::Materializing;
        root.validate().unwrap();
        root
    }

    #[derive(Debug)]
    struct EchoHash;

    impl Sha256BackendV1 for EchoHash {
        fn sha256(&self, _parts: &[&[u8]]) -> [u8; 32] {
            [0x44; 32]
        }
    }

    #[test]
    fn root_creation_owns_the_only_frozen_to_finalized_epoch_transition() {
        let epoch = frozen_epoch();
        let successor = finalize_epoch_for_settlement_root(&epoch).unwrap();
        assert_eq!(successor.phase, GeneralEpochPhaseV1::Finalized);
        assert_eq!(successor.selected_candidate_count, 1);

        let mut hostile = epoch;
        hostile.phase = GeneralEpochPhaseV1::Finalized;
        assert!(finalize_epoch_for_settlement_root(&hostile).is_err());
    }

    #[test]
    fn indexed_root_rent_paths_preserve_full_principal_and_donation() {
        let root = materializing_root();
        let upgrade = crate::prepare_indexed_settlement_root_upgrade_rent_v1(
            &root,
            id(20),
            110,
            150,
            50,
            id(21),
            &EchoHash,
        )
        .unwrap();
        assert_eq!(upgrade.data_len_before(), SETTLEMENT_ROOT_ACCOUNT_BYTES);
        assert_eq!(upgrade.data_len_after(), 1_196);
        assert_eq!(upgrade.payer_debit_lamports(), 50);
        assert_eq!(upgrade.root_balance_after_lamports(), 160);
        assert_eq!(upgrade.rent_after().refundable_principal, 150);
        assert_eq!(upgrade.rent_after().donation_floor, 10);

        let fresh_base = root
            .with_indexed_root_rent(DeletableRentOwnerV1 {
                payer: id(18),
                refundable_principal: 150,
                donation_floor: 10,
            })
            .unwrap();
        let fresh = crate::prepare_fresh_indexed_settlement_root_rent_v1(
            &fresh_base,
            id(20),
            10,
            150,
            150,
            id(21),
            &EchoHash,
        )
        .unwrap();
        assert_eq!(fresh.data_len_before(), 0);
        assert_eq!(fresh.payer_debit_lamports(), 150);
        assert_eq!(fresh.root_balance_after_lamports(), 160);
        assert_eq!(fresh.rent_after().donation_floor, 10);

        assert!(crate::prepare_indexed_settlement_root_upgrade_rent_v1(
            &root,
            id(20),
            110,
            99,
            100,
            id(21),
            &EchoHash,
        )
        .is_err());
        assert!(crate::prepare_indexed_settlement_root_upgrade_rent_v1(
            &root,
            id(20),
            110,
            150,
            49,
            id(21),
            &EchoHash,
        )
        .is_err());
    }

    #[test]
    fn fresh_final_pot_encoding_is_exact_and_root_owned() {
        let initialization = SettlementFinalPotInitializationV1 {
            account: id(1),
            market: id(2),
            epoch: id(3),
            candidate: id(4),
            owner_order_set_digest: id(5),
            settlement_witness_digest: id(6),
            kind: VirtualReceiptKindV1::Split,
            authorized_complete_set_atoms: 7,
            outcome_count: 2,
            rent: DeletableRentOwnerV1 {
                payer: id(8),
                refundable_principal: 9,
                donation_floor: 0,
            },
            stored_bump: 10,
        };
        let mut bytes = [0u8; FINAL_POT_ACCOUNT_BYTES];
        initialization.encode(&mut bytes).unwrap();
        assert_eq!(bytes[0], FINAL_POT_ACCOUNT_TAG);
        assert_eq!(bytes[1], FINAL_POT_ACCOUNT_VERSION);
        assert_eq!(bytes[FINAL_POT_ACCOUNT_BYTES - 2], 10);
        assert_eq!(bytes[FINAL_POT_ACCOUNT_BYTES - 1], 0);
        let body: [u8; 328] = bytes[2..330].try_into().unwrap();
        let decoded = AuthenticatedFinalPotV1::decode_body(&body, id(1).bytes(), true, true).unwrap();
        assert_eq!(decoded.candidate, id(4).bytes());
        assert_eq!(decoded.inventory_kind, VirtualReceiptKindV1::Split);
        assert_eq!(decoded.authorized_complete_set_atoms, 7);

        let mut short = [0u8; FINAL_POT_ACCOUNT_BYTES - 1];
        assert!(initialization.encode(&mut short).is_err());
        let hostile = SettlementFinalPotInitializationV1 {
            outcome_count: 1,
            ..initialization
        };
        assert!(hostile.encode(&mut bytes).is_err());
    }

    #[test]
    fn portfolio_archive_counts_retire_exhaustively_once() {
        let root = portfolio_settling_root();
        root.validate().unwrap();
        assert_eq!(
            root.retire_portfolio_pair_archives(1),
            Err(CodecError::InvalidState)
        );
        let post = root.retire_portfolio_pair_archives(2).unwrap();
        assert_eq!(post.phase(), SettlementRootPhaseV1::Settling);
        assert_eq!(post.counts().live_receipts, 0);
        assert_eq!(post.counts().live_reservations, 0);
        assert_eq!(post.begin_retiring(), Err(CodecError::InvalidState));
        assert_eq!(
            post.retire_portfolio_pair_archives(2),
            Err(CodecError::InvalidState)
        );
        let post = post.retire_one_owner_row().unwrap();
        let post = post.retire_one_owner_row().unwrap();
        let post = post.begin_retiring().unwrap();
        assert_eq!(post.phase(), SettlementRootPhaseV1::Retiring);
    }

    #[test]
    fn narrow_scalar_closes_are_exact_and_begin_retiring_is_a_hard_gate() {
        let mut incomplete = portfolio_settling_root();
        incomplete.counts.completed_owner_finalizations = 0;
        incomplete.validate().unwrap();
        assert_eq!(
            incomplete.retire_one_owner_row(),
            Err(CodecError::InvalidState)
        );

        let root = portfolio_settling_root();
        assert_eq!(root.begin_retiring(), Err(CodecError::InvalidState));
        let mut forged = root;
        forged.phase = SettlementRootPhaseV1::Retiring;
        assert_eq!(forged.validate(), Err(CodecError::InvalidState));
        let root = root.retire_one_receipt().unwrap();
        let root = root.retire_one_receipt().unwrap();
        assert_eq!(
            root.retire_one_receipt(),
            Err(CodecError::InvalidState)
        );
        let root = root.retire_one_reservation().unwrap();
        let root = root.retire_one_reservation().unwrap();
        assert_eq!(
            root.retire_one_reservation(),
            Err(CodecError::InvalidState)
        );
        let root = root.retire_one_owner_row().unwrap();
        assert_eq!(root.begin_retiring(), Err(CodecError::InvalidState));
        let root = root.retire_one_owner_row().unwrap();
        let root = root.begin_retiring().unwrap();
        assert_eq!(root.begin_retiring(), Err(CodecError::InvalidState));
        assert_eq!(root.retire_final_pot(), Err(CodecError::InvalidState));
        assert_eq!(root.retire_fee_record(), Err(CodecError::InvalidState));
        let root = root.retire_cash_pot().unwrap();
        assert!(root.at_retained_feed_retirement_frontier());
        assert_eq!(root.retire_cash_pot(), Err(CodecError::InvalidState));
    }

    #[test]
    fn fee_children_must_close_before_phase_change_and_fee_record_afterward() {
        let root = fee_settling_root();
        let root = root.retire_one_receipt().unwrap();
        let root = root.retire_one_receipt().unwrap();
        let root = root.retire_one_reservation().unwrap();
        let root = root.retire_one_reservation().unwrap();
        let root = root.retire_one_owner_row().unwrap();
        let root = root.retire_one_owner_row().unwrap();
        assert_eq!(root.begin_retiring(), Err(CodecError::InvalidState));
        assert_eq!(root.retire_fee_record(), Err(CodecError::InvalidState));
        let root = root.retire_one_fee_finalization().unwrap();
        let root = root.retire_one_fee_finalization().unwrap();
        assert_eq!(
            root.retire_one_fee_finalization(),
            Err(CodecError::InvalidState)
        );
        let root = root.begin_retiring().unwrap();
        assert_eq!(
            root.retire_one_fee_finalization(),
            Err(CodecError::InvalidState)
        );
        let root = root.retire_cash_pot().unwrap();
        assert!(!root.at_retained_feed_retirement_frontier());
        let root = root.retire_fee_record().unwrap();
        assert!(root.at_retained_feed_retirement_frontier());
    }

    fn close_two_scalar_children(
        root: SettlementRootV1AccountV1,
    ) -> SettlementRootV1AccountV1 {
        let root = root.retire_one_receipt().unwrap();
        let root = root.retire_one_receipt().unwrap();
        let root = root.retire_one_reservation().unwrap();
        let root = root.retire_one_reservation().unwrap();
        let root = root.retire_one_owner_row().unwrap();
        root.retire_one_owner_row().unwrap()
    }

    pub(crate) fn fee_settling_root() -> SettlementRootV1AccountV1 {
        let mut root = portfolio_settling_root();
        root.fee_record = id(20);
        root.selected_fee_atoms = 1;
        root.fee_record_state = SettlementRootChildStateV1::Live;
        root.counts.live_fee_finalizations = 2;
        root.validate().unwrap();
        root
    }

    pub(crate) fn virtual_root(
        direction: VirtualCashDirectionV1,
    ) -> SettlementRootV1AccountV1 {
        let mut root = portfolio_settling_root();
        root.virtual_cash_direction = direction;
        root.virtual_cash_atoms = 1;
        root.final_pot = id(20);
        root.final_pot_state = SettlementRootChildStateV1::Live;
        root.final_pot_rent = OptionalSettlementRentV1::present(DeletableRentOwnerV1 {
            payer: id(30),
            refundable_principal: 100,
            donation_floor: 0,
        })
        .unwrap();
        match direction {
            VirtualCashDirectionV1::Split => root.seller_credit_atoms = 9,
            VirtualCashDirectionV1::Merge => {
                root.seller_credit_atoms = 11;
                root.counts.expected_merge_payments = 2;
                root.counts.admitted_merge_payments = 2;
                root.counts.completed_merge_payments = 2;
            }
            VirtualCashDirectionV1::None => unreachable!(),
        }
        root.validate().unwrap();
        root
    }

    pub(crate) fn dealer_settling_root() -> SettlementRootV1AccountV1 {
        let mut root = portfolio_settling_root();
        root.counts.expected_dealer_children = 1;
        root.counts.admitted_dealer_children = 1;
        root.counts.live_dealer_children = 1;
        root.validate().unwrap();
        root
    }

    #[test]
    fn split_and_merge_require_their_final_pot_and_completed_merge_latches() {
        for direction in [VirtualCashDirectionV1::Split, VirtualCashDirectionV1::Merge] {
            let root = close_two_scalar_children(virtual_root(direction));
            if direction == VirtualCashDirectionV1::Merge {
                let mut incomplete = root;
                incomplete.counts.completed_merge_payments = 1;
                incomplete.validate().unwrap();
                assert_eq!(incomplete.begin_retiring(), Err(CodecError::InvalidState));
            }
            let root = root.begin_retiring().unwrap();
            let root = root.retire_cash_pot().unwrap();
            assert!(!root.at_retained_feed_retirement_frontier());
            let root = root.retire_final_pot().unwrap();
            assert!(root.at_retained_feed_retirement_frontier());
            assert_eq!(root.retire_final_pot(), Err(CodecError::InvalidState));
        }
    }

    #[test]
    fn dealer_child_retires_only_after_the_hard_phase_gate() {
        let root = dealer_settling_root();
        assert_eq!(root.retire_dealer_child(), Err(CodecError::InvalidState));
        let root = close_two_scalar_children(root);
        let root = root.begin_retiring().unwrap();
        let root = root.retire_cash_pot().unwrap();
        assert!(!root.at_retained_feed_retirement_frontier());
        let root = root.retire_dealer_child().unwrap();
        assert!(root.at_retained_feed_retirement_frontier());
    }
}
