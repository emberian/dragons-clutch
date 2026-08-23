// SPDX-License-Identifier: AGPL-3.0-or-later
//! Single-custody successor failure runtime.
//!
//! This module supersedes the reserve-owning draft in the crate root. Failure
//! state owns deterministic trigger/recovery semantics; the persisted
//! `clutch-liveness` Recovery compartment is the sole work/rent custodian.

use clutch_evidence_recovery::{
    EvidenceDecision, ExternalRecoveryAdmissionV1, ExternalRecoveryFundingV1,
    ExternalRecoveryStateV1, ExternalRecoveryTransitionPlanV1, ExternalRecoveryWorkAuthorizationV1,
    Identity as RecoveryIdentity, RecoveryAdmission, RecoveryClock, RecoveryPhase,
    EXTERNAL_RECOVERY_STATE_V1_BYTES,
};
use clutch_liveness::runtime_adapter_v1::{
    RuntimeReceiptKindV1, RuntimeReceiptObservationV1, RuntimeTransitionActionV1,
    RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    compile_ordinal_v2, CompiledOrdinalV2, EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV2,
    MarketInstanceV2Id, NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4,
    RegistryCapabilityProjectionV2, SeriesAttachmentPlanV1, SeriesFundingQuoteId,
    SeriesFundingQuoteV1, SeriesFundingTermsV2, SeriesPlanV5,
};
use clutch_source_plane_v3::{
    ContentId as SourceContentId, SourcePlaneProgramV3, StatisticKeyV3, SummaryProgramV3,
    WindowSpecV3,
};
use clutch_source_plane_v3_runtime::{
    AuthenticatedSourceReleaseV1, ClockPolicyV1, ClockSnapshotV1, FailurePolicySourceHandoffV1,
    OccurrenceSourceReceiptV1, SourceFailureKindV1, SuccessfulEvaluationHandoffV1,
};
use sha2::{Digest, Sha256};

use crate::{
    AcceptedResolutionId, Error, FailurePolicyBindingId, FailurePolicyBindingV1, FailureTriggerId,
    FailureTriggerKindV1, FailureTriggerV1, RelationRefusalV1, Result,
};

const ADMISSION_DOMAIN: &[u8] = b"dragons-clutch/failure-external-admission/v2";
const WORK_RECEIPT_DOMAIN: &[u8] = b"dragons-clutch/failure-recovery-work-receipt/v2";
const ACCEPTED_RESOLUTION_DOMAIN: &[u8] = b"dragons-clutch/failure-accepted-resolution/v2";
const RUNTIME_STATE_COMMITMENT_DOMAIN: &[u8] =
    b"dragons-clutch/failure-runtime-state-commitment/v2";
const RECOVERY_TERMINAL_DOMAIN: &[u8] = b"dragons-clutch/failure-recovery-terminal/v2";
const FULL_TERMINAL_DOMAIN: &[u8] = b"dragons-clutch/failure-full-terminal/v2";
const MAGIC: [u8; 8] = *b"DCFAILE2";
const VERSION: u16 = 2;

macro_rules! typed_external_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from exact digest bytes without claiming authenticity.
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

/// Exact canonical width of one single-custody persisted failure runtime.
pub const FAILURE_RUNTIME_EXTERNAL_V2_BYTES: usize = 2_048;

typed_external_id!(
    FailureExternalAdmissionReceiptIdV2,
    "Typed identity of a presently funded external-custody admission."
);
typed_external_id!(
    FailureRecoveryWorkReceiptIdV2,
    "Typed identity of one exact semantic recovery-work authorization."
);
typed_external_id!(
    FailureRecoveryTerminalReceiptIdV2,
    "Typed identity closing the external liveness Recovery compartment."
);
typed_external_id!(
    FailureRuntimeStateCommitmentV2,
    "Typed commitment to the complete canonical failure-runtime bytes."
);
typed_external_id!(
    FailureExternalTerminalJoinIdV2,
    "Typed identity of the full retirement/source/replay terminal join."
);

/// Authenticated relation result over one source-owned success handoff.
///
/// The relation adapter constructs this only after executing the immutable
/// relation program. This DTO never carries a payout and cannot replace the
/// bound source handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRelationResultV2 {
    /// Exact failure-policy binding.
    pub binding_id: FailurePolicyBindingId,
    /// Exact full-width occurrence.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact failure/recovery generation.
    pub generation: u64,
    /// Exact authenticated source success handoff.
    pub source_success_handoff_id: SourceContentId,
    /// Frozen relation policy implementation/content identity.
    pub relation_policy_id: [u8; 32],
    /// Nonzero authenticated relation record.
    pub relation_record_id: [u8; 32],
    /// Accepted relation or one closed deterministic refusal.
    pub disposition: RelationDispositionV2,
}

/// Exhaustive relation disposition consumed by failure policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelationDispositionV2 {
    /// The frozen relation accepted the source result for normal resolution.
    Accepted,
    /// The frozen relation selected no value because evidence was unusable.
    Refused(RelationRefusalV1),
}

/// Present-funding receipt consumed by Product/Series occurrence activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureExternalAdmissionReceiptV2 {
    id: FailureExternalAdmissionReceiptIdV2,
    binding_id: FailurePolicyBindingId,
    series_plan_id: [u8; 32],
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    funding_quote_id: SeriesFundingQuoteId,
    semantic_state_id: RecoveryIdentity,
    liveness_policy_id: LivenessId,
    liveness_lifecycle_id: LivenessId,
    recovery_compartment_account_id: LivenessId,
    generation: u64,
    work_principal_lamports: u64,
    rent_principal_lamports: u64,
}

impl FailureExternalAdmissionReceiptV2 {
    /// Complete typed receipt identity.
    pub const fn id(self) -> FailureExternalAdmissionReceiptIdV2 {
        self.id
    }

    /// Immutable failure-policy binding.
    pub const fn binding_id(self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact SeriesPlanV5 identity bytes.
    pub const fn series_plan_id(self) -> [u8; 32] {
        self.series_plan_id
    }

    /// Exact Series ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Exact full-width V2 market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact Product/Series funding quote.
    pub const fn funding_quote_id(self) -> SeriesFundingQuoteId {
        self.funding_quote_id
    }

    /// Durable failure semantic state, never a work reserve.
    pub const fn semantic_state_id(self) -> RecoveryIdentity {
        self.semantic_state_id
    }

    /// Exact liveness policy identity.
    pub const fn liveness_policy_id(self) -> LivenessId {
        self.liveness_policy_id
    }

    /// Exact occurrence lifecycle identity.
    pub const fn liveness_lifecycle_id(self) -> LivenessId {
        self.liveness_lifecycle_id
    }

    /// Sole Recovery work/rent custody account.
    pub const fn recovery_compartment_account_id(self) -> LivenessId {
        self.recovery_compartment_account_id
    }

    /// Exact shared generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Work principal presently held only by liveness.
    pub const fn work_principal_lamports(self) -> u64 {
        self.work_principal_lamports
    }

    /// Rent principal presently held only by liveness.
    pub const fn rent_principal_lamports(self) -> u64 {
        self.rent_principal_lamports
    }
}

/// Exact semantic work receipt which a liveness `SpendWork` must consume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRecoveryWorkReceiptV2 {
    id: FailureRecoveryWorkReceiptIdV2,
    binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    semantic_state_id: RecoveryIdentity,
    liveness_policy_id: LivenessId,
    liveness_lifecycle_id: LivenessId,
    recovery_compartment_account_id: LivenessId,
    semantic_owner: LivenessId,
    quote_schedule_id: LivenessId,
    generation: u64,
    attempt_index: u8,
    call_ordinal: u32,
    source_success_handoff_id: SourceContentId,
    window_id: SourceContentId,
    reward_recipient: RecoveryIdentity,
    accepted_progress_total: u64,
    exact_reward_lamports: u64,
    scheduled_ceiling_lamports: u64,
}

impl FailureRecoveryWorkReceiptV2 {
    /// Complete typed semantic receipt identity.
    pub const fn id(self) -> FailureRecoveryWorkReceiptIdV2 {
        self.id
    }

    /// Recipient that liveness must name as keeper.
    pub const fn reward_recipient(self) -> RecoveryIdentity {
        self.reward_recipient
    }

    /// Exact semantic reward that liveness pays once.
    pub const fn exact_reward_lamports(self) -> u64 {
        self.exact_reward_lamports
    }

    /// Exact ceiling removed by liveness; headroom returns to its payer.
    pub const fn scheduled_ceiling_lamports(self) -> u64 {
        self.scheduled_ceiling_lamports
    }

    /// One-based liveness call ordinal.
    pub const fn call_ordinal(self) -> u32 {
        self.call_ordinal
    }

    /// Exact source-owned successful evaluation.
    pub const fn source_success_handoff_id(self) -> SourceContentId {
        self.source_success_handoff_id
    }

    /// Exact repair Window.
    pub const fn window_id(self) -> SourceContentId {
        self.window_id
    }

    /// Project authenticated private fields for `clutch-liveness`.
    ///
    /// The concrete adapter supplies the actual readable semantic-state
    /// account and its checked program owner. This projection performs no
    /// unchecked 32-byte cast: every domain crosses through `from_bytes`.
    pub fn runtime_receipt_observation(
        self,
        receipt_account_id: LivenessId,
        receipt_account_owner_program_id: LivenessId,
    ) -> Result<RuntimeReceiptObservationV1> {
        if receipt_account_id.bytes() != self.semantic_state_id.bytes()
            || receipt_account_owner_program_id != self.semantic_owner
        {
            return Err(Error::BindingMismatch);
        }
        Ok(RuntimeReceiptObservationV1 {
            receipt_account_id,
            receipt_account_owner_program_id,
            receipt_id: LivenessId::from_bytes(self.id.bytes()),
            receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
            compartment_kind: RuntimeCompartmentKindV1::Recovery,
            semantic_owner: self.semantic_owner,
            lifecycle_id: self.liveness_lifecycle_id,
            quote_schedule_id: self.quote_schedule_id,
            generation: self.generation,
            call_ordinal: self.call_ordinal,
            call_ceiling_lamports: self.scheduled_ceiling_lamports,
        })
    }

    /// Construct the only admissible liveness intent for this receipt.
    pub fn runtime_transition_intent(self) -> RuntimeTransitionIntentV1 {
        RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Recovery,
            policy_id: self.liveness_policy_id,
            lifecycle_id: self.liveness_lifecycle_id,
            account_id: self.recovery_compartment_account_id,
            semantic_owner: self.semantic_owner,
            quote_schedule_id: self.quote_schedule_id,
            receipt_id: LivenessId::from_bytes(self.id.bytes()),
            keeper: LivenessId::from_bytes(self.reward_recipient.bytes()),
            generation: self.generation,
            call_ordinal: self.call_ordinal,
            call_ceiling_lamports: self.scheduled_ceiling_lamports,
            keeper_payment_lamports: self.exact_reward_lamports,
            flags: 0,
        }
    }
}

/// Source- and relation-bound accepted resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedResolutionV2 {
    id: AcceptedResolutionId,
    source_success_handoff_id: SourceContentId,
    window_id: SourceContentId,
    relation_record_id: [u8; 32],
}

impl AcceptedResolutionV2 {
    /// Exact accepted-resolution identity.
    pub const fn id(self) -> AcceptedResolutionId {
        self.id
    }

    /// Source success that was classified.
    pub const fn source_success_handoff_id(self) -> SourceContentId {
        self.source_success_handoff_id
    }

    /// Exact primary or repair Window.
    pub const fn window_id(self) -> SourceContentId {
        self.window_id
    }

    /// Exact relation result record.
    pub const fn relation_record_id(self) -> [u8; 32] {
        self.relation_record_id
    }
}

/// Single-custody failure runtime for one V5 occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRuntimeExternalV2 {
    binding: FailurePolicyBindingV1,
    binding_id: FailurePolicyBindingId,
    attachment_plan_id: [u8; 32],
    source_release_manifest_id: SourceContentId,
    source_release_authentication_id: SourceContentId,
    primary_window: WindowSpecV3,
    recovery: ExternalRecoveryStateV1,
    trigger: Option<FailureTriggerV1>,
}

impl FailureRuntimeExternalV2 {
    /// Recompute the full Product/Series/Source graph and admit an already
    /// persisted, presently funded liveness Recovery compartment.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_successor(
        compiled: &CompiledOrdinalV2,
        series: &SeriesPlanV5,
        template: &ProductTemplateV4,
        basis: &NativeClaimBasisV1,
        recovery_policy: &EvidenceOnlyRecoveryPolicyV1,
        price_policy: &PriceMeasurePolicyV1,
        genesis: &MarketGenesisProfileV2,
        attachment: &SeriesAttachmentPlanV1,
        registry: &RegistryCapabilityProjectionV2,
        funding_terms: &SeriesFundingTermsV2,
        funding_quote: SeriesFundingQuoteV1,
        source_plane: &SourcePlaneProgramV3,
        summary: &SummaryProgramV3,
        primary_window: WindowSpecV3,
        statistic_key: StatisticKeyV3,
        source_occurrence: OccurrenceSourceReceiptV1,
        source_release: AuthenticatedSourceReleaseV1,
        semantic_admission: RecoveryAdmission,
        liveness_policy: RuntimeLivenessPolicyV1,
        funded_recovery: RuntimeCompartmentV1,
        funded_recovery_account_lamports: u64,
        creation_clock: ClockSnapshotV1,
    ) -> Result<(Self, FailureExternalAdmissionReceiptV2)> {
        let recomputed = compile_ordinal_v2(
            series,
            template,
            basis,
            recovery_policy,
            price_policy,
            genesis,
            attachment,
            registry,
            compiled.ordinal,
        )?;
        if *compiled != recomputed {
            return Err(Error::BindingMismatch);
        }
        funding_terms.validate_bindings(
            series,
            template,
            basis,
            recovery_policy,
            price_policy,
            genesis,
            registry,
        )?;
        funding_quote.validate_recovery_binding(recovery_policy)?;
        let quote_id = funding_quote.id()?;
        if attachment.funding_quote_id != quote_id
            || semantic_admission.series_funding_quote_id != quote_id
            || semantic_admission.work_funder.bytes()
                != funding_terms.lamport_principal_refund.bytes()
            || semantic_admission.rent_payer.bytes()
                != funding_terms.lamport_principal_refund.bytes()
            || semantic_admission.neutral_sink.bytes() != funding_terms.neutral_lamport_sink.bytes()
        {
            return Err(Error::BindingMismatch);
        }
        source_plane.validate()?;
        summary.validate()?;
        primary_window.validate()?;
        statistic_key.validate()?;
        let source_plane_id = source_plane.id()?;
        let summary_id = summary.id()?;
        let primary_window_id = primary_window.id()?;
        let statistic_key_id = statistic_key.id()?;
        let source_manifest = source_release.manifest();
        let clock_policy = source_release.clock_policy();
        let clock_policy_id = clock_policy.id()?;
        let expected_coverage = u16::try_from(template.coverage_policy_registry_value)
            .map_err(|_| Error::BindingMismatch)?;
        if template.source_plane_contract_id.bytes() != source_plane_id.bytes()
            || template.source_spec_id.bytes() != primary_window.source_spec_id.bytes()
            || template.summary_program_id.bytes() != summary_id.bytes()
            || primary_window.source_plane_program_id != source_plane_id
            || primary_window.start_bucket != compiled.schedule.start_bucket
            || primary_window.end_bucket_exclusive != compiled.schedule.end_bucket_exclusive
            || primary_window.maturity_bucket_exclusive
                != compiled.schedule.primary_maturity_bucket_exclusive
            || primary_window.repair_generation != template.base_repair_generation
            || primary_window.coverage_policy_id != expected_coverage
            || primary_window.coverage_policy_parameter != template.coverage_policy_parameter
            || statistic_key.window_id != primary_window_id
            || statistic_key.summary_program_id != summary_id
            || statistic_key.statistic as u16 != template.statistic_registry_value
            || source_occurrence.series_plan_id().bytes() != compiled.series_plan_id.bytes()
            || source_occurrence.ordinal() != compiled.ordinal
            || source_occurrence.market_instance_id().bytes() != compiled.market_instance_id.bytes()
            || source_occurrence.attachment_plan_id().bytes() != compiled.attachment_plan_id.bytes()
            || source_occurrence.source_plane_contract_id() != source_plane_id
            || source_occurrence.source_spec_id() != primary_window.source_spec_id
            || source_occurrence.window_id() != primary_window_id
            || source_occurrence.statistic_key_id() != statistic_key_id
            || source_occurrence.repair_generation() != primary_window.repair_generation
            || source_occurrence.clock_policy_id() != clock_policy_id
            || source_manifest.source_plane_contract_id != source_plane_id
            || source_manifest.source_spec_id != primary_window.source_spec_id
        {
            return Err(Error::BindingMismatch);
        }

        liveness_policy
            .validate()
            .map_err(|_| Error::BindingMismatch)?;
        funded_recovery
            .validate_against_policy(liveness_policy)
            .map_err(|_| Error::BindingMismatch)?;
        let expected_balance = funded_recovery
            .expected_account_balance_lamports()
            .map_err(|_| Error::BindingMismatch)?;
        let quoted_work = funding_quote.recovery_work_principal_lamports()?;
        if funded_recovery.kind != RuntimeCompartmentKindV1::Recovery
            || funded_recovery.phase != RuntimeCompartmentPhaseV1::Active
            || funded_recovery.completed_calls != 0
            || funded_recovery.completed_work_ceiling_lamports != 0
            || funded_recovery.keeper_paid_lamports != 0
            || funded_recovery.payer_refunded_work_lamports != 0
            || funded_recovery.neutral_sinked_work_lamports != 0
            || funded_recovery.rent_refunded_lamports != 0
            || funded_recovery.donation_sinked_lamports != 0
            || funded_recovery.last_work_receipt_id != LivenessId::ZERO
            || funded_recovery.terminal_receipt_id != LivenessId::ZERO
            || funded_recovery.identity.policy_id != liveness_policy.policy_id
            || funded_recovery.identity.generation != semantic_admission.generation
            || funded_recovery.identity.payer.bytes() != semantic_admission.work_funder.bytes()
            || funded_recovery.identity.neutral_sink.bytes()
                != semantic_admission.neutral_sink.bytes()
            || funded_recovery.quote_schedule_id.bytes() != quote_id.bytes()
            || funded_recovery.capitalized_work_lamports != quoted_work
            || funded_recovery.remaining_work_lamports != quoted_work
            || funded_recovery.rent_principal_lamports
                != funding_quote.recovery_rent_principal_lamports
            || funded_recovery.rent_locked_lamports
                != funding_quote.recovery_rent_principal_lamports
            || funded_recovery_account_lamports != expected_balance
        {
            return Err(Error::BindingMismatch);
        }

        let binding = FailurePolicyBindingV1 {
            series_plan_id: compiled.series_plan_id,
            ordinal: compiled.ordinal,
            market_instance_id: compiled.market_instance_id,
            product_template_id: template.id()?,
            recovery_policy_id: recovery_policy.id()?,
            funding_quote_id: quote_id,
            funding_terms_id: funding_terms.id()?,
            source_plane_program_id: source_plane_id,
            source_spec_id: primary_window.source_spec_id,
            summary_program_id: summary_id,
            primary_window_id,
            statistic_key_id,
            source_occurrence_receipt_id: source_occurrence.id(),
            clock_policy_id,
            relation_policy_id: genesis.relation_policy_id.bytes(),
            recovery_state_id: semantic_admission.state_id,
            generation: semantic_admission.generation,
        };
        let binding_id = binding.id();
        let funding = ExternalRecoveryFundingV1 {
            policy_id: RecoveryIdentity::from_bytes(funded_recovery.identity.policy_id.bytes()),
            lifecycle_id: RecoveryIdentity::from_bytes(
                funded_recovery.identity.lifecycle_id.bytes(),
            ),
            recovery_account_id: RecoveryIdentity::from_bytes(
                funded_recovery.identity.account_id.bytes(),
            ),
            semantic_owner: RecoveryIdentity::from_bytes(funded_recovery.identity.owner.bytes()),
            payer: RecoveryIdentity::from_bytes(funded_recovery.identity.payer.bytes()),
            neutral_sink: RecoveryIdentity::from_bytes(
                funded_recovery.identity.neutral_sink.bytes(),
            ),
            receipt_program_id: RecoveryIdentity::from_bytes(
                funded_recovery.receipt_program_id.bytes(),
            ),
            quote_schedule_id: RecoveryIdentity::from_bytes(
                funded_recovery.quote_schedule_id.bytes(),
            ),
            generation: funded_recovery.identity.generation,
            capitalized_work_lamports: funded_recovery.capitalized_work_lamports,
            rent_principal_lamports: funded_recovery.rent_principal_lamports,
            maximum_calls: funded_recovery.maximum_calls,
            maximum_lamports_per_call: funded_recovery.maximum_lamports_per_call,
        };
        let recovery = ExternalRecoveryStateV1::admit(
            compiled.market_instance_id,
            recovery_policy.id()?,
            compiled.schedule,
            funding_quote,
            ExternalRecoveryAdmissionV1 {
                state_id: semantic_admission.state_id,
                generation: semantic_admission.generation,
                funding,
            },
            recovery_clock_from_snapshot(&clock_policy, creation_clock)?,
        )?;
        let runtime = Self {
            binding,
            binding_id,
            attachment_plan_id: compiled.attachment_plan_id.bytes(),
            source_release_manifest_id: source_release.manifest_id(),
            source_release_authentication_id: source_release.id(),
            primary_window,
            recovery,
            trigger: None,
        };
        runtime.check()?;
        let mut hasher = Sha256::new();
        hasher.update(ADMISSION_DOMAIN);
        hasher.update(binding_id.bytes());
        hasher.update(compiled.series_plan_id.bytes());
        hasher.update(compiled.ordinal.to_le_bytes());
        hasher.update(compiled.market_instance_id.bytes());
        hasher.update(quote_id.bytes());
        hasher.update(semantic_admission.state_id.bytes());
        hasher.update(funded_recovery.identity.policy_id.bytes());
        hasher.update(funded_recovery.identity.lifecycle_id.bytes());
        hasher.update(funded_recovery.identity.account_id.bytes());
        hasher.update(semantic_admission.generation.to_le_bytes());
        hasher.update(quoted_work.to_le_bytes());
        hasher.update(funding_quote.recovery_rent_principal_lamports.to_le_bytes());
        let receipt = FailureExternalAdmissionReceiptV2 {
            id: FailureExternalAdmissionReceiptIdV2::from_bytes(hasher.finalize().into()),
            binding_id,
            series_plan_id: compiled.series_plan_id.bytes(),
            ordinal: compiled.ordinal,
            market_instance_id: compiled.market_instance_id,
            funding_quote_id: quote_id,
            semantic_state_id: semantic_admission.state_id,
            liveness_policy_id: funded_recovery.identity.policy_id,
            liveness_lifecycle_id: funded_recovery.identity.lifecycle_id,
            recovery_compartment_account_id: funded_recovery.identity.account_id,
            generation: semantic_admission.generation,
            work_principal_lamports: quoted_work,
            rent_principal_lamports: funding_quote.recovery_rent_principal_lamports,
        };
        Ok((runtime, receipt))
    }

    /// Exact immutable cross-owner binding.
    pub const fn binding(self) -> FailurePolicyBindingV1 {
        self.binding
    }

    /// Typed immutable binding identity.
    pub const fn binding_id(self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Durable semantic state identity, not the liveness account.
    pub const fn semantic_state_id(self) -> RecoveryIdentity {
        self.recovery.state_id()
    }

    /// Sole externally funded Recovery compartment.
    pub fn recovery_compartment_account_id(self) -> LivenessId {
        LivenessId::from_bytes(self.recovery.funding().recovery_account_id.bytes())
    }

    /// Immutable liveness payer and unused-ceiling refund recipient.
    pub fn recovery_payer(self) -> LivenessId {
        LivenessId::from_bytes(self.recovery.funding().payer.bytes())
    }

    /// Immutable liveness/root donation and failure-residue sink.
    pub fn recovery_neutral_sink(self) -> LivenessId {
        LivenessId::from_bytes(self.recovery.funding().neutral_sink.bytes())
    }

    /// Current deterministic semantic phase.
    pub const fn phase(self) -> RecoveryPhase {
        self.recovery.phase()
    }

    /// Current semantic replay nonce.
    pub const fn transition_nonce(self) -> u64 {
        self.recovery.transition_nonce()
    }

    /// Exact repair attempt index the next schedule/work transition targets.
    pub const fn next_attempt_index(self) -> u8 {
        self.recovery.next_attempt_index()
    }

    /// First recorded maturity trigger.
    pub const fn trigger(self) -> Option<FailureTriggerV1> {
        self.trigger
    }

    /// Validate immutable, source, external-custody, and trigger joins.
    pub fn check(&self) -> Result<()> {
        self.recovery.check()?;
        if self.binding_id != self.binding.id()
            || self.binding.market_instance_id != self.recovery.market_instance_id()
            || self.binding.recovery_policy_id != self.recovery.recovery_policy_id()
            || self.binding.funding_quote_id != self.recovery.funding_quote_id()
            || self.binding.recovery_state_id != self.recovery.state_id()
            || self.binding.generation != self.recovery.generation()
            || self.primary_window.id()? != self.binding.primary_window_id
            || self.source_release_manifest_id.is_zero()
            || self.source_release_authentication_id.is_zero()
        {
            return Err(Error::BindingMismatch);
        }
        if matches!(
            self.recovery.phase(),
            RecoveryPhase::DegradedRecoverable | RecoveryPhase::RecoveryDormant
        ) && self.trigger.is_none()
        {
            return Err(Error::BindingMismatch);
        }
        if let Some(trigger) = self.trigger {
            self.validate_trigger(trigger)?;
        }
        Ok(())
    }

    /// Refuse new exposure after immutable maturity even before a crank.
    pub fn check_new_exposure(&self, clock: RecoveryClock) -> Result<()> {
        self.check()?;
        self.recovery.check_new_exposure(clock)?;
        Ok(())
    }

    /// Enter degraded recovery from an authenticated source failure fact.
    pub fn plan_trigger_source_handoff(
        &self,
        handoff: FailurePolicySourceHandoffV1,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Result<FailureExternalTransitionPlanV2> {
        let clock = self.validate_source_failure_handoff(handoff, source_release)?;
        let (kind, refusal_code) = match handoff.kind() {
            SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => (
                FailureTriggerKindV1::PrimaryMaturityWithoutAcceptedResolution,
                0,
            ),
            SourceFailureKindV1::SourceEvaluationRefused => (
                FailureTriggerKindV1::SourceEvaluationRefused,
                handoff.refusal_code(),
            ),
        };
        let trigger = self.make_trigger(kind, handoff.id().bytes(), refusal_code, clock)?;
        let recovery = self.recovery.plan_enter_degraded(clock)?;
        self.wrap_plan(recovery, Some(trigger), None)
    }

    /// Trigger from the frozen relation's deterministic refusal over an exact
    /// source-owned success handoff.
    pub fn plan_trigger_relation_refusal(
        &self,
        success: SuccessfulEvaluationHandoffV1,
        relation: AuthenticatedRelationResultV2,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Result<FailureExternalTransitionPlanV2> {
        let clock = self.validate_success_handoff(success, source_release, true)?;
        let refusal = match relation.disposition {
            RelationDispositionV2::Refused(value) => value,
            RelationDispositionV2::Accepted => return Err(Error::BindingMismatch),
        };
        self.validate_relation(success, relation)?;
        let trigger = self.make_trigger(
            FailureTriggerKindV1::ResolutionRelationRefused,
            success.id().bytes(),
            refusal.code(),
            clock,
        )?;
        let recovery = self.recovery.plan_enter_degraded(clock)?;
        self.wrap_plan(recovery, Some(trigger), None)
    }

    /// Advance expired recovery attempts; final expiry deterministically enters
    /// dormancy without moving externally owned funds.
    pub fn plan_advance_schedule(
        &self,
        clock: RecoveryClock,
    ) -> Result<FailureExternalTransitionPlanV2> {
        self.check()?;
        let recovery = self.recovery.plan_advance_schedule(clock)?;
        self.wrap_plan(recovery, None, None)
    }

    /// Accept one exact successful repair evaluation as one progress unit and
    /// emit the only receipt liveness may use to pay its named keeper.
    pub fn plan_accept_repair_work(
        &self,
        success: SuccessfulEvaluationHandoffV1,
        source_release: AuthenticatedSourceReleaseV1,
        reward_recipient: RecoveryIdentity,
        scheduled_ceiling_lamports: u64,
    ) -> Result<FailureExternalTransitionPlanV2> {
        let clock = self.validate_success_handoff(success, source_release, false)?;
        let attempt_index = self.recovery.next_attempt_index();
        let prior = self.recovery.accepted_progress_units(attempt_index)?;
        let accepted_progress_total = prior.checked_add(1).ok_or(Error::BindingMismatch)?;
        let recovery = self.recovery.plan_accept_work_progress(
            clock,
            RecoveryIdentity::from_bytes(success.id().bytes()),
            reward_recipient,
            accepted_progress_total,
            scheduled_ceiling_lamports,
        )?;
        let work = self.work_receipt(success, recovery)?;
        self.wrap_plan(recovery, None, Some(work))
    }

    /// Bind one exact source success to an authenticated accepted relation.
    pub fn accept_resolution(
        &self,
        success: SuccessfulEvaluationHandoffV1,
        relation: AuthenticatedRelationResultV2,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Result<AcceptedResolutionV2> {
        self.validate_success_handoff(success, source_release, false)?;
        self.validate_relation(success, relation)?;
        if relation.disposition != RelationDispositionV2::Accepted {
            return Err(Error::BindingMismatch);
        }
        let window_id = success.window_id();
        let mut hasher = Sha256::new();
        hasher.update(ACCEPTED_RESOLUTION_DOMAIN);
        hasher.update(self.binding_id.bytes());
        hasher.update(self.binding.market_instance_id.bytes());
        hasher.update(self.binding.generation.to_le_bytes());
        hasher.update(success.id().bytes());
        hasher.update(window_id.bytes());
        hasher.update(relation.relation_policy_id);
        hasher.update(relation.relation_record_id);
        Ok(AcceptedResolutionV2 {
            id: AcceptedResolutionId::from_bytes(hasher.finalize().into()),
            source_success_handoff_id: success.id(),
            window_id,
            relation_record_id: relation.relation_record_id,
        })
    }

    /// Resolve from accepted evidence without authorizing a work payment.
    pub fn plan_resolve_caller_funded(
        &self,
        clock: RecoveryClock,
        accepted: AcceptedResolutionV2,
    ) -> Result<FailureExternalTransitionPlanV2> {
        self.validate_accepted_resolution(accepted)?;
        let evidence =
            EvidenceDecision::from_adapter(RecoveryIdentity::from_bytes(accepted.id.bytes()))?;
        let recovery = self.recovery.plan_resolve_caller_funded(clock, evidence)?;
        self.wrap_plan(recovery, None, None)
    }

    /// Atomically resolve and authorize exactly one final successful repair
    /// evaluation payment through liveness.
    pub fn plan_resolve_paid_repair(
        &self,
        success: SuccessfulEvaluationHandoffV1,
        relation: AuthenticatedRelationResultV2,
        source_release: AuthenticatedSourceReleaseV1,
        reward_recipient: RecoveryIdentity,
        scheduled_ceiling_lamports: u64,
    ) -> Result<FailureExternalTransitionPlanV2> {
        let clock = self.validate_success_handoff(success, source_release, false)?;
        let accepted = self.accept_resolution(success, relation, source_release)?;
        let attempt_index = self.recovery.next_attempt_index();
        let prior = self.recovery.accepted_progress_units(attempt_index)?;
        let evidence =
            EvidenceDecision::from_adapter(RecoveryIdentity::from_bytes(accepted.id.bytes()))?;
        let recovery = self.recovery.plan_resolve_paid_progress(
            clock,
            RecoveryIdentity::from_bytes(success.id().bytes()),
            reward_recipient,
            prior.checked_add(1).ok_or(Error::BindingMismatch)?,
            scheduled_ceiling_lamports,
            evidence,
        )?;
        let work = self.work_receipt(success, recovery)?;
        self.wrap_plan(recovery, None, Some(work))
    }

    fn work_receipt(
        &self,
        success: SuccessfulEvaluationHandoffV1,
        recovery: ExternalRecoveryTransitionPlanV1,
    ) -> Result<FailureRecoveryWorkReceiptV2> {
        let work = recovery.work().ok_or(Error::BindingMismatch)?;
        let funding = self.recovery.funding();
        let mut hasher = Sha256::new();
        hasher.update(WORK_RECEIPT_DOMAIN);
        hasher.update(self.binding_id.bytes());
        hasher.update(self.binding.market_instance_id.bytes());
        hasher.update(self.recovery.state_id().bytes());
        hasher.update(funding.policy_id.bytes());
        hasher.update(funding.lifecycle_id.bytes());
        hasher.update(funding.recovery_account_id.bytes());
        hasher.update(funding.semantic_owner.bytes());
        hasher.update(funding.quote_schedule_id.bytes());
        hasher.update(self.binding.generation.to_le_bytes());
        hasher.update([work.attempt_index]);
        hasher.update(work.call_ordinal.to_le_bytes());
        hasher.update(success.id().bytes());
        hasher.update(success.window_id().bytes());
        hasher.update(work.reward_recipient.bytes());
        hasher.update(work.accepted_progress_total.to_le_bytes());
        hasher.update(work.exact_reward_lamports.to_le_bytes());
        hasher.update(work.scheduled_ceiling_lamports.to_le_bytes());
        Ok(FailureRecoveryWorkReceiptV2 {
            id: FailureRecoveryWorkReceiptIdV2::from_bytes(hasher.finalize().into()),
            binding_id: self.binding_id,
            market_instance_id: self.binding.market_instance_id,
            semantic_state_id: self.recovery.state_id(),
            liveness_policy_id: LivenessId::from_bytes(funding.policy_id.bytes()),
            liveness_lifecycle_id: LivenessId::from_bytes(funding.lifecycle_id.bytes()),
            recovery_compartment_account_id: LivenessId::from_bytes(
                funding.recovery_account_id.bytes(),
            ),
            semantic_owner: LivenessId::from_bytes(funding.semantic_owner.bytes()),
            quote_schedule_id: LivenessId::from_bytes(funding.quote_schedule_id.bytes()),
            generation: self.binding.generation,
            attempt_index: work.attempt_index,
            call_ordinal: work.call_ordinal,
            source_success_handoff_id: success.id(),
            window_id: success.window_id(),
            reward_recipient: work.reward_recipient,
            accepted_progress_total: work.accepted_progress_total,
            exact_reward_lamports: work.exact_reward_lamports,
            scheduled_ceiling_lamports: work.scheduled_ceiling_lamports,
        })
    }

    fn wrap_plan(
        &self,
        recovery: ExternalRecoveryTransitionPlanV1,
        trigger: Option<FailureTriggerV1>,
        work: Option<FailureRecoveryWorkReceiptV2>,
    ) -> Result<FailureExternalTransitionPlanV2> {
        self.check()?;
        let mut after = *self;
        after.recovery.commit_plan(recovery)?;
        if let Some(trigger) = trigger {
            if after.trigger.is_some() {
                return Err(Error::TriggerAlreadyRecorded);
            }
            after.trigger = Some(trigger);
        }
        after.check()?;
        Ok(FailureExternalTransitionPlanV2 {
            before: *self,
            after,
            work,
        })
    }

    fn validate_source_failure_handoff(
        &self,
        handoff: FailurePolicySourceHandoffV1,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Result<RecoveryClock> {
        self.check()?;
        self.validate_source_release(source_release)?;
        let clock_policy = source_release.clock_policy();
        let occurrence = handoff.occurrence();
        if handoff.failure_policy_binding_id().bytes() != self.binding_id.bytes()
            || occurrence.id() != self.binding.source_occurrence_receipt_id
            || !self.validate_occurrence(occurrence, self.binding.primary_window_id)
            || clock_policy.id()? != self.binding.clock_policy_id
        {
            return Err(Error::BindingMismatch);
        }
        let clock = recovery_clock_from_snapshot(&clock_policy, handoff.clock())?;
        if clock.current_bucket < self.primary_window.maturity_bucket_exclusive {
            return Err(Error::TriggerBeforeMaturity);
        }
        Ok(clock)
    }

    fn validate_success_handoff(
        &self,
        success: SuccessfulEvaluationHandoffV1,
        source_release: AuthenticatedSourceReleaseV1,
        require_primary: bool,
    ) -> Result<RecoveryClock> {
        self.check()?;
        self.validate_source_release(source_release)?;
        let clock_policy = source_release.clock_policy();
        let clock = recovery_clock_from_snapshot(&clock_policy, success.clock())?;
        let occurrence = success.occurrence();
        if success.failure_policy_binding_id().bytes() != self.binding_id.bytes()
            || success.clock_policy_id() != self.binding.clock_policy_id
            || clock_policy.id()? != self.binding.clock_policy_id
            || !self.validate_occurrence(occurrence, success.window_id())
        {
            return Err(Error::BindingMismatch);
        }
        if require_primary {
            if occurrence.id() != self.binding.source_occurrence_receipt_id
                || success.window_id() != self.binding.primary_window_id
            {
                return Err(Error::WrongRecoveryWindow);
            }
        } else if success.window_id() != self.expected_resolution_window(clock)? {
            return Err(Error::WrongRecoveryWindow);
        }
        Ok(clock)
    }

    fn validate_source_release(&self, source_release: AuthenticatedSourceReleaseV1) -> Result<()> {
        let manifest = source_release.manifest();
        if source_release.manifest_id() != self.source_release_manifest_id
            || source_release.id() != self.source_release_authentication_id
            || manifest.source_plane_contract_id != self.binding.source_plane_program_id
            || manifest.source_spec_id != self.binding.source_spec_id
            || source_release.clock_policy().id()? != self.binding.clock_policy_id
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn validate_occurrence(
        &self,
        occurrence: OccurrenceSourceReceiptV1,
        window_id: SourceContentId,
    ) -> bool {
        occurrence.series_plan_id().bytes() == self.binding.series_plan_id.bytes()
            && occurrence.ordinal() == self.binding.ordinal
            && occurrence.market_instance_id().bytes() == self.binding.market_instance_id.bytes()
            && occurrence.attachment_plan_id().bytes() == self.attachment_plan_id
            && occurrence.source_plane_contract_id() == self.binding.source_plane_program_id
            && occurrence.source_spec_id() == self.binding.source_spec_id
            && occurrence.window_id() == window_id
            && occurrence.clock_policy_id() == self.binding.clock_policy_id
    }

    fn expected_resolution_window(&self, clock: RecoveryClock) -> Result<SourceContentId> {
        if self.recovery.phase() == RecoveryPhase::Active {
            return Ok(self.binding.primary_window_id);
        }
        let attempt = self
            .recovery
            .current_attempt()?
            .ok_or(Error::WrongRecoveryWindow)?;
        if clock.current_bucket < attempt.opens_at_bucket
            || clock.current_bucket >= attempt.closes_at_bucket
        {
            return Err(Error::WrongRecoveryWindow);
        }
        Ok(self
            .repair_window(attempt.repair_generation, attempt.closes_at_bucket)?
            .id()?)
    }

    fn validate_relation(
        &self,
        success: SuccessfulEvaluationHandoffV1,
        relation: AuthenticatedRelationResultV2,
    ) -> Result<()> {
        if relation.binding_id != self.binding_id
            || relation.market_instance_id != self.binding.market_instance_id
            || relation.generation != self.binding.generation
            || relation.source_success_handoff_id != success.id()
            || relation.relation_policy_id != self.binding.relation_policy_id
            || relation.relation_record_id.iter().all(|byte| *byte == 0)
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn validate_accepted_resolution(&self, accepted: AcceptedResolutionV2) -> Result<()> {
        self.check()?;
        if accepted.id.bytes().iter().all(|byte| *byte == 0)
            || accepted.source_success_handoff_id.is_zero()
            || accepted.window_id.is_zero()
            || accepted.relation_record_id.iter().all(|byte| *byte == 0)
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn make_trigger(
        &self,
        kind: FailureTriggerKindV1,
        evidence_id: [u8; 32],
        refusal_code: u32,
        clock: RecoveryClock,
    ) -> Result<FailureTriggerV1> {
        self.check()?;
        if self.trigger.is_some() {
            return Err(Error::TriggerAlreadyRecorded);
        }
        if clock.current_bucket < self.primary_window.maturity_bucket_exclusive {
            return Err(Error::TriggerBeforeMaturity);
        }
        let mut hasher = Sha256::new();
        hasher.update(super::TRIGGER_DOMAIN);
        hasher.update(self.binding_id.bytes());
        hasher.update(self.binding.market_instance_id.bytes());
        hasher.update(self.binding.generation.to_le_bytes());
        hasher.update([trigger_kind_code(kind)]);
        hasher.update(evidence_id);
        hasher.update(refusal_code.to_le_bytes());
        hasher.update(clock.slot.to_le_bytes());
        hasher.update(clock.unix_timestamp.to_le_bytes());
        hasher.update(clock.current_bucket.to_le_bytes());
        Ok(FailureTriggerV1 {
            id: FailureTriggerId::from_bytes(hasher.finalize().into()),
            kind,
            evidence_id,
            refusal_code,
            clock,
        })
    }

    fn validate_trigger(&self, trigger: FailureTriggerV1) -> Result<()> {
        if trigger.clock.current_bucket < self.primary_window.maturity_bucket_exclusive
            || trigger.evidence_id.iter().all(|byte| *byte == 0)
            || (trigger.kind == FailureTriggerKindV1::PrimaryMaturityWithoutAcceptedResolution
                && trigger.refusal_code != 0)
            || (trigger.kind != FailureTriggerKindV1::PrimaryMaturityWithoutAcceptedResolution
                && trigger.refusal_code == 0)
        {
            return Err(Error::BindingMismatch);
        }
        let expected = self.make_trigger_unchecked(
            trigger.kind,
            trigger.evidence_id,
            trigger.refusal_code,
            trigger.clock,
        );
        if expected != trigger.id {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn make_trigger_unchecked(
        &self,
        kind: FailureTriggerKindV1,
        evidence_id: [u8; 32],
        refusal_code: u32,
        clock: RecoveryClock,
    ) -> FailureTriggerId {
        let mut hasher = Sha256::new();
        hasher.update(super::TRIGGER_DOMAIN);
        hasher.update(self.binding_id.bytes());
        hasher.update(self.binding.market_instance_id.bytes());
        hasher.update(self.binding.generation.to_le_bytes());
        hasher.update([trigger_kind_code(kind)]);
        hasher.update(evidence_id);
        hasher.update(refusal_code.to_le_bytes());
        hasher.update(clock.slot.to_le_bytes());
        hasher.update(clock.unix_timestamp.to_le_bytes());
        hasher.update(clock.current_bucket.to_le_bytes());
        FailureTriggerId::from_bytes(hasher.finalize().into())
    }

    fn repair_window(&self, repair_generation: u64, closes_at_bucket: u64) -> Result<WindowSpecV3> {
        let window = WindowSpecV3 {
            source_plane_program_id: self.binding.source_plane_program_id,
            source_spec_id: self.binding.source_spec_id,
            start_bucket: self.primary_window.start_bucket,
            end_bucket_exclusive: self.primary_window.end_bucket_exclusive,
            maturity_bucket_exclusive: closes_at_bucket,
            repair_generation,
            coverage_policy_id: self.primary_window.coverage_policy_id,
            coverage_policy_parameter: self.primary_window.coverage_policy_parameter,
        };
        window.validate()?;
        Ok(window)
    }

    /// Encode the complete semantic root. It contains no liveness balance.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.check()?;
        let mut writer = Writer::new(output)?;
        writer.bytes(&MAGIC)?;
        writer.u16(VERSION)?;
        writer.reserved(6)?;
        encode_binding(&mut writer, self.binding)?;
        writer.bytes(&self.binding_id.bytes())?;
        writer.bytes(&self.attachment_plan_id)?;
        writer.bytes(&self.source_release_manifest_id.bytes())?;
        writer.bytes(&self.source_release_authentication_id.bytes())?;
        let mut window = [0u8; clutch_source_plane_v3::WINDOW_SPEC_BYTES];
        clutch_source_plane_v3::FixedCodec::encode_into(&self.primary_window, &mut window)?;
        writer.bytes(&window)?;
        let mut recovery = [0u8; EXTERNAL_RECOVERY_STATE_V1_BYTES];
        self.recovery.encode_into(&mut recovery)?;
        writer.bytes(&recovery)?;
        match self.trigger {
            None => writer.reserved(96)?,
            Some(trigger) => {
                writer.u8(1)?;
                writer.u8(trigger_kind_code(trigger.kind))?;
                writer.reserved(2)?;
                writer.u32(trigger.refusal_code)?;
                writer.bytes(&trigger.id.bytes())?;
                writer.bytes(&trigger.evidence_id)?;
                writer.u64(trigger.clock.slot)?;
                writer.i64(trigger.clock.unix_timestamp)?;
                writer.u64(trigger.clock.current_bucket)?;
            }
        }
        writer.finish()
    }

    /// Decode and fully validate one exact semantic root.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input)?;
        if reader.bytes::<8>()? != MAGIC {
            return Err(Error::BadMagic);
        }
        if reader.u16()? != VERSION {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let binding = decode_binding(&mut reader)?;
        let binding_id = FailurePolicyBindingId::from_bytes(reader.bytes()?);
        let attachment_plan_id = reader.bytes()?;
        let source_release_manifest_id = SourceContentId::from_bytes(reader.bytes()?);
        let source_release_authentication_id = SourceContentId::from_bytes(reader.bytes()?);
        let window = reader.bytes::<{ clutch_source_plane_v3::WINDOW_SPEC_BYTES }>()?;
        let primary_window = <WindowSpecV3 as clutch_source_plane_v3::FixedCodec>::decode(&window)?;
        let recovery_bytes = reader.bytes::<EXTERNAL_RECOVERY_STATE_V1_BYTES>()?;
        let recovery = ExternalRecoveryStateV1::decode(&recovery_bytes)?;
        let trigger = match reader.u8()? {
            0 => {
                reader.reserved(95)?;
                None
            }
            1 => {
                let kind = decode_trigger_kind(reader.u8()?)?;
                reader.reserved(2)?;
                let refusal_code = reader.u32()?;
                let id = FailureTriggerId::from_bytes(reader.bytes()?);
                let evidence_id = reader.bytes()?;
                let clock = RecoveryClock {
                    slot: reader.u64()?,
                    unix_timestamp: reader.i64()?,
                    current_bucket: reader.u64()?,
                };
                Some(FailureTriggerV1 {
                    id,
                    kind,
                    evidence_id,
                    refusal_code,
                    clock,
                })
            }
            _ => return Err(Error::InvalidEnum),
        };
        reader.finish()?;
        let runtime = Self {
            binding,
            binding_id,
            attachment_plan_id,
            source_release_manifest_id,
            source_release_authentication_id,
            primary_window,
            recovery,
            trigger,
        };
        runtime.check()?;
        Ok(runtime)
    }
}

/// Atomic semantic transition and optional liveness-consumable work receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureExternalTransitionPlanV2 {
    before: FailureRuntimeExternalV2,
    after: FailureRuntimeExternalV2,
    work: Option<FailureRecoveryWorkReceiptV2>,
}

impl FailureExternalTransitionPlanV2 {
    /// Resulting semantic phase.
    pub const fn resulting_phase(self) -> RecoveryPhase {
        self.after.phase()
    }

    /// Exact work receipt, absent for non-work transitions.
    pub const fn work(self) -> Option<FailureRecoveryWorkReceiptV2> {
        self.work
    }
}

impl FailureRuntimeExternalV2 {
    /// Commit a current semantic plan. If `work()` is present, the caller must
    /// commit the corresponding liveness transition in the same atomic batch.
    pub fn commit_plan(&mut self, plan: FailureExternalTransitionPlanV2) -> Result<()> {
        self.check()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        plan.after.check()?;
        *self = plan.after;
        Ok(())
    }

    /// Commit every canonical semantic byte of the current runtime. This is
    /// distinct from the stable recovery-state account identity and prevents
    /// same-nonce sibling poststates from sharing a terminal receipt.
    pub fn state_commitment(&self) -> Result<FailureRuntimeStateCommitmentV2> {
        let mut bytes = [0u8; FAILURE_RUNTIME_EXTERNAL_V2_BYTES];
        self.encode_into(&mut bytes)?;
        let mut hasher = Sha256::new();
        hasher.update(RUNTIME_STATE_COMMITMENT_DOMAIN);
        hasher.update(bytes);
        Ok(FailureRuntimeStateCommitmentV2::from_bytes(
            hasher.finalize().into(),
        ))
    }

    /// Emit an authenticated terminal receipt for exactly the Recovery
    /// compartment. Resolution is success; exhausted recovery is failure.
    pub fn recovery_terminal_receipt(&self) -> Result<FailureRecoveryTerminalReceiptV2> {
        self.check()?;
        let disposition = match self.phase() {
            RecoveryPhase::Resolved => FailureRecoveryTerminalDispositionV2::Resolved,
            RecoveryPhase::RecoveryDormant => FailureRecoveryTerminalDispositionV2::Dormant,
            RecoveryPhase::Active | RecoveryPhase::DegradedRecoverable => {
                return Err(Error::WrongPhase)
            }
        };
        let funding = self.recovery.funding();
        let runtime_state_commitment = self.state_commitment()?;
        let mut hasher = Sha256::new();
        hasher.update(RECOVERY_TERMINAL_DOMAIN);
        hasher.update(self.binding_id.bytes());
        hasher.update(self.binding.market_instance_id.bytes());
        hasher.update(self.recovery.state_id().bytes());
        hasher.update(funding.policy_id.bytes());
        hasher.update(funding.lifecycle_id.bytes());
        hasher.update(funding.recovery_account_id.bytes());
        hasher.update(funding.semantic_owner.bytes());
        hasher.update(funding.quote_schedule_id.bytes());
        hasher.update(self.binding.generation.to_le_bytes());
        hasher.update(self.transition_nonce().to_le_bytes());
        hasher.update(runtime_state_commitment.bytes());
        hasher.update([disposition as u8]);
        Ok(FailureRecoveryTerminalReceiptV2 {
            id: FailureRecoveryTerminalReceiptIdV2::from_bytes(hasher.finalize().into()),
            semantic_state_id: self.recovery.state_id(),
            liveness_policy_id: LivenessId::from_bytes(funding.policy_id.bytes()),
            liveness_lifecycle_id: LivenessId::from_bytes(funding.lifecycle_id.bytes()),
            recovery_compartment_account_id: LivenessId::from_bytes(
                funding.recovery_account_id.bytes(),
            ),
            semantic_owner: LivenessId::from_bytes(funding.semantic_owner.bytes()),
            quote_schedule_id: LivenessId::from_bytes(funding.quote_schedule_id.bytes()),
            generation: self.binding.generation,
            transition_nonce: self.transition_nonce(),
            runtime_state_commitment,
            disposition,
        })
    }
}

/// Closed semantic disposition for the Recovery compartment only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureRecoveryTerminalDispositionV2 {
    /// Accepted evidence resolved the occurrence.
    Resolved = 1,
    /// Every finite repair Window expired without accepted resolution.
    Dormant = 2,
}

/// Authenticated terminal receipt consumed by the liveness Recovery close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRecoveryTerminalReceiptV2 {
    id: FailureRecoveryTerminalReceiptIdV2,
    semantic_state_id: RecoveryIdentity,
    liveness_policy_id: LivenessId,
    liveness_lifecycle_id: LivenessId,
    recovery_compartment_account_id: LivenessId,
    semantic_owner: LivenessId,
    quote_schedule_id: LivenessId,
    generation: u64,
    transition_nonce: u64,
    runtime_state_commitment: FailureRuntimeStateCommitmentV2,
    disposition: FailureRecoveryTerminalDispositionV2,
}

/// Full terminal join owned separately from the Recovery funding close.
///
/// Dormancy cannot construct this join: the occurrence must first resolve,
/// and the adapter must authenticate retirement, permanent replay tombstone,
/// and final Source release facts for the same generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureExternalTerminalJoinV2 {
    id: FailureExternalTerminalJoinIdV2,
    binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    recovery_terminal_receipt_id: FailureRecoveryTerminalReceiptIdV2,
    transition_nonce: u64,
    retirement_root_id: [u8; 32],
    replay_tombstone_id: [u8; 32],
    source_release_receipt_id: [u8; 32],
}

impl FailureExternalTerminalJoinV2 {
    /// Construct only after the adapter authenticates all three external facts.
    pub fn from_adapter(
        runtime: &FailureRuntimeExternalV2,
        generation: u64,
        retirement_root_id: [u8; 32],
        replay_tombstone_id: [u8; 32],
        source_release_receipt_id: [u8; 32],
    ) -> Result<Self> {
        runtime.check()?;
        if runtime.phase() != RecoveryPhase::Resolved {
            return Err(Error::WrongPhase);
        }
        if generation != runtime.binding.generation
            || retirement_root_id.iter().all(|byte| *byte == 0)
            || replay_tombstone_id.iter().all(|byte| *byte == 0)
            || source_release_receipt_id.iter().all(|byte| *byte == 0)
        {
            return Err(Error::BindingMismatch);
        }
        let recovery_terminal = runtime.recovery_terminal_receipt()?;
        let mut hasher = Sha256::new();
        hasher.update(FULL_TERMINAL_DOMAIN);
        hasher.update(runtime.binding_id.bytes());
        hasher.update(runtime.binding.market_instance_id.bytes());
        hasher.update(generation.to_le_bytes());
        hasher.update(recovery_terminal.id().bytes());
        hasher.update(runtime.transition_nonce().to_le_bytes());
        hasher.update(retirement_root_id);
        hasher.update(replay_tombstone_id);
        hasher.update(source_release_receipt_id);
        Ok(Self {
            id: FailureExternalTerminalJoinIdV2::from_bytes(hasher.finalize().into()),
            binding_id: runtime.binding_id,
            market_instance_id: runtime.binding.market_instance_id,
            generation,
            recovery_terminal_receipt_id: recovery_terminal.id(),
            transition_nonce: runtime.transition_nonce(),
            retirement_root_id,
            replay_tombstone_id,
            source_release_receipt_id,
        })
    }

    /// Complete typed join identity.
    pub const fn id(self) -> FailureExternalTerminalJoinIdV2 {
        self.id
    }

    /// Immutable failure-policy binding.
    pub const fn binding_id(self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact full-width occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact terminal generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact resolved Recovery terminal receipt for the current root state.
    pub const fn recovery_terminal_receipt_id(self) -> FailureRecoveryTerminalReceiptIdV2 {
        self.recovery_terminal_receipt_id
    }

    /// Exact resolved semantic transition nonce committed by this join.
    pub const fn transition_nonce(self) -> u64 {
        self.transition_nonce
    }

    /// Separately authenticated retirement root.
    pub const fn retirement_root_id(self) -> [u8; 32] {
        self.retirement_root_id
    }

    /// Permanent replay tombstone preserved after root closure.
    pub const fn replay_tombstone_id(self) -> [u8; 32] {
        self.replay_tombstone_id
    }

    /// Final Source release/lineage receipt.
    pub const fn source_release_receipt_id(self) -> [u8; 32] {
        self.source_release_receipt_id
    }
}

impl FailureRecoveryTerminalReceiptV2 {
    /// Exact typed terminal receipt identity.
    pub const fn id(self) -> FailureRecoveryTerminalReceiptIdV2 {
        self.id
    }

    /// Exact semantic transition nonce at terminal classification.
    pub const fn transition_nonce(self) -> u64 {
        self.transition_nonce
    }

    /// Commitment to the complete canonical resolved or dormant runtime.
    pub const fn runtime_state_commitment(self) -> FailureRuntimeStateCommitmentV2 {
        self.runtime_state_commitment
    }

    /// Recovery-only terminal classification.
    pub const fn disposition(self) -> FailureRecoveryTerminalDispositionV2 {
        self.disposition
    }

    /// Construct the only admissible liveness close intent.
    pub fn runtime_transition_intent(self) -> RuntimeTransitionIntentV1 {
        RuntimeTransitionIntentV1 {
            action: match self.disposition {
                FailureRecoveryTerminalDispositionV2::Resolved => {
                    RuntimeTransitionActionV1::CloseSuccess
                }
                FailureRecoveryTerminalDispositionV2::Dormant => {
                    RuntimeTransitionActionV1::CloseFailure
                }
            },
            kind: RuntimeCompartmentKindV1::Recovery,
            policy_id: self.liveness_policy_id,
            lifecycle_id: self.liveness_lifecycle_id,
            account_id: self.recovery_compartment_account_id,
            semantic_owner: self.semantic_owner,
            quote_schedule_id: self.quote_schedule_id,
            receipt_id: LivenessId::from_bytes(self.id.bytes()),
            keeper: LivenessId::ZERO,
            generation: self.generation,
            call_ordinal: 0,
            call_ceiling_lamports: 0,
            keeper_payment_lamports: 0,
            flags: 0,
        }
    }

    /// Project authenticated private fields for the liveness terminal close.
    pub fn runtime_receipt_observation(
        self,
        receipt_account_id: LivenessId,
        receipt_account_owner_program_id: LivenessId,
    ) -> Result<RuntimeReceiptObservationV1> {
        if receipt_account_id.bytes() != self.semantic_state_id.bytes()
            || receipt_account_owner_program_id != self.semantic_owner
        {
            return Err(Error::BindingMismatch);
        }
        Ok(RuntimeReceiptObservationV1 {
            receipt_account_id,
            receipt_account_owner_program_id,
            receipt_id: LivenessId::from_bytes(self.id.bytes()),
            receipt_kind: match self.disposition {
                FailureRecoveryTerminalDispositionV2::Resolved => {
                    RuntimeReceiptKindV1::TerminalSuccess
                }
                FailureRecoveryTerminalDispositionV2::Dormant => {
                    RuntimeReceiptKindV1::TerminalFailure
                }
            },
            compartment_kind: RuntimeCompartmentKindV1::Recovery,
            semantic_owner: self.semantic_owner,
            lifecycle_id: self.liveness_lifecycle_id,
            quote_schedule_id: self.quote_schedule_id,
            generation: self.generation,
            call_ordinal: 0,
            call_ceiling_lamports: 0,
        })
    }
}

fn recovery_clock_from_snapshot(
    policy: &ClockPolicyV1,
    snapshot: ClockSnapshotV1,
) -> Result<RecoveryClock> {
    policy.validate()?;
    Ok(RecoveryClock {
        slot: snapshot.slot,
        unix_timestamp: snapshot.unix_timestamp,
        current_bucket: policy.bucket_for_timestamp(snapshot.unix_timestamp)?,
    })
}

fn trigger_kind_code(kind: FailureTriggerKindV1) -> u8 {
    match kind {
        FailureTriggerKindV1::PrimaryMaturityWithoutAcceptedResolution => 1,
        FailureTriggerKindV1::SourceEvaluationRefused => 2,
        FailureTriggerKindV1::ResolutionRelationRefused => 3,
    }
}

fn decode_trigger_kind(value: u8) -> Result<FailureTriggerKindV1> {
    match value {
        1 => Ok(FailureTriggerKindV1::PrimaryMaturityWithoutAcceptedResolution),
        2 => Ok(FailureTriggerKindV1::SourceEvaluationRefused),
        3 => Ok(FailureTriggerKindV1::ResolutionRelationRefused),
        _ => Err(Error::InvalidEnum),
    }
}

fn encode_binding(writer: &mut Writer<'_>, binding: FailurePolicyBindingV1) -> Result<()> {
    writer.bytes(&binding.series_plan_id.bytes())?;
    writer.u32(binding.ordinal)?;
    writer.reserved(4)?;
    writer.bytes(&binding.market_instance_id.bytes())?;
    writer.bytes(&binding.product_template_id.bytes())?;
    writer.bytes(&binding.recovery_policy_id.bytes())?;
    writer.bytes(&binding.funding_quote_id.bytes())?;
    writer.bytes(&binding.funding_terms_id.bytes())?;
    writer.bytes(&binding.source_plane_program_id.bytes())?;
    writer.bytes(&binding.source_spec_id.bytes())?;
    writer.bytes(&binding.summary_program_id.bytes())?;
    writer.bytes(&binding.primary_window_id.bytes())?;
    writer.bytes(&binding.statistic_key_id.bytes())?;
    writer.bytes(&binding.source_occurrence_receipt_id.bytes())?;
    writer.bytes(&binding.clock_policy_id.bytes())?;
    writer.bytes(&binding.relation_policy_id)?;
    writer.bytes(&binding.recovery_state_id.bytes())?;
    writer.u64(binding.generation)
}

fn decode_binding(reader: &mut Reader<'_>) -> Result<FailurePolicyBindingV1> {
    Ok(FailurePolicyBindingV1 {
        series_plan_id: clutch_product_series::SeriesPlanV5Id::from_bytes(reader.bytes()?),
        ordinal: reader.u32()?,
        market_instance_id: {
            reader.reserved(4)?;
            MarketInstanceV2Id::from_bytes(reader.bytes()?)
        },
        product_template_id: clutch_product_series::ProductTemplateId::from_bytes(reader.bytes()?),
        recovery_policy_id: clutch_product_series::EvidenceOnlyRecoveryPolicyId::from_bytes(
            reader.bytes()?,
        ),
        funding_quote_id: SeriesFundingQuoteId::from_bytes(reader.bytes()?),
        funding_terms_id: clutch_product_series::SeriesFundingTermsV2Id::from_bytes(
            reader.bytes()?,
        ),
        source_plane_program_id: SourceContentId::from_bytes(reader.bytes()?),
        source_spec_id: SourceContentId::from_bytes(reader.bytes()?),
        summary_program_id: SourceContentId::from_bytes(reader.bytes()?),
        primary_window_id: SourceContentId::from_bytes(reader.bytes()?),
        statistic_key_id: SourceContentId::from_bytes(reader.bytes()?),
        source_occurrence_receipt_id: SourceContentId::from_bytes(reader.bytes()?),
        clock_policy_id: SourceContentId::from_bytes(reader.bytes()?),
        relation_policy_id: reader.bytes()?,
        recovery_state_id: RecoveryIdentity::from_bytes(reader.bytes()?),
        generation: reader.u64()?,
    })
}

struct Writer<'a> {
    output: &'a mut [u8],
    cursor: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Result<Self> {
        if output.len() != FAILURE_RUNTIME_EXTERNAL_V2_BYTES {
            return Err(Error::WrongLength);
        }
        Ok(Self { output, cursor: 0 })
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        let end = self
            .cursor
            .checked_add(bytes.len())
            .ok_or(Error::WrongLength)?;
        self.output
            .get_mut(self.cursor..end)
            .ok_or(Error::WrongLength)?
            .copy_from_slice(bytes);
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
        let end = self.cursor.checked_add(count).ok_or(Error::WrongLength)?;
        self.output
            .get_mut(self.cursor..end)
            .ok_or(Error::WrongLength)?
            .fill(0);
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.cursor == self.output.len() {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}

struct Reader<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn new(input: &'a [u8]) -> Result<Self> {
        if input.len() != FAILURE_RUNTIME_EXTERNAL_V2_BYTES {
            return Err(Error::WrongLength);
        }
        Ok(Self { input, cursor: 0 })
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.cursor.checked_add(N).ok_or(Error::WrongLength)?;
        let source = self.input.get(self.cursor..end).ok_or(Error::WrongLength)?;
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
        let end = self.cursor.checked_add(count).ok_or(Error::WrongLength)?;
        let bytes = self.input.get(self.cursor..end).ok_or(Error::WrongLength)?;
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        self.cursor = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.cursor == self.input.len() {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}
