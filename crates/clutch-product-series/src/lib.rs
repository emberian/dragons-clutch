#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Allocation-free recurring Product and Series identity and transition core.
//!
//! This crate freezes exact artifact bytes, typed SHA-256 identities, immutable
//! joins, recurrence arithmetic, authenticated-adapter seams, canonical
//! SourcePlane V3 occurrence identities, and segregated funding transitions.
//! It is deliberately below every account and SBF adapter: it allocates no
//! account tags or instruction intents and imports no Solana, token, oracle,
//! CPI, or account-memory type.
//!
//! The selected recovery semantics are evidence-only. No type in this crate
//! contains a data-failure payout index or vector. Legacy V3 numeric-fallback
//! artifacts are explicitly refused rather than relabeled.

mod artifacts;
mod codec;
mod compile;
mod compiler_output;
mod compiler_output_v2;
mod compiler_output_v3;
mod compiler_output_v4;
mod compiler_output_v5;
mod foundation_funding;
mod funding;
mod funding_state;
mod funding_state_v2;
mod failure_begin_schedule;
mod interval_consensus;
mod market_family_aggregator;
mod market_lifecycle;
mod market_replay;
mod product_registry;
mod registry;
mod source_series;
mod successor;

pub use artifacts::{
    EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV1, MarketInstancePreimageV1,
    NativeClaimBasisV1, ProductTemplateV4, RecoveryAttemptV1, SeriesAttachmentPlanV1,
    SeriesFundingTermsV1, SeriesPlanV4, BASIS_BYTES, EVIDENCE_ONLY_RECOVERY_POLICY_BYTES,
    MARKET_GENESIS_PROFILE_BYTES, MARKET_GENESIS_PROFILE_DOMAIN, MARKET_INSTANCE_DOMAIN,
    MARKET_INSTANCE_PREIMAGE_BYTES, MAX_BASIS_DEGREE, MAX_OUTCOMES, MAX_PAYOUTS,
    MAX_RECOVERY_ATTEMPTS, NATIVE_CLAIM_BASIS_DOMAIN, PAYOUT_MAP_UNUSED, PRODUCT_TEMPLATE_BYTES,
    PRODUCT_TEMPLATE_DOMAIN, RECOVERY_POLICY_DOMAIN, SERIES_ATTACHMENT_PLAN_BYTES,
    SERIES_ATTACHMENT_PLAN_DOMAIN, SERIES_FUNDING_TERMS_BYTES, SERIES_FUNDING_TERMS_DOMAIN,
    SERIES_PLAN_BYTES, SERIES_PLAN_DOMAIN, UNIFORM_SPACING_NONE,
};
pub use clutch_bspline::{BasisSpec as QuantizedBasisSpecV1, EdgePolicy as QuantizedEdgePolicyV1};
pub use compile::{
    compile_ordinal, AbsoluteRecoveryAttemptV1, CompiledOrdinalV1, CompiledScheduleV1,
};
pub use compiler_output::{
    assemble_compiled_product_series_bundle_v1, CompiledProductSeriesBundleV1,
    ProductSeriesBundleInputsV1, COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES,
    COMPILED_PRODUCT_SERIES_BUNDLE_V1_DOMAIN,
};
pub use compiler_output_v2::{
    assemble_compiled_product_series_bundle_v2, CompiledProductSeriesBundleV2,
    ProductSeriesBundleInputsV2, COMPILED_PRODUCT_SERIES_BUNDLE_V2_BYTES,
    COMPILED_PRODUCT_SERIES_BUNDLE_V2_DOMAIN,
};
pub use compiler_output_v3::{
    assemble_compiled_product_series_bundle_v3, CompiledProductSeriesBundleV3,
    ProductSeriesBundleInputsV3, COMPILED_PRODUCT_SERIES_BUNDLE_V3_BYTES,
    COMPILED_PRODUCT_SERIES_BUNDLE_V3_DOMAIN,
};
pub use compiler_output_v4::{
    assemble_compiled_product_series_bundle_v4, CompiledProductSeriesBundleV4,
    ProductSeriesBundleInputsV4, COMPILED_PRODUCT_SERIES_BUNDLE_V4_BYTES,
    COMPILED_PRODUCT_SERIES_BUNDLE_V4_DOMAIN,
};
pub use compiler_output_v5::{
    assemble_compiled_product_series_bundle_v5, CompiledProductSeriesBundleV5,
    ProductSeriesBundleInputsV5, COMPILED_PRODUCT_SERIES_BUNDLE_V5_BYTES,
    COMPILED_PRODUCT_SERIES_BUNDLE_V5_DOMAIN,
};
pub use foundation_funding::{
    MarketFoundationScheduleV1, MarketFoundationScheduleV2, SeriesAttachmentPlanV2,
    SeriesAttachmentPlanV3, SeriesAttachmentPlanV4, SeriesFundingComponentV2, SeriesFundingQuoteV2,
    SeriesFundingQuoteV3, SeriesFundingQuoteV4, SeriesMarketDispositionV1,
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V1, MARKET_FOUNDATION_CORE_SLOT_COUNT_V2,
    MARKET_FOUNDATION_MAX_OUTCOMES_V1, MARKET_FOUNDATION_MAX_OUTCOMES_V2,
    MARKET_FOUNDATION_SCHEDULE_V2_DOMAIN, MARKET_FOUNDATION_SLOT_COUNT_V1,
    MARKET_FOUNDATION_SLOT_COUNT_V2, SERIES_ATTACHMENT_PLAN_BYTES_V2,
    SERIES_ATTACHMENT_PLAN_BYTES_V3, SERIES_ATTACHMENT_PLAN_BYTES_V4,
    SERIES_ATTACHMENT_PLAN_V2_DOMAIN, SERIES_ATTACHMENT_PLAN_V3_DOMAIN,
    SERIES_ATTACHMENT_PLAN_V4_DOMAIN, SERIES_COLLATERAL_VAULT_COUNT_V2,
    SERIES_FUNDING_COMPONENT_COUNT_V2,
    SERIES_FUNDING_QUOTE_BYTES_V2, SERIES_FUNDING_QUOTE_BYTES_V3, SERIES_FUNDING_QUOTE_BYTES_V4,
    SERIES_FUNDING_QUOTE_V2_DOMAIN, SERIES_FUNDING_QUOTE_V3_DOMAIN, SERIES_FUNDING_QUOTE_V4_DOMAIN,
};
pub use funding::{
    project_component_debits, AdapterAuthenticatedComponentStatusV1,
    AdapterAuthenticatedFulfillmentStatusV1, ComponentDebitV1, DebitProjectionV1,
    FundingBalancesV1, RecoveryAttemptFundingV1, SeriesFundingQuoteV1, SERIES_FUNDING_QUOTE_BYTES,
    SERIES_FUNDING_QUOTE_DOMAIN,
};
pub use funding_state::{
    AuthenticatedSeriesFundingAuthorityV1, SeriesActivationContextV1, SeriesComponentCapitalV1,
    SeriesFundingComponentV1, SeriesFundingPhaseV1, SeriesFundingRequirementsV1,
    SeriesFundingStateV1, SeriesFundingTerminalProjectionV1, SERIES_FUNDING_COMPONENT_COUNT,
    SERIES_FUNDING_STATE_BYTES,
};
pub use funding_state_v2::{
    AuthenticatedSeriesFundingAuthorityV2, SeriesComponentCapitalV2, SeriesFundingPhaseV2,
    SeriesFundingStateV2, SeriesFundingTerminalProjectionV2, SERIES_COMPONENT_CAPITAL_BYTES_V2,
    SERIES_FUNDING_STATE_BYTES_V2, SERIES_FUNDING_STATE_V2_DOMAIN,
};
pub use failure_begin_schedule::{
    derive_product_failure_begin_schedule_projection_v1,
    ProductFailureBeginCompilerProvenanceV1,
    PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V1,
    PRODUCT_FAILURE_BEGIN_SCHEDULE_PROJECTION_DOMAIN_V1,
};
pub use interval_consensus::{
    advance_quantized_interval_consensus_work_v1, begin_quantized_interval_consensus_v1,
    quantized_interval_rounding_policy_id_v1,
    require_quantized_interval_consensus_runtime_capability_v1,
    restore_verified_quantized_interval_payout_v1,
    AuthenticatedQuantizedIntervalConsensusHistoryV1, QuantizedIntervalConsensusCertificateV1,
    QuantizedIntervalConsensusContextV1, QuantizedIntervalConsensusProfileV1,
    QuantizedIntervalConsensusProgressV1, QuantizedIntervalConsensusRestorationV1,
    QuantizedIntervalConsensusSessionV1, QuantizedIntervalConsensusWorkV1,
    VerifiedQuantizedIntervalPayoutV1, BASIS_EVALUATOR_VERSION_V1,
    QUANTIZED_INTERVAL_CONSENSUS_CERTIFICATE_BYTES_V1,
    QUANTIZED_INTERVAL_CONSENSUS_PROFILE_BYTES_V1,
    QUANTIZED_INTERVAL_CONSENSUS_RUNTIME_CAPABILITY_ENABLED_V1,
    QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1, QUANTIZED_INTERVAL_ROUNDING_POLICY_DOMAIN_V1,
};
pub use market_family_aggregator::{
    AuthenticatedMarketFamilyAuthorityV1, MarketFamilyAggregatorBindingV1,
    MarketFamilyAggregatorBindingV1Id, MarketFamilyAggregatorPhaseV1,
    MarketFamilyAggregatorTerminalProjectionV1, MarketFamilyAggregatorTerminalProjectionV1Id,
    MarketFamilyAggregatorV1, MarketFamilyAggregatorV1Id, MarketFamilyCountsV1,
    MarketFamilyExhaustiveSummaryV1, MarketFamilyExhaustiveSummaryV1Id, MarketFamilySlotV1,
    MarketFamilyStatusV1, MarketFamilyV1, NoMarketFamilyAuthorityV1, MARKET_FAMILIES_V1,
    MARKET_FAMILY_ADMISSION_DOMAIN_V1, MARKET_FAMILY_AGGREGATOR_BINDING_DOMAIN_V1,
    MARKET_FAMILY_AGGREGATOR_BYTES_V1, MARKET_FAMILY_AGGREGATOR_DOMAIN_V1, MARKET_FAMILY_COUNT_V1,
    MARKET_FAMILY_EXHAUSTIVE_SUMMARY_BYTES_V1, MARKET_FAMILY_EXHAUSTIVE_SUMMARY_DOMAIN_V1,
    MARKET_FAMILY_TERMINAL_DOMAIN_V1, MARKET_FAMILY_TERMINAL_PROJECTION_BYTES_V1,
    MARKET_FAMILY_TERMINAL_PROJECTION_DOMAIN_V1,
};
pub use market_lifecycle::{
    MarketFoundationAccountGraphV2, MarketFoundationCapitalV1, MarketFoundationProgressV1,
    MarketFoundationSlotV2, MarketFoundationStepProjectionV2, MarketFoundingAbortProjectionV1,
    MarketInstanceTerminalProjectionV1, MarketLifecycleBindingV1, MarketLifecyclePhaseV1,
    MarketLifecycleRootV1, MarketResolutionActivationV1, MarketSharedCoreTerminalProjectionV1,
    MarketSharedCoreV1, SeriesLinkObligationAdmissionProjectionV1,
    SeriesLinkObligationConfigurationV1, SeriesLinkObligationConfigurationV1Id,
    SeriesLinkObligationDispositionV1, SeriesLinkObligationStatusV1,
    SeriesLinkObligationTerminalProjectionV1, SeriesLinkObligationV1,
    SeriesMarketAdmissionProjectionV1, SeriesMarketLinkBindingV1, SeriesMarketLinkPhaseV1,
    SeriesMarketLinkRetirementProjectionV1, SeriesMarketLinkV1,
    MARKET_INSTANCE_TERMINAL_PROJECTION_DOMAIN_V1, MARKET_LIFECYCLE_BINDING_DOMAIN_V1,
    MARKET_LIFECYCLE_ROOT_BYTES_V1, MARKET_LIFECYCLE_ROOT_DOMAIN_V1,
    MARKET_RESOLUTION_ACTIVATION_DOMAIN_V1, MARKET_SHARED_CORE_COUNT_V1,
    SERIES_LINK_OBLIGATION_COUNT_V1, SERIES_MARKET_LINK_BYTES_V1, SERIES_MARKET_LINK_DOMAIN_V1,
};
pub use market_replay::{
    MarketLifecycleReplayReceiptV1, MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1,
    MARKET_LIFECYCLE_REPLAY_RECEIPT_DOMAIN_V1,
};
pub use product_registry::{
    RegistryCapabilityProfileV2, RegistryCapabilityProfileV3, RegistryCapabilityProfileV4,
    RegistryProgramReleaseV1, RegistryProgramReleaseV2, RegistryReleaseLocusV2,
    REGISTRY_CAPABILITY_PROFILE_V2_BYTES, REGISTRY_CAPABILITY_PROFILE_V2_DOMAIN,
    REGISTRY_CAPABILITY_PROFILE_V3_BYTES, REGISTRY_CAPABILITY_PROFILE_V3_DOMAIN,
    REGISTRY_CAPABILITY_PROFILE_V4_BYTES, REGISTRY_CAPABILITY_PROFILE_V4_DOMAIN,
    REGISTRY_PROGRAM_RELEASE_V1_BYTES, REGISTRY_PROGRAM_RELEASE_V1_DOMAIN,
    REGISTRY_PROGRAM_RELEASE_V2_BYTES, REGISTRY_PROGRAM_RELEASE_V2_DOMAIN,
};
pub use registry::{
    CapabilitySemanticOwnersV1, RealmCollateralProjectionV1, RegistryCapabilityProjectionV1,
};
pub use source_series::{
    compile_source_occurrence_v3, compile_source_occurrence_v4,
    compile_source_semantic_inputs_v1, AuthenticatedSourceSeriesAuthorityV3,
    CompiledSourceOccurrenceV3, CompiledSourceSemanticInputsV1, SOURCE_OCCURRENCE_RECORD_BYTES,
    SOURCE_OCCURRENCE_RECORD_DOMAIN,
};
pub use successor::{
    compile_ordinal_v2, compile_ordinal_v3, compile_ordinal_v4, compile_ordinal_v5,
    project_component_debits_v2, AdapterFulfillmentProjectionV2, CapabilitySemanticOwnersV2,
    CompiledOrdinalV2, MarketGenesisProfileV2, MarketInstancePreimageV2, PriceMeasurePolicyV1,
    ProjectedComponentPresenceV2, RegistryCapabilityProjectionV2, SeriesFundingTermsV2,
    SeriesPlanV5, MARKET_GENESIS_PROFILE_V2_BYTES, MARKET_GENESIS_PROFILE_V2_DOMAIN,
    MARKET_INSTANCE_PREIMAGE_V2_BYTES, MARKET_INSTANCE_V2_DOMAIN, PRICE_MEASURE_POLICY_BYTES,
    PRICE_MEASURE_POLICY_DOMAIN, SERIES_FUNDING_TERMS_V2_BYTES, SERIES_FUNDING_TERMS_V2_DOMAIN,
    SERIES_PLAN_V5_BYTES, SERIES_PLAN_V5_DOMAIN,
};

use sha2::{Digest, Sha256};

/// A canonical 32-byte external artifact or release identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentId([u8; 32]);

impl ContentId {
    /// Reserved all-zero padding identity. It is not a valid live reference.
    pub const ZERO: Self = Self([0; 32]);

    /// Construct an identity from exact bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }

    /// Whether this is the reserved all-zero identity.
    pub fn is_zero(self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }

    pub(crate) fn validate(self) -> Result<()> {
        if self.is_zero() {
            Err(Error::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

macro_rules! typed_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(ContentId);

        impl $name {
            /// Construct a typed identity from exact digest bytes.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(ContentId::from_bytes(bytes))
            }

            /// Return the exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0.bytes()
            }

            /// Return this identity through the generic content-ID boundary.
            pub const fn content_id(self) -> ContentId {
                self.0
            }

            /// Refuse the all-zero identity reserved for inactive padding.
            pub fn validate(self) -> Result<()> {
                self.0.validate()
            }
        }
    };
}

typed_id!(
    NativeClaimBasisId,
    "Typed identity of one `NativeClaimBasisV1`."
);
typed_id!(
    EvidenceOnlyRecoveryPolicyId,
    "Typed identity of one `EvidenceOnlyRecoveryPolicyV1`."
);
typed_id!(
    ProductTemplateId,
    "Typed identity of one `ProductTemplateV4`."
);
typed_id!(
    MarketGenesisProfileId,
    "Typed identity of one `MarketGenesisProfileV1`."
);
typed_id!(
    MarketInstanceId,
    "Typed identity of one economic `MarketInstancePreimageV1`."
);
typed_id!(
    SeriesAttachmentPlanId,
    "Typed identity of one versioned Series attachment plan."
);
typed_id!(SeriesPlanId, "Typed identity of one `SeriesPlanV4`.");
typed_id!(
    CompiledProductSeriesBundleV1Id,
    "Typed identity of one `CompiledProductSeriesBundleV1` compiler output."
);
typed_id!(
    CompiledProductSeriesBundleV2Id,
    "Typed identity of one `CompiledProductSeriesBundleV2` compiler output."
);
typed_id!(
    CompiledProductSeriesBundleV3Id,
    "Typed identity of the withdrawn provisional `CompiledProductSeriesBundleV3`."
);
typed_id!(
    CompiledProductSeriesBundleV4Id,
    "Typed identity of one historical `CompiledProductSeriesBundleV4` compiler output."
);
typed_id!(
    CompiledProductSeriesBundleV5Id,
    "Typed identity of one current `CompiledProductSeriesBundleV5` compiler output."
);
typed_id!(
    SeriesFundingQuoteId,
    "Typed identity of one `SeriesFundingQuoteV1`."
);
typed_id!(
    SeriesFundingQuoteV2Id,
    "Typed identity of the withdrawn six-compartment `SeriesFundingQuoteV2`."
);
typed_id!(
    SeriesFundingQuoteV3Id,
    "Typed identity of the withdrawn provisional `SeriesFundingQuoteV3`."
);
typed_id!(
    SeriesAttachmentPlanV3Id,
    "Typed identity of the withdrawn provisional `SeriesAttachmentPlanV3`."
);
typed_id!(
    SeriesFundingQuoteV4Id,
    "Typed identity of one current 46-slot `SeriesFundingQuoteV4`."
);
typed_id!(
    SeriesFundingStateV2Id,
    "Typed semantic identity of one current `SeriesFundingStateV2`."
);
typed_id!(
    SeriesAttachmentPlanV4Id,
    "Typed identity of one current `SeriesAttachmentPlanV4`."
);
typed_id!(
    MarketFoundationScheduleV1Id,
    "Typed identity of one itemized shared-Market foundation schedule."
);
typed_id!(
    MarketFoundationScheduleV2Id,
    "Typed identity of one exhaustive 46-slot shared-Market foundation schedule."
);
typed_id!(
    MarketFoundationAccountGraphV1Id,
    "Typed identity of one canonical shared-Market foundation account graph."
);
typed_id!(
    MarketFoundationAccountGraphV2Id,
    "Typed identity of one canonical 46-slot shared-Market foundation account graph."
);
typed_id!(
    MarketLifecycleRootV1Id,
    "Typed semantic-state identity of one shared Market lifecycle root."
);
typed_id!(
    MarketLifecycleReplayReceiptV1Id,
    "Typed identity of one permanent `MarketLifecycleReplayReceiptV1`."
);
typed_id!(
    SeriesMarketLinkV1Id,
    "Typed semantic-state identity of one Series ordinal's Market admission link."
);
typed_id!(
    SeriesFundingTermsId,
    "Typed identity of one `SeriesFundingTermsV1`."
);
typed_id!(
    PriceMeasurePolicyV1Id,
    "Typed identity of one quantized `PriceMeasurePolicyV1`."
);
typed_id!(
    MarketGenesisProfileV2Id,
    "Typed identity of one `MarketGenesisProfileV2`."
);
typed_id!(
    MarketInstanceV2Id,
    "Typed identity of one economic `MarketInstancePreimageV2`."
);
typed_id!(SeriesPlanV5Id, "Typed identity of one `SeriesPlanV5`.");
typed_id!(
    RegistryProgramReleaseV1Id,
    "Typed identity of one historical immutable `RegistryProgramReleaseV1`."
);
typed_id!(
    RegistryProgramReleaseV2Id,
    "Typed identity of one locus-explicit immutable `RegistryProgramReleaseV2`."
);
typed_id!(
    RegistryCapabilityProfileV2Id,
    "Typed identity of one withdrawn immutable `RegistryCapabilityProfileV2`."
);
typed_id!(
    RegistryCapabilityProfileV3Id,
    "Typed identity of one historical immutable `RegistryCapabilityProfileV3`."
);
typed_id!(
    RegistryCapabilityProfileV4Id,
    "Typed identity of one ReleaseV2-bound immutable `RegistryCapabilityProfileV4`."
);
typed_id!(
    SeriesFundingTermsV2Id,
    "Typed identity of one `SeriesFundingTermsV2`."
);
typed_id!(
    SourceOccurrenceV1Id,
    "Typed provenance identity of one compiled SourcePlane V3 occurrence record."
);
typed_id!(
    QuantizedIntervalConsensusProfileV1Id,
    "Typed identity of one bounded quantized interval-consensus work profile."
);
typed_id!(
    QuantizedIntervalConsensusCertificateV1Id,
    "Typed identity of one exhaustive quantized interval-consensus certificate."
);
typed_id!(
    QuantizedIntervalConsensusWorkV1Id,
    "Typed identity of one complete structural interval-consensus work preimage."
);
typed_id!(
    ProductFailureBeginScheduleProjectionV1Id,
    "Typed identity of one exact current Product-compiled Failure Begin schedule and provenance."
);

/// A deterministic refusal from a fixed codec or pure projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// Input or output was shorter than the exact named layout.
    Truncated,
    /// Input or output was longer than the exact named layout.
    TrailingBytes,
    /// A fixed artifact discriminator did not match.
    BadMagic,
    /// A schema was not the one exact version implemented here.
    BadVersion,
    /// Reserved bytes were not all zero.
    NonCanonicalReserved,
    /// A required identity was all zero.
    ZeroIdentity,
    /// A count, scalar, enum, or amount was outside its admitted domain.
    InvalidParameter,
    /// An inactive fixed-width entry was not canonical padding.
    NonCanonicalPadding,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// A referenced artifact did not match its recomputed typed identity.
    MismatchedArtifact,
    /// A finite recovery schedule was unordered, overlapping, or empty.
    InvalidSchedule,
    /// A current numeric-fallback Product/Payout V3 body was presented.
    LegacyNumericFallback,
    /// The requested Series ordinal is outside the immutable finite schedule.
    WrongOrdinal,
    /// An exact-existing versus absent component projection was inconsistent.
    InvalidComponentStatus,
    /// Available segregated funding could not cover the projected components.
    InsufficientPrepayment,
    /// The selected capability profile does not admit the requested semantics.
    UnsupportedCapability,
    /// No adapter-authenticated source/registry authority was supplied.
    UnauthenticatedAuthority,
    /// The mutable Series has no ordinal remaining to create or lapse.
    SeriesNotActive,
    /// The authenticated Clock lies outside this ordinal's required interval.
    OutsideCreationWindow,
    /// A terminal projection was requested before every ordinal advanced.
    SeriesNotClosed,
    /// A closed source interval exceeded the profile-selected exhaustive width.
    IntervalTooWide,
    /// A work request was zero or exceeded the profile-selected chunk bound.
    WorkLimitExceeded,
    /// Exhaustive evaluation found two distinct exact quantized payout vectors.
    IntervalPayoutDisagreement,
    /// A verified payout was requested before every coordinate was evaluated.
    WorkIncomplete,
    /// A completed work record was asked to advance again.
    WorkAlreadyComplete,
    /// Persisted work fields did not match their canonical bindings or cursor state.
    WorkStateMismatch,
    /// The pure contract exists, but no live runtime capability is activated.
    RuntimeCapabilityDisabled,
}

/// Result alias for this allocation-free core.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact fixed-layout encoding and hostile decoding contract.
pub trait FixedCodec: Sized {
    /// Exact canonical body length; shorter and longer inputs both refuse.
    const ENCODED_LEN: usize;

    /// Validate and encode into an exact-length caller-owned buffer.
    fn encode_into(&self, output: &mut [u8]) -> Result<()>;

    /// Decode one exact-length canonical value and validate all padding.
    fn decode(input: &[u8]) -> Result<Self>;
}

pub(crate) fn content_id(domain: &[u8], body: &[u8]) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(body);
    ContentId::from_bytes(hasher.finalize().into())
}
