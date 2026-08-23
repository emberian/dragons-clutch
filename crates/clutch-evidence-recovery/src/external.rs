// SPDX-License-Identifier: AGPL-3.0-or-later
//! Semantic evidence recovery whose native-lamport custody is owned elsewhere.
//!
//! This successor state never observes an account balance and never emits a
//! transfer. It owns only the finite schedule, monotone accepted-progress
//! cursor, exact reward obligation, terminal classification, and replay nonce.
//! A separately authenticated liveness compartment is the only work/rent
//! custodian and applies every movement atomically with these state writes.

use clutch_product_series::{
    AbsoluteRecoveryAttemptV1, CompiledScheduleV1, EvidenceOnlyRecoveryPolicyId,
    MarketInstanceV2Id, RecoveryAttemptFundingV1, SeriesFundingQuoteId, SeriesFundingQuoteV1,
    MAX_RECOVERY_ATTEMPTS, SERIES_FUNDING_QUOTE_BYTES,
};

use crate::{
    validate_funding_quote_projection, validate_schedule, EvidenceDecision, Identity,
    RecoveryClock, RecoveryError, RecoveryPhase, Result,
};

const MAGIC: [u8; 8] = *b"DCEXREC1";
const VERSION: u16 = 1;

/// Exact encoded width of [`ExternalRecoveryStateV1`].
pub const EXTERNAL_RECOVERY_STATE_V1_BYTES: usize = 1_176;

/// Authenticated immutable liveness facts admitted by the semantic owner.
///
/// The adapter constructs this only after decoding the funded Recovery
/// compartment under its checked liveness program owner. Amounts describe
/// that external account; they are never balances of `state_id`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRecoveryFundingV1 {
    /// Immutable runtime-liveness policy.
    pub policy_id: Identity,
    /// Exact occurrence lifecycle shared by the seven compartments.
    pub lifecycle_id: Identity,
    /// Persisted Recovery compartment account; distinct from semantic state.
    pub recovery_account_id: Identity,
    /// Failure/recovery semantic owner named by liveness.
    pub semantic_owner: Identity,
    /// Immutable principal payer and work-headroom refund recipient.
    pub payer: Identity,
    /// Immutable donation/failure residue sink.
    pub neutral_sink: Identity,
    /// Checked program owner required on semantic receipt accounts.
    pub receipt_program_id: Identity,
    /// Exact semantic quote schedule frozen into the Recovery compartment.
    pub quote_schedule_id: Identity,
    /// Nonzero compartment generation.
    pub generation: u64,
    /// Present work capital held only in the Recovery compartment.
    pub capitalized_work_lamports: u64,
    /// Present rent principal held only in the Recovery compartment.
    pub rent_principal_lamports: u64,
    /// Finite number of independently capitalized calls.
    pub maximum_calls: u32,
    /// Per-call ceiling independently enforced by liveness.
    pub maximum_lamports_per_call: u64,
}

/// Immutable recovery occurrence plus its externally funded custody binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRecoveryAdmissionV1 {
    /// Durable semantic-state account identity; never a lamport work reserve.
    pub state_id: Identity,
    /// Nonzero semantic generation, equal to liveness generation.
    pub generation: u64,
    /// Exact externally owned present-funding facts.
    pub funding: ExternalRecoveryFundingV1,
}

/// Exact required liveness movement facts for one accepted progress advance.
///
/// This is evidence, not an executable transfer plan. The outer adapter must
/// prove exact equality with the liveness `SpendWork` transition and commit
/// both state writes in one atomic batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRecoveryWorkAuthorizationV1 {
    /// Unique upstream work/evidence identity accepted by this semantic owner.
    pub work_id: Identity,
    /// Recipient that liveness must name as its keeper.
    pub reward_recipient: Identity,
    /// Zero-based recovery attempt.
    pub attempt_index: u8,
    /// One-based liveness call ordinal.
    pub call_ordinal: u32,
    /// New cumulative accepted progress for the attempt.
    pub accepted_progress_total: u64,
    /// Strictly positive newly accepted progress.
    pub accepted_progress_delta: u64,
    /// Exact semantic reward, equal to liveness keeper payment.
    pub exact_reward_lamports: u64,
    /// Exact liveness debit ceiling; unused headroom returns to its payer.
    pub scheduled_ceiling_lamports: u64,
}

/// Complete semantic recovery state with no physical-funding ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRecoveryStateV1 {
    market_instance_id: MarketInstanceV2Id,
    recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    schedule: CompiledScheduleV1,
    funding_quote_id: SeriesFundingQuoteId,
    funding_quote: SeriesFundingQuoteV1,
    state_id: Identity,
    generation: u64,
    funding: ExternalRecoveryFundingV1,
    phase: RecoveryPhase,
    transition_nonce: u64,
    last_clock: RecoveryClock,
    next_attempt_index: u8,
    accepted_progress_units: [u64; MAX_RECOVERY_ATTEMPTS],
    last_work_id: Identity,
    last_reward_recipient: Identity,
    last_work_attempt_index: u8,
    resolution_evidence_id: Identity,
    completed_work_calls: u32,
    authorized_ceiling_lamports: u64,
    exact_reward_lamports: u64,
}

impl ExternalRecoveryStateV1 {
    /// Admit a V2 occurrence after the adapter authenticated one presently
    /// funded, persisted liveness Recovery compartment.
    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        market_instance_id: MarketInstanceV2Id,
        recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
        schedule: CompiledScheduleV1,
        funding_quote: SeriesFundingQuoteV1,
        admission: ExternalRecoveryAdmissionV1,
        creation_clock: RecoveryClock,
    ) -> Result<Self> {
        market_instance_id
            .validate()
            .map_err(|_| RecoveryError::ZeroIdentity)?;
        recovery_policy_id
            .validate()
            .map_err(|_| RecoveryError::ZeroIdentity)?;
        validate_schedule(&schedule)?;
        validate_funding_quote_projection(&funding_quote, recovery_policy_id, &schedule)?;
        let funding_quote_id = funding_quote.id().map_err(super::map_funding_quote_error)?;
        let work = funding_quote
            .recovery_work_principal_lamports()
            .map_err(super::map_funding_quote_error)?;
        let funding = admission.funding;
        for identity in [
            admission.state_id,
            funding.policy_id,
            funding.lifecycle_id,
            funding.recovery_account_id,
            funding.semantic_owner,
            funding.payer,
            funding.neutral_sink,
            funding.receipt_program_id,
            funding.quote_schedule_id,
        ] {
            require_live(identity)?;
        }
        if admission.generation == 0
            || admission.generation != funding.generation
            || creation_clock.current_bucket >= schedule.primary_maturity_bucket_exclusive
            || funding.quote_schedule_id.bytes() != funding_quote_id.bytes()
            || funding.capitalized_work_lamports != work
            || funding.rent_principal_lamports != funding_quote.recovery_rent_principal_lamports
            || funding.maximum_calls == 0
            || funding.maximum_lamports_per_call == 0
            || funding.receipt_program_id != funding.semantic_owner
            || admission.state_id == funding.recovery_account_id
            || admission.state_id == funding.semantic_owner
            || admission.state_id == funding.payer
            || admission.state_id == funding.neutral_sink
            || funding.recovery_account_id == funding.semantic_owner
            || funding.recovery_account_id == funding.payer
            || funding.recovery_account_id == funding.neutral_sink
            || funding.payer == funding.neutral_sink
        {
            return Err(RecoveryError::ExternalCustodyMismatch);
        }
        let value = Self {
            market_instance_id,
            recovery_policy_id,
            schedule,
            funding_quote_id,
            funding_quote,
            state_id: admission.state_id,
            generation: admission.generation,
            funding,
            phase: RecoveryPhase::Active,
            transition_nonce: 0,
            last_clock: creation_clock,
            next_attempt_index: 0,
            accepted_progress_units: [0; MAX_RECOVERY_ATTEMPTS],
            last_work_id: Identity::ZERO,
            last_reward_recipient: Identity::ZERO,
            last_work_attempt_index: 0,
            resolution_evidence_id: Identity::ZERO,
            completed_work_calls: 0,
            authorized_ceiling_lamports: 0,
            exact_reward_lamports: 0,
        };
        value.check()?;
        Ok(value)
    }

    /// Exact full-width market identity.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact evidence-only policy identity.
    pub const fn recovery_policy_id(&self) -> EvidenceOnlyRecoveryPolicyId {
        self.recovery_policy_id
    }

    /// Exact Product/Series funding quote identity.
    pub const fn funding_quote_id(&self) -> SeriesFundingQuoteId {
        self.funding_quote_id
    }

    /// Durable semantic state account, which never holds work capital.
    pub const fn state_id(&self) -> Identity {
        self.state_id
    }

    /// Exact semantic/liveness generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Immutable external custody facts.
    pub const fn funding(&self) -> ExternalRecoveryFundingV1 {
        self.funding
    }

    /// Current semantic recovery phase.
    pub const fn phase(&self) -> RecoveryPhase {
        self.phase
    }

    /// Monotone semantic transition nonce.
    pub const fn transition_nonce(&self) -> u64 {
        self.transition_nonce
    }

    /// Last admitted authenticated Clock projection.
    pub const fn last_clock(&self) -> RecoveryClock {
        self.last_clock
    }

    /// First compiled attempt not yet closed by authenticated bucket time.
    pub const fn next_attempt_index(&self) -> u8 {
        self.next_attempt_index
    }

    /// Immutable absolute schedule.
    pub const fn schedule(&self) -> CompiledScheduleV1 {
        self.schedule
    }

    /// Current attempt, if one remains.
    pub fn current_attempt(&self) -> Result<Option<AbsoluteRecoveryAttemptV1>> {
        self.check()?;
        let index = usize::from(self.next_attempt_index);
        if index < usize::from(self.schedule.recovery_attempt_count) {
            Ok(Some(self.schedule.recovery_attempts[index]))
        } else {
            Ok(None)
        }
    }

    /// Cumulative accepted progress in one active schedule row.
    pub fn accepted_progress_units(&self, attempt_index: u8) -> Result<u64> {
        self.check()?;
        let index = usize::from(attempt_index);
        if index >= usize::from(self.schedule.recovery_attempt_count) {
            return Err(RecoveryError::ProjectionMismatch);
        }
        Ok(self.accepted_progress_units[index])
    }

    /// Exact Product/Series price row for one attempt.
    pub fn attempt_funding(&self, attempt_index: u8) -> Result<RecoveryAttemptFundingV1> {
        self.check()?;
        let index = usize::from(attempt_index);
        if index >= usize::from(self.schedule.recovery_attempt_count) {
            return Err(RecoveryError::ProjectionMismatch);
        }
        Ok(self.funding_quote.recovery_attempt_funding[index])
    }

    /// Total externally authorized call ceilings so far.
    pub const fn authorized_ceiling_lamports(&self) -> u64 {
        self.authorized_ceiling_lamports
    }

    /// Total exact keeper reward obligations so far.
    pub const fn exact_reward_lamports(&self) -> u64 {
        self.exact_reward_lamports
    }

    /// Number of exact work receipts already emitted.
    pub const fn completed_work_calls(&self) -> u32 {
        self.completed_work_calls
    }

    /// Opaque accepted-resolution identity, if resolved.
    pub fn resolution_evidence_id(&self) -> Option<Identity> {
        if self.resolution_evidence_id.is_zero() {
            None
        } else {
            Some(self.resolution_evidence_id)
        }
    }

    /// Refuse exposure once immutable primary maturity is reached, even if no
    /// keeper has yet recorded the failure trigger.
    pub fn check_new_exposure(&self, clock: RecoveryClock) -> Result<()> {
        self.check()?;
        self.validate_next_clock(clock)?;
        if self.phase != RecoveryPhase::Active
            || clock.current_bucket >= self.schedule.primary_maturity_bucket_exclusive
        {
            return Err(RecoveryError::ExposureClosed);
        }
        Ok(())
    }

    /// Validate the complete reachable semantic state and external binding.
    pub fn check(&self) -> Result<()> {
        self.market_instance_id
            .validate()
            .map_err(|_| RecoveryError::InvariantViolation)?;
        self.recovery_policy_id
            .validate()
            .map_err(|_| RecoveryError::InvariantViolation)?;
        validate_schedule(&self.schedule)?;
        validate_funding_quote_projection(
            &self.funding_quote,
            self.recovery_policy_id,
            &self.schedule,
        )?;
        if self
            .funding_quote
            .id()
            .map_err(super::map_funding_quote_error)?
            != self.funding_quote_id
        {
            return Err(RecoveryError::ProjectionMismatch);
        }
        let quoted_work = self
            .funding_quote
            .recovery_work_principal_lamports()
            .map_err(super::map_funding_quote_error)?;
        for identity in [
            self.state_id,
            self.funding.policy_id,
            self.funding.lifecycle_id,
            self.funding.recovery_account_id,
            self.funding.semantic_owner,
            self.funding.payer,
            self.funding.neutral_sink,
            self.funding.receipt_program_id,
            self.funding.quote_schedule_id,
        ] {
            require_live(identity)?;
        }
        if self.generation == 0
            || self.generation != self.funding.generation
            || self.funding.quote_schedule_id.bytes() != self.funding_quote_id.bytes()
            || self.funding.capitalized_work_lamports != quoted_work
            || self.funding.rent_principal_lamports
                != self.funding_quote.recovery_rent_principal_lamports
            || self.funding.maximum_calls == 0
            || self.funding.maximum_lamports_per_call == 0
            || self.funding.receipt_program_id != self.funding.semantic_owner
            || self.state_id == self.funding.recovery_account_id
            || self.state_id == self.funding.semantic_owner
            || self.state_id == self.funding.payer
            || self.state_id == self.funding.neutral_sink
            || self.funding.recovery_account_id == self.funding.semantic_owner
            || self.funding.recovery_account_id == self.funding.payer
            || self.funding.recovery_account_id == self.funding.neutral_sink
            || self.funding.payer == self.funding.neutral_sink
            || self.completed_work_calls > self.funding.maximum_calls
            || self.authorized_ceiling_lamports > self.funding.capitalized_work_lamports
            || self.exact_reward_lamports > self.authorized_ceiling_lamports
        {
            return Err(RecoveryError::InvariantViolation);
        }
        let count = usize::from(self.schedule.recovery_attempt_count);
        let cursor = usize::from(self.next_attempt_index);
        if cursor > count {
            return Err(RecoveryError::InvariantViolation);
        }
        let mut expected_reward = 0u64;
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let progress = self.accepted_progress_units[index];
            if index < count {
                let terms = self.funding_quote.recovery_attempt_funding[index];
                if progress > terms.max_progress_units {
                    return Err(RecoveryError::InvariantViolation);
                }
                expected_reward = checked_add(
                    expected_reward,
                    progress
                        .checked_mul(terms.lamports_per_progress_unit)
                        .ok_or(RecoveryError::ArithmeticOverflow)?,
                )?;
            } else if progress != 0 {
                return Err(RecoveryError::InvariantViolation);
            }
            index += 1;
        }
        if expected_reward != self.exact_reward_lamports {
            return Err(RecoveryError::InvariantViolation);
        }
        index = 0;
        while index < cursor {
            if self.last_clock.current_bucket
                < self.schedule.recovery_attempts[index].closes_at_bucket
            {
                return Err(RecoveryError::InvariantViolation);
            }
            index += 1;
        }
        if self.completed_work_calls == 0 {
            if !self.last_work_id.is_zero() || !self.last_reward_recipient.is_zero() {
                return Err(RecoveryError::InvariantViolation);
            }
        } else if self.last_work_id.is_zero()
            || self.last_reward_recipient.is_zero()
            || usize::from(self.last_work_attempt_index) >= count
            || self.last_reward_recipient == self.state_id
            || self.last_reward_recipient == self.funding.recovery_account_id
            || self.last_reward_recipient == self.funding.neutral_sink
        {
            return Err(RecoveryError::InvariantViolation);
        }
        match self.phase {
            RecoveryPhase::Active => {
                if cursor != 0
                    || self.last_clock.current_bucket
                        >= self.schedule.primary_maturity_bucket_exclusive
                    || !self.resolution_evidence_id.is_zero()
                    || self.completed_work_calls != 0
                {
                    return Err(RecoveryError::InvariantViolation);
                }
            }
            RecoveryPhase::DegradedRecoverable => {
                let attempt = self
                    .schedule
                    .recovery_attempts
                    .get(cursor)
                    .ok_or(RecoveryError::InvariantViolation)?;
                if self.last_clock.current_bucket < self.schedule.primary_maturity_bucket_exclusive
                    || self.last_clock.current_bucket >= attempt.closes_at_bucket
                    || !self.resolution_evidence_id.is_zero()
                {
                    return Err(RecoveryError::InvariantViolation);
                }
            }
            RecoveryPhase::RecoveryDormant => {
                if cursor != count || !self.resolution_evidence_id.is_zero() {
                    return Err(RecoveryError::InvariantViolation);
                }
            }
            RecoveryPhase::Resolved => {
                if self.resolution_evidence_id.is_zero() {
                    return Err(RecoveryError::InvariantViolation);
                }
            }
        }
        Ok(())
    }

    /// Record deterministic degradation at immutable maturity.
    pub fn plan_enter_degraded(
        &self,
        clock: RecoveryClock,
    ) -> Result<ExternalRecoveryTransitionPlanV1> {
        self.check()?;
        if self.phase != RecoveryPhase::Active {
            return Err(RecoveryError::WrongPhase);
        }
        self.validate_next_clock(clock)?;
        if clock.current_bucket < self.schedule.primary_maturity_bucket_exclusive {
            return Err(RecoveryError::RecoveryNotOpen);
        }
        let mut next = *self;
        next.phase = RecoveryPhase::DegradedRecoverable;
        next.last_clock = clock;
        next.advance_expired_attempts()?;
        next.finish_plan(*self, None)
    }

    /// Advance the finite recovery schedule without touching liveness custody.
    pub fn plan_advance_schedule(
        &self,
        clock: RecoveryClock,
    ) -> Result<ExternalRecoveryTransitionPlanV1> {
        self.check()?;
        if self.phase != RecoveryPhase::DegradedRecoverable {
            return Err(RecoveryError::WrongPhase);
        }
        self.validate_next_clock(clock)?;
        let mut next = *self;
        next.last_clock = clock;
        next.advance_expired_attempts()?;
        next.finish_plan(*self, None)
    }

    /// Advance accepted progress and emit exact facts for one external
    /// liveness `SpendWork` transition.
    pub fn plan_accept_work_progress(
        &self,
        clock: RecoveryClock,
        work_id: Identity,
        reward_recipient: Identity,
        accepted_progress_total: u64,
        scheduled_ceiling_lamports: u64,
    ) -> Result<ExternalRecoveryTransitionPlanV1> {
        let (next, work) = self.advance_work(
            clock,
            work_id,
            reward_recipient,
            accepted_progress_total,
            scheduled_ceiling_lamports,
        )?;
        next.finish_plan(*self, Some(work))
    }

    /// Resolve from caller-funded accepted evidence. No liveness work movement
    /// is authorized by this transition.
    pub fn plan_resolve_caller_funded(
        &self,
        clock: RecoveryClock,
        evidence: EvidenceDecision,
    ) -> Result<ExternalRecoveryTransitionPlanV1> {
        self.check()?;
        self.validate_next_clock(clock)?;
        if self.phase == RecoveryPhase::Resolved {
            return Err(RecoveryError::WrongPhase);
        }
        let mut next = *self;
        next.last_clock = clock;
        if next.phase == RecoveryPhase::Active
            && clock.current_bucket >= next.schedule.primary_maturity_bucket_exclusive
        {
            next.phase = RecoveryPhase::DegradedRecoverable;
        }
        if next.phase == RecoveryPhase::DegradedRecoverable {
            next.advance_expired_attempts()?;
        }
        next.phase = RecoveryPhase::Resolved;
        next.resolution_evidence_id = evidence.identity();
        next.finish_plan(*self, None)
    }

    /// Atomically authorize final paid progress and record accepted evidence.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_resolve_paid_progress(
        &self,
        clock: RecoveryClock,
        work_id: Identity,
        reward_recipient: Identity,
        accepted_progress_total: u64,
        scheduled_ceiling_lamports: u64,
        evidence: EvidenceDecision,
    ) -> Result<ExternalRecoveryTransitionPlanV1> {
        let (mut next, work) = self.advance_work(
            clock,
            work_id,
            reward_recipient,
            accepted_progress_total,
            scheduled_ceiling_lamports,
        )?;
        next.phase = RecoveryPhase::Resolved;
        next.resolution_evidence_id = evidence.identity();
        next.finish_plan(*self, Some(work))
    }

    fn advance_work(
        &self,
        clock: RecoveryClock,
        work_id: Identity,
        reward_recipient: Identity,
        accepted_progress_total: u64,
        scheduled_ceiling_lamports: u64,
    ) -> Result<(Self, ExternalRecoveryWorkAuthorizationV1)> {
        self.check()?;
        if self.phase != RecoveryPhase::DegradedRecoverable {
            return Err(RecoveryError::WrongPhase);
        }
        self.validate_next_clock(clock)?;
        require_live(work_id)?;
        require_live(reward_recipient)?;
        if reward_recipient == self.state_id
            || reward_recipient == self.funding.recovery_account_id
            || reward_recipient == self.funding.neutral_sink
        {
            return Err(RecoveryError::InterestedNeutralSink);
        }
        let mut next = *self;
        next.last_clock = clock;
        next.advance_expired_attempts()?;
        let index = usize::from(next.next_attempt_index);
        let count = usize::from(next.schedule.recovery_attempt_count);
        if index >= count {
            return Err(RecoveryError::AttemptNotOpen);
        }
        let attempt = next.schedule.recovery_attempts[index];
        if clock.current_bucket < attempt.opens_at_bucket
            || clock.current_bucket >= attempt.closes_at_bucket
        {
            return Err(RecoveryError::AttemptNotOpen);
        }
        let prior = next.accepted_progress_units[index];
        if accepted_progress_total <= prior {
            return Err(RecoveryError::NonmonotoneProgress);
        }
        let terms = next.funding_quote.recovery_attempt_funding[index];
        if accepted_progress_total > terms.max_progress_units {
            return Err(RecoveryError::ProgressLimitExceeded);
        }
        let delta = accepted_progress_total - prior;
        let reward = delta
            .checked_mul(terms.lamports_per_progress_unit)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let call_ordinal = next
            .completed_work_calls
            .checked_add(1)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let new_ceiling =
            checked_add(next.authorized_ceiling_lamports, scheduled_ceiling_lamports)?;
        if scheduled_ceiling_lamports == 0
            || reward == 0
            || reward > scheduled_ceiling_lamports
            || scheduled_ceiling_lamports > next.funding.maximum_lamports_per_call
            || new_ceiling > next.funding.capitalized_work_lamports
        {
            return Err(RecoveryError::InvalidScheduledCeiling);
        }
        if call_ordinal > next.funding.maximum_calls {
            return Err(RecoveryError::ExternalCallBudgetExhausted);
        }
        next.accepted_progress_units[index] = accepted_progress_total;
        next.completed_work_calls = call_ordinal;
        next.authorized_ceiling_lamports = new_ceiling;
        next.exact_reward_lamports = checked_add(next.exact_reward_lamports, reward)?;
        next.last_work_id = work_id;
        next.last_reward_recipient = reward_recipient;
        next.last_work_attempt_index = next.next_attempt_index;
        Ok((
            next,
            ExternalRecoveryWorkAuthorizationV1 {
                work_id,
                reward_recipient,
                attempt_index: u8::try_from(index)
                    .map_err(|_| RecoveryError::ArithmeticOverflow)?,
                call_ordinal,
                accepted_progress_total,
                accepted_progress_delta: delta,
                exact_reward_lamports: reward,
                scheduled_ceiling_lamports,
            },
        ))
    }

    fn validate_next_clock(&self, clock: RecoveryClock) -> Result<()> {
        if clock.slot < self.last_clock.slot
            || clock.unix_timestamp < self.last_clock.unix_timestamp
            || clock.current_bucket < self.last_clock.current_bucket
        {
            return Err(RecoveryError::ClockMovedBackwards);
        }
        Ok(())
    }

    fn advance_expired_attempts(&mut self) -> Result<()> {
        let count = usize::from(self.schedule.recovery_attempt_count);
        let mut index = usize::from(self.next_attempt_index);
        while index < count
            && self.last_clock.current_bucket
                >= self.schedule.recovery_attempts[index].closes_at_bucket
        {
            index += 1;
        }
        self.next_attempt_index =
            u8::try_from(index).map_err(|_| RecoveryError::ArithmeticOverflow)?;
        if index == count {
            self.phase = RecoveryPhase::RecoveryDormant;
        }
        Ok(())
    }

    fn finish_plan(
        &mut self,
        before: Self,
        work: Option<ExternalRecoveryWorkAuthorizationV1>,
    ) -> Result<ExternalRecoveryTransitionPlanV1> {
        self.transition_nonce = before
            .transition_nonce
            .checked_add(1)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        self.check()?;
        Ok(ExternalRecoveryTransitionPlanV1 {
            before,
            after: *self,
            work,
        })
    }

    /// Commit one current semantic plan. Physical liveness state must be
    /// committed by the outer adapter in the same atomic instruction.
    pub fn commit_plan(&mut self, plan: ExternalRecoveryTransitionPlanV1) -> Result<()> {
        self.check()?;
        if *self != plan.before {
            return Err(RecoveryError::StalePlan);
        }
        plan.after.check()?;
        *self = plan.after;
        Ok(())
    }

    /// Encode the exact semantic state. No balance or transfer amount is read.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.check()?;
        let mut writer = Writer::new(output)?;
        writer.bytes(&MAGIC)?;
        writer.u16(VERSION)?;
        writer.reserved(6)?;
        writer.bytes(&self.market_instance_id.bytes())?;
        writer.bytes(&self.recovery_policy_id.bytes())?;
        encode_schedule(&mut writer, self.schedule)?;
        writer.bytes(&self.funding_quote_id.bytes())?;
        let mut quote = [0u8; SERIES_FUNDING_QUOTE_BYTES];
        clutch_product_series::FixedCodec::encode_into(&self.funding_quote, &mut quote)
            .map_err(super::map_funding_quote_error)?;
        writer.bytes(&quote)?;
        writer.bytes(&self.state_id.bytes())?;
        writer.u64(self.generation)?;
        for identity in [
            self.funding.policy_id,
            self.funding.lifecycle_id,
            self.funding.recovery_account_id,
            self.funding.semantic_owner,
            self.funding.payer,
            self.funding.neutral_sink,
            self.funding.receipt_program_id,
            self.funding.quote_schedule_id,
        ] {
            writer.bytes(&identity.bytes())?;
        }
        writer.u64(self.funding.generation)?;
        writer.u64(self.funding.capitalized_work_lamports)?;
        writer.u64(self.funding.rent_principal_lamports)?;
        writer.u32(self.funding.maximum_calls)?;
        writer.reserved(4)?;
        writer.u64(self.funding.maximum_lamports_per_call)?;
        writer.u8(phase_code(self.phase))?;
        writer.u8(self.next_attempt_index)?;
        writer.reserved(6)?;
        writer.u64(self.transition_nonce)?;
        writer.u64(self.last_clock.slot)?;
        writer.i64(self.last_clock.unix_timestamp)?;
        writer.u64(self.last_clock.current_bucket)?;
        for progress in self.accepted_progress_units {
            writer.u64(progress)?;
        }
        writer.bytes(&self.last_work_id.bytes())?;
        writer.bytes(&self.last_reward_recipient.bytes())?;
        writer.u8(self.last_work_attempt_index)?;
        writer.reserved(7)?;
        writer.bytes(&self.resolution_evidence_id.bytes())?;
        writer.u32(self.completed_work_calls)?;
        writer.reserved(4)?;
        writer.u64(self.authorized_ceiling_lamports)?;
        writer.u64(self.exact_reward_lamports)?;
        writer.finish()
    }

    /// Decode and fully validate the exact semantic state.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input)?;
        if reader.bytes::<8>()? != MAGIC {
            return Err(RecoveryError::BadMagic);
        }
        if reader.u16()? != VERSION {
            return Err(RecoveryError::BadVersion);
        }
        reader.reserved(6)?;
        let market_instance_id = MarketInstanceV2Id::from_bytes(reader.bytes()?);
        let recovery_policy_id = EvidenceOnlyRecoveryPolicyId::from_bytes(reader.bytes()?);
        let schedule = decode_schedule(&mut reader)?;
        let funding_quote_id = SeriesFundingQuoteId::from_bytes(reader.bytes()?);
        let quote = reader.bytes::<SERIES_FUNDING_QUOTE_BYTES>()?;
        let funding_quote =
            <SeriesFundingQuoteV1 as clutch_product_series::FixedCodec>::decode(&quote)
                .map_err(super::map_funding_quote_error)?;
        let state_id = Identity::from_bytes(reader.bytes()?);
        let generation = reader.u64()?;
        let funding = ExternalRecoveryFundingV1 {
            policy_id: Identity::from_bytes(reader.bytes()?),
            lifecycle_id: Identity::from_bytes(reader.bytes()?),
            recovery_account_id: Identity::from_bytes(reader.bytes()?),
            semantic_owner: Identity::from_bytes(reader.bytes()?),
            payer: Identity::from_bytes(reader.bytes()?),
            neutral_sink: Identity::from_bytes(reader.bytes()?),
            receipt_program_id: Identity::from_bytes(reader.bytes()?),
            quote_schedule_id: Identity::from_bytes(reader.bytes()?),
            generation: reader.u64()?,
            capitalized_work_lamports: reader.u64()?,
            rent_principal_lamports: reader.u64()?,
            maximum_calls: reader.u32()?,
            maximum_lamports_per_call: {
                reader.reserved(4)?;
                reader.u64()?
            },
        };
        let phase = decode_phase(reader.u8()?)?;
        let next_attempt_index = reader.u8()?;
        reader.reserved(6)?;
        let transition_nonce = reader.u64()?;
        let last_clock = RecoveryClock {
            slot: reader.u64()?,
            unix_timestamp: reader.i64()?,
            current_bucket: reader.u64()?,
        };
        let mut accepted_progress_units = [0u64; MAX_RECOVERY_ATTEMPTS];
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            accepted_progress_units[index] = reader.u64()?;
            index += 1;
        }
        let value = Self {
            market_instance_id,
            recovery_policy_id,
            schedule,
            funding_quote_id,
            funding_quote,
            state_id,
            generation,
            funding,
            phase,
            transition_nonce,
            last_clock,
            next_attempt_index,
            accepted_progress_units,
            last_work_id: Identity::from_bytes(reader.bytes()?),
            last_reward_recipient: Identity::from_bytes(reader.bytes()?),
            last_work_attempt_index: reader.u8()?,
            resolution_evidence_id: {
                reader.reserved(7)?;
                Identity::from_bytes(reader.bytes()?)
            },
            completed_work_calls: reader.u32()?,
            authorized_ceiling_lamports: {
                reader.reserved(4)?;
                reader.u64()?
            },
            exact_reward_lamports: reader.u64()?,
        };
        reader.finish()?;
        value.check()?;
        Ok(value)
    }
}

/// Current semantic state plan and optional required liveness movement facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalRecoveryTransitionPlanV1 {
    before: ExternalRecoveryStateV1,
    after: ExternalRecoveryStateV1,
    work: Option<ExternalRecoveryWorkAuthorizationV1>,
}

impl ExternalRecoveryTransitionPlanV1 {
    /// Resulting semantic state.
    pub const fn after(&self) -> ExternalRecoveryStateV1 {
        self.after
    }

    /// Resulting semantic phase.
    pub const fn resulting_phase(&self) -> RecoveryPhase {
        self.after.phase
    }

    /// Exact work movement facts, absent for non-work transitions.
    pub const fn work(&self) -> Option<ExternalRecoveryWorkAuthorizationV1> {
        self.work
    }
}

fn require_live(identity: Identity) -> Result<()> {
    if identity.is_zero() {
        Err(RecoveryError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn checked_add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right)
        .ok_or(RecoveryError::ArithmeticOverflow)
}

fn phase_code(phase: RecoveryPhase) -> u8 {
    match phase {
        RecoveryPhase::Active => 0,
        RecoveryPhase::DegradedRecoverable => 1,
        RecoveryPhase::RecoveryDormant => 2,
        RecoveryPhase::Resolved => 3,
    }
}

fn decode_phase(value: u8) -> Result<RecoveryPhase> {
    match value {
        0 => Ok(RecoveryPhase::Active),
        1 => Ok(RecoveryPhase::DegradedRecoverable),
        2 => Ok(RecoveryPhase::RecoveryDormant),
        3 => Ok(RecoveryPhase::Resolved),
        _ => Err(RecoveryError::InvalidEnum),
    }
}

fn encode_schedule(writer: &mut Writer<'_>, schedule: CompiledScheduleV1) -> Result<()> {
    writer.u64(schedule.start_bucket)?;
    writer.u64(schedule.end_bucket_exclusive)?;
    writer.u64(schedule.primary_maturity_bucket_exclusive)?;
    writer.u8(schedule.recovery_attempt_count)?;
    writer.reserved(7)?;
    for attempt in schedule.recovery_attempts {
        writer.u64(attempt.repair_generation)?;
        writer.u64(attempt.opens_at_bucket)?;
        writer.u64(attempt.closes_at_bucket)?;
    }
    Ok(())
}

fn decode_schedule(reader: &mut Reader<'_>) -> Result<CompiledScheduleV1> {
    let start_bucket = reader.u64()?;
    let end_bucket_exclusive = reader.u64()?;
    let primary_maturity_bucket_exclusive = reader.u64()?;
    let recovery_attempt_count = reader.u8()?;
    reader.reserved(7)?;
    let mut recovery_attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    let mut index = 0usize;
    while index < MAX_RECOVERY_ATTEMPTS {
        recovery_attempts[index] = AbsoluteRecoveryAttemptV1 {
            repair_generation: reader.u64()?,
            opens_at_bucket: reader.u64()?,
            closes_at_bucket: reader.u64()?,
        };
        index += 1;
    }
    Ok(CompiledScheduleV1 {
        start_bucket,
        end_bucket_exclusive,
        primary_maturity_bucket_exclusive,
        recovery_attempt_count,
        recovery_attempts,
    })
}

struct Writer<'a> {
    output: &'a mut [u8],
    cursor: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Result<Self> {
        if output.len() != EXTERNAL_RECOVERY_STATE_V1_BYTES {
            return Err(RecoveryError::WrongLength);
        }
        Ok(Self { output, cursor: 0 })
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let end = self
            .cursor
            .checked_add(bytes.len())
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let destination = self
            .output
            .get_mut(self.cursor..end)
            .ok_or(RecoveryError::WrongLength)?;
        destination.copy_from_slice(bytes);
        self.cursor = end;
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<()> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn i64(&mut self, value: i64) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }

    fn reserved(&mut self, count: usize) -> Result<()> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let bytes = self
            .output
            .get_mut(self.cursor..end)
            .ok_or(RecoveryError::WrongLength)?;
        bytes.fill(0);
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.cursor == self.output.len() {
            Ok(())
        } else {
            Err(RecoveryError::WrongLength)
        }
    }
}

struct Reader<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Result<Self> {
        if input.len() != EXTERNAL_RECOVERY_STATE_V1_BYTES {
            return Err(RecoveryError::WrongLength);
        }
        Ok(Self { input, cursor: 0 })
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let source = self
            .input
            .get(self.cursor..end)
            .ok_or(RecoveryError::WrongLength)?;
        let mut output = [0u8; N];
        output.copy_from_slice(source);
        self.cursor = end;
        Ok(output)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }

    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.bytes()?))
    }

    fn reserved(&mut self, count: usize) -> Result<()> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RecoveryError::ArithmeticOverflow)?;
        let bytes = self
            .input
            .get(self.cursor..end)
            .ok_or(RecoveryError::WrongLength)?;
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(RecoveryError::NonCanonicalReserved);
        }
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.cursor == self.input.len() {
            Ok(())
        } else {
            Err(RecoveryError::WrongLength)
        }
    }
}
