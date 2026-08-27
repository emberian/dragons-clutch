// SPDX-License-Identifier: AGPL-3.0-or-later

use core::convert::TryFrom;

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerActionReceiptV1, DealerFacilityGenesisV1, DealerPhaseV2, DealerPolicyV1,
    DealerRuntimeActionV1, DealerStateV2, DeletableRentOwnerV1, Error, FacilityPositionBindingV1,
    FixedCodec, Id, Result, DELETABLE_RENT_OWNER_BYTES,
};

/// Derive the canonical immutable identity of a generic runtime-liveness
/// policy used by Dealer. Every exact codec byte participates except the
/// embedded policy ID itself, whose fixed region is replaced by zero bytes.
pub fn dealer_runtime_liveness_policy_id_v1(
    policy: clutch_liveness::runtime_v1::RuntimeLivenessPolicyV1,
) -> Result<Id> {
    use sha2::{Digest, Sha256};

    let mut bytes = [0u8; clutch_liveness::runtime_v1::RUNTIME_LIVENESS_POLICY_BYTES_V1];
    policy
        .encode(&mut bytes)
        .map_err(|_| Error::MismatchedBinding)?;
    bytes[HEADER_BYTES..HEADER_BYTES + 32].fill(0);
    let mut hasher = Sha256::new();
    hasher.update(crate::DEALER_RUNTIME_LIVENESS_POLICY_CONTENT_DOMAIN_V1);
    hasher.update(bytes);
    let identity = Id::from_bytes(hasher.finalize().into());
    identity.validate_live()?;
    if policy.policy_id.bytes() != identity.bytes() {
        return Err(Error::MismatchedBinding);
    }
    Ok(identity)
}

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
pub const DEALER_FUNDED_DEPENDENCIES_BYTES_V1: usize = HEADER_BYTES + (12 * 32) + (2 * 8);
/// Local semantic-body magic for the rent-owned counted successor.
pub const DEALER_FUNDED_DEPENDENCIES_MAGIC_V2: [u8; 8] = *b"DCFDDEP2";
/// Exact local semantic-body version for the counted successor.
pub const DEALER_FUNDED_DEPENDENCIES_VERSION_V2: u16 = 2;
/// Exact bytes: outer header, frozen V1 dependency payload, canonical Position
/// V3 purpose-binding identity, the counted Initialize receipt account and
/// semantic identities, and one deletable-rent owner. The nested V1 header is
/// intentional provenance.
pub const DEALER_FUNDED_DEPENDENCIES_BYTES_V2: usize =
    HEADER_BYTES + DEALER_FUNDED_DEPENDENCIES_BYTES_V1 + (3 * 32) + DELETABLE_RENT_OWNER_BYTES;

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
    pub(crate) runtime_policy_id: Id,
    /// Exact Realm identity selected by the runtime policy.
    pub(crate) realm_id: Id,
    /// Exact lifecycle identity; Dealer requires its facility identity.
    pub(crate) lifecycle_id: Id,
    /// Exact shared neutral sink.
    pub(crate) neutral_sink: Id,
    /// Seven distinct physical compartment account/vault identities.
    pub(crate) account_ids: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Seven semantic owners; Dealer requires its State authority.
    pub(crate) owners: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Seven exact present-funding payers.
    pub(crate) payers: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Per-compartment quote-schedule identities owned by liveness policy.
    pub(crate) quote_schedule_ids: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Programs admitted to authenticate typed successful-work receipts.
    pub(crate) receipt_program_ids: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact admission generations.
    pub(crate) generations: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact admitted present-funding classes.
    pub(crate) funding_sources:
        [DealerLivenessFundingSourceV1; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact work principal, excluding rent, by compartment.
    pub(crate) work_principal_lamports: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact refundable rent principal by compartment.
    pub(crate) rent_principal_lamports: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Aggregate maximum successful calls by compartment.
    pub(crate) maximum_calls: [u32; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Largest admitted reward for one successful call by compartment.
    pub(crate) maximum_lamports_per_call: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Four canonical terminal-path call vectors in external path order.
    pub(crate) terminal_path_calls: [[u32; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
        DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1],
    /// Exact work-lamport vectors for the same four terminal paths.
    pub(crate) terminal_path_work_lamports: [[u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
        DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1],
}

impl DealerRuntimeLivenessBindingV1 {
    /// Construct the Dealer's immutable quote projection from the canonical
    /// `clutch-liveness` policy and its seven persisted compartment bodies.
    ///
    /// This copies admission facts only; it does not create a second mutable
    /// balance/call owner. Advanced compartments reconstruct the same binding
    /// because capitalized work/rent principal, maxima, identities, and policy
    /// terminal vectors are immutable. Runtime-owned hostile donation totals
    /// are deliberately excluded because they may increase after admission.
    pub fn from_canonical(
        policy: &clutch_liveness::runtime_v1::RuntimeLivenessPolicyV1,
        compartments: &[clutch_liveness::runtime_v1::RuntimeCompartmentV1;
             DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    ) -> Result<Self> {
        policy.validate().map_err(|_| Error::MismatchedBinding)?;
        dealer_runtime_liveness_policy_id_v1(*policy)?;
        let mut value = Self {
            runtime_policy_id: from_liveness_id(policy.policy_id),
            realm_id: from_liveness_id(policy.realm_id),
            lifecycle_id: from_liveness_id(compartments[0].identity.lifecycle_id),
            neutral_sink: from_liveness_id(policy.neutral_sink),
            account_ids: [Id::ZERO; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            owners: [Id::ZERO; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            payers: [Id::ZERO; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            quote_schedule_ids: [Id::ZERO; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            receipt_program_ids: [Id::ZERO; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            generations: [0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            funding_sources: [DealerLivenessFundingSourceV1::ExternalSignerNativeLamports;
                DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            work_principal_lamports: [0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            rent_principal_lamports: [0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            maximum_calls: [0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            maximum_lamports_per_call: [0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
            terminal_path_calls: [[0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
                DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1],
            terminal_path_work_lamports: [[0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
                DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1],
        };
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            let compartment = compartments[index];
            compartment
                .validate()
                .map_err(|_| Error::MismatchedBinding)?;
            if compartment.kind.index() != index
                || compartment.identity.policy_id != policy.policy_id
                || compartment.identity.lifecycle_id != compartments[0].identity.lifecycle_id
                || compartment.identity.neutral_sink != policy.neutral_sink
            {
                return Err(Error::MismatchedBinding);
            }
            value.account_ids[index] = from_liveness_id(compartment.identity.account_id);
            value.owners[index] = from_liveness_id(compartment.identity.owner);
            value.payers[index] = from_liveness_id(compartment.identity.payer);
            value.quote_schedule_ids[index] = from_liveness_id(compartment.quote_schedule_id);
            value.receipt_program_ids[index] = from_liveness_id(compartment.receipt_program_id);
            value.generations[index] = compartment.identity.generation;
            value.funding_sources[index] = match compartment.funding_source {
                clutch_liveness::runtime_v1::PresentFundingSourceV1::ExternalSignerNativeLamports => {
                    DealerLivenessFundingSourceV1::ExternalSignerNativeLamports
                }
                clutch_liveness::runtime_v1::PresentFundingSourceV1::PrecapitalizedLivenessEndowment => {
                    DealerLivenessFundingSourceV1::PrecapitalizedLivenessEndowment
                }
            };
            value.work_principal_lamports[index] = compartment.capitalized_work_lamports;
            value.rent_principal_lamports[index] = compartment.rent_principal_lamports;
            value.maximum_calls[index] = compartment.maximum_calls;
            value.maximum_lamports_per_call[index] = compartment.maximum_lamports_per_call;
            index += 1;
        }
        let mut path = 0usize;
        while path < DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1 {
            value.terminal_path_calls[path] = policy.terminal_paths[path].calls;
            value.terminal_path_work_lamports[path] = policy.terminal_paths[path].work_lamports;
            path += 1;
        }
        value.validate()?;
        Ok(value)
    }

    /// Exact external runtime policy identity.
    pub const fn runtime_policy_id(self) -> Id {
        self.runtime_policy_id
    }

    /// Realm selected by the external runtime policy.
    pub const fn realm_id(self) -> Id {
        self.realm_id
    }

    /// Exact facility lifecycle identity.
    pub const fn lifecycle_id(self) -> Id {
        self.lifecycle_id
    }

    /// Canonical neutral lamport sink.
    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    /// Exact physical account for one canonical compartment.
    pub const fn account_id(self, compartment: DealerLivenessCompartmentV1) -> Id {
        self.account_ids[compartment.index()]
    }

    /// Exact semantic owner for one canonical compartment.
    pub const fn owner(self, compartment: DealerLivenessCompartmentV1) -> Id {
        self.owners[compartment.index()]
    }

    /// Exact present-funding payer for one canonical compartment.
    pub const fn payer(self, compartment: DealerLivenessCompartmentV1) -> Id {
        self.payers[compartment.index()]
    }

    /// Exact quote schedule for one canonical compartment.
    pub const fn quote_schedule_id(self, compartment: DealerLivenessCompartmentV1) -> Id {
        self.quote_schedule_ids[compartment.index()]
    }

    /// Exact receipt-authentication program for one canonical compartment.
    pub const fn receipt_program_id(self, compartment: DealerLivenessCompartmentV1) -> Id {
        self.receipt_program_ids[compartment.index()]
    }

    /// Immutable admission generation for one canonical compartment.
    pub const fn generation(self, compartment: DealerLivenessCompartmentV1) -> u64 {
        self.generations[compartment.index()]
    }

    /// Exact present-funding class for one canonical compartment.
    pub const fn funding_source(
        self,
        compartment: DealerLivenessCompartmentV1,
    ) -> DealerLivenessFundingSourceV1 {
        self.funding_sources[compartment.index()]
    }

    /// Work principal funded now for one canonical compartment.
    pub const fn work_principal_lamports(self, compartment: DealerLivenessCompartmentV1) -> u64 {
        self.work_principal_lamports[compartment.index()]
    }

    /// Refundable rent principal funded now for one canonical compartment.
    pub const fn rent_principal_lamports(self, compartment: DealerLivenessCompartmentV1) -> u64 {
        self.rent_principal_lamports[compartment.index()]
    }

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
    /// Construct from the canonical external compartment, transition intent,
    /// and adapter-authenticated receipt observation now on main.
    pub fn from_canonical(
        dealer_action: DealerRuntimeActionV1,
        facility_generation: u64,
        compartment: &clutch_liveness::runtime_v1::RuntimeCompartmentV1,
        intent: &clutch_liveness::runtime_adapter_v1::RuntimeTransitionIntentV1,
        receipt: &clutch_liveness::runtime_adapter_v1::RuntimeReceiptObservationV1,
    ) -> Result<Self> {
        compartment
            .validate()
            .map_err(|_| Error::MismatchedBinding)?;
        intent.validate().map_err(|_| Error::MismatchedBinding)?;
        if intent.action
            != clutch_liveness::runtime_adapter_v1::RuntimeTransitionActionV1::SpendWork
            || receipt.receipt_kind
                != clutch_liveness::runtime_adapter_v1::RuntimeReceiptKindV1::WorkCompleted
            || intent.kind != compartment.kind
            || receipt.compartment_kind != compartment.kind
            || intent.policy_id != compartment.identity.policy_id
            || intent.lifecycle_id != compartment.identity.lifecycle_id
            || intent.account_id != compartment.identity.account_id
            || intent.semantic_owner != compartment.identity.owner
            || intent.quote_schedule_id != compartment.quote_schedule_id
            || intent.receipt_id != receipt.receipt_id
            || receipt.receipt_account_owner_program_id != compartment.receipt_program_id
            || receipt.semantic_owner != compartment.identity.owner
            || receipt.lifecycle_id != compartment.identity.lifecycle_id
            || receipt.quote_schedule_id != compartment.quote_schedule_id
            || receipt.generation != compartment.identity.generation
            || receipt.call_ordinal != intent.call_ordinal
            || receipt.call_ceiling_lamports != intent.call_ceiling_lamports
        {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            action: dealer_action,
            compartment: dealer_compartment_from_canonical(compartment.kind),
            runtime_account_id: from_liveness_id(compartment.identity.account_id),
            owner: from_liveness_id(compartment.identity.owner),
            lifecycle_id: from_liveness_id(compartment.identity.lifecycle_id),
            quote_schedule_id: from_liveness_id(compartment.quote_schedule_id),
            receipt_account_id: from_liveness_id(receipt.receipt_account_id),
            receipt_program_id: from_liveness_id(receipt.receipt_account_owner_program_id),
            receipt_semantic_id: from_liveness_id(receipt.receipt_id),
            generation: compartment.identity.generation,
            facility_generation,
            call_ordinal: intent.call_ordinal,
            call_ceiling_lamports: intent.call_ceiling_lamports,
        };
        if value.canonical_receipt_semantic_id()? != value.receipt_semantic_id {
            return Err(Error::MismatchedBinding);
        }
        Ok(value)
    }

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

const fn from_liveness_id(value: clutch_liveness::Id) -> Id {
    Id::from_bytes(value.bytes())
}

const fn dealer_compartment_from_canonical(
    value: clutch_liveness::runtime_v1::RuntimeCompartmentKindV1,
) -> DealerLivenessCompartmentV1 {
    match value {
        clutch_liveness::runtime_v1::RuntimeCompartmentKindV1::Source => {
            DealerLivenessCompartmentV1::Source
        }
        clutch_liveness::runtime_v1::RuntimeCompartmentKindV1::Candidate => {
            DealerLivenessCompartmentV1::Candidate
        }
        clutch_liveness::runtime_v1::RuntimeCompartmentKindV1::Clearing => {
            DealerLivenessCompartmentV1::Clearing
        }
        clutch_liveness::runtime_v1::RuntimeCompartmentKindV1::Settlement => {
            DealerLivenessCompartmentV1::Settlement
        }
        clutch_liveness::runtime_v1::RuntimeCompartmentKindV1::Resolution => {
            DealerLivenessCompartmentV1::Resolution
        }
        clutch_liveness::runtime_v1::RuntimeCompartmentKindV1::Retirement => {
            DealerLivenessCompartmentV1::Retirement
        }
        clutch_liveness::runtime_v1::RuntimeCompartmentKindV1::Recovery => {
            DealerLivenessCompartmentV1::Recovery
        }
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
    /// Exact deployed program owning the generic policy and compartments.
    pub runtime_liveness_program_id: Id,
    /// Physical immutable generic runtime-policy account.
    pub runtime_liveness_policy_account_id: Id,
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
            self.runtime_liveness_program_id,
            self.runtime_liveness_policy_account_id,
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
            || self.runtime_liveness_program_id == self.runtime_liveness_policy_account_id
            || self.runtime_liveness_policy_account_id == self.asset_vault_authority_account_id
            || self.runtime_liveness_policy_account_id == self.neutral_sink
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
                || runtime.account_ids[index] == self.runtime_liveness_policy_account_id
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
            self.runtime_liveness_program_id,
            self.runtime_liveness_policy_account_id,
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
            runtime_liveness_program_id: reader.id(),
            runtime_liveness_policy_account_id: reader.id(),
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

/// Counted V2 dependency child with an explicit, independently refundable rent owner.
///
/// `bindings` is the frozen immutable V1 dependency transcript, not a V1
/// account authority. Only this V2 body is admitted by `DealerStateV2`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFundedDependenciesV2 {
    /// Immutable external-liveness and owner-netted fee-policy joins.
    pub bindings: DealerFundedBudgetDependenciesV1,
    /// Immutable purpose binding for the canonical shared Position V3.
    pub facility_position_binding_id: Id,
    /// Exact physical Initialize receipt retained until this child retires.
    pub initialize_receipt_account_id: Id,
    /// Exact semantic Initialize receipt retained until this child retires.
    pub initialize_receipt_semantic_id: Id,
    /// Exact rent principal owner and donation disposition for this child.
    pub rent: DeletableRentOwnerV1,
}

impl DealerFundedDependenciesV2 {
    /// Validate the nested immutable joins and V2 rent ownership.
    pub fn validate(&self) -> Result<()> {
        self.bindings.validate()?;
        self.facility_position_binding_id.validate_live()?;
        self.initialize_receipt_account_id.validate_live()?;
        self.initialize_receipt_semantic_id.validate_live()?;
        self.rent.validate()?;
        if self.rent.neutral_sink != self.bindings.neutral_sink
            || self.rent.payer == self.bindings.asset_vault_authority_account_id
            || self.initialize_receipt_account_id == self.bindings.asset_vault_authority_account_id
            || self.initialize_receipt_account_id
                == self.bindings.runtime_liveness_policy_account_id
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Validate every immutable facility and external-runtime join.
    pub fn validate_bindings(
        &self,
        genesis: &DealerFacilityGenesisV1,
        binding: &FacilityPositionBindingV1,
        policy: &DealerPolicyV1,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
    ) -> Result<()> {
        self.validate()?;
        self.bindings
            .validate_bindings(genesis, binding, policy, schedule, runtime)?;
        if self.facility_position_binding_id != binding.binding_id_for(genesis, policy)?.untyped()
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Validate the same immutable dependency plane against canonical Position V3.
    pub fn validate_bindings_v3(
        &self,
        genesis: &DealerFacilityGenesisV1,
        binding: &crate::FacilityPositionBindingV2,
        policy: &DealerPolicyV1,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
    ) -> Result<()> {
        self.validate()?;
        schedule.validate_for_facility_runtime()?;
        runtime.validate()?;
        let facility_id = genesis.facility_id_for_policy(policy)?.untyped();
        let binding_id = binding.binding_id_for(genesis, policy)?;
        if self.bindings.policy_id != policy.policy_id()?
            || self.bindings.facility_id != facility_id
            || binding.facility_id != facility_id
            || self.facility_position_binding_id != binding_id
            || self.bindings.liveness_schedule_id != schedule.schedule_id()?.untyped()
            || self.bindings.liveness_schedule_id != policy.liveness_policy_id
            || self.bindings.runtime_liveness_policy_id != runtime.runtime_policy_id
            || self.bindings.runtime_liveness_binding_digest != runtime.binding_digest()?
            || runtime.realm_id != policy.realm_id
            || runtime.lifecycle_id != facility_id
            || runtime.neutral_sink != policy.neutral_sink
            || self.bindings.fee_policy_id != policy.fee_policy_id
            || self.bindings.collateral_mint != policy.collateral_mint
            || self.bindings.token_program != policy.token_program
            || self.bindings.asset_vault_authority_account_id != binding.dealer_state_account_id
            || self.bindings.neutral_sink != policy.neutral_sink
            || self.bindings.dealer_liveness_work_principal_lamports
                != schedule.dealer_runtime_work_principal_lamports()?
            || runtime.quote_schedule_ids[DealerLivenessCompartmentV1::Source.index()]
                == self.bindings.liveness_schedule_id
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            if (index != DealerLivenessCompartmentV1::Source.index()
                && runtime.owners[index] != binding.dealer_state_account_id)
                || runtime.generations[index] != self.bindings.counted_generation
                || runtime.account_ids[index] == self.bindings.runtime_liveness_policy_account_id
                || runtime.account_ids[index] == self.bindings.asset_vault_authority_account_id
            {
                return Err(Error::MismatchedBinding);
            }
            if index != DealerLivenessCompartmentV1::Source.index() {
                let compartment = compartment_from_index(index)?;
                if runtime.quote_schedule_ids[index] != self.bindings.liveness_schedule_id
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

    /// Validate the funded plane after initialization, when the content-derived
    /// facility ID and Position-purpose binding are already persisted by State.
    /// The one-time Genesis nonce is intentionally not a live-action input.
    pub fn validate_live_bindings_v4(
        &self,
        binding: &crate::FacilityPositionBindingV2,
        policy: &DealerPolicyV1,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
    ) -> Result<()> {
        self.validate()?;
        binding.validate()?;
        policy.validate()?;
        schedule.validate_for_facility_runtime()?;
        runtime.validate()?;
        let facility_id = binding.facility_id;
        let binding_id = binding.binding_id()?;
        if self.bindings.policy_id != policy.policy_id()?
            || self.bindings.facility_id != facility_id
            || binding.policy_id != self.bindings.policy_id
            || binding.market_instance_v2_id != policy.market_instance_v2_id
            || self.facility_position_binding_id != binding_id
            || self.bindings.liveness_schedule_id != schedule.schedule_id()?.untyped()
            || self.bindings.liveness_schedule_id != policy.liveness_policy_id
            || self.bindings.runtime_liveness_policy_id != runtime.runtime_policy_id
            || self.bindings.runtime_liveness_binding_digest != runtime.binding_digest()?
            || runtime.realm_id != policy.realm_id
            || runtime.lifecycle_id != facility_id
            || runtime.neutral_sink != policy.neutral_sink
            || self.bindings.fee_policy_id != policy.fee_policy_id
            || self.bindings.collateral_mint != policy.collateral_mint
            || self.bindings.token_program != policy.token_program
            || self.bindings.asset_vault_authority_account_id != binding.dealer_state_account_id
            || self.bindings.neutral_sink != policy.neutral_sink
            || self.bindings.dealer_liveness_work_principal_lamports
                != schedule.dealer_runtime_work_principal_lamports()?
            || runtime.quote_schedule_ids[DealerLivenessCompartmentV1::Source.index()]
                == self.bindings.liveness_schedule_id
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            if (index != DealerLivenessCompartmentV1::Source.index()
                && runtime.owners[index] != binding.dealer_state_account_id)
                || runtime.generations[index] != self.bindings.counted_generation
                || runtime.account_ids[index] == self.bindings.runtime_liveness_policy_account_id
                || runtime.account_ids[index] == self.bindings.asset_vault_authority_account_id
            {
                return Err(Error::MismatchedBinding);
            }
            if index != DealerLivenessCompartmentV1::Source.index() {
                let compartment = compartment_from_index(index)?;
                if runtime.quote_schedule_ids[index] != self.bindings.liveness_schedule_id
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

    /// Canonical semantic identity retained by State V2 after child deletion.
    pub fn dependency_id(&self) -> Result<Id> {
        self.content_id(crate::DEALER_FUNDED_DEPENDENCIES_CONTENT_DOMAIN_V2)
    }

    /// Canonical counted edge at admission.
    pub fn counted_child(&self) -> Result<crate::CountedDealerChildV2> {
        self.validate()?;
        Ok(crate::CountedDealerChildV2 {
            facility_id: self.bindings.facility_id,
            facility_position_binding_id: self.facility_position_binding_id,
            kind: crate::DealerChildKindV2::FundedDependencies,
            counted_generation: self.bindings.counted_generation,
        })
    }
}

impl FixedCodec for DealerFundedDependenciesV2 {
    const ENCODED_LEN: usize = DEALER_FUNDED_DEPENDENCIES_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut nested = [0u8; DEALER_FUNDED_DEPENDENCIES_BYTES_V1];
        self.bindings.encode_into(&mut nested)?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_FUNDED_DEPENDENCIES_MAGIC_V2,
            DEALER_FUNDED_DEPENDENCIES_VERSION_V2,
        );
        writer.bytes(&nested);
        writer.id(self.facility_position_binding_id);
        writer.id(self.initialize_receipt_account_id);
        writer.id(self.initialize_receipt_semantic_id);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_FUNDED_DEPENDENCIES_MAGIC_V2,
            DEALER_FUNDED_DEPENDENCIES_VERSION_V2,
        )?;
        let nested = reader.bytes::<DEALER_FUNDED_DEPENDENCIES_BYTES_V1>();
        let value = Self {
            bindings: DealerFundedBudgetDependenciesV1::decode(&nested)?,
            facility_position_binding_id: reader.id(),
            initialize_receipt_account_id: reader.id(),
            initialize_receipt_semantic_id: reader.id(),
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Authenticated terminal projection of the external seven-account runtime.
///
/// External runtime remains the sole balance/call/receipt owner. This value is
/// consumed only to prove that every compartment selected by the original
/// binding reached one canonical terminal path before the Dealer dependency
/// child releases its own rent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerRuntimeLivenessTerminalV1 {
    /// Original binding digest committed by the dependency child.
    runtime_binding_digest: Id,
    /// Exact facility lifecycle identity.
    lifecycle_id: Id,
    /// Exact State authority used by the six Dealer-owned compartments.
    state_authority_account_id: Id,
    /// Selected external terminal path, in the frozen four-path order.
    terminal_path_index: u8,
    /// Exact final successful-call counters.
    completed_calls: [u32; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Exact final consumed work lamports.
    completed_work_lamports: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Typed terminal receipt semantic identities, one per compartment.
    terminal_receipt_ids: [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    /// Lamports observed after the external atomic terminal transfer/close.
    account_lamports_after: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
}

impl DealerRuntimeLivenessTerminalV1 {
    /// Construct only from the complete canonical liveness bundle after all
    /// seven physical accounts have executed their terminal movements.
    ///
    /// The concrete adapter must decode the policy and each compartment from
    /// its owner- and address-authenticated account before calling this
    /// constructor, then supply the exact post-close lamport observations.
    /// Public caller-shaped receipt arrays cannot mint this capability.
    pub fn from_canonical_closed_bundle(
        runtime: &DealerRuntimeLivenessBindingV1,
        policy: clutch_liveness::runtime_v1::RuntimeLivenessPolicyV1,
        bundle: clutch_liveness::runtime_v1::RuntimeLivenessBundleV1,
        state_authority_account_id: Id,
        terminal_path: clutch_liveness::runtime_v1::RuntimeTerminalPathKindV1,
        account_lamports_after: [u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1],
    ) -> Result<Self> {
        runtime.validate()?;
        policy.validate().map_err(|_| Error::MismatchedBinding)?;
        bundle
            .validate(policy)
            .map_err(|_| Error::MismatchedBinding)?;
        if !bundle.all_closed().map_err(|_| Error::MismatchedBinding)? {
            return Err(Error::InvalidPhase);
        }
        state_authority_account_id.validate_live()?;
        let reconstructed = Self::runtime_binding_from_bundle(policy, bundle)?;
        if reconstructed != *runtime
            || account_lamports_after != [0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1]
        {
            return Err(Error::MismatchedBinding);
        }
        let terminal_path_index =
            u8::try_from(terminal_path.index()).map_err(|_| Error::ArithmeticOverflow)?;
        let path = policy.terminal_paths[terminal_path.index()];
        if path.kind != terminal_path {
            return Err(Error::MismatchedBinding);
        }
        let mut completed_calls = [0u32; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
        let mut completed_work_lamports = [0u64; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
        let mut terminal_receipt_ids = [Id::ZERO; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1];
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            let compartment = bundle.compartments[index];
            let terminal_receipt_id = from_liveness_id(compartment.terminal_receipt_id);
            terminal_receipt_id.validate_live()?;
            if compartment.completed_calls != path.calls[index]
                || compartment.completed_work_ceiling_lamports != path.work_lamports[index]
                || (index != DealerLivenessCompartmentV1::Source.index()
                    && from_liveness_id(compartment.identity.owner) != state_authority_account_id)
            {
                return Err(Error::MismatchedBinding);
            }
            let mut prior = 0usize;
            while prior < index {
                if terminal_receipt_ids[prior] == terminal_receipt_id {
                    return Err(Error::MismatchedBinding);
                }
                prior += 1;
            }
            completed_calls[index] = compartment.completed_calls;
            completed_work_lamports[index] = compartment.completed_work_ceiling_lamports;
            terminal_receipt_ids[index] = terminal_receipt_id;
            index += 1;
        }
        let value = Self {
            runtime_binding_digest: runtime.binding_digest()?,
            lifecycle_id: from_liveness_id(bundle.lifecycle_id),
            state_authority_account_id,
            terminal_path_index,
            completed_calls,
            completed_work_lamports,
            terminal_receipt_ids,
            account_lamports_after,
        };
        value.validate_against(runtime)?;
        Ok(value)
    }

    fn runtime_binding_from_bundle(
        policy: clutch_liveness::runtime_v1::RuntimeLivenessPolicyV1,
        bundle: clutch_liveness::runtime_v1::RuntimeLivenessBundleV1,
    ) -> Result<DealerRuntimeLivenessBindingV1> {
        DealerRuntimeLivenessBindingV1::from_canonical(&policy, &bundle.compartments)
    }

    /// Original immutable runtime-binding digest.
    pub const fn runtime_binding_digest(self) -> Id {
        self.runtime_binding_digest
    }

    /// Facility lifecycle whose seven accounts closed.
    pub const fn lifecycle_id(self) -> Id {
        self.lifecycle_id
    }

    /// Exact Dealer State authority of the six Dealer-owned compartments.
    pub const fn state_authority_account_id(self) -> Id {
        self.state_authority_account_id
    }

    /// Selected terminal path in canonical policy order.
    pub const fn terminal_path_index(self) -> u8 {
        self.terminal_path_index
    }

    /// Exact terminal receipt identities in canonical compartment order.
    pub const fn terminal_receipt_ids(self) -> [Id; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1] {
        self.terminal_receipt_ids
    }

    /// Join all terminal receipts and exact counters to the admitted binding.
    fn validate_against(&self, runtime: &DealerRuntimeLivenessBindingV1) -> Result<()> {
        runtime.validate()?;
        self.runtime_binding_digest.validate_live()?;
        self.lifecycle_id.validate_live()?;
        self.state_authority_account_id.validate_live()?;
        let path = usize::from(self.terminal_path_index);
        if path >= DEALER_RUNTIME_LIVENESS_TERMINAL_PATH_COUNT_V1
            || self.runtime_binding_digest != runtime.binding_digest()?
            || self.lifecycle_id != runtime.lifecycle_id
            || self.completed_calls != runtime.terminal_path_calls[path]
            || self.completed_work_lamports != runtime.terminal_path_work_lamports[path]
            || self.account_lamports_after != [0; DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1]
        {
            return Err(Error::MismatchedBinding);
        }
        let mut index = 0usize;
        while index < DEALER_RUNTIME_LIVENESS_COMPARTMENT_COUNT_V1 {
            self.terminal_receipt_ids[index].validate_live()?;
            if index != DealerLivenessCompartmentV1::Source.index()
                && runtime.owners[index] != self.state_authority_account_id
            {
                return Err(Error::MismatchedBinding);
            }
            let mut other = 0usize;
            while other < index {
                if self.terminal_receipt_ids[index] == self.terminal_receipt_ids[other] {
                    return Err(Error::MismatchedBinding);
                }
                other += 1;
            }
            index += 1;
        }
        Ok(())
    }
}

/// Exact physical balance observation for deletion of the V2 dependency child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFundedDependencyCloseBalanceV2 {
    /// Dependency account lamports before the atomic close.
    pub account_lamports_before: u64,
    /// Dependency account lamports after the atomic close; must be zero.
    pub account_lamports_after: u64,
    /// Counted Initialize receipt lamports before the atomic close.
    pub initialize_receipt_lamports_before: u64,
    /// Counted Initialize receipt lamports after the atomic close; must be zero.
    pub initialize_receipt_lamports_after: u64,
}

/// Pure result of the final counted dependency close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFundedDependencyCloseV2 {
    /// Authoritative State after decrementing the exact child count.
    pub state_after: DealerStateV2,
    /// Deleted dependency account.
    pub closed_dependency_account_id: Id,
    /// Deleted Initialize receipt account co-owned by the dependency child.
    pub closed_initialize_receipt_account_id: Id,
    /// Sole refundable-principal recipient.
    pub refund_recipient: Id,
    /// Exact refundable principal.
    pub refund_lamports: u64,
    /// Sole donation/surplus recipient.
    pub neutral_sink: Id,
    /// Exact donation floor plus later surplus.
    pub sink_lamports: u64,
    /// Sole refundable-principal recipient for the Initialize receipt.
    pub initialize_receipt_refund_recipient: Id,
    /// Exact refundable Initialize-receipt principal.
    pub initialize_receipt_refund_lamports: u64,
    /// Sole Initialize-receipt donation/surplus recipient.
    pub initialize_receipt_neutral_sink: Id,
    /// Exact Initialize-receipt donation floor plus later surplus.
    pub initialize_receipt_sink_lamports: u64,
}

/// Close the V2 dependency only after Position/Replay are gone and the
/// exhaustive external runtime is terminal.
pub fn close_funded_dependencies_v2(
    state: &DealerStateV2,
    dealer_state_account_id: Id,
    dependency_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    terminal: &DealerRuntimeLivenessTerminalV1,
    initialize_receipt: &DealerActionReceiptV1,
    balance: DealerFundedDependencyCloseBalanceV2,
) -> Result<DealerFundedDependencyCloseV2> {
    state.validate_against_policy(policy)?;
    dependency.validate()?;
    dealer_state_account_id.validate_live()?;
    dependency_account_id.validate_live()?;
    initialize_receipt.validate_against(schedule, runtime)?;
    terminal.validate_against(runtime)?;
    if state.phase != DealerPhaseV2::Retiring
        || state.children.facility_positions != 0
        || state.children.facility_replays != 0
        || state.children.lp_pages != 0
        || state.children.live_lp_positions != 0
        || state.children.exit_tickets != 0
        || state.children.unclaimed_lp_positions != 0
        || state.children.funded_dependencies != 1
        || state.children.epoch_bindings != 0
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state.children.terminal_allocations != 0
        || state.children.claim_work != 0
        || state.funded_dependencies_account_id != dependency_account_id
        || state.funded_dependencies_id != dependency.dependency_id()?
        || state.facility_position_binding_id != dependency.facility_position_binding_id
        || dependency.bindings.policy_id != state.policy_id
        || dependency.bindings.facility_id != state.facility_id
        || dependency.bindings.asset_vault_authority_account_id != dealer_state_account_id
        || dependency.bindings.runtime_liveness_binding_digest != runtime.binding_digest()?
        || dependency.bindings.runtime_liveness_policy_id != runtime.runtime_policy_id
        || dependency.bindings.liveness_schedule_id != schedule.schedule_id()?.untyped()
        || initialize_receipt.action != DealerRuntimeActionV1::Initialize
        || initialize_receipt.policy_id != state.policy_id
        || initialize_receipt.facility_id != state.facility_id
        || initialize_receipt.dealer_state_account_id != dealer_state_account_id
        || initialize_receipt.replay_account_id != state.facility_replay_account_id
        || initialize_receipt.receipt_account_id != dependency.initialize_receipt_account_id
        || initialize_receipt.semantic_receipt_id()? != dependency.initialize_receipt_semantic_id
        || terminal.runtime_binding_digest != dependency.bindings.runtime_liveness_binding_digest
        || terminal.state_authority_account_id != dealer_state_account_id
        || terminal.lifecycle_id != state.facility_id
        || dependency.rent.neutral_sink != policy.neutral_sink
        || balance.account_lamports_after != 0
        || balance.initialize_receipt_lamports_after != 0
    {
        return Err(Error::MismatchedBinding);
    }
    let protected = dependency
        .rent
        .refundable_principal
        .checked_add(dependency.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    if balance.account_lamports_before < protected {
        return Err(Error::ConservationFailure);
    }
    let sink_lamports = balance
        .account_lamports_before
        .checked_sub(dependency.rent.refundable_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let receipt_rent = initialize_receipt.rent();
    let receipt_protected = receipt_rent
        .refundable_principal
        .checked_add(receipt_rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    if balance.initialize_receipt_lamports_before < receipt_protected {
        return Err(Error::ConservationFailure);
    }
    let initialize_receipt_sink_lamports = balance
        .initialize_receipt_lamports_before
        .checked_sub(receipt_rent.refundable_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut state_after = *state;
    state_after.children.funded_dependencies = 0;
    state_after.funded_dependencies_account_id = Id::ZERO;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.validate_against_policy(policy)?;
    Ok(DealerFundedDependencyCloseV2 {
        state_after,
        closed_dependency_account_id: dependency_account_id,
        closed_initialize_receipt_account_id: dependency.initialize_receipt_account_id,
        refund_recipient: dependency.rent.payer,
        refund_lamports: dependency.rent.refundable_principal,
        neutral_sink: dependency.rent.neutral_sink,
        sink_lamports,
        initialize_receipt_refund_recipient: receipt_rent.payer,
        initialize_receipt_refund_lamports: receipt_rent.refundable_principal,
        initialize_receipt_neutral_sink: receipt_rent.neutral_sink,
        initialize_receipt_sink_lamports,
    })
}

const _: () = assert!(DEALER_LIVENESS_ACTION_COUNT_V1 == 22);
const _: () = assert!(DEALER_LIVENESS_SCHEDULE_BYTES_V1 == 372);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_BYTES_V1 == 412);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_BYTES_V2 == 600);
const _: () = assert!(DEALER_LIVENESS_SCHEDULE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_FUNDED_DEPENDENCIES_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
