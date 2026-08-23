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
/// Canonical PDA seed prefix for mutable DealerState roots.
pub const DEALER_STATE_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-state-v1";
/// Canonical PDA seed prefix for counted LP pages.
pub const LP_PAGE_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-lp-page-v1";
/// Canonical PDA seed prefix for one-generation leases.
pub const DEALER_LEASE_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-lease-v1";
/// Canonical PDA seed prefix for three-stage settlement pots.
pub const SETTLEMENT_POT_PDA_DOMAIN_V1: &[u8] = b"dc-dealer-pot-v1";
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
    /// Mutable state root addressed by immutable facility identity.
    State = 1,
    /// LP page addressed by facility and page ordinal.
    LpPage = 2,
    /// Lease addressed by facility and consumed generation.
    Lease = 3,
    /// Settlement pot addressed by facility and consumed generation.
    SettlementPot = 4,
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

    /// Mutable state: `[b"dc-dealer-state-v1", facility_id]`.
    pub fn state(facility_id: Id) -> Result<Self> {
        facility_id.validate_live()?;
        Self::two(
            DealerPdaFamilyV1::State,
            DEALER_STATE_PDA_DOMAIN_V1,
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
            DealerPdaFamilyV1::State => (DEALER_STATE_PDA_DOMAIN_V1, 2usize, 0usize),
            DealerPdaFamilyV1::LpPage => (LP_PAGE_PDA_DOMAIN_V1, 3usize, 4usize),
            DealerPdaFamilyV1::Lease => (DEALER_LEASE_PDA_DOMAIN_V1, 3usize, 8usize),
            DealerPdaFamilyV1::SettlementPot => (SETTLEMENT_POT_PDA_DOMAIN_V1, 3usize, 8usize),
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
        if self.family == DealerPdaFamilyV1::LpPage {
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
const _: () = assert!(DEALER_STATE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(LP_PAGE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(DEALER_LEASE_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SETTLEMENT_POT_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(FEE_BUDGET_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(LIVENESS_BUDGET_PDA_DOMAIN_V1.len() <= 32);
