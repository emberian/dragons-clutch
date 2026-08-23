// SPDX-License-Identifier: AGPL-3.0-or-later
//! Durable Failure-owned orchestration for exhaustive quantized intervals.
//!
//! Product owns the evaluator and its private verified-payout capability.
//! SourcePlane owns the successful interval result. Liveness remains the sole
//! work-capital custodian. This module owns only the bounded transition chain,
//! permanent replay receipt, and exact work-account rent disposition.

use clutch_evidence_recovery::{Identity as RecoveryIdentity, RecoveryPhase};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    advance_quantized_interval_consensus_work_v1, begin_quantized_interval_consensus_v1,
    restore_verified_quantized_interval_payout_v1,
    AuthenticatedQuantizedIntervalConsensusHistoryV1, MarketInstanceV2Id,
    QuantizedIntervalConsensusCertificateV1Id, QuantizedIntervalConsensusContextV1,
    QuantizedIntervalConsensusProfileV1Id, QuantizedIntervalConsensusProgressV1,
    QuantizedIntervalConsensusWorkV1, QuantizedIntervalConsensusWorkV1Id,
    VerifiedQuantizedIntervalPayoutV1,
};
use clutch_source_plane_v3::ContentId as SourceContentId;
use clutch_source_plane_v3_runtime::{AuthenticatedSourceReleaseV1, SuccessfulEvaluationHandoffV1};
use sha2::{Digest, Sha256};

use crate::external_v2::{
    FailureExternalTransitionPlanV2, FailureRecoveryWorkReceiptIdV2, FailureRuntimeExternalV2,
};
use crate::{AcceptedResolutionId, Error, FailurePolicyBindingId, Result};

const FUNDING_DOMAIN: &[u8] = b"dragons-clutch/failure-interval-funding/v1";
const BINDING_DOMAIN: &[u8] = b"dragons-clutch/failure-interval-binding/v1";
const REPLAY_DOMAIN: &[u8] = b"dragons-clutch/failure-interval-replay/v1";
const TRANSITION_DOMAIN: &[u8] = b"dragons-clutch/failure-interval-transition/v1";
const RESOLUTION_DOMAIN: &[u8] = b"dragons-clutch/failure-interval-resolution/v1";
const CLOSE_DOMAIN: &[u8] = b"dragons-clutch/failure-interval-close/v1";
const TERMINAL_DOMAIN: &[u8] = b"dragons-clutch/failure-interval-terminal/v1";

macro_rules! typed_interval_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from exact bytes without claiming account authority.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Return the exact identity bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

typed_interval_id!(
    FailureIntervalConsensusAccountIdV1,
    "Typed physical-account identity used by the interval runtime."
);
typed_interval_id!(
    FailureIntervalConsensusFundingReceiptIdV1,
    "Typed receipt for present work/replay rent and external work custody."
);
typed_interval_id!(
    FailureIntervalConsensusBindingIdV1,
    "Typed immutable identity of one Failure interval-consensus lifecycle."
);
typed_interval_id!(
    FailureIntervalConsensusTransitionReceiptIdV1,
    "Typed identity of one exact bounded work transition."
);
typed_interval_id!(
    FailureIntervalConsensusReplayReceiptIdV1,
    "Typed identity of the current permanent replay-account postimage."
);
typed_interval_id!(
    FailureIntervalConsensusResolutionReceiptIdV1,
    "Typed identity proving one Product certificate resolved Failure exactly once."
);
typed_interval_id!(
    FailureIntervalConsensusCloseAuthorizationIdV1,
    "Typed exact work-account rent and donation close authorization."
);
typed_interval_id!(
    FailureIntervalConsensusTerminalReceiptIdV1,
    "Typed terminal receipt consumed by the Product occurrence lifecycle owner."
);

/// Complete facts the account adapter must authenticate after atomic funding.
///
/// This value is an expected-facts projection, never funding authority by
/// itself. A private adapter receipt must implement
/// [`AuthenticatedFailureIntervalConsensusFundingV1`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusFundingFactsV1 {
    /// Immutable Failure policy binding.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width V2 economic occurrence.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared occurrence/liveness generation.
    pub generation: u64,
    /// Dedicated mutable `0xab/v1` work account.
    pub work_account: FailureIntervalConsensusAccountIdV1,
    /// Dedicated permanent `0xac/v1` replay account.
    pub replay_account: FailureIntervalConsensusAccountIdV1,
    /// Immutable principal payer and rent-refund recipient.
    pub rent_payer: FailureIntervalConsensusAccountIdV1,
    /// Immutable sink for preexisting or later donation lamports.
    pub neutral_sink: FailureIntervalConsensusAccountIdV1,
    /// Exact refundable work-account rent principal supplied now.
    pub work_rent_principal_lamports: u64,
    /// Exact permanent replay-account rent principal supplied now.
    pub replay_rent_principal_lamports: u64,
    /// Work-PDA donation floor observed before the creation transfer.
    pub work_creation_donation_floor_lamports: u64,
    /// Work-PDA donation observed when Begin authenticates the prefund.
    pub work_observed_donation_lamports: u64,
    /// Work-PDA balance observed by Begin.
    pub work_observed_balance_lamports: u64,
    /// Replay-PDA donation floor observed before the creation transfer.
    pub replay_creation_donation_floor_lamports: u64,
    /// Replay-PDA donation observed when Begin authenticates the prefund.
    pub replay_observed_donation_lamports: u64,
    /// Replay-PDA balance observed by Begin.
    pub replay_observed_balance_lamports: u64,
    /// Sole external liveness Recovery work-capital account.
    pub recovery_compartment_account_id: LivenessId,
    /// Immutable liveness policy.
    pub liveness_policy_id: LivenessId,
    /// Exact occurrence lifecycle shared by liveness.
    pub liveness_lifecycle_id: LivenessId,
    /// Exact liveness quote schedule for every bounded call.
    pub recovery_quote_schedule_id: LivenessId,
}

/// Adapter-owned authority proving present interval funding.
///
/// Implementors must be private types minted only after authenticating the
/// Product/Series occurrence debit, the two canonical PDAs and postbalances,
/// and the already funded liveness Recovery compartment. The default refuses.
pub trait AuthenticatedFailureIntervalConsensusFundingV1 {
    /// Authenticate every exact expected funding fact.
    fn authenticate_interval_consensus_funding(
        &self,
        _expected: FailureIntervalConsensusFundingFactsV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field receipt admitted only through the funding authority boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusFundingReceiptV1 {
    id: FailureIntervalConsensusFundingReceiptIdV1,
    facts: FailureIntervalConsensusFundingFactsV1,
}

impl FailureIntervalConsensusFundingReceiptV1 {
    /// Complete authenticated funding identity.
    pub const fn id(self) -> FailureIntervalConsensusFundingReceiptIdV1 {
        self.id
    }

    /// Exact admitted funding facts.
    pub const fn facts(self) -> FailureIntervalConsensusFundingFactsV1 {
        self.facts
    }
}

/// Validate present balances and mint the pure funding receipt only after the
/// adapter authenticates their physical sources and destinations.
pub fn admit_failure_interval_consensus_funding_v1<
    A: AuthenticatedFailureIntervalConsensusFundingV1 + ?Sized,
>(
    authority: &A,
    runtime: &FailureRuntimeExternalV2,
    facts: FailureIntervalConsensusFundingFactsV1,
) -> Result<FailureIntervalConsensusFundingReceiptV1> {
    runtime.check()?;
    validate_funding_facts(runtime, facts)?;
    authority.authenticate_interval_consensus_funding(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(FUNDING_DOMAIN);
    hash_funding_facts(&mut hasher, facts);
    Ok(FailureIntervalConsensusFundingReceiptV1 {
        id: FailureIntervalConsensusFundingReceiptIdV1::from_bytes(hasher.finalize().into()),
        facts,
    })
}

/// Durable work lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureIntervalConsensusPhaseV1 {
    /// Work can advance through bounded exact chunks.
    Active,
    /// A Product private capability has resolved Failure.
    Resolved,
    /// The mutable work account was closed; replay remains permanent.
    Closed,
}

/// Failure-owned semantic state mirrored by the dedicated `0xab/v1` account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusStateV1 {
    phase: FailureIntervalConsensusPhaseV1,
    binding_id: FailureIntervalConsensusBindingIdV1,
    failure_policy_binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_success_handoff_id: SourceContentId,
    source_interval_id: SourceContentId,
    interval_profile_id: QuantizedIntervalConsensusProfileV1Id,
    funding_receipt_id: FailureIntervalConsensusFundingReceiptIdV1,
    work_account: FailureIntervalConsensusAccountIdV1,
    replay_account: FailureIntervalConsensusAccountIdV1,
    rent_payer: FailureIntervalConsensusAccountIdV1,
    neutral_sink: FailureIntervalConsensusAccountIdV1,
    work_rent_principal_lamports: u64,
    replay_rent_principal_lamports: u64,
    replay_preserved_lamports: u64,
    initial_work_id: QuantizedIntervalConsensusWorkV1Id,
    current_work_id: QuantizedIntervalConsensusWorkV1Id,
    current_transcript: SourceContentId,
    checked_coordinates: u64,
    total_coordinates: u64,
    accepted_recovery_progress_total: u64,
    transition_nonce: u64,
    last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1,
    last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2,
    certificate_id: QuantizedIntervalConsensusCertificateV1Id,
    resolution_receipt_id: FailureIntervalConsensusResolutionReceiptIdV1,
    close_authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1,
}

/// Untrusted complete semantic projection decoded from `0xab` plus `0xac`.
///
/// Public fields make account adaptation possible; this value is never
/// authority. [`restore_failure_interval_consensus_state_v1`] requires a
/// private adapter receipt that authenticated both account owner/PDA/bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusPersistedFactsV1 {
    /// Current lifecycle phase.
    pub phase: FailureIntervalConsensusPhaseV1,
    /// Immutable interval lifecycle identity.
    pub binding_id: FailureIntervalConsensusBindingIdV1,
    /// Parent Failure policy binding.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width V2 economic occurrence.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact Failure/liveness generation.
    pub generation: u64,
    /// Exact Source-owned successful interval handoff.
    pub source_success_handoff_id: SourceContentId,
    /// Immutable successful Source result owning the interval.
    pub source_interval_id: SourceContentId,
    /// Product-selected bounded interval profile.
    pub interval_profile_id: QuantizedIntervalConsensusProfileV1Id,
    /// Exact present-funding admission receipt.
    pub funding_receipt_id: FailureIntervalConsensusFundingReceiptIdV1,
    /// Canonical mutable work account.
    pub work_account: FailureIntervalConsensusAccountIdV1,
    /// Canonical permanent replay account.
    pub replay_account: FailureIntervalConsensusAccountIdV1,
    /// Immutable work-rent refund recipient.
    pub rent_payer: FailureIntervalConsensusAccountIdV1,
    /// Immutable donation sink.
    pub neutral_sink: FailureIntervalConsensusAccountIdV1,
    /// Exact refundable work rent principal.
    pub work_rent_principal_lamports: u64,
    /// Exact permanent replay rent principal.
    pub replay_rent_principal_lamports: u64,
    /// Last authenticated permanent replay balance.
    pub replay_preserved_lamports: u64,
    /// Canonical initial Product work identity.
    pub initial_work_id: QuantizedIntervalConsensusWorkV1Id,
    /// Canonical current Product work identity.
    pub current_work_id: QuantizedIntervalConsensusWorkV1Id,
    /// Current Product rolling transcript.
    pub current_transcript: SourceContentId,
    /// Exact coordinates already checked.
    pub checked_coordinates: u64,
    /// Exact inclusive interval coordinate count.
    pub total_coordinates: u64,
    /// Cumulative accepted semantic recovery progress.
    pub accepted_recovery_progress_total: u64,
    /// Monotone bounded-transition nonce.
    pub transition_nonce: u64,
    /// Last bounded transition identity, zero before first advance.
    pub last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1,
    /// Last exact liveness work receipt, zero before first advance.
    pub last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2,
    /// Exhaustive Product certificate, zero before resolution.
    pub certificate_id: QuantizedIntervalConsensusCertificateV1Id,
    /// Failure resolution receipt, zero before resolution.
    pub resolution_receipt_id: FailureIntervalConsensusResolutionReceiptIdV1,
    /// Exact work close authorization, zero before close.
    pub close_authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1,
}

/// Adapter-owned authority for restoring Failure interval state.
///
/// Implementors must be private types minted only after exact owner, PDA,
/// complete `0xab`/`0xac` body, generation, and cross-account authentication.
/// The default refuses so an empty implementation cannot restore state.
pub trait AuthenticatedFailureIntervalConsensusStateV1 {
    /// Authenticate the exact decoded facts and derived replay identity.
    fn authenticate_interval_consensus_state(
        &self,
        _facts: FailureIntervalConsensusPersistedFactsV1,
        _replay_receipt_id: FailureIntervalConsensusReplayReceiptIdV1,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

impl FailureIntervalConsensusStateV1 {
    /// Current lifecycle phase.
    pub const fn phase(&self) -> FailureIntervalConsensusPhaseV1 {
        self.phase
    }

    /// Immutable interval lifecycle identity.
    pub const fn binding_id(&self) -> FailureIntervalConsensusBindingIdV1 {
        self.binding_id
    }

    /// Current Product structural work identity.
    pub const fn current_work_id(&self) -> QuantizedIntervalConsensusWorkV1Id {
        self.current_work_id
    }

    /// Current bounded-transition nonce.
    pub const fn transition_nonce(&self) -> u64 {
        self.transition_nonce
    }

    /// Current cumulative checked coordinate count.
    pub const fn checked_coordinates(&self) -> u64 {
        self.checked_coordinates
    }

    /// Exact dedicated work account.
    pub const fn work_account(&self) -> FailureIntervalConsensusAccountIdV1 {
        self.work_account
    }

    /// Exact permanent replay account.
    pub const fn replay_account(&self) -> FailureIntervalConsensusAccountIdV1 {
        self.replay_account
    }

    /// Project complete semantic facts for exact account encoding.
    pub const fn persisted_facts(&self) -> FailureIntervalConsensusPersistedFactsV1 {
        FailureIntervalConsensusPersistedFactsV1 {
            phase: self.phase,
            binding_id: self.binding_id,
            failure_policy_binding_id: self.failure_policy_binding_id,
            market_instance_id: self.market_instance_id,
            generation: self.generation,
            source_success_handoff_id: self.source_success_handoff_id,
            source_interval_id: self.source_interval_id,
            interval_profile_id: self.interval_profile_id,
            funding_receipt_id: self.funding_receipt_id,
            work_account: self.work_account,
            replay_account: self.replay_account,
            rent_payer: self.rent_payer,
            neutral_sink: self.neutral_sink,
            work_rent_principal_lamports: self.work_rent_principal_lamports,
            replay_rent_principal_lamports: self.replay_rent_principal_lamports,
            replay_preserved_lamports: self.replay_preserved_lamports,
            initial_work_id: self.initial_work_id,
            current_work_id: self.current_work_id,
            current_transcript: self.current_transcript,
            checked_coordinates: self.checked_coordinates,
            total_coordinates: self.total_coordinates,
            accepted_recovery_progress_total: self.accepted_recovery_progress_total,
            transition_nonce: self.transition_nonce,
            last_transition_receipt_id: self.last_transition_receipt_id,
            last_liveness_receipt_id: self.last_liveness_receipt_id,
            certificate_id: self.certificate_id,
            resolution_receipt_id: self.resolution_receipt_id,
            close_authorization_id: self.close_authorization_id,
        }
    }

    /// Commit one fresh plan and reject stale siblings.
    pub fn commit_plan(&mut self, plan: FailureIntervalConsensusStatePlanV1) -> Result<()> {
        self.check()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        plan.after.check()?;
        *self = plan.after;
        Ok(())
    }

    fn check(&self) -> Result<()> {
        require_live(self.binding_id.bytes())?;
        require_live(self.failure_policy_binding_id.bytes())?;
        require_live(self.market_instance_id.bytes())?;
        require_live(self.source_success_handoff_id.bytes())?;
        require_live(self.source_interval_id.bytes())?;
        require_live(self.interval_profile_id.bytes())?;
        require_live(self.funding_receipt_id.bytes())?;
        require_live(self.work_account.bytes())?;
        require_live(self.replay_account.bytes())?;
        require_live(self.rent_payer.bytes())?;
        require_live(self.neutral_sink.bytes())?;
        require_live(self.initial_work_id.bytes())?;
        require_live(self.current_work_id.bytes())?;
        require_live(self.current_transcript.bytes())?;
        if self.generation == 0
            || self.work_account == self.replay_account
            || self.work_account == self.rent_payer
            || self.work_account == self.neutral_sink
            || self.replay_account == self.rent_payer
            || self.replay_account == self.neutral_sink
            || self.rent_payer == self.neutral_sink
            || self.work_rent_principal_lamports == 0
            || self.replay_rent_principal_lamports == 0
            || self.replay_preserved_lamports < self.replay_rent_principal_lamports
            || self.total_coordinates == 0
            || self.checked_coordinates > self.total_coordinates
        {
            return Err(Error::BindingMismatch);
        }
        let advanced = self.transition_nonce != 0;
        if advanced
            != (live(self.last_transition_receipt_id.bytes())
                && live(self.last_liveness_receipt_id.bytes()))
            || (!advanced
                && (self.checked_coordinates != 0 || self.current_work_id != self.initial_work_id))
            || (advanced && self.checked_coordinates == 0)
        {
            return Err(Error::BindingMismatch);
        }
        match self.phase {
            FailureIntervalConsensusPhaseV1::Active => {
                if live(self.certificate_id.bytes())
                    || live(self.resolution_receipt_id.bytes())
                    || live(self.close_authorization_id.bytes())
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureIntervalConsensusPhaseV1::Resolved => {
                if self.checked_coordinates != self.total_coordinates
                    || !live(self.certificate_id.bytes())
                    || !live(self.resolution_receipt_id.bytes())
                    || live(self.close_authorization_id.bytes())
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureIntervalConsensusPhaseV1::Closed => {
                if self.checked_coordinates != self.total_coordinates
                    || !live(self.certificate_id.bytes())
                    || !live(self.resolution_receipt_id.bytes())
                    || !live(self.close_authorization_id.bytes())
                {
                    return Err(Error::WrongPhase);
                }
            }
        }
        Ok(())
    }
}

/// Restore private Failure state only through an authenticated `0xab`/`0xac`
/// account join. Raw decoded projections cannot authorize a transition.
pub fn restore_failure_interval_consensus_state_v1<
    A: AuthenticatedFailureIntervalConsensusStateV1 + ?Sized,
>(
    authority: &A,
    facts: FailureIntervalConsensusPersistedFactsV1,
) -> Result<(
    FailureIntervalConsensusStateV1,
    FailureIntervalConsensusReplayV1,
)> {
    let state = state_from_persisted_facts(facts)?;
    let replay = replay_from_state(state);
    authority.authenticate_interval_consensus_state(facts, replay.id())?;
    Ok((state, replay))
}

/// Derive the canonical replay identity from untrusted decoded facts without
/// treating that identity as account authority.
pub fn project_failure_interval_consensus_replay_id_v1(
    facts: FailureIntervalConsensusPersistedFactsV1,
) -> Result<FailureIntervalConsensusReplayReceiptIdV1> {
    Ok(replay_from_state(state_from_persisted_facts(facts)?).id())
}

fn state_from_persisted_facts(
    facts: FailureIntervalConsensusPersistedFactsV1,
) -> Result<FailureIntervalConsensusStateV1> {
    let state = FailureIntervalConsensusStateV1 {
        phase: facts.phase,
        binding_id: facts.binding_id,
        failure_policy_binding_id: facts.failure_policy_binding_id,
        market_instance_id: facts.market_instance_id,
        generation: facts.generation,
        source_success_handoff_id: facts.source_success_handoff_id,
        source_interval_id: facts.source_interval_id,
        interval_profile_id: facts.interval_profile_id,
        funding_receipt_id: facts.funding_receipt_id,
        work_account: facts.work_account,
        replay_account: facts.replay_account,
        rent_payer: facts.rent_payer,
        neutral_sink: facts.neutral_sink,
        work_rent_principal_lamports: facts.work_rent_principal_lamports,
        replay_rent_principal_lamports: facts.replay_rent_principal_lamports,
        replay_preserved_lamports: facts.replay_preserved_lamports,
        initial_work_id: facts.initial_work_id,
        current_work_id: facts.current_work_id,
        current_transcript: facts.current_transcript,
        checked_coordinates: facts.checked_coordinates,
        total_coordinates: facts.total_coordinates,
        accepted_recovery_progress_total: facts.accepted_recovery_progress_total,
        transition_nonce: facts.transition_nonce,
        last_transition_receipt_id: facts.last_transition_receipt_id,
        last_liveness_receipt_id: facts.last_liveness_receipt_id,
        certificate_id: facts.certificate_id,
        resolution_receipt_id: facts.resolution_receipt_id,
        close_authorization_id: facts.close_authorization_id,
    };
    state.check()?;
    if state.persisted_facts() != facts {
        return Err(Error::BindingMismatch);
    }
    Ok(state)
}

/// Permanent replay-account semantic postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusReplayV1 {
    id: FailureIntervalConsensusReplayReceiptIdV1,
    binding_id: FailureIntervalConsensusBindingIdV1,
    work_account: FailureIntervalConsensusAccountIdV1,
    replay_account: FailureIntervalConsensusAccountIdV1,
    initial_work_id: QuantizedIntervalConsensusWorkV1Id,
    current_work_id: QuantizedIntervalConsensusWorkV1Id,
    current_transcript: SourceContentId,
    transition_nonce: u64,
    last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1,
    last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2,
    certificate_id: QuantizedIntervalConsensusCertificateV1Id,
    resolution_receipt_id: FailureIntervalConsensusResolutionReceiptIdV1,
    close_authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1,
    replay_preserved_lamports: u64,
    phase: FailureIntervalConsensusPhaseV1,
}

impl FailureIntervalConsensusReplayV1 {
    /// Exact current permanent replay postimage identity.
    pub const fn id(self) -> FailureIntervalConsensusReplayReceiptIdV1 {
        self.id
    }

    /// Exact interval lifecycle identity.
    pub const fn binding_id(self) -> FailureIntervalConsensusBindingIdV1 {
        self.binding_id
    }

    /// Monotone work transition nonce.
    pub const fn transition_nonce(self) -> u64 {
        self.transition_nonce
    }

    /// Current replay phase.
    pub const fn phase(self) -> FailureIntervalConsensusPhaseV1 {
        self.phase
    }
}

/// Begin one dedicated interval chain after Source and funding authentication.
pub fn begin_failure_interval_consensus_v1(
    runtime: &FailureRuntimeExternalV2,
    source_success: SuccessfulEvaluationHandoffV1,
    source_release: AuthenticatedSourceReleaseV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
    funding: FailureIntervalConsensusFundingReceiptV1,
) -> Result<(
    FailureIntervalConsensusStateV1,
    QuantizedIntervalConsensusWorkV1,
    FailureIntervalConsensusReplayV1,
)> {
    runtime.check()?;
    if runtime.phase() != RecoveryPhase::DegradedRecoverable {
        return Err(Error::WrongPhase);
    }
    runtime.authenticate_interval_source_success(source_success, source_release)?;
    validate_funding_facts(runtime, funding.facts)?;
    let session = begin_quantized_interval_consensus_v1(context)?;
    let work = *session.work();
    validate_source_product_join(runtime, source_success, &work)?;
    let initial_work_id = work.id()?;
    let source_interval_id = source_success.statistic_result_id()?;
    let total_coordinates = work.total_coordinates()?;
    let facts = funding.facts;
    let mut binding_hasher = Sha256::new();
    binding_hasher.update(BINDING_DOMAIN);
    binding_hasher.update(runtime.binding_id().bytes());
    binding_hasher.update(runtime.binding().market_instance_id().bytes());
    binding_hasher.update(runtime.binding().generation().to_le_bytes());
    binding_hasher.update(source_success.id().bytes());
    binding_hasher.update(source_interval_id.bytes());
    binding_hasher.update(work.interval_profile_id().bytes());
    binding_hasher.update(funding.id.bytes());
    binding_hasher.update(facts.work_account.bytes());
    binding_hasher.update(facts.replay_account.bytes());
    binding_hasher.update(initial_work_id.bytes());
    let binding_id =
        FailureIntervalConsensusBindingIdV1::from_bytes(binding_hasher.finalize().into());
    let state = FailureIntervalConsensusStateV1 {
        phase: FailureIntervalConsensusPhaseV1::Active,
        binding_id,
        failure_policy_binding_id: runtime.binding_id(),
        market_instance_id: runtime.binding().market_instance_id(),
        generation: runtime.binding().generation(),
        source_success_handoff_id: source_success.id(),
        source_interval_id,
        interval_profile_id: work.interval_profile_id(),
        funding_receipt_id: funding.id,
        work_account: facts.work_account,
        replay_account: facts.replay_account,
        rent_payer: facts.rent_payer,
        neutral_sink: facts.neutral_sink,
        work_rent_principal_lamports: facts.work_rent_principal_lamports,
        replay_rent_principal_lamports: facts.replay_rent_principal_lamports,
        replay_preserved_lamports: facts.replay_observed_balance_lamports,
        initial_work_id,
        current_work_id: initial_work_id,
        current_transcript: work.transcript(),
        checked_coordinates: 0,
        total_coordinates,
        accepted_recovery_progress_total: runtime.current_accepted_progress_units()?,
        transition_nonce: 0,
        last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1::from_bytes(
            [0; 32],
        ),
        last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2::from_bytes([0; 32]),
        certificate_id: QuantizedIntervalConsensusCertificateV1Id::from_bytes([0; 32]),
        resolution_receipt_id: FailureIntervalConsensusResolutionReceiptIdV1::from_bytes([0; 32]),
        close_authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1::from_bytes([0; 32]),
    };
    state.check()?;
    let replay = replay_from_state(state);
    Ok((state, work, replay))
}

/// One exact bounded transition and its required Failure/liveness join.
#[derive(Clone, Copy, Debug)]
pub struct FailureIntervalConsensusAdvancePlanV1 {
    state_plan: FailureIntervalConsensusStatePlanV1,
    next_work: QuantizedIntervalConsensusWorkV1,
    progress: QuantizedIntervalConsensusProgressV1,
    failure_plan: FailureExternalTransitionPlanV2,
    transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1,
    replay: FailureIntervalConsensusReplayV1,
}

impl FailureIntervalConsensusAdvancePlanV1 {
    /// New Product structural work postimage.
    pub const fn next_work(&self) -> QuantizedIntervalConsensusWorkV1 {
        self.next_work
    }

    /// Exact Product progress reported for this bounded call.
    pub const fn progress(&self) -> QuantizedIntervalConsensusProgressV1 {
        self.progress
    }

    /// Failure semantic transition that must commit atomically with liveness.
    pub const fn failure_plan(&self) -> FailureExternalTransitionPlanV2 {
        self.failure_plan
    }

    /// Unique pre/post transition identity used as the Failure work identity.
    pub const fn transition_receipt_id(&self) -> FailureIntervalConsensusTransitionReceiptIdV1 {
        self.transition_receipt_id
    }

    /// Permanent replay-account postimage for the same atomic batch.
    pub const fn replay(&self) -> FailureIntervalConsensusReplayV1 {
        self.replay
    }

    /// Failure-owned state pre/post plan.
    pub const fn state_plan(&self) -> FailureIntervalConsensusStatePlanV1 {
        self.state_plan
    }
}

/// Plan one bounded exhaustive interval chunk and its exact keeper receipt.
#[allow(clippy::too_many_arguments)]
pub fn plan_advance_failure_interval_consensus_v1(
    state: &FailureIntervalConsensusStateV1,
    runtime: &FailureRuntimeExternalV2,
    current_work: &QuantizedIntervalConsensusWorkV1,
    source_success: SuccessfulEvaluationHandoffV1,
    source_release: AuthenticatedSourceReleaseV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
    requested_coordinates: u16,
    reward_recipient: RecoveryIdentity,
    scheduled_ceiling_lamports: u64,
) -> Result<FailureIntervalConsensusAdvancePlanV1> {
    validate_active_join(state, runtime, current_work, source_success, source_release)?;
    let (next_work, progress) =
        advance_quantized_interval_consensus_work_v1(current_work, context, requested_coordinates)?;
    let next_work_id = next_work.id()?;
    let accepted_recovery_progress_total = state
        .accepted_recovery_progress_total
        .checked_add(u64::from(progress.processed_coordinates))
        .ok_or(Error::BindingMismatch)?;
    let next_nonce = state
        .transition_nonce
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let mut transition_hasher = Sha256::new();
    let prior_replay_receipt_id = replay_from_state(*state).id();
    transition_hasher.update(TRANSITION_DOMAIN);
    transition_hasher.update(state.binding_id.bytes());
    transition_hasher.update(state.transition_nonce.to_le_bytes());
    transition_hasher.update(next_nonce.to_le_bytes());
    transition_hasher.update(state.last_transition_receipt_id.bytes());
    transition_hasher.update(prior_replay_receipt_id.bytes());
    transition_hasher.update(state.current_work_id.bytes());
    transition_hasher.update(next_work_id.bytes());
    transition_hasher.update(state.current_transcript.bytes());
    transition_hasher.update(next_work.transcript().bytes());
    transition_hasher.update(progress.processed_coordinates.to_le_bytes());
    transition_hasher.update(progress.checked_coordinates.to_le_bytes());
    transition_hasher.update(reward_recipient.bytes());
    transition_hasher.update(scheduled_ceiling_lamports.to_le_bytes());
    let transition_receipt_id = FailureIntervalConsensusTransitionReceiptIdV1::from_bytes(
        transition_hasher.finalize().into(),
    );
    let clock = runtime.authenticate_interval_source_success(source_success, source_release)?;
    let failure_plan = runtime.plan_accept_authenticated_progress(
        clock,
        RecoveryIdentity::from_bytes(transition_receipt_id.bytes()),
        source_success.id(),
        source_success.window_id(),
        reward_recipient,
        accepted_recovery_progress_total,
        scheduled_ceiling_lamports,
    )?;
    let liveness_receipt = failure_plan.work().ok_or(Error::BindingMismatch)?;
    if liveness_receipt.reward_recipient() != reward_recipient
        || liveness_receipt.scheduled_ceiling_lamports() != scheduled_ceiling_lamports
    {
        return Err(Error::BindingMismatch);
    }
    let mut after = *state;
    after.current_work_id = next_work_id;
    after.current_transcript = next_work.transcript();
    after.checked_coordinates = progress.checked_coordinates;
    after.accepted_recovery_progress_total = accepted_recovery_progress_total;
    after.transition_nonce = next_nonce;
    after.last_transition_receipt_id = transition_receipt_id;
    after.last_liveness_receipt_id = liveness_receipt.id();
    after.check()?;
    let replay = replay_from_state(after);
    Ok(FailureIntervalConsensusAdvancePlanV1 {
        state_plan: FailureIntervalConsensusStatePlanV1 {
            before: *state,
            after,
        },
        next_work,
        progress,
        failure_plan,
        transition_receipt_id,
        replay,
    })
}

/// Atomic interval resolution plan carrying Product's private capability.
#[derive(Clone, Copy, Debug)]
pub struct FailureIntervalConsensusResolutionPlanV1 {
    state_plan: FailureIntervalConsensusStatePlanV1,
    failure_plan: FailureExternalTransitionPlanV2,
    verified_payout: VerifiedQuantizedIntervalPayoutV1,
    certificate_id: QuantizedIntervalConsensusCertificateV1Id,
    resolution_receipt: FailureIntervalConsensusResolutionReceiptV1,
    replay: FailureIntervalConsensusReplayV1,
}

impl FailureIntervalConsensusResolutionPlanV1 {
    /// Failure semantic resolve transition for the same atomic batch.
    pub const fn failure_plan(&self) -> FailureExternalTransitionPlanV2 {
        self.failure_plan
    }

    /// Product private capability restored from authenticated durable history.
    pub const fn verified_payout(&self) -> VerifiedQuantizedIntervalPayoutV1 {
        self.verified_payout
    }

    /// Exact exhaustive Product certificate identity.
    pub const fn certificate_id(&self) -> QuantizedIntervalConsensusCertificateV1Id {
        self.certificate_id
    }

    /// Exact once-only Failure resolution receipt.
    pub const fn resolution_receipt_id(&self) -> FailureIntervalConsensusResolutionReceiptIdV1 {
        self.resolution_receipt.id
    }

    /// Private-field authority consumed by the atomic Resolution writer.
    pub const fn resolution_receipt(&self) -> FailureIntervalConsensusResolutionReceiptV1 {
        self.resolution_receipt
    }

    /// Permanent replay postimage sealing certificate consumption.
    pub const fn replay(&self) -> FailureIntervalConsensusReplayV1 {
        self.replay
    }

    /// Failure-owned state pre/post plan.
    pub const fn state_plan(&self) -> FailureIntervalConsensusStatePlanV1 {
        self.state_plan
    }
}

/// Restore Product authority from authenticated history and resolve Failure.
pub fn plan_resolve_failure_interval_consensus_v1<
    A: AuthenticatedQuantizedIntervalConsensusHistoryV1 + ?Sized,
>(
    history_authority: &A,
    state: &FailureIntervalConsensusStateV1,
    runtime: &FailureRuntimeExternalV2,
    complete_work: &QuantizedIntervalConsensusWorkV1,
    source_success: SuccessfulEvaluationHandoffV1,
    source_release: AuthenticatedSourceReleaseV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
) -> Result<FailureIntervalConsensusResolutionPlanV1> {
    validate_active_join(
        state,
        runtime,
        complete_work,
        source_success,
        source_release,
    )?;
    if !complete_work.is_complete()
        || state.checked_coordinates != state.total_coordinates
        || state.transition_nonce == 0
    {
        return Err(Error::WrongPhase);
    }
    let verified_payout =
        restore_verified_quantized_interval_payout_v1(history_authority, complete_work, context)?;
    let (accepted, certificate_id) = runtime.accept_interval_consensus_resolution(
        source_success,
        &verified_payout,
        source_release,
    )?;
    let failure_plan = runtime.plan_resolve_caller_funded(accepted)?;
    if failure_plan.resulting_phase() != RecoveryPhase::Resolved || failure_plan.work().is_some() {
        return Err(Error::BindingMismatch);
    }
    let resolution_receipt = resolution_receipt(state, accepted.id(), certificate_id);
    let mut after = *state;
    after.phase = FailureIntervalConsensusPhaseV1::Resolved;
    after.certificate_id = certificate_id;
    after.resolution_receipt_id = resolution_receipt.id;
    after.check()?;
    let replay = replay_from_state(after);
    Ok(FailureIntervalConsensusResolutionPlanV1 {
        state_plan: FailureIntervalConsensusStatePlanV1 {
            before: *state,
            after,
        },
        failure_plan,
        verified_payout,
        certificate_id,
        resolution_receipt,
        replay,
    })
}

/// Private complete authority that binds Failure resolution to one exact
/// Product certificate and authenticated interval-account history.
///
/// There is deliberately no public constructor or account codec. The live
/// adapter may receive this value only from
/// [`plan_resolve_failure_interval_consensus_v1`] and must consume it in the
/// same atomic instruction that writes the full-width Resolution successor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusResolutionReceiptV1 {
    id: FailureIntervalConsensusResolutionReceiptIdV1,
    interval_binding_id: FailureIntervalConsensusBindingIdV1,
    failure_policy_binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_success_handoff_id: SourceContentId,
    source_interval_id: SourceContentId,
    interval_profile_id: QuantizedIntervalConsensusProfileV1Id,
    work_account: FailureIntervalConsensusAccountIdV1,
    replay_account: FailureIntervalConsensusAccountIdV1,
    terminal_work_id: QuantizedIntervalConsensusWorkV1Id,
    terminal_transcript: SourceContentId,
    transition_nonce: u64,
    last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1,
    last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2,
    product_certificate_id: QuantizedIntervalConsensusCertificateV1Id,
    accepted_resolution_id: AcceptedResolutionId,
}

impl FailureIntervalConsensusResolutionReceiptV1 {
    /// Complete receipt identity.
    pub const fn id(self) -> FailureIntervalConsensusResolutionReceiptIdV1 {
        self.id
    }

    /// Failure-owned interval lifecycle identity.
    pub const fn interval_binding_id(self) -> FailureIntervalConsensusBindingIdV1 {
        self.interval_binding_id
    }

    /// Immutable parent Failure policy binding.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Full-width Product Market identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact occurrence generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Source-owned successful-evaluation handoff.
    pub const fn source_success_handoff_id(self) -> SourceContentId {
        self.source_success_handoff_id
    }

    /// Exact Source result that owned the interval.
    pub const fn source_interval_id(self) -> SourceContentId {
        self.source_interval_id
    }

    /// Central-profile-derived bounded work identity.
    pub const fn interval_profile_id(self) -> QuantizedIntervalConsensusProfileV1Id {
        self.interval_profile_id
    }

    /// Authenticated mutable work account.
    pub const fn work_account(self) -> FailureIntervalConsensusAccountIdV1 {
        self.work_account
    }

    /// Authenticated permanent replay account.
    pub const fn replay_account(self) -> FailureIntervalConsensusAccountIdV1 {
        self.replay_account
    }

    /// Complete terminal Product work postimage identity.
    pub const fn terminal_work_id(self) -> QuantizedIntervalConsensusWorkV1Id {
        self.terminal_work_id
    }

    /// Complete terminal Product transcript.
    pub const fn terminal_transcript(self) -> SourceContentId {
        self.terminal_transcript
    }

    /// Final nonzero bounded-transition nonce.
    pub const fn transition_nonce(self) -> u64 {
        self.transition_nonce
    }

    /// Last Failure transition receipt in the authenticated chain.
    pub const fn last_transition_receipt_id(self) -> FailureIntervalConsensusTransitionReceiptIdV1 {
        self.last_transition_receipt_id
    }

    /// Last exact liveness work receipt in the authenticated chain.
    pub const fn last_liveness_receipt_id(self) -> FailureRecoveryWorkReceiptIdV2 {
        self.last_liveness_receipt_id
    }

    /// Exact exhaustive Product certificate.
    pub const fn product_certificate_id(self) -> QuantizedIntervalConsensusCertificateV1Id {
        self.product_certificate_id
    }

    /// Exact Failure accepted-resolution semantic transition.
    pub const fn accepted_resolution_id(self) -> AcceptedResolutionId {
        self.accepted_resolution_id
    }
}

/// Exact close movements for the mutable work account only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusWorkClosePlanV1 {
    /// Mutable work account closed by this plan.
    pub work_account: FailureIntervalConsensusAccountIdV1,
    /// Immutable rent-principal recipient.
    pub rent_refund_recipient: FailureIntervalConsensusAccountIdV1,
    /// Exact refundable rent principal.
    pub rent_refund_lamports: u64,
    /// Immutable donation sink.
    pub donation_sink: FailureIntervalConsensusAccountIdV1,
    /// Exact surplus over rent principal.
    pub donation_lamports: u64,
    /// Permanent replay account which must remain open.
    pub replay_account: FailureIntervalConsensusAccountIdV1,
    /// Exact replay balance that this close must leave unchanged.
    pub replay_preserved_lamports: u64,
    /// Typed close authorization committing to all movements.
    pub authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1,
}

/// Terminal plan closing `0xab` while retaining permanent `0xac`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusClosePlanV1 {
    state_plan: FailureIntervalConsensusStatePlanV1,
    movements: FailureIntervalConsensusWorkClosePlanV1,
    replay: FailureIntervalConsensusReplayV1,
    terminal_receipt: FailureIntervalConsensusTerminalReceiptV1,
}

impl FailureIntervalConsensusClosePlanV1 {
    /// Exact work-account close movements.
    pub const fn movements(self) -> FailureIntervalConsensusWorkClosePlanV1 {
        self.movements
    }

    /// Permanent terminal replay postimage.
    pub const fn replay(self) -> FailureIntervalConsensusReplayV1 {
        self.replay
    }

    /// Product occurrence lifecycle receipt.
    pub const fn terminal_receipt(self) -> FailureIntervalConsensusTerminalReceiptV1 {
        self.terminal_receipt
    }

    /// Failure-owned state pre/post plan.
    pub const fn state_plan(self) -> FailureIntervalConsensusStatePlanV1 {
        self.state_plan
    }
}

/// Private-field terminal capability for Product's occurrence lifecycle owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusTerminalReceiptV1 {
    id: FailureIntervalConsensusTerminalReceiptIdV1,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    failure_policy_binding_id: FailurePolicyBindingId,
    interval_work_account: FailureIntervalConsensusAccountIdV1,
    permanent_replay_account: FailureIntervalConsensusAccountIdV1,
    product_certificate_id: QuantizedIntervalConsensusCertificateV1Id,
    failure_resolution_receipt_id: FailureIntervalConsensusResolutionReceiptIdV1,
    work_close_authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1,
}

impl FailureIntervalConsensusTerminalReceiptV1 {
    /// Full-width V2 economic occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact shared generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Immutable Failure policy binding.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Closed mutable work account.
    pub const fn interval_work_account(self) -> FailureIntervalConsensusAccountIdV1 {
        self.interval_work_account
    }

    /// Permanent replay account retained after work close.
    pub const fn permanent_replay_account(self) -> FailureIntervalConsensusAccountIdV1 {
        self.permanent_replay_account
    }

    /// Exact Product certificate consumed by Failure resolution.
    pub const fn product_certificate_id(self) -> QuantizedIntervalConsensusCertificateV1Id {
        self.product_certificate_id
    }

    /// Exact Failure resolution receipt.
    pub const fn failure_resolution_receipt_id(
        self,
    ) -> FailureIntervalConsensusResolutionReceiptIdV1 {
        self.failure_resolution_receipt_id
    }

    /// Exact authenticated work-account close authorization.
    pub const fn work_close_authorization_id(
        self,
    ) -> FailureIntervalConsensusCloseAuthorizationIdV1 {
        self.work_close_authorization_id
    }

    /// Complete terminal receipt identity.
    pub const fn terminal_receipt_id(self) -> FailureIntervalConsensusTerminalReceiptIdV1 {
        self.id
    }
}

/// Plan the exact rent refund and donation sink after Failure resolution.
pub fn plan_close_failure_interval_consensus_work_v1(
    state: &FailureIntervalConsensusStateV1,
    runtime: &FailureRuntimeExternalV2,
    actual_work_balance_lamports: u64,
    actual_replay_balance_lamports: u64,
) -> Result<FailureIntervalConsensusClosePlanV1> {
    state.check()?;
    validate_runtime_binding(state, runtime)?;
    if state.phase != FailureIntervalConsensusPhaseV1::Resolved
        || runtime.phase() != RecoveryPhase::Resolved
        || actual_work_balance_lamports < state.work_rent_principal_lamports
        || actual_replay_balance_lamports < state.replay_preserved_lamports
    {
        return Err(Error::WrongPhase);
    }
    let donation_lamports = actual_work_balance_lamports
        .checked_sub(state.work_rent_principal_lamports)
        .ok_or(Error::BindingMismatch)?;
    let mut close_hasher = Sha256::new();
    close_hasher.update(CLOSE_DOMAIN);
    close_hasher.update(state.binding_id.bytes());
    close_hasher.update(state.work_account.bytes());
    close_hasher.update(state.replay_account.bytes());
    close_hasher.update(state.rent_payer.bytes());
    close_hasher.update(state.neutral_sink.bytes());
    close_hasher.update(state.work_rent_principal_lamports.to_le_bytes());
    close_hasher.update(donation_lamports.to_le_bytes());
    close_hasher.update(actual_replay_balance_lamports.to_le_bytes());
    close_hasher.update(state.certificate_id.bytes());
    close_hasher.update(state.resolution_receipt_id.bytes());
    let authorization_id =
        FailureIntervalConsensusCloseAuthorizationIdV1::from_bytes(close_hasher.finalize().into());
    let movements = FailureIntervalConsensusWorkClosePlanV1 {
        work_account: state.work_account,
        rent_refund_recipient: state.rent_payer,
        rent_refund_lamports: state.work_rent_principal_lamports,
        donation_sink: state.neutral_sink,
        donation_lamports,
        replay_account: state.replay_account,
        replay_preserved_lamports: actual_replay_balance_lamports,
        authorization_id,
    };
    let mut after = *state;
    after.phase = FailureIntervalConsensusPhaseV1::Closed;
    after.close_authorization_id = authorization_id;
    after.replay_preserved_lamports = actual_replay_balance_lamports;
    after.check()?;
    let replay = replay_from_state(after);
    let terminal_receipt = terminal_receipt(after, replay);
    Ok(FailureIntervalConsensusClosePlanV1 {
        state_plan: FailureIntervalConsensusStatePlanV1 {
            before: *state,
            after,
        },
        movements,
        replay,
        terminal_receipt,
    })
}

/// Stale-checked pre/post state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureIntervalConsensusStatePlanV1 {
    before: FailureIntervalConsensusStateV1,
    after: FailureIntervalConsensusStateV1,
}

impl FailureIntervalConsensusStatePlanV1 {
    /// Resulting lifecycle phase.
    pub const fn resulting_phase(self) -> FailureIntervalConsensusPhaseV1 {
        self.after.phase
    }

    /// Exact semantic poststate for account encoding in the same atomic batch.
    pub const fn resulting_state(self) -> FailureIntervalConsensusStateV1 {
        self.after
    }
}

fn validate_funding_facts(
    runtime: &FailureRuntimeExternalV2,
    facts: FailureIntervalConsensusFundingFactsV1,
) -> Result<()> {
    validate_prefund_observation(
        facts.work_rent_principal_lamports,
        facts.work_creation_donation_floor_lamports,
        facts.work_observed_donation_lamports,
        facts.work_observed_balance_lamports,
    )?;
    validate_prefund_observation(
        facts.replay_rent_principal_lamports,
        facts.replay_creation_donation_floor_lamports,
        facts.replay_observed_donation_lamports,
        facts.replay_observed_balance_lamports,
    )?;
    if facts.failure_policy_binding_id != runtime.binding_id()
        || facts.market_instance_id != runtime.binding().market_instance_id()
        || facts.generation != runtime.binding().generation()
        || facts.generation == 0
        || facts.work_rent_principal_lamports == 0
        || facts.replay_rent_principal_lamports == 0
        || facts.recovery_compartment_account_id != runtime.recovery_compartment_account_id()
        || facts.liveness_policy_id != runtime.liveness_policy_id()
        || facts.liveness_lifecycle_id != runtime.liveness_lifecycle_id()
        || facts.recovery_quote_schedule_id != runtime.recovery_quote_schedule_id()
        || facts.rent_payer.bytes() != runtime.recovery_payer().bytes()
        || facts.neutral_sink.bytes() != runtime.recovery_neutral_sink().bytes()
        || !live(facts.work_account.bytes())
        || !live(facts.replay_account.bytes())
        || !live(facts.rent_payer.bytes())
        || !live(facts.neutral_sink.bytes())
        || facts.work_account == facts.replay_account
        || facts.work_account == facts.rent_payer
        || facts.work_account == facts.neutral_sink
        || facts.replay_account == facts.rent_payer
        || facts.replay_account == facts.neutral_sink
        || facts.rent_payer == facts.neutral_sink
        || facts.work_account.bytes() == facts.recovery_compartment_account_id.bytes()
        || facts.replay_account.bytes() == facts.recovery_compartment_account_id.bytes()
        || facts.work_account.bytes() == runtime.semantic_state_id().bytes()
        || facts.replay_account.bytes() == runtime.semantic_state_id().bytes()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn validate_prefund_observation(
    principal_lamports: u64,
    creation_donation_floor_lamports: u64,
    observed_donation_lamports: u64,
    observed_balance_lamports: u64,
) -> Result<()> {
    if principal_lamports == 0
        || observed_donation_lamports < creation_donation_floor_lamports
        || observed_donation_lamports
            .checked_add(principal_lamports)
            .ok_or(Error::BindingMismatch)?
            != observed_balance_lamports
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn validate_active_join(
    state: &FailureIntervalConsensusStateV1,
    runtime: &FailureRuntimeExternalV2,
    work: &QuantizedIntervalConsensusWorkV1,
    source_success: SuccessfulEvaluationHandoffV1,
    source_release: AuthenticatedSourceReleaseV1,
) -> Result<()> {
    state.check()?;
    validate_runtime_binding(state, runtime)?;
    runtime.authenticate_interval_source_success(source_success, source_release)?;
    validate_source_product_join(runtime, source_success, work)?;
    if state.phase != FailureIntervalConsensusPhaseV1::Active
        || runtime.phase() != RecoveryPhase::DegradedRecoverable
        || state.source_success_handoff_id != source_success.id()
        || state.source_interval_id != source_success.statistic_result_id()?
        || state.interval_profile_id != work.interval_profile_id()
        || state.current_work_id != work.id()?
        || state.current_transcript != work.transcript()
        || state.checked_coordinates != work.checked_coordinates()
        || state.total_coordinates != work.total_coordinates()?
        || state.accepted_recovery_progress_total != runtime.current_accepted_progress_units()?
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn validate_runtime_binding(
    state: &FailureIntervalConsensusStateV1,
    runtime: &FailureRuntimeExternalV2,
) -> Result<()> {
    runtime.check()?;
    if state.failure_policy_binding_id != runtime.binding_id()
        || state.market_instance_id != runtime.binding().market_instance_id()
        || state.generation != runtime.binding().generation()
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn validate_source_product_join(
    runtime: &FailureRuntimeExternalV2,
    source_success: SuccessfulEvaluationHandoffV1,
    work: &QuantizedIntervalConsensusWorkV1,
) -> Result<()> {
    if work.market_instance_id() != runtime.binding().market_instance_id()
        || work.product_template_id() != runtime.binding().product_template_id()
        || work.source_occurrence_id().bytes()
            != source_success.occurrence().occurrence_record_id().bytes()
        || work.source_interval_id() != source_success.statistic_result_id()?
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn replay_from_state(state: FailureIntervalConsensusStateV1) -> FailureIntervalConsensusReplayV1 {
    let mut hasher = Sha256::new();
    hasher.update(REPLAY_DOMAIN);
    hasher.update(state.binding_id.bytes());
    hasher.update(state.work_account.bytes());
    hasher.update(state.replay_account.bytes());
    hasher.update(state.initial_work_id.bytes());
    hasher.update(state.current_work_id.bytes());
    hasher.update(state.current_transcript.bytes());
    hasher.update(state.transition_nonce.to_le_bytes());
    hasher.update(state.last_transition_receipt_id.bytes());
    hasher.update(state.last_liveness_receipt_id.bytes());
    hasher.update(state.certificate_id.bytes());
    hasher.update(state.resolution_receipt_id.bytes());
    hasher.update(state.close_authorization_id.bytes());
    hasher.update(state.replay_preserved_lamports.to_le_bytes());
    hasher.update([phase_code(state.phase)]);
    FailureIntervalConsensusReplayV1 {
        id: FailureIntervalConsensusReplayReceiptIdV1::from_bytes(hasher.finalize().into()),
        binding_id: state.binding_id,
        work_account: state.work_account,
        replay_account: state.replay_account,
        initial_work_id: state.initial_work_id,
        current_work_id: state.current_work_id,
        current_transcript: state.current_transcript,
        transition_nonce: state.transition_nonce,
        last_transition_receipt_id: state.last_transition_receipt_id,
        last_liveness_receipt_id: state.last_liveness_receipt_id,
        certificate_id: state.certificate_id,
        resolution_receipt_id: state.resolution_receipt_id,
        close_authorization_id: state.close_authorization_id,
        replay_preserved_lamports: state.replay_preserved_lamports,
        phase: state.phase,
    }
}

fn resolution_receipt(
    state: &FailureIntervalConsensusStateV1,
    accepted_resolution_id: AcceptedResolutionId,
    certificate_id: QuantizedIntervalConsensusCertificateV1Id,
) -> FailureIntervalConsensusResolutionReceiptV1 {
    let mut hasher = Sha256::new();
    hasher.update(RESOLUTION_DOMAIN);
    hasher.update(state.binding_id.bytes());
    hasher.update(state.failure_policy_binding_id.bytes());
    hasher.update(state.market_instance_id.bytes());
    hasher.update(state.generation.to_le_bytes());
    hasher.update(state.source_success_handoff_id.bytes());
    hasher.update(state.source_interval_id.bytes());
    hasher.update(state.interval_profile_id.bytes());
    hasher.update(state.work_account.bytes());
    hasher.update(state.replay_account.bytes());
    hasher.update(state.current_work_id.bytes());
    hasher.update(state.current_transcript.bytes());
    hasher.update(state.transition_nonce.to_le_bytes());
    hasher.update(state.last_transition_receipt_id.bytes());
    hasher.update(state.last_liveness_receipt_id.bytes());
    hasher.update(certificate_id.bytes());
    hasher.update(accepted_resolution_id.bytes());
    FailureIntervalConsensusResolutionReceiptV1 {
        id: FailureIntervalConsensusResolutionReceiptIdV1::from_bytes(hasher.finalize().into()),
        interval_binding_id: state.binding_id,
        failure_policy_binding_id: state.failure_policy_binding_id,
        market_instance_id: state.market_instance_id,
        generation: state.generation,
        source_success_handoff_id: state.source_success_handoff_id,
        source_interval_id: state.source_interval_id,
        interval_profile_id: state.interval_profile_id,
        work_account: state.work_account,
        replay_account: state.replay_account,
        terminal_work_id: state.current_work_id,
        terminal_transcript: state.current_transcript,
        transition_nonce: state.transition_nonce,
        last_transition_receipt_id: state.last_transition_receipt_id,
        last_liveness_receipt_id: state.last_liveness_receipt_id,
        product_certificate_id: certificate_id,
        accepted_resolution_id,
    }
}

fn terminal_receipt(
    state: FailureIntervalConsensusStateV1,
    replay: FailureIntervalConsensusReplayV1,
) -> FailureIntervalConsensusTerminalReceiptV1 {
    let mut hasher = Sha256::new();
    hasher.update(TERMINAL_DOMAIN);
    hasher.update(state.binding_id.bytes());
    hasher.update(state.market_instance_id.bytes());
    hasher.update(state.generation.to_le_bytes());
    hasher.update(state.failure_policy_binding_id.bytes());
    hasher.update(state.work_account.bytes());
    hasher.update(state.replay_account.bytes());
    hasher.update(state.certificate_id.bytes());
    hasher.update(state.resolution_receipt_id.bytes());
    hasher.update(state.close_authorization_id.bytes());
    hasher.update(replay.id.bytes());
    FailureIntervalConsensusTerminalReceiptV1 {
        id: FailureIntervalConsensusTerminalReceiptIdV1::from_bytes(hasher.finalize().into()),
        market_instance_id: state.market_instance_id,
        generation: state.generation,
        failure_policy_binding_id: state.failure_policy_binding_id,
        interval_work_account: state.work_account,
        permanent_replay_account: state.replay_account,
        product_certificate_id: state.certificate_id,
        failure_resolution_receipt_id: state.resolution_receipt_id,
        work_close_authorization_id: state.close_authorization_id,
    }
}

fn hash_funding_facts(hasher: &mut Sha256, facts: FailureIntervalConsensusFundingFactsV1) {
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.work_account.bytes());
    hasher.update(facts.replay_account.bytes());
    hasher.update(facts.rent_payer.bytes());
    hasher.update(facts.neutral_sink.bytes());
    hasher.update(facts.work_rent_principal_lamports.to_le_bytes());
    hasher.update(facts.replay_rent_principal_lamports.to_le_bytes());
    hasher.update(facts.work_creation_donation_floor_lamports.to_le_bytes());
    hasher.update(facts.work_observed_donation_lamports.to_le_bytes());
    hasher.update(facts.work_observed_balance_lamports.to_le_bytes());
    hasher.update(facts.replay_creation_donation_floor_lamports.to_le_bytes());
    hasher.update(facts.replay_observed_donation_lamports.to_le_bytes());
    hasher.update(facts.replay_observed_balance_lamports.to_le_bytes());
    hasher.update(facts.recovery_compartment_account_id.bytes());
    hasher.update(facts.liveness_policy_id.bytes());
    hasher.update(facts.liveness_lifecycle_id.bytes());
    hasher.update(facts.recovery_quote_schedule_id.bytes());
}

const fn phase_code(phase: FailureIntervalConsensusPhaseV1) -> u8 {
    match phase {
        FailureIntervalConsensusPhaseV1::Active => 1,
        FailureIntervalConsensusPhaseV1::Resolved => 2,
        FailureIntervalConsensusPhaseV1::Closed => 3,
    }
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if live(bytes) {
        Ok(())
    } else {
        Err(Error::ZeroIdentity)
    }
}

fn live(bytes: [u8; 32]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_facts() -> FailureIntervalConsensusPersistedFactsV1 {
        FailureIntervalConsensusPersistedFactsV1 {
            phase: FailureIntervalConsensusPhaseV1::Active,
            binding_id: FailureIntervalConsensusBindingIdV1::from_bytes([1; 32]),
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes([2; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([3; 32]),
            generation: 1,
            source_success_handoff_id: SourceContentId::from_bytes([4; 32]),
            source_interval_id: SourceContentId::from_bytes([5; 32]),
            interval_profile_id: QuantizedIntervalConsensusProfileV1Id::from_bytes([6; 32]),
            funding_receipt_id: FailureIntervalConsensusFundingReceiptIdV1::from_bytes([7; 32]),
            work_account: FailureIntervalConsensusAccountIdV1::from_bytes([8; 32]),
            replay_account: FailureIntervalConsensusAccountIdV1::from_bytes([9; 32]),
            rent_payer: FailureIntervalConsensusAccountIdV1::from_bytes([10; 32]),
            neutral_sink: FailureIntervalConsensusAccountIdV1::from_bytes([11; 32]),
            work_rent_principal_lamports: 100,
            replay_rent_principal_lamports: 200,
            replay_preserved_lamports: 203,
            initial_work_id: QuantizedIntervalConsensusWorkV1Id::from_bytes([12; 32]),
            current_work_id: QuantizedIntervalConsensusWorkV1Id::from_bytes([13; 32]),
            current_transcript: SourceContentId::from_bytes([14; 32]),
            checked_coordinates: 1,
            total_coordinates: 2,
            accepted_recovery_progress_total: 1,
            transition_nonce: 1,
            last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1::from_bytes(
                [15; 32],
            ),
            last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2::from_bytes([16; 32]),
            certificate_id: QuantizedIntervalConsensusCertificateV1Id::from_bytes([0; 32]),
            resolution_receipt_id: FailureIntervalConsensusResolutionReceiptIdV1::from_bytes(
                [0; 32],
            ),
            close_authorization_id: FailureIntervalConsensusCloseAuthorizationIdV1::from_bytes(
                [0; 32],
            ),
        }
    }

    #[test]
    fn replay_derivation_is_finite_deterministic_and_transition_sensitive() {
        let facts = active_facts();
        let first = project_failure_interval_consensus_replay_id_v1(facts).unwrap();
        let second = project_failure_interval_consensus_replay_id_v1(facts).unwrap();
        assert_eq!(first, second);
        assert!(first.bytes().iter().any(|byte| *byte != 0));

        let sibling = FailureIntervalConsensusPersistedFactsV1 {
            last_transition_receipt_id: FailureIntervalConsensusTransitionReceiptIdV1::from_bytes(
                [17; 32],
            ),
            ..facts
        };
        assert_ne!(
            first,
            project_failure_interval_consensus_replay_id_v1(sibling).unwrap()
        );
    }

    #[test]
    fn later_prefund_donations_cannot_grief_begin_or_discount_principal() {
        assert_eq!(validate_prefund_observation(100, 3, 9, 109), Ok(()));
        assert_eq!(
            validate_prefund_observation(100, 3, 2, 102),
            Err(Error::BindingMismatch)
        );
        assert_eq!(
            validate_prefund_observation(100, 3, 9, 108),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn replay_projection_refuses_a_half_recorded_paid_transition() {
        let facts = FailureIntervalConsensusPersistedFactsV1 {
            last_liveness_receipt_id: FailureRecoveryWorkReceiptIdV2::from_bytes([0; 32]),
            ..active_facts()
        };
        assert_eq!(
            project_failure_interval_consensus_replay_id_v1(facts),
            Err(Error::BindingMismatch)
        );
    }

    #[test]
    fn resolution_receipt_binds_the_complete_authenticated_history() {
        let first_state = state_from_persisted_facts(active_facts()).unwrap();
        let accepted_resolution_id = AcceptedResolutionId::from_bytes([30; 32]);
        let certificate_id = QuantizedIntervalConsensusCertificateV1Id::from_bytes([31; 32]);
        let first = resolution_receipt(&first_state, accepted_resolution_id, certificate_id);
        assert_eq!(first.market_instance_id(), first_state.market_instance_id);
        assert_eq!(first.product_certificate_id(), certificate_id);
        assert_eq!(first.accepted_resolution_id(), accepted_resolution_id);

        let changed_state = state_from_persisted_facts(FailureIntervalConsensusPersistedFactsV1 {
            source_interval_id: SourceContentId::from_bytes([29; 32]),
            ..active_facts()
        })
        .unwrap();
        assert_ne!(
            first.id(),
            resolution_receipt(&changed_state, accepted_resolution_id, certificate_id,).id()
        );
    }
}
