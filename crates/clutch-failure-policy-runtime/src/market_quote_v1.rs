// SPDX-License-Identifier: AGPL-3.0-or-later
//! Market-scoped exact Failure recovery quote schedule.
//!
//! Series quotes fund local admission and attachments; they do not price the
//! shared Market Recovery compartment. This content-addressed schedule is the
//! sole semantic owner of shared attempt pricing. Its ID must equal the exact
//! Recovery `quote_schedule_id` authenticated from liveness policy/account
//! bodies. Absolute attempt windows remain per-Series session data.

use clutch_product_series::{
    EvidenceOnlyRecoveryPolicyId, RecoveryAttemptFundingV1, MAX_RECOVERY_ATTEMPTS,
};
use sha2::{Digest, Sha256};

use crate::market_policy_v1::{
    FailureMarketPolicyBindingV1, FailureMarketRecoveryFundingReceiptV1,
};
use crate::{Error, FailurePolicyBindingId, Result};

const QUOTE_MAGIC_V1: [u8; 8] = *b"DCFMRQ01";
const QUOTE_VERSION_V1: u16 = 1;
const QUOTE_ID_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-market-recovery-quote/v1";
const QUOTE_ADMISSION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/failure-market-recovery-quote-admission/v1";

/// Exact canonical market quote bytes.
pub const FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1: usize = 192;

macro_rules! quote_id {
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

quote_id!(
    FailureMarketRecoveryQuoteScheduleIdV1,
    "Typed identity of one market-scoped exact recovery quote schedule."
);
quote_id!(
    FailureMarketRecoveryQuoteAdmissionReceiptIdV1,
    "Typed identity of one liveness-funded market quote admission."
);

/// Sole market-level exact reward schedule for finite recovery work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryQuoteScheduleV1 {
    /// Immutable evidence-only recovery policy being priced.
    pub recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Active attempt pricing count.
    pub attempt_count: u8,
    /// Active rows followed by canonical zero padding.
    pub attempts: [RecoveryAttemptFundingV1; MAX_RECOVERY_ATTEMPTS],
    /// Finite global call bound enforced by liveness.
    pub maximum_calls: u32,
    /// Largest progress delta admitted by one bounded call.
    pub maximum_progress_units_per_call: u64,
}

impl FailureMarketRecoveryQuoteScheduleV1 {
    /// Validate the exhaustive active prefix and all exact arithmetic.
    pub fn validate(self) -> Result<()> {
        self.recovery_policy_id.validate()?;
        let count = usize::from(self.attempt_count);
        if count == 0
            || count > MAX_RECOVERY_ATTEMPTS
            || self.maximum_calls == 0
            || self.maximum_progress_units_per_call == 0
        {
            return Err(Error::BindingMismatch);
        }
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let row = self.attempts[index];
            if index < count {
                if row.max_progress_units == 0 || row.lamports_per_progress_unit == 0 {
                    return Err(Error::BindingMismatch);
                }
                row.max_progress_units
                    .checked_mul(row.lamports_per_progress_unit)
                    .ok_or(Error::BindingMismatch)?;
            } else if row != RecoveryAttemptFundingV1::ZERO {
                return Err(Error::NonCanonicalReserved);
            }
            index += 1;
        }
        let work_principal_lamports = self.work_principal_lamports()?;
        let maximum_lamports_per_call = self.maximum_lamports_per_call()?;
        let required_calls = self.required_calls_for_full_progress()?;
        let total_call_capacity = u64::from(self.maximum_calls)
            .checked_mul(maximum_lamports_per_call)
            .ok_or(Error::BindingMismatch)?;
        if self.maximum_calls < required_calls || total_call_capacity < work_principal_lamports {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    /// Minimum number of bounded calls needed to accept every attempt row's
    /// maximum progress. Lower-priced rows still consume their own calls.
    pub fn required_calls_for_full_progress(self) -> Result<u32> {
        if self.maximum_progress_units_per_call == 0 {
            return Err(Error::BindingMismatch);
        }
        let count = usize::from(self.attempt_count);
        let mut total = 0u64;
        let mut index = 0usize;
        while index < count {
            let progress = self.attempts[index].max_progress_units;
            if progress == 0 {
                return Err(Error::BindingMismatch);
            }
            let calls = progress
                .checked_sub(1)
                .ok_or(Error::BindingMismatch)?
                .checked_div(self.maximum_progress_units_per_call)
                .ok_or(Error::BindingMismatch)?
                .checked_add(1)
                .ok_or(Error::BindingMismatch)?;
            total = total.checked_add(calls).ok_or(Error::BindingMismatch)?;
            index += 1;
        }
        u32::try_from(total).map_err(|_| Error::BindingMismatch)
    }

    /// Exact maximum shared work principal across all attempts.
    pub fn work_principal_lamports(self) -> Result<u64> {
        let count = usize::from(self.attempt_count);
        let mut total = 0u64;
        let mut index = 0usize;
        while index < count {
            let row = self.attempts[index];
            total = total
                .checked_add(
                    row.max_progress_units
                        .checked_mul(row.lamports_per_progress_unit)
                        .ok_or(Error::BindingMismatch)?,
                )
                .ok_or(Error::BindingMismatch)?;
            index += 1;
        }
        if total == 0 {
            return Err(Error::BindingMismatch);
        }
        Ok(total)
    }

    /// Exact maximum reward/ceiling for any one bounded call.
    pub fn maximum_lamports_per_call(self) -> Result<u64> {
        let count = usize::from(self.attempt_count);
        let mut maximum = 0u64;
        let mut index = 0usize;
        while index < count {
            let row = self.attempts[index];
            let progress = if row.max_progress_units < self.maximum_progress_units_per_call {
                row.max_progress_units
            } else {
                self.maximum_progress_units_per_call
            };
            let reward = progress
                .checked_mul(row.lamports_per_progress_unit)
                .ok_or(Error::BindingMismatch)?;
            if reward > maximum {
                maximum = reward;
            }
            index += 1;
        }
        if maximum == 0 {
            return Err(Error::BindingMismatch);
        }
        Ok(maximum)
    }

    /// Derive the only accepted exact reward and liveness debit ceiling.
    ///
    /// The ceiling equals the reward. This removes discretionary headroom and
    /// therefore produces no per-call payer refund branch.
    pub fn exact_progress_reward_lamports(
        self,
        attempt_index: u8,
        accepted_progress_before: u64,
        accepted_progress_after: u64,
    ) -> Result<u64> {
        self.validate()?;
        let index = usize::from(attempt_index);
        if index >= usize::from(self.attempt_count)
            || accepted_progress_after <= accepted_progress_before
        {
            return Err(Error::BindingMismatch);
        }
        let delta = accepted_progress_after
            .checked_sub(accepted_progress_before)
            .ok_or(Error::BindingMismatch)?;
        let row = self.attempts[index];
        if accepted_progress_after > row.max_progress_units
            || delta > self.maximum_progress_units_per_call
        {
            return Err(Error::BindingMismatch);
        }
        delta
            .checked_mul(row.lamports_per_progress_unit)
            .ok_or(Error::BindingMismatch)
    }

    /// Exact unspent work principal returned by the liveness runtime to its
    /// immutable payer at terminal close. Per-call exact-reward debits never
    /// reclassify this residue as a donation or Failure-owned balance.
    pub fn refundable_unused_work_principal_lamports(
        self,
        paid_rewards_lamports: u64,
    ) -> Result<u64> {
        self.validate()?;
        self.work_principal_lamports()?
            .checked_sub(paid_rewards_lamports)
            .ok_or(Error::BindingMismatch)
    }

    /// Encode every semantic byte and canonical zero padding.
    pub fn encode_into(
        self,
        output: &mut [u8; FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1],
    ) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[..8].copy_from_slice(&QUOTE_MAGIC_V1);
        output[8..10].copy_from_slice(&QUOTE_VERSION_V1.to_le_bytes());
        output[10] = self.attempt_count;
        output[16..48].copy_from_slice(&self.recovery_policy_id.bytes());
        output[48..52].copy_from_slice(&self.maximum_calls.to_le_bytes());
        output[56..64].copy_from_slice(&self.maximum_progress_units_per_call.to_le_bytes());
        let mut cursor = 64usize;
        for row in self.attempts {
            output[cursor..cursor + 8].copy_from_slice(&row.max_progress_units.to_le_bytes());
            cursor += 8;
            output[cursor..cursor + 8]
                .copy_from_slice(&row.lamports_per_progress_unit.to_le_bytes());
            cursor += 8;
        }
        Ok(())
    }

    /// Hostile-decode one exact canonical body.
    pub fn decode(input: &[u8; FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1]) -> Result<Self> {
        if input[..8] != QUOTE_MAGIC_V1 {
            return Err(Error::BadMagic);
        }
        if input[8..10] != QUOTE_VERSION_V1.to_le_bytes() {
            return Err(Error::BadVersion);
        }
        if input[11..16].iter().any(|byte| *byte != 0)
            || input[52..56].iter().any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalReserved);
        }
        let attempt_count = input[10];
        let recovery_policy_id = EvidenceOnlyRecoveryPolicyId::from_bytes(
            input[16..48].try_into().map_err(|_| Error::WrongLength)?,
        );
        let maximum_calls =
            u32::from_le_bytes(input[48..52].try_into().map_err(|_| Error::WrongLength)?);
        let maximum_progress_units_per_call =
            u64::from_le_bytes(input[56..64].try_into().map_err(|_| Error::WrongLength)?);
        let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        let mut cursor = 64usize;
        for row in &mut attempts {
            row.max_progress_units = u64::from_le_bytes(
                input[cursor..cursor + 8]
                    .try_into()
                    .map_err(|_| Error::WrongLength)?,
            );
            cursor += 8;
            row.lamports_per_progress_unit = u64::from_le_bytes(
                input[cursor..cursor + 8]
                    .try_into()
                    .map_err(|_| Error::WrongLength)?,
            );
            cursor += 8;
        }
        let value = Self {
            recovery_policy_id,
            attempt_count,
            attempts,
            maximum_calls,
            maximum_progress_units_per_call,
        };
        value.validate()?;
        Ok(value)
    }

    /// Typed content identity of the exact canonical body.
    pub fn id(self) -> Result<FailureMarketRecoveryQuoteScheduleIdV1> {
        let mut body = [0u8; FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1];
        self.encode_into(&mut body)?;
        let mut hasher = Sha256::new();
        hasher.update(QUOTE_ID_DOMAIN_V1);
        hasher.update(body);
        Ok(FailureMarketRecoveryQuoteScheduleIdV1::from_bytes(
            hasher.finalize().into(),
        ))
    }
}

/// Complete expected liveness/Failure quote join. This is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryQuoteAdmissionFactsV1 {
    /// Immutable Market Failure policy.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Exact market-scoped schedule.
    pub quote_schedule_id: FailureMarketRecoveryQuoteScheduleIdV1,
    /// Finite global liveness call bound.
    pub maximum_calls: u32,
    /// Exact maximum one-call reward/ceiling.
    pub maximum_lamports_per_call: u64,
    /// Exact present shared work principal.
    pub work_principal_lamports: u64,
}

/// Private adapter authority for exact schedule body and liveness policy/account joins.
pub trait AuthenticatedFailureMarketRecoveryQuoteV1 {
    /// Authenticate exact quote bytes and both persisted liveness bodies.
    fn authenticate_failure_market_recovery_quote(
        &self,
        _expected: FailureMarketRecoveryQuoteAdmissionFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field admitted schedule receipt consumed by subordinate sessions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryQuoteAdmissionReceiptV1 {
    id: FailureMarketRecoveryQuoteAdmissionReceiptIdV1,
    facts: FailureMarketRecoveryQuoteAdmissionFactsV1,
    schedule: FailureMarketRecoveryQuoteScheduleV1,
}

impl FailureMarketRecoveryQuoteAdmissionReceiptV1 {
    /// Exact admission receipt identity.
    pub const fn id(self) -> FailureMarketRecoveryQuoteAdmissionReceiptIdV1 {
        self.id
    }

    /// Exact authenticated join facts.
    pub const fn facts(self) -> FailureMarketRecoveryQuoteAdmissionFactsV1 {
        self.facts
    }

    /// Exact market-scoped reward schedule.
    pub const fn schedule(self) -> FailureMarketRecoveryQuoteScheduleV1 {
        self.schedule
    }
}

/// Admit the sole shared reward schedule against immutable Failure/liveness funding.
pub fn admit_failure_market_recovery_quote_v1<
    A: AuthenticatedFailureMarketRecoveryQuoteV1 + ?Sized,
>(
    authority: &A,
    binding: FailureMarketPolicyBindingV1,
    funding: FailureMarketRecoveryFundingReceiptV1,
    schedule: FailureMarketRecoveryQuoteScheduleV1,
) -> Result<FailureMarketRecoveryQuoteAdmissionReceiptV1> {
    schedule.validate()?;
    let policy = binding.facts();
    let funded = funding.facts();
    let quote_schedule_id = schedule.id()?;
    let work_principal_lamports = schedule.work_principal_lamports()?;
    let maximum_lamports_per_call = schedule.maximum_lamports_per_call()?;
    let facts = FailureMarketRecoveryQuoteAdmissionFactsV1 {
        failure_policy_binding_id: binding.id(),
        quote_schedule_id,
        maximum_calls: schedule.maximum_calls,
        maximum_lamports_per_call,
        work_principal_lamports,
    };
    if schedule.recovery_policy_id != policy.recovery_policy_id
        || quote_schedule_id.bytes() != policy.recovery_quote_schedule_id.bytes()
        || funded.failure_policy_binding_id != binding.id()
        || funded.recovery_quote_schedule_id.bytes() != quote_schedule_id.bytes()
        || funded.maximum_calls != schedule.maximum_calls
        || funded.maximum_lamports_per_call != maximum_lamports_per_call
        || funded.work_principal_lamports != work_principal_lamports
    {
        return Err(Error::BindingMismatch);
    }
    authority.authenticate_failure_market_recovery_quote(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(QUOTE_ADMISSION_DOMAIN_V1);
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.quote_schedule_id.bytes());
    hasher.update(facts.maximum_calls.to_le_bytes());
    hasher.update(facts.maximum_lamports_per_call.to_le_bytes());
    hasher.update(facts.work_principal_lamports.to_le_bytes());
    let id = FailureMarketRecoveryQuoteAdmissionReceiptIdV1::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    Ok(FailureMarketRecoveryQuoteAdmissionReceiptV1 {
        id,
        facts,
        schedule,
    })
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule() -> FailureMarketRecoveryQuoteScheduleV1 {
        let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 20,
            lamports_per_progress_unit: 5,
        };
        attempts[1] = RecoveryAttemptFundingV1 {
            max_progress_units: 10,
            lamports_per_progress_unit: 7,
        };
        FailureMarketRecoveryQuoteScheduleV1 {
            recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes([1; 32]),
            attempt_count: 2,
            attempts,
            maximum_calls: 8,
            maximum_progress_units_per_call: 4,
        }
    }

    #[test]
    fn exact_reward_is_the_only_ceiling_and_padding_is_hostile() {
        let schedule = schedule();
        assert_eq!(schedule.work_principal_lamports(), Ok(170));
        assert_eq!(schedule.maximum_lamports_per_call(), Ok(28));
        assert_eq!(schedule.required_calls_for_full_progress(), Ok(8));
        assert_eq!(
            schedule.refundable_unused_work_principal_lamports(42),
            Ok(128)
        );
        assert_eq!(schedule.exact_progress_reward_lamports(1, 2, 6), Ok(28));
        assert_eq!(
            schedule.exact_progress_reward_lamports(1, 2, 7),
            Err(Error::BindingMismatch)
        );
        let mut body = [0u8; FAILURE_MARKET_RECOVERY_QUOTE_SCHEDULE_BYTES_V1];
        schedule.encode_into(&mut body).unwrap();
        assert_eq!(
            FailureMarketRecoveryQuoteScheduleV1::decode(&body),
            Ok(schedule)
        );
        body[11] = 1;
        assert_eq!(
            FailureMarketRecoveryQuoteScheduleV1::decode(&body),
            Err(Error::NonCanonicalReserved)
        );

        let mut noncanonical = schedule;
        noncanonical.attempts[2] = RecoveryAttemptFundingV1 {
            max_progress_units: 1,
            lamports_per_progress_unit: 1,
        };
        assert_eq!(noncanonical.validate(), Err(Error::NonCanonicalReserved));

        let mut undercalled = schedule;
        undercalled.maximum_calls = 7;
        assert_eq!(undercalled.validate(), Err(Error::BindingMismatch));
    }
}
