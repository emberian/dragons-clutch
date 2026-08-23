//! Canonical hostile-byte decoding and fork-aware untrusted account indexing.
//!
//! The index retains exact bytes and provenance. Its projections help an
//! operator choose work, but never replace onchain account authentication.

use crate::rpc_index::{
    CanonicalFamily, IndexedProgramRelease, ObservedRpcAccount, ObservedRpcAccountRemoval,
    ObservedSlot, ObservedSlotUpdate, ObservedSlotUpdateKind, RpcAccountRemovalKind, RpcCommitment,
    RpcIndexPlan,
};
use crate::workflow_graph::{WorkflowLane, WorkflowPosition};
use clutch_collateral_adapter_v2::{
    ClaimLedgerV3, HoardV2, ResolutionV5, CLAIM_LEDGER_V3_BYTES, CLAIM_LEDGER_V3_TAG,
    CLAIM_LEDGER_V3_VERSION, HOARD_V2_BYTES, HOARD_V2_TAG, HOARD_V2_VERSION, RESOLUTION_V5_BYTES,
    RESOLUTION_V5_TAG, RESOLUTION_V5_VERSION,
};
use clutch_dealer_runtime_contract::{
    CoveredDealerSelectionV1, DealerActionReceiptV1, DealerClaimWorkV1, DealerEpochBindingV2,
    DealerExitTicketV1, DealerFacilityReplayV1, DealerFundedDependenciesV2, DealerLeaseV2,
    DealerLivenessScheduleV1, DealerPolicyV1, DealerRootTombstoneV2, DealerStateV2,
    DealerTerminalAllocationV1, FixedCodec as DealerFixedCodec, LpPageV2, SettlementPotV2,
    DEALER_FACILITY_REPLAY_BYTES_V1,
};
use clutch_failure_policy_runtime::market_policy_v1::FailureMarketAdmissionStateV1;
use clutch_fractional_redemption_runtime::{
    FractionalCreditTombstoneV2, FractionalCreditV2, FractionalLedgerV1, FractionalPolicyV2,
    FRACTIONAL_CREDIT_ACCOUNT_BYTES, FRACTIONAL_CREDIT_ACCOUNT_TAG,
    FRACTIONAL_CREDIT_ACCOUNT_VERSION, FRACTIONAL_CREDIT_TOMBSTONE_BYTES,
    FRACTIONAL_CREDIT_TOMBSTONE_TAG, FRACTIONAL_CREDIT_TOMBSTONE_VERSION,
    FRACTIONAL_LEDGER_ACCOUNT_BYTES, FRACTIONAL_LEDGER_ACCOUNT_TAG,
    FRACTIONAL_LEDGER_ACCOUNT_VERSION, FRACTIONAL_POLICY_ACCOUNT_BYTES,
    FRACTIONAL_POLICY_ACCOUNT_TAG, FRACTIONAL_POLICY_ACCOUNT_VERSION,
};
use clutch_general_v2_contract::{
    complete_candidate_feed_v2, AdmissionNodeV4AccountV1,
    CandidateWindowV5AccountV1, ClearWorkV3AccountV1, EconomicDomainV2AccountV1,
    EpochBudgetV2AccountV1, GeneralEpochV6AccountV1, MarketBindingV2, MarketRuntimeV3AccountV1,
    OwnerFeeFinalizationV4AccountV1, OwnerSettlementV5AccountV1, PayerAllocationV2AccountV1,
    RecipientAllocationV2AccountV1, SettlementCashPotV1AccountV1,
    SettlementRootV1AccountV1, ADMISSION_NODE_ACCOUNT_TAG, ADMISSION_NODE_ACCOUNT_VERSION_V2,
    CANDIDATE_FEED_ACCOUNT_TAG, CANDIDATE_FEED_ACCOUNT_VERSION, CANDIDATE_FEED_STAGE_ACCOUNT_TAG,
    CANDIDATE_FEED_STAGE_ACCOUNT_VERSION, CLEAR_WORK_ACCOUNT_TAG, CLEAR_WORK_ACCOUNT_VERSION_V3,
    ECONOMIC_DOMAIN_ACCOUNT_TAG, ECONOMIC_DOMAIN_ACCOUNT_VERSION, EPOCH_BUDGET_ACCOUNT_TAG,
    EPOCH_BUDGET_ACCOUNT_VERSION, FINAL_POT_ACCOUNT_BYTES, FINAL_POT_ACCOUNT_TAG,
    FINAL_POT_ACCOUNT_VERSION, GENERAL_EPOCH_ACCOUNT_TAG, GENERAL_EPOCH_ACCOUNT_VERSION,
    MARKET_BINDING_ACCOUNT_TAG, MARKET_BINDING_ACCOUNT_VERSION_V2, MARKET_RUNTIME_ACCOUNT_TAG,
    MARKET_RUNTIME_ACCOUNT_VERSION, OWNER_FEE_CARRY_ACCOUNT_BYTES_V3, OWNER_FEE_CARRY_ACCOUNT_TAG,
    OWNER_FEE_CARRY_ACCOUNT_VERSION_V3, OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4,
    OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4, OWNER_SETTLEMENT_ACCOUNT_TAG,
    OWNER_SETTLEMENT_ACCOUNT_VERSION_V5, PAYER_ALLOCATION_ACCOUNT_BYTES_V2,
    PAYER_ALLOCATION_ACCOUNT_TAG, PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
    RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V2, RECIPIENT_ALLOCATION_ACCOUNT_TAG,
    RECIPIENT_ALLOCATION_ACCOUNT_VERSION_V2, SELECTED_FEE_RECORD_ACCOUNT_BYTES,
    SELECTED_FEE_RECORD_ACCOUNT_TAG, SELECTED_FEE_RECORD_ACCOUNT_VERSION,
    SETTLEMENT_CASH_POT_ACCOUNT_TAG, SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
    SETTLEMENT_ROOT_ACCOUNT_TAG, SETTLEMENT_ROOT_ACCOUNT_VERSION, TREASURY_LEDGER_ACCOUNT_BYTES,
    TREASURY_LEDGER_ACCOUNT_TAG, TREASURY_LEDGER_ACCOUNT_VERSION, WINDOW_ACCOUNT_TAG,
    WINDOW_ACCOUNT_VERSION_V2,
};
use clutch_liveness::{
    RuntimeCompartmentPhaseV1, RuntimeCompartmentV1, RuntimeLivenessPolicyV1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_ACCOUNT_MAGIC_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1, RUNTIME_LIVENESS_POLICY_MAGIC_V1,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV5, EvidenceOnlyRecoveryPolicyV1,
    FixedCodec as ProductFixedCodec, MarketGenesisProfileV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, SeriesFundingTermsV2, SeriesPlanV5,
    SeriesFundingPhaseV2,
};
use clutch_retirement::{
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, ReplayV3Envelope,
    ReplayV3HashBackend, ReplayV3Lifecycle, POSITION_ACCOUNT_TAG, POSITION_ACCOUNT_VERSION_V3,
    PURPOSE_REPLAY_ACCOUNT_TAG, PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
};
use clutch_solana_layout::artifact::{
    decode_stage, validate_artifact, ArtifactBinding, ArtifactKind, ArtifactRegistrationStatus,
    ARTIFACT_STAGE_PDA_PREFIX_V1, ARTIFACT_STAGE_TAG, ARTIFACT_STAGE_VERSION,
    PRODUCT_ARTIFACT_PDA_PREFIX_V1,
};
use clutch_solana_layout::failure_interval_consensus::{
    FailureIntervalConsensusPhaseV1, FailureIntervalConsensusReplayAccountV1,
    FailureIntervalConsensusWorkAccountV1,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FailureMarketRootAccountV2, FailureReplayTombstoneV1,
    FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1, FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1, FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2,
    FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1, FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2,
};
use clutch_solana_layout::order_page_v5::OrderPageAccountV5;
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesFundingAccountV2, SeriesMarketLinkAccountV1,
    SeriesRegistryAccountV2, SERIES_FUNDING_PDA_PREFIX_V1, SERIES_REGISTRY_PDA_PREFIX_V1,
};
use clutch_solana_layout::registry;
use clutch_solana_layout::reservation_v9::ReservationAccountV9;
use clutch_solana_layout::settlement_receipt_v5::SettlementReceiptAccountV5;
use clutch_solana_layout::Hash32;
use clutch_source_plane_v3::{
    OpenRawPageV3, RawPageV3, SourceHeadV3, StatisticResultV3, WindowSealV3, WindowWorkV3,
    MAX_RAW_PAGE_RECORDS,
};
use clutch_source_plane_v3_runtime::{
    decode_runtime_account, ReopenLineageV1, RuntimeKey, SourceReleaseManifestV2,
    SourceWorkReceiptAccountV1, OPEN_RAW_PAGE_ACCOUNT_TAG, RAW_PAGE_ACCOUNT_TAG,
    REOPEN_LINEAGE_ACCOUNT_TAG, REOPEN_LINEAGE_ACCOUNT_VERSION, SOURCE_HEAD_ACCOUNT_TAG,
    SOURCE_RELEASE_ACCOUNT_TAG, SOURCE_RELEASE_ACCOUNT_VERSION, SOURCE_WORK_RECEIPT_ACCOUNT_BYTES,
    SOURCE_WORK_RECEIPT_ACCOUNT_TAG, SOURCE_WORK_RECEIPT_ACCOUNT_VERSION,
    STATISTIC_RESULT_ACCOUNT_TAG, WINDOW_SEAL_ACCOUNT_TAG, WINDOW_WORK_ACCOUNT_TAG,
};
use clutch_structured_claim_runtime_contract::{
    DescriptorStateV1, StructuredClaimDescriptorV1, DESCRIPTOR_ACCOUNT_BYTES,
    DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use std::collections::{BTreeMap, BTreeSet};

pub type Result<T> = core::result::Result<T, AccountIndexError>;

/// Sole decoder contract admitted by live chain serving. Historical Source V1/V2
/// and withdrawn account versions are deliberately outside this set.
pub const CANONICAL_ACCOUNT_DECODER_SET: &str =
    "dragons-clutch/canonical-account-decoders/v4-product-v5-general-successor-current";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountIndexError {
    UnknownRelease,
    WrongCluster,
    WrongOwner,
    ExecutableDataAccount,
    UnknownCodec,
    AmbiguousCodec,
    CanonicalDecodeRefused,
    InvalidFork,
    UnknownFork,
    AmbiguousFork,
    RootRegression,
    StaleObservation,
    CapacityExceeded,
}

impl core::fmt::Display for AccountIndexError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownRelease => "account observation names an unknown release",
            Self::WrongCluster => "account observation names another cluster",
            Self::WrongOwner => "account owner differs from its explicit release",
            Self::ExecutableDataAccount => "executable account cannot enter the data index",
            Self::UnknownCodec => "no canonical account owner recognized the bytes",
            Self::AmbiguousCodec => "multiple canonical owners recognized the same bytes",
            Self::CanonicalDecodeRefused => "canonical account decoder refused the bytes",
            Self::InvalidFork => "slot observation has invalid fork geometry",
            Self::UnknownFork => "account observation cannot be joined to an observed fork",
            Self::AmbiguousFork => "slot maps to more than one observed fork",
            Self::RootRegression => "finalized root would regress",
            Self::StaleObservation => "account observation regresses its receive ordering",
            Self::CapacityExceeded => "account index exceeds its explicit capacity",
        })
    }
}

impl std::error::Error for AccountIndexError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalAccountKind {
    CollateralHoardV2,
    CollateralClaimLedgerV3,
    CollateralResolutionV5,
    FractionalPolicyV2,
    FractionalLedgerV1,
    FractionalCreditV2,
    FractionalCreditTombstoneV2,
    GeneralMarketRuntime,
    GeneralEpoch,
    GeneralEconomicDomain,
    GeneralMarketBinding,
    GeneralOrderPage,
    GeneralReservation,
    GeneralCandidateWindow,
    GeneralAdmissionNode,
    GeneralCandidateFeedStage,
    GeneralCandidateFeed,
    GeneralClearWork,
    GeneralEpochBudget,
    GeneralOwnerSettlement,
    GeneralSettlementReceipt,
    GeneralSettlementRoot,
    GeneralSettlementCashPot,
    GeneralFinalPot,
    ProductMarketLifecycleRootV1,
    ProductSeriesMarketLinkV1,
    SeriesRegistryV2,
    SeriesFundingV2,
    ArtifactUploadStage,
    ArtifactRegistryProgramReleaseV2,
    ArtifactRegistryCapabilityProfileV4,
    ArtifactSourceReleaseManifestV2,
    ArtifactNativeClaimBasisV1,
    ArtifactEvidenceOnlyRecoveryPolicyV1,
    ArtifactProductTemplateV4,
    ArtifactPriceMeasurePolicyV1,
    ArtifactMarketGenesisProfileV2,
    ArtifactSeriesFundingQuoteV4,
    ArtifactSeriesAttachmentPlanV4,
    ArtifactSeriesPlanV5,
    ArtifactSeriesFundingTermsV2,
    ArtifactCompiledProductSeriesBundleV5,
    SourceRelease,
    SourceHead,
    SourceOpenRawPage,
    SourceRawPage,
    SourceWindowWork,
    SourceWindowSeal,
    SourceStatisticResult,
    SourceLineage,
    SourceWorkReceipt,
    FeeSelectedRecord,
    FeeOwnerCarryV3,
    FeeOwnerFinalizationV4,
    FeePayerAllocationV2,
    FeeRecipientAllocationV2,
    FeeTreasuryLedger,
    LivenessPolicy,
    LivenessCompartment,
    PositionV3,
    ReplayV3,
    StructuredClaimDescriptor,
    DealerPolicy,
    DealerLivenessSchedule,
    DealerStateV2,
    DealerFundedDependenciesV2,
    DealerLpPageV2,
    DealerLeaseV2,
    DealerSettlementPotV2,
    DealerEpochBindingV2,
    DealerTerminalAllocation,
    DealerClaimWork,
    DealerRootTombstoneV2,
    DealerExitTicket,
    DealerActionReceipt,
    DealerCoveredSelection,
    DealerReplay,
    FailureExternalRoot,
    FailureMarketRootV2,
    FailureLivenessPolicy,
    FailureRecoveryCompartment,
    FailureReplayTombstone,
    FailureIntervalConsensusWork,
    FailureIntervalConsensusReplay,
}

impl CanonicalAccountKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CollateralHoardV2 => "collateral-hoard-v2",
            Self::CollateralClaimLedgerV3 => "collateral-claim-ledger-v3",
            Self::CollateralResolutionV5 => "collateral-resolution-v5",
            Self::FractionalPolicyV2 => "fractional-policy-v2",
            Self::FractionalLedgerV1 => "fractional-ledger-v1",
            Self::FractionalCreditV2 => "fractional-credit-v2",
            Self::FractionalCreditTombstoneV2 => "fractional-credit-tombstone-v2",
            Self::GeneralMarketRuntime => "general-market-runtime",
            Self::GeneralEpoch => "general-epoch",
            Self::GeneralEconomicDomain => "general-economic-domain",
            Self::GeneralMarketBinding => "general-market-binding",
            Self::GeneralOrderPage => "general-order-page-v5",
            Self::GeneralReservation => "general-reservation-v9",
            Self::GeneralCandidateWindow => "general-candidate-window",
            Self::GeneralAdmissionNode => "general-admission-node",
            Self::GeneralCandidateFeedStage => "general-candidate-feed-stage",
            Self::GeneralCandidateFeed => "general-candidate-feed",
            Self::GeneralClearWork => "general-clear-work",
            Self::GeneralEpochBudget => "general-epoch-budget",
            Self::GeneralOwnerSettlement => "general-owner-settlement-v5",
            Self::GeneralSettlementReceipt => "general-settlement-receipt-v5",
            Self::GeneralSettlementRoot => "general-settlement-root-v1",
            Self::GeneralSettlementCashPot => "general-settlement-cash-pot",
            Self::GeneralFinalPot => "general-final-pot",
            Self::ProductMarketLifecycleRootV1 => "product-market-lifecycle-root-v1",
            Self::ProductSeriesMarketLinkV1 => "product-series-market-link-v1",
            Self::SeriesRegistryV2 => "series-registry-v2",
            Self::SeriesFundingV2 => "series-funding-v2",
            Self::ArtifactUploadStage => "artifact-upload-stage-v1",
            Self::ArtifactRegistryProgramReleaseV2 => "artifact-registry-program-release-v2",
            Self::ArtifactRegistryCapabilityProfileV4 => {
                "artifact-registry-capability-profile-v4"
            }
            Self::ArtifactSourceReleaseManifestV2 => "artifact-source-release-manifest-v2",
            Self::ArtifactNativeClaimBasisV1 => "artifact-native-claim-basis-v1",
            Self::ArtifactEvidenceOnlyRecoveryPolicyV1 => {
                "artifact-evidence-only-recovery-policy-v1"
            }
            Self::ArtifactProductTemplateV4 => "artifact-product-template-v4",
            Self::ArtifactPriceMeasurePolicyV1 => "artifact-price-measure-policy-v1",
            Self::ArtifactMarketGenesisProfileV2 => "artifact-market-genesis-profile-v2",
            Self::ArtifactSeriesFundingQuoteV4 => "artifact-series-funding-quote-v4",
            Self::ArtifactSeriesAttachmentPlanV4 => "artifact-series-attachment-plan-v4",
            Self::ArtifactSeriesPlanV5 => "artifact-series-plan-v5",
            Self::ArtifactSeriesFundingTermsV2 => "artifact-series-funding-terms-v2",
            Self::ArtifactCompiledProductSeriesBundleV5 => {
                "artifact-compiled-product-series-bundle-v5"
            }
            Self::SourceRelease => "source-release",
            Self::SourceHead => "source-head",
            Self::SourceOpenRawPage => "source-open-raw-page",
            Self::SourceRawPage => "source-raw-page",
            Self::SourceWindowWork => "source-window-work",
            Self::SourceWindowSeal => "source-window-seal",
            Self::SourceStatisticResult => "source-statistic-result",
            Self::SourceLineage => "source-lineage",
            Self::SourceWorkReceipt => "source-work-receipt",
            Self::FeeSelectedRecord => "fee-selected-record",
            Self::FeeOwnerCarryV3 => "fee-owner-carry-v3",
            Self::FeeOwnerFinalizationV4 => "fee-owner-finalization-v4",
            Self::FeePayerAllocationV2 => "fee-payer-allocation-v2",
            Self::FeeRecipientAllocationV2 => "fee-recipient-allocation-v2",
            Self::FeeTreasuryLedger => "fee-treasury-ledger",
            Self::LivenessPolicy => "liveness-policy",
            Self::LivenessCompartment => "liveness-compartment",
            Self::PositionV3 => "position-v3",
            Self::ReplayV3 => "replay-v3",
            Self::StructuredClaimDescriptor => "structured-claim-descriptor",
            Self::DealerPolicy => "dealer-policy-v1",
            Self::DealerLivenessSchedule => "dealer-liveness-schedule-v1",
            Self::DealerStateV2 => "dealer-state-v2",
            Self::DealerFundedDependenciesV2 => "dealer-funded-dependencies-v2",
            Self::DealerLpPageV2 => "dealer-lp-page-v2",
            Self::DealerLeaseV2 => "dealer-lease-v2",
            Self::DealerSettlementPotV2 => "dealer-settlement-pot-v2",
            Self::DealerEpochBindingV2 => "dealer-epoch-binding-v2",
            Self::DealerTerminalAllocation => "dealer-terminal-allocation-v1",
            Self::DealerClaimWork => "dealer-claim-work-v1",
            Self::DealerRootTombstoneV2 => "dealer-root-tombstone-v2",
            Self::DealerExitTicket => "dealer-exit-ticket-v1",
            Self::DealerActionReceipt => "dealer-action-receipt-v1",
            Self::DealerCoveredSelection => "dealer-covered-selection-v1",
            Self::DealerReplay => "dealer-replay",
            Self::FailureExternalRoot => "failure-external-root",
            Self::FailureMarketRootV2 => "failure-market-root-v2",
            Self::FailureLivenessPolicy => "failure-liveness-policy",
            Self::FailureRecoveryCompartment => "failure-recovery-compartment",
            Self::FailureReplayTombstone => "failure-replay-tombstone",
            Self::FailureIntervalConsensusWork => "failure-interval-consensus-work-v1",
            Self::FailureIntervalConsensusReplay => "failure-interval-consensus-replay-v1",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeState {
    Canonical,
    RequiresContext(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeeperHint {
    pub lane: Option<WorkflowLane>,
    pub position: WorkflowPosition,
    pub action: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalAccountProjection {
    pub family: CanonicalFamily,
    pub kind: CanonicalAccountKind,
    pub decode_state: DecodeState,
    pub generation: Option<u64>,
    pub primary_binding: Option<[u8; 32]>,
    pub secondary_binding: Option<[u8; 32]>,
    pub keeper_hint: Option<KeeperHint>,
}

impl CanonicalAccountProjection {
    fn canonical(family: CanonicalFamily, kind: CanonicalAccountKind) -> Self {
        Self {
            family,
            kind,
            decode_state: DecodeState::Canonical,
            generation: None,
            primary_binding: None,
            secondary_binding: None,
            keeper_hint: None,
        }
    }

    fn contextual(
        family: CanonicalFamily,
        kind: CanonicalAccountKind,
        requirement: &'static str,
    ) -> Self {
        Self {
            decode_state: DecodeState::RequiresContext(requirement),
            ..Self::canonical(family, kind)
        }
    }
}

/// Decoder inputs which are release-wide facts rather than account-local state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalDecoderContext {
    pub source_neutral_sink: RuntimeKey,
}

/// A raw immutable Product artifact may enter the index only after a hostile
/// SeriesRegistryV2 -> BundleV5 traversal supplied its exact kind, digest and
/// owning bundle.  This is a derived cache, never a second authority record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductArtifactExpectation {
    kind: ArtifactKind,
    digest: [u8; 32],
    /// Zero means the same immutable artifact is named by more than one
    /// authenticated bundle in the scan.  The exact kind/digest remains
    /// authoritative; callers must choose the owning Series context.
    unique_bundle_id: [u8; 32],
}

type ProductArtifactExpectations = BTreeMap<(String, Address), ProductArtifactExpectation>;

pub struct CanonicalAccountDecoderRegistry<'a> {
    plan: &'a RpcIndexPlan,
    context: CanonicalDecoderContext,
    product_artifacts: &'a ProductArtifactExpectations,
}

impl<'a> CanonicalAccountDecoderRegistry<'a> {
    #[must_use]
    pub const fn new(plan: &'a RpcIndexPlan, context: CanonicalDecoderContext) -> Self {
        Self {
            plan,
            context,
            product_artifacts: &EMPTY_PRODUCT_ARTIFACT_EXPECTATIONS,
        }
    }

    const fn with_product_artifacts(
        plan: &'a RpcIndexPlan,
        context: CanonicalDecoderContext,
        product_artifacts: &'a ProductArtifactExpectations,
    ) -> Self {
        Self {
            plan,
            context,
            product_artifacts,
        }
    }

    pub fn decode(&self, account: &ObservedRpcAccount) -> Result<CanonicalAccountProjection> {
        if account.executable {
            return Err(AccountIndexError::ExecutableDataAccount);
        }
        if account.provenance.cluster_key != self.plan.cluster.key() {
            return Err(AccountIndexError::WrongCluster);
        }
        let release = self
            .plan
            .releases
            .iter()
            .find(|release| release.key() == account.provenance.release_key)
            .ok_or(AccountIndexError::UnknownRelease)?;
        if release.program_id != account.owner {
            return Err(AccountIndexError::WrongOwner);
        }
        if let Some(expectation) = self
            .product_artifacts
            .get(&(account.provenance.release_key.clone(), account.address))
        {
            return decode_product_artifact_final(account, *expectation);
        }
        if release.families.iter().any(|family| {
            matches!(family, CanonicalFamily::Product | CanonicalFamily::Series)
        }) {
            if let Some(projection) = decode_artifact_stage(account)? {
                return Ok(projection);
            }
        }
        if release.families.contains(&CanonicalFamily::Series) {
            if let Some(projection) = decode_series_registry_account(account)? {
                return Ok(projection);
            }
            if let Some(projection) = decode_series_funding_account(account)? {
                return Ok(projection);
            }
        }
        let mut decoded = None;
        for family in &release.families {
            if let Some(projection) = self.decode_family(*family, account)? {
                if decoded.replace(projection).is_some() {
                    return Err(AccountIndexError::AmbiguousCodec);
                }
            }
        }
        decoded.ok_or(AccountIndexError::UnknownCodec)
    }

    fn decode_family(
        &self,
        family: CanonicalFamily,
        account: &ObservedRpcAccount,
    ) -> Result<Option<CanonicalAccountProjection>> {
        match family {
            CanonicalFamily::Collateral => decode_collateral(&account.data),
            CanonicalFamily::Fractional => decode_fractional(&account.data),
            CanonicalFamily::General => decode_general(&account.data),
            CanonicalFamily::Product => decode_product(&account.data),
            CanonicalFamily::Source => decode_source(&account.data, self.context),
            CanonicalFamily::Series => decode_series(&account.data),
            CanonicalFamily::Fees => decode_fee(&account.data),
            CanonicalFamily::Liveness => decode_liveness(&account.data),
            CanonicalFamily::PositionV3 => decode_position(&account.data),
            CanonicalFamily::ReplayV3 => decode_replay(&account.data),
            CanonicalFamily::StructuredClaim => decode_structured(&account.data),
            CanonicalFamily::Dealer => decode_dealer(&account.data),
            CanonicalFamily::Failure => decode_failure(&account.data),
        }
    }
}

static EMPTY_PRODUCT_ARTIFACT_EXPECTATIONS: ProductArtifactExpectations = BTreeMap::new();

fn decode_artifact_stage(
    account: &ObservedRpcAccount,
) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(&account.data, ARTIFACT_STAGE_TAG, ARTIFACT_STAGE_VERSION) {
        return Ok(None);
    }
    let header = decode_stage(&account.data)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    if header.binding.kind.registration_status() != ArtifactRegistrationStatus::Current {
        return Err(AccountIndexError::CanonicalDecodeRefused);
    }
    let kind = [header.binding.kind.byte()];
    let (expected, bump) = Address::find_program_address(
        &[
            ARTIFACT_STAGE_PDA_PREFIX_V1,
            &header.funder,
            &kind,
            &header.binding.context.0,
            &header.binding.digest.0,
        ],
        &account.owner,
    );
    if expected != account.address || bump != header.stored_bump {
        return Err(AccountIndexError::CanonicalDecodeRefused);
    }
    let mut projection = CanonicalAccountProjection::contextual(
        CanonicalFamily::Series,
        CanonicalAccountKind::ArtifactUploadStage,
        "self-describing upload stage; never immutable artifact authority",
    );
    projection.primary_binding = Some(header.binding.digest.0);
    projection.secondary_binding = Some(header.binding.context.0);
    if !header.is_complete() {
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::Creation),
            position: WorkflowPosition {
                phase: 1,
                item: u64::from(header.cursor),
            },
            action: "continue-or-abort-artifact-upload",
        });
    }
    Ok(Some(projection))
}

fn canonical_artifact_kind(kind: ArtifactKind) -> Result<CanonicalAccountKind> {
    match kind {
        ArtifactKind::RegistryProgramReleaseV2 => {
            Ok(CanonicalAccountKind::ArtifactRegistryProgramReleaseV2)
        }
        ArtifactKind::RegistryCapabilityProfileV4 => {
            Ok(CanonicalAccountKind::ArtifactRegistryCapabilityProfileV4)
        }
        ArtifactKind::SourceReleaseManifestV2 => {
            Ok(CanonicalAccountKind::ArtifactSourceReleaseManifestV2)
        }
        ArtifactKind::NativeClaimBasisV1 => {
            Ok(CanonicalAccountKind::ArtifactNativeClaimBasisV1)
        }
        ArtifactKind::EvidenceOnlyRecoveryPolicyV1 => {
            Ok(CanonicalAccountKind::ArtifactEvidenceOnlyRecoveryPolicyV1)
        }
        ArtifactKind::ProductTemplateV4 => {
            Ok(CanonicalAccountKind::ArtifactProductTemplateV4)
        }
        ArtifactKind::PriceMeasurePolicyV1 => {
            Ok(CanonicalAccountKind::ArtifactPriceMeasurePolicyV1)
        }
        ArtifactKind::MarketGenesisProfileV2 => {
            Ok(CanonicalAccountKind::ArtifactMarketGenesisProfileV2)
        }
        ArtifactKind::SeriesFundingQuoteV4 => {
            Ok(CanonicalAccountKind::ArtifactSeriesFundingQuoteV4)
        }
        ArtifactKind::SeriesAttachmentPlanV4 => {
            Ok(CanonicalAccountKind::ArtifactSeriesAttachmentPlanV4)
        }
        ArtifactKind::SeriesPlanV5 => Ok(CanonicalAccountKind::ArtifactSeriesPlanV5),
        ArtifactKind::SeriesFundingTermsV2 => {
            Ok(CanonicalAccountKind::ArtifactSeriesFundingTermsV2)
        }
        ArtifactKind::CompiledProductSeriesBundleV5 => {
            Ok(CanonicalAccountKind::ArtifactCompiledProductSeriesBundleV5)
        }
        _ => Err(AccountIndexError::CanonicalDecodeRefused),
    }
}

fn require_typed_product_artifact(
    kind: ArtifactKind,
    digest: [u8; 32],
    body: &[u8],
) -> Result<()> {
    let actual = match kind {
        ArtifactKind::NativeClaimBasisV1 => NativeClaimBasisV1::decode(body)
            .and_then(|value| value.id())
            .map(|id| id.bytes()),
        ArtifactKind::EvidenceOnlyRecoveryPolicyV1 => {
            EvidenceOnlyRecoveryPolicyV1::decode(body)
                .and_then(|value| value.id())
                .map(|id| id.bytes())
        }
        ArtifactKind::ProductTemplateV4 => ProductTemplateV4::decode(body)
            .and_then(|value| value.id())
            .map(|id| id.bytes()),
        ArtifactKind::PriceMeasurePolicyV1 => PriceMeasurePolicyV1::decode(body)
            .and_then(|value| value.id())
            .map(|id| id.bytes()),
        ArtifactKind::MarketGenesisProfileV2 => MarketGenesisProfileV2::decode(body)
            .and_then(|value| value.id())
            .map(|id| id.bytes()),
        ArtifactKind::SeriesPlanV5 => SeriesPlanV5::decode(body)
            .and_then(|value| value.id())
            .map(|id| id.bytes()),
        ArtifactKind::SeriesFundingTermsV2 => SeriesFundingTermsV2::decode(body)
            .and_then(|value| value.id())
            .map(|id| id.bytes()),
        _ => {
            let exact_len = u16::try_from(kind.exact_len())
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            validate_artifact(
                ArtifactBinding {
                    kind,
                    context: Hash32::ZERO,
                    digest: Hash32::from_bytes(digest),
                    exact_len,
                },
                body,
            )
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            return Ok(());
        }
    }
    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    if actual != digest {
        return Err(AccountIndexError::CanonicalDecodeRefused);
    }
    Ok(())
}

fn decode_product_artifact_final(
    account: &ObservedRpcAccount,
    expectation: ProductArtifactExpectation,
) -> Result<CanonicalAccountProjection> {
    require_typed_product_artifact(expectation.kind, expectation.digest, &account.data)?;
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::Series,
        canonical_artifact_kind(expectation.kind)?,
    );
    projection.primary_binding = Some(expectation.digest);
    if expectation.unique_bundle_id != [0; 32] {
        projection.secondary_binding = Some(expectation.unique_bundle_id);
    } else {
        projection.decode_state = DecodeState::RequiresContext(
            "artifact is authenticated by multiple BundleV5 owners",
        );
    }
    Ok(projection)
}

fn decode_series_registry_account(
    account: &ObservedRpcAccount,
) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(
        &account.data,
        registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG,
        registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2,
    ) {
        return Ok(None);
    }
    let value = SeriesRegistryAccountV2::decode(&account.data)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let (expected, bump) = Address::find_program_address(
        &[
            SERIES_REGISTRY_PDA_PREFIX_V1,
            &value.series_plan_id.bytes(),
        ],
        &account.owner,
    );
    if expected != account.address || bump != value.stored_bump {
        return Err(AccountIndexError::CanonicalDecodeRefused);
    }
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::Series,
        CanonicalAccountKind::SeriesRegistryV2,
    );
    projection.primary_binding = Some(value.series_plan_id.bytes());
    projection.secondary_binding = Some(value.compiler_bundle_id.bytes());
    Ok(Some(projection))
}

fn decode_series_funding_account(
    account: &ObservedRpcAccount,
) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(
        &account.data,
        registry::SOURCE_SERIES_FUNDING_ACCOUNT_TAG,
        registry::SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V2,
    ) {
        return Ok(None);
    }
    let value = SeriesFundingAccountV2::decode(&account.data)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let (expected, bump) = Address::find_program_address(
        &[
            SERIES_FUNDING_PDA_PREFIX_V1,
            &value.state.series_plan_id.bytes(),
        ],
        &account.owner,
    );
    if expected != account.address || bump != value.stored_bump {
        return Err(AccountIndexError::CanonicalDecodeRefused);
    }
    let terminal = value.state.phase == SeriesFundingPhaseV2::Closed;
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::Series,
        CanonicalAccountKind::SeriesFundingV2,
    );
    projection.generation = Some(value.state.transition_sequence);
    projection.primary_binding = Some(value.state.series_plan_id.bytes());
    projection.secondary_binding = Some(value.state.compiler_bundle_id.bytes());
    projection.keeper_hint = Some(KeeperHint {
        lane: Some(if terminal {
            WorkflowLane::RecoveryRetirement
        } else {
            WorkflowLane::Creation
        }),
        position: WorkflowPosition {
            phase: if terminal { 8 } else { 6 },
            item: u64::from(value.state.next_ordinal),
        },
        action: if terminal {
            "close-series-funding-v2"
        } else {
            "advance-series-occurrence-v2"
        },
    });
    Ok(Some(projection))
}

fn decode_collateral(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    let projection = if tag_version(data, HOARD_V2_TAG, HOARD_V2_VERSION)
        && data.len() == HOARD_V2_BYTES
    {
        let value = HoardV2::decode(data).map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Collateral,
            CanonicalAccountKind::CollateralHoardV2,
        );
        projection.primary_binding = Some(value.market_instance_id.bytes());
        projection.secondary_binding = Some(value.realm_id.bytes());
        projection
    } else if tag_version(data, CLAIM_LEDGER_V3_TAG, CLAIM_LEDGER_V3_VERSION)
        && data.len() == CLAIM_LEDGER_V3_BYTES
    {
        let value =
            ClaimLedgerV3::decode(data).map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Collateral,
            CanonicalAccountKind::CollateralClaimLedgerV3,
        );
        projection.primary_binding = Some(value.market_instance_id.bytes());
        projection.secondary_binding = Some(value.native_claim_basis_id.bytes());
        projection
    } else if tag_version(data, RESOLUTION_V5_TAG, RESOLUTION_V5_VERSION)
        && data.len() == RESOLUTION_V5_BYTES
    {
        let value =
            ResolutionV5::decode(data).map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Collateral,
            CanonicalAccountKind::CollateralResolutionV5,
        );
        projection.generation = Some(value.facts.generation);
        projection.primary_binding = Some(value.facts.market_instance_id.bytes());
        projection.secondary_binding = Some(value.facts.native_claim_basis_id.bytes());
        projection
    } else {
        return Ok(None);
    };
    Ok(Some(projection))
}

fn decode_fractional(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        FRACTIONAL_POLICY_ACCOUNT_TAG,
        FRACTIONAL_POLICY_ACCOUNT_VERSION,
    ) {
        if data.len() != FRACTIONAL_POLICY_ACCOUNT_BYTES {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        let value = FractionalPolicyV2::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::contextual(
            CanonicalFamily::Fractional,
            CanonicalAccountKind::FractionalPolicyV2,
            "Resolution V5 data identity and Realm collateral join",
        );
        projection.generation = Some(value.domain_generation);
        projection.primary_binding = Some(value.market_instance.bytes());
        projection.secondary_binding = Some(value.resolution_data_id.bytes());
        Ok(Some(projection))
    } else if tag_version(
        data,
        FRACTIONAL_LEDGER_ACCOUNT_TAG,
        FRACTIONAL_LEDGER_ACCOUNT_VERSION,
    ) {
        if data.len() != FRACTIONAL_LEDGER_ACCOUNT_BYTES {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        let value = FractionalLedgerV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::contextual(
            CanonicalFamily::Fractional,
            CanonicalAccountKind::FractionalLedgerV1,
            "immutable fractional policy and ClaimLedger V3 join",
        );
        projection.generation = Some(value.domain_generation);
        projection.primary_binding = Some(value.policy_account.bytes());
        projection.secondary_binding = Some(value.claim_ledger_account.bytes());
        Ok(Some(projection))
    } else if tag_version(
        data,
        FRACTIONAL_CREDIT_ACCOUNT_TAG,
        FRACTIONAL_CREDIT_ACCOUNT_VERSION,
    ) {
        if data.len() != FRACTIONAL_CREDIT_ACCOUNT_BYTES {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        let value = FractionalCreditV2::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::contextual(
            CanonicalFamily::Fractional,
            CanonicalAccountKind::FractionalCreditV2,
            "policy, aggregate ledger, and exact payout denominator join",
        );
        projection.generation = Some(value.domain_generation);
        projection.primary_binding = Some(value.market_instance.bytes());
        projection.secondary_binding = Some(value.claimant.bytes());
        Ok(Some(projection))
    } else if tag_version(
        data,
        FRACTIONAL_CREDIT_TOMBSTONE_TAG,
        FRACTIONAL_CREDIT_TOMBSTONE_VERSION,
    ) {
        if data.len() != FRACTIONAL_CREDIT_TOMBSTONE_BYTES {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        let value = FractionalCreditTombstoneV2::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Fractional,
            CanonicalAccountKind::FractionalCreditTombstoneV2,
        );
        projection.generation = Some(value.domain_generation);
        projection.primary_binding = Some(value.market_instance.bytes());
        projection.secondary_binding = Some(value.claimant.bytes());
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}
fn tag_version(data: &[u8], tag: u8, version: u8) -> bool {
    data.first() == Some(&tag) && data.get(1) == Some(&version)
}

fn decode_general(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    let mut projection = if tag_version(
        data,
        MARKET_RUNTIME_ACCOUNT_TAG,
        MARKET_RUNTIME_ACCOUNT_VERSION,
    ) {
        let value = MarketRuntimeV3AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralMarketRuntime,
        );
        projection.generation = Some(value.next_epoch_generation);
        projection.primary_binding = Some(value.market_instance_v2_id.bytes());
        projection
    } else if tag_version(
        data,
        GENERAL_EPOCH_ACCOUNT_TAG,
        GENERAL_EPOCH_ACCOUNT_VERSION,
    ) {
        let value = GeneralEpochV6AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralEpoch,
        );
        projection.generation = Some(value.generation);
        projection.primary_binding = Some(value.market_runtime.bytes());
        projection.secondary_binding = Some(value.window.bytes());
        projection
    } else if tag_version(
        data,
        ECONOMIC_DOMAIN_ACCOUNT_TAG,
        ECONOMIC_DOMAIN_ACCOUNT_VERSION,
    ) {
        let value = EconomicDomainV2AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralEconomicDomain,
        );
        projection.primary_binding = Some(value.epoch.bytes());
        projection
    } else if tag_version(
        data,
        MARKET_BINDING_ACCOUNT_TAG,
        MARKET_BINDING_ACCOUNT_VERSION_V2,
    ) {
        let value =
            MarketBindingV2::decode(data).map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralMarketBinding,
        );
        projection.primary_binding = Some(value.base().market.bytes());
        projection.secondary_binding = Some(value.base().market_instance_v2_id.bytes());
        projection
    } else if tag_version(
        data,
        registry::GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG,
        registry::GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION,
    ) {
        let value = OrderPageAccountV5::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralOrderPage,
        );
        projection.primary_binding = Some(value.page.market.bytes());
        projection.secondary_binding = Some(value.page.epoch.bytes());
        projection
    } else if tag_version(
        data,
        registry::GENERAL_RESERVATION_V9_ACCOUNT_TAG,
        registry::GENERAL_RESERVATION_V9_ACCOUNT_VERSION,
    ) {
        let value = ReservationAccountV9::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let body = value.body();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralReservation,
        );
        projection.generation = Some(body.position_generation);
        projection.primary_binding = Some(body.market.bytes());
        projection.secondary_binding = Some(body.owner.bytes());
        projection
    } else if tag_version(data, WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION_V2) {
        let value = CandidateWindowV5AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let value = value.base();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralCandidateWindow,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection
    } else if tag_version(
        data,
        ADMISSION_NODE_ACCOUNT_TAG,
        ADMISSION_NODE_ACCOUNT_VERSION_V2,
    ) {
        let value = AdmissionNodeV4AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let value = value.base();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralAdmissionNode,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection
    } else if tag_version(
        data,
        CANDIDATE_FEED_STAGE_ACCOUNT_TAG,
        CANDIDATE_FEED_STAGE_ACCOUNT_VERSION,
    ) {
        let (value, _) = complete_candidate_feed_v2(data, false)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralCandidateFeedStage,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection
    } else if tag_version(
        data,
        CANDIDATE_FEED_ACCOUNT_TAG,
        CANDIDATE_FEED_ACCOUNT_VERSION,
    ) {
        let (value, _) = complete_candidate_feed_v2(data, true)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralCandidateFeed,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection
    } else if tag_version(data, CLEAR_WORK_ACCOUNT_TAG, CLEAR_WORK_ACCOUNT_VERSION_V3) {
        let value = ClearWorkV3AccountV1::decode_account(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralClearWork,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection.secondary_binding = Some(value.node.bytes());
        projection
    } else if tag_version(data, EPOCH_BUDGET_ACCOUNT_TAG, EPOCH_BUDGET_ACCOUNT_VERSION) {
        let value = EpochBudgetV2AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralEpochBudget,
        );
        projection.generation = Some(value.epoch_generation);
        projection.primary_binding = Some(value.epoch.bytes());
        projection
    } else if tag_version(
        data,
        OWNER_SETTLEMENT_ACCOUNT_TAG,
        OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
    ) {
        let value = OwnerSettlementV5AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let expectation = value.semantic.expectation();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralOwnerSettlement,
        );
        projection.primary_binding = Some(expectation.epoch());
        projection.secondary_binding = Some(expectation.owner());
        // FinalizeOwner's cursor generation is owned by the joined Position V3,
        // not this accumulator or its Epoch. Do not invent a parallel value.
        projection
    } else if tag_version(
        data,
        registry::GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG,
        registry::GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION,
    ) {
        let value = SettlementReceiptAccountV5::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let semantic = value.semantic();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralSettlementReceipt,
        );
        projection.primary_binding = Some(semantic.epoch.bytes());
        projection.secondary_binding = Some(semantic.candidate.bytes());
        projection
    } else if tag_version(
        data,
        SETTLEMENT_ROOT_ACCOUNT_TAG,
        SETTLEMENT_ROOT_ACCOUNT_VERSION,
    ) {
        let value = SettlementRootV1AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralSettlementRoot,
        );
        projection.generation = Some(value.epoch_generation());
        projection.primary_binding = Some(value.epoch().bytes());
        projection.secondary_binding = Some(value.settlement_candidate_id().bytes());
        projection
    } else if tag_version(
        data,
        SETTLEMENT_CASH_POT_ACCOUNT_TAG,
        SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
    ) {
        let value = SettlementCashPotV1AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralSettlementCashPot,
        );
        projection.primary_binding = Some(value.semantic.expectation.epoch);
        projection.secondary_binding = Some(value.semantic.expectation.candidate);
        projection
    } else if tag_version(data, FINAL_POT_ACCOUNT_TAG, FINAL_POT_ACCOUNT_VERSION)
        && data.len() == FINAL_POT_ACCOUNT_BYTES
    {
        CanonicalAccountProjection::contextual(
            CanonicalFamily::General,
            CanonicalAccountKind::GeneralFinalPot,
            "authenticated SettlementRoot and PDA binding",
        )
    } else {
        return Ok(None);
    };
    /* No General extension tuple is executable in the checked release. Keep
     * canonical account indexing, but never advertise an impossible keeper
     * action merely because an authenticated historical/current-state account
     * has a locally recognizable cursor. */
    projection.keeper_hint = None;
    Ok(Some(projection))
}

fn decode_product(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_TAG,
        registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_VERSION,
    ) {
        let value = MarketLifecycleRootAccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let binding = value.state.binding();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Product,
            CanonicalAccountKind::ProductMarketLifecycleRootV1,
        );
        projection.generation = Some(binding.generation);
        projection.primary_binding = Some(binding.market_instance_id.bytes());
        projection.secondary_binding = Some(
            binding
                .id()
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?
                .bytes(),
        );
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}

fn decode_series(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_TAG,
        registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_VERSION,
    ) {
        let value = SeriesMarketLinkAccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let binding = value.state.binding();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Series,
            CanonicalAccountKind::ProductSeriesMarketLinkV1,
        );
        projection.generation = Some(binding.generation);
        projection.primary_binding = Some(binding.market_instance_id.bytes());
        projection.secondary_binding = Some(binding.series_plan_id.bytes());
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}

fn decode_source(
    data: &[u8],
    context: CanonicalDecoderContext,
) -> Result<Option<CanonicalAccountProjection>> {
    let runtime = |kind, generation, primary| {
        let mut projection = CanonicalAccountProjection::canonical(CanonicalFamily::Source, kind);
        projection.generation = generation;
        projection.primary_binding = primary;
        projection
    };
    let projection = match data.first().copied() {
        Some(SOURCE_RELEASE_ACCOUNT_TAG)
            if data.get(1) == Some(&SOURCE_RELEASE_ACCOUNT_VERSION) =>
        {
            let value = SourceReleaseManifestV2::decode(data)
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceRelease,
                None,
                Some(value.base.source_spec_id.bytes()),
            );
            projection.secondary_binding = Some(
                value
                    .id()
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?
                    .bytes(),
            );
            projection
        }
        Some(SOURCE_HEAD_ACCOUNT_TAG) => {
            let (header, value) =
                decode_runtime_account::<SourceHeadV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceHead,
                Some(header.generation),
                Some(value.source_spec_id.bytes()),
            );
            projection.keeper_hint = Some(KeeperHint {
                lane: Some(WorkflowLane::SourceCrank),
                position: WorkflowPosition {
                    phase: 2,
                    item: value.page_count,
                },
                action: "open-raw-page",
            });
            projection
        }
        Some(OPEN_RAW_PAGE_ACCOUNT_TAG) => {
            let (header, value) =
                decode_runtime_account::<OpenRawPageV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let full = usize::from(value.record_count) == MAX_RAW_PAGE_RECORDS;
            let next_bucket = value
                .start_bucket
                .checked_add(u64::from(value.record_count))
                .ok_or(AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceOpenRawPage,
                Some(header.generation),
                Some(value.source_spec_id.bytes()),
            );
            projection.keeper_hint = Some(KeeperHint {
                lane: Some(WorkflowLane::SourceCrank),
                position: WorkflowPosition {
                    phase: if full { 4 } else { 3 },
                    item: if full { value.page_index } else { next_bucket },
                },
                action: if full {
                    "seal-raw-page"
                } else {
                    "ingest-boundary"
                },
            });
            projection
        }
        Some(RAW_PAGE_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<RawPageV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceRawPage,
                Some(header.generation),
                None,
            )
        }
        Some(WINDOW_WORK_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<WindowWorkV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceWindowWork,
                Some(header.generation),
                None,
            )
        }
        Some(WINDOW_SEAL_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<WindowSealV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceWindowSeal,
                Some(header.generation),
                None,
            )
        }
        Some(STATISTIC_RESULT_ACCOUNT_TAG) => {
            let (header, _) =
                decode_runtime_account::<StatisticResultV3>(data, context.source_neutral_sink)
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            runtime(
                CanonicalAccountKind::SourceStatisticResult,
                Some(header.generation),
                None,
            )
        }
        Some(REOPEN_LINEAGE_ACCOUNT_TAG)
            if data.get(1) == Some(&REOPEN_LINEAGE_ACCOUNT_VERSION) =>
        {
            let value = ReopenLineageV1::decode(data)
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceLineage,
                (value.latest_generation > 0).then_some(value.latest_generation),
                Some(value.semantic_binding_id.bytes()),
            );
            projection.secondary_binding = Some(value.active_account.bytes());
            projection
        }
        Some(SOURCE_WORK_RECEIPT_ACCOUNT_TAG)
            if data.get(1) == Some(&SOURCE_WORK_RECEIPT_ACCOUNT_VERSION)
                && data.len() == SOURCE_WORK_RECEIPT_ACCOUNT_BYTES =>
        {
            let value = SourceWorkReceiptAccountV1::decode(data)
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            let mut projection = runtime(
                CanonicalAccountKind::SourceWorkReceipt,
                Some(value.generation()),
                Some(value.route_id().bytes()),
            );
            projection.secondary_binding = Some(value.receipt_id().bytes());
            projection
        }
        _ => return Ok(None),
    };
    Ok(Some(projection))
}

fn decode_fee(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        OWNER_FEE_CARRY_ACCOUNT_TAG,
        OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4,
    ) {
        if data.len() != OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4 {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        OwnerFeeFinalizationV4AccountV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        return Ok(Some(CanonicalAccountProjection::canonical(
            CanonicalFamily::Fees,
            CanonicalAccountKind::FeeOwnerFinalizationV4,
        )));
    }
    if tag_version(
        data,
        PAYER_ALLOCATION_ACCOUNT_TAG,
        PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
    ) {
        if data.len() != PAYER_ALLOCATION_ACCOUNT_BYTES_V2 {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        PayerAllocationV2AccountV1::decode_persisted(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        return Ok(Some(CanonicalAccountProjection::canonical(
            CanonicalFamily::Fees,
            CanonicalAccountKind::FeePayerAllocationV2,
        )));
    }
    if tag_version(
        data,
        RECIPIENT_ALLOCATION_ACCOUNT_TAG,
        RECIPIENT_ALLOCATION_ACCOUNT_VERSION_V2,
    ) {
        if data.len() != RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V2 {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        RecipientAllocationV2AccountV1::decode_persisted(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        return Ok(Some(CanonicalAccountProjection::canonical(
            CanonicalFamily::Fees,
            CanonicalAccountKind::FeeRecipientAllocationV2,
        )));
    }
    let contextual = if tag_version(
        data,
        SELECTED_FEE_RECORD_ACCOUNT_TAG,
        SELECTED_FEE_RECORD_ACCOUNT_VERSION,
    ) && data.len() == SELECTED_FEE_RECORD_ACCOUNT_BYTES
    {
        Some((
            CanonicalAccountKind::FeeSelectedRecord,
            "authenticated batch and revenue-policy preimages",
        ))
    } else if tag_version(
        data,
        OWNER_FEE_CARRY_ACCOUNT_TAG,
        OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
    ) {
        if data.len() != OWNER_FEE_CARRY_ACCOUNT_BYTES_V3 {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        Some((
            CanonicalAccountKind::FeeOwnerCarryV3,
            "canonical carry PDA plus authenticated selected fee record",
        ))
    } else if tag_version(
        data,
        TREASURY_LEDGER_ACCOUNT_TAG,
        TREASURY_LEDGER_ACCOUNT_VERSION,
    ) && data.len() == TREASURY_LEDGER_ACCOUNT_BYTES
    {
        Some((
            CanonicalAccountKind::FeeTreasuryLedger,
            "authenticated selected fee record",
        ))
    } else {
        None
    };
    Ok(contextual.map(|(kind, requirement)| {
        CanonicalAccountProjection::contextual(CanonicalFamily::Fees, kind, requirement)
    }))
}

fn decode_liveness(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if data.starts_with(&RUNTIME_LIVENESS_POLICY_MAGIC_V1)
        && data.len() == RUNTIME_LIVENESS_POLICY_BYTES_V1
    {
        let _ = RuntimeLivenessPolicyV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        Ok(Some(CanonicalAccountProjection::canonical(
            CanonicalFamily::Liveness,
            CanonicalAccountKind::LivenessPolicy,
        )))
    } else if data.starts_with(&RUNTIME_LIVENESS_ACCOUNT_MAGIC_V1)
        && data.len() == RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
    {
        let value = RuntimeCompartmentV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Liveness,
            CanonicalAccountKind::LivenessCompartment,
        );
        projection.generation = Some(value.identity.generation);
        if value.phase == RuntimeCompartmentPhaseV1::Active && value.remaining_calls > 0 {
            let phase = u16::try_from(value.kind.index())
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
            projection.keeper_hint = Some(KeeperHint {
                lane: None,
                position: WorkflowPosition {
                    phase,
                    item: u64::from(value.completed_calls),
                },
                action: "service-liveness-compartment",
            });
        }
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}

fn decode_position(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(data, POSITION_ACCOUNT_TAG, POSITION_ACCOUNT_VERSION_V3) {
        return Ok(None);
    }
    let value =
        PositionAccountV3::decode(data).map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::PositionV3,
        CanonicalAccountKind::PositionV3,
    );
    projection.generation = Some(value.generation());
    projection.primary_binding = Some(value.market_instance_id().bytes());
    projection.secondary_binding = Some(value.replay_account().bytes());
    if value.lifecycle() == PositionLifecycleV3::CloseRequested
        && value.terminal_projection().is_ok()
    {
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::RecoveryRetirement),
            position: WorkflowPosition { phase: 6, item: 0 },
            action: "close-position",
        });
    }
    Ok(Some(projection))
}

struct ReplaySha;

impl ReplayV3HashBackend for ReplaySha {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts {
            hash.update(part);
        }
        hash.finalize().into()
    }
}

fn decode_replay(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(
        data,
        PURPOSE_REPLAY_ACCOUNT_TAG,
        PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
    ) {
        return Ok(None);
    }
    let value = ReplayV3Envelope::decode(data, &ReplaySha)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let header = value.header();
    if header.purpose() == PositionPurposeV3::DealerFacility {
        return Ok(None);
    }
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::ReplayV3,
        CanonicalAccountKind::ReplayV3,
    );
    projection.generation = Some(header.position_generation());
    projection.primary_binding = Some(header.position_account().bytes());
    projection.secondary_binding = Some(header.purpose_binding_id().bytes());
    if header.lifecycle() == ReplayV3Lifecycle::Terminal {
        projection.keeper_hint = Some(KeeperHint {
            lane: Some(WorkflowLane::RecoveryRetirement),
            position: WorkflowPosition {
                phase: 6,
                item: header.next_sequence(),
            },
            action: "close-position-replay",
        });
    }
    Ok(Some(projection))
}

fn decode_structured(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if !tag_version(data, DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION)
        || data.len() != DESCRIPTOR_ACCOUNT_BYTES
    {
        return Ok(None);
    }
    let value = StructuredClaimDescriptorV1::decode(data)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
    let mut projection = CanonicalAccountProjection::canonical(
        CanonicalFamily::StructuredClaim,
        CanonicalAccountKind::StructuredClaimDescriptor,
    );
    projection.primary_binding = Some(value.market);
    if value.state == DescriptorStateV1::Active {
        projection.keeper_hint = Some(KeeperHint {
            lane: None,
            position: WorkflowPosition { phase: 10, item: 0 },
            action: "inspect-structured-claim-retirement",
        });
    }
    Ok(Some(projection))
}

const DEALER_ACCOUNT_ENVELOPE_BYTES: usize = 8;

fn decode_current_dealer_body<T: DealerFixedCodec>(
    data: &[u8],
    tag: u8,
    version: u8,
    account_bytes: usize,
) -> Result<Option<T>> {
    if !tag_version(data, tag, version) {
        return Ok(None);
    }
    if data.len() != account_bytes
        || account_bytes != DEALER_ACCOUNT_ENVELOPE_BYTES.saturating_add(T::ENCODED_LEN)
        || data[3..DEALER_ACCOUNT_ENVELOPE_BYTES]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(AccountIndexError::CanonicalDecodeRefused);
    }
    <T as DealerFixedCodec>::decode(&data[DEALER_ACCOUNT_ENVELOPE_BYTES..])
        .map(Some)
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)
}

fn decode_dealer(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        registry::DEALER_POLICY_ACCOUNT_TAG,
        registry::DEALER_POLICY_ACCOUNT_VERSION,
    ) {
        if data.len() != registry::DEALER_POLICY_ACCOUNT_BYTES
            || data[3..8].iter().any(|byte| *byte != 0)
            || data[8..40].iter().all(|byte| *byte == 0)
            || u64::from_le_bytes(
                data[40..48]
                    .try_into()
                    .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?,
            ) == 0
        {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        let value = <DealerPolicyV1 as DealerFixedCodec>::decode(
            &data[registry::DEALER_POLICY_ACCOUNT_HEADER_BYTES..],
        )
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerPolicy,
        );
        projection.primary_binding = Some(
            value
                .policy_id()
                .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?
                .bytes(),
        );
        projection.secondary_binding = data[8..40].try_into().ok();
        return Ok(Some(projection));
    }

    macro_rules! dealer_body {
        ($type:ty, $tag:ident, $version:ident, $bytes:ident, $kind:ident) => {
            if let Some(_) = decode_current_dealer_body::<$type>(
                data,
                registry::$tag,
                registry::$version,
                registry::$bytes,
            )? {
                return Ok(Some(CanonicalAccountProjection::canonical(
                    CanonicalFamily::Dealer,
                    CanonicalAccountKind::$kind,
                )));
            }
        };
    }
    dealer_body!(
        DealerLivenessScheduleV1,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
        DealerLivenessSchedule
    );

    if let Some(value) = decode_current_dealer_body::<DealerStateV2>(
        data,
        registry::DEALER_STATE_V2_ACCOUNT_TAG,
        registry::DEALER_STATE_V2_ACCOUNT_VERSION,
        registry::DEALER_STATE_V2_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerStateV2,
        );
        projection.generation = Some(value.generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.policy_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerFundedDependenciesV2>(
        data,
        registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
        registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
        registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerFundedDependenciesV2,
        );
        projection.generation = Some(value.bindings.counted_generation);
        projection.primary_binding = Some(value.bindings.facility_id.bytes());
        projection.secondary_binding = Some(value.bindings.policy_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<LpPageV2>(
        data,
        registry::DEALER_LP_PAGE_V2_ACCOUNT_TAG,
        registry::DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
        registry::DEALER_LP_PAGE_V2_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerLpPageV2,
        );
        projection.generation = Some(value.counted_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.dealer_state_account_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerLeaseV2>(
        data,
        registry::DEALER_LEASE_V2_ACCOUNT_TAG,
        registry::DEALER_LEASE_V2_ACCOUNT_VERSION,
        registry::DEALER_LEASE_V2_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerLeaseV2,
        );
        projection.generation = Some(value.post_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.epoch_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<SettlementPotV2>(
        data,
        registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
        registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
        registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerSettlementPotV2,
        );
        projection.generation = Some(value.post_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.lease_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerEpochBindingV2>(
        data,
        registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG,
        registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
        registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerEpochBindingV2,
        );
        projection.generation = Some(value.counted_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.epoch_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerTerminalAllocationV1>(
        data,
        registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG,
        registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION,
        registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerTerminalAllocation,
        );
        projection.generation = Some(value.counted_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.lp_page_account_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerClaimWorkV1>(
        data,
        registry::DEALER_CLAIM_WORK_ACCOUNT_TAG,
        registry::DEALER_CLAIM_WORK_ACCOUNT_VERSION,
        registry::DEALER_CLAIM_WORK_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerClaimWork,
        );
        projection.generation = Some(value.counted_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.market_instance_v2_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerRootTombstoneV2>(
        data,
        registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_TAG,
        registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_VERSION,
        registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerRootTombstoneV2,
        );
        projection.generation = Some(value.terminal_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.dealer_state_account_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerExitTicketV1>(
        data,
        registry::DEALER_EXIT_TICKET_ACCOUNT_TAG,
        registry::DEALER_EXIT_TICKET_ACCOUNT_VERSION,
        registry::DEALER_EXIT_TICKET_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerExitTicket,
        );
        projection.generation = Some(value.counted_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.owner.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<DealerActionReceiptV1>(
        data,
        registry::DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        registry::DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        registry::DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerActionReceipt,
        );
        projection.generation = Some(value.facility_generation);
        projection.primary_binding = Some(value.facility_id.bytes());
        projection.secondary_binding = Some(value.receipt_account_id.bytes());
        return Ok(Some(projection));
    }
    if let Some(value) = decode_current_dealer_body::<CoveredDealerSelectionV1>(
        data,
        registry::DEALER_COVERED_SELECTION_ACCOUNT_TAG,
        registry::DEALER_COVERED_SELECTION_ACCOUNT_VERSION,
        registry::DEALER_COVERED_SELECTION_ACCOUNT_BYTES,
    )? {
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerCoveredSelection,
        );
        projection.generation = Some(value.dealer_generation);
        projection.primary_binding = Some(value.market_instance_v2_id.bytes());
        projection.secondary_binding = Some(value.facility_id.bytes());
        return Ok(Some(projection));
    }

    if tag_version(
        data,
        PURPOSE_REPLAY_ACCOUNT_TAG,
        PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
    ) && data.len() == DEALER_FACILITY_REPLAY_BYTES_V1
    {
        let envelope = ReplayV3Envelope::decode(data, &ReplaySha)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        if envelope.header().purpose() != PositionPurposeV3::DealerFacility {
            return Ok(None);
        }
        let value = <DealerFacilityReplayV1 as DealerFixedCodec>::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Dealer,
            CanonicalAccountKind::DealerReplay,
        );
        projection.generation = Some(value.position_generation());
        projection.primary_binding = Some(value.facility_position_account_id().bytes());
        projection.secondary_binding = Some(value.facility_position_binding_id().bytes());
        return Ok(Some(projection));
    };
    Ok(None)
}

fn decode_failure(data: &[u8]) -> Result<Option<CanonicalAccountProjection>> {
    if tag_version(
        data,
        registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
        registry::FAILURE_MARKET_ROOT_ACCOUNT_VERSION_V2,
    ) {
        let bytes: &[u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2] = data
            .try_into()
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let record = FailureMarketRootAccountV2::decode(bytes)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let state = FailureMarketAdmissionStateV1::decode(&record.admission_body)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let policy = state.binding();
        let facts = policy.facts();
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureMarketRootV2,
        );
        projection.generation = Some(facts.generation);
        projection.primary_binding = Some(facts.market_instance_id.bytes());
        projection.secondary_binding = Some(policy.id().bytes());
        Ok(Some(projection))
    } else if tag_version(
        data,
        registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
    ) && data.len() == FAILURE_EXTERNAL_ROOT_ACCOUNT_BYTES_V1
    {
        let _ = decode_failure_account_body_v1(
            data,
            registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
            registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_VERSION,
            FAILURE_EXTERNAL_ROOT_BODY_BYTES_V2,
        )
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        Ok(Some(CanonicalAccountProjection::contextual(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureExternalRoot,
            "failure adapter root digest and account-key authentication",
        )))
    } else if tag_version(
        data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
    ) && data.len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1
    {
        let framed = decode_failure_account_body_v1(
            data,
            registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
            registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
            FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
        )
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let _ = RuntimeLivenessPolicyV1::decode(framed.body)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        Ok(Some(CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureLivenessPolicy,
        )))
    } else if tag_version(
        data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
    ) && data.len() == FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1
    {
        let framed = decode_failure_account_body_v1(
            data,
            registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
            registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
            FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
        )
        .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let value = RuntimeCompartmentV1::decode(framed.body)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureRecoveryCompartment,
        );
        projection.generation = Some(value.identity.generation);
        projection.keeper_hint = (value.phase == RuntimeCompartmentPhaseV1::Active
            && value.remaining_calls > 0)
            .then_some(KeeperHint {
                lane: None,
                position: WorkflowPosition {
                    phase: 1,
                    item: u64::from(value.completed_calls),
                },
                action: "advance-failure-recovery",
            });
        Ok(Some(projection))
    } else if tag_version(
        data,
        registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG,
        registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION,
    ) {
        let value = FailureReplayTombstoneV1::decode(data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureReplayTombstone,
        );
        projection.generation = Some(value.generation);
        projection.primary_binding = Some(value.market_instance_v2_id);
        Ok(Some(projection))
    } else if tag_version(
        data,
        registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG,
        registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION,
    ) {
        let bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES] = data
            .try_into()
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let value = FailureIntervalConsensusWorkAccountV1::decode(bytes)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureIntervalConsensusWork,
        );
        projection.generation = Some(value.generation);
        projection.primary_binding = Some(value.interval_binding_id);
        projection.secondary_binding = Some(value.failure_policy_binding_id);
        projection.keeper_hint = (value.phase == FailureIntervalConsensusPhaseV1::Active)
            .then_some(KeeperHint {
                lane: None,
                position: WorkflowPosition {
                    phase: 1,
                    item: value.transition_nonce,
                },
                action: "advance-failure-interval-consensus",
            });
        Ok(Some(projection))
    } else if tag_version(
        data,
        registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG,
        registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION,
    ) {
        let bytes: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES] = data
            .try_into()
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let value = FailureIntervalConsensusReplayAccountV1::decode(bytes)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let mut projection = CanonicalAccountProjection::canonical(
            CanonicalFamily::Failure,
            CanonicalAccountKind::FailureIntervalConsensusReplay,
        );
        projection.generation = Some(value.generation);
        projection.primary_binding = Some(value.interval_binding_id);
        projection.secondary_binding = Some(value.failure_policy_binding_id);
        Ok(Some(projection))
    } else {
        Ok(None)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForkNode {
    pub slot: u64,
    pub parent_slot: u64,
    pub blockhash: String,
    pub previous_blockhash: String,
    pub receive_sequence: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ForkLedger {
    nodes: BTreeMap<String, ForkNode>,
    hashes_by_slot: BTreeMap<u64, BTreeSet<String>>,
    frozen_slots: BTreeSet<u64>,
    dead_slots: BTreeSet<u64>,
    finalized_root: Option<(u64, String)>,
}

impl ForkLedger {
    pub fn observe(&mut self, slot: ObservedSlot, expected_cluster: &str) -> Result<()> {
        if slot.cluster_key != expected_cluster
            || slot.commitment != RpcCommitment::Processed
            || slot.slot == 0
            || slot.parent_slot >= slot.slot
            || slot.blockhash == slot.previous_blockhash
            || self.dead_slots.contains(&slot.slot)
        {
            return Err(AccountIndexError::InvalidFork);
        }
        if let Some(existing) = self.nodes.get(&slot.blockhash) {
            if existing.slot != slot.slot
                || existing.parent_slot != slot.parent_slot
                || existing.previous_blockhash != slot.previous_blockhash
            {
                return Err(AccountIndexError::InvalidFork);
            }
            return Ok(());
        }
        if self
            .nodes
            .get(&slot.previous_blockhash)
            .is_some_and(|parent| parent.slot != slot.parent_slot)
            || self.nodes.values().any(|child| {
                child.previous_blockhash == slot.blockhash && child.parent_slot != slot.slot
            })
        {
            return Err(AccountIndexError::InvalidFork);
        }
        self.hashes_by_slot
            .entry(slot.slot)
            .or_default()
            .insert(slot.blockhash.clone());
        self.nodes.insert(
            slot.blockhash.clone(),
            ForkNode {
                slot: slot.slot,
                parent_slot: slot.parent_slot,
                blockhash: slot.blockhash,
                previous_blockhash: slot.previous_blockhash,
                receive_sequence: slot.receive_sequence,
            },
        );
        Ok(())
    }

    pub fn observe_update(
        &mut self,
        update: ObservedSlotUpdate,
        expected_cluster: &str,
    ) -> Result<()> {
        if update.cluster_key != expected_cluster
            || update.slot == 0
            || update
                .parent_slot
                .is_some_and(|parent| parent >= update.slot)
        {
            return Err(AccountIndexError::InvalidFork);
        }
        match update.kind {
            ObservedSlotUpdateKind::Frozen => {
                if self.dead_slots.contains(&update.slot) {
                    return Err(AccountIndexError::InvalidFork);
                }
                self.frozen_slots.insert(update.slot);
            }
            ObservedSlotUpdateKind::Dead => {
                let conflicts_with_root = self.finalized_root.as_ref().is_some_and(|(_, root)| {
                    self.hashes_by_slot.get(&update.slot).is_some_and(|hashes| {
                        hashes.iter().any(|hash| self.is_ancestor(hash, root))
                    })
                });
                if conflicts_with_root {
                    return Err(AccountIndexError::InvalidFork);
                }
                self.frozen_slots.remove(&update.slot);
                self.dead_slots.insert(update.slot);
            }
            ObservedSlotUpdateKind::FirstShred
            | ObservedSlotUpdateKind::Completed
            | ObservedSlotUpdateKind::CreatedBank
            | ObservedSlotUpdateKind::OptimisticConfirmation
            | ObservedSlotUpdateKind::Root => {}
        }
        Ok(())
    }

    pub fn unique_hash_at(&self, slot: u64) -> Result<&str> {
        let hashes = self
            .hashes_by_slot
            .get(&slot)
            .ok_or(AccountIndexError::UnknownFork)?;
        if hashes.len() != 1 {
            return Err(AccountIndexError::AmbiguousFork);
        }
        hashes
            .first()
            .map(String::as_str)
            .ok_or(AccountIndexError::UnknownFork)
    }

    pub fn finalize_root(&mut self, slot: u64) -> Result<()> {
        if self
            .finalized_root
            .as_ref()
            .is_some_and(|(root, _)| slot < *root)
        {
            return Err(AccountIndexError::RootRegression);
        }
        let hash = self.unique_hash_at(slot)?.to_string();
        if self
            .finalized_root
            .as_ref()
            .is_some_and(|(_, prior)| !self.is_ancestor(prior, &hash))
        {
            return Err(AccountIndexError::InvalidFork);
        }
        if self.branch_contains_dead_slot(&hash) {
            return Err(AccountIndexError::InvalidFork);
        }
        self.finalized_root = Some((slot, hash));
        Ok(())
    }

    #[must_use]
    pub fn finalized_root(&self) -> Option<(u64, &str)> {
        self.finalized_root
            .as_ref()
            .map(|(slot, hash)| (*slot, hash.as_str()))
    }

    #[must_use]
    pub fn nodes(&self) -> Vec<&ForkNode> {
        self.nodes.values().collect()
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn contains_hash(&self, blockhash: &str) -> bool {
        self.nodes.contains_key(blockhash)
    }

    #[must_use]
    pub fn tracked_status_slot_count(&self) -> usize {
        self.frozen_slots.len() + self.dead_slots.len()
    }

    #[must_use]
    pub fn frozen_slots(&self) -> Vec<u64> {
        self.frozen_slots.iter().copied().collect()
    }

    #[must_use]
    pub fn is_frozen(&self, slot: u64) -> bool {
        self.frozen_slots.contains(&slot)
    }

    #[must_use]
    pub fn is_dead(&self, slot: u64) -> bool {
        self.dead_slots.contains(&slot)
    }

    #[must_use]
    pub fn slot_is_on_dead_branch(&self, slot: u64) -> bool {
        self.hashes_by_slot.get(&slot).is_some_and(|hashes| {
            hashes
                .iter()
                .any(|hash| self.branch_contains_dead_slot(hash))
        })
    }

    #[must_use]
    pub fn dead_slots(&self) -> Vec<u64> {
        self.dead_slots.iter().copied().collect()
    }

    fn is_ancestor(&self, ancestor: &str, descendant: &str) -> bool {
        let mut cursor = descendant;
        let mut remaining = self.nodes.len();
        while remaining > 0 {
            if cursor == ancestor {
                return true;
            }
            let Some(node) = self.nodes.get(cursor) else {
                return false;
            };
            cursor = &node.previous_blockhash;
            remaining -= 1;
        }
        false
    }

    fn branch_contains_dead_slot(&self, descendant: &str) -> bool {
        let mut cursor = descendant;
        let mut remaining = self.nodes.len();
        while remaining > 0 {
            let Some(node) = self.nodes.get(cursor) else {
                return false;
            };
            if self.dead_slots.contains(&node.slot) {
                return true;
            }
            cursor = &node.previous_blockhash;
            remaining -= 1;
        }
        false
    }

    fn processed_tip(&self) -> Option<&str> {
        self.nodes
            .values()
            .filter(|node| match self.finalized_root.as_ref() {
                Some((_, root)) => {
                    self.is_ancestor(root, &node.blockhash)
                        && !self.branch_contains_dead_slot(&node.blockhash)
                        && self.frozen_slots.contains(&node.slot)
                }
                None => {
                    !self.branch_contains_dead_slot(&node.blockhash)
                        && self.frozen_slots.contains(&node.slot)
                }
            })
            .max_by(|left, right| {
                (left.slot, left.receive_sequence, &left.blockhash).cmp(&(
                    right.slot,
                    right.receive_sequence,
                    &right.blockhash,
                ))
            })
            .map(|node| node.blockhash.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexedBranch {
    FinalizedScan,
    Processed { blockhash: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedAccountVersion {
    pub account: ObservedRpcAccount,
    pub projection: CanonicalAccountProjection,
    pub data_sha256: [u8; 32],
    pub branch: IndexedBranch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedAccountAbsence {
    pub release_key: String,
    pub slot: u64,
    pub receive_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessedAccountRemoval {
    pub address: Address,
    pub release_key: String,
    pub observed_owner: Address,
    pub observed_lamports: u64,
    pub observed_data_bytes: usize,
    pub kind: RpcAccountRemovalKind,
    pub slot: u64,
    pub receive_sequence: u64,
    pub blockhash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexCapacity {
    pub maximum_addresses: usize,
    pub maximum_versions_per_address: usize,
    pub maximum_fork_nodes: usize,
}

#[derive(Clone)]
pub struct CanonicalAccountIndex {
    plan: RpcIndexPlan,
    context: CanonicalDecoderContext,
    capacity: IndexCapacity,
    forks: ForkLedger,
    versions: BTreeMap<Address, Vec<IndexedAccountVersion>>,
    processed_removals: BTreeMap<Address, Vec<ProcessedAccountRemoval>>,
    finalized_absences: BTreeMap<Address, FinalizedAccountAbsence>,
    product_artifacts: ProductArtifactExpectations,
}

fn insert_product_artifact_expectation(
    expectations: &mut ProductArtifactExpectations,
    release_key: &str,
    program_id: Address,
    kind: ArtifactKind,
    digest: [u8; 32],
    bundle_id: [u8; 32],
) -> Result<Address> {
    if kind.registration_status() != ArtifactRegistrationStatus::Current || digest == [0; 32] {
        return Err(AccountIndexError::CanonicalDecodeRefused);
    }
    let kind_seed = [kind.byte()];
    let address = Address::find_program_address(
        &[PRODUCT_ARTIFACT_PDA_PREFIX_V1, &kind_seed, &digest],
        &program_id,
    )
    .0;
    let key = (release_key.to_string(), address);
    match expectations.get_mut(&key) {
        None => {
            expectations.insert(
                key,
                ProductArtifactExpectation {
                    kind,
                    digest,
                    unique_bundle_id: bundle_id,
                },
            );
        }
        Some(existing) => {
            if existing.kind != kind || existing.digest != digest {
                return Err(AccountIndexError::AmbiguousCodec);
            }
            if existing.unique_bundle_id != bundle_id {
                existing.unique_bundle_id = [0; 32];
            }
        }
    }
    Ok(address)
}

fn product_artifact_expectations_for_scan(
    plan: &RpcIndexPlan,
    release_key: &str,
    accounts: &[ObservedRpcAccount],
) -> Result<ProductArtifactExpectations> {
    let release = plan
        .releases
        .iter()
        .find(|release| release.key() == release_key)
        .ok_or(AccountIndexError::UnknownRelease)?;
    let mut by_address = BTreeMap::new();
    for account in accounts {
        if account.provenance.release_key != release_key
            || account.provenance.cluster_key != plan.cluster.key()
            || account.owner != release.program_id
            || account.executable
        {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        if by_address.insert(account.address, account).is_some() {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
    }

    let mut expectations = BTreeMap::new();
    for account in accounts {
        if !tag_version(
            &account.data,
            registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG,
            registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2,
        ) {
            continue;
        }
        let registration = SeriesRegistryAccountV2::decode(&account.data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let (registry_address, registry_bump) = Address::find_program_address(
            &[
                SERIES_REGISTRY_PDA_PREFIX_V1,
                &registration.series_plan_id.bytes(),
            ],
            &release.program_id,
        );
        if registry_address != account.address || registry_bump != registration.stored_bump {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }

        let bundle_id = registration.compiler_bundle_id.bytes();
        let bundle_address = insert_product_artifact_expectation(
            &mut expectations,
            release_key,
            release.program_id,
            ArtifactKind::CompiledProductSeriesBundleV5,
            bundle_id,
            bundle_id,
        )?;
        let bundle_account = by_address
            .get(&bundle_address)
            .ok_or(AccountIndexError::CanonicalDecodeRefused)?;
        let bundle = CompiledProductSeriesBundleV5::decode(&bundle_account.data)
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?;
        let decoded_bundle_id = bundle
            .id()
            .map_err(|_| AccountIndexError::CanonicalDecodeRefused)?
            .bytes();
        if decoded_bundle_id != bundle_id
            || bundle.registry_release_id.bytes() != registration.registry_release_id.bytes()
            || bundle.capability_profile_id.bytes()
                != registration.capability_profile_id.bytes()
            || bundle.series_plan_id.bytes() != registration.series_plan_id.bytes()
            || bundle.funding_terms_id.bytes() != registration.funding_terms_id.bytes()
        {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }

        for (kind, digest) in [
            (
                ArtifactKind::RegistryProgramReleaseV2,
                bundle.registry_release_id.bytes(),
            ),
            (
                ArtifactKind::RegistryCapabilityProfileV4,
                bundle.capability_profile_id.bytes(),
            ),
            (
                ArtifactKind::SourceReleaseManifestV2,
                bundle.source_release_manifest_id.bytes(),
            ),
            (
                ArtifactKind::NativeClaimBasisV1,
                bundle.native_claim_basis_id.bytes(),
            ),
            (
                ArtifactKind::EvidenceOnlyRecoveryPolicyV1,
                bundle.evidence_only_recovery_policy_id.bytes(),
            ),
            (
                ArtifactKind::ProductTemplateV4,
                bundle.product_template_id.bytes(),
            ),
            (
                ArtifactKind::PriceMeasurePolicyV1,
                bundle.price_measure_policy_id.bytes(),
            ),
            (
                ArtifactKind::MarketGenesisProfileV2,
                bundle.market_genesis_profile_id.bytes(),
            ),
            (
                ArtifactKind::SeriesFundingQuoteV4,
                bundle.funding_quote_id.bytes(),
            ),
            (
                ArtifactKind::SeriesAttachmentPlanV4,
                bundle.attachment_plan_id.bytes(),
            ),
            (ArtifactKind::SeriesPlanV5, bundle.series_plan_id.bytes()),
            (
                ArtifactKind::SeriesFundingTermsV2,
                bundle.funding_terms_id.bytes(),
            ),
        ] {
            let artifact_address = insert_product_artifact_expectation(
                &mut expectations,
                release_key,
                release.program_id,
                kind,
                digest,
                bundle_id,
            )?;
            if !by_address.contains_key(&artifact_address) {
                return Err(AccountIndexError::CanonicalDecodeRefused);
            }
        }
    }
    Ok(expectations)
}

impl CanonicalAccountIndex {
    pub fn new(
        plan: RpcIndexPlan,
        context: CanonicalDecoderContext,
        capacity: IndexCapacity,
    ) -> Result<Self> {
        plan.validate()
            .map_err(|_| AccountIndexError::UnknownRelease)?;
        if capacity.maximum_addresses == 0
            || capacity.maximum_versions_per_address == 0
            || capacity.maximum_fork_nodes == 0
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        Ok(Self {
            plan,
            context,
            capacity,
            forks: ForkLedger::default(),
            versions: BTreeMap::new(),
            processed_removals: BTreeMap::new(),
            finalized_absences: BTreeMap::new(),
            product_artifacts: BTreeMap::new(),
        })
    }

    /// Rebuild the derived Product-artifact authority cache from one complete
    /// finalized program scan.  RegistryV2 and BundleV5 must both be present;
    /// a partial graph is refused rather than carried forward from an older
    /// scan.
    pub fn prepare_complete_finalized_scan(
        &mut self,
        release_key: &str,
        accounts: &[ObservedRpcAccount],
    ) -> Result<()> {
        let next = product_artifact_expectations_for_scan(&self.plan, release_key, accounts)?;
        self.product_artifacts
            .retain(|(known_release, _), _| known_release != release_key);
        self.product_artifacts.extend(next);
        Ok(())
    }

    pub fn observe_slot(&mut self, slot: ObservedSlot) -> Result<()> {
        if !self.forks.contains_hash(&slot.blockhash)
            && self.forks.node_count() >= self.capacity.maximum_fork_nodes
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        self.forks.observe(slot, &self.plan.cluster.key())
    }

    pub fn observe_slot_update(&mut self, update: ObservedSlotUpdate) -> Result<()> {
        if matches!(
            update.kind,
            ObservedSlotUpdateKind::Frozen | ObservedSlotUpdateKind::Dead
        ) && !self.forks.is_frozen(update.slot)
            && !self.forks.is_dead(update.slot)
            && self.forks.tracked_status_slot_count() >= self.capacity.maximum_fork_nodes
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        self.forks.observe_update(update, &self.plan.cluster.key())
    }

    pub fn finalize_root(&mut self, slot: u64) -> Result<()> {
        self.forks.finalize_root(slot)
    }

    /// Invalidate every non-final subscription observation after a transport
    /// disconnect. Finalized scans remain available, while fork topology and
    /// processed versions are rebuilt only from a complete new subscription
    /// generation. Rooted subscription rows are deliberately dropped too:
    /// the next finalized scan, not a disconnected feed, owns their promotion.
    pub fn rollback_processed_transport(&mut self) -> usize {
        let mut removed = 0_usize;
        self.versions.retain(|_, versions| {
            let before = versions.len();
            versions.retain(|version| matches!(&version.branch, IndexedBranch::FinalizedScan));
            removed = removed.saturating_add(before.saturating_sub(versions.len()));
            !versions.is_empty()
        });
        removed = removed.saturating_add(
            self.processed_removals
                .values()
                .map(Vec::len)
                .sum::<usize>(),
        );
        self.processed_removals.clear();
        self.forks = ForkLedger::default();
        removed
    }

    /// Remove already-indexed rows on every branch whose ancestry now contains
    /// a dead slot. Finalized-scan baselines are never removed by a processed
    /// rollback observation.
    pub fn rollback_dead_processed_versions(&mut self) -> usize {
        let forks = &self.forks;
        let mut removed = 0_usize;
        self.versions.retain(|_, versions| {
            let before = versions.len();
            versions.retain(|version| match &version.branch {
                IndexedBranch::FinalizedScan => true,
                IndexedBranch::Processed { blockhash } => {
                    !forks.branch_contains_dead_slot(blockhash)
                }
            });
            removed = removed.saturating_add(before.saturating_sub(versions.len()));
            !versions.is_empty()
        });
        self.processed_removals.retain(|_, removals| {
            let before = removals.len();
            removals.retain(|removal| !forks.branch_contains_dead_slot(&removal.blockhash));
            removed = removed.saturating_add(before.saturating_sub(removals.len()));
            !removals.is_empty()
        });
        removed
    }

    fn reserve_observation_slot(&mut self, address: Address, finalized: bool) -> Result<()> {
        let account_count = self.versions.get(&address).map_or(0, Vec::len);
        let removal_count = self.processed_removals.get(&address).map_or(0, Vec::len);
        if account_count.saturating_add(removal_count) < self.capacity.maximum_versions_per_address
        {
            return Ok(());
        }
        let oldest_account = self.versions.get(&address).and_then(|versions| {
            versions
                .iter()
                .enumerate()
                .filter(|(_, version)| matches!(&version.branch, IndexedBranch::Processed { .. }))
                .min_by_key(|(_, version)| {
                    (
                        version.account.provenance.slot,
                        version.account.provenance.receive_sequence,
                    )
                })
                .map(|(index, version)| {
                    (
                        index,
                        version.account.provenance.slot,
                        version.account.provenance.receive_sequence,
                    )
                })
        });
        let oldest_removal = self.processed_removals.get(&address).and_then(|removals| {
            removals
                .iter()
                .enumerate()
                .min_by_key(|(_, removal)| (removal.slot, removal.receive_sequence))
                .map(|(index, removal)| (index, removal.slot, removal.receive_sequence))
        });
        match (oldest_account, oldest_removal) {
            (Some((index, account_slot, account_sequence)), Some((_, slot, sequence)))
                if (account_slot, account_sequence) <= (slot, sequence) =>
            {
                self.versions
                    .get_mut(&address)
                    .ok_or(AccountIndexError::CapacityExceeded)?
                    .remove(index);
            }
            (_, Some((index, _, _))) => {
                let removals = self
                    .processed_removals
                    .get_mut(&address)
                    .ok_or(AccountIndexError::CapacityExceeded)?;
                removals.remove(index);
                let empty = removals.is_empty();
                if empty {
                    self.processed_removals.remove(&address);
                }
            }
            (Some((index, _, _)), None) => {
                self.versions
                    .get_mut(&address)
                    .ok_or(AccountIndexError::CapacityExceeded)?
                    .remove(index);
            }
            (None, None) if finalized => {
                self.versions
                    .get_mut(&address)
                    .and_then(|versions| (!versions.is_empty()).then(|| versions.remove(0)))
                    .ok_or(AccountIndexError::CapacityExceeded)?;
            }
            (None, None) => return Err(AccountIndexError::CapacityExceeded),
        }
        Ok(())
    }

    pub fn record_processed_removal(&mut self, removal: ObservedRpcAccountRemoval) -> Result<bool> {
        if removal.provenance.cluster_key != self.plan.cluster.key() {
            return Err(AccountIndexError::WrongCluster);
        }
        if removal.provenance.commitment != RpcCommitment::Processed
            || !matches!(
                &removal.provenance.source,
                crate::rpc_index::RpcObservationSource::ProcessedSubscription { .. }
            )
        {
            return Err(AccountIndexError::InvalidFork);
        }
        let release = self
            .release(&removal.provenance.release_key)
            .ok_or(AccountIndexError::UnknownRelease)?;
        if removal.address == Address::default()
            || removal.observed_executable
            || match removal.kind {
                RpcAccountRemovalKind::Closed => {
                    removal.observed_lamports != 0 || removal.observed_data_bytes != 0
                }
                RpcAccountRemovalKind::OwnerChanged => removal.observed_owner == release.program_id,
            }
        {
            return Err(AccountIndexError::CanonicalDecodeRefused);
        }
        let known_release_address = self.versions.get(&removal.address).is_some_and(|versions| {
            versions.iter().any(|version| {
                version.account.provenance.release_key == removal.provenance.release_key
            })
        });
        if !known_release_address {
            return Ok(false);
        }
        let blockhash = self
            .forks
            .unique_hash_at(removal.provenance.slot)?
            .to_string();
        self.reserve_observation_slot(removal.address, false)?;
        self.processed_removals
            .entry(removal.address)
            .or_default()
            .push(ProcessedAccountRemoval {
                address: removal.address,
                release_key: removal.provenance.release_key,
                observed_owner: removal.observed_owner,
                observed_lamports: removal.observed_lamports,
                observed_data_bytes: removal.observed_data_bytes,
                kind: removal.kind,
                slot: removal.provenance.slot,
                receive_sequence: removal.provenance.receive_sequence,
                blockhash,
            });
        Ok(true)
    }

    pub fn ingest(&mut self, account: ObservedRpcAccount) -> Result<()> {
        let branch = match account.provenance.commitment {
            RpcCommitment::Finalized => IndexedBranch::FinalizedScan,
            RpcCommitment::Processed => IndexedBranch::Processed {
                blockhash: self
                    .forks
                    .unique_hash_at(account.provenance.slot)?
                    .to_string(),
            },
        };
        let projection = CanonicalAccountDecoderRegistry::with_product_artifacts(
            &self.plan,
            self.context,
            &self.product_artifacts,
        )
        .decode(&account)?;
        let data_sha256 = Sha256::digest(&account.data).into();
        if !self.versions.contains_key(&account.address)
            && self.versions.len() >= self.capacity.maximum_addresses
        {
            return Err(AccountIndexError::CapacityExceeded);
        }
        if self
            .versions
            .get(&account.address)
            .and_then(|versions| versions.last())
            .is_some_and(|previous| {
                previous.account.provenance.receive_sequence >= account.provenance.receive_sequence
                    && previous.account.provenance.slot >= account.provenance.slot
            })
        {
            return Err(AccountIndexError::StaleObservation);
        }
        if account.provenance.commitment == RpcCommitment::Finalized {
            let release_key = account.provenance.release_key.as_str();
            if let Some(versions) = self.versions.get_mut(&account.address) {
                versions.retain(|version| {
                    !matches!(&version.branch, IndexedBranch::FinalizedScan)
                        || version.account.provenance.release_key != release_key
                });
            }
        }
        self.reserve_observation_slot(
            account.address,
            account.provenance.commitment == RpcCommitment::Finalized,
        )?;
        let versions = self.versions.entry(account.address).or_default();
        versions.push(IndexedAccountVersion {
            account,
            projection,
            data_sha256,
            branch,
        });
        Ok(())
    }

    pub fn reconcile_finalized_scan(
        &mut self,
        release_key: &str,
        slot: u64,
        receive_sequence: u64,
        seen: &BTreeSet<Address>,
    ) -> Result<()> {
        if self.release(release_key).is_none() {
            return Err(AccountIndexError::UnknownRelease);
        }
        for (address, versions) in &self.versions {
            let belongs_to_release = versions
                .iter()
                .any(|version| version.account.provenance.release_key == release_key);
            if belongs_to_release && !seen.contains(address) {
                let absence = FinalizedAccountAbsence {
                    release_key: release_key.to_string(),
                    slot,
                    receive_sequence,
                };
                let should_replace = self.finalized_absences.get(address).is_none_or(|prior| {
                    (prior.slot, prior.receive_sequence) < (absence.slot, absence.receive_sequence)
                });
                if should_replace {
                    self.finalized_absences.insert(*address, absence);
                }
            }
        }
        Ok(())
    }

    pub fn current(
        &self,
        address: Address,
        commitment: RpcCommitment,
    ) -> Option<&IndexedAccountVersion> {
        let versions = self.versions.get(&address)?;
        let selected = match commitment {
            RpcCommitment::Processed => {
                let tip = self.forks.processed_tip();
                versions
                    .iter()
                    .filter(|version| match (&version.branch, tip) {
                        (IndexedBranch::FinalizedScan, _) => true,
                        (IndexedBranch::Processed { blockhash }, Some(tip)) => {
                            self.forks.is_ancestor(blockhash, tip)
                        }
                        _ => false,
                    })
                    .filter(|version| {
                        let version_key = (
                            version.account.provenance.slot,
                            version.account.provenance.receive_sequence,
                        );
                        self.processed_removals
                            .get(&address)
                            .and_then(|removals| {
                                removals
                                    .iter()
                                    .filter(|removal| {
                                        removal.release_key
                                            == version.account.provenance.release_key
                                            && tip.is_some_and(|tip| {
                                                self.forks.is_ancestor(&removal.blockhash, tip)
                                            })
                                    })
                                    .max_by_key(|removal| (removal.slot, removal.receive_sequence))
                            })
                            .is_none_or(|removal| {
                                (removal.slot, removal.receive_sequence) < version_key
                            })
                    })
                    .max_by_key(|version| {
                        (
                            version.account.provenance.slot,
                            version.account.provenance.receive_sequence,
                        )
                    })
            }
            RpcCommitment::Finalized => versions
                .iter()
                .filter(|version| matches!(&version.branch, IndexedBranch::FinalizedScan))
                .max_by_key(|version| {
                    (
                        version.account.provenance.slot,
                        version.account.provenance.receive_sequence,
                    )
                }),
        };
        selected.filter(|version| {
            self.finalized_absences.get(&address).is_none_or(|absence| {
                absence.release_key != version.account.provenance.release_key
                    || version.account.provenance.slot > absence.slot
            })
        })
    }

    pub fn current_accounts(&self, commitment: RpcCommitment) -> Vec<&IndexedAccountVersion> {
        self.versions
            .keys()
            .filter_map(|address| self.current(*address, commitment))
            .collect()
    }

    #[must_use]
    pub fn finalized_absence(&self, address: Address) -> Option<&FinalizedAccountAbsence> {
        self.finalized_absences.get(&address)
    }

    #[must_use]
    pub const fn forks(&self) -> &ForkLedger {
        &self.forks
    }

    #[must_use]
    pub fn release(&self, key: &str) -> Option<&IndexedProgramRelease> {
        self.plan
            .releases
            .iter()
            .find(|release| release.key() == key)
    }

    #[must_use]
    pub fn releases(&self) -> &[IndexedProgramRelease] {
        &self.plan.releases
    }

    #[must_use]
    pub const fn acquisition_plan(&self) -> &RpcIndexPlan {
        &self.plan
    }

    #[must_use]
    pub fn cluster_key(&self) -> String {
        self.plan.cluster.key()
    }
}

#[cfg(test)]
mod current_decoder_tests {
    use super::*;

    #[test]
    fn product_artifact_final_typing_refuses_withdrawn_and_hostile_bodies() {
        assert_eq!(
            canonical_artifact_kind(ArtifactKind::CompiledProductSeriesBundleV4),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );
        assert_eq!(
            require_typed_product_artifact(
                ArtifactKind::CompiledProductSeriesBundleV5,
                [0x51; 32],
                &[0; 8],
            ),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );
        assert_eq!(
            require_typed_product_artifact(
                ArtifactKind::NativeClaimBasisV1,
                [0x52; 32],
                &[0; 8],
            ),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );
    }

    #[test]
    fn shared_artifact_cache_drops_unique_bundle_claim() {
        let program_id = Address::new_from_array([0x31; 32]);
        let mut expectations = BTreeMap::new();
        let address = insert_product_artifact_expectation(
            &mut expectations,
            "release",
            program_id,
            ArtifactKind::SourceReleaseManifestV2,
            [0x32; 32],
            [0x33; 32],
        )
        .unwrap();
        insert_product_artifact_expectation(
            &mut expectations,
            "release",
            program_id,
            ArtifactKind::SourceReleaseManifestV2,
            [0x32; 32],
            [0x34; 32],
        )
        .unwrap();
        assert_eq!(
            expectations
                .get(&(String::from("release"), address))
                .unwrap()
                .unique_bundle_id,
            [0; 32]
        );
    }

    #[test]
    fn withdrawn_versions_do_not_enter_the_live_decoder_set() {
        let general_versions = [
            (WINDOW_ACCOUNT_TAG, 4, WINDOW_ACCOUNT_VERSION_V2),
            (
                ADMISSION_NODE_ACCOUNT_TAG,
                1,
                ADMISSION_NODE_ACCOUNT_VERSION_V2,
            ),
            (
                MARKET_BINDING_ACCOUNT_TAG,
                1,
                MARKET_BINDING_ACCOUNT_VERSION_V2,
            ),
            (CLEAR_WORK_ACCOUNT_TAG, 2, CLEAR_WORK_ACCOUNT_VERSION_V3),
            (
                OWNER_SETTLEMENT_ACCOUNT_TAG,
                4,
                OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
            ),
            (
                registry::GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_TAG,
                4,
                registry::GENERAL_SETTLEMENT_RECEIPT_V5_ACCOUNT_VERSION,
            ),
            (
                registry::GENERAL_RESERVATION_V9_ACCOUNT_TAG,
                7,
                registry::GENERAL_RESERVATION_V9_ACCOUNT_VERSION,
            ),
            (
                registry::GENERAL_ORDER_PAGE_V5_ACCOUNT_TAG,
                4,
                registry::GENERAL_ORDER_PAGE_V5_ACCOUNT_VERSION,
            ),
        ];
        for (tag, withdrawn, current) in general_versions {
            assert_eq!(decode_general(&[tag, withdrawn]), Ok(None));
            assert_eq!(
                decode_general(&[tag, current]),
                Err(AccountIndexError::CanonicalDecodeRefused)
            );
        }
        assert_eq!(
            decode_general(&[SETTLEMENT_ROOT_ACCOUNT_TAG, SETTLEMENT_ROOT_ACCOUNT_VERSION]),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );

        let context = CanonicalDecoderContext {
            source_neutral_sink: RuntimeKey::from_bytes([0x41; 32]),
        };
        assert_eq!(
            decode_source(&[SOURCE_RELEASE_ACCOUNT_TAG, 1], context),
            Ok(None)
        );
        assert_eq!(
            decode_source(
                &[SOURCE_RELEASE_ACCOUNT_TAG, SOURCE_RELEASE_ACCOUNT_VERSION],
                context
            ),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );

        for (tag, withdrawn, current) in [
            (
                FRACTIONAL_POLICY_ACCOUNT_TAG,
                registry::FRACTIONAL_REDEMPTION_POLICY_ACCOUNT_V1_VERSION,
                FRACTIONAL_POLICY_ACCOUNT_VERSION,
            ),
            (
                FRACTIONAL_CREDIT_ACCOUNT_TAG,
                registry::FRACTIONAL_REDEMPTION_CREDIT_ACCOUNT_V1_VERSION,
                FRACTIONAL_CREDIT_ACCOUNT_VERSION,
            ),
            (
                FRACTIONAL_CREDIT_TOMBSTONE_TAG,
                registry::FRACTIONAL_REDEMPTION_CREDIT_TOMBSTONE_ACCOUNT_V1_VERSION,
                FRACTIONAL_CREDIT_TOMBSTONE_VERSION,
            ),
        ] {
            assert_eq!(decode_fractional(&[tag, withdrawn]), Ok(None));
            assert_eq!(
                decode_fractional(&[tag, current]),
                Err(AccountIndexError::CanonicalDecodeRefused)
            );
        }
        assert_eq!(
            decode_fractional(&[
                FRACTIONAL_LEDGER_ACCOUNT_TAG,
                FRACTIONAL_LEDGER_ACCOUNT_VERSION,
            ]),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );

        assert_eq!(
            decode_product(&[
                registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_TAG,
                registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_VERSION,
            ]),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );
        assert_eq!(
            decode_series(&[
                registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_TAG,
                registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_VERSION,
            ]),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );

        for (tag, version) in [
            (
                OWNER_FEE_CARRY_ACCOUNT_TAG,
                OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
            ),
            (
                OWNER_FEE_CARRY_ACCOUNT_TAG,
                OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4,
            ),
            (
                PAYER_ALLOCATION_ACCOUNT_TAG,
                PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
            ),
            (
                RECIPIENT_ALLOCATION_ACCOUNT_TAG,
                RECIPIENT_ALLOCATION_ACCOUNT_VERSION_V2,
            ),
        ] {
            assert_eq!(
                decode_fee(&[tag, version]),
                Err(AccountIndexError::CanonicalDecodeRefused)
            );
        }

        assert_eq!(decode_dealer(b"DCDSTAT1"), Ok(None));
        assert_eq!(
            decode_dealer(&[
                registry::DEALER_POLICY_STAGE_ACCOUNT_TAG,
                registry::DEALER_POLICY_STAGE_ACCOUNT_VERSION,
            ]),
            Ok(None)
        );
        for (tag, version) in [
            (
                registry::DEALER_POLICY_ACCOUNT_TAG,
                registry::DEALER_POLICY_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG,
                registry::DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_STATE_V2_ACCOUNT_TAG,
                registry::DEALER_STATE_V2_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
                registry::DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_LP_PAGE_V2_ACCOUNT_TAG,
                registry::DEALER_LP_PAGE_V2_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_LEASE_V2_ACCOUNT_TAG,
                registry::DEALER_LEASE_V2_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_TAG,
                registry::DEALER_SETTLEMENT_POT_V2_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG,
                registry::DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_TAG,
                registry::DEALER_TERMINAL_ALLOCATION_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_CLAIM_WORK_ACCOUNT_TAG,
                registry::DEALER_CLAIM_WORK_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_TAG,
                registry::DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_EXIT_TICKET_ACCOUNT_TAG,
                registry::DEALER_EXIT_TICKET_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
                registry::DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
            ),
            (
                registry::DEALER_COVERED_SELECTION_ACCOUNT_TAG,
                registry::DEALER_COVERED_SELECTION_ACCOUNT_VERSION,
            ),
        ] {
            assert_eq!(
                decode_dealer(&[tag, version]),
                Err(AccountIndexError::CanonicalDecodeRefused)
            );
        }

        for (tag, version) in [
            (
                registry::FAILURE_EXTERNAL_ROOT_ACCOUNT_TAG,
                registry::FAILURE_MARKET_ROOT_ACCOUNT_VERSION_V2,
            ),
            (
                registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG,
                registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION,
            ),
            (
                registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG,
                registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION,
            ),
        ] {
            assert_eq!(
                decode_failure(&[tag, version]),
                Err(AccountIndexError::CanonicalDecodeRefused)
            );
        }
    }
}

#[cfg(test)]
mod collateral_decoder_tests {
    use super::*;
    use clutch_collateral_adapter_v2::{
        Id as CollateralId, MarketLiabilityLifecycleV1, ResolutionFinalizationFactsV5,
        ResolutionPayoutUnitBoundaryV5,
    };
    use clutch_retirement::{DeletableRentOwnerV1, Identity32V1, MAX_OUTCOMES};

    fn id(value: u8) -> CollateralId {
        CollateralId::from_bytes([value; 32])
    }

    fn rent() -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1::from_persisted(Identity32V1::new([90; 32]).unwrap(), 1, 0).unwrap()
    }

    fn hoard() -> HoardV2 {
        HoardV2 {
            market_instance_id: id(1),
            realm_id: id(2),
            profile_id: id(3),
            collateral_policy_id: id(4),
            collateral_release_id: id(5),
            authority: id(6),
            token_account: id(7),
            collateral_cap_atoms: 100,
            cash_liability_atoms: 8,
            locked_claim_principal_atoms: 9,
            lifecycle: MarketLiabilityLifecycleV1::Open,
            outcome_count: 2,
            stored_bump: 1,
            rent: rent(),
        }
    }

    #[test]
    fn canonical_collateral_bodies_index_by_full_market_identity() {
        let mut hoard_bytes = [0; HOARD_V2_BYTES];
        hoard().encode(&mut hoard_bytes).unwrap();
        let projection = decode_collateral(&hoard_bytes).unwrap().unwrap();
        assert_eq!(projection.kind, CanonicalAccountKind::CollateralHoardV2);
        assert_eq!(projection.primary_binding, Some([1; 32]));
        assert_eq!(projection.secondary_binding, Some([2; 32]));

        let claim_ledger = ClaimLedgerV3 {
            market_instance_id: id(1),
            realm_id: id(2),
            native_claim_basis_id: id(8),
            fractional_policy_id: CollateralId::ZERO,
            fractional_ledger_account: CollateralId::ZERO,
            resolution_account: CollateralId::ZERO,
            aggregate_internal_supply: [0; MAX_OUTCOMES],
            aggregate_materialized_supply: [0; MAX_OUTCOMES],
            next_fractional_sequence: 0,
            last_fractional_transition_id: CollateralId::ZERO,
            fractional_binding:
                clutch_collateral_adapter_v2::FractionalBindingStateV1::OpenUnlatched,
            lifecycle: MarketLiabilityLifecycleV1::Open,
            outcome_count: 2,
            stored_bump: 2,
            rent: rent(),
        };
        let mut claim_bytes = [0; CLAIM_LEDGER_V3_BYTES];
        claim_ledger.encode(&mut claim_bytes).unwrap();
        let projection = decode_collateral(&claim_bytes).unwrap().unwrap();
        assert_eq!(
            projection.kind,
            CanonicalAccountKind::CollateralClaimLedgerV3
        );
        assert_eq!(projection.primary_binding, Some([1; 32]));
        assert_eq!(projection.secondary_binding, Some([8; 32]));

        let mut weights = [0; MAX_OUTCOMES];
        weights[0] = 3;
        weights[1] = 2;
        let resolution = ResolutionV5::finalized(
            ResolutionFinalizationFactsV5 {
                market_instance_id: id(1),
                native_claim_basis_id: id(8),
                finalization_evidence_id: id(11),
                outcome_count: 2,
                payout_denominator: 5,
                payout_weights: weights,
                generation: 4,
                payout_unit_boundary: ResolutionPayoutUnitBoundaryV5::ExactWholeCollateralAtoms,
            },
            3,
            rent(),
        )
        .unwrap();
        let mut resolution_bytes = [0; RESOLUTION_V5_BYTES];
        resolution.encode(&mut resolution_bytes).unwrap();
        let projection = decode_collateral(&resolution_bytes).unwrap().unwrap();
        assert_eq!(
            projection.kind,
            CanonicalAccountKind::CollateralResolutionV5
        );
        assert_eq!(projection.generation, Some(4));
        assert_eq!(projection.primary_binding, Some([1; 32]));
        assert_eq!(projection.secondary_binding, Some([8; 32]));
    }

    #[test]
    fn malformed_canonical_tag_is_not_downgraded_to_unknown() {
        let mut bytes = [0; HOARD_V2_BYTES];
        hoard().encode(&mut bytes).unwrap();
        bytes[5] = 1;
        assert_eq!(
            decode_collateral(&bytes),
            Err(AccountIndexError::CanonicalDecodeRefused)
        );
    }
}

#[cfg(test)]
mod processed_fork_tests {
    use super::*;
    use crate::rpc_index::{
        RpcAcquisitionBounds, RpcClusterBinding, RpcObservationProvenance, RpcObservationSource,
    };

    const CLUSTER: &str = "test:11111111111111111111111111111111";

    fn slot(slot: u64, parent_slot: u64, blockhash: &str, previous: &str) -> ObservedSlot {
        ObservedSlot {
            cluster_key: CLUSTER.to_string(),
            slot,
            parent_slot,
            blockhash: blockhash.to_string(),
            previous_blockhash: previous.to_string(),
            commitment: RpcCommitment::Processed,
            receive_sequence: slot,
        }
    }

    #[test]
    fn later_root_must_descend_from_the_previous_root() {
        let mut ledger = ForkLedger::default();
        ledger
            .observe(slot(10, 9, "root-10", "unknown-9"), CLUSTER)
            .unwrap();
        ledger.finalize_root(10).unwrap();
        ledger
            .observe(slot(12, 11, "orphan-12", "orphan-11"), CLUSTER)
            .unwrap();
        assert_eq!(
            ledger.finalize_root(12),
            Err(AccountIndexError::InvalidFork)
        );
        let mut valid = ForkLedger::default();
        valid
            .observe(slot(10, 9, "root-10", "unknown-9"), CLUSTER)
            .unwrap();
        valid.finalize_root(10).unwrap();
        valid
            .observe(slot(11, 10, "child-11", "root-10"), CLUSTER)
            .unwrap();
        valid
            .observe(slot(12, 11, "child-12", "child-11"), CLUSTER)
            .unwrap();
        assert_eq!(valid.finalize_root(12), Ok(()));
    }

    #[test]
    fn dead_ancestor_marks_its_known_descendants_rollbackable() {
        let mut ledger = ForkLedger::default();
        ledger
            .observe(slot(20, 19, "branch-20", "unknown-19"), CLUSTER)
            .unwrap();
        ledger
            .observe(slot(21, 20, "branch-21", "branch-20"), CLUSTER)
            .unwrap();
        ledger
            .observe_update(
                ObservedSlotUpdate {
                    cluster_key: CLUSTER.to_string(),
                    slot: 20,
                    parent_slot: Some(19),
                    kind: ObservedSlotUpdateKind::Dead,
                    receive_sequence: 22,
                },
                CLUSTER,
            )
            .unwrap();
        assert!(ledger.slot_is_on_dead_branch(20));
        assert!(ledger.slot_is_on_dead_branch(21));
    }

    #[test]
    fn fork_bound_removal_masks_only_processed_and_reverts_with_dead_branch() {
        let release = IndexedProgramRelease {
            program_id: Address::new_from_array([0x31; 32]),
            program_data: Address::new_from_array([0x32; 32]),
            elf_sha256: [0x33; 32],
            deployment_slot: 1,
            release_manifest_sha256: [0x34; 32],
            capability_profile_id: [0x35; 32],
            source_commit: "36".repeat(20),
            source_profile: crate::rpc_index::CompiledSourceProfile::ProductionInert,
            wire_surface: crate::rpc_index::ManifestWireSurfaceV1 {
                identity_sha256: [0x37; 32],
                legacy_intent_pairs: vec![],
                dedicated_direct_intent_pairs: vec![],
                outer_request_actions: vec![],
                source_generation_discriminants: vec![],
            },
            enabled_intents: vec![],
            families: vec![CanonicalFamily::General],
        };
        let release_key = release.key();
        let plan = RpcIndexPlan {
            cluster: RpcClusterBinding {
                cluster_name: "test".to_string(),
                genesis_hash: "11111111111111111111111111111111".to_string(),
                rpc_http_url: "http://127.0.0.1:8899".to_string(),
                rpc_websocket_url: "ws://127.0.0.1:8900".to_string(),
            },
            releases: vec![release.clone()],
            bounds: RpcAcquisitionBounds {
                maximum_accounts_per_scan: 8,
                maximum_account_data_bytes: 1024,
                maximum_total_response_bytes: 8192,
                maximum_subscriptions: 8,
            },
        };
        let mut index = CanonicalAccountIndex::new(
            plan,
            CanonicalDecoderContext {
                source_neutral_sink: RuntimeKey::from_bytes([0x55; 32]),
            },
            IndexCapacity {
                maximum_addresses: 8,
                maximum_versions_per_address: 4,
                maximum_fork_nodes: 8,
            },
        )
        .unwrap();
        let address = Address::new_from_array([0x41; 32]);
        index.versions.insert(
            address,
            vec![IndexedAccountVersion {
                account: ObservedRpcAccount {
                    address,
                    owner: release.program_id,
                    lamports: 1,
                    executable: false,
                    rent_epoch: 0,
                    data: vec![1],
                    provenance: RpcObservationProvenance {
                        cluster_key: CLUSTER.to_string(),
                        release_key: release_key.clone(),
                        slot: 10,
                        commitment: RpcCommitment::Finalized,
                        source: RpcObservationSource::FinalizedScan,
                        receive_sequence: 1,
                    },
                },
                projection: CanonicalAccountProjection::canonical(
                    CanonicalFamily::General,
                    CanonicalAccountKind::GeneralMarketRuntime,
                ),
                data_sha256: [0; 32],
                branch: IndexedBranch::FinalizedScan,
            }],
        );
        index
            .observe_slot(slot(11, 10, "branch-11", "unknown-10"))
            .unwrap();
        index
            .observe_slot_update(ObservedSlotUpdate {
                cluster_key: CLUSTER.to_string(),
                slot: 11,
                parent_slot: Some(10),
                kind: ObservedSlotUpdateKind::Frozen,
                receive_sequence: 2,
            })
            .unwrap();
        assert!(index
            .record_processed_removal(ObservedRpcAccountRemoval {
                address,
                observed_owner: Address::default(),
                observed_lamports: 0,
                observed_executable: false,
                observed_data_bytes: 0,
                kind: RpcAccountRemovalKind::Closed,
                provenance: RpcObservationProvenance {
                    cluster_key: CLUSTER.to_string(),
                    release_key,
                    slot: 11,
                    commitment: RpcCommitment::Processed,
                    source: RpcObservationSource::ProcessedSubscription { subscription_id: 7 },
                    receive_sequence: 3,
                },
            })
            .unwrap());
        assert!(index.current(address, RpcCommitment::Processed).is_none());
        assert!(index.current(address, RpcCommitment::Finalized).is_some());
        index
            .observe_slot_update(ObservedSlotUpdate {
                cluster_key: CLUSTER.to_string(),
                slot: 11,
                parent_slot: Some(10),
                kind: ObservedSlotUpdateKind::Dead,
                receive_sequence: 4,
            })
            .unwrap();
        assert_eq!(index.rollback_dead_processed_versions(), 1);
        assert!(index.current(address, RpcCommitment::Processed).is_some());
    }
}
