// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{DealerFacilityIdV1, DealerLivenessScheduleIdV1, Error, Id, Result, MAX_LP_PAGES};

/// Canonical PDA seed prefix for immutable DealerPolicy artifacts.
pub const DEALER_POLICY_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-policy-v1";
/// Canonical PDA seed prefix for immutable facility-genesis artifacts.
pub const DEALER_FACILITY_GENESIS_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-facility-v1";
/// Canonical PDA seed prefix for the singleton Position authority binding.
pub const FACILITY_POSITION_BINDING_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-pos-bind-v1";
/// Canonical PDA seed prefix for the singleton Facility Position account.
pub const DEALER_FACILITY_POSITION_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-position-v1";
/// Canonical PDA seed prefix for the singleton Facility Replay account.
pub const DEALER_FACILITY_REPLAY_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-replay-v1";
/// Canonical PDA seed prefix for immutable liveness schedules.
pub const DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-live-sched-v1";
/// Canonical PDA seed prefix for immutable funded-dependency artifacts.
pub const DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-funded-v1";
/// Canonical PDA seed prefix for counted, rent-owned funded dependencies.
pub const DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2: &[u8] = b"dc-dealer-funded-v2";
/// Canonical PDA seed prefix for mutable DealerState roots.
pub const DEALER_STATE_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-state-v1";
/// Canonical PDA seed prefix for authoritative DealerState V2 roots.
pub const DEALER_STATE_PDA_DOMAIN_V2: &[u8] = b"dc-dealer-state-v2";
/// Canonical PDA seed prefix for counted LP pages.
pub const LP_PAGE_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-lp-page-v1";
/// Canonical PDA seed prefix for V2 LP ownership pages.
pub const LP_PAGE_PDA_DOMAIN_V2: &[u8] = b"dc-dealer-lp-page-v2";
/// Canonical PDA seed prefix for one-generation leases.
pub const DEALER_LEASE_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-lease-v1";
/// Canonical PDA seed prefix for external-liveness DealerLease V2 accounts.
pub const DEALER_LEASE_PDA_DOMAIN_V2: &[u8] = b"dc-dealer-lease-v2";
/// Canonical PDA seed prefix for three-stage settlement pots.
pub const SETTLEMENT_POT_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-pot-v1";
/// Canonical PDA seed prefix for owner-netted SettlementPot V2 accounts.
pub const SETTLEMENT_POT_PDA_DOMAIN_V2: &[u8] = b"dc-dealer-pot-v2";
/// Canonical PDA seed prefix for one active Dealer Epoch binding.
pub const DEALER_EPOCH_BINDING_PDA_DOMAIN_V2: &[u8] = b"dc-dealer-epoch-v2";
/// Canonical PDA seed prefix for one page-scoped terminal allocation.
pub const DEALER_TERMINAL_ALLOCATION_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-term-v1";
/// Canonical PDA seed prefix for singleton terminal claim work.
pub const DEALER_CLAIM_WORK_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-claim-v1";
/// Canonical PDA seed prefix for one owner-scoped exit ticket.
pub const DEALER_EXIT_TICKET_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-exit-v1";
/// Canonical PDA seed prefix for the permanent V2 root tombstone.
pub const DEALER_ROOT_TOMBSTONE_PDA_DOMAIN_V2: &[u8] = b"dc-dealer-root-v2";
/// Canonical PDA seed prefix for one content-addressed action receipt.
pub const DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-action-receipt-v1";
/// Canonical selection attachment addressed by counted Epoch and final candidate.
pub const DEALER_COVERED_SELECTION_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-covered-v1";
/// Facility-lifetime Product Series-obligation binding.
pub const DEALER_SERIES_OBLIGATION_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-series-obligation-v1";
/// Canonical PDA seed prefix for segregated fee budgets.
pub const FEE_BUDGET_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-fee-v1";
/// Canonical PDA seed prefix for segregated liveness budgets.
pub const LIVENESS_BUDGET_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-live-v1";

/// Maximum canonical seed count in a V1 dealer PDA recipe.
pub const MAX_DEALER_PDA_SEEDS: usize = 3;

/// Disjoint V1 PDA families. Values are local documentation selectors, not tags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerPdaFamilyV1 {
    /// Immutable policy content address.
    Policy = 0,
    /// Immutable facility genesis addressed by its canonical content identity.
    FacilityGenesis = 7,
    /// Singleton external Position authority binding addressed by facility.
    FacilityPositionBinding = 8,
    /// Singleton Facility Position asset-accounting owner.
    FacilityPosition = 9,
    /// Singleton Facility Position Replay companion.
    FacilityReplay = 10,
    /// Immutable policy-selected liveness schedule.
    LivenessSchedule = 11,
    /// Immutable external-liveness and fee-policy dependency artifact.
    FundedDependencies = 12,
    /// Rent-owned funded dependency successor counted by State V2.
    FundedDependenciesV2 = 13,
    /// Mutable state root addressed by immutable facility identity.
    State = 1,
    /// Authoritative successor state without legacy budget children.
    StateV2 = 14,
    /// LP page addressed by facility and page ordinal.
    LpPage = 2,
    /// V2 LP page addressed by facility and page ordinal.
    LpPageV2 = 17,
    /// Lease addressed by facility and consumed generation.
    Lease = 3,
    /// External-liveness/owner-netted fee lease successor.
    LeaseV2 = 15,
    /// Settlement pot addressed by facility and consumed generation.
    SettlementPot = 4,
    /// External-liveness/owner-netted fee pot successor.
    SettlementPotV2 = 16,
    /// Active Dealer Epoch binding addressed by consumed generation.
    EpochBindingV2 = 18,
    /// Page-scoped terminal allocation addressed by page ordinal.
    TerminalAllocationV1 = 19,
    /// Singleton terminal claim work.
    ClaimWorkV1 = 20,
    /// Permanent V2 root tombstone.
    RootTombstoneV2 = 21,
    /// Owner-scoped mutable exit ticket.
    ExitTicketV1 = 22,
    /// Deletable immutable action receipt addressed by its semantic slot.
    ActionReceiptV1 = 23,
    /// Counted CoveredDealer selection attachment.
    CoveredDealerSelectionV1 = 24,
    /// Counted facility-lifetime Product Series obligation.
    SeriesObligationV1 = 25,
    /// Singleton fee budget addressed by facility.
    FeeBudget = 5,
    /// Singleton liveness budget addressed by facility.
    LivenessBudget = 6,
}

/// One canonical nonempty Solana-compatible seed component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerSeedV1 {
    length: u8,
    bytes: [u8; 32],
}

impl DealerSeedV1 {
    const EMPTY: Self = Self {
        length: 0,
        bytes: [0; 32],
    };

    fn new(value: &[u8]) -> Result<Self> {
        if value.is_empty() || value.len() > 32 {
            return Err(Error::InvalidParameter);
        }
        let mut result = Self::EMPTY;
        result.length = u8::try_from(value.len()).map_err(|_| Error::InvalidParameter)?;
        result.bytes[..value.len()].copy_from_slice(value);
        Ok(result)
    }

    /// Exact active bytes without fixed-capacity padding.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }

    fn validate(&self) -> Result<()> {
        let length = usize::from(self.length);
        if length == 0
            || length > self.bytes.len()
            || self.bytes[length..].iter().any(|byte| *byte != 0)
        {
            Err(Error::NonCanonicalPadding)
        } else {
            Ok(())
        }
    }
}

/// Exact ordered PDA seed preimage, excluding the executing program identity
/// and Solana's bump. An adapter must derive under and authenticate its exact
/// deployed program; this crate never computes a Solana address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPdaPreimageV1 {
    /// Disjoint semantic family.
    family: DealerPdaFamilyV1,
    count: u8,
    seeds: [DealerSeedV1; MAX_DEALER_PDA_SEEDS],
}

impl DealerPdaPreimageV1 {
    /// Return the immutable semantic family selected by the constructor.
    pub const fn family(&self) -> DealerPdaFamilyV1 {
        self.family
    }

    /// Immutable policy: `[b"dc-dealer-policy-v1", policy_id]`.
    pub fn policy(policy_id: Id) -> Result<Self> {
        policy_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::Policy,
            DEALER_POLICY_PDA_DOMAIN_V1,
            &policy_id.bytes(),
        )
    }

    /// Facility genesis: `[b"dc-dealer-facility-v1", facility_id]`.
    pub fn facility_genesis(facility_id: DealerFacilityIdV1) -> Result<Self> {
        Self::two(
            DealerPdaFamilyV1::FacilityGenesis,
            DEALER_FACILITY_GENESIS_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Position binding: `[b"dc-dealer-pos-bind-v1", facility_id]`.
    pub fn facility_position_binding(facility_id: DealerFacilityIdV1) -> Result<Self> {
        Self::two(
            DealerPdaFamilyV1::FacilityPositionBinding,
            FACILITY_POSITION_BINDING_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Facility Position: `[b"dc-dealer-position-v1", facility_id]`.
    pub fn facility_position(facility_id: DealerFacilityIdV1) -> Result<Self> {
        Self::two(
            DealerPdaFamilyV1::FacilityPosition,
            DEALER_FACILITY_POSITION_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Facility Replay: `[b"dc-dealer-replay-v1", facility_id]`.
    pub fn facility_replay(facility_id: DealerFacilityIdV1) -> Result<Self> {
        Self::two(
            DealerPdaFamilyV1::FacilityReplay,
            DEALER_FACILITY_REPLAY_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Liveness schedule: `[b"dc-dealer-live-sched-v1", schedule_id]`.
    pub fn liveness_schedule(schedule_id: DealerLivenessScheduleIdV1) -> Result<Self> {
        Self::two(
            DealerPdaFamilyV1::LivenessSchedule,
            DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1,
            &schedule_id.bytes(),
        )
    }

    /// Funded dependencies: `[b"dc-dealer-funded-v1", facility_id]`.
    pub fn funded_dependencies(facility_id: DealerFacilityIdV1) -> Result<Self> {
        Self::two(
            DealerPdaFamilyV1::FundedDependencies,
            DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Counted V2 funded dependencies: `[b"dc-dealer-funded-v2", facility_id]`.
    pub fn funded_dependencies_v2(facility_id: DealerFacilityIdV1) -> Result<Self> {
        Self::two(
            DealerPdaFamilyV1::FundedDependenciesV2,
            DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2,
            &facility_id.bytes(),
        )
    }

    /// Mutable state: `[b"dc-dealer-state-v1", facility_id]`.
    pub fn state(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::State,
            DEALER_STATE_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Authoritative state successor: `[b"dc-dealer-state-v2", facility_id]`.
    pub fn state_v2(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::StateV2,
            DEALER_STATE_PDA_DOMAIN_V2,
            &facility_id.bytes(),
        )
    }

    /// Counted LP page: `[b"dc-dealer-lp-page-v1", facility_id, ordinal_le]`.
    pub fn lp_page(facility_id: Id, page_ordinal: u32) -> Result<Self> {
        facility_id.validate_live()?;
        if page_ordinal >= MAX_LP_PAGES {
            return Err(Error::InvalidParameter);
        }
        Self::three(
            DealerPdaFamilyV1::LpPage,
            LP_PAGE_PDA_DOMAIN_V1,
            &facility_id.bytes(),
            &page_ordinal.to_le_bytes(),
        )
    }

    /// V2 LP page: `[b"dc-dealer-lp-page-v2", facility_id, ordinal_le]`.
    pub fn lp_page_v2(facility_id: Id, page_ordinal: u32) -> Result<Self> {
        facility_id.validate_live()?;
        if page_ordinal >= MAX_LP_PAGES {
            return Err(Error::InvalidParameter);
        }
        Self::three(
            DealerPdaFamilyV1::LpPageV2,
            LP_PAGE_PDA_DOMAIN_V2,
            &facility_id.bytes(),
            &page_ordinal.to_le_bytes(),
        )
    }

    /// One-generation lease: `[b"dc-dealer-lease-v1", facility_id, generation_le]`.
    pub fn lease(facility_id: Id, pre_generation: u64) -> Result<Self> {
        facility_id.validate_live()?;
        Self::three(
            DealerPdaFamilyV1::Lease,
            DEALER_LEASE_PDA_DOMAIN_V1,
            &facility_id.bytes(),
            &pre_generation.to_le_bytes(),
        )
    }

    /// External-liveness lease successor addressed by consumed generation.
    pub fn lease_v2(facility_id: Id, pre_generation: u64) -> Result<Self> {
        facility_id.validate_live()?;
        Self::three(
            DealerPdaFamilyV1::LeaseV2,
            DEALER_LEASE_PDA_DOMAIN_V2,
            &facility_id.bytes(),
            &pre_generation.to_le_bytes(),
        )
    }

    /// Two-phase pot: `[b"dc-dealer-pot-v1", facility_id, generation_le]`.
    pub fn settlement_pot(facility_id: Id, pre_generation: u64) -> Result<Self> {
        facility_id.validate_live()?;
        Self::three(
            DealerPdaFamilyV1::SettlementPot,
            SETTLEMENT_POT_PDA_DOMAIN_V1,
            &facility_id.bytes(),
            &pre_generation.to_le_bytes(),
        )
    }

    /// Owner-netted pot successor addressed by consumed generation.
    pub fn settlement_pot_v2(facility_id: Id, pre_generation: u64) -> Result<Self> {
        facility_id.validate_live()?;
        Self::three(
            DealerPdaFamilyV1::SettlementPotV2,
            SETTLEMENT_POT_PDA_DOMAIN_V2,
            &facility_id.bytes(),
            &pre_generation.to_le_bytes(),
        )
    }

    /// Epoch binding: `[b"dc-dealer-epoch-v2", facility_id, generation_le]`.
    pub fn epoch_binding_v2(facility_id: Id, generation: u64) -> Result<Self> {
        facility_id.validate_live()?;
        Self::three(
            DealerPdaFamilyV1::EpochBindingV2,
            DEALER_EPOCH_BINDING_PDA_DOMAIN_V2,
            &facility_id.bytes(),
            &generation.to_le_bytes(),
        )
    }

    /// CoveredDealer selection: `[b"dc-dealer-covered-v1", epoch, candidate]`.
    pub fn covered_dealer_selection_v1(epoch_account_id: Id, candidate_id: Id) -> Result<Self> {
        epoch_account_id.validate_live()?;
        candidate_id.validate_live()?;
        Self::three(
            DealerPdaFamilyV1::CoveredDealerSelectionV1,
            DEALER_COVERED_SELECTION_PDA_DOMAIN_V1,
            &epoch_account_id.bytes(),
            &candidate_id.bytes(),
        )
    }

    /// Facility Product obligation: `[b"dc-dealer-series-obligation-v1", facility_id]`.
    pub fn series_obligation_v1(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::SeriesObligationV1,
            DEALER_SERIES_OBLIGATION_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Terminal allocation: `[b"dc-dealer-term-v1", facility_id, ordinal_le]`.
    pub fn terminal_allocation_v1(facility_id: Id, page_ordinal: u32) -> Result<Self> {
        facility_id.validate_live()?;
        if page_ordinal >= MAX_LP_PAGES {
            return Err(Error::InvalidParameter);
        }
        Self::three(
            DealerPdaFamilyV1::TerminalAllocationV1,
            DEALER_TERMINAL_ALLOCATION_PDA_DOMAIN_V1,
            &facility_id.bytes(),
            &page_ordinal.to_le_bytes(),
        )
    }

    /// Singleton claim work: `[b"dc-dealer-claim-v1", facility_id]`.
    pub fn claim_work_v1(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::ClaimWorkV1,
            DEALER_CLAIM_WORK_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Exit ticket: `[b"dc-dealer-exit-v1", facility_id, owner]`.
    pub fn exit_ticket_v1(facility_id: Id, owner: Id) -> Result<Self> {
        facility_id.validate_live()?;
        owner.validate_live()?;
        Self::three(
            DealerPdaFamilyV1::ExitTicketV1,
            DEALER_EXIT_TICKET_PDA_DOMAIN_V1,
            &facility_id.bytes(),
            &owner.bytes(),
        )
    }

    /// Action receipt: `[b"dc-dealer-action-receipt-v1", slot_id]`.
    pub fn action_receipt_v1(slot_id: Id) -> Result<Self> {
        slot_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::ActionReceiptV1,
            DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1,
            &slot_id.bytes(),
        )
    }

    /// Permanent root tombstone: `[b"dc-dealer-root-v2", facility_id]`.
    pub fn root_tombstone_v2(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::RootTombstoneV2,
            DEALER_ROOT_TOMBSTONE_PDA_DOMAIN_V2,
            &facility_id.bytes(),
        )
    }

    /// Singleton fee budget: `[b"dc-dealer-fee-v1", facility_id]`.
    pub fn fee_budget(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::FeeBudget,
            FEE_BUDGET_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Singleton liveness budget: `[b"dc-dealer-live-v1", facility_id]`.
    pub fn liveness_budget(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::LivenessBudget,
            LIVENESS_BUDGET_PDA_DOMAIN_V1,
            &facility_id.bytes(),
        )
    }

    /// Number of active seed components.
    pub const fn seed_count(&self) -> u8 {
        self.count
    }

    /// Return one exact active seed, refusing an inactive index.
    pub fn seed(&self, index: usize) -> Result<&[u8]> {
        self.validate()?;
        if index >= usize::from(self.count) {
            return Err(Error::InvalidParameter);
        }
        Ok(self.seeds[index].as_bytes())
    }

    /// Validate all active seeds and exact empty trailing capacity.
    pub fn validate(&self) -> Result<()> {
        let count = usize::from(self.count);
        if count == 0 || count > MAX_DEALER_PDA_SEEDS {
            return Err(Error::InvalidParameter);
        }
        let mut index = 0usize;
        while index < count {
            self.seeds[index].validate()?;
            index += 1;
        }
        while index < MAX_DEALER_PDA_SEEDS {
            if self.seeds[index] != DealerSeedV1::EMPTY {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        let (expected_domain, expected_count, expected_tail_len) = match self.family {
            DealerPdaFamilyV1::Policy => (DEALER_POLICY_PDA_DOMAIN_V1, 2usize, 0usize),
            DealerPdaFamilyV1::FacilityGenesis => {
                (DEALER_FACILITY_GENESIS_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::FacilityPositionBinding => {
                (FACILITY_POSITION_BINDING_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::FacilityPosition => {
                (DEALER_FACILITY_POSITION_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::FacilityReplay => {
                (DEALER_FACILITY_REPLAY_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::LivenessSchedule => {
                (DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::FundedDependencies => {
                (DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::FundedDependenciesV2 => {
                (DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2, 2usize, 0usize)
            }
            DealerPdaFamilyV1::State => (DEALER_STATE_PDA_DOMAIN_V1, 2usize, 0usize),
            DealerPdaFamilyV1::StateV2 => (DEALER_STATE_PDA_DOMAIN_V2, 2usize, 0usize),
            DealerPdaFamilyV1::LpPage => (LP_PAGE_PDA_DOMAIN_V1, 3usize, 4usize),
            DealerPdaFamilyV1::LpPageV2 => (LP_PAGE_PDA_DOMAIN_V2, 3usize, 4usize),
            DealerPdaFamilyV1::Lease => (DEALER_LEASE_PDA_DOMAIN_V1, 3usize, 8usize),
            DealerPdaFamilyV1::LeaseV2 => (DEALER_LEASE_PDA_DOMAIN_V2, 3usize, 8usize),
            DealerPdaFamilyV1::SettlementPot => (SETTLEMENT_POT_PDA_DOMAIN_V1, 3usize, 8usize),
            DealerPdaFamilyV1::SettlementPotV2 => {
                (SETTLEMENT_POT_PDA_DOMAIN_V2, 3usize, 8usize)
            }
            DealerPdaFamilyV1::EpochBindingV2 => {
                (DEALER_EPOCH_BINDING_PDA_DOMAIN_V2, 3usize, 8usize)
            }
            DealerPdaFamilyV1::TerminalAllocationV1 => {
                (DEALER_TERMINAL_ALLOCATION_PDA_DOMAIN_V1, 3usize, 4usize)
            }
            DealerPdaFamilyV1::ClaimWorkV1 => {
                (DEALER_CLAIM_WORK_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::ExitTicketV1 => {
                (DEALER_EXIT_TICKET_PDA_DOMAIN_V1, 3usize, crate::ID_BYTES)
            }
            DealerPdaFamilyV1::ActionReceiptV1 => {
                (DEALER_ACTION_RECEIPT_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::CoveredDealerSelectionV1 => {
                (DEALER_COVERED_SELECTION_PDA_DOMAIN_V1, 3usize, crate::ID_BYTES)
            }
            DealerPdaFamilyV1::SeriesObligationV1 => {
                (DEALER_SERIES_OBLIGATION_PDA_DOMAIN_V1, 2usize, 0usize)
            }
            DealerPdaFamilyV1::RootTombstoneV2 => {
                (DEALER_ROOT_TOMBSTONE_PDA_DOMAIN_V2, 2usize, 0usize)
            }
            DealerPdaFamilyV1::FeeBudget => (FEE_BUDGET_PDA_DOMAIN_V1, 2usize, 0usize),
            DealerPdaFamilyV1::LivenessBudget => (LIVENESS_BUDGET_PDA_DOMAIN_V1, 2usize, 0usize),
        };
        if count != expected_count
            || self.seeds[0].as_bytes() != expected_domain
            || self.seeds[1].as_bytes().len() != crate::ID_BYTES
            || self.seeds[1].as_bytes().iter().all(|byte| *byte == 0)
            || (expected_count == 3 && self.seeds[2].as_bytes().len() != expected_tail_len)
        {
            return Err(Error::MismatchedBinding);
        }
        if matches!(
            self.family,
            DealerPdaFamilyV1::LpPage
                | DealerPdaFamilyV1::LpPageV2
                | DealerPdaFamilyV1::TerminalAllocationV1
        ) {
            let bytes = self.seeds[2].as_bytes();
            let ordinal = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            if ordinal >= MAX_LP_PAGES {
                return Err(Error::InvalidParameter);
            }
        }
        Ok(())
    }

    fn two(family: DealerPdaFamilyV1, first: &[u8], second: &[u8]) -> Result<Self> {
        let value = Self {
            family,
            count: 2,
            seeds: [
                DealerSeedV1::new(first)?,
                DealerSeedV1::new(second)?,
                DealerSeedV1::EMPTY,
            ],
        };
        value.validate()?;
        Ok(value)
    }

    fn three(family: DealerPdaFamilyV1, first: &[u8], second: &[u8], third: &[u8]) -> Result<Self> {
        let value = Self {
            family,
            count: 3,
            seeds: [
                DealerSeedV1::new(first)?,
                DealerSeedV1::new(second)?,
                DealerSeedV1::new(third)?,
            ],
        };
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_POLICY_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_FACILITY_GENESIS_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(FACILITY_POSITION_BINDING_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_FACILITY_POSITION_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_FACILITY_REPLAY_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_LIVENESS_SCHEDULE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_PDA_DOMAIN_V2.len() <= 32);
const _: () = assert!(DEALER_STATE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_STATE_PDA_DOMAIN_V2.len() <= 32);
const _: () = assert!(LP_PAGE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(LP_PAGE_PDA_DOMAIN_V2.len() <= 32);
const _: () = assert!(DEALER_LEASE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_LEASE_PDA_DOMAIN_V2.len() <= 32);
const _: () = assert!(SETTLEMENT_POT_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SETTLEMENT_POT_PDA_DOMAIN_V2.len() <= 32);
const _: () = assert!(DEALER_EPOCH_BINDING_PDA_DOMAIN_V2.len() <= 32);
const _: () = assert!(DEALER_TERMINAL_ALLOCATION_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_CLAIM_WORK_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_EXIT_TICKET_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_ROOT_TOMBSTONE_PDA_DOMAIN_V2.len() <= 32);
const _: () = assert!(DEALER_COVERED_SELECTION_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_SERIES_OBLIGATION_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(FEE_BUDGET_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(LIVENESS_BUDGET_PDA_DOMAIN_V1.len() <= 32);
