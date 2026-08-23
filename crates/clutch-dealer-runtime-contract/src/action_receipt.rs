// SPDX-License-Identifier: AGPL-3.0-or-later

//! Immutable receipt carrier for one paid Dealer runtime action.
//!
//! The external liveness runtime remains the sole work-capital and ordinal
//! owner. This body is only the Dealer-owned, content-addressed evidence that
//! lets the SBF adapter project an actual account postimage into the generic
//! liveness transition contract. It owns no work balance and grants no
//! authority merely by being constructible in pure Rust.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    dealer_action_liveness_class_v1, DealerActionLivenessAuthorizationV1,
    DealerActionLivenessClassV1, DealerLivenessCompartmentV1, DealerLivenessScheduleV1,
    DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1, DeletableRentOwnerV1, Error,
    FixedCodec, Id, Result, DEALER_ACTION_RECEIPT_SLOT_DOMAIN_V1, DELETABLE_RENT_OWNER_BYTES,
};
use clutch_liveness::runtime_adapter_v1::{
    RuntimeReceiptKindV1, RuntimeReceiptObservationV1, RuntimeTransitionActionV1,
    RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::RuntimeCompartmentKindV1;
use sha2::{Digest, Sha256};

/// Exact local receipt magic.
pub const DEALER_ACTION_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCACTRC1";
/// Exact local receipt semantic-body version.
pub const DEALER_ACTION_RECEIPT_VERSION_V1: u16 = 1;
/// Exact bytes in one receipt body, excluding the global eight-byte envelope.
pub const DEALER_ACTION_RECEIPT_BYTES_V1: usize =
    HEADER_BYTES + (12 * 32) + 56 + DELETABLE_RENT_OWNER_BYTES;

/// Immutable Dealer-owned postimage for one successful paid action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActionReceiptV1 {
    /// Exact authenticated Dealer policy.
    pub policy_id: Id,
    /// Immutable facility lifecycle identity.
    pub facility_id: Id,
    /// Dealer State account which owns the selected liveness compartment.
    pub dealer_state_account_id: Id,
    /// Exact fine-grained Dealer quote schedule.
    pub liveness_schedule_id: Id,
    /// Exact generic runtime-liveness policy.
    pub runtime_policy_id: Id,
    /// Physical generic-runtime compartment account spent atomically.
    pub runtime_account_id: Id,
    /// Semantic owner committed by that compartment.
    pub runtime_owner: Id,
    /// Generic-runtime quote schedule identity.
    pub quote_schedule_id: Id,
    /// Physical immutable receipt PDA containing this exact body.
    pub receipt_account_id: Id,
    /// Program which owns and authenticated the receipt postimage.
    pub receipt_program_id: Id,
    /// Keeper paid by the generic-runtime transition.
    pub keeper: Id,
    /// Facility Replay account whose next postimage consumes this receipt.
    pub replay_account_id: Id,
    /// Exact Dealer action completed by the atomic transaction.
    pub action: DealerRuntimeActionV1,
    /// Canonical generic-runtime compartment consumed by the action.
    pub compartment: DealerLivenessCompartmentV1,
    /// Generic-runtime compartment generation.
    pub runtime_generation: u64,
    /// Dealer economic generation at which the action succeeds.
    pub facility_generation: u64,
    /// Next monotone call ordinal owned by the generic runtime.
    pub call_ordinal: u32,
    /// Dealer-scheduled maximum payment for this exact action.
    pub call_ceiling_lamports: u64,
    /// Actual keeper payment debited from work capital.
    pub keeper_payment_lamports: u64,
    /// Replay ordinal consumed by the same atomic Dealer transition.
    pub expected_replay_ordinal: u64,
    /// Explicit close/refund ownership; never work or collateral principal.
    pub rent: DeletableRentOwnerV1,
}

impl DealerActionReceiptV1 {
    /// Validate canonical shape before any account authority is considered.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.liveness_schedule_id,
            self.runtime_policy_id,
            self.runtime_account_id,
            self.runtime_owner,
            self.quote_schedule_id,
            self.receipt_account_id,
            self.receipt_program_id,
            self.keeper,
            self.replay_account_id,
        ] {
            identity.validate_live()?;
        }
        let expected_compartment = match dealer_action_liveness_class_v1(self.action) {
            DealerActionLivenessClassV1::Compartment(value) => value,
            DealerActionLivenessClassV1::OptionalQueueMaintenance => {
                DealerLivenessCompartmentV1::Retirement
            }
            DealerActionLivenessClassV1::OutsideFacilityRuntime
            | DealerActionLivenessClassV1::CallerFunded => return Err(Error::InvalidSchedule),
        };
        if self.compartment != expected_compartment
            || self.runtime_owner != self.dealer_state_account_id
            || self.receipt_account_id == self.runtime_account_id
            || self.receipt_account_id == self.runtime_owner
            || self.receipt_account_id == self.replay_account_id
            || self.receipt_account_id == self.rent.neutral_sink
            || self.call_ordinal == 0
            || self.call_ceiling_lamports == 0
            || self.keeper_payment_lamports > self.call_ceiling_lamports
            || self.facility_generation == 0
        {
            return Err(Error::InvalidParameter);
        }
        self.rent.validate()
    }

    /// Content address used as the second PDA seed.
    ///
    /// It excludes the receipt address and rent record so PDA derivation is
    /// acyclic, while binding every fact that makes one liveness call unique.
    pub fn receipt_slot_id(&self) -> Result<Id> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(DEALER_ACTION_RECEIPT_SLOT_DOMAIN_V1);
        hasher.update(self.facility_id.bytes());
        hasher.update(self.runtime_account_id.bytes());
        hasher.update(self.replay_account_id.bytes());
        hasher.update([self.action as u8, self.compartment as u8]);
        hasher.update(self.runtime_generation.to_le_bytes());
        hasher.update(self.facility_generation.to_le_bytes());
        hasher.update(self.call_ordinal.to_le_bytes());
        hasher.update(self.expected_replay_ordinal.to_le_bytes());
        Ok(Id::from_bytes(hasher.finalize().into()))
    }

    /// Canonical semantic receipt consumed by both liveness and Replay.
    pub fn semantic_receipt_id(&self) -> Result<Id> {
        self.validate()?;
        let authorization = DealerActionLivenessAuthorizationV1 {
            action: self.action,
            compartment: self.compartment,
            runtime_account_id: self.runtime_account_id,
            owner: self.runtime_owner,
            lifecycle_id: self.facility_id,
            quote_schedule_id: self.quote_schedule_id,
            receipt_account_id: self.receipt_account_id,
            receipt_program_id: self.receipt_program_id,
            receipt_semantic_id: Id::ZERO,
            generation: self.runtime_generation,
            facility_generation: self.facility_generation,
            call_ordinal: self.call_ordinal,
            call_ceiling_lamports: self.call_ceiling_lamports,
        };
        authorization.canonical_receipt_semantic_id()
    }

    /// Join the exact immutable schedules and authenticated seven-vault view.
    pub fn validate_against(
        &self,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
    ) -> Result<()> {
        self.validate()?;
        schedule.validate_for_facility_runtime()?;
        runtime.validate()?;
        let index = self.compartment.index();
        if self.liveness_schedule_id != schedule.schedule_id()?.untyped()
            || self.runtime_policy_id != runtime.runtime_policy_id()
            || self.facility_id != runtime.lifecycle_id()
            || self.runtime_account_id != runtime.account_id(self.compartment)
            || self.runtime_owner != runtime.owner(self.compartment)
            || self.quote_schedule_id != runtime.quote_schedule_id(self.compartment)
            || self.receipt_program_id != runtime.receipt_program_id(self.compartment)
            || self.runtime_generation != runtime.generation(self.compartment)
            || self.rent.neutral_sink != runtime.neutral_sink()
            || self.call_ceiling_lamports != schedule.reward_lamports[self.action as usize]
            || index == DealerLivenessCompartmentV1::Source.index()
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Project the authenticated postimage into the generic liveness intent.
    pub fn runtime_transition_intent(&self) -> Result<RuntimeTransitionIntentV1> {
        self.validate()?;
        let value = RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: runtime_compartment(self.compartment),
            policy_id: liveness_id(self.runtime_policy_id),
            lifecycle_id: liveness_id(self.facility_id),
            account_id: liveness_id(self.runtime_account_id),
            semantic_owner: liveness_id(self.runtime_owner),
            quote_schedule_id: liveness_id(self.quote_schedule_id),
            receipt_id: liveness_id(self.semantic_receipt_id()?),
            keeper: liveness_id(self.keeper),
            generation: self.runtime_generation,
            call_ordinal: self.call_ordinal,
            call_ceiling_lamports: self.call_ceiling_lamports,
            keeper_payment_lamports: self.keeper_payment_lamports,
            flags: 0,
        };
        value.validate().map_err(|_| Error::MismatchedBinding)?;
        Ok(value)
    }

    /// Project the authenticated postimage into the generic receipt view.
    pub fn runtime_receipt_observation(&self) -> Result<RuntimeReceiptObservationV1> {
        self.validate()?;
        Ok(RuntimeReceiptObservationV1 {
            receipt_account_id: liveness_id(self.receipt_account_id),
            receipt_account_owner_program_id: liveness_id(self.receipt_program_id),
            receipt_id: liveness_id(self.semantic_receipt_id()?),
            receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
            compartment_kind: runtime_compartment(self.compartment),
            semantic_owner: liveness_id(self.runtime_owner),
            lifecycle_id: liveness_id(self.facility_id),
            quote_schedule_id: liveness_id(self.quote_schedule_id),
            generation: self.runtime_generation,
            call_ordinal: self.call_ordinal,
            call_ceiling_lamports: self.call_ceiling_lamports,
        })
    }

    /// Produce the private-runtime-checked Dealer authorization from this body.
    pub fn authorization(
        &self,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
        compartment: &clutch_liveness::runtime_v1::RuntimeCompartmentV1,
    ) -> Result<DealerActionLivenessAuthorizationV1> {
        self.validate_against(schedule, runtime)?;
        let intent = self.runtime_transition_intent()?;
        let observation = self.runtime_receipt_observation()?;
        let authorization = DealerActionLivenessAuthorizationV1::from_canonical(
            self.action,
            self.facility_generation,
            compartment,
            &intent,
            &observation,
        )?;
        authorization.validate_against(schedule, runtime)?;
        Ok(authorization)
    }

    /// Exact rent ownership retained by this deletable receipt.
    pub const fn rent(&self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

impl FixedCodec for DealerActionReceiptV1 {
    const ENCODED_LEN: usize = DEALER_ACTION_RECEIPT_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_ACTION_RECEIPT_MAGIC_V1,
            DEALER_ACTION_RECEIPT_VERSION_V1,
        );
        for identity in [
            self.policy_id,
            self.facility_id,
            self.dealer_state_account_id,
            self.liveness_schedule_id,
            self.runtime_policy_id,
            self.runtime_account_id,
            self.runtime_owner,
            self.quote_schedule_id,
            self.receipt_account_id,
            self.receipt_program_id,
            self.keeper,
            self.replay_account_id,
        ] {
            writer.id(identity);
        }
        writer.u8(self.action as u8);
        writer.u8(self.compartment as u8);
        writer.reserved(6);
        writer.u64(self.runtime_generation);
        writer.u64(self.facility_generation);
        writer.u32(self.call_ordinal);
        writer.reserved(4);
        writer.u64(self.call_ceiling_lamports);
        writer.u64(self.keeper_payment_lamports);
        writer.u64(self.expected_replay_ordinal);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_ACTION_RECEIPT_MAGIC_V1,
            DEALER_ACTION_RECEIPT_VERSION_V1,
        )?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let dealer_state_account_id = reader.id();
        let liveness_schedule_id = reader.id();
        let runtime_policy_id = reader.id();
        let runtime_account_id = reader.id();
        let runtime_owner = reader.id();
        let quote_schedule_id = reader.id();
        let receipt_account_id = reader.id();
        let receipt_program_id = reader.id();
        let keeper = reader.id();
        let replay_account_id = reader.id();
        let action = DealerRuntimeActionV1::from_index(usize::from(reader.u8()))?;
        let compartment = decode_compartment(reader.u8())?;
        reader.reserved(6)?;
        let runtime_generation = reader.u64();
        let facility_generation = reader.u64();
        let call_ordinal = reader.u32();
        reader.reserved(4)?;
        let value = Self {
            policy_id,
            facility_id,
            dealer_state_account_id,
            liveness_schedule_id,
            runtime_policy_id,
            runtime_account_id,
            runtime_owner,
            quote_schedule_id,
            receipt_account_id,
            receipt_program_id,
            keeper,
            replay_account_id,
            action,
            compartment,
            runtime_generation,
            facility_generation,
            call_ordinal,
            call_ceiling_lamports: reader.u64(),
            keeper_payment_lamports: reader.u64(),
            expected_replay_ordinal: reader.u64(),
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const fn liveness_id(value: Id) -> clutch_liveness::Id {
    clutch_liveness::Id::from_bytes(value.bytes())
}

const fn runtime_compartment(value: DealerLivenessCompartmentV1) -> RuntimeCompartmentKindV1 {
    match value {
        DealerLivenessCompartmentV1::Source => RuntimeCompartmentKindV1::Source,
        DealerLivenessCompartmentV1::Candidate => RuntimeCompartmentKindV1::Candidate,
        DealerLivenessCompartmentV1::Clearing => RuntimeCompartmentKindV1::Clearing,
        DealerLivenessCompartmentV1::Settlement => RuntimeCompartmentKindV1::Settlement,
        DealerLivenessCompartmentV1::Resolution => RuntimeCompartmentKindV1::Resolution,
        DealerLivenessCompartmentV1::Retirement => RuntimeCompartmentKindV1::Retirement,
        DealerLivenessCompartmentV1::Recovery => RuntimeCompartmentKindV1::Recovery,
    }
}

fn decode_compartment(value: u8) -> Result<DealerLivenessCompartmentV1> {
    match value {
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

const _: () = assert!(DEALER_ACTION_RECEIPT_BYTES_V1 == 532);
const _: () = assert!(DEALER_ACTION_RECEIPT_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
