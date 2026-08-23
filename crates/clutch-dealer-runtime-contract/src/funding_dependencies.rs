// SPDX-License-Identifier: AGPL-3.0-or-later

use core::convert::TryFrom;

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerFacilityGenesisV1, DealerPolicyV1, DealerRuntimeActionV1, Error,
    FacilityPositionBindingV1, FixedCodec, Id, Result,
};

/// Number of frozen Dealer action coordinates in one liveness schedule.
pub const DEALER_LIVENESS_ACTION_COUNT_V1: usize = 22;
/// Local semantic-body magic for a Dealer maximum-call liveness schedule.
pub const DEALER_LIVENESS_SCHEDULE_MAGIC_V1: [u8; 8] = *b"DCLSCHV1";
/// Exact local semantic-body version.
pub const DEALER_LIVENESS_SCHEDULE_VERSION_V1: u16 = 1;
/// Exact bytes in one liveness schedule body.
pub const DEALER_LIVENESS_SCHEDULE_BYTES_V1: usize =
    HEADER_BYTES + 8 + (2 * DEALER_LIVENESS_ACTION_COUNT_V1 * 8);

/// Local semantic-body magic for immutable funded-budget dependencies.
pub const DEALER_FUNDED_DEPENDENCIES_MAGIC_V1: [u8; 8] = *b"DCFDDEP1";
/// Exact local semantic-body version.
pub const DEALER_FUNDED_DEPENDENCIES_VERSION_V1: u16 = 1;
/// Exact bytes in one funded-budget dependency body.
pub const DEALER_FUNDED_DEPENDENCIES_BYTES_V1: usize = HEADER_BYTES + (10 * 32) + (2 * 8);

/// Typed content identity for a frozen Dealer liveness schedule.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DealerLivenessScheduleIdV1(Id);

impl DealerLivenessScheduleIdV1 {
    /// Recover a typed identity from authenticated persisted bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Id::from_bytes(bytes))
    }

    /// Return the exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Project to an existing untyped join field.
    pub const fn untyped(self) -> Id {
        self.0
    }
}

/// Exact policy-selected maximum-call liveness schedule.
///
/// Index `i` is exactly `DealerRuntimeActionV1 as u8 == i`; there is no second
/// action enumeration. A zero/zero pair means that policy does not promise a
/// prepaid permissionless reward for that action. A nonzero maximum requires a
/// nonzero per-call lamport amount, and the full checked dot product is the
/// presently funded liveness principal. This crate intentionally selects no
/// counts or prices: the liveness-policy owner must provide and review them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLivenessScheduleV1 {
    /// Bit `i` is one iff action `i` has a nonzero scheduled compartment.
    pub scheduled_action_mask: u32,
    /// Exact maximum paid calls for every Dealer action coordinate.
    pub maximum_calls: [u64; DEALER_LIVENESS_ACTION_COUNT_V1],
    /// Exact prepaid lamports paid for one successful call of each action.
    pub reward_lamports: [u64; DEALER_LIVENESS_ACTION_COUNT_V1],
}

impl DealerLivenessScheduleV1 {
    /// Validate the canonical mask/vector relation and checked principal.
    pub fn validate(&self) -> Result<()> {
        let active_mask = (1u32 << DEALER_LIVENESS_ACTION_COUNT_V1) - 1;
        if self.scheduled_action_mask == 0 || self.scheduled_action_mask & !active_mask != 0 {
            return Err(Error::InvalidParameter);
        }
        let mut index = 0usize;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            let selected = self.scheduled_action_mask & (1u32 << index) != 0;
            if selected != (self.maximum_calls[index] != 0 && self.reward_lamports[index] != 0)
                || (!selected
                    && (self.maximum_calls[index] != 0 || self.reward_lamports[index] != 0))
            {
                return Err(Error::InvalidParameter);
            }
            index += 1;
        }
        self.required_principal_lamports()?;
        Ok(())
    }

    /// Return the exact index owned by one frozen runtime action.
    pub const fn action_index(action: DealerRuntimeActionV1) -> usize {
        action as usize
    }

    /// Exact checked maximum-call dot product in lamports.
    pub fn required_principal_lamports(&self) -> Result<u64> {
        let mut total = 0u64;
        let mut index = 0usize;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            total = total
                .checked_add(
                    self.maximum_calls[index]
                        .checked_mul(self.reward_lamports[index])
                        .ok_or(Error::ArithmeticOverflow)?,
                )
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        if total == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(total)
    }

    /// Canonical typed schedule identity.
    pub fn schedule_id(&self) -> Result<DealerLivenessScheduleIdV1> {
        self.validate()?;
        Ok(DealerLivenessScheduleIdV1(self.content_id(
            crate::DEALER_LIVENESS_SCHEDULE_CONTENT_DOMAIN_V1,
        )?))
    }
}

impl FixedCodec for DealerLivenessScheduleV1 {
    const ENCODED_LEN: usize = DEALER_LIVENESS_SCHEDULE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_LIVENESS_SCHEDULE_MAGIC_V1,
            DEALER_LIVENESS_SCHEDULE_VERSION_V1,
        );
        writer.u32(self.scheduled_action_mask);
        writer.reserved(4);
        let mut index = 0usize;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            writer.u64(self.maximum_calls[index]);
            index += 1;
        }
        index = 0;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            writer.u64(self.reward_lamports[index]);
            index += 1;
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_LIVENESS_SCHEDULE_MAGIC_V1,
            DEALER_LIVENESS_SCHEDULE_VERSION_V1,
        )?;
        let scheduled_action_mask = reader.u32();
        reader.reserved(4)?;
        let mut maximum_calls = [0; DEALER_LIVENESS_ACTION_COUNT_V1];
        let mut index = 0usize;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            maximum_calls[index] = reader.u64();
            index += 1;
        }
        let mut reward_lamports = [0; DEALER_LIVENESS_ACTION_COUNT_V1];
        index = 0;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            reward_lamports[index] = reader.u64();
            index += 1;
        }
        let value = Self {
            scheduled_action_mask,
            maximum_calls,
            reward_lamports,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Number of canonical compartments owned by the external liveness runtime.
pub const DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1: usize = 7;
/// Number of canonical terminal paths owned by the external liveness runtime.
pub const DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1: usize = 4;

/// Canonical external compartment selector.
///
/// Numeric values match the external runtime-liveness ABI. The Dealer owns
/// only the action-to-compartment projection and its fine-grained quote; the
/// external runtime owns physical accounts, balances, calls, rent, receipts,
/// and terminal conservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerLivenessCompartmentV1 {
    /// External source lifecycle; no Dealer action spends this compartment.
    Source = 0,
    /// Candidate binding, lapse, and selected Begin.
    Candidate = 1,
    /// Facility initialization, LP-page creation, and activation.
    Clearing = 2,
    /// Collection, delivery, Finalize, and terminal LP claim delivery.
    Settlement = 3,
    /// Authenticated resolution and redemption work.
    Resolution = 4,
    /// Permissionless unwind and counted retirement work.
    Retirement = 5,
    /// Cancellation, sponsor refund, and pre-collection abort work.
    Recovery = 6,
}

impl DealerLivenessCompartmentV1 {
    /// Canonical external vector index.
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Present-funding origin projected from the external liveness ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerLivenessFundingSourceV1 {
    /// Native lamports debited now from an external signer.
    ExternalSignerNativeLamports = 0,
    /// Native lamports debited now from a precapitalized endowment.
    PrecapitalizedLivenessEndowment = 1,
}

/// Exact action-spending class frozen by Dealer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerActionLivenessClassV1 {
    /// Immutable publication is outside the facility lifecycle.
    OutsideFacilityRuntime,
    /// The initiating owner/signature funds the call; no keeper promise.
    CallerFunded,
    /// The named external compartment pays a successful keeper call.
    Compartment(DealerLivenessCompartmentV1),
    /// Caller-funded unless the schedule explicitly promises Retirement work.
    OptionalQueueMaintenance,
}

/// Return the frozen funding class of one Dealer action.
pub const fn dealer_action_liveness_class_v1(
    action: DealerRuntimeActionV1,
) -> DealerActionLivenessClassV1 {
    use DealerActionLivenessClassV1::{
        CallerFunded, Compartment, OptionalQueueMaintenance, OutsideFacilityRuntime,
    };
    use DealerLivenessCompartmentV1::{
        Candidate, Clearing, Recovery, Resolution, Retirement, Settlement,
    };
    match action {
        DealerRuntimeActionV1::CreatePolicy => OutsideFacilityRuntime,
        DealerRuntimeActionV1::Initialize
        | DealerRuntimeActionV1::CreateLpPage
        | DealerRuntimeActionV1::Activate => Compartment(Clearing),
        DealerRuntimeActionV1::Contribute
        | DealerRuntimeActionV1::WithdrawFunding
        | DealerRuntimeActionV1::SponsorHalt => CallerFunded,
        DealerRuntimeActionV1::CancelFunding
        | DealerRuntimeActionV1::RefundCancelledSponsor
        | DealerRuntimeActionV1::AbortBeforeCollection => Compartment(Recovery),
        DealerRuntimeActionV1::BindEpoch
        | DealerRuntimeActionV1::LapseEpoch
        | DealerRuntimeActionV1::SelectLeaseAndBegin => Compartment(Candidate),
        DealerRuntimeActionV1::Collect
        | DealerRuntimeActionV1::Deliver
        | DealerRuntimeActionV1::FinalizeSettlement
        | DealerRuntimeActionV1::Claim => Compartment(Settlement),
        DealerRuntimeActionV1::QueueExit => OptionalQueueMaintenance,
        DealerRuntimeActionV1::EnterUnwind
        | DealerRuntimeActionV1::TimedClose
        | DealerRuntimeActionV1::Retire => Compartment(Retirement),
        DealerRuntimeActionV1::Resolve => Compartment(Resolution),
    }
}

impl DealerLivenessScheduleV1 {
    /// Exact grouped work capital for one external compartment.
    ///
    /// Source always returns zero because its schedule is owned outside the
    /// Dealer. Optional QueueExit contributes to Retirement only when selected.
    pub fn compartment_work_principal_lamports(
        &self,
        compartment: DealerLivenessCompartmentV1,
    ) -> Result<u64> {
        self.validate()?;
        let mut total = 0u64;
        let mut index = 0usize;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            let action = DealerRuntimeActionV1::from_index(index)?;
            let selected_compartment = match dealer_action_liveness_class_v1(action) {
                DealerActionLivenessClassV1::Compartment(value) => Some(value),
                DealerActionLivenessClassV1::OptionalQueueMaintenance
                    if self.maximum_calls[index] != 0 =>
                {
                    Some(DealerLivenessCompartmentV1::Retirement)
                }
                _ => None,
            };
            if selected_compartment == Some(compartment) {
                total = total
                    .checked_add(
                        self.maximum_calls[index]
                            .checked_mul(self.reward_lamports[index])
                            .ok_or(Error::ArithmeticOverflow)?,
                    )
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        Ok(total)
    }

    /// Sum the exact six Dealer-owned non-Source compartment principals.
    pub fn dealer_runtime_work_principal_lamports(&self) -> Result<u64> {
        self.validate()?;
        let mut total = 0u64;
        let mut index = 1usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            let compartment = match index {
                1 => DealerLivenessCompartmentV1::Candidate,
                2 => DealerLivenessCompartmentV1::Clearing,
                3 => DealerLivenessCompartmentV1::Settlement,
                4 => DealerLivenessCompartmentV1::Resolution,
                5 => DealerLivenessCompartmentV1::Retirement,
                6 => DealerLivenessCompartmentV1::Recovery,
                _ => return Err(Error::InvalidParameter),
            };
            total = total
                .checked_add(self.compartment_work_principal_lamports(compartment)?)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(total)
    }

    /// Validate caller-funded exclusions and mandatory facility groups.
    pub fn validate_for_facility_runtime(&self) -> Result<()> {
        self.validate()?;
        for index in [0usize, 3, 4, 16] {
            if self.maximum_calls[index] != 0 || self.reward_lamports[index] != 0 {
                return Err(Error::InvalidSchedule);
            }
        }
        let mut index = 1usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            let compartment = compartment_from_index(index)?;
            if self.compartment_work_principal_lamports(compartment)? == 0 {
                return Err(Error::InvalidSchedule);
            }
            index += 1;
        }
        Ok(())
    }

    /// Exact aggregate maximum-call count for one external compartment.
    pub fn compartment_maximum_calls(
        &self,
        compartment: DealerLivenessCompartmentV1,
    ) -> Result<u32> {
        self.validate_for_facility_runtime()?;
        let mut total = 0u64;
        let mut index = 0usize;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            let action = DealerRuntimeActionV1::from_index(index)?;
            let class = dealer_action_liveness_class_v1(action);
            let matches = class == DealerActionLivenessClassV1::Compartment(compartment)
                || (class == DealerActionLivenessClassV1::OptionalQueueMaintenance
                    && compartment == DealerLivenessCompartmentV1::Retirement);
            if matches {
                total = total
                    .checked_add(self.maximum_calls[index])
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            index += 1;
        }
        u32::try_from(total).map_err(|_| Error::ArithmeticOverflow)
    }

    /// Largest one-call reward in one external compartment.
    pub fn compartment_maximum_lamports_per_call(
        &self,
        compartment: DealerLivenessCompartmentV1,
    ) -> Result<u64> {
        self.validate_for_facility_runtime()?;
        let mut maximum = 0u64;
        let mut index = 0usize;
        while index < DEALER_LIVENESS_ACTION_COUNT_V1 {
            let action = DealerRuntimeActionV1::from_index(index)?;
            let class = dealer_action_liveness_class_v1(action);
            let matches = class == DealerActionLivenessClassV1::Compartment(compartment)
                || (class == DealerActionLivenessClassV1::OptionalQueueMaintenance
                    && compartment == DealerLivenessCompartmentV1::Retirement);
            if matches {
                maximum = core::cmp::max(maximum, self.reward_lamports[index]);
            }
            index += 1;
        }
        Ok(maximum)
    }
}

fn compartment_from_index(index: usize) -> Result<DealerLivenessCompartmentV1> {
    match index {
        0 => Ok(DealerLivenessCompartmentV1::Source),
        1 => Ok(DealerLivenessCompartmentV1::Candidate),
        2 => Ok(DealerLivenessCompartmentV1::Clearing),
        3 => Ok(DealerLivenessCompartmentV1::Settlement),
        4 => Ok(DealerLivenessCompartmentV1::Resolution),
        5 => Ok(DealerLivenessCompartmentV1::Retirement),
        6 => Ok(DealerLivenessCompartmentV1::Recovery),
        _ => Err(Error::InvalidParameter),
    }
}

/// Adapter-authenticated projection of the separately owned seven-account runtime.
///
/// This value is never persisted as another mutable budget. Its digest binds
/// the immutable Dealer dependency artifact to external runtime policy/bundle
/// facts. The adapter must construct it from authenticated external bodies and
/// revalidate those bodies on every later work or terminal transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerRuntimeLivenessBindingV1 {
    /// Exact external runtime-liveness policy identity.
    pub runtime_policy_id: Id,
    /// Exact Realm identity selected by the runtime policy.
    pub realm_id: Id,
    /// Exact lifecycle identity; Dealer requires its facility identity.
    pub lifecycle_id: Id,
    /// Exact shared neutral sink.
    pub neutral_sink: Id,
    /// Seven distinct physical compartment account/vault identities.
    pub account_ids: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Seven semantic owners; Dealer requires its State authority.
    pub owners: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Seven exact present-funding payers.
    pub payers: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Per-compartment quote-schedule identities owned by liveness policy.
    pub quote_schedule_ids: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Programs admitted to authenticate typed successful-work receipts.
    pub receipt_program_ids: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact admission generations.
    pub generations: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact admitted present-funding classes.
    pub funding_sources:
        [DealerLivenessFundingSourceV1; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact work principal, excluding rent, by compartment.
    pub work_principal_lamports: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact refundable rent principal by compartment.
    pub rent_principal_lamports: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Aggregate maximum successful calls by compartment.
    pub maximum_calls: [u32; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Largest admitted reward for one successful call by compartment.
    pub maximum_lamports_per_call: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Account balance observed before present admission funding.
    pub account_balance_before: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Account balance observed after present admission funding.
    pub account_balance_after: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Four canonical terminal-path call vectors in external path order.
    pub terminal_path_calls: [[u32; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
        DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1],
    /// Exact work-lamport vectors for the same four terminal paths.
    pub terminal_path_work_lamports: [[u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
        DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1],
}

impl DealerRuntimeLivenessBindingV1 {
    /// Validate identities, account uniqueness, funding arithmetic, and bounds.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.runtime_policy_id,
            self.realm_id,
            self.lifecycle_id,
            self.neutral_sink,
        ] {
            identity.validate_live()?;
        }
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            for identity in [
                self.account_ids[index],
                self.owners[index],
                self.payers[index],
                self.quote_schedule_ids[index],
                self.receipt_program_ids[index],
            ] {
                identity.validate_live()?;
            }
            if self.account_ids[index] == self.owners[index]
                || self.account_ids[index] == self.payers[index]
                || self.account_ids[index] == self.neutral_sink
                || self.owners[index] == self.neutral_sink
                || self.payers[index] == self.neutral_sink
                || self.work_principal_lamports[index] == 0
                || self.rent_principal_lamports[index] == 0
                || self.maximum_calls[index] == 0
                || self.maximum_lamports_per_call[index] == 0
                || self.work_principal_lamports[index]
                    > u64::from(self.maximum_calls[index])
                        .checked_mul(self.maximum_lamports_per_call[index])
                        .ok_or(Error::ArithmeticOverflow)?
            {
                return Err(Error::InvalidParameter);
            }
            let required = self.work_principal_lamports[index]
                .checked_add(self.rent_principal_lamports[index])
                .ok_or(Error::ArithmeticOverflow)?;
            if self.account_balance_before[index]
                .checked_add(required)
                .ok_or(Error::ArithmeticOverflow)?
                != self.account_balance_after[index]
            {
                return Err(Error::ConservationFailure);
            }
            let mut prior = 0usize;
            while prior < index {
                if self.account_ids[prior] == self.account_ids[index] {
                    return Err(Error::InvalidParameter);
                }
                prior += 1;
            }
            index += 1;
        }
        let mut path = 0usize;
        while path < DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1 {
            index = 0;
            while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
                if self.terminal_path_calls[path][index] > self.maximum_calls[index]
                    || self.terminal_path_work_lamports[path][index]
                        > self.work_principal_lamports[index]
                    || self.terminal_path_work_lamports[path][index]
                        > u64::from(self.terminal_path_calls[path][index])
                            .checked_mul(self.maximum_lamports_per_call[index])
                            .ok_or(Error::ArithmeticOverflow)?
                {
                    return Err(Error::InvalidSchedule);
                }
                index += 1;
            }
            path += 1;
        }
        Ok(())
    }

    /// Frozen streaming transcript digest; no second persisted balance owner.
    pub fn binding_digest(&self) -> Result<Id> {
        self.validate()?;
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(crate::DEALER_RUNTIME_LIVENESS_BINDING_CONTENT_DOMAIN_V1);
        for identity in [
            self.runtime_policy_id,
            self.realm_id,
            self.lifecycle_id,
            self.neutral_sink,
        ] {
            hasher.update(identity.bytes());
        }
        for identities in [
            self.account_ids,
            self.owners,
            self.payers,
            self.quote_schedule_ids,
            self.receipt_program_ids,
        ] {
            let mut index = 0usize;
            while index < identities.len() {
                hasher.update(identities[index].bytes());
                index += 1;
            }
        }
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            hasher.update(self.generations[index].to_le_bytes());
            hasher.update([self.funding_sources[index] as u8]);
            hasher.update(self.work_principal_lamports[index].to_le_bytes());
            hasher.update(self.rent_principal_lamports[index].to_le_bytes());
            hasher.update(self.maximum_calls[index].to_le_bytes());
            hasher.update(self.maximum_lamports_per_call[index].to_le_bytes());
            hasher.update(self.account_balance_before[index].to_le_bytes());
            hasher.update(self.account_balance_after[index].to_le_bytes());
            index += 1;
        }
        let mut path = 0usize;
        while path < DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1 {
            index = 0;
            while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
                hasher.update(self.terminal_path_calls[path][index].to_le_bytes());
                hasher.update(self.terminal_path_work_lamports[path][index].to_le_bytes());
                index += 1;
            }
            path += 1;
        }
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

/// Typed successful Dealer work receipt admitted by the external runtime.
///
/// The external runtime owns monotone ordinals, remaining work, keeper/refund
/// transfers, and account writes. This transcript makes the Dealer action and
/// its exact per-action ceiling part of the semantic receipt identity rather
/// than accepting an opaque caller-selected digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActionLivenessAuthorizationV1 {
    /// Exact Dealer action performed by the successful atomic transaction.
    pub action: DealerRuntimeActionV1,
    /// Exact external compartment consuming the call.
    pub compartment: DealerLivenessCompartmentV1,
    /// Exact external runtime compartment account.
    pub runtime_account_id: Id,
    /// Exact semantic owner of that compartment.
    pub owner: Id,
    /// Exact facility lifecycle identity.
    pub lifecycle_id: Id,
    /// Exact Dealer fine-grained quote schedule identity.
    pub quote_schedule_id: Id,
    /// Exact typed receipt account.
    pub receipt_account_id: Id,
    /// Exact program owner admitted to authenticate the receipt.
    pub receipt_program_id: Id,
    /// Canonical semantic digest of this receipt transcript.
    pub receipt_semantic_id: Id,
    /// Exact runtime compartment generation.
    pub generation: u64,
    /// Exact Dealer economic generation at which the action succeeds.
    pub facility_generation: u64,
    /// Exact next monotone call ordinal checked by the external runtime.
    pub call_ordinal: u32,
    /// Exact Dealer action reward ceiling consumed by the external runtime.
    pub call_ceiling_lamports: u64,
}

impl DealerActionLivenessAuthorizationV1 {
    /// Validate this action receipt against the immutable schedule and runtime projection.
    pub fn validate_against(
        &self,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
    ) -> Result<()> {
        schedule.validate_for_facility_runtime()?;
        runtime.validate()?;
        for identity in [
            self.runtime_account_id,
            self.owner,
            self.lifecycle_id,
            self.quote_schedule_id,
            self.receipt_account_id,
            self.receipt_program_id,
            self.receipt_semantic_id,
        ] {
            identity.validate_live()?;
        }
        let expected_compartment = match dealer_action_liveness_class_v1(self.action) {
            DealerActionLivenessClassV1::Compartment(value) => value,
            DealerActionLivenessClassV1::OptionalQueueMaintenance => {
                DealerLivenessCompartmentV1::Retirement
            }
            _ => return Err(Error::InvalidSchedule),
        };
        let action_index = self.action as usize;
        let compartment_index = expected_compartment.index();
        if self.compartment != expected_compartment
            || schedule.maximum_calls[action_index] == 0
            || self.call_ceiling_lamports != schedule.reward_lamports[action_index]
            || self.call_ordinal == 0
            || self.runtime_account_id != runtime.account_ids[compartment_index]
            || self.owner != runtime.owners[compartment_index]
            || self.lifecycle_id != runtime.lifecycle_id
            || self.quote_schedule_id != runtime.quote_schedule_ids[compartment_index]
            || self.receipt_program_id != runtime.receipt_program_ids[compartment_index]
            || self.generation != runtime.generations[compartment_index]
            || self.receipt_account_id == self.runtime_account_id
            || self.receipt_account_id == self.owner
            || self.receipt_account_id == runtime.neutral_sink
            || self.receipt_semantic_id != self.canonical_receipt_semantic_id()?
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical action-bound receipt semantic identity.
    pub fn canonical_receipt_semantic_id(&self) -> Result<Id> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(crate::DEALER_ACTION_LIVENESS_RECEIPT_CONTENT_DOMAIN_V1);
        hasher.update([self.action as u8, self.compartment as u8]);
        for identity in [
            self.runtime_account_id,
            self.owner,
            self.lifecycle_id,
            self.quote_schedule_id,
            self.receipt_account_id,
            self.receipt_program_id,
        ] {
            identity.validate_live()?;
            hasher.update(identity.bytes());
        }
        hasher.update(self.generation.to_le_bytes());
        hasher.update(self.facility_generation.to_le_bytes());
        hasher.update(self.call_ordinal.to_le_bytes());
        hasher.update(self.call_ceiling_lamports.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }
}

/// Immutable typed dependency interface for external liveness and fee policy.
///
/// Dealer's fine-grained schedule prices successful calls, while the external
/// seven-account runtime solely owns native-lamport custody and conservation.
/// Fee settlement is separately owner-netted from ordinary Positions under
/// the bound fee policy; this type deliberately owns no fee vault or principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFundedBudgetDependenciesV1 {
    /// Exact Dealer policy identity.
    pub policy_id: Id,
    /// Canonical facility identity.
    pub facility_id: Id,
    /// Exact fine-grained Dealer schedule identity.
    pub liveness_schedule_id: Id,
    /// Exact external runtime-liveness policy identity.
    pub runtime_liveness_policy_id: Id,
    /// Digest of the authenticated external seven-account binding projection.
    pub runtime_liveness_binding_digest: Id,
    /// Exact immutable fee-policy identity.
    pub fee_policy_id: Id,
    /// Exact collateral mint of the facility and owner-netted fee plane.
    pub collateral_mint: Id,
    /// Exact admitted token program.
    pub token_program: Id,
    /// Exact DealerState PDA owning facility and liveness semantic authorities.
    pub asset_vault_authority_account_id: Id,
    /// One policy sink for donations and non-refund surplus.
    pub neutral_sink: Id,
    /// Parent generation at which all dependencies were admitted.
    pub counted_generation: u64,
    /// Exact Dealer-scheduled work principal across six non-Source compartments.
    pub dealer_liveness_work_principal_lamports: u64,
}

impl DealerFundedBudgetDependenciesV1 {
    /// Validate live IDs and nonzero present liveness principal.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.policy_id,
            self.facility_id,
            self.liveness_schedule_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.fee_policy_id,
            self.collateral_mint,
            self.token_program,
            self.asset_vault_authority_account_id,
            self.neutral_sink,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index].validate_live()?;
            index += 1;
        }
        if self.counted_generation != 0
            || self.dealer_liveness_work_principal_lamports == 0
            || self.asset_vault_authority_account_id == self.neutral_sink
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Join the exact facility, fee plane, Dealer quote, and liveness runtime.
    pub fn validate_bindings(
        &self,
        genesis: &DealerFacilityGenesisV1,
        binding: &FacilityPositionBindingV1,
        policy: &DealerPolicyV1,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
    ) -> Result<()> {
        self.validate()?;
        schedule.validate_for_facility_runtime()?;
        runtime.validate()?;
        let facility_id = genesis.facility_id_for_policy(policy)?;
        binding.binding_id_for(genesis, policy)?;
        let schedule_id = schedule.schedule_id()?;
        if self.policy_id != genesis.policy_id
            || self.facility_id != facility_id.untyped()
            || self.facility_id != binding.facility_id
            || self.liveness_schedule_id != schedule_id.untyped()
            || self.liveness_schedule_id != policy.liveness_policy_id
            || self.runtime_liveness_policy_id != runtime.runtime_policy_id
            || self.runtime_liveness_binding_digest != runtime.binding_digest()?
            || runtime.realm_id != policy.realm_id
            || runtime.lifecycle_id != self.facility_id
            || runtime.neutral_sink != policy.neutral_sink
            || self.fee_policy_id != policy.fee_policy_id
            || self.collateral_mint != policy.collateral_mint
            || self.token_program != policy.token_program
            || self.asset_vault_authority_account_id != binding.dealer_state_account_id
            || self.neutral_sink != policy.neutral_sink
            || self.dealer_liveness_work_principal_lamports
                != schedule.dealer_runtime_work_principal_lamports()?
            || runtime.quote_schedule_ids[DealerLivenessCompartmentV1::Source.index()]
                == self.liveness_schedule_id
        {
            return Err(Error::MismatchedBinding);
        }
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            if (index != DealerLivenessCompartmentV1::Source.index()
                && runtime.owners[index] != binding.dealer_state_account_id)
                || runtime.generations[index] != self.counted_generation
                || runtime.account_ids[index] == self.asset_vault_authority_account_id
                || runtime.account_ids[index] == binding.facility_position_account_id
                || runtime.account_ids[index] == binding.facility_replay_account_id
            {
                return Err(Error::MismatchedBinding);
            }
            if index != DealerLivenessCompartmentV1::Source.index() {
                let compartment = compartment_from_index(index)?;
                if runtime.quote_schedule_ids[index] != self.liveness_schedule_id
                    || runtime.work_principal_lamports[index]
                        != schedule.compartment_work_principal_lamports(compartment)?
                    || runtime.maximum_calls[index]
                        != schedule.compartment_maximum_calls(compartment)?
                    || runtime.maximum_lamports_per_call[index]
                        != schedule.compartment_maximum_lamports_per_call(compartment)?
                {
                    return Err(Error::MismatchedBinding);
                }
            }
            index += 1;
        }
        Ok(())
    }

    /// Join every immutable and presently funded dependency before activation.
    pub fn validate_for_activation(
        &self,
        genesis: &DealerFacilityGenesisV1,
        binding: &FacilityPositionBindingV1,
        policy: &DealerPolicyV1,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
    ) -> Result<()> {
        self.validate_bindings(genesis, binding, policy, schedule, runtime)
    }

    /// Canonical immutable dependency identity.
    pub fn dependency_id(&self) -> Result<Id> {
        self.content_id(crate::DEALER_FUNDED_DEPENDENCIES_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for DealerFundedBudgetDependenciesV1 {
    const ENCODED_LEN: usize = DEALER_FUNDED_DEPENDENCIES_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_FUNDED_DEPENDENCIES_MAGIC_V1,
            DEALER_FUNDED_DEPENDENCIES_VERSION_V1,
        );
        for identity in [
            self.policy_id,
            self.facility_id,
            self.liveness_schedule_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.fee_policy_id,
            self.collateral_mint,
            self.token_program,
            self.asset_vault_authority_account_id,
            self.neutral_sink,
        ] {
            writer.id(identity);
        }
        writer.u64(self.counted_generation);
        writer.u64(self.dealer_liveness_work_principal_lamports);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_FUNDED_DEPENDENCIES_MAGIC_V1,
            DEALER_FUNDED_DEPENDENCIES_VERSION_V1,
        )?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            liveness_schedule_id: reader.id(),
            runtime_liveness_policy_id: reader.id(),
            runtime_liveness_binding_digest: reader.id(),
            fee_policy_id: reader.id(),
            collateral_mint: reader.id(),
            token_program: reader.id(),
            asset_vault_authority_account_id: reader.id(),
            neutral_sink: reader.id(),
            counted_generation: reader.u64(),
            dealer_liveness_work_principal_lamports: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_LIVENESS_ACTION_COUNT_V1 == 22);
const _: () = assert!(DEALER_LIVENESS_SCHEDULE_BYTES_V1 == 372);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_BYTES_V1 == 348);
const _: () = assert!(DEALER_LIVENESS_SCHEDULE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
