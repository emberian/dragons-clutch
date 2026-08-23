// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Successor failure-policy and funded evidence-recovery runtime join.
//!
//! This crate composes existing semantic owners. It neither owns a payout
//! vector nor treats elapsed time, source refusal, ambiguity, or stale evidence
//! as a value fact. The only live consequence is finite independently prepaid
//! evidence recovery followed by recoverable dormancy.

pub mod external_v2;
pub mod interval_consensus_v1;
pub mod market_policy_v1;
pub mod market_quote_v1;
pub mod market_runtime_v1;
pub mod relation_execution_v1;
pub mod retirement_v1;

use clutch_evidence_recovery::{
    EvidenceDecision, FundingObservation, Identity as RecoveryIdentity, RecoveryAdmission,
    RecoveryClock, RecoveryError, RecoveryLedger, RecoveryPhase, RecoveryState, TransferPlan,
    TransitionPlan as RecoveryTransitionPlan, RECOVERY_STATE_V2_BYTES,
};
use clutch_product_series::{
    compile_ordinal_v2, CompiledOrdinalV2, EvidenceOnlyRecoveryPolicyId,
    EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV2, MarketInstanceV2Id, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateId, ProductTemplateV4, RegistryCapabilityProjectionV2,
    SeriesAttachmentPlanV1, SeriesFundingQuoteId, SeriesFundingQuoteV1, SeriesFundingTermsV2,
    SeriesFundingTermsV2Id, SeriesPlanV5, SeriesPlanV5Id,
};
use clutch_source_plane_v3::{
    ContentId as SourceContentId, SourcePlaneProgramV3, StatisticKeyV3, StatisticResultStatusV3,
    StatisticResultV3, SummaryProgramV3, WindowSealV3, WindowSpecV3,
};
use clutch_source_plane_v3_runtime::{
    ClockPolicyV1, ClockSnapshotV1, FailurePolicySourceHandoffV1, OccurrenceSourceReceiptV1,
    SourceFailureKindV1,
};
use sha2::{Digest, Sha256};

const POLICY_BINDING_DOMAIN: &[u8] = b"dragons-clutch/failure-policy-binding/v1";
const ADMISSION_RECEIPT_DOMAIN: &[u8] = b"dragons-clutch/failure-admission-receipt/v1";
const TRIGGER_DOMAIN: &[u8] = b"dragons-clutch/failure-trigger/v1";
const ACCEPTED_RESOLUTION_DOMAIN: &[u8] = b"dragons-clutch/failure-accepted-resolution/v1";
const RECOVERY_TERMINAL_RECEIPT_DOMAIN: &[u8] =
    b"dragons-clutch/failure-recovery-terminal-receipt/v1";
const TERMINAL_JOIN_DOMAIN: &[u8] = b"dragons-clutch/failure-terminal-join/v1";
const FAILURE_RUNTIME_MAGIC: [u8; 8] = *b"DCFAILR1";
const FAILURE_RUNTIME_SCHEMA: u16 = 1;

/// Exact canonical width of one persisted successor failure runtime.
pub const FAILURE_RUNTIME_V1_BYTES: usize = 1_856;

/// Result alias for the successor failure-policy join.
pub type Result<T> = core::result::Result<T, Error>;

macro_rules! typed_id {
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

            /// Return the exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }

            fn is_zero(self) -> bool {
                self.0.iter().all(|byte| *byte == 0)
            }
        }
    };
}

typed_id!(
    FailurePolicyBindingId,
    "Typed identity of one immutable successor failure-policy binding."
);
typed_id!(
    FailureAdmissionReceiptId,
    "Typed identity of one presently capitalized failure-runtime admission."
);
typed_id!(
    FailureTriggerId,
    "Typed identity of one deterministic maturity trigger and its classification."
);
typed_id!(
    AcceptedResolutionId,
    "Typed identity of one source- and relation-bound accepted resolution."
);
typed_id!(
    FailureRecoveryTerminalReceiptId,
    "Typed receipt closing a finite recovery-liveness funding compartment."
);
typed_id!(
    FailureTerminalJoinId,
    "Typed identity of one separately authenticated terminal-owner join."
);

/// Deterministic refusal from the successor failure-policy composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A persisted failure runtime was not the one exact canonical width.
    WrongLength,
    /// A persisted failure runtime used another discriminator.
    BadMagic,
    /// A persisted failure runtime used another schema version.
    BadVersion,
    /// Reserved persisted bytes were nonzero.
    NonCanonicalReserved,
    /// A persisted enum discriminant was unknown.
    InvalidEnum,
    /// A Product/Series semantic owner refused the supplied join.
    Product(clutch_product_series::Error),
    /// SourcePlane refused the supplied source/evaluator join.
    Source(clutch_source_plane_v3::Error),
    /// SourcePlane runtime/account owner refused the supplied authenticated handoff.
    SourceRuntime(clutch_source_plane_v3_runtime::Error),
    /// The funded recovery owner refused the transition.
    Recovery(RecoveryError),
    /// An exact typed identity or immutable field did not match.
    BindingMismatch,
    /// A required identity was the reserved all-zero value.
    ZeroIdentity,
    /// The immutable source window or recovery generation was not eligible.
    WrongRecoveryWindow,
    /// A trigger was attempted before immutable primary maturity.
    TriggerBeforeMaturity,
    /// A second trigger attempted to replace the recorded first trigger.
    TriggerAlreadyRecorded,
    /// A source refusal was required but the exact result succeeded.
    SourceDidNotRefuse,
    /// An accepted source result was required but the exact result refused.
    SourceDidNotSucceed,
    /// A resolution or terminal join targeted the wrong recovery phase.
    WrongPhase,
    /// A transition plan no longer matches the complete runtime state.
    StalePlan,
    /// The actual post-transfer reserve balance differs from the exact plan.
    PostBalanceMismatch,
}

impl From<clutch_product_series::Error> for Error {
    fn from(value: clutch_product_series::Error) -> Self {
        Self::Product(value)
    }
}

impl From<clutch_source_plane_v3::Error> for Error {
    fn from(value: clutch_source_plane_v3::Error) -> Self {
        Self::Source(value)
    }
}

impl From<clutch_source_plane_v3_runtime::Error> for Error {
    fn from(value: clutch_source_plane_v3_runtime::Error) -> Self {
        Self::SourceRuntime(value)
    }
}

impl From<RecoveryError> for Error {
    fn from(value: RecoveryError) -> Self {
        Self::Recovery(value)
    }
}

/// The only failure consequence implemented by this runtime generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureConsequenceV1 {
    /// Finite prepaid repair followed by recoverable dormancy; never a payout.
    EvidenceOnlyRecovery,
}

/// Immutable exact joins for one V5 Series occurrence.
///
/// Fields are private so only [`FailureRuntimeV1::admit_successor`] can mint a
/// binding after recomputing the complete Product/Series and SourcePlane graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailurePolicyBindingV1 {
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    product_template_id: ProductTemplateId,
    recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    funding_quote_id: SeriesFundingQuoteId,
    funding_terms_id: SeriesFundingTermsV2Id,
    source_plane_program_id: SourceContentId,
    source_spec_id: SourceContentId,
    summary_program_id: SourceContentId,
    primary_window_id: SourceContentId,
    statistic_key_id: SourceContentId,
    source_occurrence_receipt_id: SourceContentId,
    clock_policy_id: SourceContentId,
    relation_policy_id: [u8; 32],
    recovery_state_id: RecoveryIdentity,
    generation: u64,
}

impl FailurePolicyBindingV1 {
    /// Typed identity of this exact immutable cross-owner join.
    pub fn id(&self) -> FailurePolicyBindingId {
        let mut hasher = Sha256::new();
        hasher.update(POLICY_BINDING_DOMAIN);
        hasher.update(self.series_plan_id.bytes());
        hasher.update(self.ordinal.to_le_bytes());
        hasher.update(self.market_instance_id.bytes());
        hasher.update(self.product_template_id.bytes());
        hasher.update(self.recovery_policy_id.bytes());
        hasher.update(self.funding_quote_id.bytes());
        hasher.update(self.funding_terms_id.bytes());
        hasher.update(self.source_plane_program_id.bytes());
        hasher.update(self.source_spec_id.bytes());
        hasher.update(self.summary_program_id.bytes());
        hasher.update(self.primary_window_id.bytes());
        hasher.update(self.statistic_key_id.bytes());
        hasher.update(self.source_occurrence_receipt_id.bytes());
        hasher.update(self.clock_policy_id.bytes());
        hasher.update(self.relation_policy_id);
        hasher.update(self.recovery_state_id.bytes());
        hasher.update(self.generation.to_le_bytes());
        FailurePolicyBindingId::from_bytes(hasher.finalize().into())
    }

    /// Exact V5 Series identity.
    pub const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact ordinal in the finite V5 Series.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Full-width V2 economic occurrence identity.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact funding quote whose budget is admitted.
    pub const fn funding_quote_id(&self) -> SeriesFundingQuoteId {
        self.funding_quote_id
    }

    /// Exact successor funding-ownership artifact.
    pub const fn funding_terms_id(&self) -> SeriesFundingTermsV2Id {
        self.funding_terms_id
    }

    /// Exact reusable ProductTemplate identity.
    pub const fn product_template_id(&self) -> ProductTemplateId {
        self.product_template_id
    }

    /// Exact evidence-only recovery-policy identity.
    pub const fn recovery_policy_id(&self) -> EvidenceOnlyRecoveryPolicyId {
        self.recovery_policy_id
    }

    /// Exact SourcePlane release/contract identity.
    pub const fn source_plane_program_id(&self) -> SourceContentId {
        self.source_plane_program_id
    }

    /// Exact source-description identity.
    pub const fn source_spec_id(&self) -> SourceContentId {
        self.source_spec_id
    }

    /// Exact source-neutral evaluator identity.
    pub const fn summary_program_id(&self) -> SourceContentId {
        self.summary_program_id
    }

    /// Exact predictable primary Window identity.
    pub const fn primary_window_id(&self) -> SourceContentId {
        self.primary_window_id
    }

    /// Exact predictable statistic-request identity.
    pub const fn statistic_key_id(&self) -> SourceContentId {
        self.statistic_key_id
    }

    /// Authenticated Product/Series-to-Source occurrence receipt identity.
    pub const fn source_occurrence_receipt_id(&self) -> SourceContentId {
        self.source_occurrence_receipt_id
    }

    /// Immutable source Clock/bucket policy identity.
    pub const fn clock_policy_id(&self) -> SourceContentId {
        self.clock_policy_id
    }

    /// Exact frozen settlement/evidence-relation policy identity.
    pub const fn relation_policy_id(&self) -> [u8; 32] {
        self.relation_policy_id
    }

    /// Exact recovery state/reserve identity.
    pub const fn recovery_state_id(&self) -> RecoveryIdentity {
        self.recovery_state_id
    }

    /// Nonzero recovery-state generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

/// Private-field receipt proving the recovery owner admitted present principal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureAdmissionReceiptV1 {
    binding_id: FailurePolicyBindingId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    funding_quote_id: SeriesFundingQuoteId,
    recovery_state_id: RecoveryIdentity,
    generation: u64,
    work_principal_lamports: u64,
    rent_principal_lamports: u64,
    admitted_reserve_balance: u64,
}

impl FailureAdmissionReceiptV1 {
    /// Typed digest of the exact activation receipt.
    pub fn id(&self) -> FailureAdmissionReceiptId {
        let mut hasher = Sha256::new();
        hasher.update(ADMISSION_RECEIPT_DOMAIN);
        hasher.update(self.binding_id.bytes());
        hasher.update(self.series_plan_id.bytes());
        hasher.update(self.ordinal.to_le_bytes());
        hasher.update(self.market_instance_id.bytes());
        hasher.update(self.funding_quote_id.bytes());
        hasher.update(self.recovery_state_id.bytes());
        hasher.update(self.generation.to_le_bytes());
        hasher.update(self.work_principal_lamports.to_le_bytes());
        hasher.update(self.rent_principal_lamports.to_le_bytes());
        hasher.update(self.admitted_reserve_balance.to_le_bytes());
        FailureAdmissionReceiptId::from_bytes(hasher.finalize().into())
    }

    /// Exact immutable failure-policy binding.
    pub const fn binding_id(&self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact V5 Series identity.
    pub const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact occurrence ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Exact full-width V2 market identity.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact quote whose independently owned principal is present.
    pub const fn funding_quote_id(&self) -> SeriesFundingQuoteId {
        self.funding_quote_id
    }

    /// Exact recovery state/reserve identity.
    pub const fn recovery_state_id(&self) -> RecoveryIdentity {
        self.recovery_state_id
    }

    /// Exact nonzero state generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Present work principal owned by the recovery runtime.
    pub const fn work_principal_lamports(&self) -> u64 {
        self.work_principal_lamports
    }

    /// Present distinct rent principal owned by the recovery runtime.
    pub const fn rent_principal_lamports(&self) -> u64 {
        self.rent_principal_lamports
    }

    /// Exact reserve balance observed after admission transfers.
    pub const fn admitted_reserve_balance(&self) -> u64 {
        self.admitted_reserve_balance
    }
}

/// Closed deterministic relation refusals that may classify a maturity trigger.
///
/// Every class means the frozen relation selected no payout. The class never
/// changes the maturity boundary, repair budget, or terminal disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RelationRefusalV1 {
    /// Conservative evidence straddled more than one admissible value region.
    AmbiguousInterval = 1,
    /// A ratio statistic's denominator interval included zero.
    AmbiguousDenominator = 2,
    /// The frozen refusing edge policy rejected the value interval.
    ValueOutOfRange = 3,
    /// Smooth degree-two or degree-three semantics received non-point evidence.
    NonPointEvidence = 4,
    /// The exact sealed source window contained no accepted coverage.
    NoAcceptedCoverage = 5,
}

impl RelationRefusalV1 {
    /// Stable code committed by trigger and adapter intent preimages.
    pub const fn code(self) -> u32 {
        match self {
            Self::AmbiguousInterval => 1,
            Self::AmbiguousDenominator => 2,
            Self::ValueOutOfRange => 3,
            Self::NonPointEvidence => 4,
            Self::NoAcceptedCoverage => 5,
        }
    }
}

/// Adapter-authenticated deterministic relation refusal.
///
/// This is a forgeable boundary DTO. A live adapter must construct it only
/// after running the immutable relation release named by `relation_policy_id`.
/// Even a forged value cannot trigger before maturity or select a payout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterAuthenticatedRelationRefusalV1 {
    /// Exact failure-policy binding.
    pub binding_id: FailurePolicyBindingId,
    /// Full-width market occurrence.
    pub market_instance_id: MarketInstanceV2Id,
    /// Recovery generation.
    pub generation: u64,
    /// Exact successful SourcePlane statistic result inspected by the relation.
    pub statistic_result_id: SourceContentId,
    /// Frozen relation implementation/policy identity.
    pub relation_policy_id: [u8; 32],
    /// Deterministic no-payout refusal class.
    pub refusal: RelationRefusalV1,
}

/// Classification of the first immutable-maturity transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureTriggerKindV1 {
    /// No accepted resolution existed when primary maturity arrived.
    PrimaryMaturityWithoutAcceptedResolution,
    /// The exact SourcePlane evaluator emitted a stable refusal.
    SourceEvaluationRefused,
    /// The exact frozen relation refused a successful source result.
    ResolutionRelationRefused,
}

/// Recorded deterministic trigger; it contains no payout or resolver identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureTriggerV1 {
    id: FailureTriggerId,
    kind: FailureTriggerKindV1,
    evidence_id: [u8; 32],
    refusal_code: u32,
    clock: RecoveryClock,
}

impl FailureTriggerV1 {
    /// Typed trigger identity.
    pub const fn id(&self) -> FailureTriggerId {
        self.id
    }

    /// Exact trigger classification.
    pub const fn kind(&self) -> FailureTriggerKindV1 {
        self.kind
    }

    /// Source or relation evidence identity used only for classification.
    pub const fn evidence_id(&self) -> [u8; 32] {
        self.evidence_id
    }

    /// Stable source/relation refusal code, or zero for maturity alone.
    pub const fn refusal_code(&self) -> u32 {
        self.refusal_code
    }

    /// Exact monotone clock at the maturity transition.
    pub const fn clock(&self) -> RecoveryClock {
        self.clock
    }
}

/// Exact SourcePlane repair-window identity for one compiled attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryWorkJoinV1 {
    /// Zero-based attempt index in the immutable compiled schedule.
    pub attempt_index: u8,
    /// Exact compiled source repair generation.
    pub repair_generation: u64,
    /// Exact deterministic SourcePlane Window identity.
    pub window_id: SourceContentId,
    /// Exact immutable FundingQuote identity pricing this attempt.
    pub funding_quote_id: SeriesFundingQuoteId,
    /// Exact cumulative progress cap for this attempt.
    pub max_progress_units: u64,
    /// Exact lamports paid per newly accepted progress unit.
    pub lamports_per_progress_unit: u64,
    /// Maximum additional lamports still payable at the current cursor.
    pub maximum_remaining_lamports: u64,
}

/// Private-field join to one liveness-runtime accepted-work receipt.
///
/// A live adapter must authenticate the receipt under the liveness runtime
/// before calling [`FailureRuntimeV1::join_liveness_work_receipt`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivenessWorkReceiptJoinV1 {
    work_receipt_id: [u8; 32],
    binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    attempt_index: u8,
    window_id: SourceContentId,
    accepted_progress_total: u64,
    quote_schedule_id: [u8; 32],
    scheduled_ceiling_lamports: u64,
}

impl LivenessWorkReceiptJoinV1 {
    /// Exact authenticated liveness-runtime work receipt.
    pub const fn work_receipt_id(&self) -> [u8; 32] {
        self.work_receipt_id
    }

    /// Exact cumulative accepted progress carried by the receipt.
    pub const fn accepted_progress_total(&self) -> u64 {
        self.accepted_progress_total
    }

    /// Exact liveness quote-schedule identity, equal to the FundingQuote ID.
    pub const fn quote_schedule_id(&self) -> [u8; 32] {
        self.quote_schedule_id
    }

    /// Exact authenticated per-call liveness ceiling.
    pub const fn scheduled_ceiling_lamports(&self) -> u64 {
        self.scheduled_ceiling_lamports
    }
}

/// Private-field capability for an exact successful source result and relation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedResolutionV1 {
    id: AcceptedResolutionId,
    binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    window_id: SourceContentId,
    statistic_result_id: SourceContentId,
    relation_record_id: [u8; 32],
}

impl AcceptedResolutionV1 {
    /// Typed accepted-resolution identity passed to the funded recovery owner.
    pub const fn id(&self) -> AcceptedResolutionId {
        self.id
    }

    /// Exact SourcePlane Window whose accepted evidence was related.
    pub const fn window_id(&self) -> SourceContentId {
        self.window_id
    }

    /// Exact successful SourcePlane statistic-result identity.
    pub const fn statistic_result_id(&self) -> SourceContentId {
        self.statistic_result_id
    }

    /// Exact adapter-authenticated frozen-relation record identity.
    pub const fn relation_record_id(&self) -> [u8; 32] {
        self.relation_record_id
    }
}

/// Complete pure runtime for one successor occurrence.
///
/// This is not a persisted ABI. The nested [`RecoveryState`] remains the sole
/// mutable owner of work, rent, donation, phase, and reserve conservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRuntimeV1 {
    binding: FailurePolicyBindingV1,
    binding_id: FailurePolicyBindingId,
    primary_window: WindowSpecV3,
    statistic_key_id: SourceContentId,
    summary_program_id: SourceContentId,
    recovery: RecoveryState,
    trigger: Option<FailureTriggerV1>,
}

impl FailureRuntimeV1 {
    /// Recompute every immutable successor and SourcePlane join, then admit
    /// exact present work/rent principal into the recovery owner.
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
        clock_policy: &ClockPolicyV1,
        recovery_admission: RecoveryAdmission,
        creation_clock: ClockSnapshotV1,
        funding_observation: FundingObservation,
    ) -> Result<(Self, FailureAdmissionReceiptV1)> {
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
        if attachment.funding_quote_id != funding_quote.id()?
            || recovery_admission.series_funding_quote_id != funding_quote.id()?
            || recovery_admission.work_funder.bytes()
                != funding_terms.lamport_principal_refund.bytes()
            || recovery_admission.rent_payer.bytes()
                != funding_terms.lamport_principal_refund.bytes()
            || recovery_admission.neutral_sink.bytes() != funding_terms.neutral_lamport_sink.bytes()
        {
            return Err(Error::BindingMismatch);
        }

        source_plane.validate()?;
        summary.validate()?;
        primary_window.validate()?;
        statistic_key.validate()?;
        let source_plane_id = source_plane.id()?;
        let summary_program_id = summary.id()?;
        let primary_window_id = primary_window.id()?;
        let statistic_key_id = statistic_key.id()?;
        let source_occurrence_receipt_id = source_occurrence.id();
        let clock_policy_id = clock_policy.id()?;
        let creation_clock = recovery_clock_from_snapshot(clock_policy, creation_clock)?;
        let expected_coverage = u16::try_from(template.coverage_policy_registry_value)
            .map_err(|_| Error::BindingMismatch)?;
        if template.source_plane_contract_id.bytes() != source_plane_id.bytes()
            || template.source_spec_id.bytes() != primary_window.source_spec_id.bytes()
            || template.summary_program_id.bytes() != summary_program_id.bytes()
            || primary_window.source_plane_program_id != source_plane_id
            || primary_window.start_bucket != compiled.schedule.start_bucket
            || primary_window.end_bucket_exclusive != compiled.schedule.end_bucket_exclusive
            || primary_window.maturity_bucket_exclusive
                != compiled.schedule.primary_maturity_bucket_exclusive
            || primary_window.repair_generation != template.base_repair_generation
            || primary_window.coverage_policy_id != expected_coverage
            || primary_window.coverage_policy_parameter != template.coverage_policy_parameter
            || statistic_key.window_id != primary_window_id
            || statistic_key.summary_program_id != summary_program_id
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
        {
            return Err(Error::BindingMismatch);
        }

        let quote_id = funding_quote.id()?;
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
            summary_program_id,
            primary_window_id,
            statistic_key_id,
            source_occurrence_receipt_id,
            clock_policy_id,
            relation_policy_id: genesis.relation_policy_id.bytes(),
            recovery_state_id: recovery_admission.state_id,
            generation: recovery_admission.generation,
        };
        let binding_id = binding.id();
        let recovery = RecoveryState::admit_v2(
            compiled.market_instance_id,
            recovery_policy.id()?,
            compiled.schedule,
            funding_quote,
            recovery_admission,
            creation_clock,
            funding_observation,
        )?;
        let runtime = Self {
            binding,
            binding_id,
            primary_window,
            statistic_key_id,
            summary_program_id,
            recovery,
            trigger: None,
        };
        runtime.check()?;
        let ledger = runtime.recovery.ledger();
        let receipt = FailureAdmissionReceiptV1 {
            binding_id,
            series_plan_id: compiled.series_plan_id,
            ordinal: compiled.ordinal,
            market_instance_id: compiled.market_instance_id,
            funding_quote_id: quote_id,
            recovery_state_id: recovery_admission.state_id,
            generation: recovery_admission.generation,
            work_principal_lamports: ledger.work_initial,
            rent_principal_lamports: ledger.rent_initial,
            admitted_reserve_balance: funding_observation.reserve_balance_after,
        };
        Ok((runtime, receipt))
    }

    /// Exact immutable binding.
    pub const fn binding(&self) -> FailurePolicyBindingV1 {
        self.binding
    }

    /// Typed immutable binding identity.
    pub const fn binding_id(&self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// The only implemented failure consequence.
    pub const fn consequence(&self) -> FailureConsequenceV1 {
        FailureConsequenceV1::EvidenceOnlyRecovery
    }

    /// Current funded recovery phase.
    pub const fn phase(&self) -> RecoveryPhase {
        self.recovery.phase()
    }

    /// Exact funded-recovery accounting snapshot.
    pub const fn ledger(&self) -> RecoveryLedger {
        self.recovery.ledger()
    }

    /// Immutable neutral sink selected by funded recovery admission.
    pub const fn recovery_neutral_sink(&self) -> RecoveryIdentity {
        self.recovery.neutral_sink()
    }

    /// Monotone funded-recovery transition nonce used for replay protection.
    pub const fn transition_nonce(&self) -> u64 {
        self.recovery.transition_nonce()
    }

    /// First recorded maturity trigger, if resolution did not precede it.
    pub const fn trigger(&self) -> Option<FailureTriggerV1> {
        self.trigger
    }

    /// Validate immutable binding, typed occurrence, generation, and phase join.
    pub fn check(&self) -> Result<()> {
        self.recovery.check()?;
        if self.binding_id.is_zero()
            || self.binding_id != self.binding.id()
            || self.binding.source_occurrence_receipt_id.is_zero()
            || self.binding.clock_policy_id.is_zero()
            || self.recovery.market_instance_v2_id() != Some(self.binding.market_instance_id)
            || self.recovery.recovery_policy_id() != self.binding.recovery_policy_id
            || self.recovery.series_funding_quote_id() != self.binding.funding_quote_id
            || self.recovery.state_id() != self.binding.recovery_state_id
            || self.recovery.generation() != self.binding.generation
            || self.primary_window.id()? != self.binding.primary_window_id
            || self.statistic_key_id != self.binding.statistic_key_id
            || self.summary_program_id != self.binding.summary_program_id
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

    /// Encode the complete canonical runtime state into adapter-owned account data.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.check()?;
        let mut writer = RuntimeWriter::new(output)?;
        writer.bytes(&FAILURE_RUNTIME_MAGIC)?;
        writer.u16(FAILURE_RUNTIME_SCHEMA)?;
        writer.reserved(6)?;
        writer.bytes(&self.binding.series_plan_id.bytes())?;
        writer.u32(self.binding.ordinal)?;
        writer.reserved(4)?;
        writer.bytes(&self.binding.market_instance_id.bytes())?;
        writer.bytes(&self.binding.product_template_id.bytes())?;
        writer.bytes(&self.binding.recovery_policy_id.bytes())?;
        writer.bytes(&self.binding.funding_quote_id.bytes())?;
        writer.bytes(&self.binding.funding_terms_id.bytes())?;
        writer.bytes(&self.binding.source_plane_program_id.bytes())?;
        writer.bytes(&self.binding.source_spec_id.bytes())?;
        writer.bytes(&self.binding.summary_program_id.bytes())?;
        writer.bytes(&self.binding.primary_window_id.bytes())?;
        writer.bytes(&self.binding.statistic_key_id.bytes())?;
        writer.bytes(&self.binding.source_occurrence_receipt_id.bytes())?;
        writer.bytes(&self.binding.clock_policy_id.bytes())?;
        writer.bytes(&self.binding.relation_policy_id)?;
        writer.bytes(&self.binding.recovery_state_id.bytes())?;
        writer.u64(self.binding.generation)?;
        writer.bytes(&self.binding_id.bytes())?;
        let mut window = [0; clutch_source_plane_v3::WINDOW_SPEC_BYTES];
        clutch_source_plane_v3::FixedCodec::encode_into(&self.primary_window, &mut window)?;
        writer.bytes(&window)?;
        writer.bytes(&self.statistic_key_id.bytes())?;
        writer.bytes(&self.summary_program_id.bytes())?;
        let mut recovery = [0; RECOVERY_STATE_V2_BYTES];
        self.recovery.encode_into(&mut recovery)?;
        writer.bytes(&recovery)?;
        match self.trigger {
            None => writer.reserved(96)?,
            Some(trigger) => {
                writer.u8(1)?;
                writer.u8(kind_code(trigger.kind))?;
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

    /// Decode and fully validate one complete canonical persisted runtime.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = RuntimeReader::new(input)?;
        if reader.bytes::<8>()? != FAILURE_RUNTIME_MAGIC {
            return Err(Error::BadMagic);
        }
        if reader.u16()? != FAILURE_RUNTIME_SCHEMA {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let series_plan_id = SeriesPlanV5Id::from_bytes(reader.bytes()?);
        let ordinal = reader.u32()?;
        reader.reserved(4)?;
        let binding = FailurePolicyBindingV1 {
            series_plan_id,
            ordinal,
            market_instance_id: MarketInstanceV2Id::from_bytes(reader.bytes()?),
            product_template_id: ProductTemplateId::from_bytes(reader.bytes()?),
            recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes(reader.bytes()?),
            funding_quote_id: SeriesFundingQuoteId::from_bytes(reader.bytes()?),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes(reader.bytes()?),
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
        };
        let binding_id = FailurePolicyBindingId::from_bytes(reader.bytes()?);
        let window = reader.bytes::<{ clutch_source_plane_v3::WINDOW_SPEC_BYTES }>()?;
        let primary_window = <WindowSpecV3 as clutch_source_plane_v3::FixedCodec>::decode(&window)?;
        let statistic_key_id = SourceContentId::from_bytes(reader.bytes()?);
        let summary_program_id = SourceContentId::from_bytes(reader.bytes()?);
        let recovery_bytes = reader.bytes::<RECOVERY_STATE_V2_BYTES>()?;
        let recovery = RecoveryState::decode(&recovery_bytes)?;
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
            primary_window,
            statistic_key_id,
            summary_program_id,
            recovery,
            trigger,
        };
        runtime.check()?;
        Ok(runtime)
    }

    /// Refuse new exposure through the funded recovery owner's crank-lag-safe gate.
    pub fn check_new_exposure(&self, clock: RecoveryClock) -> Result<()> {
        self.check()?;
        self.recovery.check_new_exposure(clock)?;
        Ok(())
    }

    /// Trigger from a complete SourcePlane runtime handoff proving either exact
    /// absence or one stable evaluator refusal at immutable primary maturity.
    pub fn plan_trigger_source_handoff(
        &self,
        actual_reserve_balance: u64,
        handoff: FailurePolicySourceHandoffV1,
        clock_policy: &ClockPolicyV1,
    ) -> Result<FailureTransitionPlanV1> {
        let clock = self.recovery_clock_for_source_handoff(handoff, clock_policy)?;
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
        let recovery = self
            .recovery
            .plan_enter_degraded(clock, actual_reserve_balance)?;
        self.wrap_plan(recovery, Some(trigger))
    }

    /// Derive and validate the exact recovery Clock committed by a SourcePlane
    /// runtime failure handoff.
    pub fn recovery_clock_for_source_handoff(
        &self,
        handoff: FailurePolicySourceHandoffV1,
        clock_policy: &ClockPolicyV1,
    ) -> Result<RecoveryClock> {
        self.check()?;
        clock_policy.validate()?;
        let occurrence = handoff.occurrence();
        if handoff.failure_policy_binding_id().bytes() != self.binding_id.bytes()
            || occurrence.id() != self.binding.source_occurrence_receipt_id
            || occurrence.series_plan_id().bytes() != self.binding.series_plan_id.bytes()
            || occurrence.ordinal() != self.binding.ordinal
            || occurrence.market_instance_id().bytes() != self.binding.market_instance_id.bytes()
            || occurrence.source_plane_contract_id() != self.binding.source_plane_program_id
            || occurrence.source_spec_id() != self.binding.source_spec_id
            || occurrence.window_id() != self.binding.primary_window_id
            || occurrence.statistic_key_id() != self.binding.statistic_key_id
            || occurrence.repair_generation() != self.primary_window.repair_generation
            || occurrence.clock_policy_id() != self.binding.clock_policy_id
            || clock_policy.id()? != self.binding.clock_policy_id
            || handoff.source_fact_receipt_id().is_zero()
        {
            return Err(Error::BindingMismatch);
        }
        match handoff.kind() {
            SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
                if !handoff.window_evidence_id().is_zero()
                    || !handoff.statistic_result_id().is_zero()
                    || handoff.refusal_code() != 0
                {
                    return Err(Error::BindingMismatch);
                }
            }
            SourceFailureKindV1::SourceEvaluationRefused => {
                if handoff.window_evidence_id().is_zero()
                    || handoff.statistic_result_id().is_zero()
                    || handoff.refusal_code() == 0
                {
                    return Err(Error::BindingMismatch);
                }
            }
        }
        let clock = recovery_clock_from_snapshot(clock_policy, handoff.clock())?;
        if clock.current_bucket < self.primary_window.maturity_bucket_exclusive {
            return Err(Error::TriggerBeforeMaturity);
        }
        Ok(clock)
    }

    /// Trigger at immutable maturity while recording the frozen relation's
    /// deterministic no-payout refusal over a successful SourcePlane result.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_trigger_relation_refusal(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        refusal: AdapterAuthenticatedRelationRefusalV1,
        result: &StatisticResultV3,
        key: &StatisticKeyV3,
        summary: &SummaryProgramV3,
        seal: &WindowSealV3,
        window: &WindowSpecV3,
    ) -> Result<FailureTransitionPlanV1> {
        self.validate_primary_result(result, key, summary, seal, window)?;
        if result.status() != StatisticResultStatusV3::Success {
            return Err(Error::SourceDidNotSucceed);
        }
        let result_id = result.id()?;
        if refusal.binding_id != self.binding_id
            || refusal.market_instance_id != self.binding.market_instance_id
            || refusal.generation != self.binding.generation
            || refusal.statistic_result_id != result_id
            || refusal.relation_policy_id != self.binding.relation_policy_id
        {
            return Err(Error::BindingMismatch);
        }
        let trigger = self.make_trigger(
            FailureTriggerKindV1::ResolutionRelationRefused,
            result_id.bytes(),
            refusal.refusal.code(),
            clock,
        )?;
        let recovery = self
            .recovery
            .plan_enter_degraded(clock, actual_reserve_balance)?;
        self.wrap_plan(recovery, Some(trigger))
    }

    /// Advance the finite immutable schedule; final close enters dormancy and
    /// applies the recovery owner's exact neutral residue disposition.
    pub fn plan_advance_schedule(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
    ) -> Result<FailureTransitionPlanV1> {
        self.check()?;
        let recovery = self
            .recovery
            .plan_advance_schedule(clock, actual_reserve_balance)?;
        self.wrap_plan(recovery, None)
    }

    /// Derive the exact SourcePlane Window required by the eligible repair attempt.
    pub fn recovery_work_join(&self, clock: RecoveryClock) -> Result<RecoveryWorkJoinV1> {
        self.check()?;
        let (attempt_index, attempt) = self.eligible_attempt(clock)?;
        let window = self.repair_window(attempt.repair_generation, attempt.closes_at_bucket)?;
        let funding = self.recovery.attempt_funding(attempt_index)?;
        let accepted = self.recovery.accepted_progress_units(attempt_index)?;
        let remaining_progress = funding
            .max_progress_units
            .checked_sub(accepted)
            .ok_or(Error::BindingMismatch)?;
        let maximum_remaining_lamports = remaining_progress
            .checked_mul(funding.lamports_per_progress_unit)
            .ok_or(Error::BindingMismatch)?;
        Ok(RecoveryWorkJoinV1 {
            attempt_index,
            repair_generation: attempt.repair_generation,
            window_id: window.id()?,
            funding_quote_id: self.binding.funding_quote_id,
            max_progress_units: funding.max_progress_units,
            lamports_per_progress_unit: funding.lamports_per_progress_unit,
            maximum_remaining_lamports,
        })
    }

    /// Bind an adapter-authenticated liveness work receipt to the exact current
    /// recovery occurrence, generation, attempt, Window, and progress cursor.
    #[allow(clippy::too_many_arguments)]
    pub fn join_liveness_work_receipt(
        &self,
        clock: RecoveryClock,
        work_receipt_id: [u8; 32],
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        attempt_index: u8,
        window_id: SourceContentId,
        accepted_progress_total: u64,
        quote_schedule_id: [u8; 32],
        scheduled_ceiling_lamports: u64,
    ) -> Result<LivenessWorkReceiptJoinV1> {
        let expected = self.recovery_work_join(clock)?;
        let prior = self
            .recovery
            .accepted_progress_units(expected.attempt_index)?;
        let progress_delta = accepted_progress_total
            .checked_sub(prior)
            .ok_or(Error::BindingMismatch)?;
        let exact_reward = progress_delta
            .checked_mul(expected.lamports_per_progress_unit)
            .ok_or(Error::BindingMismatch)?;
        if work_receipt_id.iter().all(|byte| *byte == 0)
            || market_instance_id != self.binding.market_instance_id
            || generation != self.binding.generation
            || attempt_index != expected.attempt_index
            || window_id != expected.window_id
            || accepted_progress_total > expected.max_progress_units
            || quote_schedule_id != expected.funding_quote_id.bytes()
            || scheduled_ceiling_lamports == 0
            || exact_reward == 0
            || exact_reward > scheduled_ceiling_lamports
            || scheduled_ceiling_lamports > expected.maximum_remaining_lamports
        {
            return Err(Error::BindingMismatch);
        }
        Ok(LivenessWorkReceiptJoinV1 {
            work_receipt_id,
            binding_id: self.binding_id,
            market_instance_id,
            generation,
            attempt_index,
            window_id,
            accepted_progress_total,
            quote_schedule_id,
            scheduled_ceiling_lamports,
        })
    }

    /// Pay strictly advancing accepted work only when its exact SourcePlane
    /// generation and deterministic repair Window match the compiled attempt.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_accept_work_progress(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        window: &WindowSpecV3,
        work_id: RecoveryIdentity,
        reward_recipient: RecoveryIdentity,
        accepted_progress_total: u64,
    ) -> Result<FailureTransitionPlanV1> {
        let expected = self.recovery_work_join(clock)?;
        if window.id()? != expected.window_id
            || window.repair_generation != expected.repair_generation
        {
            return Err(Error::WrongRecoveryWindow);
        }
        let recovery = self.recovery.plan_accept_work_progress(
            clock,
            actual_reserve_balance,
            work_id,
            reward_recipient,
            accepted_progress_total,
        )?;
        self.wrap_plan(recovery, None)
    }

    /// Pay an exact liveness-runtime receipt after rechecking its private-field
    /// failure-policy join. The receipt identity becomes the recovery Work ID.
    pub fn plan_accept_liveness_work_progress(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        window: &WindowSpecV3,
        reward_recipient: RecoveryIdentity,
        receipt: LivenessWorkReceiptJoinV1,
    ) -> Result<FailureTransitionPlanV1> {
        self.validate_liveness_receipt(clock, window, receipt)?;
        self.plan_accept_work_progress(
            clock,
            actual_reserve_balance,
            window,
            RecoveryIdentity::from_bytes(receipt.work_receipt_id),
            reward_recipient,
            receipt.accepted_progress_total,
        )
    }

    /// Mint a private-field accepted-resolution capability after exact
    /// SourcePlane validation and a frozen relation-record identity join.
    ///
    /// The adapter must authenticate `relation_record_id` as the output of the
    /// relation policy named by the immutable market Genesis. No caller or
    /// signer identity enters the capability.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_resolution_from_adapter(
        &self,
        result: &StatisticResultV3,
        key: &StatisticKeyV3,
        summary: &SummaryProgramV3,
        seal: &WindowSealV3,
        window: &WindowSpecV3,
        relation_policy_id: [u8; 32],
        relation_record_id: [u8; 32],
    ) -> Result<AcceptedResolutionV1> {
        self.check()?;
        result.validate_against(key, summary, seal, window)?;
        if result.status() != StatisticResultStatusV3::Success {
            return Err(Error::SourceDidNotSucceed);
        }
        self.validate_resolution_window(window)?;
        if summary.id()? != self.summary_program_id
            || key.summary_program_id != self.summary_program_id
            || key.window_id != window.id()?
            || relation_policy_id != self.binding.relation_policy_id
            || relation_record_id.iter().all(|byte| *byte == 0)
        {
            return Err(Error::BindingMismatch);
        }
        let statistic_result_id = result.id()?;
        let window_id = window.id()?;
        let mut hasher = Sha256::new();
        hasher.update(ACCEPTED_RESOLUTION_DOMAIN);
        hasher.update(self.binding_id.bytes());
        hasher.update(self.binding.market_instance_id.bytes());
        hasher.update(self.binding.generation.to_le_bytes());
        hasher.update(window_id.bytes());
        hasher.update(statistic_result_id.bytes());
        hasher.update(relation_policy_id);
        hasher.update(relation_record_id);
        let id = AcceptedResolutionId::from_bytes(hasher.finalize().into());
        Ok(AcceptedResolutionV1 {
            id,
            binding_id: self.binding_id,
            market_instance_id: self.binding.market_instance_id,
            generation: self.binding.generation,
            window_id,
            statistic_result_id,
            relation_record_id,
        })
    }

    /// Resolve from caller-funded accepted evidence, including after dormancy.
    pub fn plan_resolve_caller_funded(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        accepted: AcceptedResolutionV1,
    ) -> Result<FailureTransitionPlanV1> {
        self.validate_accepted_resolution(accepted)?;
        let evidence =
            EvidenceDecision::from_adapter(RecoveryIdentity::from_bytes(accepted.id.bytes()))?;
        let recovery =
            self.recovery
                .plan_resolve_caller_funded(clock, actual_reserve_balance, evidence)?;
        self.wrap_plan(recovery, None)
    }

    /// Resolve with one final exact paid progress advance.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_resolve_paid_progress(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        window: &WindowSpecV3,
        work_id: RecoveryIdentity,
        reward_recipient: RecoveryIdentity,
        accepted_progress_total: u64,
        accepted: AcceptedResolutionV1,
    ) -> Result<FailureTransitionPlanV1> {
        let expected = self.recovery_work_join(clock)?;
        if window.id()? != expected.window_id
            || window.repair_generation != expected.repair_generation
        {
            return Err(Error::WrongRecoveryWindow);
        }
        self.validate_accepted_resolution(accepted)?;
        if accepted.window_id != expected.window_id {
            return Err(Error::BindingMismatch);
        }
        let evidence =
            EvidenceDecision::from_adapter(RecoveryIdentity::from_bytes(accepted.id.bytes()))?;
        let recovery = self.recovery.plan_resolve_paid_progress(
            clock,
            actual_reserve_balance,
            work_id,
            reward_recipient,
            accepted_progress_total,
            evidence,
        )?;
        self.wrap_plan(recovery, None)
    }

    /// Resolve with one final adapter-authenticated liveness work receipt.
    ///
    /// The receipt is rechecked against the current progress cursor, FundingQuote
    /// schedule, exact call ceiling, attempt, generation, and repair Window. The
    /// accepted resolution must come from that same repair Window.
    pub fn plan_resolve_paid_liveness_progress(
        &self,
        clock: RecoveryClock,
        actual_reserve_balance: u64,
        window: &WindowSpecV3,
        reward_recipient: RecoveryIdentity,
        receipt: LivenessWorkReceiptJoinV1,
        accepted: AcceptedResolutionV1,
    ) -> Result<FailureTransitionPlanV1> {
        self.validate_liveness_receipt(clock, window, receipt)?;
        self.plan_resolve_paid_progress(
            clock,
            actual_reserve_balance,
            window,
            RecoveryIdentity::from_bytes(receipt.work_receipt_id),
            reward_recipient,
            receipt.accepted_progress_total,
            accepted,
        )
    }

    fn validate_liveness_receipt(
        &self,
        clock: RecoveryClock,
        window: &WindowSpecV3,
        receipt: LivenessWorkReceiptJoinV1,
    ) -> Result<()> {
        let expected = self.recovery_work_join(clock)?;
        let prior = self
            .recovery
            .accepted_progress_units(expected.attempt_index)?;
        let progress_delta = receipt
            .accepted_progress_total
            .checked_sub(prior)
            .ok_or(Error::BindingMismatch)?;
        let exact_reward = progress_delta
            .checked_mul(expected.lamports_per_progress_unit)
            .ok_or(Error::BindingMismatch)?;
        if receipt.binding_id != self.binding_id
            || receipt.market_instance_id != self.binding.market_instance_id
            || receipt.generation != self.binding.generation
            || receipt.attempt_index != expected.attempt_index
            || receipt.window_id != expected.window_id
            || receipt.accepted_progress_total > expected.max_progress_units
            || receipt.quote_schedule_id != expected.funding_quote_id.bytes()
            || receipt.scheduled_ceiling_lamports == 0
            || exact_reward == 0
            || exact_reward > receipt.scheduled_ceiling_lamports
            || receipt.scheduled_ceiling_lamports > expected.maximum_remaining_lamports
            || window.id()? != expected.window_id
            || window.repair_generation != expected.repair_generation
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn validate_primary_result(
        &self,
        result: &StatisticResultV3,
        key: &StatisticKeyV3,
        summary: &SummaryProgramV3,
        seal: &WindowSealV3,
        window: &WindowSpecV3,
    ) -> Result<()> {
        self.check()?;
        if *window != self.primary_window
            || window.id()? != self.binding.primary_window_id
            || key.id()? != self.statistic_key_id
            || summary.id()? != self.summary_program_id
        {
            return Err(Error::WrongRecoveryWindow);
        }
        result.validate_against(key, summary, seal, window)?;
        Ok(())
    }

    fn validate_resolution_window(&self, window: &WindowSpecV3) -> Result<()> {
        if *window == self.primary_window {
            return Ok(());
        }
        let schedule = self.recovery.schedule();
        let mut index = 0_usize;
        while index < usize::from(schedule.recovery_attempt_count) {
            let attempt = schedule.recovery_attempts[index];
            if *window == self.repair_window(attempt.repair_generation, attempt.closes_at_bucket)? {
                return Ok(());
            }
            index += 1;
        }
        Err(Error::WrongRecoveryWindow)
    }

    fn validate_accepted_resolution(&self, accepted: AcceptedResolutionV1) -> Result<()> {
        self.check()?;
        if accepted.id.is_zero()
            || accepted.binding_id != self.binding_id
            || accepted.market_instance_id != self.binding.market_instance_id
            || accepted.generation != self.binding.generation
            || accepted.window_id.is_zero()
            || accepted.statistic_result_id.is_zero()
            || accepted.relation_record_id.iter().all(|byte| *byte == 0)
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn eligible_attempt(
        &self,
        clock: RecoveryClock,
    ) -> Result<(u8, clutch_product_series::AbsoluteRecoveryAttemptV1)> {
        let schedule = self.recovery.schedule();
        let mut index = usize::from(self.recovery.next_attempt_index());
        while index < usize::from(schedule.recovery_attempt_count) {
            let attempt = schedule.recovery_attempts[index];
            if clock.current_bucket < attempt.closes_at_bucket {
                if clock.current_bucket < attempt.opens_at_bucket {
                    return Err(Error::WrongRecoveryWindow);
                }
                let attempt_index = u8::try_from(index).map_err(|_| Error::WrongRecoveryWindow)?;
                return Ok((attempt_index, attempt));
            }
            index += 1;
        }
        Err(Error::WrongRecoveryWindow)
    }

    fn repair_window(&self, repair_generation: u64, closes_at_bucket: u64) -> Result<WindowSpecV3> {
        let window = WindowSpecV3 {
            source_spec_id: self.binding.source_spec_id,
            source_plane_program_id: self.binding.source_plane_program_id,
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
        if clock.current_bucket < self.recovery.schedule().primary_maturity_bucket_exclusive {
            return Err(Error::TriggerBeforeMaturity);
        }
        if evidence_id.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroIdentity);
        }
        let id = trigger_id(self.binding_id, kind, evidence_id, refusal_code, clock);
        Ok(FailureTriggerV1 {
            id,
            kind,
            evidence_id,
            refusal_code,
            clock,
        })
    }

    fn validate_trigger(&self, trigger: FailureTriggerV1) -> Result<()> {
        if trigger.id.is_zero()
            || trigger.evidence_id.iter().all(|byte| *byte == 0)
            || trigger.clock.current_bucket
                < self.recovery.schedule().primary_maturity_bucket_exclusive
        {
            return Err(Error::BindingMismatch);
        }
        let expected = trigger_id(
            self.binding_id,
            trigger.kind,
            trigger.evidence_id,
            trigger.refusal_code,
            trigger.clock,
        );
        if trigger.id != expected {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }

    fn wrap_plan(
        &self,
        recovery_plan: RecoveryTransitionPlan,
        trigger: Option<FailureTriggerV1>,
    ) -> Result<FailureTransitionPlanV1> {
        self.check()?;
        let mut after = *self;
        after
            .recovery
            .commit_plan(recovery_plan, recovery_plan.expected_post_balance())?;
        if let Some(trigger) = trigger {
            if after.trigger.is_some() {
                return Err(Error::TriggerAlreadyRecorded);
            }
            after.trigger = Some(trigger);
        }
        after.check()?;
        Ok(FailureTransitionPlanV1 {
            before: *self,
            after,
            expected_pre_balance: recovery_plan.expected_pre_balance(),
            expected_post_balance: recovery_plan.expected_post_balance(),
            transfers: recovery_plan.transfers(),
        })
    }
}

/// Atomic failure-runtime and reserve-transfer plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureTransitionPlanV1 {
    before: FailureRuntimeV1,
    after: FailureRuntimeV1,
    expected_pre_balance: u64,
    expected_post_balance: u64,
    transfers: TransferPlan,
}

impl FailureTransitionPlanV1 {
    /// Exact reserve balance used to make this plan.
    pub const fn expected_pre_balance(&self) -> u64 {
        self.expected_pre_balance
    }

    /// Exact reserve balance required after all transfers.
    pub const fn expected_post_balance(&self) -> u64 {
        self.expected_post_balance
    }

    /// Exact work/rent/donation transfer compartments.
    pub const fn transfers(&self) -> TransferPlan {
        self.transfers
    }

    /// Resulting funded recovery phase.
    pub const fn resulting_phase(&self) -> RecoveryPhase {
        self.after.recovery.phase()
    }
}

impl FailureRuntimeV1 {
    /// Commit a current plan only after the adapter verifies exact reserve balance.
    pub fn commit_plan(
        &mut self,
        plan: FailureTransitionPlanV1,
        actual_post_balance: u64,
    ) -> Result<()> {
        self.check()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        if actual_post_balance != plan.expected_post_balance {
            return Err(Error::PostBalanceMismatch);
        }
        plan.after.check()?;
        *self = plan.after;
        Ok(())
    }
}

/// Exact disposition of the finite recovery-liveness funding compartment.
///
/// `Dormant` closes only the finite funded repair campaign. It does not settle,
/// retire, or make the unresolved market terminal; caller-funded evidence
/// recovery remains available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureRecoveryTerminalDispositionV1 {
    /// Accepted evidence resolved the market.
    Resolved = 1,
    /// The immutable finite funded schedule ended without accepted evidence.
    Dormant = 2,
}

impl FailureRecoveryTerminalDispositionV1 {
    /// Stable receipt-preimage and liveness-projection code.
    pub const fn code(self) -> u8 {
        match self {
            Self::Resolved => 1,
            Self::Dormant => 2,
        }
    }
}

/// Immutable receipt consumed by the separately owned liveness runtime to
/// close only its Recovery compartment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRecoveryTerminalReceiptV1 {
    id: FailureRecoveryTerminalReceiptId,
    binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    funding_quote_id: SeriesFundingQuoteId,
    recovery_state_id: RecoveryIdentity,
    generation: u64,
    transition_nonce: u64,
    disposition: FailureRecoveryTerminalDispositionV1,
}

impl FailureRecoveryTerminalReceiptV1 {
    /// Construct from one checked terminal phase of the finite recovery campaign.
    pub fn from_runtime(runtime: &FailureRuntimeV1) -> Result<Self> {
        runtime.check()?;
        let disposition = match runtime.phase() {
            RecoveryPhase::Resolved => FailureRecoveryTerminalDispositionV1::Resolved,
            RecoveryPhase::RecoveryDormant => FailureRecoveryTerminalDispositionV1::Dormant,
            RecoveryPhase::Active | RecoveryPhase::DegradedRecoverable => {
                return Err(Error::WrongPhase)
            }
        };
        let transition_nonce = runtime.transition_nonce();
        let mut hasher = Sha256::new();
        hasher.update(RECOVERY_TERMINAL_RECEIPT_DOMAIN);
        hasher.update(runtime.binding_id.bytes());
        hasher.update(runtime.binding.market_instance_id.bytes());
        hasher.update(runtime.binding.funding_quote_id.bytes());
        hasher.update(runtime.binding.recovery_state_id.bytes());
        hasher.update(runtime.binding.generation.to_le_bytes());
        hasher.update(transition_nonce.to_le_bytes());
        hasher.update([disposition.code()]);
        let id = FailureRecoveryTerminalReceiptId::from_bytes(hasher.finalize().into());
        Ok(Self {
            id,
            binding_id: runtime.binding_id,
            market_instance_id: runtime.binding.market_instance_id,
            funding_quote_id: runtime.binding.funding_quote_id,
            recovery_state_id: runtime.binding.recovery_state_id,
            generation: runtime.binding.generation,
            transition_nonce,
            disposition,
        })
    }

    /// Typed receipt identity projected to liveness `terminal_receipt_id`.
    pub const fn id(&self) -> FailureRecoveryTerminalReceiptId {
        self.id
    }

    /// Exact immutable failure-policy binding.
    pub const fn binding_id(&self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact full-width V2 market identity.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact immutable funding quote for the Recovery compartment.
    pub const fn funding_quote_id(&self) -> SeriesFundingQuoteId {
        self.funding_quote_id
    }

    /// Exact funded recovery state/reserve identity.
    pub const fn recovery_state_id(&self) -> RecoveryIdentity {
        self.recovery_state_id
    }

    /// Exact recovery generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Exact terminal recovery-state replay nonce.
    pub const fn transition_nonce(&self) -> u64 {
        self.transition_nonce
    }

    /// Closed disposition mapped to liveness success/failure.
    pub const fn disposition(&self) -> FailureRecoveryTerminalDispositionV1 {
        self.disposition
    }
}

/// Typed terminal boundary to separately authenticated lifecycle owners.
///
/// Construction does not infer zero liabilities. A live adapter must first
/// authenticate the retirement root, pre-funded predictable replay account,
/// and final SourcePlane release receipt under the same market generation.
/// The adapter seals the resulting join into that account atomically with
/// close. Dormancy is deliberately insufficient; accepted evidence must have
/// resolved the market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureTerminalJoinV1 {
    id: FailureTerminalJoinId,
    binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    retirement_root_id: [u8; 32],
    replay_tombstone_id: [u8; 32],
    source_release_receipt_id: [u8; 32],
}

impl FailureTerminalJoinV1 {
    /// Construct after the adapter authenticates every separately owned
    /// terminal fact and the pending immutable replay-account binding.
    pub fn from_adapter(
        runtime: &FailureRuntimeV1,
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
        let mut hasher = Sha256::new();
        hasher.update(TERMINAL_JOIN_DOMAIN);
        hasher.update(runtime.binding_id.bytes());
        hasher.update(runtime.binding.market_instance_id.bytes());
        hasher.update(generation.to_le_bytes());
        hasher.update(retirement_root_id);
        hasher.update(replay_tombstone_id);
        hasher.update(source_release_receipt_id);
        let id = FailureTerminalJoinId::from_bytes(hasher.finalize().into());
        Ok(Self {
            id,
            binding_id: runtime.binding_id,
            market_instance_id: runtime.binding.market_instance_id,
            generation,
            retirement_root_id,
            replay_tombstone_id,
            source_release_receipt_id,
        })
    }

    /// Typed exact terminal join identity.
    pub const fn id(&self) -> FailureTerminalJoinId {
        self.id
    }

    /// Exact immutable failure-policy binding.
    pub const fn binding_id(&self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact full-width V2 market identity.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact terminal generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Adapter-authenticated retirement-root identity.
    pub const fn retirement_root_id(&self) -> [u8; 32] {
        self.retirement_root_id
    }

    /// Adapter-authenticated permanent replay-tombstone identity.
    pub const fn replay_tombstone_id(&self) -> [u8; 32] {
        self.replay_tombstone_id
    }

    /// Adapter-authenticated final source-release receipt identity.
    pub const fn source_release_receipt_id(&self) -> [u8; 32] {
        self.source_release_receipt_id
    }
}

fn recovery_clock_from_snapshot(
    policy: &ClockPolicyV1,
    snapshot: ClockSnapshotV1,
) -> Result<RecoveryClock> {
    policy.validate()?;
    let elapsed = snapshot
        .unix_timestamp
        .checked_sub(policy.anchor_unix_timestamp)
        .ok_or(Error::BindingMismatch)?;
    let current_bucket = elapsed / u64::from(policy.bucket_seconds);
    let unix_timestamp =
        i64::try_from(snapshot.unix_timestamp).map_err(|_| Error::BindingMismatch)?;
    Ok(RecoveryClock {
        slot: snapshot.slot,
        unix_timestamp,
        current_bucket,
    })
}

const fn kind_code(kind: FailureTriggerKindV1) -> u8 {
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

fn trigger_id(
    binding_id: FailurePolicyBindingId,
    kind: FailureTriggerKindV1,
    evidence_id: [u8; 32],
    refusal_code: u32,
    clock: RecoveryClock,
) -> FailureTriggerId {
    let mut hasher = Sha256::new();
    hasher.update(TRIGGER_DOMAIN);
    hasher.update(binding_id.bytes());
    hasher.update([kind_code(kind)]);
    hasher.update(evidence_id);
    hasher.update(refusal_code.to_le_bytes());
    hasher.update(clock.slot.to_le_bytes());
    hasher.update(clock.unix_timestamp.to_le_bytes());
    hasher.update(clock.current_bucket.to_le_bytes());
    FailureTriggerId::from_bytes(hasher.finalize().into())
}

struct RuntimeWriter<'a> {
    output: &'a mut [u8],
    at: usize,
}

impl<'a> RuntimeWriter<'a> {
    fn new(output: &'a mut [u8]) -> Result<Self> {
        if output.len() != FAILURE_RUNTIME_V1_BYTES {
            return Err(Error::WrongLength);
        }
        output.fill(0);
        Ok(Self { output, at: 0 })
    }

    fn bytes(&mut self, value: &[u8]) -> Result<()> {
        let end = self.at.checked_add(value.len()).ok_or(Error::WrongLength)?;
        let target = self
            .output
            .get_mut(self.at..end)
            .ok_or(Error::WrongLength)?;
        target.copy_from_slice(value);
        self.at = end;
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
        let end = self.at.checked_add(count).ok_or(Error::WrongLength)?;
        if end > self.output.len() {
            return Err(Error::WrongLength);
        }
        self.at = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.at == FAILURE_RUNTIME_V1_BYTES {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}

struct RuntimeReader<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> RuntimeReader<'a> {
    fn new(input: &'a [u8]) -> Result<Self> {
        if input.len() != FAILURE_RUNTIME_V1_BYTES {
            return Err(Error::WrongLength);
        }
        Ok(Self { input, at: 0 })
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(Error::WrongLength)?;
        let source = self.input.get(self.at..end).ok_or(Error::WrongLength)?;
        let mut value = [0; N];
        value.copy_from_slice(source);
        self.at = end;
        Ok(value)
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
        let end = self.at.checked_add(count).ok_or(Error::WrongLength)?;
        let source = self.input.get(self.at..end).ok_or(Error::WrongLength)?;
        if source.iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        self.at = end;
        Ok(())
    }

    fn finish(self) -> Result<()> {
        if self.at == FAILURE_RUNTIME_V1_BYTES {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}
