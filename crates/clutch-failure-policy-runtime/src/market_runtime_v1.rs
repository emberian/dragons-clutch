// SPDX-License-Identifier: AGPL-3.0-or-later
//! Market-scoped Failure runtime successor.
//!
//! The legacy external runtime binds one Series and ordinal into its identity.
//! This successor instead derives its identity only from the immutable shared
//! Market policy. Series links and Source occurrences are subordinate session
//! inputs and never alter the runtime account identity. The existing evidence-recovery
//! engine remains the semantic owner of finite schedules and exact rewards;
//! its founding Series quote is funding provenance, not a Series-scoped account key.

use clutch_evidence_recovery::{
    ExternalRecoveryAdmissionV1, ExternalRecoveryFundingV1, ExternalRecoveryStateV1,
    Identity as RecoveryIdentity, RecoveryClock, RecoveryPhase, EXTERNAL_RECOVERY_STATE_V1_BYTES,
};
use clutch_product_series::{
    CompiledScheduleV1, ContentId as ProductContentId, MarketInstanceV2Id, SeriesFundingQuoteId,
    SeriesFundingQuoteV1,
};
use sha2::{Digest, Sha256};

use crate::market_policy_v1::{
    FailureMarketAccountIdV1, FailureMarketAdmissionStateIdV1, FailureMarketAdmissionStateV1,
    FailureMarketRecoveryFundingReceiptIdV1,
};
use crate::{Error, FailurePolicyBindingId, Result};

const RUNTIME_ADMISSION_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-runtime-admission/v1";
const RUNTIME_COMMITMENT_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-runtime-commitment/v1";
const SCHEDULE_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-recovery-schedule/v1";
const MAGIC_V1: [u8; 8] = *b"DCFMRUN1";
const VERSION_V1: u16 = 1;
const HEADER_BYTES_V1: usize = 16;
const ID_BYTES_V1: usize = 32;
const PREFIX_ID_COUNT_V1: usize = 4;
const ROOT_FUNDING_ID_COUNT_V1: usize = 2;
const ROOT_FUNDING_AMOUNT_COUNT_V1: usize = 3;
const PHASE_BYTES_V1: usize = 8;
const SESSION_ID_COUNT_V1: usize = 8;

/// Canonical semantic body width inside the FailureRuntimeRoot account.
pub const FAILURE_MARKET_RUNTIME_BYTES_V1: usize = 2_048;

macro_rules! runtime_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from digest bytes without claiming authenticity.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Return exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

runtime_id!(
    FailureMarketRuntimeAdmissionReceiptIdV1,
    "Typed identity of one authenticated Market runtime foundation."
);
runtime_id!(
    FailureMarketRuntimeStateCommitmentV1,
    "Typed commitment to one complete canonical Market runtime state."
);
runtime_id!(
    FailureMarketRecoveryScheduleIdV1,
    "Typed identity of one exact absolute finite recovery schedule."
);

/// Current Market runtime lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketRuntimePhaseV1 {
    /// Funded and ready; no Series/source interval session is pinned.
    Ready = 1,
    /// A subordinate Series/source interval session is active.
    IntervalActive = 2,
    /// The interval resolved but its mutable work remains open.
    IntervalResolved = 3,
    /// Interval work closed and permanent replay is readable.
    IntervalClosed = 4,
    /// Sole liveness Recovery custody closed successfully.
    RecoveryClosed = 5,
    /// Durable market-level Failure terminal receipt persisted.
    FamilyTerminal = 6,
}

impl FailureMarketRuntimePhaseV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Ready => 1,
            Self::IntervalActive => 2,
            Self::IntervalResolved => 3,
            Self::IntervalClosed => 4,
            Self::RecoveryClosed => 5,
            Self::FamilyTerminal => 6,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Ready),
            2 => Ok(Self::IntervalActive),
            3 => Ok(Self::IntervalResolved),
            4 => Ok(Self::IntervalClosed),
            5 => Ok(Self::RecoveryClosed),
            6 => Ok(Self::FamilyTerminal),
            _ => Err(Error::WrongPhase),
        }
    }
}

/// Complete expected foundation facts. This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeAdmissionFactsV1 {
    /// Shared Market policy binding.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Immutable admission-state content identity.
    pub admission_state_id: FailureMarketAdmissionStateIdV1,
    /// Distinct mutable runtime root account.
    pub runtime_account_id: FailureMarketAccountIdV1,
    /// Product private foundation poststate receipt.
    pub foundation_receipt_id: ProductContentId,
    /// Immutable Product-prepaid runtime-account rent ownership.
    pub root_funding: FailureMarketRuntimeRootFundingFactsV1,
    /// Present Recovery funding receipt retained by the admission root.
    pub recovery_funding_receipt_id: FailureMarketRecoveryFundingReceiptIdV1,
    /// Exact absolute finite schedule selected by immutable policy.
    pub recovery_schedule_id: FailureMarketRecoveryScheduleIdV1,
    /// Founder quote whose Recovery pricing capitalized the shared account.
    pub recovery_funding_quote_id: SeriesFundingQuoteId,
    /// Complete initial runtime postimage.
    pub runtime_state_commitment: FailureMarketRuntimeStateCommitmentV1,
}

/// Exact native-lamport ownership of the mutable Failure runtime account.
///
/// This is disjoint from liveness Recovery custody. The refund owner need not
/// sign or pay again: Product already debited the founder's prepaid MarketCore
/// custody before this state can be admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeRootFundingFactsV1 {
    /// Immutable refund recipient for the exact account-rent principal.
    pub rent_refund_owner: FailureMarketAccountIdV1,
    /// System-owned destination for prior and later unsolicited lamports.
    pub neutral_sink: FailureMarketAccountIdV1,
    /// Canonical Rent minimum for the framed runtime account at creation.
    pub rent_principal_lamports: u64,
    /// Unsolicited lamports already present before Product capitalization.
    pub donation_floor_lamports: u64,
    /// Exact post-capitalization account balance.
    pub observed_balance_lamports: u64,
}

/// Product/SBF-owned authority for the exact funded runtime foundation.
pub trait AuthenticatedFailureMarketRuntimeAdmissionV1 {
    /// Authenticate the expected graph and physical funded poststate.
    fn authenticate_failure_market_runtime_admission(
        &self,
        _expected: FailureMarketRuntimeAdmissionFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field foundation receipt consumed by Product activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeAdmissionReceiptV1 {
    id: FailureMarketRuntimeAdmissionReceiptIdV1,
    facts: FailureMarketRuntimeAdmissionFactsV1,
}

impl FailureMarketRuntimeAdmissionReceiptV1 {
    /// Complete foundation receipt identity.
    pub const fn id(self) -> FailureMarketRuntimeAdmissionReceiptIdV1 {
        self.id
    }

    /// Exact authenticated foundation facts.
    pub const fn facts(self) -> FailureMarketRuntimeAdmissionFactsV1 {
        self.facts
    }
}

/// Market-scoped dynamic Failure runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRuntimeV1 {
    policy_binding_id: FailurePolicyBindingId,
    admission_state_id: FailureMarketAdmissionStateIdV1,
    runtime_account_id: FailureMarketAccountIdV1,
    foundation_receipt_id: ProductContentId,
    root_funding: FailureMarketRuntimeRootFundingFactsV1,
    recovery: ExternalRecoveryStateV1,
    phase: FailureMarketRuntimePhaseV1,
    transition_sequence: u64,
    session_ids: [ProductContentId; SESSION_ID_COUNT_V1],
}

impl FailureMarketRuntimeV1 {
    /// Immutable shared policy identity.
    pub const fn policy_binding_id(self) -> FailurePolicyBindingId {
        self.policy_binding_id
    }

    /// Immutable admission-state identity.
    pub const fn admission_state_id(self) -> FailureMarketAdmissionStateIdV1 {
        self.admission_state_id
    }

    /// Physical mutable runtime root.
    pub const fn runtime_account_id(self) -> FailureMarketAccountIdV1 {
        self.runtime_account_id
    }

    /// Current lifecycle phase.
    pub const fn phase(self) -> FailureMarketRuntimePhaseV1 {
        self.phase
    }

    /// Monotone wrapper transition sequence.
    pub const fn transition_sequence(self) -> u64 {
        self.transition_sequence
    }

    /// Immutable Product-prepaid runtime-account rent ownership.
    pub const fn root_funding(self) -> FailureMarketRuntimeRootFundingFactsV1 {
        self.root_funding
    }

    /// Underlying finite recovery engine. Its founder quote is funding
    /// provenance and is excluded from the Market runtime account identity.
    pub const fn recovery(self) -> ExternalRecoveryStateV1 {
        self.recovery
    }

    /// Canonical state commitment.
    pub fn commitment(self) -> Result<FailureMarketRuntimeStateCommitmentV1> {
        let mut bytes = [0u8; FAILURE_MARKET_RUNTIME_BYTES_V1];
        self.encode_into(&mut bytes)?;
        let mut hasher = Sha256::new();
        hasher.update(RUNTIME_COMMITMENT_DOMAIN_V1);
        hasher.update(bytes);
        Ok(FailureMarketRuntimeStateCommitmentV1::from_bytes(
            hasher.finalize().into(),
        ))
    }

    /// Encode every semantic and reserved byte canonically.
    pub fn encode_into(self, output: &mut [u8; FAILURE_MARKET_RUNTIME_BYTES_V1]) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[..8].copy_from_slice(&MAGIC_V1);
        output[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
        let mut cursor = HEADER_BYTES_V1;
        for id in [
            self.policy_binding_id.bytes(),
            self.admission_state_id.bytes(),
            self.runtime_account_id.bytes(),
            self.foundation_receipt_id.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        for id in [
            self.root_funding.rent_refund_owner.bytes(),
            self.root_funding.neutral_sink.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        for amount in [
            self.root_funding.rent_principal_lamports,
            self.root_funding.donation_floor_lamports,
            self.root_funding.observed_balance_lamports,
        ] {
            put_u64(output, &mut cursor, amount)?;
        }
        let recovery_end = cursor
            .checked_add(EXTERNAL_RECOVERY_STATE_V1_BYTES)
            .ok_or(Error::WrongLength)?;
        self.recovery.encode_into(
            output
                .get_mut(cursor..recovery_end)
                .ok_or(Error::WrongLength)?,
        )?;
        cursor = recovery_end;
        output[cursor] = self.phase.byte();
        cursor = cursor
            .checked_add(PHASE_BYTES_V1)
            .ok_or(Error::WrongLength)?;
        put_u64(output, &mut cursor, self.transition_sequence)?;
        for id in self.session_ids {
            put_id(output, &mut cursor, id.bytes())?;
        }
        if output
            .get(cursor..)
            .ok_or(Error::WrongLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::WrongLength);
        }
        Ok(())
    }

    /// Decode only against the independently authenticated immutable
    /// admission root. Raw bytes cannot select their own policy binding.
    pub fn decode_for_admission(
        input: &[u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
        admission: FailureMarketAdmissionStateV1,
    ) -> Result<Self> {
        if input[..8] != MAGIC_V1 {
            return Err(Error::BadMagic);
        }
        if input[8..10] != VERSION_V1.to_le_bytes() {
            return Err(Error::BadVersion);
        }
        if input[10..HEADER_BYTES_V1].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        let mut cursor = HEADER_BYTES_V1;
        let policy_binding_id = FailurePolicyBindingId::from_bytes(take_id(input, &mut cursor)?);
        let admission_state_id =
            FailureMarketAdmissionStateIdV1::from_bytes(take_id(input, &mut cursor)?);
        let runtime_account_id = FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?);
        let foundation_receipt_id = ProductContentId::from_bytes(take_id(input, &mut cursor)?);
        let root_funding = FailureMarketRuntimeRootFundingFactsV1 {
            rent_refund_owner: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            neutral_sink: FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?),
            rent_principal_lamports: take_u64(input, &mut cursor)?,
            donation_floor_lamports: take_u64(input, &mut cursor)?,
            observed_balance_lamports: take_u64(input, &mut cursor)?,
        };
        let recovery_end = cursor
            .checked_add(EXTERNAL_RECOVERY_STATE_V1_BYTES)
            .ok_or(Error::WrongLength)?;
        let recovery = ExternalRecoveryStateV1::decode(
            input.get(cursor..recovery_end).ok_or(Error::WrongLength)?,
        )?;
        cursor = recovery_end;
        let phase = FailureMarketRuntimePhaseV1::decode(input[cursor])?;
        if input[cursor + 1..cursor + PHASE_BYTES_V1]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReserved);
        }
        cursor = cursor
            .checked_add(PHASE_BYTES_V1)
            .ok_or(Error::WrongLength)?;
        let transition_sequence = take_u64(input, &mut cursor)?;
        let mut session_ids = [ProductContentId::ZERO; SESSION_ID_COUNT_V1];
        let mut index = 0usize;
        while index < SESSION_ID_COUNT_V1 {
            session_ids[index] = ProductContentId::from_bytes(take_id(input, &mut cursor)?);
            index += 1;
        }
        if input
            .get(cursor..)
            .ok_or(Error::WrongLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            policy_binding_id,
            admission_state_id,
            runtime_account_id,
            foundation_receipt_id,
            root_funding,
            recovery,
            phase,
            transition_sequence,
            session_ids,
        };
        value.validate_against_admission(admission)?;
        Ok(value)
    }

    fn validate(self) -> Result<()> {
        self.recovery.check()?;
        require_live(self.policy_binding_id.bytes())?;
        require_live(self.admission_state_id.bytes())?;
        require_live(self.runtime_account_id.bytes())?;
        require_live(self.foundation_receipt_id.bytes())?;
        require_live(self.root_funding.rent_refund_owner.bytes())?;
        require_live(self.root_funding.neutral_sink.bytes())?;
        if self.root_funding.rent_principal_lamports == 0
            || self.root_funding.observed_balance_lamports
                != self
                    .root_funding
                    .rent_principal_lamports
                    .checked_add(self.root_funding.donation_floor_lamports)
                    .ok_or(Error::BindingMismatch)?
            || self.runtime_account_id == self.root_funding.rent_refund_owner
            || self.runtime_account_id == self.root_funding.neutral_sink
            || self.root_funding.rent_refund_owner == self.root_funding.neutral_sink
        {
            return Err(Error::BindingMismatch);
        }
        let live = |index: usize| !self.session_ids[index].is_zero();
        match self.phase {
            FailureMarketRuntimePhaseV1::Ready => {
                if self.transition_sequence != 0
                    || self.session_ids.iter().any(|id| !id.is_zero())
                    || self.recovery.phase() != RecoveryPhase::Active
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::IntervalActive => {
                if self.transition_sequence == 0
                    || self.recovery.phase() != RecoveryPhase::DegradedRecoverable
                    || !(live(0) && live(1) && live(2) && live(3))
                    || self.session_ids[4..].iter().any(|id| !id.is_zero())
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::IntervalResolved => {
                if self.transition_sequence == 0
                    || self.recovery.phase() != RecoveryPhase::Resolved
                    || !(live(0) && live(1) && live(2) && live(3))
                    || self.session_ids[4..].iter().any(|id| !id.is_zero())
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::IntervalClosed => {
                if self.transition_sequence == 0
                    || self.recovery.phase() != RecoveryPhase::Resolved
                    || !(live(0) && live(1) && live(2) && live(3) && live(4))
                    || self.session_ids[5..].iter().any(|id| !id.is_zero())
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::RecoveryClosed => {
                if self.transition_sequence == 0
                    || self.recovery.phase() != RecoveryPhase::Resolved
                    || !(live(0) && live(1) && live(2) && live(3) && live(4) && live(5) && live(6))
                    || live(7)
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketRuntimePhaseV1::FamilyTerminal => {
                if self.transition_sequence == 0
                    || self.recovery.phase() != RecoveryPhase::Resolved
                    || self.session_ids.iter().any(|id| id.is_zero())
                {
                    return Err(Error::WrongPhase);
                }
            }
        }
        Ok(())
    }

    fn validate_against_admission(self, admission: FailureMarketAdmissionStateV1) -> Result<()> {
        self.validate()?;
        let policy = admission.binding().facts();
        let recovery_funding = admission.recovery_funding().facts();
        if self.policy_binding_id != admission.binding().id()
            || self.admission_state_id != admission.id()?
            || self.runtime_account_id.bytes() != policy.recovery_state_id.bytes()
            || self.runtime_account_id == admission.root_funding().facts().root_account_id
            || self.recovery.market_instance_id() != policy.market_instance_id
            || self.recovery.recovery_policy_id() != policy.recovery_policy_id
            || self.recovery.state_id().bytes() != self.runtime_account_id.bytes()
            || self.recovery.generation() != policy.generation
            || self.recovery.funding().policy_id.bytes() != policy.liveness_policy_id.bytes()
            || self.recovery.funding().lifecycle_id.bytes() != policy.liveness_lifecycle_id.bytes()
            || self.recovery.funding().recovery_account_id.bytes()
                != policy.recovery_compartment_account_id.bytes()
            || self.recovery.funding().quote_schedule_id.bytes()
                != policy.recovery_quote_schedule_id.bytes()
            || self.recovery.funding().capitalized_work_lamports
                != recovery_funding.work_principal_lamports
            || self.recovery.funding().rent_principal_lamports
                != recovery_funding.rent_principal_lamports
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Admit the distinct mutable Market runtime from exact Product and liveness
/// authority. The founder quote capitalizes the sole shared Recovery custody;
/// later converging Series cannot replace it or alter runtime identity.
#[allow(clippy::too_many_arguments)]
pub fn admit_failure_market_runtime_v1<A: AuthenticatedFailureMarketRuntimeAdmissionV1 + ?Sized>(
    authority: &A,
    admission: FailureMarketAdmissionStateV1,
    runtime_account_id: FailureMarketAccountIdV1,
    foundation_receipt_id: ProductContentId,
    root_funding: FailureMarketRuntimeRootFundingFactsV1,
    schedule: CompiledScheduleV1,
    funding_quote: SeriesFundingQuoteV1,
    creation_clock: RecoveryClock,
) -> Result<(
    FailureMarketRuntimeV1,
    FailureMarketRuntimeAdmissionReceiptV1,
)> {
    let policy = admission.binding().facts();
    let funding = admission.recovery_funding().facts();
    let external_funding = ExternalRecoveryFundingV1 {
        policy_id: RecoveryIdentity::from_bytes(policy.liveness_policy_id.bytes()),
        lifecycle_id: RecoveryIdentity::from_bytes(policy.liveness_lifecycle_id.bytes()),
        recovery_account_id: RecoveryIdentity::from_bytes(
            policy.recovery_compartment_account_id.bytes(),
        ),
        semantic_owner: RecoveryIdentity::from_bytes(policy.recovery_receipt_program_id.bytes()),
        payer: RecoveryIdentity::from_bytes(policy.recovery_refund_owner.bytes()),
        neutral_sink: RecoveryIdentity::from_bytes(policy.neutral_sink.bytes()),
        receipt_program_id: RecoveryIdentity::from_bytes(
            policy.recovery_receipt_program_id.bytes(),
        ),
        quote_schedule_id: RecoveryIdentity::from_bytes(policy.recovery_quote_schedule_id.bytes()),
        generation: policy.generation,
        capitalized_work_lamports: funding.work_principal_lamports,
        rent_principal_lamports: funding.rent_principal_lamports,
        maximum_calls: funding.maximum_calls,
        maximum_lamports_per_call: funding.maximum_lamports_per_call,
    };
    let recovery = ExternalRecoveryStateV1::admit(
        policy.market_instance_id,
        policy.recovery_policy_id,
        schedule,
        funding_quote,
        ExternalRecoveryAdmissionV1 {
            state_id: RecoveryIdentity::from_bytes(runtime_account_id.bytes()),
            generation: policy.generation,
            funding: external_funding,
        },
        creation_clock,
    )?;
    let runtime = FailureMarketRuntimeV1 {
        policy_binding_id: admission.binding().id(),
        admission_state_id: admission.id()?,
        runtime_account_id,
        foundation_receipt_id,
        root_funding,
        recovery,
        phase: FailureMarketRuntimePhaseV1::Ready,
        transition_sequence: 0,
        session_ids: [ProductContentId::ZERO; SESSION_ID_COUNT_V1],
    };
    runtime.validate_against_admission(admission)?;
    let facts = FailureMarketRuntimeAdmissionFactsV1 {
        failure_policy_binding_id: runtime.policy_binding_id,
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        admission_state_id: runtime.admission_state_id,
        runtime_account_id,
        foundation_receipt_id,
        root_funding,
        recovery_funding_receipt_id: admission.recovery_funding().id(),
        recovery_schedule_id: schedule_id(schedule)?,
        recovery_funding_quote_id: recovery.funding_quote_id(),
        runtime_state_commitment: runtime.commitment()?,
    };
    authority.authenticate_failure_market_runtime_admission(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(RUNTIME_ADMISSION_DOMAIN_V1);
    hash_admission_facts(&mut hasher, facts);
    let id = FailureMarketRuntimeAdmissionReceiptIdV1::from_bytes(hasher.finalize().into());
    if id.bytes().iter().all(|byte| *byte == 0) {
        return Err(Error::BindingMismatch);
    }
    Ok((
        runtime,
        FailureMarketRuntimeAdmissionReceiptV1 { id, facts },
    ))
}

fn schedule_id(schedule: CompiledScheduleV1) -> Result<FailureMarketRecoveryScheduleIdV1> {
    schedule.validate()?;
    let mut hasher = Sha256::new();
    hasher.update(SCHEDULE_DOMAIN_V1);
    hasher.update(schedule.start_bucket.to_le_bytes());
    hasher.update(schedule.end_bucket_exclusive.to_le_bytes());
    hasher.update(schedule.primary_maturity_bucket_exclusive.to_le_bytes());
    hasher.update([schedule.recovery_attempt_count]);
    for attempt in schedule.recovery_attempts {
        hasher.update(attempt.repair_generation.to_le_bytes());
        hasher.update(attempt.opens_at_bucket.to_le_bytes());
        hasher.update(attempt.closes_at_bucket.to_le_bytes());
    }
    Ok(FailureMarketRecoveryScheduleIdV1::from_bytes(
        hasher.finalize().into(),
    ))
}

fn hash_admission_facts(hasher: &mut Sha256, facts: FailureMarketRuntimeAdmissionFactsV1) {
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.admission_state_id.bytes());
    hasher.update(facts.runtime_account_id.bytes());
    hasher.update(facts.foundation_receipt_id.bytes());
    hasher.update(facts.root_funding.rent_refund_owner.bytes());
    hasher.update(facts.root_funding.neutral_sink.bytes());
    hasher.update(facts.root_funding.rent_principal_lamports.to_le_bytes());
    hasher.update(facts.root_funding.donation_floor_lamports.to_le_bytes());
    hasher.update(facts.root_funding.observed_balance_lamports.to_le_bytes());
    hasher.update(facts.recovery_funding_receipt_id.bytes());
    hasher.update(facts.recovery_schedule_id.bytes());
    hasher.update(facts.recovery_funding_quote_id.bytes());
    hasher.update(facts.runtime_state_commitment.bytes());
}

fn put_id(
    output: &mut [u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
    cursor: &mut usize,
    value: [u8; ID_BYTES_V1],
) -> Result<()> {
    let end = cursor.checked_add(ID_BYTES_V1).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value);
    *cursor = end;
    Ok(())
}

fn take_id(
    input: &[u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
    cursor: &mut usize,
) -> Result<[u8; ID_BYTES_V1]> {
    let end = cursor.checked_add(ID_BYTES_V1).ok_or(Error::WrongLength)?;
    let value = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(value)
}

fn put_u64(
    output: &mut [u8; FAILURE_MARKET_RUNTIME_BYTES_V1],
    cursor: &mut usize,
    value: u64,
) -> Result<()> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value.to_le_bytes());
    *cursor = end;
    Ok(())
}

fn take_u64(input: &[u8; FAILURE_MARKET_RUNTIME_BYTES_V1], cursor: &mut usize) -> Result<u64> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    let bytes = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(u64::from_le_bytes(bytes))
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::BindingMismatch)
    } else {
        Ok(())
    }
}

const _: () = assert!(
    HEADER_BYTES_V1
        + PREFIX_ID_COUNT_V1 * ID_BYTES_V1
        + ROOT_FUNDING_ID_COUNT_V1 * ID_BYTES_V1
        + ROOT_FUNDING_AMOUNT_COUNT_V1 * 8
        + EXTERNAL_RECOVERY_STATE_V1_BYTES
        + PHASE_BYTES_V1
        + 8
        + SESSION_ID_COUNT_V1 * ID_BYTES_V1
        <= FAILURE_MARKET_RUNTIME_BYTES_V1
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_policy_v1::{
        admit_failure_market_policy_v1, admit_failure_market_recovery_funding_v1,
        admit_failure_market_root_funding_v1, AuthenticatedFailureMarketPolicyV1,
        AuthenticatedFailureMarketRecoveryFundingV1, AuthenticatedFailureMarketRootFundingV1,
        FailureMarketPolicyFactsV1, FailureMarketPrepaidDebitReceiptIdV1,
        FailureMarketRecoveryFundingFactsV1, FailureMarketRootFundingFactsV1,
    };
    use clutch_liveness::Id as LivenessId;
    use clutch_product_series::{
        AbsoluteRecoveryAttemptV1, ComponentDebitV1, EvidenceOnlyRecoveryPolicyId,
        MarketGenesisProfileV2Id, NativeClaimBasisId, PriceMeasurePolicyV1Id, ProductTemplateId,
        QuantizedIntervalConsensusProfileV1Id, RecoveryAttemptFundingV1,
        RegistryCapabilityProfileV2Id, RegistryProgramReleaseV1Id, MAX_RECOVERY_ATTEMPTS,
    };
    use clutch_source_plane_v3::ContentId as SourceContentId;

    #[derive(Clone, Copy, Debug)]
    struct ExactPolicy(FailureMarketPolicyFactsV1);

    impl AuthenticatedFailureMarketPolicyV1 for ExactPolicy {
        fn authenticate_failure_market_policy(
            &self,
            expected: FailureMarketPolicyFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ExactRecovery(FailureMarketRecoveryFundingFactsV1);

    impl AuthenticatedFailureMarketRecoveryFundingV1 for ExactRecovery {
        fn authenticate_failure_market_recovery_funding(
            &self,
            expected: FailureMarketRecoveryFundingFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ExactRoot(FailureMarketRootFundingFactsV1);

    impl AuthenticatedFailureMarketRootFundingV1 for ExactRoot {
        fn authenticate_failure_market_root_funding(
            &self,
            expected: FailureMarketRootFundingFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ExactRuntime(FailureMarketRuntimeAdmissionFactsV1);

    impl AuthenticatedFailureMarketRuntimeAdmissionV1 for ExactRuntime {
        fn authenticate_failure_market_runtime_admission(
            &self,
            expected: FailureMarketRuntimeAdmissionFactsV1,
        ) -> Result<()> {
            if self.0 == expected {
                Ok(())
            } else {
                Err(Error::BindingMismatch)
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Refusing;

    impl AuthenticatedFailureMarketRuntimeAdmissionV1 for Refusing {}

    fn schedule() -> CompiledScheduleV1 {
        let mut attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = AbsoluteRecoveryAttemptV1 {
            repair_generation: 2,
            opens_at_bucket: 10,
            closes_at_bucket: 20,
        };
        CompiledScheduleV1 {
            start_bucket: 1,
            end_bucket_exclusive: 5,
            primary_maturity_bucket_exclusive: 10,
            recovery_attempt_count: 1,
            recovery_attempts: attempts,
        }
    }

    fn quote(recovery_policy_id: EvidenceOnlyRecoveryPolicyId) -> SeriesFundingQuoteV1 {
        let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 10,
            lamports_per_progress_unit: 100,
        };
        SeriesFundingQuoteV1 {
            evidence_only_recovery_policy_id: recovery_policy_id,
            market_core: ComponentDebitV1 {
                lamports: 600,
                collateral_atoms: 0,
            },
            failure_root_rent_principal_lamports: 300,
            failure_replay_tombstone_rent_principal_lamports: 200,
            recovery_reserve: ComponentDebitV1 {
                lamports: 1_200,
                collateral_atoms: 0,
            },
            source_work: ComponentDebitV1::ZERO,
            liquidity_facility: ComponentDebitV1::ZERO,
            wrapper_set: ComponentDebitV1::ZERO,
            recovery_attempt_count: 1,
            recovery_attempt_funding: attempts,
            recovery_rent_principal_lamports: 200,
        }
    }

    fn admission() -> (FailureMarketAdmissionStateV1, SeriesFundingQuoteV1) {
        let recovery_policy_id = EvidenceOnlyRecoveryPolicyId::from_bytes([4; 32]);
        let quote = quote(recovery_policy_id);
        let quote_id = quote.id().unwrap();
        let facts = FailureMarketPolicyFactsV1 {
            market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
            product_template_id: ProductTemplateId::from_bytes([2; 32]),
            native_claim_basis_id: NativeClaimBasisId::from_bytes([3; 32]),
            recovery_policy_id,
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes([5; 32]),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes([6; 32]),
            relation_policy_id: ProductContentId::from_bytes([7; 32]),
            registry_release_id: RegistryProgramReleaseV1Id::from_bytes([8; 32]),
            capability_profile_id: RegistryCapabilityProfileV2Id::from_bytes([9; 32]),
            interval_consensus_profile_id: QuantizedIntervalConsensusProfileV1Id::from_bytes(
                [10; 32],
            ),
            maximum_interval_width: 1_000,
            maximum_coordinates_per_advance: 32,
            source_release_manifest_id: SourceContentId::from_bytes([11; 32]),
            source_release_authentication_id: SourceContentId::from_bytes([12; 32]),
            source_release_account_id: FailureMarketAccountIdV1::from_bytes([13; 32]),
            source_plane_contract_id: SourceContentId::from_bytes([14; 32]),
            source_spec_id: SourceContentId::from_bytes([15; 32]),
            summary_program_id: SourceContentId::from_bytes([16; 32]),
            primary_window_id: SourceContentId::from_bytes([17; 32]),
            statistic_key_id: SourceContentId::from_bytes([18; 32]),
            clock_policy_id: SourceContentId::from_bytes([19; 32]),
            recovery_state_id: RecoveryIdentity::from_bytes([20; 32]),
            recovery_compartment_account_id: LivenessId::from_bytes([21; 32]),
            liveness_policy_id: LivenessId::from_bytes([22; 32]),
            liveness_lifecycle_id: LivenessId::from_bytes([23; 32]),
            recovery_quote_schedule_id: LivenessId::from_bytes(quote_id.bytes()),
            recovery_receipt_program_id: LivenessId::from_bytes([24; 32]),
            recovery_refund_owner: LivenessId::from_bytes([25; 32]),
            neutral_sink: LivenessId::from_bytes([26; 32]),
            generation: 1,
        };
        let binding = admit_failure_market_policy_v1(&ExactPolicy(facts), facts).unwrap();
        let recovery_facts = FailureMarketRecoveryFundingFactsV1 {
            failure_policy_binding_id: binding.id(),
            prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1::from_bytes([90; 32]),
            recovery_compartment_account_id: facts.recovery_compartment_account_id,
            liveness_policy_id: facts.liveness_policy_id,
            liveness_lifecycle_id: facts.liveness_lifecycle_id,
            recovery_quote_schedule_id: facts.recovery_quote_schedule_id,
            generation: 1,
            work_principal_lamports: 1_000,
            rent_principal_lamports: 200,
            donation_lamports: 7,
            observed_balance_lamports: 1_207,
            maximum_calls: 10,
            maximum_lamports_per_call: 100,
        };
        let recovery = admit_failure_market_recovery_funding_v1(
            &ExactRecovery(recovery_facts),
            binding,
            recovery_facts,
        )
        .unwrap();
        let root_facts = FailureMarketRootFundingFactsV1 {
            failure_policy_binding_id: binding.id(),
            prepaid_debit_receipt_id: FailureMarketPrepaidDebitReceiptIdV1::from_bytes([91; 32]),
            root_account_id: FailureMarketAccountIdV1::from_bytes([27; 32]),
            rent_payer: FailureMarketAccountIdV1::from_bytes([28; 32]),
            rent_principal_lamports: 3_000,
            donation_floor_lamports: 11,
            observed_balance_lamports: 3_011,
        };
        let root =
            admit_failure_market_root_funding_v1(&ExactRoot(root_facts), binding, root_facts)
                .unwrap();
        (
            FailureMarketAdmissionStateV1::from_receipts(binding, recovery, root).unwrap(),
            quote,
        )
    }

    fn runtime_root_funding() -> FailureMarketRuntimeRootFundingFactsV1 {
        FailureMarketRuntimeRootFundingFactsV1 {
            rent_refund_owner: FailureMarketAccountIdV1::from_bytes([29; 32]),
            neutral_sink: FailureMarketAccountIdV1::from_bytes([30; 32]),
            rent_principal_lamports: 4_000,
            donation_floor_lamports: 13,
            observed_balance_lamports: 4_013,
        }
    }

    #[test]
    fn market_runtime_round_trips_and_refuses_root_alias_or_fake_authority() {
        let (admission, quote) = admission();
        let runtime_account = FailureMarketAccountIdV1::from_bytes(
            admission.binding().facts().recovery_state_id.bytes(),
        );
        let foundation_receipt = ProductContentId::from_bytes([92; 32]);
        let initial = admit_failure_market_runtime_v1(
            &Refusing,
            admission,
            runtime_account,
            foundation_receipt,
            runtime_root_funding(),
            schedule(),
            quote,
            RecoveryClock {
                slot: 1,
                unix_timestamp: 1,
                current_bucket: 1,
            },
        );
        assert_eq!(initial, Err(Error::BindingMismatch));

        let engine = ExternalRecoveryStateV1::admit(
            admission.binding().facts().market_instance_id,
            admission.binding().facts().recovery_policy_id,
            schedule(),
            quote,
            ExternalRecoveryAdmissionV1 {
                state_id: RecoveryIdentity::from_bytes(runtime_account.bytes()),
                generation: 1,
                funding: ExternalRecoveryFundingV1 {
                    policy_id: RecoveryIdentity::from_bytes(
                        admission.binding().facts().liveness_policy_id.bytes(),
                    ),
                    lifecycle_id: RecoveryIdentity::from_bytes(
                        admission.binding().facts().liveness_lifecycle_id.bytes(),
                    ),
                    recovery_account_id: RecoveryIdentity::from_bytes(
                        admission
                            .binding()
                            .facts()
                            .recovery_compartment_account_id
                            .bytes(),
                    ),
                    semantic_owner: RecoveryIdentity::from_bytes(
                        admission
                            .binding()
                            .facts()
                            .recovery_receipt_program_id
                            .bytes(),
                    ),
                    payer: RecoveryIdentity::from_bytes(
                        admission.binding().facts().recovery_refund_owner.bytes(),
                    ),
                    neutral_sink: RecoveryIdentity::from_bytes(
                        admission.binding().facts().neutral_sink.bytes(),
                    ),
                    receipt_program_id: RecoveryIdentity::from_bytes(
                        admission
                            .binding()
                            .facts()
                            .recovery_receipt_program_id
                            .bytes(),
                    ),
                    quote_schedule_id: RecoveryIdentity::from_bytes(
                        admission
                            .binding()
                            .facts()
                            .recovery_quote_schedule_id
                            .bytes(),
                    ),
                    generation: 1,
                    capitalized_work_lamports: 1_000,
                    rent_principal_lamports: 200,
                    maximum_calls: 10,
                    maximum_lamports_per_call: 100,
                },
            },
            RecoveryClock {
                slot: 1,
                unix_timestamp: 1,
                current_bucket: 1,
            },
        )
        .unwrap();
        let expected_runtime = FailureMarketRuntimeV1 {
            policy_binding_id: admission.binding().id(),
            admission_state_id: admission.id().unwrap(),
            runtime_account_id: runtime_account,
            foundation_receipt_id: foundation_receipt,
            root_funding: runtime_root_funding(),
            recovery: engine,
            phase: FailureMarketRuntimePhaseV1::Ready,
            transition_sequence: 0,
            session_ids: [ProductContentId::ZERO; SESSION_ID_COUNT_V1],
        };
        let expected = FailureMarketRuntimeAdmissionFactsV1 {
            failure_policy_binding_id: admission.binding().id(),
            market_instance_id: admission.binding().facts().market_instance_id,
            generation: 1,
            admission_state_id: admission.id().unwrap(),
            runtime_account_id: runtime_account,
            foundation_receipt_id: foundation_receipt,
            root_funding: runtime_root_funding(),
            recovery_funding_receipt_id: admission.recovery_funding().id(),
            recovery_schedule_id: schedule_id(schedule()).unwrap(),
            recovery_funding_quote_id: engine.funding_quote_id(),
            runtime_state_commitment: expected_runtime.commitment().unwrap(),
        };
        let (runtime, receipt) = admit_failure_market_runtime_v1(
            &ExactRuntime(expected),
            admission,
            runtime_account,
            foundation_receipt,
            runtime_root_funding(),
            schedule(),
            quote,
            RecoveryClock {
                slot: 1,
                unix_timestamp: 1,
                current_bucket: 1,
            },
        )
        .unwrap();
        assert_eq!(receipt.facts(), expected);
        let mut encoded = [0; FAILURE_MARKET_RUNTIME_BYTES_V1];
        runtime.encode_into(&mut encoded).unwrap();
        assert_eq!(
            FailureMarketRuntimeV1::decode_for_admission(&encoded, admission),
            Ok(runtime)
        );
        encoded[FAILURE_MARKET_RUNTIME_BYTES_V1 - 1] = 1;
        assert_eq!(
            FailureMarketRuntimeV1::decode_for_admission(&encoded, admission),
            Err(Error::NonCanonicalReserved)
        );

        assert_eq!(
            admit_failure_market_runtime_v1(
                &ExactRuntime(expected),
                admission,
                admission.root_funding().facts().root_account_id,
                foundation_receipt,
                runtime_root_funding(),
                schedule(),
                quote,
                RecoveryClock {
                    slot: 1,
                    unix_timestamp: 1,
                    current_bucket: 1,
                },
            ),
            Err(Error::BindingMismatch)
        );
    }
}
