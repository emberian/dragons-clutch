//! Current Product/Series account authentication for the 47-slot successor.
//!
//! This module owns only hostile account authentication. Historical RegistryV2,
//! FundingV2, replay V1, root V1, and link V1 helpers remain available to
//! decode old bytes, but no current successor receipt is constructible from
//! them. Mutation remains in event-specific atomic composers; this module does
//! not expose a generic successor writer.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, transfer_data,
    SYSTEM_PROGRAM_ID,
};
use crate::claim_release::AuthenticatedClaimIssuanceReleaseV1;
use crate::instructions::collateral_position_v3::{
    accept_general_market_liability_founding_postwrite_v3,
    AuthenticatedMarketLiabilityFoundingPostwriteV3, GeneralMarketValueAuthorityV2,
    RuntimeSha256,
};
use crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2;
use crate::instructions::product_market_foundation_current::{
    AuthenticatedProductMarketFoundationStepPostwriteV3,
    AuthenticatedProductMarketFounderCurrentCreationV3,
};
use crate::instructions::product_series::physical_v4::AuthenticatedSeriesPhysicalFounderV4;
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, authenticate_registry_capability_for_registration_v3,
};
use crate::instructions::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV6, AuthenticatedSeriesSourceArtifactsV5,
};
use crate::instructions::product_direct_global_liveness::{
    activate_product_direct_global_liveness_from_current_founder_v2,
    AuthenticatedProductDirectGlobalLivenessActivationV2,
    AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
};
use crate::instructions::source_occurrence_foundation_v1::
    AuthenticatedPreRootSourceOccurrencePostwriteV3;
use crate::seeds;
use crate::token;
use clutch_collateral_adapter_v2::{
    accept_claim_mint_founding_step_v2, accept_outcome_custody_founding_step_v1,
    accept_market_core_founding_v4, admit_collateral_mint_v2,
    compose_market_core_founding_v4, prepare_claim_mint_founding_v2,
    prepare_outcome_custody_founding_v1, AcceptedClaimMintFoundingStepV2,
    AcceptedMarketLiabilityFoundingV3, AcceptedOutcomeCustodyFoundingStepV1,
    BoundCollateralProfileV2, ClaimLedgerV3, ClaimMintFoundingPlanV2,
    ClaimMintFoundingPostwriteV2, ClaimMintFoundingRequestV2, CustodyCreationPlanV2,
    CustodyInitializationStepV2,
    HoardV2, Id as CollateralId, MarketLiabilityFoundingPlanV3,
    MarketLiabilityFoundingRequestV3,
    OutcomeCustodyFoundingPlanV1,
    OutcomeCustodyFoundingRequestV1, RuntimeAccountViewV2, CLAIM_LEDGER_V3_BYTES,
    HOARD_V2_BYTES, prepare_hoard_creation_v2, prepare_market_liability_founding_v3,
};
use clutch_product_series::{
    authenticate_market_foundation_account_graph_bytes_v3,
    AuthenticatedMarketFoundationAccountGraphBytesV3, CompiledProductSeriesBundleV6, ContentId,
    FixedCodec,
    MarketFoundationAccountGraphV3, MarketFoundationScheduleV3, MarketFoundationSlotV3,
    MarketInstancePreimageV2, MarketInstanceV2Id,
    AuthenticatedMarketFamilyAuthorityV1, MarketFamilyAggregatorV1, MarketFamilyStatusV1,
    MarketFamilyV1,
    MarketLifecyclePhaseV2, MarketLifecycleRootV2, MarketResolutionActivationV2,
    AuthenticatedSeriesFundingAuthorityV4, SeriesFundingCompletionAuthorizationV4,
    SeriesFundingCompletionAuthorizationV4Id, SeriesFundingCompletionBindingV4,
    SeriesFundingCompletionBindingV4Id, SeriesFundingReservationBindingV4,
    SeriesFundingReservationBindingV4Id, SeriesFundingStateV4,
    SeriesFundingStateV4Id, SeriesFundingStateV5,
    SeriesFundingComponentV2, SeriesFundingQuoteV5, SeriesFundingTermsV2Id,
    RegistryCapabilityProjectionV2,
    SeriesAttachmentPlanV5, SeriesAttachmentPlanV5Id, SeriesLifecycleReplayBindingV2Id,
    SeriesLifecycleReplayV2, SeriesLinkObligationAdmissionProjectionV2,
    SeriesLinkObligationDispositionV2, SeriesLinkObligationStatusV2,
    SeriesLinkObligationTerminalProjectionV2, SeriesLinkObligationV2,
    SeriesMarketAdmissionProjectionV2, SeriesMarketDispositionV1,
    SeriesMarketLinkPhaseV2, SeriesMarketLinkV2,
    SeriesMarketLinkV2Id, SeriesPlanV5, SeriesPlanV5Id, SourceOccurrenceV1Id,
    SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v2, MarketLifecycleRootAccountV2,
    SeriesFundingAccountV4, SeriesFundingAccountV5, SeriesLifecycleReplayAccountV2,
    SeriesMarketLinkAccountV2,
    SeriesRegistryAccountV3, SeriesRegistryAccountV4, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2,
    SERIES_FUNDING_ACCOUNT_BYTES_V4, SERIES_FUNDING_ACCOUNT_BYTES_V5,
    SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V2, SERIES_REGISTRY_ACCOUNT_BYTES_V3,
    SERIES_REGISTRY_ACCOUNT_BYTES_V4,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1};

const SERIES_REGISTRY_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-registry-account-authentication/v3\0";
const SERIES_REGISTRY_CAPABILITY_REFS_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-registry-capability-refs/v3\0";
const REGISTRY_CAPABILITY_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/registry-capability-authentication/v4\0";
const SERIES_REGISTRY_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/series-registry-account-authentication/v4\0";
const SERIES_REGISTRY_CAPABILITY_REFS_DOMAIN_V4: &[u8] =
    b"dragons-clutch/series-registry-capability-refs/v4\0";
const REGISTRY_CAPABILITY_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/registry-capability-authentication/v5\0";
const SERIES_FUNDING_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/series-funding-account-authentication/v4\0";
const SERIES_FUNDING_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/series-funding-account-authentication/v5\0";
const SERIES_FUNDING_RESERVATION_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-funding-reservation-postwrite/v4\0";
const SERIES_FUNDING_COMPLETION_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-funding-completion-postwrite/v4\0";
const PRODUCT_CURRENT_LINK_ACTIVATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-current-link-activation/v4\0";
const PRODUCT_CURRENT_ACTIVATION_COMPLETION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-current-activation-completion/v4\0";
const PRODUCT_CURRENT_ROOT_SLOT_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-current-root-slot-postwrite/v4\0";
const PRODUCT_CURRENT_RETAINED_PREALLOCATION_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-current-retained-preallocation-postwrite/v3\0";
const PRODUCT_CURRENT_MARKET_LIABILITY_PLAN_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-current-market-liability-plan/v3\0";
const PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-current-market-liability-slot-postwrite/v3\0";
const PRODUCT_CURRENT_CLAIM_MINT_PLAN_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-current-claim-mint-plan/v2\0";
const PRODUCT_CURRENT_CLAIM_MINT_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-current-claim-mint-postwrite/v2\0";
const PRODUCT_CURRENT_OUTCOME_CUSTODY_PLAN_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-current-outcome-custody-plan/v1\0";
const PRODUCT_CURRENT_OUTCOME_CUSTODY_POSTWRITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-current-outcome-custody-postwrite/v1\0";
const PRODUCT_CURRENT_FOUNDER_ACTIVATED_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/product-current-founder-activated/v4\0";
const SERIES_LIFECYCLE_REPLAY_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-lifecycle-replay-authentication/v2\0";
const MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-lifecycle-account-authentication/v2\0";
const MARKET_FOUNDATION_PREALLOCATION_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/market-foundation-preallocation-authentication/v3\0";
const SERIES_WRAPPER_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-wrapper-authentication/v2\0";
const SERIES_WRAPPER_ADMISSION_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-wrapper-admission-authentication/v2\0";
const SERIES_WRAPPER_TERMINAL_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-wrapper-terminal-authentication/v2\0";
const SERIES_DEALER_AUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-dealer-authorization/v2\0";
const SERIES_DEALER_ADMISSION_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-dealer-admission-postwrite/v2\0";
const SERIES_DEALER_TERMINAL_OBSERVATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-dealer-terminal-observation/v2\0";
const SERIES_DEALER_TERMINAL_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-dealer-terminal-postwrite/v2\0";
const SERIES_FAILURE_BEGIN_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/series-failure-begin-authentication/v2\0";
const SERIES_FAILURE_RELEASE_PREAUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-failure-release-preauthentication/v3\0";
const SERIES_FAILURE_RELEASE_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-failure-release-authentication/v3\0";
const PRODUCT_FRACTIONAL_ADMISSION_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-fractional-admission/v2\0";
const PRODUCT_FRACTIONAL_TERMINAL_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-fractional-terminal/v2\0";
const SERIES_LIFECYCLE_REPLAY_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/series-lifecycle-replay-postwrite/v2\0";
const MARKET_RESOLUTION_ACTIVATION_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/market-resolution-activation-postwrite/v2\0";

/// Exact current 0x7f/version3 registry authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesRegistryAccountV3 {
    account: Pubkey,
    value: SeriesRegistryAccountV3,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesRegistryAccountV3 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> SeriesRegistryAccountV3 { self.value }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(self) -> bool { self.writable }
    pub(crate) const fn data_id(self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(self) -> ContentId { self.authentication_id }
}

/// Private exact capability references projected only from hostile RegistryV3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesRegistryCapabilityRefsV3 {
    id: ContentId,
    series_registry_account: Pubkey,
    series_registry_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: clutch_product_series::SeriesFundingTermsV2Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
    activation_consumed: bool,
}

impl AuthenticatedSeriesRegistryCapabilityRefsV3 {
    pub(crate) const fn id(self) -> ContentId { self.id }
    pub(crate) const fn series_registry_account(self) -> Pubkey {
        self.series_registry_account
    }
    pub(crate) const fn series_registry_authentication_id(self) -> ContentId {
        self.series_registry_authentication_id
    }
    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn funding_terms_id(
        self,
    ) -> clutch_product_series::SeriesFundingTermsV2Id {
        self.funding_terms_id
    }
    pub(crate) const fn registry_release_id(self) -> ContentId { self.registry_release_id }
    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
    pub(crate) const fn compiler_bundle_id(
        self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV6Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn activation_consumed(self) -> bool { self.activation_consumed }
}

/// Exact RegistryV3-bound ReleaseV2/ProfileV4 loader authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedRegistryCapabilityV4 {
    id: ContentId,
    series_registry_account: Pubkey,
    series_registry_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: clutch_product_series::SeriesFundingTermsV2Id,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
    activation_consumed: bool,
    program_account: Pubkey,
    programdata_account: Pubkey,
    release_artifact_account: Pubkey,
    profile_artifact_account: Pubkey,
    release: clutch_product_series::RegistryProgramReleaseV2,
    profile: clutch_product_series::RegistryCapabilityProfileV4,
    projection: RegistryCapabilityProjectionV2,
    programdata_sha256: ContentId,
}

impl AuthenticatedRegistryCapabilityV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn series_registry_account(&self) -> Pubkey {
        self.series_registry_account
    }
    pub(crate) const fn series_registry_authentication_id(&self) -> ContentId {
        self.series_registry_authentication_id
    }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn funding_terms_id(
        &self,
    ) -> clutch_product_series::SeriesFundingTermsV2Id {
        self.funding_terms_id
    }
    pub(crate) const fn compiler_bundle_id(
        &self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV6Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn activation_consumed(&self) -> bool { self.activation_consumed }
    pub(crate) const fn program_account(&self) -> Pubkey { self.program_account }
    pub(crate) const fn programdata_account(&self) -> Pubkey { self.programdata_account }
    pub(crate) const fn release_artifact_account(&self) -> Pubkey {
        self.release_artifact_account
    }
    pub(crate) const fn profile_artifact_account(&self) -> Pubkey {
        self.profile_artifact_account
    }
    pub(crate) const fn release(&self) -> clutch_product_series::RegistryProgramReleaseV2 {
        self.release
    }
    pub(crate) const fn profile(&self) -> clutch_product_series::RegistryCapabilityProfileV4 {
        self.profile
    }
    pub(crate) const fn registry_release_id(&self) -> ContentId {
        self.projection.registry_release_id
    }
    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.projection.capability_profile_id
    }
    pub(crate) const fn projection(&self) -> RegistryCapabilityProjectionV2 {
        self.projection
    }
    pub(crate) const fn programdata_sha256(&self) -> ContentId { self.programdata_sha256 }
    pub(crate) const fn semantic_owners(
        &self,
    ) -> clutch_product_series::CapabilitySemanticOwnersV2 {
        self.profile.rules.semantic_owners
    }
    pub(crate) const fn realm_collateral(
        &self,
    ) -> clutch_product_series::RealmCollateralProjectionV1 {
        self.profile.rules.realm_collateral
    }
    pub(crate) const fn statistic_registry_value(&self) -> u16 {
        self.profile.rules.statistic_registry_value
    }
    pub(crate) const fn resolved_statistic(
        &self,
    ) -> clutch_source_plane_v3::StatisticKindV3 {
        self.profile.rules.resolved_statistic
    }
    pub(crate) const fn coverage_policy_registry_value(&self) -> u16 {
        self.profile.rules.coverage_policy_registry_value
    }
    pub(crate) const fn ambiguity_policy_registry_value(&self) -> u8 {
        self.profile.rules.ambiguity_policy_registry_value
    }
    pub(crate) const fn edge_policy_registry_value(&self) -> u8 {
        self.profile.rules.edge_policy_registry_value
    }
    pub(crate) const fn resolved_edge_policy(
        &self,
    ) -> clutch_product_series::QuantizedEdgePolicyV1 {
        self.profile.rules.resolved_edge_policy
    }
}

/// Exact current 0x7f/version4 registry authentication.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesRegistryAccountV4 {
    account: Pubkey,
    value: SeriesRegistryAccountV4,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesRegistryAccountV4 {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn value(&self) -> &SeriesRegistryAccountV4 { &self.value }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(&self) -> bool { self.writable }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Move-only exact capability references projected from hostile RegistryV4.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesRegistryCapabilityRefsV4 {
    id: ContentId,
    series_registry_account: Pubkey,
    series_registry_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: clutch_product_series::SeriesFundingTermsV2Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV7Id,
    activation_consumed: bool,
}

impl AuthenticatedSeriesRegistryCapabilityRefsV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn series_registry_account(&self) -> Pubkey {
        self.series_registry_account
    }
    pub(crate) const fn series_registry_authentication_id(&self) -> ContentId {
        self.series_registry_authentication_id
    }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn funding_terms_id(
        &self,
    ) -> clutch_product_series::SeriesFundingTermsV2Id {
        self.funding_terms_id
    }
    pub(crate) const fn registry_release_id(&self) -> ContentId { self.registry_release_id }
    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.capability_profile_id
    }
    pub(crate) const fn compiler_bundle_id(
        &self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV7Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn activation_consumed(&self) -> bool { self.activation_consumed }
}

/// Move-only RegistryV4-bound ReleaseV2/ProfileV4 loader authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedRegistryCapabilityV5 {
    id: ContentId,
    series_registry_account: Pubkey,
    series_registry_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: clutch_product_series::SeriesFundingTermsV2Id,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV7Id,
    activation_consumed: bool,
    program_account: Pubkey,
    programdata_account: Pubkey,
    release_artifact_account: Pubkey,
    profile_artifact_account: Pubkey,
    release: clutch_product_series::RegistryProgramReleaseV2,
    profile: clutch_product_series::RegistryCapabilityProfileV4,
    projection: RegistryCapabilityProjectionV2,
    programdata_sha256: ContentId,
}

impl AuthenticatedRegistryCapabilityV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn series_registry_account(&self) -> Pubkey {
        self.series_registry_account
    }
    pub(crate) const fn series_registry_authentication_id(&self) -> ContentId {
        self.series_registry_authentication_id
    }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn funding_terms_id(
        &self,
    ) -> clutch_product_series::SeriesFundingTermsV2Id {
        self.funding_terms_id
    }
    pub(crate) const fn compiler_bundle_id(
        &self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV7Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn activation_consumed(&self) -> bool { self.activation_consumed }
    pub(crate) const fn program_account(&self) -> Pubkey { self.program_account }
    pub(crate) const fn programdata_account(&self) -> Pubkey { self.programdata_account }
    pub(crate) const fn release_artifact_account(&self) -> Pubkey {
        self.release_artifact_account
    }
    pub(crate) const fn profile_artifact_account(&self) -> Pubkey {
        self.profile_artifact_account
    }
    pub(crate) const fn release(&self) -> clutch_product_series::RegistryProgramReleaseV2 {
        self.release
    }
    pub(crate) const fn profile(&self) -> clutch_product_series::RegistryCapabilityProfileV4 {
        self.profile
    }
    pub(crate) const fn registry_release_id(&self) -> ContentId {
        self.projection.registry_release_id
    }
    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.projection.capability_profile_id
    }
    pub(crate) const fn projection(&self) -> RegistryCapabilityProjectionV2 {
        self.projection
    }
    pub(crate) const fn programdata_sha256(&self) -> ContentId { self.programdata_sha256 }
    pub(crate) const fn semantic_owners(
        &self,
    ) -> clutch_product_series::CapabilitySemanticOwnersV2 {
        self.profile.rules.semantic_owners
    }
    pub(crate) const fn realm_collateral(
        &self,
    ) -> clutch_product_series::RealmCollateralProjectionV1 {
        self.profile.rules.realm_collateral
    }
    pub(crate) const fn statistic_registry_value(&self) -> u16 {
        self.profile.rules.statistic_registry_value
    }
    pub(crate) const fn resolved_statistic(
        &self,
    ) -> clutch_source_plane_v3::StatisticKindV3 {
        self.profile.rules.resolved_statistic
    }
    pub(crate) const fn coverage_policy_registry_value(&self) -> u16 {
        self.profile.rules.coverage_policy_registry_value
    }
    pub(crate) const fn ambiguity_policy_registry_value(&self) -> u8 {
        self.profile.rules.ambiguity_policy_registry_value
    }
    pub(crate) const fn edge_policy_registry_value(&self) -> u8 {
        self.profile.rules.edge_policy_registry_value
    }
    pub(crate) const fn resolved_edge_policy(
        &self,
    ) -> clutch_product_series::QuantizedEdgePolicyV1 {
        self.profile.rules.resolved_edge_policy
    }
}

/// Exact current 0x80/version5 acyclic funding authentication.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesFundingAccountV5 {
    account: Pubkey,
    value: SeriesFundingAccountV5,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesFundingAccountV5 {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn value(&self) -> &SeriesFundingAccountV5 { &self.value }
    pub(crate) const fn state(&self) -> &SeriesFundingStateV5 { &self.value.state }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(&self) -> bool { self.writable }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Historical 0x80/version4 acyclic funding authentication.
///
/// The receipt is non-Copy; downstream code borrows the decoded body so the
/// 756-byte account is moved exactly once across each transition boundary.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesFundingAccountV4 {
    account: Pubkey,
    value: SeriesFundingAccountV4,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesFundingAccountV4 {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn value(&self) -> &SeriesFundingAccountV4 { &self.value }
    pub(crate) const fn state(&self) -> &SeriesFundingStateV4 { &self.value.state }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(&self) -> bool { self.writable }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Non-detachable hostile Pending postwrite produced by the sole current
/// founder reservation transition.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesFundingReservationV4 {
    id: ContentId,
    binding: Box<SeriesFundingReservationBindingV4>,
    reservation_receipt_id: ContentId,
    funding_account: Pubkey,
    funding_state_before_id: SeriesFundingStateV4Id,
    funding_data_before_id: ContentId,
    funding_authentication_before_id: ContentId,
    pending: AuthenticatedSeriesFundingAccountV4,
}

/// Move-only pre-Replay authority. It owns the sole Pending reservation and
/// the deterministic Funding poststate preview; only the final replay-bound
/// writer below may consume it.
#[derive(Debug)]
struct AuthenticatedSeriesFundingCompletionAuthorizationV4 {
    id: SeriesFundingCompletionAuthorizationV4Id,
    facts: Box<SeriesFundingCompletionAuthorizationV4>,
    projected_state_after: Box<SeriesFundingStateV4>,
    reservation: Box<AuthenticatedProductSeriesFundingReservationV4>,
}

impl AuthenticatedSeriesFundingCompletionAuthorizationV4 {
    const fn id(&self) -> SeriesFundingCompletionAuthorizationV4Id { self.id }
    fn facts(&self) -> &SeriesFundingCompletionAuthorizationV4 { &self.facts }
}

impl AuthenticatedProductSeriesFundingReservationV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) fn binding(&self) -> &SeriesFundingReservationBindingV4 { &self.binding }
    pub(crate) const fn reservation_receipt_id(&self) -> ContentId {
        self.reservation_receipt_id
    }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.funding_account }
    pub(crate) const fn funding_state_before_id(&self) -> SeriesFundingStateV4Id {
        self.funding_state_before_id
    }
    pub(crate) const fn funding_data_before_id(&self) -> ContentId {
        self.funding_data_before_id
    }
    pub(crate) const fn funding_authentication_before_id(&self) -> ContentId {
        self.funding_authentication_before_id
    }
    pub(crate) fn funding_state_pending_id(&self) -> Outcome<SeriesFundingStateV4Id> {
        self.pending
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    }
    pub(crate) const fn funding_data_pending_id(&self) -> ContentId {
        self.pending.data_id()
    }
    pub(crate) const fn funding_authentication_pending_id(&self) -> ContentId {
        self.pending.authentication_id()
    }
    pub(crate) const fn pending(&self) -> &AuthenticatedSeriesFundingAccountV4 {
        &self.pending
    }
}

/// Exact hostile Pending→Active/Closed postwrite produced only after the
/// Source/Root/Link/replay completion join is physically persisted.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesFundingCompletionV4 {
    id: ContentId,
    completion_authorization_id: SeriesFundingCompletionAuthorizationV4Id,
    projected_state_after_id: SeriesFundingStateV4Id,
    completion_binding_id: SeriesFundingCompletionBindingV4Id,
    reservation_postwrite_id: ContentId,
    funding_account: Pubkey,
    funding_state_before_id: SeriesFundingStateV4Id,
    funding_state_after_id: SeriesFundingStateV4Id,
    funding_data_before_id: ContentId,
    funding_data_after_id: ContentId,
    funding_authentication_before_id: ContentId,
    funding_authentication_after_id: ContentId,
    completed_ordinal: u32,
    rebound: AuthenticatedSeriesFundingAccountV4,
}

impl AuthenticatedProductSeriesFundingCompletionV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn completion_authorization_id(
        &self,
    ) -> SeriesFundingCompletionAuthorizationV4Id {
        self.completion_authorization_id
    }
    pub(crate) const fn projected_state_after_id(&self) -> SeriesFundingStateV4Id {
        self.projected_state_after_id
    }
    pub(crate) const fn completion_binding_id(&self) -> SeriesFundingCompletionBindingV4Id {
        self.completion_binding_id
    }
    pub(crate) const fn reservation_postwrite_id(&self) -> ContentId {
        self.reservation_postwrite_id
    }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.funding_account }
    pub(crate) const fn funding_state_before_id(&self) -> SeriesFundingStateV4Id {
        self.funding_state_before_id
    }
    pub(crate) const fn funding_state_after_id(&self) -> SeriesFundingStateV4Id {
        self.funding_state_after_id
    }
    pub(crate) const fn funding_data_before_id(&self) -> ContentId {
        self.funding_data_before_id
    }
    pub(crate) const fn funding_data_after_id(&self) -> ContentId {
        self.funding_data_after_id
    }
    pub(crate) const fn funding_authentication_before_id(&self) -> ContentId {
        self.funding_authentication_before_id
    }
    pub(crate) const fn funding_authentication_after_id(&self) -> ContentId {
        self.funding_authentication_after_id
    }
    pub(crate) const fn completed_ordinal(&self) -> u32 { self.completed_ordinal }
    pub(crate) const fn rebound(&self) -> &AuthenticatedSeriesFundingAccountV4 {
        &self.rebound
    }
}

/// Sole move-only Product postwrite after RootV2/LinkV2 activation, replayV2
/// admission, and FundingV4 completion. The Direct capitalization cannot be
/// extracted until every Product write named by this receipt is complete.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesActivationCompletionV4 {
    id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_complete_receipt_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_after: ContentId,
    root_semantic_after: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    final_foundation_donation_lamports: u64,
    market_family_capability_policy_id: ContentId,
    market_family_capability_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_after: ContentId,
    link_semantic_after: SeriesMarketLinkV2Id,
    link_activation_receipt_id: ContentId,
    market_admission_receipt_id: ContentId,
    replay_account: Pubkey,
    replay_authentication_after: ContentId,
    replay_state_after_id: ContentId,
    replay_admission_projection_id: ContentId,
    funding_completion: Box<AuthenticatedProductSeriesFundingCompletionV4>,
    source: Box<AuthenticatedPreRootSourceOccurrencePostwriteV3>,
    direct_capitalization: AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
    physical: AuthenticatedSeriesPhysicalFounderV4,
}

impl AuthenticatedProductSeriesActivationCompletionV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn founder_creation_receipt_id(&self) -> ContentId {
        self.founder_creation_receipt_id
    }
    pub(crate) const fn founder_preauthorization_id(&self) -> ContentId {
        self.founder_preauthorization_id
    }
    pub(crate) const fn foundation_complete_receipt_id(&self) -> ContentId {
        self.foundation_complete_receipt_id
    }
    pub(crate) const fn funding_completion(
        &self,
    ) -> &AuthenticatedProductSeriesFundingCompletionV4 {
        &self.funding_completion
    }
    pub(crate) const fn source(
        &self,
    ) -> &AuthenticatedPreRootSourceOccurrencePostwriteV3 {
        &self.source
    }
    pub(crate) const fn root_transition_sequence_before(&self) -> u64 {
        self.root_transition_sequence_before
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn final_foundation_donation_lamports(&self) -> u64 {
        self.final_foundation_donation_lamports
    }

    pub(super) fn into_direct_activation_parts(
        self,
    ) -> (
        ContentId,
        Pubkey,
        ContentId,
        ContentId,
        ContentId,
        ContentId,
        AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
        AuthenticatedSeriesPhysicalFounderV4,
    ) {
        (
            self.founder_creation_receipt_id,
            self.root_account,
            self.root_binding_id,
            self.root_authentication_after,
            self.root_semantic_after,
            self.founder_preauthorization_id,
            self.direct_capitalization,
            self.physical,
        )
    }
}

/// Exact current permanent 0xb8/version2 replay authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesLifecycleReplayV2 {
    account: Pubkey,
    value: SeriesLifecycleReplayAccountV2,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesLifecycleReplayV2 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> SeriesLifecycleReplayAccountV2 { self.value }
    pub(crate) const fn state(self) -> SeriesLifecycleReplayV2 { self.value.state }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(self) -> bool { self.writable }
    pub(crate) const fn data_id(self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(self) -> ContentId { self.authentication_id }
}

/// Exact current shared 0xaa/version2 root authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketLifecycleRootV2<'state> {
    account: Pubkey,
    owner_program: Pubkey,
    value: &'state MarketLifecycleRootAccountV2,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl<'state> AuthenticatedMarketLifecycleRootV2<'state> {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn owner_program(self) -> Pubkey { self.owner_program }
    pub(crate) const fn value(self) -> &'state MarketLifecycleRootAccountV2 { self.value }
    pub(crate) const fn state(self) -> &'state MarketLifecycleRootV2 { &self.value.state }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(self) -> bool { self.writable }
    pub(crate) const fn data_id(self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(self) -> ContentId { self.authentication_id }
}

/// Exact current per-Series 0xad/version2 link authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesMarketLinkV2<'state> {
    account: Pubkey,
    owner_program: Pubkey,
    value: &'state SeriesMarketLinkAccountV2,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl<'state> AuthenticatedSeriesMarketLinkV2<'state> {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn owner_program(self) -> Pubkey { self.owner_program }
    pub(crate) const fn value(self) -> &'state SeriesMarketLinkAccountV2 { self.value }
    pub(crate) const fn state(self) -> &'state SeriesMarketLinkV2 { &self.value.state }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(self) -> bool { self.writable }
    pub(crate) const fn data_id(self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(self) -> ContentId { self.authentication_id }
}

/// Product-private current Wrapper authorization derived from exact live
/// LinkV2 + BundleV6 + AttachmentV5 accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesWrapperAuthorizationV2 {
    id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV2Id,
    link_binding_id: ContentId,
    wrapper_obligation_configuration_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    attachment_plan_id: SeriesAttachmentPlanV5Id,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
    capability_profile_id: ContentId,
    wrapper_recipe_set_id: ContentId,
    rent_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    wrapper_status: SeriesLinkObligationStatusV2,
    wrapper_admission_receipt_id: ContentId,
    link_transition_sequence: u64,
}

impl AuthenticatedSeriesWrapperAuthorizationV2 {
    pub(crate) const fn id(self) -> ContentId { self.id }
    pub(crate) const fn link_account(self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_id(self) -> ContentId { self.link_authentication_id }
    pub(crate) const fn link_semantic_id(self) -> SeriesMarketLinkV2Id { self.link_semantic_id }
    pub(crate) const fn link_binding_id(self) -> ContentId { self.link_binding_id }
    pub(crate) const fn wrapper_obligation_configuration_id(self) -> ContentId {
        self.wrapper_obligation_configuration_id
    }
    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(self) -> u32 { self.ordinal }
    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id { self.market_instance_id }
    pub(crate) const fn generation(self) -> u64 { self.generation }
    pub(crate) const fn attachment_plan_id(self) -> SeriesAttachmentPlanV5Id {
        self.attachment_plan_id
    }
    pub(crate) const fn compiler_bundle_id(self) -> clutch_product_series::CompiledProductSeriesBundleV6Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn funding_quote_id(self) -> clutch_product_series::SeriesFundingQuoteV5Id {
        self.funding_quote_id
    }
    pub(crate) const fn capability_profile_id(self) -> ContentId { self.capability_profile_id }
    pub(crate) const fn wrapper_recipe_set_id(self) -> ContentId { self.wrapper_recipe_set_id }
    pub(crate) const fn rent_refund_owner(self) -> ContentId { self.rent_refund_owner }
    pub(crate) const fn neutral_lamport_sink(self) -> ContentId { self.neutral_lamport_sink }
    pub(crate) const fn wrapper_status(self) -> SeriesLinkObligationStatusV2 { self.wrapper_status }
    pub(crate) const fn wrapper_admission_receipt_id(self) -> ContentId {
        self.wrapper_admission_receipt_id
    }
    pub(crate) const fn link_transition_sequence(self) -> u64 { self.link_transition_sequence }
    pub(crate) const fn requires_product_admission(self) -> bool {
        matches!(self.wrapper_status, SeriesLinkObligationStatusV2::EnabledNeverFounded)
    }
}

/// Product-private first-lease authorization for the Dealer obligation.
///
/// This compact receipt is minted only from a hostile current RootV2,
/// writable LinkV2, RegistryV3/Release/Profile, BundleV6, and AttachmentV5.
/// It contains no Dealer-created state and is consumed once by the narrow
/// admission writer below.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesDealerAuthorizationV2 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_binding_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_data_id: ContentId,
    link_semantic_id: SeriesMarketLinkV2Id,
    link_binding_id: ContentId,
    link_transition_sequence: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    funding_terms_id: SeriesFundingTermsV2Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
    attachment_plan_id: SeriesAttachmentPlanV5Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    dealer_obligation_configuration_id: ContentId,
    rent_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
}

impl AuthenticatedSeriesDealerAuthorizationV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_binding_id(&self) -> ContentId { self.link_binding_id }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.ordinal }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn compiler_bundle_id(
        &self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV6Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn attachment_plan_id(&self) -> SeriesAttachmentPlanV5Id {
        self.attachment_plan_id
    }
    pub(crate) const fn rent_refund_owner(&self) -> ContentId { self.rent_refund_owner }
    pub(crate) const fn neutral_lamport_sink(&self) -> ContentId {
        self.neutral_lamport_sink
    }
}

/// Dealer-owned prewrite accepted only by the first-lease Product admission.
///
/// The sole implementation must be Dealer's private, non-Copy prewrite over
/// the canonical 0xaf/v2 and StateV3 promotion plan. Every accessor defaults to
/// refusal so a caller-shaped DTO cannot accidentally satisfy this boundary.
pub(crate) trait AuthenticatedSeriesDealerAdmissionOwnerV2 {
    fn owner_admission_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_obligation_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_state_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_state_presemantic_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_facility_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_position_binding_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_rent_principal_lamports(&self) -> Outcome<u64> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_prefund_donation_lamports(&self) -> Outcome<u64> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn rent_refund_owner(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn neutral_lamport_sink(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_dealer_admission_owner_v2(
        &self,
        _authorization_id: ContentId,
        _root_account: Pubkey,
        _root_binding_id: ContentId,
        _link_account: Pubkey,
        _link_binding_id: ContentId,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
        _compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
        _attachment_plan_id: SeriesAttachmentPlanV5Id,
        _registry_release_id: ContentId,
        _capability_profile_id: ContentId,
        _dealer_obligation_configuration_id: ContentId,
        _dealer_obligation_account: Pubkey,
        _dealer_state_account: Pubkey,
        _dealer_state_presemantic_id: ContentId,
        _dealer_facility_id: ContentId,
        _dealer_position_binding_id: ContentId,
        _dealer_rent_principal_lamports: u64,
        _dealer_prefund_donation_lamports: u64,
        _rent_refund_owner: ContentId,
        _neutral_lamport_sink: ContentId,
        _owner_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Non-detachable hostile LinkV2 postwrite consumed by Dealer action14.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesDealerAdmissionV2 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_binding_id: ContentId,
    link_account: Pubkey,
    link_binding_id: ContentId,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_data_before: ContentId,
    link_data_after: ContentId,
    link_semantic_before: SeriesMarketLinkV2Id,
    link_semantic_after: SeriesMarketLinkV2Id,
    link_transition_sequence_after: u64,
    product_admission_projection: SeriesLinkObligationAdmissionProjectionV2,
    owner_admission_receipt_id: ContentId,
    dealer_obligation_account: Pubkey,
    dealer_state_account: Pubkey,
    dealer_state_presemantic_id: ContentId,
    dealer_facility_id: ContentId,
    dealer_position_binding_id: ContentId,
    dealer_rent_principal_lamports: u64,
    dealer_prefund_donation_lamports: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
    attachment_plan_id: SeriesAttachmentPlanV5Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    dealer_obligation_configuration_id: ContentId,
    registry_capability_id: ContentId,
    rent_refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
}

impl AuthenticatedSeriesDealerAdmissionV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId { self.root_semantic_id }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_binding_id(&self) -> ContentId { self.link_binding_id }
    pub(crate) const fn link_authentication_before(&self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(&self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_data_before(&self) -> ContentId { self.link_data_before }
    pub(crate) const fn link_data_after(&self) -> ContentId { self.link_data_after }
    pub(crate) const fn link_semantic_before(&self) -> SeriesMarketLinkV2Id {
        self.link_semantic_before
    }
    pub(crate) const fn link_semantic_after(&self) -> SeriesMarketLinkV2Id {
        self.link_semantic_after
    }
    pub(crate) const fn link_transition_sequence_after(&self) -> u64 {
        self.link_transition_sequence_after
    }
    pub(crate) const fn product_admission_projection(
        &self,
    ) -> SeriesLinkObligationAdmissionProjectionV2 {
        self.product_admission_projection
    }
    pub(crate) const fn owner_admission_receipt_id(&self) -> ContentId {
        self.owner_admission_receipt_id
    }
    pub(crate) const fn dealer_obligation_account(&self) -> Pubkey {
        self.dealer_obligation_account
    }
    pub(crate) const fn dealer_state_account(&self) -> Pubkey { self.dealer_state_account }
    pub(crate) const fn dealer_state_presemantic_id(&self) -> ContentId {
        self.dealer_state_presemantic_id
    }
    pub(crate) const fn dealer_facility_id(&self) -> ContentId { self.dealer_facility_id }
    pub(crate) const fn dealer_position_binding_id(&self) -> ContentId {
        self.dealer_position_binding_id
    }
    pub(crate) const fn dealer_rent_principal_lamports(&self) -> u64 {
        self.dealer_rent_principal_lamports
    }
    pub(crate) const fn dealer_prefund_donation_lamports(&self) -> u64 {
        self.dealer_prefund_donation_lamports
    }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.ordinal }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn compiler_bundle_id(
        &self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV6Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn attachment_plan_id(&self) -> SeriesAttachmentPlanV5Id {
        self.attachment_plan_id
    }
    pub(crate) const fn funding_quote_id(
        &self,
    ) -> clutch_product_series::SeriesFundingQuoteV5Id {
        self.funding_quote_id
    }
    pub(crate) const fn registry_release_id(&self) -> ContentId { self.registry_release_id }
    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.capability_profile_id
    }
    pub(crate) const fn dealer_obligation_configuration_id(&self) -> ContentId {
        self.dealer_obligation_configuration_id
    }
    pub(crate) const fn registry_capability_id(&self) -> ContentId {
        self.registry_capability_id
    }
    pub(crate) const fn rent_refund_owner(&self) -> ContentId { self.rent_refund_owner }
    pub(crate) const fn neutral_lamport_sink(&self) -> ContentId {
        self.neutral_lamport_sink
    }
}

/// Product-local observation consumed by Dealer's single non-Copy action25
/// prewrite before LinkV2 is mutated.
///
/// Every Product semantic identity in this value is derived from hostile
/// account bytes inside [`terminalize_series_dealer_obligation_v2`]. Dealer
/// may compare the observation with its retained live-Product receipt, but it
/// cannot supply or substitute any of these identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesDealerTerminalObservationV2 {
    owner_authentication_id: ContentId,
    dealer_obligation_account: Pubkey,
    dealer_obligation_presemantic_id: ContentId,
    dealer_state_account: Pubkey,
    dealer_state_presemantic_id: ContentId,
    terminal_state_receipt_id: ContentId,
    replay_presemantic_id: ContentId,
    replay_pre_ordinal: u64,
    owner_terminal_receipt_id: ContentId,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    root_data_id: ContentId,
    root_semantic_id: ContentId,
    root_binding_id: ContentId,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    resolution_activation_receipt_id: ContentId,
    registry_account: Pubkey,
    registry_authentication_id: ContentId,
    registry_capability_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    registry_release_artifact_account: Pubkey,
    capability_profile_artifact_account: Pubkey,
    registry_program: Pubkey,
    registry_programdata: Pubkey,
    registry_programdata_sha256: ContentId,
    compiler_bundle_account: Pubkey,
    compiler_bundle_id: ContentId,
    compiler_bundle_semantic_id: ContentId,
    attachment_account: Pubkey,
    attachment_plan_id: ContentId,
    attachment_semantic_id: ContentId,
    liquidity_facility_plan_id: ContentId,
    dealer_obligation_configuration_id: ContentId,
    link_account: Pubkey,
    link_binding_id: ContentId,
    link_authentication_before: ContentId,
    link_data_before: ContentId,
    link_semantic_before: ContentId,
    dealer_admission_receipt_id: ContentId,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
}

impl SeriesDealerTerminalObservationV2 {
    pub(crate) const fn owner_authentication_id(self) -> ContentId {
        self.owner_authentication_id
    }
    pub(crate) const fn dealer_obligation_account(self) -> Pubkey {
        self.dealer_obligation_account
    }
    pub(crate) const fn dealer_obligation_presemantic_id(self) -> ContentId {
        self.dealer_obligation_presemantic_id
    }
    pub(crate) const fn dealer_state_account(self) -> Pubkey { self.dealer_state_account }
    pub(crate) const fn dealer_state_presemantic_id(self) -> ContentId {
        self.dealer_state_presemantic_id
    }
    pub(crate) const fn terminal_state_receipt_id(self) -> ContentId {
        self.terminal_state_receipt_id
    }
    pub(crate) const fn replay_presemantic_id(self) -> ContentId {
        self.replay_presemantic_id
    }
    pub(crate) const fn replay_pre_ordinal(self) -> u64 { self.replay_pre_ordinal }
    pub(crate) const fn owner_terminal_receipt_id(self) -> ContentId {
        self.owner_terminal_receipt_id
    }
    pub(crate) const fn rent_refund_owner(self) -> Pubkey { self.rent_refund_owner }
    pub(crate) const fn neutral_lamport_sink(self) -> Pubkey { self.neutral_lamport_sink }
    pub(crate) const fn root_account(self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn root_data_id(self) -> ContentId { self.root_data_id }
    pub(crate) const fn root_semantic_id(self) -> ContentId { self.root_semantic_id }
    pub(crate) const fn root_binding_id(self) -> ContentId { self.root_binding_id }
    pub(crate) const fn resolution_semantic_id(self) -> ContentId {
        self.resolution_semantic_id
    }
    pub(crate) const fn resolution_data_id(self) -> ContentId { self.resolution_data_id }
    pub(crate) const fn resolution_activation_receipt_id(self) -> ContentId {
        self.resolution_activation_receipt_id
    }
    pub(crate) const fn registry_account(self) -> Pubkey { self.registry_account }
    pub(crate) const fn registry_authentication_id(self) -> ContentId {
        self.registry_authentication_id
    }
    pub(crate) const fn registry_capability_id(self) -> ContentId {
        self.registry_capability_id
    }
    pub(crate) const fn registry_release_id(self) -> ContentId { self.registry_release_id }
    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }
    pub(crate) const fn registry_release_artifact_account(self) -> Pubkey {
        self.registry_release_artifact_account
    }
    pub(crate) const fn capability_profile_artifact_account(self) -> Pubkey {
        self.capability_profile_artifact_account
    }
    pub(crate) const fn registry_program(self) -> Pubkey { self.registry_program }
    pub(crate) const fn registry_programdata(self) -> Pubkey { self.registry_programdata }
    pub(crate) const fn registry_programdata_sha256(self) -> ContentId {
        self.registry_programdata_sha256
    }
    pub(crate) const fn compiler_bundle_account(self) -> Pubkey {
        self.compiler_bundle_account
    }
    pub(crate) const fn compiler_bundle_id(self) -> ContentId { self.compiler_bundle_id }
    pub(crate) const fn compiler_bundle_semantic_id(self) -> ContentId {
        self.compiler_bundle_semantic_id
    }
    pub(crate) const fn attachment_account(self) -> Pubkey { self.attachment_account }
    pub(crate) const fn attachment_plan_id(self) -> ContentId { self.attachment_plan_id }
    pub(crate) const fn attachment_semantic_id(self) -> ContentId {
        self.attachment_semantic_id
    }
    pub(crate) const fn liquidity_facility_plan_id(self) -> ContentId {
        self.liquidity_facility_plan_id
    }
    pub(crate) const fn dealer_obligation_configuration_id(self) -> ContentId {
        self.dealer_obligation_configuration_id
    }
    pub(crate) const fn link_account(self) -> Pubkey { self.link_account }
    pub(crate) const fn link_binding_id(self) -> ContentId { self.link_binding_id }
    pub(crate) const fn link_authentication_before(self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_data_before(self) -> ContentId { self.link_data_before }
    pub(crate) const fn link_semantic_before(self) -> ContentId {
        self.link_semantic_before
    }
    pub(crate) const fn dealer_admission_receipt_id(self) -> ContentId {
        self.dealer_admission_receipt_id
    }
    pub(crate) const fn link_transition_sequence_before(self) -> u64 {
        self.link_transition_sequence_before
    }
    pub(crate) const fn link_transition_sequence_after(self) -> u64 {
        self.link_transition_sequence_after
    }

    fn id(self) -> ContentId {
        hashv(&[
            SERIES_DEALER_TERMINAL_OBSERVATION_DOMAIN_V2,
            &self.owner_authentication_id.bytes(),
            self.dealer_obligation_account.as_ref(),
            &self.dealer_obligation_presemantic_id.bytes(),
            self.dealer_state_account.as_ref(),
            &self.dealer_state_presemantic_id.bytes(),
            &self.terminal_state_receipt_id.bytes(),
            &self.replay_presemantic_id.bytes(),
            &self.replay_pre_ordinal.to_le_bytes(),
            &self.owner_terminal_receipt_id.bytes(),
            self.rent_refund_owner.as_ref(),
            self.neutral_lamport_sink.as_ref(),
            self.root_account.as_ref(),
            &self.root_authentication_id.bytes(),
            &self.root_data_id.bytes(),
            &self.root_semantic_id.bytes(),
            &self.root_binding_id.bytes(),
            &self.resolution_semantic_id.bytes(),
            &self.resolution_data_id.bytes(),
            &self.resolution_activation_receipt_id.bytes(),
            self.registry_account.as_ref(),
            &self.registry_authentication_id.bytes(),
            &self.registry_capability_id.bytes(),
            &self.registry_release_id.bytes(),
            &self.capability_profile_id.bytes(),
            self.registry_release_artifact_account.as_ref(),
            self.capability_profile_artifact_account.as_ref(),
            self.registry_program.as_ref(),
            self.registry_programdata.as_ref(),
            &self.registry_programdata_sha256.bytes(),
            self.compiler_bundle_account.as_ref(),
            &self.compiler_bundle_id.bytes(),
            &self.compiler_bundle_semantic_id.bytes(),
            self.attachment_account.as_ref(),
            &self.attachment_plan_id.bytes(),
            &self.attachment_semantic_id.bytes(),
            &self.liquidity_facility_plan_id.bytes(),
            &self.dealer_obligation_configuration_id.bytes(),
            self.link_account.as_ref(),
            &self.link_binding_id.bytes(),
            &self.link_authentication_before.bytes(),
            &self.link_data_before.bytes(),
            &self.link_semantic_before.bytes(),
            &self.dealer_admission_receipt_id.bytes(),
            &self.link_transition_sequence_before.to_le_bytes(),
            &self.link_transition_sequence_after.to_le_bytes(),
        ])
    }
}

/// Dealer-owned action25 prewrite accepted only by Product's sole current
/// LinkV2 Dealer terminal writer.
///
/// The sole implementation is Dealer's private
/// `AuthenticatedDealerSeriesTerminalPrewriteV2`. Every accessor defaults to
/// refusal. The authority is consumed by value after Product reconstructs the
/// complete current account graph and before the first LinkV2 write.
pub(crate) trait AuthenticatedSeriesDealerTerminalOwnerV2 {
    fn owner_authentication_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_obligation_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_obligation_presemantic_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_state_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn dealer_state_presemantic_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn terminal_state_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn replay_presemantic_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn replay_pre_ordinal(&self) -> Outcome<u64> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn owner_terminal_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn expected_link_transition_sequence(&self) -> Outcome<u64> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn rent_refund_owner(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn neutral_lamport_sink(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn consume_series_dealer_terminal_owner_v2(
        self,
        _observed: SeriesDealerTerminalObservationV2,
    ) -> Outcome<()>
    where
        Self: Sized,
    {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Non-Copy Product postwrite consumed by Dealer before terminalizing `0xaf/v2`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesDealerTerminalV2 {
    id: ContentId,
    observation: SeriesDealerTerminalObservationV2,
    link_authentication_after: ContentId,
    link_data_after: ContentId,
    link_semantic_after: ContentId,
    terminal_projection: SeriesLinkObligationTerminalProjectionV2,
    terminal_projection_id: ContentId,
}

impl AuthenticatedSeriesDealerTerminalV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn owner_authentication_id(&self) -> ContentId {
        self.observation.owner_authentication_id
    }
    pub(crate) const fn dealer_obligation_account(&self) -> Pubkey {
        self.observation.dealer_obligation_account
    }
    pub(crate) const fn dealer_obligation_presemantic_id(&self) -> ContentId {
        self.observation.dealer_obligation_presemantic_id
    }
    pub(crate) const fn dealer_state_account(&self) -> Pubkey {
        self.observation.dealer_state_account
    }
    pub(crate) const fn dealer_state_presemantic_id(&self) -> ContentId {
        self.observation.dealer_state_presemantic_id
    }
    pub(crate) const fn terminal_state_receipt_id(&self) -> ContentId {
        self.observation.terminal_state_receipt_id
    }
    pub(crate) const fn replay_presemantic_id(&self) -> ContentId {
        self.observation.replay_presemantic_id
    }
    pub(crate) const fn replay_pre_ordinal(&self) -> u64 {
        self.observation.replay_pre_ordinal
    }
    pub(crate) const fn owner_terminal_receipt_id(&self) -> ContentId {
        self.observation.owner_terminal_receipt_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.observation.root_account }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.observation.root_authentication_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId {
        self.observation.root_semantic_id
    }
    pub(crate) const fn root_data_id(&self) -> ContentId { self.observation.root_data_id }
    pub(crate) const fn root_binding_id(&self) -> ContentId {
        self.observation.root_binding_id
    }
    pub(crate) const fn resolution_semantic_id(&self) -> ContentId {
        self.observation.resolution_semantic_id
    }
    pub(crate) const fn resolution_data_id(&self) -> ContentId {
        self.observation.resolution_data_id
    }
    pub(crate) const fn resolution_activation_receipt_id(&self) -> ContentId {
        self.observation.resolution_activation_receipt_id
    }
    pub(crate) const fn registry_capability_id(&self) -> ContentId {
        self.observation.registry_capability_id
    }
    pub(crate) const fn registry_account(&self) -> Pubkey {
        self.observation.registry_account
    }
    pub(crate) const fn registry_authentication_id(&self) -> ContentId {
        self.observation.registry_authentication_id
    }
    pub(crate) const fn registry_release_id(&self) -> ContentId {
        self.observation.registry_release_id
    }
    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.observation.capability_profile_id
    }
    pub(crate) const fn registry_release_artifact_account(&self) -> Pubkey {
        self.observation.registry_release_artifact_account
    }
    pub(crate) const fn capability_profile_artifact_account(&self) -> Pubkey {
        self.observation.capability_profile_artifact_account
    }
    pub(crate) const fn compiler_bundle_id(&self) -> ContentId {
        self.observation.compiler_bundle_id
    }
    pub(crate) const fn compiler_bundle_account(&self) -> Pubkey {
        self.observation.compiler_bundle_account
    }
    pub(crate) const fn compiler_bundle_semantic_id(&self) -> ContentId {
        self.observation.compiler_bundle_semantic_id
    }
    pub(crate) const fn attachment_plan_id(&self) -> ContentId {
        self.observation.attachment_plan_id
    }
    pub(crate) const fn attachment_account(&self) -> Pubkey {
        self.observation.attachment_account
    }
    pub(crate) const fn attachment_semantic_id(&self) -> ContentId {
        self.observation.attachment_semantic_id
    }
    pub(crate) const fn liquidity_facility_plan_id(&self) -> ContentId {
        self.observation.liquidity_facility_plan_id
    }
    pub(crate) const fn dealer_obligation_configuration_id(&self) -> ContentId {
        self.observation.dealer_obligation_configuration_id
    }
    pub(crate) const fn registry_programdata(&self) -> Pubkey {
        self.observation.registry_programdata
    }
    pub(crate) const fn registry_program(&self) -> Pubkey {
        self.observation.registry_program
    }
    pub(crate) const fn registry_programdata_sha256(&self) -> ContentId {
        self.observation.registry_programdata_sha256
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.observation.link_account }
    pub(crate) const fn link_binding_id(&self) -> ContentId {
        self.observation.link_binding_id
    }
    pub(crate) const fn link_authentication_before(&self) -> ContentId {
        self.observation.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(&self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_data_before(&self) -> ContentId {
        self.observation.link_data_before
    }
    pub(crate) const fn link_data_after(&self) -> ContentId { self.link_data_after }
    pub(crate) const fn link_semantic_before(&self) -> ContentId {
        self.observation.link_semantic_before
    }
    pub(crate) const fn link_semantic_after(&self) -> ContentId {
        self.link_semantic_after
    }
    pub(crate) const fn link_transition_sequence_after(&self) -> u64 {
        self.observation.link_transition_sequence_after
    }
    pub(crate) const fn link_transition_sequence_before(&self) -> u64 {
        self.observation.link_transition_sequence_before
    }
    pub(crate) const fn dealer_admission_receipt_id(&self) -> ContentId {
        self.observation.dealer_admission_receipt_id
    }
    pub(crate) const fn terminal_projection(
        &self,
    ) -> SeriesLinkObligationTerminalProjectionV2 {
        self.terminal_projection
    }
    pub(crate) const fn terminal_projection_id(&self) -> ContentId {
        self.terminal_projection_id
    }
    pub(crate) const fn rent_refund_owner(&self) -> Pubkey {
        self.observation.rent_refund_owner
    }
    pub(crate) const fn neutral_lamport_sink(&self) -> Pubkey {
        self.observation.neutral_lamport_sink
    }
}

/// Default-refusing exact Structured admission owner. Implementations must be
/// private same-instruction Structured postwrites, never decoded DTOs.
pub(crate) trait AuthenticatedSeriesWrapperAdmissionOwnerV2 {
    fn owner_admission_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_wrapper_admission_owner_v2(
        &self,
        _authorization_id: ContentId,
        _link_account: Pubkey,
        _link_binding_id: ContentId,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _attachment_plan_id: SeriesAttachmentPlanV5Id,
        _compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
        _funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
        _capability_profile_id: ContentId,
        _wrapper_obligation_configuration_id: ContentId,
        _wrapper_recipe_set_id: ContentId,
        _rent_refund_owner: ContentId,
        _neutral_lamport_sink: ContentId,
        _owner_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Default-refusing exact Structured terminal postwrite owner.
pub(crate) trait AuthenticatedSeriesWrapperTerminalOwnerV2 {
    fn owner_terminal_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn structured_root_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn structured_root_semantic_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn structured_root_data_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_wrapper_terminal_owner_v2(
        &self,
        _authorization_id: ContentId,
        _link_account: Pubkey,
        _link_binding_id: ContentId,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _attachment_plan_id: SeriesAttachmentPlanV5Id,
        _compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
        _funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
        _capability_profile_id: ContentId,
        _wrapper_obligation_configuration_id: ContentId,
        _wrapper_recipe_set_id: ContentId,
        _rent_refund_owner: ContentId,
        _neutral_lamport_sink: ContentId,
        _wrapper_admission_receipt_id: ContentId,
        _owner_terminal_receipt_id: ContentId,
        _structured_root_account: Pubkey,
        _structured_root_semantic_id: ContentId,
        _structured_root_data_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Hostile postwrite receipt for one exact current Wrapper admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesWrapperAdmissionV2 {
    id: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: SeriesMarketLinkV2Id,
    link_semantic_after: SeriesMarketLinkV2Id,
    owner_admission_receipt_id: ContentId,
    product_admission_projection_id: ContentId,
}

impl AuthenticatedSeriesWrapperAdmissionV2 {
    pub(crate) const fn id(self) -> ContentId { self.id }
    pub(crate) const fn link_account(self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_before(self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_semantic_before(self) -> SeriesMarketLinkV2Id {
        self.link_semantic_before
    }
    pub(crate) const fn link_semantic_after(self) -> SeriesMarketLinkV2Id {
        self.link_semantic_after
    }
    pub(crate) const fn owner_admission_receipt_id(self) -> ContentId {
        self.owner_admission_receipt_id
    }
    pub(crate) const fn product_admission_projection_id(self) -> ContentId {
        self.product_admission_projection_id
    }
}

/// Hostile postwrite receipt for one exact current Wrapper terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesWrapperTerminalV2 {
    id: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: ContentId,
    link_semantic_after: ContentId,
    wrapper_admission_receipt_id: ContentId,
    owner_terminal_receipt_id: ContentId,
    product_terminal_projection: SeriesLinkObligationTerminalProjectionV2,
    structured_root_account: Pubkey,
    structured_root_semantic_id: ContentId,
    structured_root_data_id: ContentId,
}

impl AuthenticatedSeriesWrapperTerminalV2 {
    pub(crate) const fn id(self) -> ContentId { self.id }
    pub(crate) const fn link_account(self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_before(self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_semantic_before(self) -> ContentId { self.link_semantic_before }
    pub(crate) const fn link_semantic_after(self) -> ContentId { self.link_semantic_after }
    pub(crate) const fn wrapper_admission_receipt_id(self) -> ContentId {
        self.wrapper_admission_receipt_id
    }
    pub(crate) const fn owner_terminal_receipt_id(self) -> ContentId {
        self.owner_terminal_receipt_id
    }
    pub(crate) const fn product_terminal_projection(
        self,
    ) -> SeriesLinkObligationTerminalProjectionV2 {
        self.product_terminal_projection
    }
    pub(crate) const fn structured_root_account(self) -> Pubkey {
        self.structured_root_account
    }
    pub(crate) const fn structured_root_semantic_id(self) -> ContentId {
        self.structured_root_semantic_id
    }
    pub(crate) const fn structured_root_data_id(self) -> ContentId {
        self.structured_root_data_id
    }
}

/// Exhaustive current reason one pinned Failure session may release LinkV2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureSessionReleaseDispositionV3 {
    /// ResolutionV5 and Product activation were persisted before release.
    Resolved,
    /// The finite liveness schedule exhausted without a resolution.
    Exhausted,
    /// The canonical Source attempt proved physical absence.
    SourceAbsent,
    /// The canonical Source attempt produced a typed refusal.
    SourceRefused,
}

impl FailureSessionReleaseDispositionV3 {
    pub(crate) const fn wire_byte(self) -> u8 {
        match self {
            Self::Resolved => 1,
            Self::Exhausted => 2,
            Self::SourceAbsent => 3,
            Self::SourceRefused => 4,
        }
    }

    const fn requires_writable_root(self) -> bool {
        matches!(self, Self::Resolved)
    }
}

/// Default-refusing current Failure Begin owner for the sole exclusive pin.
pub(crate) trait AuthenticatedSeriesFailureSessionBeginV3 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_failure_session_begin_v3(
        &self,
        _root_account: Pubkey,
        _root_authentication_id: ContentId,
        _link_account: Pubkey,
        _link_authentication_id: ContentId,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _begin_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Hostile postwrite receipt for one exclusive current Failure session pin.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesFailureSessionPinV2 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: SeriesMarketLinkV2Id,
    link_semantic_after: SeriesMarketLinkV2Id,
    begin_admission_receipt_id: ContentId,
    session_binding_id: ContentId,
}

impl AuthenticatedSeriesFailureSessionPinV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_before(&self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(&self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_semantic_before(&self) -> SeriesMarketLinkV2Id {
        self.link_semantic_before
    }
    pub(crate) const fn link_semantic_after(&self) -> SeriesMarketLinkV2Id {
        self.link_semantic_after
    }
    pub(crate) const fn begin_admission_receipt_id(&self) -> ContentId {
        self.begin_admission_receipt_id
    }
    pub(crate) const fn session_binding_id(&self) -> ContentId { self.session_binding_id }
}

/// Exact current root/link prestate retained until one typed Failure release.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedWritableFailureSessionReleaseLinkV3 {
    id: ContentId,
    disposition: FailureSessionReleaseDispositionV3,
    root_account: Pubkey,
    root_owner_program: Pubkey,
    root_observed_lamports: u64,
    root_data_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    link_account: Pubkey,
    link_owner_program: Pubkey,
    link_observed_lamports: u64,
    link_data_id: ContentId,
    link_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV2Id,
    market_binding_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    transition_sequence: u64,
    failure_sessions_started: u32,
    failure_session_transcript_id: ContentId,
}

impl AuthenticatedWritableFailureSessionReleaseLinkV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn disposition(&self) -> FailureSessionReleaseDispositionV3 {
        self.disposition
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_owner_program(&self) -> Pubkey { self.root_owner_program }
    pub(crate) const fn root_observed_lamports(&self) -> u64 { self.root_observed_lamports }
    pub(crate) const fn root_data_id(&self) -> ContentId { self.root_data_id }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId { self.root_semantic_id }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_owner_program(&self) -> Pubkey { self.link_owner_program }
    pub(crate) const fn link_observed_lamports(&self) -> u64 { self.link_observed_lamports }
    pub(crate) const fn link_data_id(&self) -> ContentId { self.link_data_id }
    pub(crate) const fn link_authentication_id(&self) -> ContentId {
        self.link_authentication_id
    }
    pub(crate) const fn link_semantic_id(&self) -> SeriesMarketLinkV2Id {
        self.link_semantic_id
    }
    pub(crate) const fn market_binding_id(&self) -> ContentId { self.market_binding_id }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.ordinal }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn source_occurrence_id(&self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }
    pub(crate) const fn transition_sequence(&self) -> u64 { self.transition_sequence }
    pub(crate) const fn failure_sessions_started(&self) -> u32 {
        self.failure_sessions_started
    }
    pub(crate) const fn session_binding_id(&self) -> ContentId {
        self.failure_session_transcript_id
    }
}

/// Default-refusing exact current Failure archive/reset owner.
pub(crate) trait AuthenticatedSeriesFailureArchivePostwriteV3 {
    fn archive_postwrite_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn append_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn reset_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn market_instance_id(&self) -> Outcome<MarketInstanceV2Id> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn generation(&self) -> Outcome<u64> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn source_occurrence_id(&self) -> Outcome<SourceOccurrenceV1Id> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn session_binding_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn session_terminal_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn release_link_preauthorization_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn release_disposition(&self) -> Outcome<FailureSessionReleaseDispositionV3> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_failure_archive_release_postwrite_v3(
        &self,
        _archive_postwrite_id: ContentId,
        _append_receipt_id: ContentId,
        _reset_receipt_id: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _session_binding_id: ContentId,
        _session_terminal_receipt_id: ContentId,
        _disposition: FailureSessionReleaseDispositionV3,
        _release_link_preauthorization_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Hostile postwrite receipt for one exact current Failure pin release.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesFailureSessionReleaseV3 {
    id: ContentId,
    disposition: FailureSessionReleaseDispositionV3,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: SeriesMarketLinkV2Id,
    link_semantic_after: SeriesMarketLinkV2Id,
    transition_sequence_before: u64,
    transition_sequence_after: u64,
    failure_session_transcript_before: ContentId,
    failure_session_transcript_after: ContentId,
    session_terminal_receipt_id: ContentId,
    archive_postwrite_id: ContentId,
    append_receipt_id: ContentId,
    reset_receipt_id: ContentId,
    release_link_preauthorization_id: ContentId,
}

impl AuthenticatedSeriesFailureSessionReleaseV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn disposition(&self) -> FailureSessionReleaseDispositionV3 {
        self.disposition
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_before(&self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(&self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_semantic_before(&self) -> SeriesMarketLinkV2Id {
        self.link_semantic_before
    }
    pub(crate) const fn link_semantic_after(&self) -> SeriesMarketLinkV2Id {
        self.link_semantic_after
    }
    pub(crate) const fn transition_sequence_before(&self) -> u64 {
        self.transition_sequence_before
    }
    pub(crate) const fn transition_sequence_after(&self) -> u64 {
        self.transition_sequence_after
    }
    pub(crate) const fn failure_session_transcript_before(&self) -> ContentId {
        self.failure_session_transcript_before
    }
    pub(crate) const fn failure_session_transcript_after(&self) -> ContentId {
        self.failure_session_transcript_after
    }
    pub(crate) const fn session_terminal_receipt_id(&self) -> ContentId {
        self.session_terminal_receipt_id
    }
    pub(crate) const fn archive_postwrite_id(&self) -> ContentId { self.archive_postwrite_id }
    pub(crate) const fn append_receipt_id(&self) -> ContentId { self.append_receipt_id }
    pub(crate) const fn reset_receipt_id(&self) -> ContentId { self.reset_receipt_id }
    pub(crate) const fn release_link_preauthorization_id(&self) -> ContentId {
        self.release_link_preauthorization_id
    }
}

/// Default-refusing current Fractional founding postwrite owner.
pub(crate) trait AuthenticatedProductFractionalFamilyAdmissionOwnerV2 {
    fn admission_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn verification_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn postwrite_authentication_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn policy_state_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn ledger_state_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn claim_ledger_before_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn claim_ledger_after_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn claim_ledger_latch_transition_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_fractional_family_admission_owner_v2(
        &self,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _policy_account: Pubkey,
        _policy_state_id: ContentId,
        _ledger_account: Pubkey,
        _ledger_state_id: ContentId,
        _claim_ledger_account: Pubkey,
        _claim_ledger_before_id: ContentId,
        _claim_ledger_after_id: ContentId,
        _claim_ledger_latch_transition_id: ContentId,
        _claim_issuance_binding_id: ContentId,
        _runtime_release_id: ContentId,
        _capability_profile_id: ContentId,
        _resolution_account: Pubkey,
        _resolution_semantic_id: ContentId,
        _resolution_data_id: ContentId,
        _native_claim_basis_id: ContentId,
        _admission_receipt_id: ContentId,
        _verification_id: ContentId,
        _postwrite_authentication_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Default-refusing current Fractional terminal postwrite owner.
pub(crate) trait AuthenticatedProductFractionalFamilyTerminalOwnerV2 {
    fn terminal_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn verification_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn postwrite_authentication_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn policy_terminal_state_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn ledger_terminal_state_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn claim_ledger_post_state_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn claim_ledger_transition_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn fractional_release_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn claim_release_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    fn rent_disposition_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_fractional_family_terminal_owner_v2(
        &self,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _policy_account: Pubkey,
        _policy_terminal_state_id: ContentId,
        _ledger_account: Pubkey,
        _ledger_terminal_state_id: ContentId,
        _claim_ledger_account: Pubkey,
        _claim_ledger_post_state_id: ContentId,
        _claim_ledger_transition_id: ContentId,
        _fractional_release_id: ContentId,
        _capability_profile_id: ContentId,
        _claim_release_receipt_id: ContentId,
        _rent_disposition_id: ContentId,
        _resolution_account: Pubkey,
        _resolution_semantic_id: ContentId,
        _resolution_data_id: ContentId,
        _native_claim_basis_id: ContentId,
        _terminal_receipt_id: ContentId,
        _verification_id: ContentId,
        _postwrite_authentication_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Hostile root postwrite for the sole current Fractional admission.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFractionalFamilyAdmissionV2 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
    admission_receipt_id: ContentId,
    verification_id: ContentId,
    postwrite_authentication_id: ContentId,
    policy_state_id: ContentId,
    ledger_state_id: ContentId,
    claim_ledger_before_id: ContentId,
    claim_ledger_after_id: ContentId,
    claim_ledger_latch_transition_id: ContentId,
}

impl AuthenticatedProductFractionalFamilyAdmissionV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_before(&self) -> ContentId {
        self.root_authentication_before
    }
    pub(crate) const fn root_authentication_after(&self) -> ContentId {
        self.root_authentication_after
    }
    pub(crate) const fn root_semantic_before(&self) -> ContentId {
        self.root_semantic_before
    }
    pub(crate) const fn root_semantic_after(&self) -> ContentId {
        self.root_semantic_after
    }
    pub(crate) const fn admission_receipt_id(&self) -> ContentId {
        self.admission_receipt_id
    }
    pub(crate) const fn verification_id(&self) -> ContentId { self.verification_id }
    pub(crate) const fn postwrite_authentication_id(&self) -> ContentId {
        self.postwrite_authentication_id
    }
    pub(crate) const fn policy_state_id(&self) -> ContentId { self.policy_state_id }
    pub(crate) const fn ledger_state_id(&self) -> ContentId { self.ledger_state_id }
    pub(crate) const fn claim_ledger_before_id(&self) -> ContentId {
        self.claim_ledger_before_id
    }
    pub(crate) const fn claim_ledger_after_id(&self) -> ContentId {
        self.claim_ledger_after_id
    }
    pub(crate) const fn claim_ledger_latch_transition_id(&self) -> ContentId {
        self.claim_ledger_latch_transition_id
    }
}

/// Hostile root postwrite for the sole current Fractional terminal.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFractionalFamilyTerminalV2 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
    terminal_receipt_id: ContentId,
    verification_id: ContentId,
    postwrite_authentication_id: ContentId,
    policy_terminal_state_id: ContentId,
    ledger_terminal_state_id: ContentId,
    claim_ledger_post_state_id: ContentId,
    claim_ledger_transition_id: ContentId,
    fractional_release_id: ContentId,
    claim_release_receipt_id: ContentId,
    rent_disposition_id: ContentId,
}

impl AuthenticatedProductFractionalFamilyTerminalV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_before(&self) -> ContentId {
        self.root_authentication_before
    }
    pub(crate) const fn root_authentication_after(&self) -> ContentId {
        self.root_authentication_after
    }
    pub(crate) const fn root_semantic_before(&self) -> ContentId {
        self.root_semantic_before
    }
    pub(crate) const fn root_semantic_after(&self) -> ContentId {
        self.root_semantic_after
    }
    pub(crate) const fn terminal_receipt_id(&self) -> ContentId {
        self.terminal_receipt_id
    }
    pub(crate) const fn verification_id(&self) -> ContentId { self.verification_id }
    pub(crate) const fn postwrite_authentication_id(&self) -> ContentId {
        self.postwrite_authentication_id
    }
    pub(crate) const fn policy_terminal_state_id(&self) -> ContentId {
        self.policy_terminal_state_id
    }
    pub(crate) const fn ledger_terminal_state_id(&self) -> ContentId {
        self.ledger_terminal_state_id
    }
    pub(crate) const fn claim_ledger_post_state_id(&self) -> ContentId {
        self.claim_ledger_post_state_id
    }
    pub(crate) const fn claim_ledger_transition_id(&self) -> ContentId {
        self.claim_ledger_transition_id
    }
    pub(crate) const fn fractional_release_id(&self) -> ContentId {
        self.fractional_release_id
    }
    pub(crate) const fn claim_release_receipt_id(&self) -> ContentId {
        self.claim_release_receipt_id
    }
    pub(crate) const fn rent_disposition_id(&self) -> ContentId { self.rent_disposition_id }
}

struct ExactFractionalFamilyAuthorityV2 {
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    fractional_root_id: ContentId,
    sequence: u32,
    receipt_id: ContentId,
    terminal: bool,
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactFractionalFamilyAuthorityV2 {
    fn authenticate_admission(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if self.terminal
            || family != MarketFamilyV1::Fractional
            || current.binding().market_instance_id != self.market_instance_id
            || current.binding().generation != self.generation
            || family_root_id != self.fractional_root_id
            || family_admission_sequence != self.sequence
            || admission_receipt_id != self.receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticate_terminal(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if !self.terminal
            || family != MarketFamilyV1::Fractional
            || current.binding().market_instance_id != self.market_instance_id
            || current.binding().generation != self.generation
            || family_root_id != self.fractional_root_id
            || family_terminal_sequence != self.sequence
            || terminal_receipt_id != self.receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Consume the exact current a4/a5/ClaimLedger postwrite and admit Fractional
/// in RootV2. No raw root successor or caller-shaped family receipt is exposed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_family_admission_postwrite_v2<'next, A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV2<'_>,
    owner: &A,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
    successor_output: &mut MarketLifecycleRootV2,
    rebound_output: &'next mut MarketLifecycleRootAccountV2,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV2<'next>,
    AuthenticatedProductFractionalFamilyAdmissionV2,
)>
where
    A: AuthenticatedProductFractionalFamilyAdmissionOwnerV2 + ?Sized,
{
    let current = authenticated.state();
    let binding = current.binding();
    let family = current.product_families().family(MarketFamilyV1::Fractional);
    let fractional_root_id = current.product_families().binding()
        .family_root_id(MarketFamilyV1::Fractional);
    let schedule_id = schedule.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph.id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy_account = graph.account(MarketFoundationSlotV3::FractionalPolicy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let ledger_account = graph.account(MarketFoundationSlotV3::FractionalLedger)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger_account = graph.account(MarketFoundationSlotV3::ClaimLedger)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let resolution_account = graph.account(MarketFoundationSlotV3::ResolutionV5)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let admission_receipt_id = owner.admission_receipt_id()?;
    let verification_id = owner.verification_id()?;
    let postwrite_authentication_id = owner.postwrite_authentication_id()?;
    let policy_state_id = owner.policy_state_id()?;
    let ledger_state_id = owner.ledger_state_id()?;
    let claim_ledger_before_id = owner.claim_ledger_before_id()?;
    let claim_ledger_after_id = owner.claim_ledger_after_id()?;
    let claim_ledger_latch_transition_id = owner.claim_ledger_latch_transition_id()?;
    for id in [admission_receipt_id, verification_id, postwrite_authentication_id,
        policy_state_id, ledger_state_id, claim_ledger_before_id, claim_ledger_after_id,
        claim_ledger_latch_transition_id, current.resolution_semantic_id(),
        current.resolution_data_id(), current.resolution_activation_receipt_id()] {
        require_live(id)?;
    }
    require(authenticated.is_writable() && root_account.is_writable
        && current.phase() == MarketLifecyclePhaseV2::Active
        && binding.foundation_schedule_id == schedule_id
        && binding.foundation_account_graph_id == graph_id
        && graph.market_instance_id == binding.market_instance_id
        && graph.generation == binding.generation
        && fractional_root_id == policy_account
        && binding.resolution_account_id == resolution_account
        && family.status() == MarketFamilyStatusV1::EnabledNeverFounded
        && family.counts().admitted == 0 && family.counts().live == 0
        && family.counts().terminal == 0
        && policy_account != ledger_account && policy_account != claim_ledger_account
        && ledger_account != claim_ledger_account
        && policy_state_id != ledger_state_id
        && claim_ledger_before_id != claim_ledger_after_id
        && admission_receipt_id != verification_id
        && admission_receipt_id != postwrite_authentication_id
        && verification_id != postwrite_authentication_id,
        ClutchError::MismatchedState)?;
    owner.authenticate_product_fractional_family_admission_owner_v2(
        binding.market_instance_id, binding.generation,
        Pubkey::new_from_array(policy_account.bytes()), policy_state_id,
        Pubkey::new_from_array(ledger_account.bytes()), ledger_state_id,
        Pubkey::new_from_array(claim_ledger_account.bytes()), claim_ledger_before_id,
        claim_ledger_after_id, claim_ledger_latch_transition_id,
        binding.claim_issuance_binding_id, binding.registry_release_id,
        binding.capability_profile_id, Pubkey::new_from_array(resolution_account.bytes()),
        current.resolution_semantic_id(), current.resolution_data_id(),
        binding.native_claim_basis_id, admission_receipt_id, verification_id,
        postwrite_authentication_id)?;
    let semantic_before = current.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = authenticated.authentication_id();
    let sequence = family.counts().admitted;
    let authority = ExactFractionalFamilyAuthorityV2 {
        market_instance_id: binding.market_instance_id, generation: binding.generation,
        fractional_root_id, sequence, receipt_id: admission_receipt_id, terminal: false,
    };
    current.admit_product_family_child_into(
        &authority, MarketFamilyV1::Fractional, sequence, admission_receipt_id,
        successor_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_market_lifecycle_root_v2(
        program_id, root_account, authenticated, successor_output, rebound_output)?;
    let semantic_after = rebound.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let after = rebound.state().product_families().family(MarketFamilyV1::Fractional);
    require(after.status() == MarketFamilyStatusV1::Live
        && after.counts().admitted == 1 && after.counts().live == 1
        && after.counts().terminal == 0,
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        PRODUCT_FRACTIONAL_ADMISSION_AUTHENTICATION_DOMAIN_V2, program_id.as_ref(),
        root_account.key.as_ref(), &authentication_before.bytes(),
        &rebound.authentication_id().bytes(), &semantic_before.bytes(),
        &semantic_after.bytes(), &admission_receipt_id.bytes(), &verification_id.bytes(),
        &postwrite_authentication_id.bytes(), &policy_state_id.bytes(),
        &ledger_state_id.bytes(), &claim_ledger_before_id.bytes(),
        &claim_ledger_after_id.bytes(), &claim_ledger_latch_transition_id.bytes(),
        &binding.market_instance_id.bytes(), &binding.generation.to_le_bytes(),
        &schedule_id.bytes(), &graph_id.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedProductFractionalFamilyAdmissionV2 {
        id, root_account: *root_account.key, root_authentication_before: authentication_before,
        root_authentication_after: rebound.authentication_id(), root_semantic_before: semantic_before,
        root_semantic_after: semantic_after, admission_receipt_id, verification_id,
        postwrite_authentication_id, policy_state_id, ledger_state_id, claim_ledger_before_id,
        claim_ledger_after_id, claim_ledger_latch_transition_id,
    }))
}

/// Consume the exact move-only physical a4/a5 terminal receipt and latch its
/// terminal states in RootV2 only after the family rent has been disposed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_fractional_family_terminal_postwrite_v2<'next, A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV2<'_>,
    owner: &A,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
    successor_output: &mut MarketLifecycleRootV2,
    rebound_output: &'next mut MarketLifecycleRootAccountV2,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV2<'next>,
    AuthenticatedProductFractionalFamilyTerminalV2,
)>
where
    A: AuthenticatedProductFractionalFamilyTerminalOwnerV2 + ?Sized,
{
    let current = authenticated.state();
    let binding = current.binding();
    let family = current.product_families().family(MarketFamilyV1::Fractional);
    let fractional_root_id = current.product_families().binding()
        .family_root_id(MarketFamilyV1::Fractional);
    let schedule_id = schedule.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph.id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy_account = graph.account(MarketFoundationSlotV3::FractionalPolicy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let ledger_account = graph.account(MarketFoundationSlotV3::FractionalLedger)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger_account = graph.account(MarketFoundationSlotV3::ClaimLedger)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let resolution_account = graph.account(MarketFoundationSlotV3::ResolutionV5)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_receipt_id = owner.terminal_receipt_id()?;
    let verification_id = owner.verification_id()?;
    let postwrite_authentication_id = owner.postwrite_authentication_id()?;
    let policy_terminal_state_id = owner.policy_terminal_state_id()?;
    let ledger_terminal_state_id = owner.ledger_terminal_state_id()?;
    let claim_ledger_post_state_id = owner.claim_ledger_post_state_id()?;
    let claim_ledger_transition_id = owner.claim_ledger_transition_id()?;
    let fractional_release_id = owner.fractional_release_id()?;
    let claim_release_receipt_id = owner.claim_release_receipt_id()?;
    let rent_disposition_id = owner.rent_disposition_id()?;
    for id in [terminal_receipt_id, verification_id, postwrite_authentication_id,
        policy_terminal_state_id, ledger_terminal_state_id, claim_ledger_post_state_id,
        claim_ledger_transition_id, fractional_release_id, claim_release_receipt_id,
        rent_disposition_id, current.resolution_semantic_id(), current.resolution_data_id(),
        current.resolution_activation_receipt_id()] {
        require_live(id)?;
    }
    require(authenticated.is_writable() && root_account.is_writable
        && matches!(current.phase(), MarketLifecyclePhaseV2::Active | MarketLifecyclePhaseV2::Retiring)
        && binding.foundation_schedule_id == schedule_id
        && binding.foundation_account_graph_id == graph_id
        && graph.market_instance_id == binding.market_instance_id
        && graph.generation == binding.generation
        && fractional_root_id == policy_account
        && binding.resolution_account_id == resolution_account
        && fractional_release_id == binding.registry_release_id
        && family.status() == MarketFamilyStatusV1::Live
        && family.counts().admitted == 1 && family.counts().live == 1
        && family.counts().terminal == 0
        && policy_account != ledger_account && policy_account != claim_ledger_account
        && ledger_account != claim_ledger_account
        && policy_terminal_state_id != ledger_terminal_state_id
        && terminal_receipt_id != verification_id
        && terminal_receipt_id != postwrite_authentication_id
        && terminal_receipt_id != claim_release_receipt_id
        && verification_id != postwrite_authentication_id
        && verification_id != claim_release_receipt_id
        && postwrite_authentication_id != claim_release_receipt_id,
        ClutchError::MismatchedState)?;
    owner.authenticate_product_fractional_family_terminal_owner_v2(
        binding.market_instance_id, binding.generation,
        Pubkey::new_from_array(policy_account.bytes()), policy_terminal_state_id,
        Pubkey::new_from_array(ledger_account.bytes()), ledger_terminal_state_id,
        Pubkey::new_from_array(claim_ledger_account.bytes()), claim_ledger_post_state_id,
        claim_ledger_transition_id, fractional_release_id, binding.capability_profile_id,
        claim_release_receipt_id,
        rent_disposition_id, Pubkey::new_from_array(resolution_account.bytes()),
        current.resolution_semantic_id(), current.resolution_data_id(),
        binding.native_claim_basis_id, terminal_receipt_id, verification_id,
        postwrite_authentication_id)?;
    let semantic_before = current.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = authenticated.authentication_id();
    let sequence = family.counts().terminal;
    let authority = ExactFractionalFamilyAuthorityV2 {
        market_instance_id: binding.market_instance_id, generation: binding.generation,
        fractional_root_id, sequence, receipt_id: terminal_receipt_id, terminal: true,
    };
    current.terminalize_fractional_family_into(
        &authority, sequence, terminal_receipt_id,
        policy_terminal_state_id, ledger_terminal_state_id, successor_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_market_lifecycle_root_v2(
        program_id, root_account, authenticated, successor_output, rebound_output)?;
    let semantic_after = rebound.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let after = rebound.state().product_families().family(MarketFamilyV1::Fractional);
    require(after.counts().admitted == 1 && after.counts().live == 0
        && after.counts().terminal == 1,
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        PRODUCT_FRACTIONAL_TERMINAL_AUTHENTICATION_DOMAIN_V2, program_id.as_ref(),
        root_account.key.as_ref(), &authentication_before.bytes(),
        &rebound.authentication_id().bytes(), &semantic_before.bytes(),
        &semantic_after.bytes(), &terminal_receipt_id.bytes(), &verification_id.bytes(),
        &postwrite_authentication_id.bytes(), &policy_terminal_state_id.bytes(),
        &ledger_terminal_state_id.bytes(), &claim_ledger_post_state_id.bytes(),
        &claim_ledger_transition_id.bytes(), &fractional_release_id.bytes(),
        &claim_release_receipt_id.bytes(), &rent_disposition_id.bytes(),
        &binding.market_instance_id.bytes(), &binding.generation.to_le_bytes(),
        &schedule_id.bytes(), &graph_id.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedProductFractionalFamilyTerminalV2 {
        id, root_account: *root_account.key, root_authentication_before: authentication_before,
        root_authentication_after: rebound.authentication_id(), root_semantic_before: semantic_before,
        root_semantic_after: semantic_after, terminal_receipt_id, verification_id,
        postwrite_authentication_id, policy_terminal_state_id, ledger_terminal_state_id,
        claim_ledger_post_state_id, claim_ledger_transition_id, fractional_release_id,
        claim_release_receipt_id, rent_disposition_id,
    }))
}

/// Private raw RootV2 writer. All crate-visible callers derive one exact legal
/// successor and hostile-reauthenticate the complete current and postwrite frames.
fn write_market_lifecycle_root_v2<'next>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV2<'_>,
    successor: &MarketLifecycleRootV2,
    rebound_output: &'next mut MarketLifecycleRootAccountV2,
) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>> {
    let binding = authenticated.state().binding_ref();
    require(account.is_writable && *account.key == authenticated.account()
        && account.owner == program_id && successor.binding_ref() == binding,
        ClutchError::MismatchedState)?;
    let live = authenticate_market_lifecycle_root_v2(
        program_id, account, binding.market_instance_id, binding.generation, true,
        rebound_output)?;
    require(live.account() == authenticated.account()
        && live.owner_program() == authenticated.owner_program()
        && live.value() == authenticated.value()
        && live.observed_lamports() == authenticated.observed_lamports()
        && live.data_id() == authenticated.data_id()
        && live.authentication_id() == authenticated.authentication_id(),
        ClutchError::MismatchedState)?;
    let rent_principal_lamports = authenticated.value().rent_principal_lamports;
    let stored_bump = authenticated.value().stored_bump;
    let mut data = account.try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV2::encode_parts(
        successor, rent_principal_lamports, stored_bump, &mut data)?;
    drop(data);
    let rebound = authenticate_market_lifecycle_root_v2(
        program_id, account, binding.market_instance_id, binding.generation, true,
        rebound_output)?;
    require(rebound.state() == successor
        && rebound.value().rent_principal_lamports == rent_principal_lamports
        && rebound.value().stored_bump == stored_bump,
        ClutchError::MismatchedState)?;
    Ok(rebound)
}

/// Exact retained zero-data preallocation under the 47-slot owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketFoundationPreallocationV3 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    slot: MarketFoundationSlotV3,
    account: Pubkey,
    foundation_schedule_id: ContentId,
    foundation_account_graph_id: ContentId,
    foundation_transcript_id: ContentId,
    principal_lamports: u64,
    donation_lamports: u64,
    observed_balance_lamports: u64,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
}

impl AuthenticatedMarketFoundationPreallocationV3 {
    pub(crate) const fn id(self) -> ContentId { self.id }
    pub(crate) const fn root_account(self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(self) -> ContentId { self.root_authentication_id }
    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id { self.market_instance_id }
    pub(crate) const fn generation(self) -> u64 { self.generation }
    pub(crate) const fn slot(self) -> MarketFoundationSlotV3 { self.slot }
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn foundation_schedule_id(self) -> ContentId { self.foundation_schedule_id }
    pub(crate) const fn foundation_account_graph_id(self) -> ContentId { self.foundation_account_graph_id }
    pub(crate) const fn foundation_transcript_id(self) -> ContentId { self.foundation_transcript_id }
    pub(crate) const fn principal_lamports(self) -> u64 { self.principal_lamports }
    pub(crate) const fn donation_lamports(self) -> u64 { self.donation_lamports }
    pub(crate) const fn observed_balance_lamports(self) -> u64 { self.observed_balance_lamports }
    pub(crate) const fn rent_refund_owner(self) -> Pubkey { self.rent_refund_owner }
    pub(crate) const fn neutral_lamport_sink(self) -> Pubkey { self.neutral_lamport_sink }
}

/// Default-refusing current authority for the once-only RootV2 Resolution write.
///
/// The Failure/Collateral owner may implement this only on its private final
/// postwrite receipt, after the exact ResolutionV5 account and every collateral
/// liability postimage have been hostile-reauthenticated. A pure
/// [`MarketResolutionActivationV2`] or a caller-shaped collection of IDs is not
/// sufficient authority.
pub(crate) trait AuthenticatedMarketResolutionActivationWriteV2 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_market_resolution_activation_write_v2(
        &self,
        _root_account: Pubkey,
        _root_authentication_before: ContentId,
        _root_data_before: ContentId,
        _root_semantic_before: ContentId,
        _activation: MarketResolutionActivationV2,
        _slot10: AuthenticatedMarketFoundationPreallocationV3,
        _collateral_plan_receipt_id: ContentId,
        _collateral_postwrite_receipt_id: ContentId,
        _failure_resolution_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Exact RootV2 postwrite minted only by the narrow Resolution compositor.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedMarketResolutionActivationPostwriteV2 {
    id: ContentId,
    activation: MarketResolutionActivationV2,
    slot10_preallocation_id: ContentId,
    collateral_plan_receipt_id: ContentId,
    collateral_postwrite_receipt_id: ContentId,
    failure_resolution_receipt_id: ContentId,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_before: ContentId,
    root_authentication_after: ContentId,
    root_data_before: ContentId,
    root_data_after: ContentId,
    root_semantic_before: ContentId,
    root_semantic_after: ContentId,
}

impl AuthenticatedMarketResolutionActivationPostwriteV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn activation(&self) -> MarketResolutionActivationV2 { self.activation }
    pub(crate) const fn slot10_preallocation_id(&self) -> ContentId {
        self.slot10_preallocation_id
    }
    pub(crate) const fn collateral_plan_receipt_id(&self) -> ContentId {
        self.collateral_plan_receipt_id
    }
    pub(crate) const fn collateral_postwrite_receipt_id(&self) -> ContentId {
        self.collateral_postwrite_receipt_id
    }
    pub(crate) const fn failure_resolution_receipt_id(&self) -> ContentId {
        self.failure_resolution_receipt_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn root_authentication_before(&self) -> ContentId {
        self.root_authentication_before
    }
    pub(crate) const fn root_authentication_after(&self) -> ContentId {
        self.root_authentication_after
    }
    pub(crate) const fn root_data_before(&self) -> ContentId { self.root_data_before }
    pub(crate) const fn root_data_after(&self) -> ContentId { self.root_data_after }
    pub(crate) const fn root_semantic_before(&self) -> ContentId {
        self.root_semantic_before
    }
    pub(crate) const fn root_semantic_after(&self) -> ContentId { self.root_semantic_after }
}

/// Record current ResolutionV5 exactly once in the live RootV2.
///
/// `slot10` is the retained prewrite authority for the same canonical
/// Resolution account. The concrete authority must additionally prove that its
/// private Failure and Collateral postwrites are the exact ones named here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_market_resolution_activation_v2<
    'next,
    A: AuthenticatedMarketResolutionActivationWriteV2 + ?Sized,
>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV2<'_>,
    activation: MarketResolutionActivationV2,
    slot10: AuthenticatedMarketFoundationPreallocationV3,
    collateral_plan_receipt_id: ContentId,
    collateral_postwrite_receipt_id: ContentId,
    failure_resolution_receipt_id: ContentId,
    authority: &A,
    successor_output: &mut MarketLifecycleRootV2,
    rebound_output: &'next mut MarketLifecycleRootAccountV2,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV2<'next>,
    AuthenticatedMarketResolutionActivationPostwriteV2,
)> {
    require_live(collateral_plan_receipt_id)?;
    require_live(collateral_postwrite_receipt_id)?;
    require_live(failure_resolution_receipt_id)?;
    let root = authenticated.state();
    let binding = root.binding();
    let root_binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_before = root
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        authenticated.is_writable()
            && *account.key == authenticated.account()
            && root.phase() == MarketLifecyclePhaseV2::Active
            && root.resolution_semantic_id() == ContentId::ZERO
            && root.resolution_data_id() == ContentId::ZERO
            && root.resolution_activation_receipt_id() == ContentId::ZERO
            && activation.market_binding_id() == root_binding_id
            && activation.market_instance_id() == binding.market_instance_id
            && activation.generation() == binding.generation
            && activation.resolution_account_id() == binding.resolution_account_id
            && activation.failure_resolution_receipt_id() == failure_resolution_receipt_id
            && slot10.root_account() == authenticated.account()
            && slot10.root_authentication_id() == authenticated.authentication_id()
            && slot10.market_instance_id() == binding.market_instance_id
            && slot10.generation() == binding.generation
            && slot10.slot() == MarketFoundationSlotV3::ResolutionV5
            && slot10.account().to_bytes() == binding.resolution_account_id.bytes()
            && slot10.foundation_schedule_id() == binding.foundation_schedule_id.content_id()
            && slot10.foundation_account_graph_id()
                == binding.foundation_account_graph_id.content_id()
            && slot10.foundation_transcript_id() == root.foundation().transcript_id
            && slot10.principal_lamports() != 0
            && slot10.observed_balance_lamports()
                == slot10
                    .principal_lamports()
                    .checked_add(slot10.donation_lamports())
                    .ok_or(ClutchError::Arithmetic)?
            && slot10.rent_refund_owner().to_bytes() == root.capital().rent_refund_owner.bytes()
            && slot10.neutral_lamport_sink().to_bytes()
                == root.capital().neutral_lamport_sink.bytes()
            && collateral_plan_receipt_id != collateral_postwrite_receipt_id
            && collateral_plan_receipt_id != failure_resolution_receipt_id
            && collateral_postwrite_receipt_id != failure_resolution_receipt_id,
        ClutchError::MismatchedState,
    )?;
    authority.authenticate_market_resolution_activation_write_v2(
        authenticated.account(),
        authenticated.authentication_id(),
        authenticated.data_id(),
        root_semantic_before,
        activation,
        slot10,
        collateral_plan_receipt_id,
        collateral_postwrite_receipt_id,
        failure_resolution_receipt_id,
    )?;
    root.record_resolution_activation_into(activation, successor_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_market_lifecycle_root_v2(
        program_id,
        account,
        authenticated,
        successor_output,
        rebound_output,
    )?;
    let root_semantic_after = rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        rebound.state().binding() == binding
            && rebound.state().resolution_semantic_id() == activation.resolution_semantic_id()
            && rebound.state().resolution_data_id() == activation.resolution_data_id()
            && rebound.state().resolution_activation_receipt_id() == activation.id()
            && rebound.state().transition_sequence()
                == root.transition_sequence().checked_add(1).ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        MARKET_RESOLUTION_ACTIVATION_POSTWRITE_DOMAIN_V2,
        program_id.as_ref(),
        account.key.as_ref(),
        &root_binding_id.bytes(),
        &authenticated.authentication_id().bytes(),
        &rebound.authentication_id().bytes(),
        &authenticated.data_id().bytes(),
        &rebound.data_id().bytes(),
        &root_semantic_before.bytes(),
        &root_semantic_after.bytes(),
        &activation.id().bytes(),
        &slot10.id().bytes(),
        &collateral_plan_receipt_id.bytes(),
        &collateral_postwrite_receipt_id.bytes(),
        &failure_resolution_receipt_id.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedMarketResolutionActivationPostwriteV2 {
        id,
        activation,
        slot10_preallocation_id: slot10.id(),
        collateral_plan_receipt_id,
        collateral_postwrite_receipt_id,
        failure_resolution_receipt_id,
        root_account: *account.key,
        root_binding_id,
        root_authentication_before: authenticated.authentication_id(),
        root_authentication_after: rebound.authentication_id(),
        root_data_before: authenticated.data_id(),
        root_data_after: rebound.data_id(),
        root_semantic_before,
        root_semantic_after,
    }))
}

pub(crate) fn authenticate_series_registry_account_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesRegistryAccountV3> {
    require(
        !account.is_signer && !account.executable && account.is_writable == require_writable
            && account.owner == program_id && account.data_len() == SERIES_REGISTRY_ACCOUNT_BYTES_V3,
        ClutchError::MismatchedState,
    )?;
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesRegistryAccountV3::decode(&data)?;
    require(value.series_plan_id == expected_series_plan_id, ClutchError::MismatchedState)?;
    let (expected, bump) = seeds::series_registry_pda(program_id, &expected_series_plan_id.bytes());
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let observed_lamports = account.lamports();
    require(observed_lamports >= value.rent_principal_lamports, ClutchError::MismatchedState)?;
    let authentication_id = hashv(&[
        SERIES_REGISTRY_AUTHENTICATION_DOMAIN_V3, account.key.as_ref(), program_id.as_ref(),
        &data_id.bytes(), &value.series_plan_id.bytes(), &value.funding_terms_id.bytes(),
        &value.registry_release_id.bytes(), &value.capability_profile_id.bytes(),
        &value.compiler_bundle_id.bytes(), &value.rent_principal_lamports.to_le_bytes(),
        &observed_lamports.to_le_bytes(), &[value.stored_bump], &[u8::from(value.activation_consumed)],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesRegistryAccountV3 { account: *account.key, value, observed_lamports,
        writable: account.is_writable, data_id, authentication_id })
}

/// Project the exact capability references from one already hostile-authenticated
/// RegistryV3. This is the only current replacement for the withdrawn V2 refs.
pub(crate) fn authenticate_series_registry_capability_refs_v3(
    registry: AuthenticatedSeriesRegistryAccountV3,
) -> Outcome<AuthenticatedSeriesRegistryCapabilityRefsV3> {
    require(!registry.is_writable(), ClutchError::UnexpectedWritable)?;
    let value = registry.value();
    let id = hashv(&[
        SERIES_REGISTRY_CAPABILITY_REFS_DOMAIN_V3, registry.account().as_ref(),
        &registry.authentication_id().bytes(), &value.series_plan_id.bytes(),
        &value.funding_terms_id.bytes(), &value.registry_release_id.bytes(),
        &value.capability_profile_id.bytes(), &value.compiler_bundle_id.bytes(),
        &[u8::from(value.activation_consumed)],
    ]);
    require_live(id)?;
    Ok(AuthenticatedSeriesRegistryCapabilityRefsV3 {
        id, series_registry_account: registry.account(),
        series_registry_authentication_id: registry.authentication_id(),
        series_plan_id: value.series_plan_id, funding_terms_id: value.funding_terms_id,
        registry_release_id: value.registry_release_id,
        capability_profile_id: value.capability_profile_id,
        compiler_bundle_id: value.compiler_bundle_id,
        activation_consumed: value.activation_consumed,
    })
}

/// Join exact RegistryV3 bytes to the live loader and immutable Release/Profile.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_registry_capability_v4(
    program_id: &Pubkey,
    registry: AuthenticatedSeriesRegistryAccountV3,
    program_account: &AccountInfo<'_>,
    programdata_account: &AccountInfo<'_>,
    release_artifact: &AccountInfo<'_>,
    profile_artifact: &AccountInfo<'_>,
) -> Outcome<AuthenticatedRegistryCapabilityV4> {
    let refs = authenticate_series_registry_capability_refs_v3(registry)?;
    authenticate_registry_capability_from_refs_v4(
        program_id, refs, program_account, programdata_account, release_artifact,
        profile_artifact)
}

/// Join exact RegistryV3 references to the current loader and ReleaseV2/ProfileV4.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_registry_capability_from_refs_v4(
    program_id: &Pubkey,
    refs: AuthenticatedSeriesRegistryCapabilityRefsV3,
    program_account: &AccountInfo<'_>,
    programdata_account: &AccountInfo<'_>,
    release_artifact: &AccountInfo<'_>,
    profile_artifact: &AccountInfo<'_>,
) -> Outcome<AuthenticatedRegistryCapabilityV4> {
    let release = authenticate_registry_capability_for_registration_v3(
        program_id, release_artifact, profile_artifact, refs.registry_release_id,
        refs.capability_profile_id, program_account, programdata_account)?;
    let projection = release.projection();
    require(projection.registry_release_id == refs.registry_release_id
        && projection.capability_profile_id == refs.capability_profile_id
        && refs.compiler_bundle_id.content_id() != refs.funding_terms_id.content_id()
        && refs.series_registry_account != release.program_account()
        && refs.series_registry_account != release.programdata_account()
        && refs.series_registry_account != release.release_artifact_account()
        && refs.series_registry_account != release.profile_artifact_account(),
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        REGISTRY_CAPABILITY_AUTHENTICATION_DOMAIN_V4, program_id.as_ref(),
        refs.series_registry_account.as_ref(), &refs.series_registry_authentication_id.bytes(),
        &refs.id.bytes(), &refs.series_plan_id.bytes(), &refs.funding_terms_id.bytes(),
        &refs.compiler_bundle_id.bytes(), &refs.registry_release_id.bytes(),
        &refs.capability_profile_id.bytes(), &[u8::from(refs.activation_consumed)],
        release.program_account().as_ref(),
        release.programdata_account().as_ref(), release.release_artifact_account().as_ref(),
        release.profile_artifact_account().as_ref(), &release.programdata_sha256().bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedRegistryCapabilityV4 {
        id, series_registry_account: refs.series_registry_account,
        series_registry_authentication_id: refs.series_registry_authentication_id,
        series_plan_id: refs.series_plan_id, funding_terms_id: refs.funding_terms_id,
        compiler_bundle_id: refs.compiler_bundle_id,
        activation_consumed: refs.activation_consumed,
        program_account: release.program_account(),
        programdata_account: release.programdata_account(),
        release_artifact_account: release.release_artifact_account(),
        profile_artifact_account: release.profile_artifact_account(),
        release: release.release(), profile: release.profile(), projection,
        programdata_sha256: release.programdata_sha256(),
    })
}

/// Hostile-authenticate the exact BundleV7-bound RegistryV4 account.
pub(crate) fn authenticate_series_registry_account_v4(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesRegistryAccountV4> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == SERIES_REGISTRY_ACCOUNT_BYTES_V4,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesRegistryAccountV4::decode(&data)?;
    require(
        value.series_plan_id == expected_series_plan_id,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::series_registry_pda(program_id, &expected_series_plan_id.bytes());
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let observed_lamports = account.lamports();
    require(
        observed_lamports >= value.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let authentication_id = hashv(&[
        SERIES_REGISTRY_AUTHENTICATION_DOMAIN_V4,
        account.key.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &value.series_plan_id.bytes(),
        &value.funding_terms_id.bytes(),
        &value.registry_release_id.bytes(),
        &value.capability_profile_id.bytes(),
        &value.compiler_bundle_id.bytes(),
        &value.rent_principal_lamports.to_le_bytes(),
        &observed_lamports.to_le_bytes(),
        &[value.stored_bump],
        &[u8::from(value.activation_consumed)],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesRegistryAccountV4 {
        account: *account.key,
        value,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        authentication_id,
    })
}

/// Consume hostile RegistryV4 bytes into the sole exact capability references.
pub(crate) fn authenticate_series_registry_capability_refs_v4(
    registry: AuthenticatedSeriesRegistryAccountV4,
) -> Outcome<AuthenticatedSeriesRegistryCapabilityRefsV4> {
    require(!registry.is_writable(), ClutchError::UnexpectedWritable)?;
    let value = registry.value();
    let id = hashv(&[
        SERIES_REGISTRY_CAPABILITY_REFS_DOMAIN_V4,
        registry.account().as_ref(),
        &registry.authentication_id().bytes(),
        &value.series_plan_id.bytes(),
        &value.funding_terms_id.bytes(),
        &value.registry_release_id.bytes(),
        &value.capability_profile_id.bytes(),
        &value.compiler_bundle_id.bytes(),
        &[u8::from(value.activation_consumed)],
    ]);
    require_live(id)?;
    Ok(AuthenticatedSeriesRegistryCapabilityRefsV4 {
        id,
        series_registry_account: registry.account(),
        series_registry_authentication_id: registry.authentication_id(),
        series_plan_id: value.series_plan_id,
        funding_terms_id: value.funding_terms_id,
        registry_release_id: value.registry_release_id,
        capability_profile_id: value.capability_profile_id,
        compiler_bundle_id: value.compiler_bundle_id,
        activation_consumed: value.activation_consumed,
    })
}

/// Consume exact RegistryV4 references into the current loader/release authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_registry_capability_v5(
    program_id: &Pubkey,
    registry: AuthenticatedSeriesRegistryAccountV4,
    program_account: &AccountInfo<'_>,
    programdata_account: &AccountInfo<'_>,
    release_artifact: &AccountInfo<'_>,
    profile_artifact: &AccountInfo<'_>,
) -> Outcome<AuthenticatedRegistryCapabilityV5> {
    let refs = authenticate_series_registry_capability_refs_v4(registry)?;
    authenticate_registry_capability_from_refs_v5(
        program_id,
        refs,
        program_account,
        programdata_account,
        release_artifact,
        profile_artifact,
    )
}

/// Join move-only RegistryV4 refs to exact ReleaseV2/ProfileV4 ProgramData.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_registry_capability_from_refs_v5(
    program_id: &Pubkey,
    refs: AuthenticatedSeriesRegistryCapabilityRefsV4,
    program_account: &AccountInfo<'_>,
    programdata_account: &AccountInfo<'_>,
    release_artifact: &AccountInfo<'_>,
    profile_artifact: &AccountInfo<'_>,
) -> Outcome<AuthenticatedRegistryCapabilityV5> {
    let release = authenticate_registry_capability_for_registration_v3(
        program_id,
        release_artifact,
        profile_artifact,
        refs.registry_release_id(),
        refs.capability_profile_id(),
        program_account,
        programdata_account,
    )?;
    let projection = release.projection();
    require(
        projection.registry_release_id == refs.registry_release_id()
            && projection.capability_profile_id == refs.capability_profile_id()
            && refs.compiler_bundle_id().content_id() != refs.funding_terms_id().content_id()
            && refs.series_registry_account() != release.program_account()
            && refs.series_registry_account() != release.programdata_account()
            && refs.series_registry_account() != release.release_artifact_account()
            && refs.series_registry_account() != release.profile_artifact_account(),
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        REGISTRY_CAPABILITY_AUTHENTICATION_DOMAIN_V5,
        program_id.as_ref(),
        refs.series_registry_account().as_ref(),
        &refs.series_registry_authentication_id().bytes(),
        &refs.id().bytes(),
        &refs.series_plan_id().bytes(),
        &refs.funding_terms_id().bytes(),
        &refs.compiler_bundle_id().bytes(),
        &refs.registry_release_id().bytes(),
        &refs.capability_profile_id().bytes(),
        &[u8::from(refs.activation_consumed())],
        release.program_account().as_ref(),
        release.programdata_account().as_ref(),
        release.release_artifact_account().as_ref(),
        release.profile_artifact_account().as_ref(),
        &release.programdata_sha256().bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedRegistryCapabilityV5 {
        id,
        series_registry_account: refs.series_registry_account(),
        series_registry_authentication_id: refs.series_registry_authentication_id(),
        series_plan_id: refs.series_plan_id(),
        funding_terms_id: refs.funding_terms_id(),
        compiler_bundle_id: refs.compiler_bundle_id(),
        activation_consumed: refs.activation_consumed(),
        program_account: release.program_account(),
        programdata_account: release.programdata_account(),
        release_artifact_account: release.release_artifact_account(),
        profile_artifact_account: release.profile_artifact_account(),
        release: release.release(),
        profile: release.profile(),
        projection,
        programdata_sha256: release.programdata_sha256(),
    })
}

/// Hostile-authenticate only the exact acyclic FundingV5 account coordinate.
pub(crate) fn authenticate_series_funding_account_v5(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesFundingAccountV5> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == SERIES_FUNDING_ACCOUNT_BYTES_V5,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesFundingAccountV5::decode(&data)?;
    require(
        value.state.series_plan_id == expected_series_plan_id,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::series_funding_pda(program_id, &expected_series_plan_id.bytes());
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let state_id = value
        .state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_lamports = account.lamports();
    require(
        observed_lamports >= value.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let mut vault_rent = [0u8; 40];
    for (index, principal) in value
        .collateral_vault_rent_principal_lamports
        .iter()
        .enumerate()
    {
        let at = index.checked_mul(8).ok_or(ClutchError::Arithmetic)?;
        vault_rent[at..at + 8].copy_from_slice(&principal.to_le_bytes());
    }
    let authentication_id = hashv(&[
        SERIES_FUNDING_AUTHENTICATION_DOMAIN_V5,
        account.key.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &state_id.bytes(),
        &value.rent_principal_lamports.to_le_bytes(),
        &vault_rent,
        &observed_lamports.to_le_bytes(),
        &[value.stored_bump],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesFundingAccountV5 {
        account: *account.key,
        value,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        authentication_id,
    })
}

/// Hostile-authenticate only the historical acyclic FundingV4 account coordinate.
/// Historical FundingV1-V3 bytes are refused by exact length/version decode.
pub(crate) fn authenticate_series_funding_account_v4(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesFundingAccountV4> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == SERIES_FUNDING_ACCOUNT_BYTES_V4,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesFundingAccountV4::decode(&data)?;
    require(
        value.state.series_plan_id == expected_series_plan_id,
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::series_funding_pda(program_id, &expected_series_plan_id.bytes());
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let state_id = value
        .state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_lamports = account.lamports();
    require(
        observed_lamports >= value.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let mut vault_rent = [0u8; 40];
    for (index, principal) in value
        .collateral_vault_rent_principal_lamports
        .iter()
        .enumerate()
    {
        let at = index.checked_mul(8).ok_or(ClutchError::Arithmetic)?;
        vault_rent[at..at + 8].copy_from_slice(&principal.to_le_bytes());
    }
    let authentication_id = hashv(&[
        SERIES_FUNDING_AUTHENTICATION_DOMAIN_V4,
        account.key.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &state_id.bytes(),
        &value.rent_principal_lamports.to_le_bytes(),
        &vault_rent,
        &observed_lamports.to_le_bytes(),
        &[value.stored_bump],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesFundingAccountV4 {
        account: *account.key,
        value,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        authentication_id,
    })
}

pub(crate) fn authenticate_series_lifecycle_replay_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesLifecycleReplayV2> {
    require(
        !account.is_signer && !account.executable && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesLifecycleReplayAccountV2::decode(&data)?;
    let binding = value.state.binding();
    require(binding.series_plan_id == expected_series_plan_id, ClutchError::MismatchedState)?;
    let (expected, bump) = seeds::product_series_lifecycle_replay_pda(
        program_id, &expected_series_plan_id.bytes());
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let state_id = value.state.id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding_id: SeriesLifecycleReplayBindingV2Id = binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_lamports = account.lamports();
    require(observed_lamports >= value.permanent_rent_principal_lamports,
        ClutchError::MismatchedState)?;
    let authentication_id = hashv(&[
        SERIES_LIFECYCLE_REPLAY_AUTHENTICATION_DOMAIN_V2, account.key.as_ref(), program_id.as_ref(),
        &data_id.bytes(), &state_id.bytes(), &binding_id.bytes(),
        &value.permanent_rent_principal_lamports.to_le_bytes(), &observed_lamports.to_le_bytes(),
        &[value.stored_bump],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesLifecycleReplayV2 { account: *account.key, value, observed_lamports,
        writable: account.is_writable, data_id, authentication_id })
}

/// Private raw FundingV4 writer. Only the sole reservation/completion/abort
/// compositors in this module may turn an authenticated body into a successor.
fn write_series_funding_state_v4(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesFundingAccountV4,
    successor: SeriesFundingStateV4,
) -> Outcome<AuthenticatedSeriesFundingAccountV4> {
    let before = authenticated.value();
    require(
        account.is_writable
            && *account.key == authenticated.account()
            && account.owner == program_id
            && successor.series_plan_id == before.state.series_plan_id
            && successor.funding_terms_id == before.state.funding_terms_id
            && successor.funding_quote_id == before.state.funding_quote_id
            && successor.attachment_plan_id == before.state.attachment_plan_id
            && successor.compiler_bundle_id == before.state.compiler_bundle_id
            && successor.instance_count == before.state.instance_count,
        ClutchError::MismatchedState,
    )?;
    let live = authenticate_series_funding_account_v4(
        program_id,
        account,
        before.state.series_plan_id,
        true,
    )?;
    require(live == authenticated, ClutchError::MismatchedState)?;
    let successor_account = SeriesFundingAccountV4 {
        state: successor,
        rent_principal_lamports: before.rent_principal_lamports,
        collateral_vault_rent_principal_lamports: before
            .collateral_vault_rent_principal_lamports,
        stored_bump: before.stored_bump,
    };
    let observed_lamports_before = authenticated.observed_lamports();
    let authentication_before = authenticated.authentication_id();
    let data_before = authenticated.data_id();
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        successor_account.encode(&mut data)?;
    }
    let rebound = authenticate_series_funding_account_v4(
        program_id,
        account,
        successor_account.state.series_plan_id,
        true,
    )?;
    require(
        rebound.value() == &successor_account
            && rebound.observed_lamports() == observed_lamports_before
            && rebound.authentication_id() != authentication_before
            && rebound.data_id() != data_before,
        ClutchError::MismatchedState,
    )?;
    Ok(rebound)
}

#[derive(Debug)]
struct ExactSeriesFundingReservationAuthorityV4 {
    state_before_id: SeriesFundingStateV4Id,
    binding_id: SeriesFundingReservationBindingV4Id,
    reservation_receipt_id: ContentId,
}

#[derive(Debug)]
struct ExactSeriesFundingCompletionAuthorityV4 {
    state_before_id: SeriesFundingStateV4Id,
    binding_id: SeriesFundingCompletionBindingV4Id,
    completion_receipt_id: ContentId,
}

impl AuthenticatedSeriesFundingAuthorityV4 for ExactSeriesFundingCompletionAuthorityV4 {
    fn authenticate_activation(
        &self,
        _series: &SeriesPlanV5,
        _funding_terms_id: SeriesFundingTermsV2Id,
        _compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
        _quote: &SeriesFundingQuoteV5,
        _attachment: &SeriesAttachmentPlanV5,
        _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn current_bucket(&self, _series: &SeriesPlanV5) -> clutch_product_series::Result<u64> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &SeriesFundingReservationBindingV4,
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        state: &SeriesFundingStateV4,
        binding: &SeriesFundingCompletionBindingV4,
        completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if state.id()? != self.state_before_id
            || binding.id()? != self.binding_id
            || completion_receipt_id != self.completion_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &clutch_product_series::SeriesFundingAbortBindingV4,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV4,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        _state: &SeriesFundingStateV4,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

impl AuthenticatedSeriesFundingAuthorityV4 for ExactSeriesFundingReservationAuthorityV4 {
    fn authenticate_activation(
        &self,
        _series: &SeriesPlanV5,
        _funding_terms_id: SeriesFundingTermsV2Id,
        _compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
        _quote: &SeriesFundingQuoteV5,
        _attachment: &SeriesAttachmentPlanV5,
        _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn current_bucket(&self, _series: &SeriesPlanV5) -> clutch_product_series::Result<u64> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_reservation(
        &self,
        state: &SeriesFundingStateV4,
        binding: &SeriesFundingReservationBindingV4,
        reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if state.id()? != self.state_before_id
            || binding.id()? != self.binding_id
            || reservation_receipt_id != self.reservation_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticate_pending_completion(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &clutch_product_series::SeriesFundingCompletionBindingV4,
        _completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &clutch_product_series::SeriesFundingAbortBindingV4,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV4,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        _state: &SeriesFundingStateV4,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

/// Private raw reserve half. The sole exported current founder compositor must
/// construct the binding from its retained V3 preauthorization and exact 0xba
/// capitalization; no other module can persist Pending through this function.
#[inline(never)]
fn reserve_series_funding_v4_with_binding(
    program_id: &Pubkey,
    funding_account: &AccountInfo<'_>,
    authenticated_funding: AuthenticatedSeriesFundingAccountV4,
    series: &SeriesPlanV5,
    quote: &SeriesFundingQuoteV5,
    attachment: &SeriesAttachmentPlanV5,
    binding: SeriesFundingReservationBindingV4,
    reservation_receipt_id: ContentId,
) -> Outcome<AuthenticatedProductSeriesFundingReservationV4> {
    require(
        authenticated_funding.is_writable()
            && authenticated_funding.account() == *funding_account.key
            && binding.funding_account_id.bytes() == funding_account.key.to_bytes()
            && binding.funding_account_authentication_before_id
                == authenticated_funding.authentication_id()
            && binding.funding_state_before_id
                == authenticated_funding
                    .state()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && binding.funding_transition_sequence_before
                == authenticated_funding.state().transition_sequence,
        ClutchError::MismatchedState,
    )?;
    require_live(reservation_receipt_id)?;
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let before_state_id = authenticated_funding
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let before_data_id = authenticated_funding.data_id();
    let before_authentication_id = authenticated_funding.authentication_id();
    let authority = ExactSeriesFundingReservationAuthorityV4 {
        state_before_id: before_state_id,
        binding_id,
        reservation_receipt_id,
    };
    let mut successor = *authenticated_funding.state();
    let reserved_ordinal = successor
        .reserve_created(
            &authority,
            series,
            quote,
            attachment,
            &binding,
            reservation_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(reserved_ordinal == binding.ordinal, ClutchError::MismatchedState)?;
    let rebound = write_series_funding_state_v4(
        program_id,
        funding_account,
        authenticated_funding,
        successor,
    )?;
    let pending_state_id = rebound
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        rebound.state().phase == clutch_product_series::SeriesFundingPhaseV4::Pending
            && rebound.state().pending_pre_source_reservation_binding_id
                == binding_id.content_id()
            && rebound.state().pending_reservation_receipt_id == reservation_receipt_id
            && rebound.state().pending_clock_receipt_id == binding.clock_receipt_id
            && rebound.state().pending_clock_bucket == binding.clock_bucket,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        SERIES_FUNDING_RESERVATION_POSTWRITE_DOMAIN_V4,
        program_id.as_ref(),
        funding_account.key.as_ref(),
        &before_state_id.bytes(),
        &before_data_id.bytes(),
        &before_authentication_id.bytes(),
        &binding_id.bytes(),
        &reservation_receipt_id.bytes(),
        &pending_state_id.bytes(),
        &rebound.data_id().bytes(),
        &rebound.authentication_id().bytes(),
        &binding.clock_receipt_id.bytes(),
        &binding.clock_bucket.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesFundingReservationV4 {
        id,
        binding: Box::new(binding),
        reservation_receipt_id,
        funding_account: *funding_account.key,
        funding_state_before_id: before_state_id,
        funding_data_before_id: before_data_id,
        funding_authentication_before_id: before_authentication_id,
        pending: rebound,
    })
}

/// Consume the sole Pending reservation after Source, RootV2, and LinkV2 are
/// persisted, and mint the acyclic authority Replay may borrow. The predicted
/// Funding poststate is semantic evidence only; no account write occurs here.
#[inline(never)]
fn authorize_series_funding_completion_v4(
    reservation: AuthenticatedProductSeriesFundingReservationV4,
    series: &SeriesPlanV5,
    quote: &SeriesFundingQuoteV5,
    attachment: &SeriesAttachmentPlanV5,
    facts: Box<SeriesFundingCompletionAuthorizationV4>,
) -> Outcome<AuthenticatedSeriesFundingCompletionAuthorizationV4> {
    let reservation_binding_id = reservation.binding().id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let pending_state_id = reservation.funding_state_pending_id()?;
    let projected_state_after = reservation.pending().state()
        .project_pending_completion_poststate(series, quote, attachment)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projected_state_after_id = projected_state_after.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        facts.reservation_binding_id == reservation_binding_id
            && facts.funding_account_id.bytes() == reservation.funding_account().to_bytes()
            && facts.funding_account_authentication_pending_id
                == reservation.funding_authentication_pending_id()
            && facts.funding_pending_state_id == pending_state_id
            && facts.funding_projected_state_after_id == projected_state_after_id,
        ClutchError::MismatchedState,
    )?;
    let id = facts.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedSeriesFundingCompletionAuthorizationV4 {
        id,
        facts,
        projected_state_after: Box::new(projected_state_after),
        reservation: Box::new(reservation),
    })
}

/// Private final FundingV4 completion half. Its caller must have already
/// persisted the exact Replay poststate named by `binding`; Funding clears
/// Pending last and returns one hostile postwrite joining both authorities.
#[inline(never)]
fn complete_series_funding_v4_with_binding(
    program_id: &Pubkey,
    funding_account: &AccountInfo<'_>,
    authorization: AuthenticatedSeriesFundingCompletionAuthorizationV4,
    series: &SeriesPlanV5,
    quote: &SeriesFundingQuoteV5,
    attachment: &SeriesAttachmentPlanV5,
    binding: SeriesFundingCompletionBindingV4,
) -> Outcome<AuthenticatedProductSeriesFundingCompletionV4> {
    let authorization_id = authorization.id();
    let projected_state_after_id = authorization.facts().funding_projected_state_after_id;
    let reservation = *authorization.reservation;
    let reservation_binding_id = reservation
        .binding()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let before_state_id = reservation.funding_state_pending_id()?;
    require(
        reservation.funding_account() == *funding_account.key
            && reservation.pending().is_writable()
            && reservation.pending().account() == *funding_account.key
            && reservation.pending().state().phase
                == clutch_product_series::SeriesFundingPhaseV4::Pending
            && binding.completion_authorization_id == authorization_id,
        ClutchError::MismatchedState,
    )?;
    let completion_receipt_id = authorization_id.content_id();
    require_live(completion_receipt_id)?;
    let authority = ExactSeriesFundingCompletionAuthorityV4 {
        state_before_id: before_state_id,
        binding_id,
        completion_receipt_id,
    };
    let mut successor = *reservation.pending().state();
    let completed_ordinal = successor
        .complete_pending(
            &authority,
            series,
            quote,
            attachment,
            &binding,
            completion_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        completed_ordinal == reservation.binding().ordinal,
        ClutchError::MismatchedState,
    )?;
    let reservation_postwrite_id = reservation.id();
    let before_data_id = reservation.funding_data_pending_id();
    let before_authentication_id = reservation.funding_authentication_pending_id();
    let rebound = write_series_funding_state_v4(
        program_id,
        funding_account,
        reservation.pending,
        successor,
    )?;
    let after_state_id = rebound
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        after_state_id == projected_state_after_id
            && rebound.state() == &*authorization.projected_state_after
            && rebound.state().phase != clutch_product_series::SeriesFundingPhaseV4::Pending
            && rebound.state().pending_pre_source_reservation_binding_id.is_zero()
            && rebound.state().pending_reservation_receipt_id.is_zero()
            && rebound.state().pending_clock_receipt_id.is_zero()
            && rebound.state().pending_clock_bucket == 0,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        SERIES_FUNDING_COMPLETION_POSTWRITE_DOMAIN_V4,
        program_id.as_ref(),
        funding_account.key.as_ref(),
        &reservation_postwrite_id.bytes(),
        &reservation_binding_id.bytes(),
        &authorization_id.bytes(),
        &projected_state_after_id.bytes(),
        &binding_id.bytes(),
        &completion_receipt_id.bytes(),
        &before_state_id.bytes(),
        &after_state_id.bytes(),
        &before_data_id.bytes(),
        &rebound.data_id().bytes(),
        &before_authentication_id.bytes(),
        &rebound.authentication_id().bytes(),
        &completed_ordinal.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesFundingCompletionV4 {
        id,
        completion_authorization_id: authorization_id,
        projected_state_after_id,
        completion_binding_id: binding_id,
        reservation_postwrite_id,
        funding_account: *funding_account.key,
        funding_state_before_id: before_state_id,
        funding_state_after_id: after_state_id,
        funding_data_before_id: before_data_id,
        funding_data_after_id: rebound.data_id(),
        funding_authentication_before_id: before_authentication_id,
        funding_authentication_after_id: rebound.authentication_id(),
        completed_ordinal,
        rebound,
    })
}

/// Private raw replayV2 writer. No projection or count-only receipt can invoke
/// it outside the single Product lifecycle compositor.
fn write_series_lifecycle_replay_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesLifecycleReplayV2,
    successor: SeriesLifecycleReplayV2,
) -> Outcome<AuthenticatedSeriesLifecycleReplayV2> {
    let before = authenticated.value();
    require(
        account.is_writable
            && *account.key == authenticated.account()
            && account.owner == program_id
            && successor.binding() == before.state.binding(),
        ClutchError::MismatchedState,
    )?;
    let live = authenticate_series_lifecycle_replay_v2(
        program_id,
        account,
        before.state.binding().series_plan_id,
        true,
    )?;
    require(live == authenticated, ClutchError::MismatchedState)?;
    let successor_account = SeriesLifecycleReplayAccountV2 {
        state: successor,
        permanent_rent_principal_lamports: before.permanent_rent_principal_lamports,
        stored_bump: before.stored_bump,
    };
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        successor_account.encode(&mut data)?;
    }
    let rebound = authenticate_series_lifecycle_replay_v2(
        program_id,
        account,
        before.state.binding().series_plan_id,
        true,
    )?;
    require(
        rebound.value() == successor_account
            && rebound.observed_lamports() == authenticated.observed_lamports()
            && rebound.authentication_id() != authenticated.authentication_id()
            && rebound.data_id() != authenticated.data_id(),
        ClutchError::MismatchedState,
    )?;
    Ok(rebound)
}

pub(crate) fn authenticate_market_lifecycle_root_v2<'state>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    require_writable: bool,
    output: &'state mut MarketLifecycleRootAccountV2,
) -> Outcome<AuthenticatedMarketLifecycleRootV2<'state>> {
    require(
        !account.is_signer && !account.executable && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV2::decode_into(&data, output)?;
    let binding = output.state.binding();
    let observed_lamports = account.lamports();
    require(binding.market_instance_id == expected_market_instance_id
        && binding.generation == expected_generation
        && observed_lamports >= output.rent_principal_lamports,
        ClutchError::MismatchedState)?;
    let (expected, bump) = seeds::product_market_lifecycle_root_pda(
        program_id, &expected_market_instance_id.bytes(), expected_generation);
    expect_pda(account.key, (expected, bump), Some(output.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let semantic_id = output.state.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = hashv(&[
        MARKET_LIFECYCLE_AUTHENTICATION_DOMAIN_V2, account.key.as_ref(), program_id.as_ref(),
        &data_id.bytes(), &semantic_id.bytes(), &output.rent_principal_lamports.to_le_bytes(),
        &observed_lamports.to_le_bytes(), &[output.stored_bump],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedMarketLifecycleRootV2 { account: *account.key, owner_program: *program_id,
        value: output, observed_lamports, writable: account.is_writable, data_id,
        authentication_id })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_series_market_link_v2<'state>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    expected_ordinal: u32,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    expected_market_root: Pubkey,
    require_writable: bool,
    output: &'state mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedSeriesMarketLinkV2<'state>> {
    require(
        !account.is_signer && !account.executable && account.is_writable == require_writable
            && account.owner == program_id && account.data_len() == SERIES_MARKET_LINK_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV2::decode_into(&data, output)?;
    let binding = output.state.binding();
    let accounted_lamports = output.state.rent_principal_lamports()
        .checked_add(output.state.current_donation_lamports())
        .ok_or(ClutchError::Arithmetic)?;
    let observed_lamports = account.lamports();
    require(binding.series_plan_id == expected_series_plan_id
        && binding.ordinal == expected_ordinal
        && binding.market_instance_id == expected_market_instance_id
        && binding.generation == expected_generation
        && binding.market_root_account_id.bytes() == expected_market_root.to_bytes()
        && observed_lamports >= accounted_lamports,
        ClutchError::MismatchedState)?;
    let (expected, bump) = seeds::product_series_market_link_pda(
        program_id, &expected_series_plan_id.bytes(), expected_ordinal);
    expect_pda(account.key, (expected, bump), Some(output.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let semantic_id = output.state.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = ContentId::from_bytes(series_market_link_authentication_id_v2(
        account.key.to_bytes(), program_id.to_bytes(), data_id.bytes(), semantic_id.bytes(),
        expected_market_root.to_bytes(), observed_lamports).0);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesMarketLinkV2 { account: *account.key, owner_program: *program_id,
        value: output, observed_lamports, writable: account.is_writable, data_id,
        authentication_id })
}

/// Join an exact active LinkV2 to its content-addressed BundleV6 and
/// AttachmentV5 accounts. The returned receipt retains the observed link
/// privilege; only the narrow admission and terminal writers can consume a
/// writable receipt.
pub(crate) fn authenticate_series_wrapper_authorization_v2(
    program_id: &Pubkey,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    compiler_bundle_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSeriesWrapperAuthorizationV2> {
    let binding = link.state().binding();
    let wrapper_status = link.state().obligation_status(SeriesLinkObligationV2::Wrapper);
    require(link.state().phase() == SeriesMarketLinkPhaseV2::Active
        && matches!(wrapper_status,
            SeriesLinkObligationStatusV2::EnabledNeverFounded | SeriesLinkObligationStatusV2::Live)
        && (wrapper_status == SeriesLinkObligationStatusV2::Live || link.is_writable()),
        ClutchError::MismatchedState)?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id, compiler_bundle_account, binding.compiler_bundle_id.content_id())?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV5>(
        program_id, attachment_account, binding.attachment_plan_id.content_id())?;
    let bundle_id = bundle.value().id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = attachment.value().id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(bundle_id == binding.compiler_bundle_id
        && bundle.value().series_plan_id == binding.series_plan_id
        && bundle.value().funding_terms_id == binding.funding_terms_id
        && bundle.value().funding_quote_id == binding.funding_quote_id
        && bundle.value().attachment_plan_id == binding.attachment_plan_id
        && bundle.value().capability_profile_id.content_id() == binding.capability_profile_id
        && attachment_id == binding.attachment_plan_id
        && attachment.value().funding_quote_id == bundle.value().funding_quote_id
        && attachment.value().funding_quote_id == binding.funding_quote_id,
        ClutchError::MismatchedState)?;
    let link_semantic_id = link.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_binding_id = binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let wrapper_admission_receipt_id = link.state()
        .obligation_admission_receipt_id(SeriesLinkObligationV2::Wrapper);
    let link_transition_sequence = link.state().transition_sequence();
    let id = hashv(&[
        SERIES_WRAPPER_AUTHENTICATION_DOMAIN_V2, link.account().as_ref(),
        &link.authentication_id().bytes(), &link_semantic_id.bytes(),
        compiler_bundle_account.key.as_ref(), &bundle.semantic_id().bytes(),
        attachment_account.key.as_ref(), &attachment.semantic_id().bytes(),
        &attachment.value().wrapper_recipe_set_id.bytes(),
        &[wrapper_status.wire_byte()], &wrapper_admission_receipt_id.bytes(),
        &link_transition_sequence.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedSeriesWrapperAuthorizationV2 {
        id, link_account: link.account(), link_authentication_id: link.authentication_id(),
        link_semantic_id, link_binding_id,
        wrapper_obligation_configuration_id: binding.obligation_configuration_id.content_id(),
        series_plan_id: binding.series_plan_id, ordinal: binding.ordinal,
        market_instance_id: binding.market_instance_id, generation: binding.generation,
        attachment_plan_id: binding.attachment_plan_id,
        compiler_bundle_id: binding.compiler_bundle_id, funding_quote_id: binding.funding_quote_id,
        capability_profile_id: binding.capability_profile_id,
        wrapper_recipe_set_id: attachment.value().wrapper_recipe_set_id,
        rent_refund_owner: binding.rent_refund_owner,
        neutral_lamport_sink: binding.neutral_lamport_sink, wrapper_status,
        wrapper_admission_receipt_id, link_transition_sequence,
    })
}

/// Authenticate the exact current Product state required before Dealer's first
/// lease can create 0xaf/v2 and promote its StateV3.
#[allow(clippy::too_many_arguments)]
fn authenticate_series_dealer_authorization_v2(
    program_id: &Pubkey,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    compiler_bundle_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSeriesDealerAuthorizationV2> {
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_binding_id = link_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id,
        compiler_bundle_account,
        link_binding.compiler_bundle_id.content_id(),
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV5>(
        program_id,
        attachment_account,
        link_binding.attachment_plan_id.content_id(),
    )?;
    let bundle_id = bundle
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = attachment
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        !root.is_writable()
            && link.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV2::Active
            && root.state().resolution_semantic_id() == ContentId::ZERO
            && root.state().resolution_data_id() == ContentId::ZERO
            && root.state().resolution_activation_receipt_id() == ContentId::ZERO
            && link.state().phase() == SeriesMarketLinkPhaseV2::Active
            && link
                .state()
                .obligation_status(SeriesLinkObligationV2::Dealer)
                == SeriesLinkObligationStatusV2::EnabledNeverFounded
            && link
                .state()
                .obligation_admission_receipt_id(SeriesLinkObligationV2::Dealer)
                == ContentId::ZERO
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.capability_profile_id == root_binding.capability_profile_id
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.activation_consumed()
            && registry.funding_terms_id() == link_binding.funding_terms_id
            && registry.compiler_bundle_id() == link_binding.compiler_bundle_id
            && registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && bundle_id == link_binding.compiler_bundle_id
            && bundle.value().series_plan_id == link_binding.series_plan_id
            && bundle.value().funding_terms_id == link_binding.funding_terms_id
            && bundle.value().funding_quote_id == link_binding.funding_quote_id
            && bundle.value().attachment_plan_id == link_binding.attachment_plan_id
            && bundle.value().registry_release_id == registry.registry_release_id()
            && bundle.value().capability_profile_id.content_id()
                == registry.capability_profile_id()
            && attachment_id == link_binding.attachment_plan_id
            && attachment.value().funding_quote_id == link_binding.funding_quote_id
            && root.account() != link.account()
            && root.account() != registry.series_registry_account()
            && link.account() != registry.series_registry_account(),
        ClutchError::MismatchedState,
    )?;
    let link_transition_sequence = link.state().transition_sequence();
    let id = hashv(&[
        SERIES_DEALER_AUTHORIZATION_DOMAIN_V2,
        program_id.as_ref(),
        root.account().as_ref(),
        &root.authentication_id().bytes(),
        &root_semantic_id.bytes(),
        &root_binding_id.bytes(),
        link.account().as_ref(),
        &link.authentication_id().bytes(),
        &link.data_id().bytes(),
        &link_semantic_id.bytes(),
        &link_binding_id.bytes(),
        &link_transition_sequence.to_le_bytes(),
        registry.series_registry_account().as_ref(),
        &registry.series_registry_authentication_id().bytes(),
        &registry.id().bytes(),
        compiler_bundle_account.key.as_ref(),
        &bundle.semantic_id().bytes(),
        attachment_account.key.as_ref(),
        &attachment.semantic_id().bytes(),
        &link_binding.obligation_configuration_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedSeriesDealerAuthorizationV2 {
        id,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        root_semantic_id,
        root_binding_id,
        link_account: link.account(),
        link_authentication_id: link.authentication_id(),
        link_data_id: link.data_id(),
        link_semantic_id,
        link_binding_id,
        link_transition_sequence,
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        funding_terms_id: link_binding.funding_terms_id,
        funding_quote_id: link_binding.funding_quote_id,
        compiler_bundle_id: link_binding.compiler_bundle_id,
        attachment_plan_id: link_binding.attachment_plan_id,
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        dealer_obligation_configuration_id: link_binding.obligation_configuration_id.content_id(),
        rent_refund_owner: link_binding.rent_refund_owner,
        neutral_lamport_sink: link_binding.neutral_lamport_sink,
    })
}

/// Atomically consume Dealer's private prewrite and admit its LinkV2 obligation.
///
/// Dealer action14 must create 0xaf/v2 and promote its StateV3 after this call;
/// Solana rollback makes those writes and this Product latch indivisible.
pub(crate) fn admit_series_dealer_obligation_v2<'next, A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    authenticated_root: AuthenticatedMarketLifecycleRootV2<'_>,
    root_reauth_output: &mut MarketLifecycleRootAccountV2,
    link_account: &AccountInfo<'_>,
    authenticated_link: AuthenticatedSeriesMarketLinkV2<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    compiler_bundle_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
    owner: &A,
    rebound_output: &'next mut SeriesMarketLinkAccountV2,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV2<'next>,
    AuthenticatedSeriesDealerAdmissionV2,
)>
where
    A: AuthenticatedSeriesDealerAdmissionOwnerV2 + ?Sized,
{
    let root_binding = authenticated_root.state().binding();
    let live_root = authenticate_market_lifecycle_root_v2(
        program_id,
        root_account,
        root_binding.market_instance_id,
        root_binding.generation,
        false,
        root_reauth_output,
    )?;
    require(
        live_root.account() == authenticated_root.account()
            && live_root.owner_program() == authenticated_root.owner_program()
            && live_root.value() == authenticated_root.value()
            && live_root.observed_lamports() == authenticated_root.observed_lamports()
            && live_root.data_id() == authenticated_root.data_id()
            && live_root.authentication_id() == authenticated_root.authentication_id(),
        ClutchError::MismatchedState,
    )?;
    let authorization = authenticate_series_dealer_authorization_v2(
        program_id,
        live_root,
        authenticated_link,
        registry,
        compiler_bundle_account,
        attachment_account,
    )?;
    let owner_admission_receipt_id = owner.owner_admission_receipt_id()?;
    let dealer_obligation_account = owner.dealer_obligation_account()?;
    let dealer_state_account = owner.dealer_state_account()?;
    let dealer_state_presemantic_id = owner.dealer_state_presemantic_id()?;
    let dealer_facility_id = owner.dealer_facility_id()?;
    let dealer_position_binding_id = owner.dealer_position_binding_id()?;
    let dealer_rent_principal_lamports = owner.dealer_rent_principal_lamports()?;
    let dealer_prefund_donation_lamports = owner.dealer_prefund_donation_lamports()?;
    let rent_refund_owner = owner.rent_refund_owner()?;
    let neutral_lamport_sink = owner.neutral_lamport_sink()?;
    for id in [
        owner_admission_receipt_id,
        dealer_state_presemantic_id,
        dealer_facility_id,
        dealer_position_binding_id,
        rent_refund_owner,
        neutral_lamport_sink,
    ] {
        require_live(id)?;
    }
    require(
        link_account.is_writable
            && *link_account.key == authenticated_link.account()
            && authorization.link_account == authenticated_link.account()
            && authorization.link_authentication_id == authenticated_link.authentication_id()
            && authorization.link_data_id == authenticated_link.data_id()
            && authorization.link_semantic_id
                == authenticated_link
                    .state()
                    .semantic_id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && authorization.link_binding_id
                == authenticated_link
                    .state()
                    .binding()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && authorization.link_transition_sequence
                == authenticated_link.state().transition_sequence()
            && authenticated_link.state().phase() == SeriesMarketLinkPhaseV2::Active
            && authenticated_link
                .state()
                .obligation_status(SeriesLinkObligationV2::Dealer)
                == SeriesLinkObligationStatusV2::EnabledNeverFounded
            && rent_refund_owner == authorization.rent_refund_owner
            && neutral_lamport_sink == authorization.neutral_lamport_sink
            && dealer_rent_principal_lamports != 0
            && dealer_obligation_account != authorization.root_account
            && dealer_obligation_account != authorization.link_account
            && dealer_obligation_account != dealer_state_account
            && dealer_state_account != authorization.root_account
            && dealer_state_account != authorization.link_account
            && dealer_obligation_account.to_bytes() != rent_refund_owner.bytes()
            && dealer_obligation_account.to_bytes() != neutral_lamport_sink.bytes()
            && dealer_state_account.to_bytes() != rent_refund_owner.bytes()
            && dealer_state_account.to_bytes() != neutral_lamport_sink.bytes(),
        ClutchError::MismatchedState,
    )?;
    owner.authenticate_series_dealer_admission_owner_v2(
        authorization.id,
        authorization.root_account,
        authorization.root_binding_id,
        authorization.link_account,
        authorization.link_binding_id,
        authorization.series_plan_id,
        authorization.ordinal,
        authorization.market_instance_id,
        authorization.generation,
        authorization.funding_quote_id,
        authorization.compiler_bundle_id,
        authorization.attachment_plan_id,
        authorization.registry_release_id,
        authorization.capability_profile_id,
        authorization.dealer_obligation_configuration_id,
        dealer_obligation_account,
        dealer_state_account,
        dealer_state_presemantic_id,
        dealer_facility_id,
        dealer_position_binding_id,
        dealer_rent_principal_lamports,
        dealer_prefund_donation_lamports,
        rent_refund_owner,
        neutral_lamport_sink,
        owner_admission_receipt_id,
    )?;
    let link_transition_sequence_after = authenticated_link
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let product_admission_projection = SeriesLinkObligationAdmissionProjectionV2 {
        link_semantic_id: authorization.link_semantic_id,
        obligation: SeriesLinkObligationV2::Dealer,
        link_transition_sequence: link_transition_sequence_after,
        owner_admission_receipt_id,
    };
    let product_admission_projection_id = product_admission_projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = authenticated_link
        .state()
        .admit_obligation(product_admission_projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_authentication_before = authenticated_link.authentication_id();
    let link_data_before = authenticated_link.data_id();
    let rebound = write_series_market_link_v2(
        program_id,
        link_account,
        authenticated_link,
        &successor,
        rebound_output,
    )?;
    let link_semantic_after = rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        rebound
            .state()
            .obligation_status(SeriesLinkObligationV2::Dealer)
            == SeriesLinkObligationStatusV2::Live
            && rebound
                .state()
                .obligation_admission_receipt_id(SeriesLinkObligationV2::Dealer)
                == product_admission_projection_id
            && rebound.state().transition_sequence() == link_transition_sequence_after,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        SERIES_DEALER_ADMISSION_POSTWRITE_DOMAIN_V2,
        program_id.as_ref(),
        &authorization.id.bytes(),
        authorization.root_account.as_ref(),
        &authorization.root_authentication_id.bytes(),
        &authorization.root_semantic_id.bytes(),
        &authorization.root_binding_id.bytes(),
        link_account.key.as_ref(),
        &link_authentication_before.bytes(),
        &rebound.authentication_id().bytes(),
        &link_data_before.bytes(),
        &rebound.data_id().bytes(),
        &authorization.link_semantic_id.bytes(),
        &link_semantic_after.bytes(),
        &link_transition_sequence_after.to_le_bytes(),
        &product_admission_projection_id.bytes(),
        &owner_admission_receipt_id.bytes(),
        dealer_obligation_account.as_ref(),
        dealer_state_account.as_ref(),
        &dealer_state_presemantic_id.bytes(),
        &dealer_facility_id.bytes(),
        &dealer_position_binding_id.bytes(),
        &dealer_rent_principal_lamports.to_le_bytes(),
        &dealer_prefund_donation_lamports.to_le_bytes(),
        &registry.id().bytes(),
        &authorization.funding_quote_id.bytes(),
        &authorization.compiler_bundle_id.bytes(),
        &authorization.attachment_plan_id.bytes(),
        &authorization.registry_release_id.bytes(),
        &authorization.capability_profile_id.bytes(),
        &authorization.dealer_obligation_configuration_id.bytes(),
        &rent_refund_owner.bytes(),
        &neutral_lamport_sink.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedSeriesDealerAdmissionV2 {
        id,
        root_account: authorization.root_account,
        root_authentication_id: authorization.root_authentication_id,
        root_semantic_id: authorization.root_semantic_id,
        root_binding_id: authorization.root_binding_id,
        link_account: authorization.link_account,
        link_binding_id: authorization.link_binding_id,
        link_authentication_before,
        link_authentication_after: rebound.authentication_id(),
        link_data_before,
        link_data_after: rebound.data_id(),
        link_semantic_before: authorization.link_semantic_id,
        link_semantic_after,
        link_transition_sequence_after,
        product_admission_projection,
        owner_admission_receipt_id,
        dealer_obligation_account,
        dealer_state_account,
        dealer_state_presemantic_id,
        dealer_facility_id,
        dealer_position_binding_id,
        dealer_rent_principal_lamports,
        dealer_prefund_donation_lamports,
        series_plan_id: authorization.series_plan_id,
        ordinal: authorization.ordinal,
        market_instance_id: authorization.market_instance_id,
        generation: authorization.generation,
        compiler_bundle_id: authorization.compiler_bundle_id,
        attachment_plan_id: authorization.attachment_plan_id,
        funding_quote_id: authorization.funding_quote_id,
        registry_release_id: authorization.registry_release_id,
        capability_profile_id: authorization.capability_profile_id,
        dealer_obligation_configuration_id: authorization.dealer_obligation_configuration_id,
        registry_capability_id: registry.id(),
        rent_refund_owner,
        neutral_lamport_sink,
    }))
}

/// Exact Product-owned account frame inside Dealer action25.
///
/// Dealer State, Replay, Position, and `0xaf/v2` are authenticated by the
/// non-Copy owner. Product independently hostile-decodes every account it owns
/// or whose checked loader release authorizes this LinkV2 mutation.
#[derive(Debug)]
pub(crate) struct SeriesDealerTerminalAccountsV2<'a, 'info> {
    pub(crate) market_lifecycle_root: &'a AccountInfo<'info>,
    pub(crate) series_registry: &'a AccountInfo<'info>,
    pub(crate) registry_program: &'a AccountInfo<'info>,
    pub(crate) registry_programdata: &'a AccountInfo<'info>,
    pub(crate) registry_release: &'a AccountInfo<'info>,
    pub(crate) capability_profile: &'a AccountInfo<'info>,
    pub(crate) series_market_link: &'a AccountInfo<'info>,
    pub(crate) compiler_bundle: &'a AccountInfo<'info>,
    pub(crate) attachment: &'a AccountInfo<'info>,
}

/// Consume Dealer's exact action25 prewrite into the sole current LinkV2
/// Dealer Live-to-Terminal transition.
///
/// No semantic identity is accepted as an argument. The untrusted LinkV2 body
/// selects only the expected PDA coordinates; RootV2, RegistryCapabilityV4
/// (RegistryV3/ReleaseV2/ProfileV4/ProgramData), BundleV6, AttachmentV5, and
/// LinkV2 are then independently hostile-reauthenticated. Dealer's authority
/// is consumed before the private Link writer is reached.
#[inline(never)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn terminalize_series_dealer_obligation_v2<A>(
    program_id: &Pubkey,
    accounts: SeriesDealerTerminalAccountsV2<'_, '_>,
    owner: A,
    root_output: &mut MarketLifecycleRootAccountV2,
    link_pre_output: &mut SeriesMarketLinkAccountV2,
    link_rebound_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedSeriesDealerTerminalV2>
where
    A: AuthenticatedSeriesDealerTerminalOwnerV2,
{
    let product_accounts = [
        accounts.market_lifecycle_root,
        accounts.series_registry,
        accounts.registry_program,
        accounts.registry_programdata,
        accounts.registry_release,
        accounts.capability_profile,
        accounts.series_market_link,
        accounts.compiler_bundle,
        accounts.attachment,
    ];
    let mut left = 0usize;
    while left < product_accounts.len() {
        let mut right = left + 1;
        while right < product_accounts.len() {
            require(
                product_accounts[left].key != product_accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }

    let link_data = accounts
        .series_market_link
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV2::decode_into(&link_data, link_pre_output)?;
    drop(link_data);
    let untrusted_binding = link_pre_output.state.binding();

    let root = authenticate_market_lifecycle_root_v2(
        program_id,
        accounts.market_lifecycle_root,
        untrusted_binding.market_instance_id,
        untrusted_binding.generation,
        false,
        root_output,
    )?;
    let registry_account = authenticate_series_registry_account_v3(
        program_id,
        accounts.series_registry,
        untrusted_binding.series_plan_id,
        false,
    )?;
    let registry = authenticate_registry_capability_v4(
        program_id,
        registry_account,
        accounts.registry_program,
        accounts.registry_programdata,
        accounts.registry_release,
        accounts.capability_profile,
    )?;
    let link = authenticate_series_market_link_v2(
        program_id,
        accounts.series_market_link,
        untrusted_binding.series_plan_id,
        untrusted_binding.ordinal,
        untrusted_binding.market_instance_id,
        untrusted_binding.generation,
        *accounts.market_lifecycle_root.key,
        true,
        link_pre_output,
    )?;
    let root_binding = root.state().binding();
    let root_capital = root.state().capital();
    let resolution_semantic_id = root.state().resolution_semantic_id();
    let resolution_data_id = root.state().resolution_data_id();
    let resolution_activation_receipt_id = root.state().resolution_activation_receipt_id();
    let link_binding = link.state().binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_binding_id = link_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_before = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id,
        accounts.compiler_bundle,
        link_binding.compiler_bundle_id.content_id(),
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV5>(
        program_id,
        accounts.attachment,
        link_binding.attachment_plan_id.content_id(),
    )?;
    let compiler_bundle_id = bundle
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_plan_id = attachment
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let dealer_admission_receipt_id = link
        .state()
        .obligation_admission_receipt_id(SeriesLinkObligationV2::Dealer);
    let link_transition_sequence_before = link.state().transition_sequence();
    let link_transition_sequence_after = link_transition_sequence_before
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;

    for id in [
        resolution_semantic_id,
        resolution_data_id,
        resolution_activation_receipt_id,
    ] {
        require_live(id)?;
    }
    require(
        root.state().phase() == MarketLifecyclePhaseV2::Active
            && link.state().phase() == SeriesMarketLinkPhaseV2::Active
            && link
                .state()
                .obligation_status(SeriesLinkObligationV2::Dealer)
                == SeriesLinkObligationStatusV2::Live
            && dealer_admission_receipt_id != ContentId::ZERO
            && link
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV2::Dealer)
                == ContentId::ZERO
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.capability_profile_id == root_binding.capability_profile_id
            && registry.activation_consumed()
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.funding_terms_id() == link_binding.funding_terms_id
            && registry.compiler_bundle_id() == link_binding.compiler_bundle_id
            && registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && compiler_bundle_id == link_binding.compiler_bundle_id
            && bundle.value().series_plan_id == link_binding.series_plan_id
            && bundle.value().funding_terms_id == link_binding.funding_terms_id
            && bundle.value().funding_quote_id == link_binding.funding_quote_id
            && bundle.value().attachment_plan_id == link_binding.attachment_plan_id
            && bundle.value().registry_release_id == registry.registry_release_id()
            && bundle.value().capability_profile_id.content_id()
                == registry.capability_profile_id()
            && attachment_plan_id == link_binding.attachment_plan_id
            && attachment.value().funding_quote_id == link_binding.funding_quote_id
            && link_binding.rent_refund_owner == root_capital.rent_refund_owner
            && link_binding.neutral_lamport_sink == root_capital.neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;

    let owner_authentication_id = owner.owner_authentication_id()?;
    let dealer_obligation_account = owner.dealer_obligation_account()?;
    let dealer_obligation_presemantic_id = owner.dealer_obligation_presemantic_id()?;
    let dealer_state_account = owner.dealer_state_account()?;
    let dealer_state_presemantic_id = owner.dealer_state_presemantic_id()?;
    let terminal_state_receipt_id = owner.terminal_state_receipt_id()?;
    let replay_presemantic_id = owner.replay_presemantic_id()?;
    let replay_pre_ordinal = owner.replay_pre_ordinal()?;
    let owner_terminal_receipt_id = owner.owner_terminal_receipt_id()?;
    let expected_link_transition_sequence = owner.expected_link_transition_sequence()?;
    let rent_refund_owner = owner.rent_refund_owner()?;
    let neutral_lamport_sink = owner.neutral_lamport_sink()?;
    for id in [
        owner_authentication_id,
        dealer_obligation_presemantic_id,
        dealer_state_presemantic_id,
        terminal_state_receipt_id,
        replay_presemantic_id,
        owner_terminal_receipt_id,
    ] {
        require_live(id)?;
    }
    require(
        replay_pre_ordinal != 0
            && expected_link_transition_sequence == link_transition_sequence_after
            && rent_refund_owner.to_bytes() == link_binding.rent_refund_owner.bytes()
            && neutral_lamport_sink.to_bytes() == link_binding.neutral_lamport_sink.bytes()
            && rent_refund_owner != neutral_lamport_sink
            && dealer_obligation_account != Pubkey::default()
            && dealer_state_account != Pubkey::default()
            && dealer_obligation_account != dealer_state_account
            && dealer_obligation_account != rent_refund_owner
            && dealer_obligation_account != neutral_lamport_sink
            && dealer_state_account != rent_refund_owner
            && dealer_state_account != neutral_lamport_sink
            && product_accounts
                .iter()
                .all(|account| *account.key != dealer_obligation_account)
            && product_accounts
                .iter()
                .all(|account| *account.key != dealer_state_account)
            && product_accounts.iter().all(|account| {
                *account.key != rent_refund_owner && *account.key != neutral_lamport_sink
            }),
        ClutchError::AuthorizationUnavailable,
    )?;

    let observation = SeriesDealerTerminalObservationV2 {
        owner_authentication_id,
        dealer_obligation_account,
        dealer_obligation_presemantic_id,
        dealer_state_account,
        dealer_state_presemantic_id,
        terminal_state_receipt_id,
        replay_presemantic_id,
        replay_pre_ordinal,
        owner_terminal_receipt_id,
        rent_refund_owner,
        neutral_lamport_sink,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        root_data_id: root.data_id(),
        root_semantic_id,
        root_binding_id,
        resolution_semantic_id,
        resolution_data_id,
        resolution_activation_receipt_id,
        registry_account: registry.series_registry_account(),
        registry_authentication_id: registry.series_registry_authentication_id(),
        registry_capability_id: registry.id(),
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        registry_release_artifact_account: registry.release_artifact_account(),
        capability_profile_artifact_account: registry.profile_artifact_account(),
        registry_program: registry.program_account(),
        registry_programdata: registry.programdata_account(),
        registry_programdata_sha256: registry.programdata_sha256(),
        compiler_bundle_account: *accounts.compiler_bundle.key,
        compiler_bundle_id: compiler_bundle_id.content_id(),
        compiler_bundle_semantic_id: bundle.semantic_id(),
        attachment_account: *accounts.attachment.key,
        attachment_plan_id: attachment_plan_id.content_id(),
        attachment_semantic_id: attachment.semantic_id(),
        liquidity_facility_plan_id: attachment.value().liquidity_facility_plan_id,
        dealer_obligation_configuration_id: link_binding
            .obligation_configuration_id
            .content_id(),
        link_account: link.account(),
        link_binding_id,
        link_authentication_before: link.authentication_id(),
        link_data_before: link.data_id(),
        link_semantic_before: link_semantic_before.content_id(),
        dealer_admission_receipt_id,
        link_transition_sequence_before,
        link_transition_sequence_after,
    };
    let observation_id = observation.id();
    require_live(observation_id)?;
    owner.consume_series_dealer_terminal_owner_v2(observation)?;

    let terminal_projection = SeriesLinkObligationTerminalProjectionV2 {
        link_semantic_id: link_semantic_before,
        obligation: SeriesLinkObligationV2::Dealer,
        disposition: SeriesLinkObligationDispositionV2::Terminal,
        link_transition_sequence: link_transition_sequence_after,
        owner_terminal_receipt_id,
    };
    let terminal_projection_id = terminal_projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = link
        .state()
        .consume_obligation(terminal_projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_series_market_link_v2(
        program_id,
        accounts.series_market_link,
        link,
        &successor,
        link_rebound_output,
    )?;
    let link_semantic_after = rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        rebound
            .state()
            .obligation_status(SeriesLinkObligationV2::Dealer)
            == SeriesLinkObligationStatusV2::Terminal
            && rebound
                .state()
                .obligation_admission_receipt_id(SeriesLinkObligationV2::Dealer)
                == dealer_admission_receipt_id
            && rebound
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV2::Dealer)
                == terminal_projection_id
            && rebound.state().transition_sequence() == link_transition_sequence_after
            && rebound.state().binding() == link_binding,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        SERIES_DEALER_TERMINAL_POSTWRITE_DOMAIN_V2,
        program_id.as_ref(),
        &observation_id.bytes(),
        &terminal_projection_id.bytes(),
        &rebound.authentication_id().bytes(),
        &rebound.data_id().bytes(),
        &link_semantic_after.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedSeriesDealerTerminalV2 {
        id,
        observation,
        link_authentication_after: rebound.authentication_id(),
        link_data_after: rebound.data_id(),
        link_semantic_after: link_semantic_after.content_id(),
        terminal_projection,
        terminal_projection_id,
    })
}

/// Persist the first Wrapper admission only after the Structured owner accepts
/// the exact same immutable Product authorization.
pub(crate) fn admit_series_wrapper_obligation_v2<'next, A>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV2<'_>,
    authorization: AuthenticatedSeriesWrapperAuthorizationV2,
    owner: &A,
    rebound_output: &'next mut SeriesMarketLinkAccountV2,
) -> Outcome<(AuthenticatedSeriesMarketLinkV2<'next>, AuthenticatedSeriesWrapperAdmissionV2)>
where
    A: AuthenticatedSeriesWrapperAdmissionOwnerV2 + ?Sized,
{
    let owner_admission_receipt_id = owner.owner_admission_receipt_id()?;
    require_live(owner_admission_receipt_id)?;
    let semantic_before = authenticated.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(authenticated.is_writable() && authorization.requires_product_admission()
        && authorization.link_account == authenticated.account()
        && authorization.link_authentication_id == authenticated.authentication_id()
        && authorization.link_semantic_id == semantic_before
        && authorization.wrapper_admission_receipt_id == ContentId::ZERO
        && authorization.link_transition_sequence == authenticated.state().transition_sequence(),
        ClutchError::MismatchedState)?;
    owner.authenticate_series_wrapper_admission_owner_v2(
        authorization.id, authorization.link_account, authorization.link_binding_id,
        authorization.series_plan_id, authorization.ordinal, authorization.market_instance_id,
        authorization.generation, authorization.attachment_plan_id,
        authorization.compiler_bundle_id, authorization.funding_quote_id,
        authorization.capability_profile_id, authorization.wrapper_obligation_configuration_id,
        authorization.wrapper_recipe_set_id, authorization.rent_refund_owner,
        authorization.neutral_lamport_sink, owner_admission_receipt_id)?;
    let next_sequence = authorization.link_transition_sequence.checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let projection = SeriesLinkObligationAdmissionProjectionV2 {
        link_semantic_id: semantic_before,
        obligation: SeriesLinkObligationV2::Wrapper,
        link_transition_sequence: next_sequence,
        owner_admission_receipt_id,
    };
    let projection_id = projection.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = authenticated.state().admit_obligation(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = authenticated.authentication_id();
    let rebound = write_series_market_link_v2(
        program_id, account, authenticated, &successor, rebound_output)?;
    let semantic_after = rebound.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(rebound.state().obligation_status(SeriesLinkObligationV2::Wrapper)
            == SeriesLinkObligationStatusV2::Live
        && rebound.state().obligation_admission_receipt_id(SeriesLinkObligationV2::Wrapper)
            == projection_id,
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        SERIES_WRAPPER_ADMISSION_AUTHENTICATION_DOMAIN_V2, account.key.as_ref(),
        &authorization.id.bytes(), &authentication_before.bytes(),
        &rebound.authentication_id().bytes(), &semantic_before.bytes(),
        &semantic_after.bytes(), &owner_admission_receipt_id.bytes(), &projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedSeriesWrapperAdmissionV2 {
        id, link_account: *account.key, link_authentication_before: authentication_before,
        link_authentication_after: rebound.authentication_id(), link_semantic_before: semantic_before,
        link_semantic_after: semantic_after, owner_admission_receipt_id,
        product_admission_projection_id: projection_id,
    }))
}

/// Consume an exact Structured terminal postwrite into the live Wrapper latch.
pub(crate) fn terminalize_series_wrapper_obligation_v2<'next, A>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV2<'_>,
    authorization: AuthenticatedSeriesWrapperAuthorizationV2,
    owner: &A,
    rebound_output: &'next mut SeriesMarketLinkAccountV2,
) -> Outcome<(AuthenticatedSeriesMarketLinkV2<'next>, AuthenticatedSeriesWrapperTerminalV2)>
where
    A: AuthenticatedSeriesWrapperTerminalOwnerV2 + ?Sized,
{
    let binding = authenticated.state().binding();
    let binding_id = binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_before = authenticated.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_before_content = semantic_before.content_id();
    let admission_receipt = authenticated.state()
        .obligation_admission_receipt_id(SeriesLinkObligationV2::Wrapper);
    let owner_terminal_receipt_id = owner.owner_terminal_receipt_id()?;
    let structured_root_account = owner.structured_root_account()?;
    let structured_root_semantic_id = owner.structured_root_semantic_id()?;
    let structured_root_data_id = owner.structured_root_data_id()?;
    for id in [admission_receipt, owner_terminal_receipt_id,
        structured_root_semantic_id, structured_root_data_id] { require_live(id)?; }
    require(authenticated.is_writable()
        && authenticated.state().phase() == SeriesMarketLinkPhaseV2::Active
        && authenticated.state().obligation_status(SeriesLinkObligationV2::Wrapper)
            == SeriesLinkObligationStatusV2::Live
        && authorization.link_account == authenticated.account()
        && authorization.link_authentication_id == authenticated.authentication_id()
        && authorization.link_semantic_id == semantic_before
        && authorization.link_binding_id == binding_id
        && authorization.wrapper_status == SeriesLinkObligationStatusV2::Live
        && authorization.wrapper_admission_receipt_id == admission_receipt
        && authorization.link_transition_sequence == authenticated.state().transition_sequence()
        && owner_terminal_receipt_id != admission_receipt
        && structured_root_account != Pubkey::default()
        && structured_root_account != authenticated.account(),
        ClutchError::MismatchedState)?;
    owner.authenticate_series_wrapper_terminal_owner_v2(
        authorization.id, authenticated.account(), binding_id, binding.series_plan_id,
        binding.ordinal, binding.market_instance_id, binding.generation,
        authorization.attachment_plan_id, authorization.compiler_bundle_id,
        authorization.funding_quote_id, authorization.capability_profile_id,
        authorization.wrapper_obligation_configuration_id, authorization.wrapper_recipe_set_id,
        authorization.rent_refund_owner, authorization.neutral_lamport_sink, admission_receipt,
        owner_terminal_receipt_id, structured_root_account,
        structured_root_semantic_id, structured_root_data_id)?;
    let next_sequence = authenticated.state().transition_sequence().checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let projection = SeriesLinkObligationTerminalProjectionV2 {
        link_semantic_id: semantic_before,
        obligation: SeriesLinkObligationV2::Wrapper,
        disposition: SeriesLinkObligationDispositionV2::Terminal,
        link_transition_sequence: next_sequence,
        owner_terminal_receipt_id,
    };
    let projection_id = projection.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = authenticated.state().consume_obligation(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = authenticated.authentication_id();
    let rebound = write_series_market_link_v2(
        program_id, account, authenticated, &successor, rebound_output)?;
    let semantic_after = rebound.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_after_content = semantic_after.content_id();
    require(rebound.state().obligation_status(SeriesLinkObligationV2::Wrapper)
            == SeriesLinkObligationStatusV2::Terminal
        && rebound.state().obligation_terminal_receipt_id(SeriesLinkObligationV2::Wrapper)
            == projection_id,
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        SERIES_WRAPPER_TERMINAL_AUTHENTICATION_DOMAIN_V2, account.key.as_ref(),
        &authorization.id.bytes(),
        &authentication_before.bytes(), &rebound.authentication_id().bytes(),
        &semantic_before_content.bytes(), &semantic_after_content.bytes(),
        &admission_receipt.bytes(), &owner_terminal_receipt_id.bytes(), &projection_id.bytes(),
        structured_root_account.as_ref(), &structured_root_semantic_id.bytes(),
        &structured_root_data_id.bytes(), &binding_id.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedSeriesWrapperTerminalV2 {
        id, link_account: *account.key, link_authentication_before: authentication_before,
        link_authentication_after: rebound.authentication_id(),
        link_semantic_before: semantic_before_content, link_semantic_after: semantic_after_content,
        wrapper_admission_receipt_id: admission_receipt, owner_terminal_receipt_id,
        product_terminal_projection: projection, structured_root_account,
        structured_root_semantic_id, structured_root_data_id,
    }))
}

/// Private raw LinkV2 writer. Every crate-visible mutation above rederives one
/// exact legal successor and hostile-reauthenticates both sides.
fn write_series_market_link_v2<'next>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV2<'_>,
    successor: &SeriesMarketLinkV2,
    rebound_output: &'next mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedSeriesMarketLinkV2<'next>> {
    let binding = authenticated.state().binding_ref();
    require(account.is_writable && *account.key == authenticated.account()
        && account.owner == program_id && successor.binding_ref() == binding,
        ClutchError::MismatchedState)?;
    let live = authenticate_series_market_link_v2(
        program_id, account, binding.series_plan_id, binding.ordinal,
        binding.market_instance_id, binding.generation,
        Pubkey::new_from_array(binding.market_root_account_id.bytes()), true, rebound_output)?;
    require(live.account() == authenticated.account()
        && live.owner_program() == authenticated.owner_program()
        && live.value() == authenticated.value()
        && live.observed_lamports() == authenticated.observed_lamports()
        && live.data_id() == authenticated.data_id()
        && live.authentication_id() == authenticated.authentication_id(),
        ClutchError::MismatchedState)?;
    let mut data = account.try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV2::encode_parts(successor, authenticated.value().stored_bump, &mut data)?;
    drop(data);
    let rebound = authenticate_series_market_link_v2(
        program_id, account, binding.series_plan_id, binding.ordinal,
        binding.market_instance_id, binding.generation,
        Pubkey::new_from_array(binding.market_root_account_id.bytes()), true, rebound_output)?;
    require(rebound.state() == successor
        && rebound.value().stored_bump == authenticated.value().stored_bump,
        ClutchError::MismatchedState)?;
    Ok(rebound)
}

const SERIES_ADMISSION_COMPONENT_SEED_V4: u8 = 1;

fn is_retained_current_foundation_slot_v3(slot: MarketFoundationSlotV3) -> bool {
    matches!(
        slot,
        MarketFoundationSlotV3::FailureReplay
            | MarketFoundationSlotV3::FailureIntervalWork
            | MarketFoundationSlotV3::FailureIntervalHistory
            | MarketFoundationSlotV3::ResolutionV5
            | MarketFoundationSlotV3::FractionalPolicy
            | MarketFoundationSlotV3::FractionalLedger
            | MarketFoundationSlotV3::ProductReplayAnchor
    )
}

/// Move-only proof that Product physically moved one current Foundation debit
/// into the exact retained zero-data account and then reobserved both sides.
///
/// These slots are intentionally not allocated during founding. Their sole
/// later family writers consume the RootV2 transcript-bound preallocation,
/// allocate the exact current layout, and preserve its principal/donation
/// ownership. Holding only an account key or a balance cannot construct this
/// postwrite.
struct AuthenticatedProductMarketRetainedPreallocationPostwriteV3<'info> {
    id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    slot: MarketFoundationSlotV3,
    account_id: ContentId,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    minimum_donation_lamports: u64,
    destination_donation_lamports: u64,
    destination_observed_balance_lamports: u64,
    vault_observed_balance_lamports: u64,
    foundation_vault_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    foundation_vault: AccountInfo<'info>,
    destination: AccountInfo<'info>,
}

impl AuthenticatedProductMarketFoundationStepPostwriteV3
    for AuthenticatedProductMarketRetainedPreallocationPostwriteV3<'_>
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v3(
        self,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        slot: MarketFoundationSlotV3,
        account_id: ContentId,
        principal_lamports: u64,
        principal_before_lamports: u64,
        principal_after_lamports: u64,
        minimum_donation_lamports: u64,
        foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey,
        neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        require(
            self.id != ContentId::ZERO
                && is_retained_current_foundation_slot_v3(self.slot)
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && slot == self.slot
                && account_id == self.account_id
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && minimum_donation_lamports == self.minimum_donation_lamports
                && foundation_vault_account == self.foundation_vault_account
                && rent_refund_owner == self.rent_refund_owner
                && neutral_lamport_sink == self.neutral_lamport_sink
                && *self.foundation_vault.key == self.foundation_vault_account
                && *self.destination.key == Pubkey::new_from_array(self.account_id.bytes())
                && self.foundation_vault.is_writable
                && self.destination.is_writable
                && !self.foundation_vault.is_signer
                && !self.destination.is_signer
                && !self.foundation_vault.executable
                && !self.destination.executable
                && *self.foundation_vault.owner == SYSTEM_PROGRAM_ID
                && *self.destination.owner == SYSTEM_PROGRAM_ID
                && self.foundation_vault.data_len() == 0
                && self.destination.data_len() == 0
                && self.foundation_vault.lamports()
                    == self.vault_observed_balance_lamports
                && self.destination.lamports()
                    == self.destination_observed_balance_lamports
                && self.destination_observed_balance_lamports
                    == self
                        .destination_donation_lamports
                        .checked_add(self.principal_lamports)
                        .ok_or(ClutchError::Arithmetic)?,
            ClutchError::MismatchedState,
        )?;
        let observed_vault_donation_lamports = self
            .vault_observed_balance_lamports
            .checked_sub(self.principal_after_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        require(
            observed_vault_donation_lamports >= self.minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        Ok((self.id, observed_vault_donation_lamports))
    }
}

/// Current shared liability plan for slots 3, 4, and 14. The exact collateral
/// deployment is retained before any value-bearing token CPI; the plan cannot
/// be reconstructed from caller-provided semantic IDs.
#[derive(Debug)]
pub(crate) struct AuthenticatedCurrentMarketLiabilityFoundationPlanV3 {
    id: ContentId,
    bound: BoundCollateralProfileV2,
    deployment: AuthenticatedCollateralReleaseDeploymentV2,
    plan: MarketLiabilityFoundingPlanV3,
    hoard_custody: CustodyCreationPlanV2,
    graph_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    market_runtime_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    hoard_token_prefund_donation_lamports: u64,
}

impl AuthenticatedCurrentMarketLiabilityFoundationPlanV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
}

/// Freeze the liability accounts, their exact persisted rent owners, and the
/// Hoard collateral-token creation contract from current artifacts and PDAs.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn authenticate_current_market_liability_foundation_plan_v3(
    program_id: &Pubkey,
    bound: BoundCollateralProfileV2,
    deployment: AuthenticatedCollateralReleaseDeploymentV2,
    market_instance: MarketInstancePreimageV2,
    native_claim_basis_id: ContentId,
    market_runtime_account: Pubkey,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
    hoard_account: &AccountInfo<'_>,
    claim_ledger_account: &AccountInfo<'_>,
    hoard_token_account: &AccountInfo<'_>,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AuthenticatedCurrentMarketLiabilityFoundationPlanV3> {
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    market_instance
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_instance_id = market_instance
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = market_instance_id.bytes();
    let (expected_hoard, hoard_bump) = seeds::hoard_v2_pda(program_id, &market);
    let (expected_ledger, ledger_bump) = seeds::claim_ledger_v3_pda(program_id, &market);
    let expected_hoard_authority = seeds::hoard_authority_v2_pda(program_id, &market).0;
    let expected_hoard_token = seeds::hoard_token_v2_pda(program_id, &market).0;
    let expected_binding = seeds::general_v2_market_binding_pda(program_id, &market).0;
    let expected_runtime =
        seeds::general_v2_market_runtime_pda(program_id, &expected_binding.to_bytes()).0;
    let hoard_slot = MarketFoundationSlotV3::Hoard
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let ledger_slot = MarketFoundationSlotV3::ClaimLedger
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let token_slot = MarketFoundationSlotV3::HoardCollateralVault
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent = read_rent(rent_sysvar)?;
    let hoard_principal = schedule.slot_principal_lamports[hoard_slot];
    let ledger_principal = schedule.slot_principal_lamports[ledger_slot];
    require(
        bound.market().market.bytes() == market
            && bound.market().collateral_cap_atoms == market_instance.collateral_cap
            && bound.market().hoard_authority.bytes() == expected_hoard_authority.to_bytes()
            && bound.market().hoard_token_account.bytes() == expected_hoard_token.to_bytes()
            && deployment.release() == bound.release()
            && deployment.release_id()
                == bound
                    .release()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
            && deployment.programdata_account() != CollateralId::ZERO
            && deployment.receipt_id() != CollateralId::ZERO
            && deployment.deployment_slot() != 0
            && graph.market_instance_id == market_instance_id
            && graph.generation != 0
            && graph.account_ids[hoard_slot].bytes() == expected_hoard.to_bytes()
            && graph.account_ids[ledger_slot].bytes() == expected_ledger.to_bytes()
            && graph.account_ids[token_slot].bytes() == expected_hoard_token.to_bytes()
            && *hoard_account.key == expected_hoard
            && *claim_ledger_account.key == expected_ledger
            && *hoard_token_account.key == expected_hoard_token
            && market_runtime_account == expected_runtime
            && rent_refund_owner != Pubkey::default()
            && neutral_lamport_sink != Pubkey::default()
            && rent_refund_owner != neutral_lamport_sink
            && hoard_principal == rent.minimum_balance(HOARD_V2_BYTES)?
            && ledger_principal == rent.minimum_balance(CLAIM_LEDGER_V3_BYTES)?,
        ClutchError::MismatchedState,
    )?;
    for account in [hoard_account, claim_ledger_account, hoard_token_account] {
        require_unallocated_system_account(account)?;
        require(
            account.key != &rent_refund_owner
                && account.key != &neutral_lamport_sink
                && account.key != &market_runtime_account,
            ClutchError::AccountAlias,
        )?;
    }
    let payer = Identity32V1::new(rent_refund_owner.to_bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let hoard_rent = DeletableRentOwnerV1::from_persisted(
        payer,
        hoard_principal,
        hoard_account.lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let claim_ledger_rent = DeletableRentOwnerV1::from_persisted(
        payer,
        ledger_principal,
        claim_ledger_account.lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let plan = prepare_market_liability_founding_v3(
        bound,
        MarketLiabilityFoundingRequestV3 {
            hoard_account: CollateralId::from_bytes(expected_hoard.to_bytes()),
            claim_ledger_account: CollateralId::from_bytes(expected_ledger.to_bytes()),
            market_instance_id: CollateralId::from_bytes(market),
            native_claim_basis_id: CollateralId::from_bytes(native_claim_basis_id.bytes()),
            claim_mint_authority: CollateralId::from_bytes(expected_runtime.to_bytes()),
            outcome_count: schedule.outcome_count,
            hoard_bump,
            claim_ledger_bump: ledger_bump,
            hoard_rent,
            claim_ledger_rent,
        },
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let hoard_custody = prepare_hoard_creation_v2(bound)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let token_principal = schedule.slot_principal_lamports[token_slot];
    require(
        hoard_custody.account.bytes() == expected_hoard_token.to_bytes()
            && hoard_custody.owner_authority.bytes() == expected_hoard_authority.to_bytes()
            && token_principal
                == rent.minimum_balance(usize::from(hoard_custody.account_bytes))?,
        ClutchError::MismatchedState,
    )?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let id = hashv(&[
        PRODUCT_CURRENT_MARKET_LIABILITY_PLAN_DOMAIN_V3,
        program_id.as_ref(),
        &market,
        &graph.generation.to_le_bytes(),
        &graph_id.bytes(),
        &plan.founding_id().bytes(),
        &plan.hoard_id().bytes(),
        &plan.claim_ledger_id().bytes(),
        &deployment.release_id().bytes(),
        &deployment.programdata_account().bytes(),
        &deployment.deployment_slot().to_le_bytes(),
        &deployment.receipt_id().bytes(),
        &hoard_principal.to_le_bytes(),
        &ledger_principal.to_le_bytes(),
        &token_principal.to_le_bytes(),
        &hoard_account.lamports().to_le_bytes(),
        &claim_ledger_account.lamports().to_le_bytes(),
        &hoard_token_account.lamports().to_le_bytes(),
        rent_refund_owner.as_ref(),
        neutral_lamport_sink.as_ref(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedCurrentMarketLiabilityFoundationPlanV3 {
        id,
        bound,
        deployment,
        plan,
        hoard_custody,
        graph_id,
        market_instance_id,
        market_runtime_account,
        rent_refund_owner,
        neutral_lamport_sink,
        hoard_token_prefund_donation_lamports: hoard_token_account.lamports(),
    })
}

struct AuthenticatedProductMarketLiabilityStatePostwriteV3<'info> {
    id: ContentId,
    plan_authentication_id: ContentId,
    semantic_id: CollateralId,
    data_id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    slot: MarketFoundationSlotV3,
    account_id: ContentId,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    minimum_donation_lamports: u64,
    vault_observed_balance_lamports: u64,
    state_observed_balance_lamports: u64,
    foundation_vault_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    program_id: Pubkey,
    foundation_vault: AccountInfo<'info>,
    state_account: AccountInfo<'info>,
}

impl AuthenticatedProductMarketFoundationStepPostwriteV3
    for AuthenticatedProductMarketLiabilityStatePostwriteV3<'_>
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v3(
        self,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        slot: MarketFoundationSlotV3,
        account_id: ContentId,
        principal_lamports: u64,
        principal_before_lamports: u64,
        principal_after_lamports: u64,
        minimum_donation_lamports: u64,
        foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey,
        neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        let state_data = self
            .state_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observed_data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            self.state_account.key.as_ref(),
            &state_data,
        ]);
        drop(state_data);
        require(
            self.id != ContentId::ZERO
                && self.plan_authentication_id != ContentId::ZERO
                && self.semantic_id != CollateralId::ZERO
                && observed_data_id == self.data_id
                && matches!(self.slot, MarketFoundationSlotV3::Hoard
                    | MarketFoundationSlotV3::ClaimLedger)
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && slot == self.slot
                && account_id == self.account_id
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && minimum_donation_lamports == self.minimum_donation_lamports
                && foundation_vault_account == self.foundation_vault_account
                && rent_refund_owner == self.rent_refund_owner
                && neutral_lamport_sink == self.neutral_lamport_sink
                && *self.foundation_vault.key == self.foundation_vault_account
                && *self.foundation_vault.owner == SYSTEM_PROGRAM_ID
                && self.foundation_vault.data_len() == 0
                && self.foundation_vault.lamports() == self.vault_observed_balance_lamports
                && self.state_account.key.to_bytes() == self.account_id.bytes()
                && *self.state_account.owner == self.program_id
                && self.state_account.is_writable
                && !self.state_account.is_signer
                && !self.state_account.executable,
            ClutchError::MismatchedState,
        )?;
        require(
            self.state_account.lamports() == self.state_observed_balance_lamports,
            ClutchError::MismatchedState,
        )?;
        let observed_vault_donation = self
            .vault_observed_balance_lamports
            .checked_sub(self.principal_after_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        require(
            observed_vault_donation >= self.minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        Ok((self.id, observed_vault_donation))
    }
}

struct AuthenticatedProductMarketHoardCustodyPostwriteV3<'info> {
    id: ContentId,
    accepted: AuthenticatedMarketLiabilityFoundingPostwriteV3,
    plan_authentication_id: ContentId,
    hoard_data_id: ContentId,
    claim_ledger_data_id: ContentId,
    hoard_token_data_id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    account_id: ContentId,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    minimum_donation_lamports: u64,
    vault_observed_balance_lamports: u64,
    token_observed_balance_lamports: u64,
    foundation_vault_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    program_id: Pubkey,
    collateral_token_program: Pubkey,
    foundation_vault: AccountInfo<'info>,
    hoard_account: AccountInfo<'info>,
    claim_ledger_account: AccountInfo<'info>,
    hoard_token_account: AccountInfo<'info>,
}

impl AuthenticatedProductMarketFoundationStepPostwriteV3
    for AuthenticatedProductMarketHoardCustodyPostwriteV3<'_>
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v3(
        self,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        slot: MarketFoundationSlotV3,
        account_id: ContentId,
        principal_lamports: u64,
        principal_before_lamports: u64,
        principal_after_lamports: u64,
        minimum_donation_lamports: u64,
        foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey,
        neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        let hoard_data = self.hoard_account.try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observed_hoard_data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            self.hoard_account.key.as_ref(),
            &hoard_data,
        ]);
        drop(hoard_data);
        let ledger_data = self.claim_ledger_account.try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observed_ledger_data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            self.claim_ledger_account.key.as_ref(),
            &ledger_data,
        ]);
        drop(ledger_data);
        let token_data = self.hoard_token_account.try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observed_token_data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            self.hoard_token_account.key.as_ref(),
            &token_data,
        ]);
        drop(token_data);
        require(
            self.id != ContentId::ZERO
                && self.plan_authentication_id != ContentId::ZERO
                && self.accepted.receipt_id() != CollateralId::ZERO
                && self.accepted.accepted().receipt_id() != CollateralId::ZERO
                && self.accepted.deployment().receipt_id() != CollateralId::ZERO
                && observed_hoard_data_id == self.hoard_data_id
                && observed_ledger_data_id == self.claim_ledger_data_id
                && observed_token_data_id == self.hoard_token_data_id
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && slot == MarketFoundationSlotV3::HoardCollateralVault
                && account_id == self.account_id
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && minimum_donation_lamports == self.minimum_donation_lamports
                && foundation_vault_account == self.foundation_vault_account
                && rent_refund_owner == self.rent_refund_owner
                && neutral_lamport_sink == self.neutral_lamport_sink
                && *self.foundation_vault.key == self.foundation_vault_account
                && *self.foundation_vault.owner == SYSTEM_PROGRAM_ID
                && self.foundation_vault.data_len() == 0
                && self.foundation_vault.lamports() == self.vault_observed_balance_lamports
                && self.hoard_token_account.key.to_bytes() == self.account_id.bytes()
                && *self.hoard_account.owner == self.program_id
                && *self.claim_ledger_account.owner == self.program_id
                && *self.hoard_token_account.owner == self.collateral_token_program
                && self.hoard_account.is_writable
                && self.claim_ledger_account.is_writable
                && self.hoard_token_account.is_writable
                && !self.hoard_account.is_signer
                && !self.claim_ledger_account.is_signer
                && !self.hoard_token_account.is_signer
                && !self.hoard_account.executable
                && !self.claim_ledger_account.executable
                && !self.hoard_token_account.executable
                && self.hoard_token_account.lamports() == self.token_observed_balance_lamports,
            ClutchError::MismatchedState,
        )?;
        let observed_vault_donation = self.vault_observed_balance_lamports
            .checked_sub(self.principal_after_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        require(
            observed_vault_donation >= self.minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        Ok((self.id, observed_vault_donation))
    }
}

/// Current claim-mint plan reconstructed from the complete GraphV3 and the
/// independently authenticated Token-2022 claim release.
#[derive(Debug)]
pub(crate) struct AuthenticatedCurrentClaimMintFoundationPlanV2 {
    id: ContentId,
    plan: ClaimMintFoundingPlanV2,
    claim_release: AuthenticatedClaimIssuanceReleaseV1,
    general_value: GeneralMarketValueAuthorityV2,
    graph_id: ContentId,
    market_runtime_account: Pubkey,
}

impl AuthenticatedCurrentClaimMintFoundationPlanV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
}

/// Reconstruct the exact active OutcomeMintV2 prefix from current PDAs. The
/// inactive tail remains graph-level canonical absence and is never accepted
/// as an account list supplied by a caller.
#[inline(never)]
pub(crate) fn authenticate_current_claim_mint_foundation_plan_v2(
    program_id: &Pubkey,
    general_value: GeneralMarketValueAuthorityV2,
    claim_release: AuthenticatedClaimIssuanceReleaseV1,
    market_runtime_account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
) -> Outcome<AuthenticatedCurrentClaimMintFoundationPlanV2> {
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = general_value
        .liabilities
        .market_binding
        .base()
        .market_instance_v2_id
        .bytes();
    let market_id = MarketInstanceV2Id::from_bytes(market);
    let expected_binding = seeds::general_v2_market_binding_pda(program_id, &market).0;
    let expected_runtime =
        seeds::general_v2_market_runtime_pda(program_id, &expected_binding.to_bytes()).0;
    let claim = claim_release.bound();
    require(
        graph.market_instance_id == market_id
            && graph.generation != 0
            && *market_runtime_account.key == expected_runtime
            && market_runtime_account.owner == program_id
            && !market_runtime_account.is_signer
            && !market_runtime_account.executable
            && general_value.liabilities.market_runtime.market_instance_v2_id.bytes() == market
            && general_value.liabilities.market_runtime.market_binding.bytes()
                == expected_binding.to_bytes()
            && claim.binding_id().bytes()
                == general_value
                    .liabilities
                    .market_binding
                    .base()
                    .claim_issuance_binding_id
                    .bytes()
            && claim_release.receipt_id() != CollateralId::ZERO
            && claim_release.token_programdata() != CollateralId::ZERO
            && claim_release.loader_receipt_id() != CollateralId::ZERO
            && claim_release.deployment_slot() != 0,
        ClutchError::MismatchedState,
    )?;
    let mut outcome_mints = [CollateralId::ZERO; MARKET_FOUNDATION_MAX_OUTCOMES_V3];
    let mut index = 0usize;
    while index < MARKET_FOUNDATION_MAX_OUTCOMES_V3 {
        let outcome = u8::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
        let slot = MarketFoundationSlotV3::OutcomeMint(outcome)
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let mint_id = graph.account_ids[slot];
        if index < usize::from(schedule.outcome_count) {
            let expected_mint = seeds::outcome_mint_v2_pda(program_id, &market, outcome).0;
            require(mint_id.bytes() == expected_mint.to_bytes(), ClutchError::WrongPda)?;
            outcome_mints[index] = CollateralId::from_bytes(mint_id.bytes());
        } else {
            require(mint_id == ContentId::ZERO, ClutchError::MismatchedState)?;
        }
        index = index.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    let plan = prepare_claim_mint_founding_v2(
        claim,
        ClaimMintFoundingRequestV2 {
            market_instance_id: CollateralId::from_bytes(market),
            mint_authority: CollateralId::from_bytes(expected_runtime.to_bytes()),
            outcome_count: schedule.outcome_count,
            outcome_mints,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let id = hashv(&[
        PRODUCT_CURRENT_CLAIM_MINT_PLAN_DOMAIN_V2,
        program_id.as_ref(),
        &plan.founding_id().bytes(),
        &claim_release.receipt_id().bytes(),
        &claim_release.token_programdata().bytes(),
        &claim_release.deployment_slot().to_le_bytes(),
        &claim_release.loader_receipt_id().bytes(),
        &general_value.receipt_id.bytes(),
        &graph_id.bytes(),
        market_runtime_account.key.as_ref(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedCurrentClaimMintFoundationPlanV2 {
        id,
        plan,
        claim_release,
        general_value,
        graph_id,
        market_runtime_account: *market_runtime_account.key,
    })
}

struct AuthenticatedProductMarketClaimMintPostwriteV2<'info> {
    id: ContentId,
    accepted_receipt_id: CollateralId,
    claim_plan_authentication_id: ContentId,
    claim_release_receipt_id: CollateralId,
    claim_programdata_id: CollateralId,
    claim_loader_receipt_id: CollateralId,
    general_value_authentication_id: CollateralId,
    mint_data_id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    slot: MarketFoundationSlotV3,
    account_id: ContentId,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    minimum_donation_lamports: u64,
    vault_observed_balance_lamports: u64,
    mint_observed_balance_lamports: u64,
    foundation_vault_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    claim_token_program: Pubkey,
    foundation_vault: AccountInfo<'info>,
    mint: AccountInfo<'info>,
}

impl AuthenticatedProductMarketFoundationStepPostwriteV3
    for AuthenticatedProductMarketClaimMintPostwriteV2<'_>
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v3(
        self,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        slot: MarketFoundationSlotV3,
        account_id: ContentId,
        principal_lamports: u64,
        principal_before_lamports: u64,
        principal_after_lamports: u64,
        minimum_donation_lamports: u64,
        foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey,
        neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        let mint_data = self
            .mint
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observed_mint_data_id = hashv(&[
            PRODUCT_CURRENT_CLAIM_MINT_POSTWRITE_DOMAIN_V2,
            self.mint.key.as_ref(),
            &mint_data,
        ]);
        drop(mint_data);
        require(
            self.id != ContentId::ZERO
                && self.accepted_receipt_id != CollateralId::ZERO
                && self.claim_plan_authentication_id != ContentId::ZERO
                && self.claim_release_receipt_id != CollateralId::ZERO
                && self.claim_programdata_id != CollateralId::ZERO
                && self.claim_loader_receipt_id != CollateralId::ZERO
                && self.general_value_authentication_id != CollateralId::ZERO
                && observed_mint_data_id == self.mint_data_id
                && matches!(self.slot, MarketFoundationSlotV3::OutcomeMint(_))
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && slot == self.slot
                && account_id == self.account_id
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && minimum_donation_lamports == self.minimum_donation_lamports
                && foundation_vault_account == self.foundation_vault_account
                && rent_refund_owner == self.rent_refund_owner
                && neutral_lamport_sink == self.neutral_lamport_sink
                && *self.foundation_vault.key == self.foundation_vault_account
                && *self.foundation_vault.owner == SYSTEM_PROGRAM_ID
                && self.foundation_vault.data_len() == 0
                && self.foundation_vault.lamports() == self.vault_observed_balance_lamports
                && self.mint.key.to_bytes() == self.account_id.bytes()
                && *self.mint.owner == self.claim_token_program
                && self.mint.is_writable
                && !self.mint.is_signer
                && !self.mint.executable
                && self.mint.lamports() == self.mint_observed_balance_lamports,
            ClutchError::MismatchedState,
        )?;
        let observed_vault_donation = self
            .vault_observed_balance_lamports
            .checked_sub(self.principal_after_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        require(
            observed_vault_donation >= self.minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        Ok((self.id, observed_vault_donation))
    }
}

/// Current release-bound custody plan reconstructed from the complete GraphV3.
///
/// Private fields prevent a caller from pairing arbitrary token accounts with
/// Product slots. The retained collateral value authority includes the exact
/// Realm/Profile policy, ProgramData release, Hoard, ClaimLedger, and General
/// MarketBinding/Runtime poststates.
#[derive(Debug)]
pub(crate) struct AuthenticatedCurrentOutcomeCustodyFoundationPlanV1 {
    id: ContentId,
    plan: OutcomeCustodyFoundingPlanV1,
    value: GeneralMarketValueAuthorityV2,
    graph_id: ContentId,
    market_runtime_account: Pubkey,
}

impl AuthenticatedCurrentOutcomeCustodyFoundationPlanV1 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
}

/// Reconstruct the exact active custody suffix from canonical current PDAs.
/// Inactive mint/custody tails must remain zero in GraphV3 and never appear as
/// physical accounts.
#[inline(never)]
pub(crate) fn authenticate_current_outcome_custody_foundation_plan_v1(
    program_id: &Pubkey,
    value: GeneralMarketValueAuthorityV2,
    market_runtime_account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
) -> Outcome<AuthenticatedCurrentOutcomeCustodyFoundationPlanV1> {
    schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = value
        .liabilities
        .market_binding
        .base()
        .market_instance_v2_id
        .bytes();
    let market_id = MarketInstanceV2Id::from_bytes(market);
    let expected_binding = seeds::general_v2_market_binding_pda(program_id, &market).0;
    let expected_runtime =
        seeds::general_v2_market_runtime_pda(program_id, &expected_binding.to_bytes()).0;
    require(
        graph.market_instance_id == market_id
            && graph.generation != 0
            && *market_runtime_account.key == expected_runtime
            && market_runtime_account.owner == program_id
            && !market_runtime_account.is_signer
            && !market_runtime_account.executable
            && value.liabilities.market_runtime.market_instance_v2_id.bytes() == market
            && value.liabilities.market_runtime.market_binding.bytes()
                == expected_binding.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let mut outcome_mints = [CollateralId::ZERO; MARKET_FOUNDATION_MAX_OUTCOMES_V3];
    let mut outcome_custodies = [CollateralId::ZERO; MARKET_FOUNDATION_MAX_OUTCOMES_V3];
    let mut index = 0usize;
    while index < MARKET_FOUNDATION_MAX_OUTCOMES_V3 {
        let outcome = u8::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
        let mint_slot = MarketFoundationSlotV3::OutcomeMint(outcome)
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let custody_slot = MarketFoundationSlotV3::OutcomeCustody(outcome)
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let mint_id = graph.account_ids[mint_slot];
        let custody_id = graph.account_ids[custody_slot];
        if index < usize::from(schedule.outcome_count) {
            let expected_mint = seeds::outcome_mint_v2_pda(program_id, &market, outcome).0;
            let expected_custody =
                seeds::outcome_custody_v1_pda(program_id, &market, graph.generation, outcome).0;
            require(
                mint_id.bytes() == expected_mint.to_bytes()
                    && custody_id.bytes() == expected_custody.to_bytes(),
                ClutchError::WrongPda,
            )?;
            outcome_mints[index] = CollateralId::from_bytes(mint_id.bytes());
            outcome_custodies[index] = CollateralId::from_bytes(custody_id.bytes());
        } else {
            require(
                mint_id == ContentId::ZERO && custody_id == ContentId::ZERO,
                ClutchError::MismatchedState,
            )?;
        }
        index = index.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    let plan = prepare_outcome_custody_founding_v1(
        value.liabilities.bound,
        OutcomeCustodyFoundingRequestV1 {
            market_instance_id: CollateralId::from_bytes(market),
            generation: graph.generation,
            owner_authority: CollateralId::from_bytes(expected_runtime.to_bytes()),
            outcome_count: schedule.outcome_count,
            outcome_mints,
            outcome_custodies,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let id = hashv(&[
        PRODUCT_CURRENT_OUTCOME_CUSTODY_PLAN_DOMAIN_V1,
        program_id.as_ref(),
        &plan.founding_id().bytes(),
        &value.receipt_id.bytes(),
        &value.deployment.receipt_id().bytes(),
        &value.deployment.programdata_account().bytes(),
        &value.deployment.deployment_slot().to_le_bytes(),
        &graph_id.bytes(),
        market_runtime_account.key.as_ref(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedCurrentOutcomeCustodyFoundationPlanV1 {
        id,
        plan,
        value,
        graph_id,
        market_runtime_account: *market_runtime_account.key,
    })
}

struct AuthenticatedProductMarketOutcomeCustodyPostwriteV1<'info> {
    id: ContentId,
    accepted_receipt_id: CollateralId,
    custody_plan_authentication_id: ContentId,
    collateral_value_authentication_id: CollateralId,
    collateral_deployment_receipt_id: CollateralId,
    claim_release_receipt_id: CollateralId,
    custody_data_id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    slot: MarketFoundationSlotV3,
    account_id: ContentId,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    minimum_donation_lamports: u64,
    vault_observed_balance_lamports: u64,
    custody_observed_balance_lamports: u64,
    foundation_vault_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    collateral_token_program: Pubkey,
    foundation_vault: AccountInfo<'info>,
    custody: AccountInfo<'info>,
}

impl AuthenticatedProductMarketFoundationStepPostwriteV3
    for AuthenticatedProductMarketOutcomeCustodyPostwriteV1<'_>
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v3(
        self,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        slot: MarketFoundationSlotV3,
        account_id: ContentId,
        principal_lamports: u64,
        principal_before_lamports: u64,
        principal_after_lamports: u64,
        minimum_donation_lamports: u64,
        foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey,
        neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        let custody_data = self
            .custody
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observed_custody_data_id = hashv(&[
            PRODUCT_CURRENT_OUTCOME_CUSTODY_POSTWRITE_DOMAIN_V1,
            self.custody.key.as_ref(),
            &custody_data,
        ]);
        drop(custody_data);
        require(
            self.id != ContentId::ZERO
                && self.accepted_receipt_id != CollateralId::ZERO
                && self.custody_plan_authentication_id != ContentId::ZERO
                && self.collateral_value_authentication_id != CollateralId::ZERO
                && self.collateral_deployment_receipt_id != CollateralId::ZERO
                && self.claim_release_receipt_id != CollateralId::ZERO
                && observed_custody_data_id == self.custody_data_id
                && matches!(self.slot, MarketFoundationSlotV3::OutcomeCustody(_))
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && slot == self.slot
                && account_id == self.account_id
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && minimum_donation_lamports == self.minimum_donation_lamports
                && foundation_vault_account == self.foundation_vault_account
                && rent_refund_owner == self.rent_refund_owner
                && neutral_lamport_sink == self.neutral_lamport_sink
                && *self.foundation_vault.key == self.foundation_vault_account
                && *self.foundation_vault.owner == SYSTEM_PROGRAM_ID
                && self.foundation_vault.data_len() == 0
                && self.foundation_vault.lamports()
                    == self.vault_observed_balance_lamports
                && self.custody.key.to_bytes() == self.account_id.bytes()
                && *self.custody.owner == self.collateral_token_program
                && self.custody.is_writable
                && !self.custody.is_signer
                && !self.custody.executable
                && self.custody.lamports() == self.custody_observed_balance_lamports,
            ClutchError::MismatchedState,
        )?;
        let observed_vault_donation = self
            .vault_observed_balance_lamports
            .checked_sub(self.principal_after_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        require(
            observed_vault_donation >= self.minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        Ok((self.id, observed_vault_donation))
    }
}

fn current_collateral_runtime_view<'a>(
    account: &AccountInfo<'_>,
    data: &'a [u8],
) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: CollateralId::from_bytes(account.key.to_bytes()),
        owner_program: CollateralId::from_bytes(account.owner.to_bytes()),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

fn invoke_current_outcome_custody_initialization_v1<'info>(
    creation: clutch_collateral_adapter_v2::CustodyCreationPlanV2,
    custody: &AccountInfo<'info>,
    collateral_mint: &AccountInfo<'info>,
    collateral_token_program: &AccountInfo<'info>,
) -> Outcome<()> {
    require(
        creation.account.bytes() == custody.key.to_bytes()
            && creation.mint.bytes() == collateral_mint.key.to_bytes()
            && creation.token_program.bytes() == collateral_token_program.key.to_bytes()
            && creation.step_count != 0
            && usize::from(creation.step_count) <= creation.steps.len(),
        ClutchError::MismatchedState,
    )?;
    let mut index = 0usize;
    while index < usize::from(creation.step_count) {
        match creation.steps[index] {
            CustodyInitializationStepV2::None => {
                return Err(Refusal::Adapter(ClutchError::MismatchedState));
            }
            CustodyInitializationStepV2::InitializeImmutableOwner { account, data } => {
                require(
                    account.bytes() == custody.key.to_bytes(),
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *collateral_token_program.key,
                    &data,
                    vec![AccountMeta::new(*custody.key, false)],
                );
                invoke(
                    &instruction,
                    &[custody.clone(), collateral_token_program.clone()],
                )
                .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
            }
            CustodyInitializationStepV2::InitializeAccount3 {
                account,
                mint,
                owner_authority,
                data,
            } => {
                require(
                    account.bytes() == custody.key.to_bytes()
                        && mint.bytes() == collateral_mint.key.to_bytes()
                        && owner_authority == creation.owner_authority,
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *collateral_token_program.key,
                    &data,
                    vec![
                        AccountMeta::new(*custody.key, false),
                        AccountMeta::new_readonly(*collateral_mint.key, false),
                    ],
                );
                invoke(
                    &instruction,
                    &[
                        custody.clone(),
                        collateral_mint.clone(),
                        collateral_token_program.clone(),
                    ],
                )
                .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
            }
        }
        index = index.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    Ok(())
}

/// Cursor which keeps the unique founder creation authority inside one SBF
/// call while concrete family composers consume heterogeneous typed slot
/// postwrites. A failed or incomplete closure cannot return the private final
/// receipt, so every earlier CPI and RootV2 write rolls back.
pub(crate) struct CurrentProductMarketFoundationCursorV4<'outer, 'info> {
    program_id: &'outer Pubkey,
    creation: AuthenticatedProductMarketFounderCurrentCreationV3,
    schedule: &'outer MarketFoundationScheduleV3,
    graph: &'outer MarketFoundationAccountGraphV3,
    root_account: &'outer AccountInfo<'info>,
    market_liability_plan_id: ContentId,
    market_core_liability_plan: Option<(
        BoundCollateralProfileV2,
        CustodyCreationPlanV2,
        MarketLiabilityFoundingPlanV3,
    )>,
    claim_mint_plan: Option<ClaimMintFoundingPlanV2>,
    outcome_custody_plan: Option<OutcomeCustodyFoundingPlanV1>,
    hoard_slot_receipt_id: ContentId,
    claim_ledger_slot_receipt_id: ContentId,
    accepted_market_liability: Option<AcceptedMarketLiabilityFoundingV3>,
    accepted_claim_mints: [Option<AcceptedClaimMintFoundingStepV2>;
        MARKET_FOUNDATION_MAX_OUTCOMES_V3],
    accepted_outcome_custodies: [Option<AcceptedOutcomeCustodyFoundingStepV1>;
        MARKET_FOUNDATION_MAX_OUTCOMES_V3],
}

impl<'outer, 'info> CurrentProductMarketFoundationCursorV4<'outer, 'info> {
    /// Fund and consume the next canonical retained zero-data slot.
    ///
    /// The transfer and RootV2 transcript write are deliberately inseparable:
    /// this method does not expose the physical postwrite or a reusable debit
    /// authority to a family caller.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_next_retained_preallocation<'next>(
        &mut self,
        root: AuthenticatedMarketLifecycleRootV2<'_>,
        foundation_vault: &AccountInfo<'info>,
        destination: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        successor_output: &mut MarketLifecycleRootV2,
        rebound_output: &'next mut MarketLifecycleRootAccountV2,
    ) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>> {
        require_system_program(system_program)?;
        self.schedule
            .validate()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.graph
            .validate(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let slot = self.creation.next_foundation_slot_v3()?;
        require(
            is_retained_current_foundation_slot_v3(slot),
            ClutchError::MismatchedState,
        )?;
        let preauthorization = self.creation.preauthorization();
        let state = root.state();
        let capital = state.capital();
        let binding_id = state
            .binding_ref()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let schedule_id = self
            .schedule
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let graph_id = self
            .graph
            .id(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let index = slot
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let account_id = self
            .graph
            .account(slot)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let principal_lamports = self.schedule.slot_principal_lamports[index];
        let principal_before_lamports = capital.principal_remaining_lamports;
        let principal_after_lamports = principal_before_lamports
            .checked_sub(principal_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        let minimum_donation_lamports = capital.vault_current_donation_lamports;
        let rent_refund_owner = Pubkey::new_from_array(capital.rent_refund_owner.bytes());
        let neutral_lamport_sink =
            Pubkey::new_from_array(capital.neutral_lamport_sink.bytes());
        let market = preauthorization.market_instance_id().bytes();
        let generation = preauthorization.generation();
        let (expected_vault, vault_bump) =
            seeds::product_market_foundation_vault_pda(self.program_id, &market, generation);
        require(
            root.is_writable()
                && root.account() == *self.root_account.key
                && root.owner_program() == *self.program_id
                && state.phase() == MarketLifecyclePhaseV2::Founding
                && state.binding_ref() == self.creation.market_binding()
                && binding_id
                    == self
                        .creation
                        .market_binding()
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                && schedule_id.content_id() == preauthorization.foundation_schedule_id()
                && graph_id.content_id() == preauthorization.foundation_graph_id()
                && principal_lamports != 0
                && account_id.bytes() == destination.key.to_bytes()
                && *foundation_vault.key == expected_vault
                && *foundation_vault.key == preauthorization.foundation_vault_account()
                && foundation_vault.key != destination.key
                && destination.key != self.root_account.key
                && destination.key != &rent_refund_owner
                && destination.key != &neutral_lamport_sink
                && foundation_vault.key != &rent_refund_owner
                && foundation_vault.key != &neutral_lamport_sink,
            ClutchError::MismatchedState,
        )?;
        require_system_vault(foundation_vault)?;
        require_unallocated_system_account(destination)?;

        let vault_before = foundation_vault.lamports();
        let observed_vault_donation = vault_before
            .checked_sub(principal_before_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        let vault_after = principal_after_lamports
            .checked_add(observed_vault_donation)
            .ok_or(ClutchError::Arithmetic)?;
        let destination_donation = destination.lamports();
        let destination_after = destination_donation
            .checked_add(principal_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        require(
            observed_vault_donation >= minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        let generation_bytes = generation.to_le_bytes();
        let bump_seed = [vault_bump];
        invoke_current_founder_transfer(
            foundation_vault,
            destination,
            system_program,
            principal_lamports,
            &[
                seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
                &market,
                &generation_bytes,
                &bump_seed,
            ],
        )?;
        require(
            foundation_vault.lamports() == vault_after
                && destination.lamports() == destination_after,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
        let id = hashv(&[
            PRODUCT_CURRENT_RETAINED_PREALLOCATION_POSTWRITE_DOMAIN_V3,
            self.program_id.as_ref(),
            &self.creation.id().bytes(),
            &preauthorization.id().bytes(),
            &self.creation.foundation_steps_id().bytes(),
            &binding_id.bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &slot_index.to_le_bytes(),
            destination.key.as_ref(),
            &principal_lamports.to_le_bytes(),
            &principal_before_lamports.to_le_bytes(),
            &principal_after_lamports.to_le_bytes(),
            &minimum_donation_lamports.to_le_bytes(),
            &destination_donation.to_le_bytes(),
            &destination_after.to_le_bytes(),
            foundation_vault.key.as_ref(),
            &vault_before.to_le_bytes(),
            &vault_after.to_le_bytes(),
            rent_refund_owner.as_ref(),
            neutral_lamport_sink.as_ref(),
        ]);
        require_live(id)?;
        let postwrite = AuthenticatedProductMarketRetainedPreallocationPostwriteV3 {
            id,
            founder_creation_receipt_id: self.creation.id(),
            founder_preauthorization_id: preauthorization.id(),
            foundation_steps_id: self.creation.foundation_steps_id(),
            market_binding_id: binding_id,
            foundation_schedule_id: schedule_id.content_id(),
            foundation_graph_id: graph_id.content_id(),
            slot,
            account_id,
            principal_lamports,
            principal_before_lamports,
            principal_after_lamports,
            minimum_donation_lamports,
            destination_donation_lamports: destination_donation,
            destination_observed_balance_lamports: destination_after,
            vault_observed_balance_lamports: vault_after,
            foundation_vault_account: *foundation_vault.key,
            rent_refund_owner,
            neutral_lamport_sink,
            foundation_vault: foundation_vault.clone(),
            destination: destination.clone(),
        };
        self.record_foundation_step(root, postwrite, successor_output, rebound_output)
    }

    /// Create either canonical program-owned shared-liability state account.
    /// HoardV2 must be slot 3 and ClaimLedgerV3 slot 4; the cursor retains both
    /// receipts until slot 14 supplies the exact external Hoard custody.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_next_market_liability_state_v3<'next>(
        &mut self,
        root: AuthenticatedMarketLifecycleRootV2<'_>,
        liability_plan: &AuthenticatedCurrentMarketLiabilityFoundationPlanV3,
        foundation_vault: &AccountInfo<'info>,
        state_account: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        successor_output: &mut MarketLifecycleRootV2,
        rebound_output: &'next mut MarketLifecycleRootAccountV2,
    ) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>> {
        require_system_program(system_program)?;
        let slot = self.creation.next_foundation_slot_v3()?;
        let (
            expected_account,
            account_bytes,
            stored_bump,
            semantic_id,
            expected_principal,
            expected_donation,
        ) =
            match slot {
                MarketFoundationSlotV3::Hoard => {
                    let value = liability_plan.plan.hoard();
                    (
                        Pubkey::new_from_array(liability_plan.plan.hoard_account().bytes()),
                        HOARD_V2_BYTES,
                        value.stored_bump,
                        liability_plan.plan.hoard_id(),
                        value.rent.refundable_principal(),
                        value.rent.donation_floor(),
                    )
                }
                MarketFoundationSlotV3::ClaimLedger => {
                    let value = liability_plan.plan.claim_ledger();
                    (
                        Pubkey::new_from_array(
                            liability_plan.plan.claim_ledger_account().bytes(),
                        ),
                        CLAIM_LEDGER_V3_BYTES,
                        value.stored_bump,
                        liability_plan.plan.claim_ledger_id(),
                        value.rent.refundable_principal(),
                        value.rent.donation_floor(),
                    )
                }
                _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
            };
        let preauthorization = self.creation.preauthorization();
        let state = root.state();
        let capital = state.capital();
        let binding_id = state
            .binding_ref()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let schedule_id = self.schedule.id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let graph_id = self.graph.id(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let index = slot.index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let principal_lamports = self.schedule.slot_principal_lamports[index];
        let principal_before_lamports = capital.principal_remaining_lamports;
        let principal_after_lamports = principal_before_lamports
            .checked_sub(principal_lamports).ok_or(ClutchError::Arithmetic)?;
        let minimum_donation_lamports = capital.vault_current_donation_lamports;
        let market = preauthorization.market_instance_id().bytes();
        let (expected_vault, vault_bump) = seeds::product_market_foundation_vault_pda(
            self.program_id, &market, preauthorization.generation());
        require(
            root.is_writable()
                && root.account() == *self.root_account.key
                && root.owner_program() == *self.program_id
                && state.phase() == MarketLifecyclePhaseV2::Founding
                && state.binding_ref() == self.creation.market_binding()
                && graph_id.content_id() == liability_plan.graph_id
                && liability_plan.market_instance_id == preauthorization.market_instance_id()
                && liability_plan.market_runtime_account
                    == seeds::general_v2_market_runtime_pda(
                        self.program_id,
                        &seeds::general_v2_market_binding_pda(self.program_id, &market)
                            .0
                            .to_bytes(),
                    )
                    .0
                && liability_plan.rent_refund_owner
                    == Pubkey::new_from_array(capital.rent_refund_owner.bytes())
                && liability_plan.neutral_lamport_sink
                    == Pubkey::new_from_array(capital.neutral_lamport_sink.bytes())
                && self.graph.account_ids[index].bytes() == expected_account.to_bytes()
                && *state_account.key == expected_account
                && principal_lamports == expected_principal
                && *foundation_vault.key == expected_vault
                && *foundation_vault.key == preauthorization.foundation_vault_account()
                && foundation_vault.key != state_account.key
                && state_account.key != &liability_plan.rent_refund_owner
                && state_account.key != &liability_plan.neutral_lamport_sink
                && (self.market_liability_plan_id == ContentId::ZERO
                    || self.market_liability_plan_id == liability_plan.id)
                && self.market_core_liability_plan.as_ref().map_or(
                    true,
                    |current| {
                        current
                            == &(
                                liability_plan.bound,
                                liability_plan.hoard_custody,
                                liability_plan.plan,
                            )
                    },
                ),
            ClutchError::MismatchedState,
        )?;
        require_system_vault(foundation_vault)?;
        require_unallocated_system_account(state_account)?;
        let vault_before = foundation_vault.lamports();
        let observed_vault_donation = vault_before
            .checked_sub(principal_before_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        let vault_after = principal_after_lamports
            .checked_add(observed_vault_donation).ok_or(ClutchError::Arithmetic)?;
        let state_donation = state_account.lamports();
        require(state_donation == expected_donation, ClutchError::MismatchedState)?;
        let state_after = state_donation
            .checked_add(principal_lamports).ok_or(ClutchError::Arithmetic)?;
        require(
            observed_vault_donation >= minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        let generation_bytes = preauthorization.generation().to_le_bytes();
        let vault_bump_seed = [vault_bump];
        invoke_current_founder_transfer(
            foundation_vault,
            state_account,
            system_program,
            principal_lamports,
            &[
                seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
                &market,
                &generation_bytes,
                &vault_bump_seed,
            ],
        )?;
        let seed = match slot {
            MarketFoundationSlotV3::Hoard => seeds::SEED_HOARD_V2,
            MarketFoundationSlotV3::ClaimLedger => seeds::SEED_CLAIM_LEDGER_V3,
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
        let state_bump_seed = [stored_bump];
        allocate_assign_current_founder_account(
            self.program_id,
            state_account,
            system_program,
            account_bytes,
            &[seed, &market, &state_bump_seed],
        )?;
        {
            let mut output = state_account
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            match slot {
                MarketFoundationSlotV3::Hoard => liability_plan
                    .plan
                    .hoard()
                    .encode(&mut output[..])
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
                MarketFoundationSlotV3::ClaimLedger => liability_plan
                    .plan
                    .claim_ledger()
                    .encode(&mut output[..])
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
                _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
            }
        }
        let state_data = state_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            state_account.key.as_ref(),
            &state_data,
        ]);
        match slot {
            MarketFoundationSlotV3::Hoard => require(
                HoardV2::decode(&state_data)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    == liability_plan.plan.hoard(),
                ClutchError::MismatchedState,
            )?,
            MarketFoundationSlotV3::ClaimLedger => require(
                ClaimLedgerV3::decode(&state_data)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    == liability_plan.plan.claim_ledger(),
                ClutchError::MismatchedState,
            )?,
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        }
        drop(state_data);
        require(
            foundation_vault.lamports() == vault_after
                && state_account.lamports() == state_after
                && *state_account.owner == *self.program_id,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
        let id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            self.program_id.as_ref(),
            &self.creation.id().bytes(),
            &preauthorization.id().bytes(),
            &liability_plan.id.bytes(),
            &semantic_id.bytes(),
            &binding_id.bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &slot_index.to_le_bytes(),
            state_account.key.as_ref(),
            &data_id.bytes(),
            &principal_lamports.to_le_bytes(),
            &principal_before_lamports.to_le_bytes(),
            &principal_after_lamports.to_le_bytes(),
            &state_donation.to_le_bytes(),
            &state_after.to_le_bytes(),
            &vault_before.to_le_bytes(),
            &vault_after.to_le_bytes(),
        ]);
        require_live(id)?;
        let postwrite = AuthenticatedProductMarketLiabilityStatePostwriteV3 {
            id,
            plan_authentication_id: liability_plan.id,
            semantic_id,
            data_id,
            founder_creation_receipt_id: self.creation.id(),
            founder_preauthorization_id: preauthorization.id(),
            foundation_steps_id: self.creation.foundation_steps_id(),
            market_binding_id: binding_id,
            foundation_schedule_id: schedule_id.content_id(),
            foundation_graph_id: graph_id.content_id(),
            slot,
            account_id: self.graph.account_ids[index],
            principal_lamports,
            principal_before_lamports,
            principal_after_lamports,
            minimum_donation_lamports,
            vault_observed_balance_lamports: vault_after,
            state_observed_balance_lamports: state_after,
            foundation_vault_account: *foundation_vault.key,
            rent_refund_owner: liability_plan.rent_refund_owner,
            neutral_lamport_sink: liability_plan.neutral_lamport_sink,
            program_id: *self.program_id,
            foundation_vault: foundation_vault.clone(),
            state_account: state_account.clone(),
        };
        let next_root =
            self.record_foundation_step(root, postwrite, successor_output, rebound_output)?;
        self.market_liability_plan_id = liability_plan.id;
        if self.market_core_liability_plan.is_none() {
            self.market_core_liability_plan = Some((
                liability_plan.bound,
                liability_plan.hoard_custody,
                liability_plan.plan,
            ));
        }
        match slot {
            MarketFoundationSlotV3::Hoard => self.hoard_slot_receipt_id = id,
            MarketFoundationSlotV3::ClaimLedger => self.claim_ledger_slot_receipt_id = id,
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        }
        Ok(next_root)
    }

    /// Create the release-selected Hoard collateral vault at slot 14 and only
    /// then accept the complete HoardV2/ClaimLedgerV3/custody founding plane.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_current_hoard_collateral_vault_v3<'next>(
        &mut self,
        root: AuthenticatedMarketLifecycleRootV2<'_>,
        liability_plan: &AuthenticatedCurrentMarketLiabilityFoundationPlanV3,
        foundation_vault: &AccountInfo<'info>,
        hoard_account: &AccountInfo<'info>,
        claim_ledger_account: &AccountInfo<'info>,
        hoard_token_account: &AccountInfo<'info>,
        collateral_mint: &AccountInfo<'info>,
        collateral_token_program: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        rent_sysvar: &AccountInfo<'info>,
        successor_output: &mut MarketLifecycleRootV2,
        rebound_output: &'next mut MarketLifecycleRootAccountV2,
    ) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>> {
        require_system_program(system_program)?;
        let slot = self.creation.next_foundation_slot_v3()?;
        require(
            slot == MarketFoundationSlotV3::HoardCollateralVault
                && self.market_liability_plan_id == liability_plan.id
                && self.hoard_slot_receipt_id != ContentId::ZERO
                && self.claim_ledger_slot_receipt_id != ContentId::ZERO
                && self.accepted_market_liability.is_none(),
            ClutchError::MismatchedState,
        )?;
        let state = root.state();
        let capital = state.capital();
        let preauthorization = self.creation.preauthorization();
        let binding_id = state.binding_ref().id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let schedule_id = self.schedule.id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let graph_id = self.graph.id(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let index = slot.index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let account_id = self.graph.account_ids[index];
        let principal_lamports = self.schedule.slot_principal_lamports[index];
        let principal_before_lamports = capital.principal_remaining_lamports;
        let principal_after_lamports = principal_before_lamports
            .checked_sub(principal_lamports).ok_or(ClutchError::Arithmetic)?;
        let minimum_donation_lamports = capital.vault_current_donation_lamports;
        let market = preauthorization.market_instance_id().bytes();
        let (expected_token, token_bump) = seeds::hoard_token_v2_pda(self.program_id, &market);
        let (expected_vault, vault_bump) = seeds::product_market_foundation_vault_pda(
            self.program_id, &market, preauthorization.generation());
        let creation = liability_plan.hoard_custody;
        require(
            root.is_writable()
                && root.account() == *self.root_account.key
                && root.owner_program() == *self.program_id
                && state.phase() == MarketLifecyclePhaseV2::Founding
                && state.binding_ref() == self.creation.market_binding()
                && graph_id.content_id() == liability_plan.graph_id
                && liability_plan.market_instance_id == preauthorization.market_instance_id()
                && account_id.bytes() == expected_token.to_bytes()
                && *hoard_token_account.key == expected_token
                && hoard_account.key.to_bytes()
                    == liability_plan.plan.hoard_account().bytes()
                && claim_ledger_account.key.to_bytes()
                    == liability_plan.plan.claim_ledger_account().bytes()
                && collateral_token_program.key.to_bytes() == creation.token_program.bytes()
                && collateral_mint.key.to_bytes() == creation.mint.bytes()
                && creation.account.bytes() == expected_token.to_bytes()
                && creation.owner_authority
                    == liability_plan.bound.market().hoard_authority
                && liability_plan.deployment.release() == liability_plan.bound.release()
                && liability_plan.deployment.release_id()
                    == liability_plan.bound.release().id()
                        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
                && liability_plan.deployment.programdata_account() != CollateralId::ZERO
                && liability_plan.deployment.receipt_id() != CollateralId::ZERO
                && hoard_token_account.lamports()
                    == liability_plan.hoard_token_prefund_donation_lamports
                && *foundation_vault.key == expected_vault
                && *foundation_vault.key == preauthorization.foundation_vault_account()
                && foundation_vault.key != hoard_token_account.key
                && hoard_token_account.key != collateral_mint.key
                && hoard_token_account.key != collateral_token_program.key
                && hoard_token_account.key != &liability_plan.rent_refund_owner
                && hoard_token_account.key != &liability_plan.neutral_lamport_sink,
            ClutchError::MismatchedState,
        )?;
        require_system_vault(foundation_vault)?;
        require_unallocated_system_account(hoard_token_account)?;
        require(
            !collateral_token_program.is_signer
                && !collateral_token_program.is_writable
                && collateral_token_program.executable
                && !collateral_mint.is_signer
                && !collateral_mint.is_writable
                && !collateral_mint.executable,
            ClutchError::MismatchedState,
        )?;
        let collateral_mint_data = collateral_mint.try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let collateral_observation = admit_collateral_mint_v2(
            liability_plan.bound,
            current_collateral_runtime_view(collateral_mint, &collateral_mint_data),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MintNotAdmitted))?;
        drop(collateral_mint_data);
        require(
            collateral_observation.address.bytes() == collateral_mint.key.to_bytes(),
            ClutchError::MismatchedState,
        )?;
        let rent = read_rent(rent_sysvar)?;
        require(
            principal_lamports
                == rent.minimum_balance(usize::from(creation.account_bytes))?,
            ClutchError::MismatchedState,
        )?;
        let vault_before = foundation_vault.lamports();
        let observed_vault_donation = vault_before
            .checked_sub(principal_before_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        let vault_after = principal_after_lamports
            .checked_add(observed_vault_donation).ok_or(ClutchError::Arithmetic)?;
        let token_donation = hoard_token_account.lamports();
        let token_after = token_donation
            .checked_add(principal_lamports).ok_or(ClutchError::Arithmetic)?;
        require(
            observed_vault_donation >= minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        let generation_bytes = preauthorization.generation().to_le_bytes();
        let vault_bump_seed = [vault_bump];
        invoke_current_founder_transfer(
            foundation_vault,
            hoard_token_account,
            system_program,
            principal_lamports,
            &[
                seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
                &market,
                &generation_bytes,
                &vault_bump_seed,
            ],
        )?;
        let token_bump_seed = [token_bump];
        allocate_assign_current_founder_account(
            collateral_token_program.key,
            hoard_token_account,
            system_program,
            usize::from(creation.account_bytes),
            &[seeds::SEED_HOARD_TOKEN_V2, &market, &token_bump_seed],
        )?;
        invoke_current_outcome_custody_initialization_v1(
            creation,
            hoard_token_account,
            collateral_mint,
            collateral_token_program,
        )?;
        let accepted = accept_general_market_liability_founding_postwrite_v3(
            self.program_id,
            liability_plan.bound,
            liability_plan.deployment,
            liability_plan.plan,
            hoard_account,
            claim_ledger_account,
            hoard_token_account,
        )?;
        let accepted_pure = accepted.accepted();
        let hoard_data = hoard_account.try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            hoard_account.key.as_ref(), &hoard_data]);
        drop(hoard_data);
        let ledger_data = claim_ledger_account.try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let claim_ledger_data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            claim_ledger_account.key.as_ref(), &ledger_data]);
        drop(ledger_data);
        let token_data = hoard_token_account.try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let hoard_token_data_id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            hoard_token_account.key.as_ref(), &token_data]);
        drop(token_data);
        require(
            foundation_vault.lamports() == vault_after
                && hoard_token_account.lamports() == token_after
                && accepted_pure.plan() == liability_plan.plan
                && accepted_pure.visible_hoard_atoms() == 0,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
        let id = hashv(&[
            PRODUCT_CURRENT_MARKET_LIABILITY_SLOT_POSTWRITE_DOMAIN_V3,
            self.program_id.as_ref(),
            &self.creation.id().bytes(),
            &preauthorization.id().bytes(),
            &liability_plan.id.bytes(),
            &self.hoard_slot_receipt_id.bytes(),
            &self.claim_ledger_slot_receipt_id.bytes(),
            &accepted.receipt_id().bytes(),
            &accepted_pure.receipt_id().bytes(),
            &liability_plan.deployment.receipt_id().bytes(),
            &binding_id.bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &slot_index.to_le_bytes(),
            hoard_token_account.key.as_ref(),
            collateral_mint.key.as_ref(),
            collateral_token_program.key.as_ref(),
            &hoard_data_id.bytes(),
            &claim_ledger_data_id.bytes(),
            &hoard_token_data_id.bytes(),
            &principal_lamports.to_le_bytes(),
            &principal_before_lamports.to_le_bytes(),
            &principal_after_lamports.to_le_bytes(),
            &token_donation.to_le_bytes(),
            &token_after.to_le_bytes(),
            &vault_before.to_le_bytes(),
            &vault_after.to_le_bytes(),
        ]);
        require_live(id)?;
        let postwrite = AuthenticatedProductMarketHoardCustodyPostwriteV3 {
            id,
            accepted,
            plan_authentication_id: liability_plan.id,
            hoard_data_id,
            claim_ledger_data_id,
            hoard_token_data_id,
            founder_creation_receipt_id: self.creation.id(),
            founder_preauthorization_id: preauthorization.id(),
            foundation_steps_id: self.creation.foundation_steps_id(),
            market_binding_id: binding_id,
            foundation_schedule_id: schedule_id.content_id(),
            foundation_graph_id: graph_id.content_id(),
            account_id,
            principal_lamports,
            principal_before_lamports,
            principal_after_lamports,
            minimum_donation_lamports,
            vault_observed_balance_lamports: vault_after,
            token_observed_balance_lamports: token_after,
            foundation_vault_account: *foundation_vault.key,
            rent_refund_owner: liability_plan.rent_refund_owner,
            neutral_lamport_sink: liability_plan.neutral_lamport_sink,
            program_id: *self.program_id,
            collateral_token_program: *collateral_token_program.key,
            foundation_vault: foundation_vault.clone(),
            hoard_account: hoard_account.clone(),
            claim_ledger_account: claim_ledger_account.clone(),
            hoard_token_account: hoard_token_account.clone(),
        };
        let next_root =
            self.record_foundation_step(root, postwrite, successor_output, rebound_output)?;
        self.accepted_market_liability = Some(accepted_pure);
        Ok(next_root)
    }

    /// Create one active exact Token-2022 OutcomeMintV2 and consume its
    /// hostile postwrite into the next canonical Product slot.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_next_claim_mint_v2<'next>(
        &mut self,
        root: AuthenticatedMarketLifecycleRootV2<'_>,
        claim_plan: &AuthenticatedCurrentClaimMintFoundationPlanV2,
        foundation_vault: &AccountInfo<'info>,
        outcome_mint: &AccountInfo<'info>,
        claim_token_program: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        rent_sysvar: &AccountInfo<'info>,
        successor_output: &mut MarketLifecycleRootV2,
        rebound_output: &'next mut MarketLifecycleRootAccountV2,
    ) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>> {
        require_system_program(system_program)?;
        self.schedule
            .validate()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.graph
            .validate(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let slot = self.creation.next_foundation_slot_v3()?;
        let outcome = match slot {
            MarketFoundationSlotV3::OutcomeMint(outcome) => outcome,
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
        require(outcome < self.schedule.outcome_count, ClutchError::MismatchedState)?;
        let step = claim_plan
            .plan
            .step(outcome)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            self.accepted_claim_mints[usize::from(outcome)].is_none()
                && self
                    .claim_mint_plan
                    .as_ref()
                    .map_or(true, |current| current == &claim_plan.plan),
            ClutchError::MismatchedState,
        )?;
        let state = root.state();
        let capital = state.capital();
        let preauthorization = self.creation.preauthorization();
        let binding_id = state
            .binding_ref()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let schedule_id = self
            .schedule
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let graph_id = self
            .graph
            .id(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let index = slot
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let account_id = self.graph.account_ids[index];
        let principal_lamports = self.schedule.slot_principal_lamports[index];
        let principal_before_lamports = capital.principal_remaining_lamports;
        let principal_after_lamports = principal_before_lamports
            .checked_sub(principal_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        let minimum_donation_lamports = capital.vault_current_donation_lamports;
        let rent_refund_owner = Pubkey::new_from_array(capital.rent_refund_owner.bytes());
        let neutral_lamport_sink = Pubkey::new_from_array(capital.neutral_lamport_sink.bytes());
        let market = preauthorization.market_instance_id().bytes();
        let expected_runtime = claim_plan.market_runtime_account;
        let (expected_mint, mint_bump) =
            seeds::outcome_mint_v2_pda(self.program_id, &market, outcome);
        let (expected_vault, vault_bump) = seeds::product_market_foundation_vault_pda(
            self.program_id,
            &market,
            preauthorization.generation(),
        );
        let claim_release = claim_plan.claim_release;
        let claim_binding = claim_release.bound();
        require(
            root.is_writable()
                && root.account() == *self.root_account.key
                && root.owner_program() == *self.program_id
                && state.phase() == MarketLifecyclePhaseV2::Founding
                && state.binding_ref() == self.creation.market_binding()
                && binding_id
                    == self
                        .creation
                        .market_binding()
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                && schedule_id.content_id() == preauthorization.foundation_schedule_id()
                && graph_id.content_id() == preauthorization.foundation_graph_id()
                && graph_id.content_id() == claim_plan.graph_id
                && claim_plan.plan.market_instance_id().bytes() == market
                && claim_plan.plan.outcome_count() == self.schedule.outcome_count
                && claim_plan.plan.mint_authority().bytes() == expected_runtime.to_bytes()
                && claim_plan.plan.binding_id() == claim_binding.binding_id()
                && step.mint().bytes() == expected_mint.to_bytes()
                && account_id.bytes() == expected_mint.to_bytes()
                && outcome_mint.key == &expected_mint
                && claim_token_program.key.to_bytes()
                    == claim_binding.binding().token_program.bytes()
                && *claim_token_program.key == token::TOKEN_2022_PROGRAM_ID
                && claim_release.receipt_id() != CollateralId::ZERO
                && claim_release.token_programdata() != CollateralId::ZERO
                && claim_release.loader_receipt_id() != CollateralId::ZERO
                && claim_plan.general_value.receipt_id != CollateralId::ZERO
                && claim_binding.binding_id().bytes()
                    == state.binding_ref().claim_issuance_binding_id.bytes()
                && principal_lamports != 0
                && *foundation_vault.key == expected_vault
                && *foundation_vault.key == preauthorization.foundation_vault_account()
                && foundation_vault.key != outcome_mint.key
                && outcome_mint.key != claim_token_program.key
                && outcome_mint.key != &expected_runtime
                && outcome_mint.key != &rent_refund_owner
                && outcome_mint.key != &neutral_lamport_sink,
            ClutchError::MismatchedState,
        )?;
        require_system_vault(foundation_vault)?;
        require_unallocated_system_account(outcome_mint)?;
        require(
            !claim_token_program.is_signer
                && !claim_token_program.is_writable
                && claim_token_program.executable,
            ClutchError::MismatchedState,
        )?;
        let mint_account_bytes = claim_plan.plan.mint_account_bytes();
        let rent = read_rent(rent_sysvar)?;
        require(
            principal_lamports == rent.minimum_balance(mint_account_bytes)?,
            ClutchError::MismatchedState,
        )?;
        let vault_before = foundation_vault.lamports();
        let observed_vault_donation = vault_before
            .checked_sub(principal_before_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        let vault_after = principal_after_lamports
            .checked_add(observed_vault_donation)
            .ok_or(ClutchError::Arithmetic)?;
        let mint_donation = outcome_mint.lamports();
        let mint_after = mint_donation
            .checked_add(principal_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        require(
            observed_vault_donation >= minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        let generation_bytes = preauthorization.generation().to_le_bytes();
        let vault_bump_seed = [vault_bump];
        invoke_current_founder_transfer(
            foundation_vault,
            outcome_mint,
            system_program,
            principal_lamports,
            &[
                seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
                &market,
                &generation_bytes,
                &vault_bump_seed,
            ],
        )?;
        require(
            foundation_vault.lamports() == vault_after && outcome_mint.lamports() == mint_after,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        let outcome_seed = [outcome];
        let mint_bump_seed = [mint_bump];
        allocate_assign_current_founder_account(
            claim_token_program.key,
            outcome_mint,
            system_program,
            mint_account_bytes,
            &[
                seeds::SEED_OUTCOME_MINT_V2,
                &market,
                &outcome_seed,
                &mint_bump_seed,
            ],
        )?;
        token::initialize_outcome_mint(claim_token_program, outcome_mint, &expected_runtime)?;
        let observation = token::admit_mint(
            outcome_mint,
            &token::MintPolicy::outcome(*outcome_mint.key, expected_runtime),
        )?;
        let account_bytes = u16::try_from(mint_account_bytes)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        let accepted = accept_claim_mint_founding_step_v2(
            claim_binding,
            step,
            ClaimMintFoundingPostwriteV2 {
                mint: CollateralId::from_bytes(outcome_mint.key.to_bytes()),
                owner_program: CollateralId::from_bytes(outcome_mint.owner.to_bytes()),
                writable: outcome_mint.is_writable,
                signer: outcome_mint.is_signer,
                executable: outcome_mint.executable,
                account_bytes,
                initialized: true,
                decimals: observation.decimals,
                supply_atoms: observation.supply,
                mint_authority: observation.mint_authority.map(CollateralId::from_bytes),
                freeze_authority: observation.freeze_authority.map(CollateralId::from_bytes),
                extensions: observation.extensions,
            },
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MintNotAdmitted))?;
        let mint_data = outcome_mint
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mint_data_id = hashv(&[
            PRODUCT_CURRENT_CLAIM_MINT_POSTWRITE_DOMAIN_V2,
            outcome_mint.key.as_ref(),
            &mint_data,
        ]);
        drop(mint_data);
        require(
            accepted.step() == step
                && outcome_mint.lamports() == mint_after
                && *outcome_mint.owner == *claim_token_program.key,
            ClutchError::MismatchedState,
        )?;
        let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
        let id = hashv(&[
            PRODUCT_CURRENT_CLAIM_MINT_POSTWRITE_DOMAIN_V2,
            self.program_id.as_ref(),
            &self.creation.id().bytes(),
            &preauthorization.id().bytes(),
            &self.creation.foundation_steps_id().bytes(),
            &binding_id.bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &slot_index.to_le_bytes(),
            &[outcome],
            outcome_mint.key.as_ref(),
            claim_token_program.key.as_ref(),
            &accepted.receipt_id().bytes(),
            &claim_plan.id.bytes(),
            &claim_release.receipt_id().bytes(),
            &claim_release.token_programdata().bytes(),
            &claim_release.loader_receipt_id().bytes(),
            &claim_plan.general_value.receipt_id.bytes(),
            &mint_data_id.bytes(),
            &principal_lamports.to_le_bytes(),
            &principal_before_lamports.to_le_bytes(),
            &principal_after_lamports.to_le_bytes(),
            &mint_donation.to_le_bytes(),
            &mint_after.to_le_bytes(),
            &vault_before.to_le_bytes(),
            &vault_after.to_le_bytes(),
            rent_refund_owner.as_ref(),
            neutral_lamport_sink.as_ref(),
        ]);
        require_live(id)?;
        let postwrite = AuthenticatedProductMarketClaimMintPostwriteV2 {
            id,
            accepted_receipt_id: accepted.receipt_id(),
            claim_plan_authentication_id: claim_plan.id,
            claim_release_receipt_id: claim_release.receipt_id(),
            claim_programdata_id: claim_release.token_programdata(),
            claim_loader_receipt_id: claim_release.loader_receipt_id(),
            general_value_authentication_id: claim_plan.general_value.receipt_id,
            mint_data_id,
            founder_creation_receipt_id: self.creation.id(),
            founder_preauthorization_id: preauthorization.id(),
            foundation_steps_id: self.creation.foundation_steps_id(),
            market_binding_id: binding_id,
            foundation_schedule_id: schedule_id.content_id(),
            foundation_graph_id: graph_id.content_id(),
            slot,
            account_id,
            principal_lamports,
            principal_before_lamports,
            principal_after_lamports,
            minimum_donation_lamports,
            vault_observed_balance_lamports: vault_after,
            mint_observed_balance_lamports: mint_after,
            foundation_vault_account: *foundation_vault.key,
            rent_refund_owner,
            neutral_lamport_sink,
            claim_token_program: *claim_token_program.key,
            foundation_vault: foundation_vault.clone(),
            mint: outcome_mint.clone(),
        };
        let next_root =
            self.record_foundation_step(root, postwrite, successor_output, rebound_output)?;
        self.accepted_claim_mints[usize::from(outcome)] = Some(accepted);
        if self.claim_mint_plan.is_none() {
            self.claim_mint_plan = Some(claim_plan.plan);
        }
        Ok(next_root)
    }

    /// Create one active release-selected outcome custody and consume its
    /// hostile postwrite into the next canonical Product slot.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_next_outcome_custody_v1<'next>(
        &mut self,
        root: AuthenticatedMarketLifecycleRootV2<'_>,
        custody_plan: &AuthenticatedCurrentOutcomeCustodyFoundationPlanV1,
        claim_release: AuthenticatedClaimIssuanceReleaseV1,
        foundation_vault: &AccountInfo<'info>,
        custody: &AccountInfo<'info>,
        collateral_mint: &AccountInfo<'info>,
        outcome_mint: &AccountInfo<'info>,
        collateral_token_program: &AccountInfo<'info>,
        system_program: &AccountInfo<'info>,
        rent_sysvar: &AccountInfo<'info>,
        successor_output: &mut MarketLifecycleRootV2,
        rebound_output: &'next mut MarketLifecycleRootAccountV2,
    ) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>> {
        require_system_program(system_program)?;
        self.schedule
            .validate()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.graph
            .validate(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let slot = self.creation.next_foundation_slot_v3()?;
        let outcome = match slot {
            MarketFoundationSlotV3::OutcomeCustody(outcome) => outcome,
            _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
        require(
            outcome < self.schedule.outcome_count,
            ClutchError::MismatchedState,
        )?;
        let step = custody_plan
            .plan
            .step(outcome)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let accepted_mint = self.accepted_claim_mints[usize::from(outcome)]
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            self.accepted_outcome_custodies[usize::from(outcome)].is_none()
                && accepted_mint.step().outcome() == outcome
                && accepted_mint.step().market_instance_id() == step.market_instance_id()
                && accepted_mint.step().mint() == step.outcome_mint()
                && accepted_mint.step().mint_authority() == step.owner_authority()
                && self
                    .outcome_custody_plan
                    .as_ref()
                    .map_or(true, |current| current == &custody_plan.plan)
                && accepted_mint.step().binding_id() == claim_release.bound().binding_id(),
            ClutchError::MismatchedState,
        )?;
        let creation = step.creation();
        let state = root.state();
        let capital = state.capital();
        let preauthorization = self.creation.preauthorization();
        let binding_id = state
            .binding_ref()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let schedule_id = self
            .schedule
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let graph_id = self
            .graph
            .id(self.schedule)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let index = slot
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let account_id = self.graph.account_ids[index];
        let principal_lamports = self.schedule.slot_principal_lamports[index];
        let principal_before_lamports = capital.principal_remaining_lamports;
        let principal_after_lamports = principal_before_lamports
            .checked_sub(principal_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        let minimum_donation_lamports = capital.vault_current_donation_lamports;
        let rent_refund_owner = Pubkey::new_from_array(capital.rent_refund_owner.bytes());
        let neutral_lamport_sink =
            Pubkey::new_from_array(capital.neutral_lamport_sink.bytes());
        let market = preauthorization.market_instance_id().bytes();
        let generation = preauthorization.generation();
        let expected_runtime = custody_plan.market_runtime_account;
        let expected_mint = seeds::outcome_mint_v2_pda(self.program_id, &market, outcome).0;
        let (expected_custody, custody_bump) =
            seeds::outcome_custody_v1_pda(self.program_id, &market, generation, outcome);
        let (expected_vault, vault_bump) =
            seeds::product_market_foundation_vault_pda(self.program_id, &market, generation);
        let claim_binding = claim_release.bound();
        let claim_token_program = claim_binding.binding().token_program;
        require(
            root.is_writable()
                && root.account() == *self.root_account.key
                && root.owner_program() == *self.program_id
                && state.phase() == MarketLifecyclePhaseV2::Founding
                && state.binding_ref() == self.creation.market_binding()
                && binding_id
                    == self
                        .creation
                        .market_binding()
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                && schedule_id.content_id() == preauthorization.foundation_schedule_id()
                && graph_id.content_id() == preauthorization.foundation_graph_id()
                && graph_id.content_id() == custody_plan.graph_id
                && custody_plan.plan.market_instance_id().bytes() == market
                && custody_plan.plan.generation() == generation
                && custody_plan.plan.outcome_count() == self.schedule.outcome_count
                && custody_plan.plan.owner_authority().bytes() == expected_runtime.to_bytes()
                && step.outcome_mint().bytes() == expected_mint.to_bytes()
                && step.outcome_custody().bytes() == expected_custody.to_bytes()
                && account_id.bytes() == expected_custody.to_bytes()
                && custody.key == &expected_custody
                && outcome_mint.key == &expected_mint
                && collateral_token_program.key.to_bytes() == creation.token_program.bytes()
                && collateral_mint.key.to_bytes() == creation.mint.bytes()
                && creation.owner_authority.bytes() == expected_runtime.to_bytes()
                && claim_binding.binding_id().bytes()
                    == state.binding_ref().claim_issuance_binding_id.bytes()
                && outcome_mint.owner.to_bytes() == claim_token_program.bytes()
                && claim_release.receipt_id() != CollateralId::ZERO
                && custody_plan.value.receipt_id != CollateralId::ZERO
                && custody_plan.value.deployment.release() == custody_plan.value.liabilities.bound.release()
                && custody_plan.value.deployment.release_id()
                    == custody_plan
                        .value
                        .liabilities
                        .bound
                        .release()
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
                && collateral_token_program.key.to_bytes()
                    == custody_plan.value.deployment.release().token_program.bytes()
                && principal_lamports != 0
                && *foundation_vault.key == expected_vault
                && *foundation_vault.key == preauthorization.foundation_vault_account()
                && foundation_vault.key != custody.key
                && custody.key != collateral_mint.key
                && custody.key != outcome_mint.key
                && custody.key != collateral_token_program.key
                && custody.key != &expected_runtime
                && custody.key != &rent_refund_owner
                && custody.key != &neutral_lamport_sink,
            ClutchError::MismatchedState,
        )?;
        require_system_vault(foundation_vault)?;
        require_unallocated_system_account(custody)?;
        require(
            !collateral_token_program.is_signer
                && !collateral_token_program.is_writable
                && collateral_token_program.executable
                && !collateral_mint.is_signer
                && !collateral_mint.is_writable
                && !collateral_mint.executable
                && !outcome_mint.is_signer
                && !outcome_mint.is_writable
                && !outcome_mint.executable,
            ClutchError::MismatchedState,
        )?;
        let collateral_mint_data = collateral_mint
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let collateral_mint_observation = admit_collateral_mint_v2(
            custody_plan.value.liabilities.bound,
            current_collateral_runtime_view(collateral_mint, &collateral_mint_data),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MintNotAdmitted))?;
        drop(collateral_mint_data);
        require(
            collateral_mint_observation.address.bytes() == collateral_mint.key.to_bytes(),
            ClutchError::MismatchedState,
        )?;
        let outcome_mint_observation = token::admit_mint(
            outcome_mint,
            &token::MintPolicy::outcome(*outcome_mint.key, expected_runtime),
        )
        .map_err(Refusal::from)?;
        require(
            outcome_mint_observation.supply == 0
                && outcome_mint_observation.decimals == 0
                && outcome_mint_observation.mint_authority == Some(expected_runtime.to_bytes())
                && outcome_mint_observation.freeze_authority.is_none(),
            ClutchError::MintNotAdmitted,
        )?;

        let rent = read_rent(rent_sysvar)?;
        require(
            principal_lamports == rent.minimum_balance(usize::from(creation.account_bytes))?,
            ClutchError::MismatchedState,
        )?;
        let vault_before = foundation_vault.lamports();
        let observed_vault_donation = vault_before
            .checked_sub(principal_before_lamports)
            .ok_or(ClutchError::MismatchedState)?;
        let vault_after = principal_after_lamports
            .checked_add(observed_vault_donation)
            .ok_or(ClutchError::Arithmetic)?;
        let custody_donation = custody.lamports();
        let custody_after = custody_donation
            .checked_add(principal_lamports)
            .ok_or(ClutchError::Arithmetic)?;
        require(
            observed_vault_donation >= minimum_donation_lamports,
            ClutchError::MismatchedState,
        )?;
        let generation_bytes = generation.to_le_bytes();
        let vault_bump_seed = [vault_bump];
        invoke_current_founder_transfer(
            foundation_vault,
            custody,
            system_program,
            principal_lamports,
            &[
                seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
                &market,
                &generation_bytes,
                &vault_bump_seed,
            ],
        )?;
        require(
            foundation_vault.lamports() == vault_after && custody.lamports() == custody_after,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        let outcome_seed = [outcome];
        let custody_bump_seed = [custody_bump];
        allocate_assign_current_founder_account(
            collateral_token_program.key,
            custody,
            system_program,
            usize::from(creation.account_bytes),
            &[
                seeds::SEED_OUTCOME_CUSTODY_V1,
                &market,
                &generation_bytes,
                &outcome_seed,
                &custody_bump_seed,
            ],
        )?;
        invoke_current_outcome_custody_initialization_v1(
            creation,
            custody,
            collateral_mint,
            collateral_token_program,
        )?;
        let custody_data = custody
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let accepted = accept_outcome_custody_founding_step_v1(
            step,
            current_collateral_runtime_view(custody, &custody_data),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let custody_data_id = hashv(&[
            PRODUCT_CURRENT_OUTCOME_CUSTODY_POSTWRITE_DOMAIN_V1,
            custody.key.as_ref(),
            &custody_data,
        ]);
        drop(custody_data);
        require(
            accepted.step() == step
                && custody.lamports() == custody_after
                && *custody.owner == *collateral_token_program.key,
            ClutchError::MismatchedState,
        )?;
        let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
        let id = hashv(&[
            PRODUCT_CURRENT_OUTCOME_CUSTODY_POSTWRITE_DOMAIN_V1,
            self.program_id.as_ref(),
            &self.creation.id().bytes(),
            &preauthorization.id().bytes(),
            &self.creation.foundation_steps_id().bytes(),
            &binding_id.bytes(),
            &schedule_id.bytes(),
            &graph_id.bytes(),
            &slot_index.to_le_bytes(),
            &[outcome],
            custody.key.as_ref(),
            outcome_mint.key.as_ref(),
            collateral_mint.key.as_ref(),
            collateral_token_program.key.as_ref(),
            &accepted.receipt_id().bytes(),
            &custody_plan.id.bytes(),
            &custody_plan.value.receipt_id.bytes(),
            &custody_plan.value.deployment.receipt_id().bytes(),
            &claim_release.receipt_id().bytes(),
            &custody_data_id.bytes(),
            &principal_lamports.to_le_bytes(),
            &principal_before_lamports.to_le_bytes(),
            &principal_after_lamports.to_le_bytes(),
            &custody_donation.to_le_bytes(),
            &custody_after.to_le_bytes(),
            &vault_before.to_le_bytes(),
            &vault_after.to_le_bytes(),
            rent_refund_owner.as_ref(),
            neutral_lamport_sink.as_ref(),
        ]);
        require_live(id)?;
        let postwrite = AuthenticatedProductMarketOutcomeCustodyPostwriteV1 {
            id,
            accepted_receipt_id: accepted.receipt_id(),
            custody_plan_authentication_id: custody_plan.id,
            collateral_value_authentication_id: custody_plan.value.receipt_id,
            collateral_deployment_receipt_id: custody_plan.value.deployment.receipt_id(),
            claim_release_receipt_id: claim_release.receipt_id(),
            custody_data_id,
            founder_creation_receipt_id: self.creation.id(),
            founder_preauthorization_id: preauthorization.id(),
            foundation_steps_id: self.creation.foundation_steps_id(),
            market_binding_id: binding_id,
            foundation_schedule_id: schedule_id.content_id(),
            foundation_graph_id: graph_id.content_id(),
            slot,
            account_id,
            principal_lamports,
            principal_before_lamports,
            principal_after_lamports,
            minimum_donation_lamports,
            vault_observed_balance_lamports: vault_after,
            custody_observed_balance_lamports: custody_after,
            foundation_vault_account: *foundation_vault.key,
            rent_refund_owner,
            neutral_lamport_sink,
            collateral_token_program: *collateral_token_program.key,
            foundation_vault: foundation_vault.clone(),
            custody: custody.clone(),
        };
        let next_root =
            self.record_foundation_step(root, postwrite, successor_output, rebound_output)?;
        self.accepted_outcome_custodies[usize::from(outcome)] = Some(accepted);
        if self.outcome_custody_plan.is_none() {
            self.outcome_custody_plan = Some(custody_plan.plan);
        }
        Ok(next_root)
    }

    /// Consume one exact family-private physical postwrite, advance RootV2,
    /// persist it, and hostile-reopen it before another slot can be consumed.
    fn record_foundation_step<'next, P>(
        &mut self,
        root: AuthenticatedMarketLifecycleRootV2<'_>,
        postwrite: P,
        successor_output: &mut MarketLifecycleRootV2,
        rebound_output: &'next mut MarketLifecycleRootAccountV2,
    ) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>>
    where
        P: AuthenticatedProductMarketFoundationStepPostwriteV3,
    {
        let step = self.creation.take_next_foundation_step_v3(
            root,
            self.schedule,
            self.graph,
            postwrite,
        )?;
        root.state()
            .record_foundation_step_into(
                self.schedule,
                self.graph,
                step,
                successor_output,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        write_market_lifecycle_root_v2(
            self.program_id,
            self.root_account,
            root,
            successor_output,
            rebound_output,
        )
    }

    /// Consume the exhaustive cursor, physically create the founder LinkV2,
    /// admit it into the complete RootV2, and run the sole Replay/Funding tail.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish(
        self,
        complete_root: AuthenticatedMarketLifecycleRootV2<'_>,
        registry: &AuthenticatedRegistryCapabilityV4,
        bundle: AuthenticatedCompiledProductSeriesBundleV6,
        artifacts: &AuthenticatedSeriesSourceArtifactsV5,
        series_admission_vault: &AccountInfo<'info>,
        link_account: &AccountInfo<'info>,
        funding_account: &AccountInfo<'info>,
        replay_account: &AccountInfo<'info>,
        authenticated_replay: AuthenticatedSeriesLifecycleReplayV2,
        system_program: &AccountInfo<'info>,
        rent_sysvar: &AccountInfo<'info>,
        link_initial_output: &mut SeriesMarketLinkAccountV2,
        root_admission_output: &mut MarketLifecycleRootV2,
        root_admission_rebound: &mut MarketLifecycleRootAccountV2,
        root_activation_output: &mut MarketLifecycleRootV2,
        root_activation_rebound: &mut MarketLifecycleRootAccountV2,
        link_activation_output: &mut SeriesMarketLinkV2,
        link_activation_rebound: &mut SeriesMarketLinkAccountV2,
    ) -> Outcome<AuthenticatedProductSeriesActivationCompletionV4> {
        require(
            self.market_liability_plan_id != ContentId::ZERO
                && self.hoard_slot_receipt_id != ContentId::ZERO
                && self.claim_ledger_slot_receipt_id != ContentId::ZERO
                && self.accepted_market_liability.is_some(),
            ClutchError::MismatchedState,
        )?;
        let active_outcomes = usize::from(self.schedule.outcome_count);
        let mut outcome_index = 0usize;
        while outcome_index < MARKET_FOUNDATION_MAX_OUTCOMES_V3 {
            let active = outcome_index < active_outcomes;
            require(
                active == self.accepted_claim_mints[outcome_index].is_some()
                    && active == self.accepted_outcome_custodies[outcome_index].is_some(),
                ClutchError::MismatchedState,
            )?;
            outcome_index = outcome_index
                .checked_add(1)
                .ok_or(ClutchError::Arithmetic)?;
        }
        let (bound, hoard_custody, liability_plan) = self
            .market_core_liability_plan
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let claim_mint_plan = self
            .claim_mint_plan
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let outcome_custody_plan = self
            .outcome_custody_plan
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let accepted_liability = self
            .accepted_market_liability
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let market_core = compose_market_core_founding_v4(
            bound,
            liability_plan,
            hoard_custody,
            claim_mint_plan,
            outcome_custody_plan,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let accepted_market_core = accept_market_core_founding_v4(
            market_core,
            accepted_liability,
            self.accepted_claim_mints,
            self.accepted_outcome_custodies,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            accepted_market_core.receipt_id().bytes()
                == self.creation.accepted_market_core_receipt_id().bytes(),
            ClutchError::MismatchedState,
        )?;
        let preauthorization = self.creation.preauthorization();
        let series = preauthorization.series_plan_id();
        let ordinal = preauthorization.ordinal();
        let market = preauthorization.market_instance_id();
        let generation = preauthorization.generation();
        let link_binding = self.creation.founder_link_binding();
        let obligation_configuration = self.creation.obligation_configuration();
        let expected_link_semantic_id = self.creation.founder_link_semantic_id();
        let pending_funding = self.creation.funding_reservation().pending().state();
        let component_index = SeriesFundingComponentV2::SeriesAdmission.index();
        let link_principal = pending_funding.pending_debits[component_index];
        let component = pending_funding.components[component_index];
        let link_donation = link_account.lamports();
        let rent = read_rent(rent_sysvar)?;
        require(
            complete_root.account() == *self.root_account.key
                && complete_root.is_writable()
                && *system_program.key == SYSTEM_PROGRAM_ID
                && system_program.executable
                && series_admission_vault.key != self.root_account.key
                && series_admission_vault.key != link_account.key
                && series_admission_vault.key != funding_account.key
                && series_admission_vault.key != replay_account.key
                && link_principal.lamports
                    == rent.minimum_balance(SERIES_MARKET_LINK_ACCOUNT_BYTES_V2)?
                && link_principal.collateral_atoms == 0
                && component.remaining_principal.collateral_atoms == 0
                && component.donations.collateral_atoms == 0,
            ClutchError::MismatchedState,
        )?;
        require_unallocated_system_account(link_account)?;
        require_system_vault(series_admission_vault)?;
        let series_bytes = series.bytes();
        let (expected_vault, vault_bump) = seeds::series_lamport_vault_pda(
            self.program_id,
            &series_bytes,
            SERIES_ADMISSION_COMPONENT_SEED_V4,
        );
        let (expected_link, link_bump) = seeds::product_series_market_link_pda(
            self.program_id,
            &series_bytes,
            ordinal,
        );
        require(
            *series_admission_vault.key == expected_vault
                && *link_account.key == expected_link
                && *link_account.key == preauthorization.founder_link_account(),
            ClutchError::MismatchedState,
        )?;
        let expected_vault_principal = component
            .remaining_principal
            .lamports
            .checked_add(link_principal.lamports)
            .ok_or(ClutchError::Arithmetic)?;
        let expected_vault_accounted = expected_vault_principal
            .checked_add(component.donations.lamports)
            .ok_or(ClutchError::Arithmetic)?;
        let vault_before = series_admission_vault.lamports();
        let vault_after = vault_before
            .checked_sub(link_principal.lamports)
            .ok_or(ClutchError::Arithmetic)?;
        require(
            vault_before >= expected_vault_accounted,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        SeriesMarketLinkV2::initialize_pending_from_ref_into(
            link_binding,
            obligation_configuration,
            link_principal.lamports,
            link_donation,
            &mut link_initial_output.state,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            link_initial_output.state.semantic_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == expected_link_semantic_id,
            ClutchError::MismatchedState,
        )?;
        invoke_current_founder_transfer(
            series_admission_vault,
            link_account,
            system_program,
            link_principal.lamports,
            &[
                seeds::SEED_SERIES_LAMPORT_VAULT_V1,
                &series_bytes,
                &[SERIES_ADMISSION_COMPONENT_SEED_V4],
                &[vault_bump],
            ],
        )?;
        require(
            series_admission_vault.lamports() == vault_after
                && link_account.lamports()
                    == link_principal.lamports
                        .checked_add(link_donation).ok_or(ClutchError::Arithmetic)?,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        allocate_assign_current_founder_account(
            self.program_id,
            link_account,
            system_program,
            SERIES_MARKET_LINK_ACCOUNT_BYTES_V2,
            &[
                seeds::SEED_PRODUCT_SERIES_MARKET_LINK,
                &series_bytes,
                &ordinal.to_le_bytes(),
                &[link_bump],
            ],
        )?;
        {
            let mut data = link_account.try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            require(data.iter().all(|byte| *byte == 0), ClutchError::AlreadyInitialized)?;
            SeriesMarketLinkAccountV2::encode_parts(
                &link_initial_output.state,
                link_bump,
                &mut data,
            )?;
        }
        let authenticated_link = authenticate_series_market_link_v2(
            self.program_id,
            link_account,
            series,
            ordinal,
            market,
            generation,
            *self.root_account.key,
            true,
            link_initial_output,
        )?;
        require(
            authenticated_link.state().semantic_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == expected_link_semantic_id,
            ClutchError::MismatchedState,
        )?;

        let activation_parts = self.creation.into_product_activation_parts_v3(
            complete_root,
            self.schedule,
            self.graph,
        )?;
        let (
            foundation_complete_receipt_id,
            founder_creation_receipt_id,
            founder_preauthorization_id,
            expected_root_account,
            expected_root_authentication_id,
            expected_root_data_id,
            expected_root_semantic_id,
            reservation,
            source,
            direct_capitalization,
            expected_market_binding,
            expected_link_binding,
            expected_obligation_configuration,
            expected_founder_link_semantic_id,
            accepted_market_core_receipt_id,
            physical,
            market_family_capability_policy_id,
            market_family_capability_authentication_id,
        ) = activation_parts.into_components();
        require(
            expected_root_account == complete_root.account()
                && expected_root_authentication_id == complete_root.authentication_id()
                && expected_root_data_id == complete_root.data_id()
                && expected_root_semantic_id
                    == complete_root.state().semantic_id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                && expected_market_binding.as_ref() == complete_root.state().binding_ref()
                && expected_link_binding.as_ref() == authenticated_link.state().binding_ref()
                && expected_obligation_configuration == obligation_configuration
                && expected_founder_link_semantic_id == expected_link_semantic_id,
            ClutchError::MismatchedState,
        )?;
        let admission = SeriesMarketAdmissionProjectionV2::new_from_ref(
            expected_market_binding.as_ref(),
            authenticated_link.state(),
            1,
        ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        complete_root.state().admit_series_link_into(admission, root_admission_output)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let admitted_root = write_market_lifecycle_root_v2(
            self.program_id,
            self.root_account,
            complete_root,
            root_admission_output,
            root_admission_rebound,
        )?;
        require(
            admitted_root.state().transition_sequence()
                == u64::from(admitted_root.state().foundation().sequence)
                    .checked_add(1).ok_or(ClutchError::Arithmetic)?,
            ClutchError::MismatchedState,
        )?;
        let completion = activate_record_and_complete_current_series_v4(
            self.program_id,
            registry,
            bundle,
            artifacts,
            self.graph,
            founder_creation_receipt_id,
            founder_preauthorization_id,
            foundation_complete_receipt_id,
            accepted_market_core_receipt_id,
            reservation,
            source,
            direct_capitalization,
            physical,
            market_family_capability_policy_id,
            market_family_capability_authentication_id,
            self.root_account,
            admitted_root,
            link_account,
            authenticated_link,
            funding_account,
            replay_account,
            authenticated_replay,
            root_activation_output,
            root_activation_rebound,
            link_activation_output,
            link_activation_rebound,
        )?;
        Ok(completion)
    }
}

/// Final callable founder receipt after RootV2, LinkV2, replayV2, FundingV4,
/// and `0xba/v2` are all physically active. It retains the unique move-only
/// physical lineage, but no detachable foundation-step capability.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductMarketFounderActivatedV4 {
    id: ContentId,
    direct_activation: AuthenticatedProductDirectGlobalLivenessActivationV2,
    physical: AuthenticatedSeriesPhysicalFounderV4,
    facts: Box<ProductMarketFounderActivatedFactsV4>,
}

#[derive(Debug, Eq, PartialEq)]
struct ProductMarketFounderActivatedFactsV4 {
    activation_completion_id: ContentId,
    foundation_complete_receipt_id: ContentId,
    funding_completion_id: ContentId,
    source_postwrite_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV2Id,
    link_activation_receipt_id: ContentId,
    market_admission_receipt_id: ContentId,
    replay_account: Pubkey,
    replay_authentication_id: ContentId,
    replay_state_id: ContentId,
    replay_admission_projection_id: ContentId,
    funding_account: Pubkey,
    funding_state_id: SeriesFundingStateV4Id,
    funding_authentication_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    final_foundation_donation_lamports: u64,
    physical_founder_id: ContentId,
    physical_capitalization_id: ContentId,
    market_family_capability_policy_id: ContentId,
    market_family_capability_authentication_id: ContentId,
}

impl AuthenticatedProductMarketFounderActivatedV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn activation_completion_id(&self) -> ContentId {
        self.facts.activation_completion_id
    }
    pub(crate) const fn foundation_complete_receipt_id(&self) -> ContentId {
        self.facts.foundation_complete_receipt_id
    }
    pub(crate) const fn direct_activation(
        &self,
    ) -> &AuthenticatedProductDirectGlobalLivenessActivationV2 {
        &self.direct_activation
    }
    pub(crate) const fn funding_completion_id(&self) -> ContentId {
        self.facts.funding_completion_id
    }
    pub(crate) const fn source_postwrite_id(&self) -> ContentId {
        self.facts.source_postwrite_id
    }
    pub(crate) const fn root_transition_sequence_before(&self) -> u64 {
        self.facts.root_transition_sequence_before
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.facts.root_transition_sequence_after
    }
    pub(crate) const fn final_foundation_donation_lamports(&self) -> u64 {
        self.facts.final_foundation_donation_lamports
    }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.facts.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.facts.ordinal }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.facts.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.facts.generation }
    pub(crate) const fn root_account(&self) -> Pubkey { self.facts.root_account }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.facts.root_binding_id }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.facts.root_authentication_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId { self.facts.root_semantic_id }
    pub(crate) const fn link_account(&self) -> Pubkey { self.facts.link_account }
    pub(crate) const fn link_authentication_id(&self) -> ContentId {
        self.facts.link_authentication_id
    }
    pub(crate) const fn link_semantic_id(&self) -> SeriesMarketLinkV2Id {
        self.facts.link_semantic_id
    }
    pub(crate) const fn link_activation_receipt_id(&self) -> ContentId {
        self.facts.link_activation_receipt_id
    }
    pub(crate) const fn market_admission_receipt_id(&self) -> ContentId {
        self.facts.market_admission_receipt_id
    }
    pub(crate) const fn replay_account(&self) -> Pubkey { self.facts.replay_account }
    pub(crate) const fn replay_authentication_id(&self) -> ContentId {
        self.facts.replay_authentication_id
    }
    pub(crate) const fn replay_state_id(&self) -> ContentId { self.facts.replay_state_id }
    pub(crate) const fn replay_admission_projection_id(&self) -> ContentId {
        self.facts.replay_admission_projection_id
    }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.facts.funding_account }
    pub(crate) const fn funding_state_id(&self) -> SeriesFundingStateV4Id {
        self.facts.funding_state_id
    }
    pub(crate) const fn funding_authentication_id(&self) -> ContentId {
        self.facts.funding_authentication_id
    }
    pub(crate) const fn physical_founder_id(&self) -> ContentId {
        self.facts.physical_founder_id
    }
    pub(crate) const fn physical_capitalization_id(&self) -> ContentId {
        self.facts.physical_capitalization_id
    }
    pub(crate) const fn market_family_capability_policy_id(&self) -> ContentId {
        self.facts.market_family_capability_policy_id
    }
    pub(crate) const fn market_family_capability_authentication_id(&self) -> ContentId {
        self.facts.market_family_capability_authentication_id
    }
    pub(crate) const fn physical(&self) -> &AuthenticatedSeriesPhysicalFounderV4 {
        &self.physical
    }
}

/// Sole callable current Product founder outer. The closure can consume only
/// the cursor's typed one-shot slot method and can return only the private
/// completion minted by `finish`; returning an error rolls back every write.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn compose_current_product_market_founder_v4<'outer, 'info, 'root, F>(
    program_id: &'outer Pubkey,
    creation: AuthenticatedProductMarketFounderCurrentCreationV3,
    schedule: &'outer MarketFoundationScheduleV3,
    graph: &'outer MarketFoundationAccountGraphV3,
    root_account: &'outer AccountInfo<'info>,
    foundation_vault: &AccountInfo<'info>,
    direct_global_liveness_account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
    root_initial_output: &'root mut MarketLifecycleRootAccountV2,
    root_final_output: &mut MarketLifecycleRootAccountV2,
    compose_foundation: F,
) -> Outcome<AuthenticatedProductMarketFounderActivatedV4>
where
    F: FnOnce(
        CurrentProductMarketFoundationCursorV4<'outer, 'info>,
        AuthenticatedMarketLifecycleRootV2<'root>,
    ) -> Outcome<AuthenticatedProductSeriesActivationCompletionV4>,
{
    let expected_creation_id = creation.id();
    let expected_root_account = creation.preauthorization().lifecycle_root_account();
    let initial_root = initialize_current_founder_root_v4(
        program_id,
        &creation,
        root_account,
        foundation_vault,
        system_program,
        rent_sysvar,
        schedule,
        graph,
        root_initial_output,
    )?;
    let cursor = CurrentProductMarketFoundationCursorV4 {
        program_id,
        creation,
        root_account,
        schedule,
        graph,
        market_liability_plan_id: ContentId::ZERO,
        market_core_liability_plan: None,
        claim_mint_plan: None,
        outcome_custody_plan: None,
        hoard_slot_receipt_id: ContentId::ZERO,
        claim_ledger_slot_receipt_id: ContentId::ZERO,
        accepted_market_liability: None,
        accepted_claim_mints: [None; MARKET_FOUNDATION_MAX_OUTCOMES_V3],
        accepted_outcome_custodies: [None; MARKET_FOUNDATION_MAX_OUTCOMES_V3],
    };
    let completion = compose_foundation(cursor, initial_root)?;
    require(
        completion.founder_creation_receipt_id() == expected_creation_id,
        ClutchError::MismatchedState,
    )?;
    let activation_completion_id = completion.id();
    let foundation_complete_receipt_id = completion.foundation_complete_receipt_id();
    let funding_completion_id = completion.funding_completion().id();
    let source_postwrite_id = completion.source().id();
    let root_transition_sequence_before = completion.root_transition_sequence_before();
    let root_transition_sequence_after = completion.root_transition_sequence_after();
    let final_foundation_donation_lamports =
        completion.final_foundation_donation_lamports();
    let physical_founder_id = completion.physical.id();
    let physical_capitalization_id = completion.physical.capitalization_id();
    let market_family_capability_policy_id = completion.market_family_capability_policy_id;
    let market_family_capability_authentication_id =
        completion.market_family_capability_authentication_id;
    let facts = Box::new(ProductMarketFounderActivatedFactsV4 {
        activation_completion_id,
        foundation_complete_receipt_id,
        funding_completion_id,
        source_postwrite_id,
        series_plan_id: completion.series_plan_id,
        ordinal: completion.ordinal,
        market_instance_id: completion.market_instance_id,
        generation: completion.generation,
        root_account: completion.root_account,
        root_binding_id: completion.root_binding_id,
        root_authentication_id: completion.root_authentication_after,
        root_semantic_id: completion.root_semantic_after,
        link_account: completion.link_account,
        link_authentication_id: completion.link_authentication_after,
        link_semantic_id: completion.link_semantic_after,
        link_activation_receipt_id: completion.link_activation_receipt_id,
        market_admission_receipt_id: completion.market_admission_receipt_id,
        replay_account: completion.replay_account,
        replay_authentication_id: completion.replay_authentication_after,
        replay_state_id: completion.replay_state_after_id,
        replay_admission_projection_id: completion.replay_admission_projection_id,
        funding_account: completion.funding_completion().funding_account,
        funding_state_id: completion.funding_completion().funding_state_after_id(),
        funding_authentication_id:
            completion.funding_completion().funding_authentication_after_id(),
        root_transition_sequence_before,
        root_transition_sequence_after,
        final_foundation_donation_lamports,
        physical_founder_id,
        physical_capitalization_id,
        market_family_capability_policy_id,
        market_family_capability_authentication_id,
    });
    let final_root = authenticate_market_lifecycle_root_v2(
        program_id,
        root_account,
        completion.market_instance_id,
        completion.generation,
        true,
        root_final_output,
    )?;
    require(
        final_root.account() == expected_root_account
            && final_root.state().transition_sequence() == root_transition_sequence_after,
        ClutchError::MismatchedState,
    )?;
    let (direct_activation, physical) =
        activate_product_direct_global_liveness_from_current_founder_v2(
        program_id,
        completion,
        direct_global_liveness_account,
        final_root,
    )?;
    let id = hashv(&[
        PRODUCT_CURRENT_FOUNDER_ACTIVATED_DOMAIN_V4,
        program_id.as_ref(),
        &expected_creation_id.bytes(),
        root_account.key.as_ref(),
        &activation_completion_id.bytes(),
        &foundation_complete_receipt_id.bytes(),
        &funding_completion_id.bytes(),
        &source_postwrite_id.bytes(),
        &direct_activation.id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &root_transition_sequence_after.to_le_bytes(),
        &final_foundation_donation_lamports.to_le_bytes(),
        &physical_founder_id.bytes(),
        &physical_capitalization_id.bytes(),
        &market_family_capability_policy_id.bytes(),
        &market_family_capability_authentication_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductMarketFounderActivatedV4 {
        id,
        direct_activation,
        physical,
        facts,
    })
}

#[allow(clippy::too_many_arguments)]
fn initialize_current_founder_root_v4<'next>(
    program_id: &Pubkey,
    creation: &AuthenticatedProductMarketFounderCurrentCreationV3,
    root_account: &AccountInfo<'_>,
    foundation_vault: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
    output: &'next mut MarketLifecycleRootAccountV2,
) -> Outcome<AuthenticatedMarketLifecycleRootV2<'next>> {
    let preauthorization = creation.preauthorization();
    let market = preauthorization.market_instance_id();
    let generation = preauthorization.generation();
    let market_bytes = market.bytes();
    let (expected_root, root_bump) = seeds::product_market_lifecycle_root_pda(
        program_id,
        &market_bytes,
        generation,
    );
    let (expected_vault, vault_bump) = seeds::product_market_foundation_vault_pda(
        program_id,
        &market_bytes,
        generation,
    );
    let root_principal = schedule.slot_principal_lamports[
        MarketFoundationSlotV3::LifecycleRoot
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
    ];
    let rent = read_rent(rent_sysvar)?;
    require_unallocated_system_account(root_account)?;
    require_system_vault(foundation_vault)?;
    require(
        *system_program.key == SYSTEM_PROGRAM_ID
            && system_program.executable
            && *root_account.key == expected_root
            && *root_account.key == preauthorization.lifecycle_root_account()
            && *foundation_vault.key == expected_vault
            && *foundation_vault.key == preauthorization.foundation_vault_account()
            && graph.account(MarketFoundationSlotV3::LifecycleRoot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes() == root_account.key.to_bytes()
            && root_principal
                == rent.minimum_balance(MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2)?,
        ClutchError::MismatchedState,
    )?;
    let mut capital = *creation.foundation_capital();
    let vault_before = foundation_vault.lamports();
    let current_donation = vault_before
        .checked_sub(capital.principal_total_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        capital.principal_remaining_lamports == capital.principal_total_lamports
            && current_donation >= capital.vault_current_donation_lamports,
        ClutchError::MismatchedState,
    )?;
    capital.vault_current_donation_lamports = current_donation;
    let root_donation = root_account.lamports();
    let vault_after = vault_before
        .checked_sub(root_principal)
        .ok_or(ClutchError::Arithmetic)?;
    let root_after = root_donation
        .checked_add(root_principal)
        .ok_or(ClutchError::Arithmetic)?;
    let root_postwrite_receipt_id = hashv(&[
        PRODUCT_CURRENT_ROOT_SLOT_POSTWRITE_DOMAIN_V4,
        program_id.as_ref(),
        &creation.id().bytes(),
        &preauthorization.id().bytes(),
        root_account.key.as_ref(),
        foundation_vault.key.as_ref(),
        &root_principal.to_le_bytes(),
        &root_donation.to_le_bytes(),
        &root_after.to_le_bytes(),
        &vault_before.to_le_bytes(),
        &vault_after.to_le_bytes(),
        &current_donation.to_le_bytes(),
    ]);
    require_live(root_postwrite_receipt_id)?;
    MarketLifecycleRootV2::initialize_founder_from_ref_into(
        creation.market_binding(),
        schedule,
        graph,
        capital,
        creation.product_families(),
        root_postwrite_receipt_id,
        &mut output.state,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    invoke_current_founder_transfer(
        foundation_vault,
        root_account,
        system_program,
        root_principal,
        &[
            seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
            &market_bytes,
            &generation.to_le_bytes(),
            &[vault_bump],
        ],
    )?;
    require(
        foundation_vault.lamports() == vault_after
            && root_account.lamports() == root_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    allocate_assign_current_founder_account(
        program_id,
        root_account,
        system_program,
        MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2,
        &[
            seeds::SEED_PRODUCT_MARKET_LIFECYCLE_ROOT,
            &market_bytes,
            &generation.to_le_bytes(),
            &[root_bump],
        ],
    )?;
    {
        let mut data = root_account.try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(data.iter().all(|byte| *byte == 0), ClutchError::AlreadyInitialized)?;
        MarketLifecycleRootAccountV2::encode_parts(
            &output.state,
            root_principal,
            root_bump,
            &mut data,
        )?;
    }
    let authenticated = authenticate_market_lifecycle_root_v2(
        program_id,
        root_account,
        market,
        generation,
        true,
        output,
    )?;
    require(
        authenticated.observed_lamports() == root_after
            && authenticated.value().rent_principal_lamports == root_principal
            && authenticated.state().capital().vault_current_donation_lamports
                == current_donation,
        ClutchError::MismatchedState,
    )?;
    Ok(authenticated)
}

fn require_unallocated_system_account(account: &AccountInfo<'_>) -> Outcome<()> {
    require(
        account.is_writable
            && !account.is_signer
            && !account.executable
            && *account.owner == SYSTEM_PROGRAM_ID
            && account.data_len() == 0,
        ClutchError::MismatchedState,
    )
}

fn require_system_vault(account: &AccountInfo<'_>) -> Outcome<()> {
    require_unallocated_system_account(account)
}

fn invoke_current_founder_transfer<'info>(
    source: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(lamports),
        vec![
            AccountMeta::new(*source.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[source.clone(), destination.clone(), system_program.clone()],
        &[signer_seeds],
    ).map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))
}

fn allocate_assign_current_founder_account<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    account_bytes: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_bytes),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    ).map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &assign,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    ).map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))
}

/// Private current founder tail. The sole outer must obtain every move-only
/// argument by consuming the exhaustive foundation-complete authority; none
/// of these facts is a caller-facing DTO or an alternate raw writer.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn activate_record_and_complete_current_series_v4(
    program_id: &Pubkey,
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    graph: &MarketFoundationAccountGraphV3,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_complete_receipt_id: ContentId,
    accepted_market_core_receipt_id: ContentId,
    reservation: AuthenticatedProductSeriesFundingReservationV4,
    source: AuthenticatedPreRootSourceOccurrencePostwriteV3,
    direct_capitalization: AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
    physical: AuthenticatedSeriesPhysicalFounderV4,
    market_family_capability_policy_id: ContentId,
    market_family_capability_authentication_id: ContentId,
    root_account: &AccountInfo<'_>,
    authenticated_root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    authenticated_link: AuthenticatedSeriesMarketLinkV2<'_>,
    funding_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    authenticated_replay: AuthenticatedSeriesLifecycleReplayV2,
    root_successor_output: &mut MarketLifecycleRootV2,
    root_rebound_output: &mut MarketLifecycleRootAccountV2,
    link_successor_output: &mut SeriesMarketLinkV2,
    link_rebound_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedProductSeriesActivationCompletionV4> {
    for id in [
        founder_creation_receipt_id,
        founder_preauthorization_id,
        foundation_complete_receipt_id,
        accepted_market_core_receipt_id,
        source.id(),
        source.capitalization().id(),
        direct_capitalization.global_capitalization_receipt_id(),
        direct_capitalization.global_bundle_binding_id(),
        physical.id(),
        physical.capitalization_id(),
        market_family_capability_policy_id,
        market_family_capability_authentication_id,
    ] {
        require_live(id)?;
    }
    let series = artifacts.series();
    let quote = artifacts.quote();
    let attachment = artifacts.attachment();
    let series_id = series.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let quote_id = quote.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = attachment.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = quote.foundation.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph.validate(&quote.foundation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph.id(&quote.foundation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root = authenticated_root.state();
    let root_binding = root.binding_ref();
    let root_binding_id = root_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link = authenticated_link.state();
    let link_binding = link.binding_ref();
    let link_semantic_before = link.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay = authenticated_replay.state();
    let replay_binding: SeriesLifecycleReplayBindingV2 = replay.binding();
    let replay_binding_id = replay_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let reservation_binding = reservation.binding();
    let reservation_binding_id = reservation_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_pending_state_id = reservation.funding_state_pending_id()?;
    let funding = reservation.pending();
    let source_occurrence = source.occurrence();
    let source_capitalization_id = source.capitalization().id();
    let market_admission = SeriesMarketAdmissionProjectionV2::new_from_ref(root_binding, link, 1)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_admission_receipt_id = market_admission.id();
    let root_transition_sequence_before = root.transition_sequence();
    let final_foundation_donation_lamports = root.capital().vault_current_donation_lamports;
    let expected_root_link_transcript = hashv(&[
        b"dragons-clutch/market-series-link-transcript/v2",
        &ContentId::ZERO.bytes(),
        &market_admission_receipt_id.bytes(),
        &root_transition_sequence_before.to_le_bytes(),
    ]);
    require(
        authenticated_root.is_writable()
            && authenticated_link.is_writable()
            && funding.is_writable()
            && authenticated_replay.is_writable()
            && root_account.is_writable
            && link_account.is_writable
            && funding_account.is_writable
            && replay_account.is_writable
            && root_account.key != link_account.key
            && root_account.key != funding_account.key
            && root_account.key != replay_account.key
            && link_account.key != funding_account.key
            && link_account.key != replay_account.key
            && funding_account.key != replay_account.key
            && authenticated_root.account() == *root_account.key
            && authenticated_link.account() == *link_account.key
            && funding.account() == *funding_account.key
            && authenticated_replay.account() == *replay_account.key
            && root.phase() == MarketLifecyclePhaseV2::Founding
            && root.foundation().complete()
            && root.capital().principal_remaining_lamports == 0
            && root.admitted_series_links() == 1
            && root.live_series_links() == 1
            && root.retired_series_links() == 0
            && root_transition_sequence_before
                == u64::from(root.foundation().sequence)
                    .checked_add(1).ok_or(ClutchError::Arithmetic)?
            && root.series_link_transcript_id() == expected_root_link_transcript
            && link.phase() == SeriesMarketLinkPhaseV2::PendingMarket
            && replay.phase() == SeriesLifecycleReplayPhaseV2::Open
            && root_binding.foundation_schedule_id == schedule_id
            && root_binding.foundation_account_graph_id == graph_id
            && root_binding.direct_global_liveness_binding_id
                == direct_capitalization.global_bundle_binding_id()
            && graph.market_instance_id == root_binding.market_instance_id
            && graph.generation == root_binding.generation
            && link_binding.market_root_account_id.bytes() == root_account.key.to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.series_plan_id == series_id
            && link_binding.funding_terms_id == registry.funding_terms_id()
            && link_binding.funding_quote_id == quote_id
            && link_binding.attachment_plan_id == attachment_id
            && link_binding.compiler_bundle_id == bundle.bundle_id()
            && link_binding.capability_profile_id == registry.capability_profile_id()
            && link_binding.source_occurrence_id.content_id()
                == source_occurrence.occurrence_record_id()
            && link_binding.source_occurrence_account_id.bytes()
                == source_occurrence.occurrence_account().bytes()
            && link_binding.source_occurrence_account_authentication_id
                == source_occurrence.occurrence_account_authentication_id()
            && link_binding.source_occurrence_receipt_id == source.id()
            && link_binding.funding_state_account_id.bytes() == funding_account.key.to_bytes()
            && link_binding.funding_debit_receipt_id
                == reservation.reservation_receipt_id()
            && link_binding.funding_transition_sequence == funding.state().transition_sequence
            && source.product_preauthorization_id() == founder_preauthorization_id
            && source.capitalization().product_preauthorization_id()
                == founder_preauthorization_id
            && source.capitalization().facts().funding_reservation_postwrite_id
                == reservation.id()
            && source.capitalization().facts().pending_pre_source_reservation_binding_id
                == reservation_binding_id.content_id()
            && reservation_binding.market_binding_id == root_binding_id
            && reservation_binding.market_root_account_id.bytes()
                == root_account.key.to_bytes()
            && reservation_binding.series_market_link_account_id.bytes()
                == link_account.key.to_bytes()
            && reservation_binding.source_occurrence_id == link_binding.source_occurrence_id
            && reservation_binding.product_founder_preauthorization_id
                == founder_preauthorization_id
            && reservation_binding.direct_global_liveness_capitalization_id
                == direct_capitalization.global_capitalization_receipt_id()
            && registry.series_plan_id() == series_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && bundle.bundle().series_plan_id == series_id
            && bundle.bundle().funding_terms_id == registry.funding_terms_id()
            && bundle.bundle().funding_quote_id == quote_id
            && bundle.bundle().attachment_plan_id == attachment_id
            && replay_binding.series_plan_id == series_id
            && replay_binding.funding_terms_id == registry.funding_terms_id()
            && replay_binding.funding_quote_id == quote_id
            && replay_binding.attachment_plan_id == attachment_id
            && replay_binding.compiler_bundle_id == bundle.bundle_id()
            && replay_binding.registry_release_id.content_id()
                == registry.registry_release_id()
            && replay_binding.capability_profile_id.content_id()
                == registry.capability_profile_id()
            && replay_binding.registry_account_id.bytes()
                == registry.series_registry_account().to_bytes()
            && replay_binding.funding_account_id.bytes() == funding_account.key.to_bytes()
            && replay_binding.lifecycle_replay_account_id.bytes()
                == replay_account.key.to_bytes()
            && replay_binding.instance_count == series.instance_count,
        ClutchError::MismatchedState,
    )?;

    let root_semantic_before = root.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_authentication_before = authenticated_root.authentication_id();
    root.activate_into(
        &quote.foundation,
        accepted_market_core_receipt_id,
        root_successor_output,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound_root = write_market_lifecycle_root_v2(
        program_id,
        root_account,
        authenticated_root,
        root_successor_output,
        root_rebound_output,
    )?;
    let root_semantic_after = rebound_root.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_transition_sequence_after = rebound_root.state().transition_sequence();
    require(
        root_transition_sequence_after
            == root_transition_sequence_before
                .checked_add(1).ok_or(ClutchError::Arithmetic)?
            && rebound_root.state().capital().vault_current_donation_lamports
                == final_foundation_donation_lamports,
        ClutchError::MismatchedState,
    )?;

    link.activate_into(1, market_admission_receipt_id, link_successor_output)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_authentication_before = authenticated_link.authentication_id();
    let rebound_link = write_series_market_link_v2(
        program_id,
        link_account,
        authenticated_link,
        link_successor_output,
        link_rebound_output,
    )?;
    let link_semantic_after = rebound_link.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let disposition_byte = match link_binding.disposition {
        SeriesMarketDispositionV1::Founder => 1,
        SeriesMarketDispositionV1::Converger => 2,
    };
    let link_activation_receipt_id = hashv(&[
        PRODUCT_CURRENT_LINK_ACTIVATION_DOMAIN_V4,
        program_id.as_ref(),
        root_account.key.as_ref(),
        &root_authentication_before.bytes(),
        &rebound_root.authentication_id().bytes(),
        &root_semantic_before.bytes(),
        &root_semantic_after.bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &root_transition_sequence_after.to_le_bytes(),
        &final_foundation_donation_lamports.to_le_bytes(),
        link_account.key.as_ref(),
        &link_authentication_before.bytes(),
        &rebound_link.authentication_id().bytes(),
        &link_semantic_before.bytes(),
        &link_semantic_after.bytes(),
        &market_admission_receipt_id.bytes(),
        &accepted_market_core_receipt_id.bytes(),
        &[disposition_byte],
    ]);
    require_live(link_activation_receipt_id)?;

    let authorization_facts = Box::new(SeriesFundingCompletionAuthorizationV4 {
        reservation_binding_id,
        funding_account_id: ContentId::from_bytes(funding_account.key.to_bytes()),
        funding_account_authentication_pending_id:
            reservation.funding_authentication_pending_id(),
        funding_pending_state_id,
        source_capitalization_receipt_id: source_capitalization_id,
        pre_root_source_occurrence_id: source.id(),
        market_root_account_id: ContentId::from_bytes(root_account.key.to_bytes()),
        market_binding_id: root_binding_id,
        root_semantic_before_id: root_semantic_before,
        root_semantic_after_id: root_semantic_after,
        series_market_link_account_id: ContentId::from_bytes(link_account.key.to_bytes()),
        link_semantic_before_id: link_semantic_before.content_id(),
        link_semantic_after_id: link_semantic_after.content_id(),
        market_admission_receipt_id,
        link_activation_receipt_id,
        accepted_market_core_receipt_id,
        funding_projected_state_after_id: reservation.pending().state()
            .project_pending_completion_poststate(series, quote, attachment)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    });
    let authorization = authorize_series_funding_completion_v4(
        reservation,
        series,
        quote,
        attachment,
        authorization_facts,
    )?;
    let completion_authorization_id = authorization.id();
    let projected_funding_state_after_id = authorization
        .projected_state_after
        .id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_state_before_id = replay.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.content_id();
    let replay_authentication_before = authenticated_replay.authentication_id();
    let admission = SeriesLifecycleAdmissionProjectionV2 {
        binding_id: replay_binding_id,
        series_plan_id: series_id,
        ordinal: link_binding.ordinal,
        funding_account_id: ContentId::from_bytes(funding_account.key.to_bytes()),
        funding_state_before_id: funding_pending_state_id.content_id(),
        funding_state_after_id: projected_funding_state_after_id.content_id(),
        occurrence_completion_receipt_id: completion_authorization_id.content_id(),
        link_account_id: ContentId::from_bytes(link_account.key.to_bytes()),
        link_activation_receipt_id,
        market_admission_receipt_id,
        market_instance_id: link_binding.market_instance_id,
        source_occurrence_id: link_binding.source_occurrence_id,
        compiler_bundle_id: bundle.bundle_id(),
        disposition: link_binding.disposition,
        generation: link_binding.generation,
    };
    let replay_admission_projection_id = admission.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_successor = replay.record_admission(admission)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound_replay = write_series_lifecycle_replay_v2(
        program_id,
        replay_account,
        authenticated_replay,
        replay_successor,
    )?;
    let replay_state_after_id = rebound_replay.state().id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.content_id();
    let completion_binding = SeriesFundingCompletionBindingV4 {
        completion_authorization_id,
        lifecycle_replay_account_id: ContentId::from_bytes(replay_account.key.to_bytes()),
        lifecycle_replay_state_before_id: replay_state_before_id,
        lifecycle_replay_state_after_id: replay_state_after_id,
        lifecycle_replay_authentication_before_id: replay_authentication_before,
        lifecycle_replay_authentication_after_id: rebound_replay.authentication_id(),
        lifecycle_replay_admission_projection_id: replay_admission_projection_id,
    };
    let completion_binding_id = completion_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_completion = complete_series_funding_v4_with_binding(
        program_id,
        funding_account,
        authorization,
        series,
        quote,
        attachment,
        completion_binding,
    )?;
    require(
        funding_completion.completion_authorization_id() == completion_authorization_id
            && funding_completion.projected_state_after_id()
                == projected_funding_state_after_id
            && funding_completion.completion_binding_id() == completion_binding_id
            && funding_completion.completed_ordinal() == link_binding.ordinal,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_CURRENT_ACTIVATION_COMPLETION_DOMAIN_V4,
        program_id.as_ref(),
        &founder_creation_receipt_id.bytes(),
        &founder_preauthorization_id.bytes(),
        &foundation_complete_receipt_id.bytes(),
        root_account.key.as_ref(),
        &root_binding_id.bytes(),
        &root_authentication_before.bytes(),
        &rebound_root.authentication_id().bytes(),
        &root_semantic_before.bytes(),
        &root_semantic_after.bytes(),
        link_account.key.as_ref(),
        &link_authentication_before.bytes(),
        &rebound_link.authentication_id().bytes(),
        &link_semantic_before.bytes(),
        &link_semantic_after.bytes(),
        &link_activation_receipt_id.bytes(),
        &market_admission_receipt_id.bytes(),
        replay_account.key.as_ref(),
        &replay_authentication_before.bytes(),
        &rebound_replay.authentication_id().bytes(),
        &replay_state_before_id.bytes(),
        &replay_state_after_id.bytes(),
        &replay_admission_projection_id.bytes(),
        &completion_authorization_id.bytes(),
        &completion_binding_id.bytes(),
        &funding_completion.id().bytes(),
        &source.id().bytes(),
        &direct_capitalization.global_capitalization_receipt_id().bytes(),
        &physical.id().bytes(),
        &physical.capitalization_id().bytes(),
        &market_family_capability_policy_id.bytes(),
        &market_family_capability_authentication_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesActivationCompletionV4 {
        id,
        founder_creation_receipt_id,
        founder_preauthorization_id,
        foundation_complete_receipt_id,
        series_plan_id: series_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        root_account: rebound_root.account(),
        root_binding_id,
        root_authentication_after: rebound_root.authentication_id(),
        root_semantic_after,
        root_transition_sequence_before,
        root_transition_sequence_after,
        final_foundation_donation_lamports,
        link_account: rebound_link.account(),
        link_authentication_after: rebound_link.authentication_id(),
        link_semantic_after,
        link_activation_receipt_id,
        market_admission_receipt_id,
        replay_account: rebound_replay.account(),
        replay_authentication_after: rebound_replay.authentication_id(),
        replay_state_after_id,
        replay_admission_projection_id,
        funding_completion: Box::new(funding_completion),
        source: Box::new(source),
        direct_capitalization,
        physical,
        market_family_capability_policy_id,
        market_family_capability_authentication_id,
    })
}

/// Promote one exact unresolved RootV2/LinkV2 pair into an exclusive Failure pin.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pin_series_market_link_failure_v2<'next, A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    authenticated_root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    authenticated_link: AuthenticatedSeriesMarketLinkV2<'_>,
    begin_admission_receipt_id: ContentId,
    authority: &A,
    root_rebound_output: &mut MarketLifecycleRootAccountV2,
    link_rebound_output: &'next mut SeriesMarketLinkAccountV2,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV2<'next>,
    AuthenticatedSeriesFailureSessionPinV2,
)>
where
    A: AuthenticatedSeriesFailureSessionBeginV3 + ?Sized,
{
    require_live(begin_admission_receipt_id)?;
    let root_binding = authenticated_root.state().binding();
    let live_root = authenticate_market_lifecycle_root_v2(
        program_id, root_account, root_binding.market_instance_id,
        root_binding.generation, false, root_rebound_output)?;
    let live_root_binding = live_root.state().binding();
    let root_binding_id = live_root_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_binding = authenticated_link.state().binding();
    require_unresolved_market_resolution_v2(live_root.state())?;
    require(!authenticated_root.is_writable()
        && live_root.account() == authenticated_root.account()
        && live_root.owner_program() == authenticated_root.owner_program()
        && live_root.value() == authenticated_root.value()
        && live_root.observed_lamports() == authenticated_root.observed_lamports()
        && live_root.data_id() == authenticated_root.data_id()
        && live_root.authentication_id() == authenticated_root.authentication_id()
        && root_account.key != link_account.key
        && live_root.state().phase() == MarketLifecyclePhaseV2::Active
        && authenticated_link.is_writable()
        && authenticated_link.state().phase() == SeriesMarketLinkPhaseV2::Active
        && authenticated_link.state().active_failure_sessions() == 0
        && link_binding.market_root_account_id.bytes() == live_root.account().to_bytes()
        && link_binding.market_binding_id == root_binding_id
        && link_binding.market_instance_id == live_root_binding.market_instance_id
        && link_binding.generation == live_root_binding.generation,
        ClutchError::MismatchedState)?;
    authority.authenticate_series_failure_session_begin_v3(
        live_root.account(), live_root.authentication_id(), authenticated_link.account(),
        authenticated_link.authentication_id(), link_binding.series_plan_id,
        link_binding.ordinal, link_binding.market_instance_id, link_binding.generation,
        link_binding.source_occurrence_id, begin_admission_receipt_id)?;
    let semantic_before = authenticated_link.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = authenticated_link.authentication_id();
    let successor = authenticated_link.state().pin_failure_session(begin_admission_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_series_market_link_v2(
        program_id, link_account, authenticated_link, &successor, link_rebound_output)?;
    let semantic_after = rebound.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let session_binding_id = rebound.state().failure_session_transcript_id();
    require(rebound.state().active_failure_sessions() == 1
        && rebound.state().failure_sessions_started()
            == authenticated_link.state().failure_sessions_started()
                .checked_add(1).ok_or(ClutchError::Arithmetic)?
        && session_binding_id != ContentId::ZERO
        && session_binding_id != authenticated_link.state().failure_session_transcript_id(),
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        SERIES_FAILURE_BEGIN_AUTHENTICATION_DOMAIN_V2, program_id.as_ref(),
        live_root.account().as_ref(), &live_root.authentication_id().bytes(),
        rebound.account().as_ref(), &authentication_before.bytes(),
        &rebound.authentication_id().bytes(), &semantic_before.bytes(),
        &semantic_after.bytes(), &begin_admission_receipt_id.bytes(),
        &session_binding_id.bytes(), &link_binding.series_plan_id.bytes(),
        &link_binding.ordinal.to_le_bytes(), &link_binding.market_instance_id.bytes(),
        &link_binding.generation.to_le_bytes(), &link_binding.source_occurrence_id.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedSeriesFailureSessionPinV2 {
        id, root_account: live_root.account(), root_authentication_id: live_root.authentication_id(),
        link_account: rebound.account(), link_authentication_before: authentication_before,
        link_authentication_after: rebound.authentication_id(), link_semantic_before: semantic_before,
        link_semantic_after: semantic_after, begin_admission_receipt_id, session_binding_id,
    }))
}

/// Hostile-authenticate the writable RootV2/link prestate used by Resolution.
pub(crate) fn authenticate_writable_failure_resolution_link_v3(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV2,
    link_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV3> {
    authenticate_writable_failure_session_release_link_v3(
        program_id, root_account, root, link_account,
        FailureSessionReleaseDispositionV3::Resolved, root_output, link_output)
}

/// Hostile-authenticate an unresolved exhausted-session RootV2/link prestate.
pub(crate) fn authenticate_writable_failure_exhausted_link_v3(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV2,
    link_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV3> {
    authenticate_writable_failure_session_release_link_v3(
        program_id, root_account, root, link_account,
        FailureSessionReleaseDispositionV3::Exhausted, root_output, link_output)
}

/// Hostile-authenticate an unresolved Source-absence RootV2/link prestate.
pub(crate) fn authenticate_writable_failure_source_absent_link_v3(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV2,
    link_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV3> {
    authenticate_writable_failure_session_release_link_v3(
        program_id, root_account, root, link_account,
        FailureSessionReleaseDispositionV3::SourceAbsent, root_output, link_output)
}

/// Hostile-authenticate an unresolved Source-refusal RootV2/link prestate.
pub(crate) fn authenticate_writable_failure_source_refused_link_v3(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV2,
    link_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV3> {
    authenticate_writable_failure_session_release_link_v3(
        program_id, root_account, root, link_account,
        FailureSessionReleaseDispositionV3::SourceRefused, root_output, link_output)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_writable_failure_session_release_link_v3(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    cached_root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    disposition: FailureSessionReleaseDispositionV3,
    root_output: &mut MarketLifecycleRootAccountV2,
    link_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV3> {
    let cached_binding = cached_root.state().binding();
    let root_requires_writable = disposition.requires_writable_root();
    let live_root = authenticate_market_lifecycle_root_v2(
        program_id, root_account, cached_binding.market_instance_id,
        cached_binding.generation, root_requires_writable, root_output)?;
    let root_binding = live_root.state().binding();
    let root_binding_id = root_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_semantic_id = live_root.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_unresolved_market_resolution_v2(live_root.state())?;
    require(live_root.is_writable() == root_requires_writable
        && cached_root.is_writable() == root_requires_writable
        && live_root.account() == cached_root.account()
        && live_root.owner_program() == cached_root.owner_program()
        && live_root.value() == cached_root.value()
        && live_root.observed_lamports() == cached_root.observed_lamports()
        && live_root.data_id() == cached_root.data_id()
        && live_root.authentication_id() == cached_root.authentication_id()
        && live_root.state().phase() == MarketLifecyclePhaseV2::Active
        && root_account.key != link_account.key,
        ClutchError::MismatchedState)?;
    let data = link_account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV2::decode_into(&data, link_output)?;
    let decoded_binding = link_output.state.binding();
    drop(data);
    let link = authenticate_series_market_link_v2(
        program_id, link_account, decoded_binding.series_plan_id, decoded_binding.ordinal,
        root_binding.market_instance_id, root_binding.generation, live_root.account(), true,
        link_output)?;
    let state = link.state();
    let binding = state.binding();
    let semantic_id = state.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let transcript = state.failure_session_transcript_id();
    require(link.is_writable() && state.phase() == SeriesMarketLinkPhaseV2::Active
        && state.active_failure_sessions() == 1 && state.failure_sessions_started() != 0
        && transcript != ContentId::ZERO
        && binding.market_root_account_id.bytes() == live_root.account().to_bytes()
        && binding.market_binding_id == root_binding_id
        && binding.market_instance_id == root_binding.market_instance_id
        && binding.generation == root_binding.generation,
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        SERIES_FAILURE_RELEASE_PREAUTHENTICATION_DOMAIN_V3,
        &[disposition.wire_byte()], program_id.as_ref(), live_root.account().as_ref(),
        live_root.owner_program().as_ref(), &live_root.observed_lamports().to_le_bytes(),
        &live_root.data_id().bytes(), &live_root.authentication_id().bytes(),
        &root_semantic_id.bytes(), link.account().as_ref(), link.owner_program().as_ref(),
        &link.observed_lamports().to_le_bytes(), &link.data_id().bytes(),
        &link.authentication_id().bytes(), &semantic_id.bytes(), &root_binding_id.bytes(),
        &binding.series_plan_id.bytes(), &binding.ordinal.to_le_bytes(),
        &binding.market_instance_id.bytes(), &binding.generation.to_le_bytes(),
        &binding.source_occurrence_id.bytes(), &state.transition_sequence().to_le_bytes(),
        &state.failure_sessions_started().to_le_bytes(), &transcript.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedWritableFailureSessionReleaseLinkV3 {
        id, disposition, root_account: live_root.account(),
        root_owner_program: live_root.owner_program(),
        root_observed_lamports: live_root.observed_lamports(), root_data_id: live_root.data_id(),
        root_authentication_id: live_root.authentication_id(), root_semantic_id,
        link_account: link.account(), link_owner_program: link.owner_program(),
        link_observed_lamports: link.observed_lamports(), link_data_id: link.data_id(),
        link_authentication_id: link.authentication_id(), link_semantic_id: semantic_id,
        market_binding_id: root_binding_id, series_plan_id: binding.series_plan_id,
        ordinal: binding.ordinal, market_instance_id: binding.market_instance_id,
        generation: binding.generation, source_occurrence_id: binding.source_occurrence_id,
        transition_sequence: state.transition_sequence(),
        failure_sessions_started: state.failure_sessions_started(),
        failure_session_transcript_id: transcript,
    })
}

/// Release one exact Failure pin only after a same-disposition archive/reset.
pub(crate) fn release_series_market_link_failure_v3<'next, A>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV2<'_>,
    release_link: &AuthenticatedWritableFailureSessionReleaseLinkV3,
    archive: &A,
    rebound_output: &'next mut SeriesMarketLinkAccountV2,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV2<'next>,
    AuthenticatedSeriesFailureSessionReleaseV3,
)>
where
    A: AuthenticatedSeriesFailureArchivePostwriteV3 + ?Sized,
{
    let binding = authenticated.state().binding();
    let semantic_before = authenticated.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_before = authenticated.authentication_id();
    let transition_sequence_before = authenticated.state().transition_sequence();
    let transcript_before = authenticated.state().failure_session_transcript_id();
    let sessions_started_before = authenticated.state().failure_sessions_started();
    let archive_postwrite_id = archive.archive_postwrite_id()?;
    let append_receipt_id = archive.append_receipt_id()?;
    let reset_receipt_id = archive.reset_receipt_id()?;
    let market_instance_id = archive.market_instance_id()?;
    let generation = archive.generation()?;
    let source_occurrence_id = archive.source_occurrence_id()?;
    let session_binding_id = archive.session_binding_id()?;
    let session_terminal_receipt_id = archive.session_terminal_receipt_id()?;
    let disposition = archive.release_disposition()?;
    let preauthorization_id = archive.release_link_preauthorization_id()?;
    for id in [archive_postwrite_id, append_receipt_id, reset_receipt_id,
        session_binding_id, session_terminal_receipt_id, preauthorization_id] {
        require_live(id)?;
    }
    require(authenticated.is_writable()
        && authenticated.state().phase() == SeriesMarketLinkPhaseV2::Active
        && authenticated.state().active_failure_sessions() == 1
        && disposition == release_link.disposition
        && preauthorization_id == release_link.id
        && release_link.link_account == *account.key
        && release_link.link_owner_program == *program_id
        && release_link.link_observed_lamports == authenticated.observed_lamports()
        && release_link.link_data_id == authenticated.data_id()
        && release_link.link_authentication_id == authentication_before
        && release_link.link_semantic_id == semantic_before
        && release_link.root_account
            == Pubkey::new_from_array(binding.market_root_account_id.bytes())
        && release_link.market_binding_id == binding.market_binding_id
        && release_link.series_plan_id == binding.series_plan_id
        && release_link.ordinal == binding.ordinal
        && release_link.market_instance_id == binding.market_instance_id
        && release_link.generation == binding.generation
        && release_link.source_occurrence_id == binding.source_occurrence_id
        && release_link.transition_sequence == transition_sequence_before
        && release_link.failure_sessions_started == sessions_started_before
        && release_link.failure_session_transcript_id == transcript_before
        && session_binding_id == transcript_before
        && market_instance_id == binding.market_instance_id && generation == binding.generation
        && source_occurrence_id == binding.source_occurrence_id
        && archive_postwrite_id != append_receipt_id
        && archive_postwrite_id != reset_receipt_id
        && append_receipt_id != reset_receipt_id
        && session_terminal_receipt_id != session_binding_id,
        ClutchError::MismatchedState)?;
    archive.authenticate_series_failure_archive_release_postwrite_v3(
        archive_postwrite_id, append_receipt_id, reset_receipt_id,
        market_instance_id, generation, source_occurrence_id, session_binding_id,
        session_terminal_receipt_id, disposition, preauthorization_id)?;
    let successor = authenticated.state().release_failure_session(session_terminal_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_series_market_link_v2(
        program_id, account, authenticated, &successor, rebound_output)?;
    let semantic_after = rebound.state().semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let sequence_after = rebound.state().transition_sequence();
    let transcript_after = rebound.state().failure_session_transcript_id();
    require(rebound.state().phase() == SeriesMarketLinkPhaseV2::Active
        && rebound.state().active_failure_sessions() == 0
        && rebound.state().failure_sessions_started() == sessions_started_before
        && sequence_after == transition_sequence_before.checked_add(1)
            .ok_or(ClutchError::Arithmetic)?
        && transcript_after != transcript_before,
        ClutchError::MismatchedState)?;
    let id = hashv(&[
        SERIES_FAILURE_RELEASE_AUTHENTICATION_DOMAIN_V3, &[disposition.wire_byte()],
        account.key.as_ref(), &authentication_before.bytes(),
        &rebound.authentication_id().bytes(), &semantic_before.bytes(),
        &semantic_after.bytes(), &transition_sequence_before.to_le_bytes(),
        &sequence_after.to_le_bytes(), &transcript_before.bytes(), &transcript_after.bytes(),
        &session_terminal_receipt_id.bytes(), &archive_postwrite_id.bytes(),
        &append_receipt_id.bytes(), &reset_receipt_id.bytes(), &preauthorization_id.bytes(),
        &binding.series_plan_id.bytes(), &binding.ordinal.to_le_bytes(),
        &binding.market_instance_id.bytes(), &binding.generation.to_le_bytes(),
        &binding.source_occurrence_id.bytes(),
    ]);
    require_live(id)?;
    Ok((rebound, AuthenticatedSeriesFailureSessionReleaseV3 {
        id, disposition, link_account: *account.key,
        link_authentication_before: authentication_before,
        link_authentication_after: rebound.authentication_id(),
        link_semantic_before: semantic_before, link_semantic_after: semantic_after,
        transition_sequence_before, transition_sequence_after: sequence_after,
        failure_session_transcript_before: transcript_before,
        failure_session_transcript_after: transcript_after, session_terminal_receipt_id,
        archive_postwrite_id, append_receipt_id, reset_receipt_id,
        release_link_preauthorization_id: preauthorization_id,
    }))
}

fn require_unresolved_market_resolution_v2(root: &MarketLifecycleRootV2) -> Outcome<()> {
    require(root.resolution_semantic_id() == ContentId::ZERO
        && root.resolution_data_id() == ContentId::ZERO
        && root.resolution_activation_receipt_id() == ContentId::ZERO,
        ClutchError::MismatchedState)
}

/// Stack-bounded retained-slot authentication from one hostile 1,576-byte
/// GraphV3 preimage. No graph ID or principal supplied by the caller is trusted.
pub(crate) fn authenticate_market_foundation_preallocation_from_bytes_v3(
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV3,
    account_graph_bytes: &[u8],
    slot: MarketFoundationSlotV3,
) -> Outcome<AuthenticatedMarketFoundationPreallocationV3> {
    let graph = authenticate_market_foundation_account_graph_bytes_v3(account_graph_bytes, schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_canonical_market_foundation_core_v3(root.owner_program(), root.account(), graph)?;
    let index = slot.index().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bit = 1u64.checked_shl(u32::try_from(index).map_err(|_| ClutchError::Arithmetic)?)
        .ok_or(ClutchError::Arithmetic)?;
    require(matches!(slot,
        MarketFoundationSlotV3::FailureReplay
            | MarketFoundationSlotV3::FailureIntervalWork
            | MarketFoundationSlotV3::FailureIntervalHistory
            | MarketFoundationSlotV3::ResolutionV5
            | MarketFoundationSlotV3::FractionalPolicy
            | MarketFoundationSlotV3::FractionalLedger
            | MarketFoundationSlotV3::ProductReplayAnchor)
        && (matches!(root.state().phase(), MarketLifecyclePhaseV2::Active | MarketLifecyclePhaseV2::Retiring)
            || (slot == MarketFoundationSlotV3::ProductReplayAnchor
                && root.state().phase() == MarketLifecyclePhaseV2::Terminal))
        && root.state().foundation().initialized_bitmap & bit != 0,
        ClutchError::MismatchedState)?;
    let binding = root.state().binding();
    let capital = root.state().capital();
    let schedule_id = schedule.id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph.graph_id();
    let graph_account = graph.account(slot)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(schedule_id == binding.foundation_schedule_id
        && graph_id == binding.foundation_account_graph_id
        && graph.market_instance_id() == binding.market_instance_id
        && graph.generation() == binding.generation
        && graph_account.bytes() == account.key.to_bytes()
        && account.is_writable && !account.is_signer && !account.executable
        && account.owner.to_bytes() == SYSTEM_PROGRAM_ID && account.data_len() == 0,
        ClutchError::MismatchedState)?;
    let principal_lamports = schedule.slot_principal_lamports[index];
    let observed_balance_lamports = account.lamports();
    let donation_lamports = observed_balance_lamports.checked_sub(principal_lamports)
        .ok_or(ClutchError::MismatchedState)?;
    let rent_refund_owner = Pubkey::new_from_array(capital.rent_refund_owner.bytes());
    let neutral_lamport_sink = Pubkey::new_from_array(capital.neutral_lamport_sink.bytes());
    require(principal_lamports != 0 && account.key != &root.account()
        && account.key != &rent_refund_owner && account.key != &neutral_lamport_sink,
        ClutchError::AccountAlias)?;
    let foundation_transcript_id = root.state().foundation().transcript_id;
    let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
    let id = hashv(&[
        MARKET_FOUNDATION_PREALLOCATION_AUTHENTICATION_DOMAIN_V3, root.account().as_ref(),
        &root.authentication_id().bytes(), &binding.market_instance_id.bytes(),
        &binding.generation.to_le_bytes(), &slot_index.to_le_bytes(), account.key.as_ref(),
        &schedule_id.bytes(), &graph_id.bytes(), &foundation_transcript_id.bytes(),
        &principal_lamports.to_le_bytes(), &donation_lamports.to_le_bytes(),
        &observed_balance_lamports.to_le_bytes(), rent_refund_owner.as_ref(),
        neutral_lamport_sink.as_ref(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedMarketFoundationPreallocationV3 { id, root_account: root.account(),
        root_authentication_id: root.authentication_id(), market_instance_id: binding.market_instance_id,
        generation: binding.generation, slot, account: *account.key,
        foundation_schedule_id: schedule_id.content_id(), foundation_account_graph_id: graph_id.content_id(),
        foundation_transcript_id, principal_lamports, donation_lamports,
        observed_balance_lamports, rent_refund_owner, neutral_lamport_sink })
}

/// Authenticate one retained slot from a fully reconstructed typed GraphV3.
///
/// This overload exists for fixed account frames which reconstruct all 47
/// roles from their canonical accounts. It derives the same private receipt as
/// the hostile byte-preimage path and accepts no caller-supplied graph digest.
pub(crate) fn authenticate_market_foundation_preallocation_v3(
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
    slot: MarketFoundationSlotV3,
) -> Outcome<AuthenticatedMarketFoundationPreallocationV3> {
    graph.validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_canonical_market_foundation_graph_v3(root.owner_program(), root.account(), graph)?;
    let index = slot.index().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bit = 1u64.checked_shl(u32::try_from(index).map_err(|_| ClutchError::Arithmetic)?)
        .ok_or(ClutchError::Arithmetic)?;
    require(matches!(slot,
        MarketFoundationSlotV3::FailureReplay
            | MarketFoundationSlotV3::FailureIntervalWork
            | MarketFoundationSlotV3::FailureIntervalHistory
            | MarketFoundationSlotV3::ResolutionV5
            | MarketFoundationSlotV3::FractionalPolicy
            | MarketFoundationSlotV3::FractionalLedger
            | MarketFoundationSlotV3::ProductReplayAnchor)
        && (matches!(root.state().phase(), MarketLifecyclePhaseV2::Active | MarketLifecyclePhaseV2::Retiring)
            || (slot == MarketFoundationSlotV3::ProductReplayAnchor
                && root.state().phase() == MarketLifecyclePhaseV2::Terminal))
        && root.state().foundation().initialized_bitmap & bit != 0,
        ClutchError::MismatchedState)?;
    let binding = root.state().binding();
    let capital = root.state().capital();
    let schedule_id = schedule.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph.id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_account = graph.account(slot)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(schedule_id == binding.foundation_schedule_id
        && graph_id == binding.foundation_account_graph_id
        && graph.market_instance_id == binding.market_instance_id
        && graph.generation == binding.generation
        && graph_account.bytes() == account.key.to_bytes()
        && account.is_writable && !account.is_signer && !account.executable
        && account.owner.to_bytes() == SYSTEM_PROGRAM_ID && account.data_len() == 0,
        ClutchError::MismatchedState)?;
    let principal_lamports = schedule.slot_principal_lamports[index];
    let observed_balance_lamports = account.lamports();
    let donation_lamports = observed_balance_lamports.checked_sub(principal_lamports)
        .ok_or(ClutchError::MismatchedState)?;
    let rent_refund_owner = Pubkey::new_from_array(capital.rent_refund_owner.bytes());
    let neutral_lamport_sink = Pubkey::new_from_array(capital.neutral_lamport_sink.bytes());
    require(principal_lamports != 0 && account.key != &root.account()
        && account.key != &rent_refund_owner && account.key != &neutral_lamport_sink,
        ClutchError::AccountAlias)?;
    let foundation_transcript_id = root.state().foundation().transcript_id;
    let slot_index = u64::try_from(index).map_err(|_| ClutchError::Arithmetic)?;
    let id = hashv(&[
        MARKET_FOUNDATION_PREALLOCATION_AUTHENTICATION_DOMAIN_V3, root.account().as_ref(),
        &root.authentication_id().bytes(), &binding.market_instance_id.bytes(),
        &binding.generation.to_le_bytes(), &slot_index.to_le_bytes(), account.key.as_ref(),
        &schedule_id.bytes(), &graph_id.bytes(), &foundation_transcript_id.bytes(),
        &principal_lamports.to_le_bytes(), &donation_lamports.to_le_bytes(),
        &observed_balance_lamports.to_le_bytes(), rent_refund_owner.as_ref(),
        neutral_lamport_sink.as_ref(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedMarketFoundationPreallocationV3 {
        id, root_account: root.account(), root_authentication_id: root.authentication_id(),
        market_instance_id: binding.market_instance_id, generation: binding.generation,
        slot, account: *account.key, foundation_schedule_id: schedule_id.content_id(),
        foundation_account_graph_id: graph_id.content_id(), foundation_transcript_id,
        principal_lamports, donation_lamports, observed_balance_lamports,
        rent_refund_owner, neutral_lamport_sink,
    })
}

fn require_canonical_market_foundation_core_v3(
    program_id: &Pubkey,
    root_account: Pubkey,
    graph: AuthenticatedMarketFoundationAccountGraphBytesV3<'_>,
) -> Outcome<()> {
    let market = graph.market_instance_id().bytes();
    let generation = graph.generation();
    let market_binding = graph.account(MarketFoundationSlotV3::MarketBinding)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fixed = [
        (MarketFoundationSlotV3::LifecycleRoot,
            seeds::product_market_lifecycle_root_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::MarketBinding,
            seeds::general_v2_market_binding_pda(program_id, &market).0),
        (MarketFoundationSlotV3::MarketRuntime,
            seeds::general_v2_market_runtime_pda(program_id, &market_binding.bytes()).0),
        (MarketFoundationSlotV3::Hoard, seeds::hoard_v2_pda(program_id, &market).0),
        (MarketFoundationSlotV3::ClaimLedger, seeds::claim_ledger_v3_pda(program_id, &market).0),
        (MarketFoundationSlotV3::FailureAdmissionRoot,
            seeds::failure_market_root_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureRuntimeRoot,
            seeds::failure_external_root_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureReplay,
            seeds::failure_market_replay_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureIntervalWork,
            seeds::failure_market_interval_cell_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureIntervalHistory,
            seeds::failure_market_interval_history_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::ResolutionV5, seeds::resolution_v5_pda(program_id, &market).0),
        (MarketFoundationSlotV3::ProductReplayAnchor,
            seeds::product_market_lifecycle_replay_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::HoardCollateralVault,
            seeds::hoard_token_v2_pda(program_id, &market).0),
    ];
    require(graph.account(MarketFoundationSlotV3::LifecycleRoot)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.bytes()
        == root_account.to_bytes(), ClutchError::MismatchedState)?;
    for (slot, expected) in fixed {
        require(graph.account(slot)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.bytes()
            == expected.to_bytes(), ClutchError::MismatchedState)?;
    }
    Ok(())
}

fn require_canonical_market_foundation_graph_v3(
    program_id: &Pubkey,
    root_account: Pubkey,
    graph: &MarketFoundationAccountGraphV3,
) -> Outcome<()> {
    let market = graph.market_instance_id.bytes();
    let generation = graph.generation;
    let market_binding = graph.account(MarketFoundationSlotV3::MarketBinding)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fixed = [
        (MarketFoundationSlotV3::LifecycleRoot,
            seeds::product_market_lifecycle_root_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::MarketBinding,
            seeds::general_v2_market_binding_pda(program_id, &market).0),
        (MarketFoundationSlotV3::MarketRuntime,
            seeds::general_v2_market_runtime_pda(program_id, &market_binding.bytes()).0),
        (MarketFoundationSlotV3::Hoard, seeds::hoard_v2_pda(program_id, &market).0),
        (MarketFoundationSlotV3::ClaimLedger,
            seeds::claim_ledger_v3_pda(program_id, &market).0),
        (MarketFoundationSlotV3::FailureAdmissionRoot,
            seeds::failure_market_root_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureRuntimeRoot,
            seeds::failure_external_root_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureReplay,
            seeds::failure_market_replay_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureIntervalWork,
            seeds::failure_market_interval_cell_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::FailureIntervalHistory,
            seeds::failure_market_interval_history_v2_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::ResolutionV5, seeds::resolution_v5_pda(program_id, &market).0),
        (MarketFoundationSlotV3::ProductReplayAnchor,
            seeds::product_market_lifecycle_replay_pda(program_id, &market, generation).0),
        (MarketFoundationSlotV3::HoardCollateralVault,
            seeds::hoard_token_v2_pda(program_id, &market).0),
    ];
    require(graph.account(MarketFoundationSlotV3::LifecycleRoot)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.bytes()
        == root_account.to_bytes(), ClutchError::MismatchedState)?;
    for (slot, expected) in fixed {
        require(graph.account(slot)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.bytes()
            == expected.to_bytes(), ClutchError::MismatchedState)?;
    }
    Ok(())
}

fn hash_data(data: &[u8]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(id != ContentId::ZERO, ClutchError::MismatchedState)
}

#[cfg(test)]
mod source_contract_tests {
    use super::{
        AuthenticatedSeriesDealerTerminalOwnerV2, SeriesDealerTerminalObservationV2,
    };
    use clutch_product_series::ContentId;
    use solana_pubkey::Pubkey;

    fn test_id(byte: u8) -> ContentId { ContentId::from_bytes([byte; 32]) }

    fn dealer_terminal_observation() -> SeriesDealerTerminalObservationV2 {
        SeriesDealerTerminalObservationV2 {
            owner_authentication_id: test_id(1),
            dealer_obligation_account: Pubkey::new_from_array([2; 32]),
            dealer_obligation_presemantic_id: test_id(3),
            dealer_state_account: Pubkey::new_from_array([4; 32]),
            dealer_state_presemantic_id: test_id(5),
            terminal_state_receipt_id: test_id(6),
            replay_presemantic_id: test_id(7),
            replay_pre_ordinal: 1,
            owner_terminal_receipt_id: test_id(8),
            rent_refund_owner: Pubkey::new_from_array([9; 32]),
            neutral_lamport_sink: Pubkey::new_from_array([10; 32]),
            root_account: Pubkey::new_from_array([11; 32]),
            root_authentication_id: test_id(12),
            root_data_id: test_id(13),
            root_semantic_id: test_id(14),
            root_binding_id: test_id(15),
            resolution_semantic_id: test_id(16),
            resolution_data_id: test_id(17),
            resolution_activation_receipt_id: test_id(18),
            registry_account: Pubkey::new_from_array([19; 32]),
            registry_authentication_id: test_id(20),
            registry_capability_id: test_id(21),
            registry_release_id: test_id(22),
            capability_profile_id: test_id(23),
            registry_release_artifact_account: Pubkey::new_from_array([24; 32]),
            capability_profile_artifact_account: Pubkey::new_from_array([25; 32]),
            registry_program: Pubkey::new_from_array([26; 32]),
            registry_programdata: Pubkey::new_from_array([27; 32]),
            registry_programdata_sha256: test_id(28),
            compiler_bundle_account: Pubkey::new_from_array([29; 32]),
            compiler_bundle_id: test_id(30),
            compiler_bundle_semantic_id: test_id(31),
            attachment_account: Pubkey::new_from_array([32; 32]),
            attachment_plan_id: test_id(33),
            attachment_semantic_id: test_id(34),
            liquidity_facility_plan_id: test_id(35),
            dealer_obligation_configuration_id: test_id(36),
            link_account: Pubkey::new_from_array([37; 32]),
            link_binding_id: test_id(38),
            link_authentication_before: test_id(39),
            link_data_before: test_id(40),
            link_semantic_before: test_id(41),
            dealer_admission_receipt_id: test_id(42),
            link_transition_sequence_before: 7,
            link_transition_sequence_after: 8,
        }
    }

    #[test]
    fn wrapper_mutation_is_narrow_and_current_artifact_bound() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("fn write_series_market_link_v2<'next>("));
        assert!(!source.contains("pub(crate) fn write_series_market_link_v2<'next>("));
        assert!(source.contains("bundle.value().funding_terms_id == binding.funding_terms_id"));
        assert!(source.contains("bundle.value().funding_quote_id == binding.funding_quote_id"));
        assert!(source.contains("bundle.value().attachment_plan_id == binding.attachment_plan_id"));
        assert!(source.contains("attachment.value().funding_quote_id == binding.funding_quote_id"));
        assert!(source.contains(
            "authorization.link_authentication_id == authenticated.authentication_id()"
        ));
    }

    #[test]
    fn wrapper_terminal_consumes_current_authorization_and_private_owner() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains(
            "authorization: AuthenticatedSeriesWrapperAuthorizationV2"
        ));
        assert!(source.contains("A: AuthenticatedSeriesWrapperTerminalOwnerV2 + ?Sized"));
        assert!(source.contains("authorization.wrapper_admission_receipt_id == admission_receipt"));
        assert!(source.contains("owner.authenticate_series_wrapper_terminal_owner_v2("));
        assert!(source.contains("rebound.state().obligation_status(SeriesLinkObligationV2::Wrapper)"));
    }

    #[test]
    fn failure_release_dispositions_are_disjoint_and_exhaustive() {
        use super::FailureSessionReleaseDispositionV3;

        assert_eq!(FailureSessionReleaseDispositionV3::Resolved.wire_byte(), 1);
        assert_eq!(FailureSessionReleaseDispositionV3::Exhausted.wire_byte(), 2);
        assert_eq!(FailureSessionReleaseDispositionV3::SourceAbsent.wire_byte(), 3);
        assert_eq!(FailureSessionReleaseDispositionV3::SourceRefused.wire_byte(), 4);
    }

    #[test]
    fn current_failure_pin_and_release_have_no_generic_writer_escape() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("require_unresolved_market_resolution_v2(live_root.state())?"));
        assert!(source.contains("state.active_failure_sessions() == 1"));
        assert!(source.contains("disposition == release_link.disposition"));
        assert!(source.contains("session_binding_id == transcript_before"));
        assert!(source.contains("archive.authenticate_series_failure_archive_release_postwrite_v3("));
        assert!(!source.contains("pub(crate) fn write_series_market_link_v2<'next>("));
    }

    #[test]
    fn typed_graph_preallocation_rederives_the_full_current_identity() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("graph.validate(schedule)"));
        assert!(source.contains("require_canonical_market_foundation_graph_v3("));
        assert!(source.contains("let graph_id = graph.id(schedule)"));
        assert!(source.contains("graph_account.bytes() == account.key.to_bytes()"));
        assert!(!source.contains("_caller_foundation_account_graph_id"));
    }

    #[test]
    fn current_registry_refs_are_minted_only_from_hostile_v3_authentication() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains(
            "refs = authenticate_series_registry_capability_refs_v3(registry)?"
        ));
        assert!(source.contains("&refs.id.bytes()"));
        assert!(source.contains("&[u8::from(refs.activation_consumed)]"));
        assert!(source.contains("refs.compiler_bundle_id.content_id() != refs.funding_terms_id.content_id()"));
        assert!(!source.contains("SeriesRegistryAccountV2::decode"));
    }

    #[test]
    fn fractional_root_mutations_are_current_exact_and_narrow() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("current.admit_product_family_child_into("));
        assert!(source.contains("current.terminalize_fractional_family_into("));
        assert!(source.contains("owner.authenticate_product_fractional_family_admission_owner_v2("));
        assert!(source.contains("owner.authenticate_product_fractional_family_terminal_owner_v2("));
        assert!(source.contains("claim_ledger_before_id != claim_ledger_after_id"));
        assert!(source.contains("fractional_release_id == binding.registry_release_id"));
        assert!(source.contains("claim_ledger_transition_id, fractional_release_id, binding.capability_profile_id"));
        assert!(!source.contains("pub(crate) fn write_market_lifecycle_root_v2<'next>("));
    }

    #[test]
    fn historical_product_market_has_no_wrapper_writer_authority() {
        let historical = include_str!("product_market.rs");
        assert!(!historical.contains(concat!(
            "AuthenticatedSeriesWrapperAuthorization",
            "V1"
        )));
        assert!(!historical.contains(concat!(
            "authenticate_series_wrapper_authorization_",
            "v1("
        )));
        assert!(!historical.contains(concat!(
            "admit_series_wrapper_obligation_",
            "v1("
        )));
        assert!(!historical.contains(concat!(
            "terminalize_series_wrapper_obligation_",
            "v1("
        )));
    }

    #[test]
    fn current_completion_authorization_and_replay_writers_are_private() {
        let source = include_str!("product_series_current.rs");
        assert!(!source.contains("activate_complete_and_record_current_series_v3"));
        assert!(!source.contains("AuthenticatedProductSeriesActivationCompletionV3"));
        assert!(!source.contains("AuthenticatedSeriesFundingAccountV3"));
        assert!(!source.contains("fn write_series_funding_state_v3("));
        assert!(source.contains("fn authorize_series_funding_completion_v4("));
        assert!(!source.contains(
            "pub(crate) fn authorize_series_funding_completion_v4("
        ));
        assert!(source.contains("fn write_series_lifecycle_replay_v2("));
        assert!(!source.contains("pub(crate) fn write_series_lifecycle_replay_v2("));
        assert!(source.contains("project_pending_completion_poststate(series, quote, attachment)"));
        assert!(source.contains("binding.completion_authorization_id == authorization_id"));
    }

    #[test]
    fn current_founder_tail_orders_authorization_replay_then_funding_clear() {
        let source = include_str!("product_series_current.rs");
        let tail = source
            .split("fn activate_record_and_complete_current_series_v4(")
            .nth(1)
            .unwrap()
            .split("/// Promote one exact unresolved")
            .next()
            .unwrap();
        let authorization = tail
            .find("authorize_series_funding_completion_v4(")
            .unwrap();
        let replay = tail.find(".record_admission(admission)").unwrap();
        let funding = tail
            .find("complete_series_funding_v4_with_binding(")
            .unwrap();
        assert!(authorization < replay && replay < funding);
        assert!(tail.contains("occurrence_completion_receipt_id: completion_authorization_id.content_id()"));
        assert!(tail.contains("funding_state_after_id: projected_funding_state_after_id.content_id()"));
        assert!(tail.contains("completion_authorization_id,"));
        assert!(source.contains("struct AuthenticatedProductSeriesActivationCompletionV4"));
        assert!(source.contains("pub(super) fn into_direct_activation_parts("));
    }

    #[test]
    fn current_founder_outer_is_one_shot_and_uses_live_sequences_and_donations() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("pub(crate) fn compose_current_product_market_founder_v4<"));
        assert!(source.contains("CurrentProductMarketFoundationCursorV4"));
        assert!(source.contains("P: AuthenticatedProductMarketFoundationStepPostwriteV3"));
        assert!(source.contains("self.creation.into_product_activation_parts_v3("));
        assert!(source.contains("root_transition_sequence_before.to_le_bytes()"));
        assert!(!source.contains("&2_u64.to_le_bytes()"));
        assert!(source.contains("final_foundation_donation_lamports.to_le_bytes()"));
        assert!(source.contains("capital.principal_remaining_lamports == capital.principal_total_lamports"));
        assert!(source.contains("root_principal\n                == rent.minimum_balance(MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2)?"));
        assert!(!source.contains("pub(crate) fn write_market_lifecycle_root_v2<'next>("));
        assert!(!source.contains("pub(crate) fn write_series_market_link_v2<'next>("));
    }

    #[test]
    fn funding_v4_authentication_is_fresh_and_raw_writer_is_private() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("SERIES_FUNDING_AUTHENTICATION_DOMAIN_V4"));
        assert!(source.contains("SeriesFundingAccountV4::decode(&data)"));
        assert!(source.contains("account.data_len() == SERIES_FUNDING_ACCOUNT_BYTES_V4"));
        assert!(source.contains("fn write_series_funding_state_v4("));
        assert!(!source.contains("pub(crate) fn write_series_funding_state_v4("));
        assert!(source.contains("rebound.authentication_id() != authentication_before"));
        assert!(source.contains("rebound.data_id() != data_before"));
        assert!(source.contains("struct AuthenticatedProductSeriesFundingReservationV4"));
        assert!(source.contains("fn reserve_series_funding_v4_with_binding("));
        assert!(!source.contains("pub(crate) fn reserve_series_funding_v4_with_binding("));
        assert!(source.contains(
            "pending_pre_source_reservation_binding_id\n                == binding_id.content_id()"
        ));
        assert!(source.contains(
            "pending_clock_receipt_id == binding.clock_receipt_id"
        ));
        assert!(source.contains("fn complete_series_funding_v4_with_binding("));
        assert!(!source.contains(
            "pub(crate) fn complete_series_funding_v4_with_binding("
        ));
        assert!(source.contains(
            "struct AuthenticatedSeriesFundingCompletionAuthorizationV4"
        ));
        assert!(source.contains(
            "facts.funding_account_authentication_pending_id\n                == reservation.funding_authentication_pending_id()"
        ));
        assert!(source.contains("binding.completion_authorization_id == authorization_id"));
        assert!(source.contains("after_state_id == projected_state_after_id"));
        assert!(source.contains(
            "rebound.state().pending_pre_source_reservation_binding_id.is_zero()"
        ));
    }

    #[test]
    fn current_resolution_write_requires_exact_slot_and_private_postwrites() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("trait AuthenticatedMarketResolutionActivationWriteV2"));
        assert!(source.contains("slot10.slot() == MarketFoundationSlotV3::ResolutionV5"));
        assert!(source.contains(
            "slot10.root_authentication_id() == authenticated.authentication_id()"
        ));
        assert!(source.contains(
            "activation.failure_resolution_receipt_id() == failure_resolution_receipt_id"
        ));
        assert!(source.contains("root.record_resolution_activation_into("));
        assert!(source.contains("AuthenticatedMarketResolutionActivationPostwriteV2"));
        assert!(!source.contains("pub(crate) fn write_market_lifecycle_root_v2<'next>("));
    }

    #[test]
    fn current_dealer_admission_is_first_lease_only_and_owner_bound() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("fn authenticate_series_dealer_authorization_v2("));
        assert!(source.contains("trait AuthenticatedSeriesDealerAdmissionOwnerV2"));
        assert!(source.contains("fn admit_series_dealer_obligation_v2<'next, A>("));
        assert!(source.contains("root.state().phase() == MarketLifecyclePhaseV2::Active"));
        assert!(source.contains("root.state().resolution_semantic_id() == ContentId::ZERO"));
        assert!(source.contains("SeriesLinkObligationV2::Dealer"));
        assert!(source.contains("SeriesLinkObligationStatusV2::EnabledNeverFounded"));
        assert!(source.contains("owner.authenticate_series_dealer_admission_owner_v2("));
        assert!(source.contains("AuthenticatedSeriesDealerAdmissionV2"));
        assert!(!source.contains("pub(crate) fn write_series_market_link_v2<'next>("));
    }

    #[test]
    fn current_dealer_terminal_writer_is_release_bound_and_ordered() {
        let source = include_str!("product_series_current.rs");
        let terminal = source
            .split_once("pub(crate) fn terminalize_series_dealer_obligation_v2<A>(")
            .expect("current Dealer terminal writer")
            .1
            .split_once("/// Persist the first Wrapper admission")
            .expect("bounded Dealer terminal writer")
            .0;
        let signature = terminal
            .split_once("where\n    A: AuthenticatedSeriesDealerTerminalOwnerV2")
            .expect("private by-value Dealer authority")
            .0;
        assert!(!signature.contains("ContentId"));
        assert!(terminal.contains("authenticate_market_lifecycle_root_v2("));
        assert!(terminal.contains("authenticate_series_registry_account_v3("));
        assert!(terminal.contains("authenticate_registry_capability_v4("));
        assert!(terminal.contains("accounts.registry_programdata"));
        assert!(terminal.contains("registry.programdata_sha256()"));
        assert!(terminal.contains("CompiledProductSeriesBundleV6"));
        assert!(terminal.contains("SeriesAttachmentPlanV5"));
        assert!(terminal.contains(
            "let resolution_semantic_id = root.state().resolution_semantic_id()"
        ));
        assert!(terminal.contains("resolution_activation_receipt_id"));
        assert!(terminal.contains(
            "root.state().phase() == MarketLifecyclePhaseV2::Active"
        ));
        assert!(!terminal.contains("MarketLifecyclePhaseV2::Retiring"));
        assert!(terminal.contains("link_binding.rent_refund_owner == root_capital.rent_refund_owner"));
        assert!(terminal.contains("SeriesLinkObligationStatusV2::Live"));
        assert!(terminal.contains("SeriesLinkObligationStatusV2::Terminal"));
        assert!(!terminal.contains("SeriesMarketLinkAccountV1"));
        assert!(!terminal.contains("SeriesRegistryAccountV2"));

        let owner_accept = terminal
            .find("owner.consume_series_dealer_terminal_owner_v2(observation)?")
            .expect("Dealer owner acceptance");
        let transition = terminal
            .find("let terminal_projection = SeriesLinkObligationTerminalProjectionV2")
            .expect("Product terminal projection");
        let write = terminal
            .find("let rebound = write_series_market_link_v2(")
            .expect("private Product writer");
        assert!(owner_accept < transition && transition < write);
        assert!(source.contains(concat!(
            "#[derive(Debug, Eq, PartialEq)]\n",
            "pub(crate) struct AuthenticatedSeriesDealerTerminalV2"
        )));
    }

    #[test]
    fn dealer_terminal_observation_commits_hostile_release_and_link_prestate() {
        let valid = dealer_terminal_observation();
        assert_ne!(valid.id(), ContentId::ZERO);

        let mut hostile_programdata = valid;
        hostile_programdata.registry_programdata_sha256 = test_id(43);
        assert_ne!(valid.id(), hostile_programdata.id());

        let mut hostile_release = valid;
        hostile_release.registry_release_id = test_id(44);
        assert_ne!(valid.id(), hostile_release.id());

        let mut hostile_resolution = valid;
        hostile_resolution.resolution_semantic_id = test_id(45);
        assert_ne!(valid.id(), hostile_resolution.id());

        let mut hostile_link = valid;
        hostile_link.link_semantic_before = test_id(46);
        assert_ne!(valid.id(), hostile_link.id());

        let mut hostile_owner = valid;
        hostile_owner.owner_terminal_receipt_id = test_id(47);
        assert_ne!(valid.id(), hostile_owner.id());
    }

    #[test]
    fn dealer_terminal_owner_defaults_to_refusal() {
        struct RefusingOwner;
        impl AuthenticatedSeriesDealerTerminalOwnerV2 for RefusingOwner {}

        assert!(RefusingOwner.owner_authentication_id().is_err());
        assert!(RefusingOwner
            .consume_series_dealer_terminal_owner_v2(dealer_terminal_observation())
            .is_err());
    }

    #[test]
    fn current_retained_preallocation_partition_is_exact() {
        for slot in [
            MarketFoundationSlotV3::FailureReplay,
            MarketFoundationSlotV3::FailureIntervalWork,
            MarketFoundationSlotV3::FailureIntervalHistory,
            MarketFoundationSlotV3::ResolutionV5,
            MarketFoundationSlotV3::FractionalPolicy,
            MarketFoundationSlotV3::FractionalLedger,
            MarketFoundationSlotV3::ProductReplayAnchor,
        ] {
            assert!(is_retained_current_foundation_slot_v3(slot));
        }
        for slot in [
            MarketFoundationSlotV3::LifecycleRoot,
            MarketFoundationSlotV3::MarketBinding,
            MarketFoundationSlotV3::MarketRuntime,
            MarketFoundationSlotV3::Hoard,
            MarketFoundationSlotV3::ClaimLedger,
            MarketFoundationSlotV3::FailureAdmissionRoot,
            MarketFoundationSlotV3::FailureRuntimeRoot,
            MarketFoundationSlotV3::HoardCollateralVault,
            MarketFoundationSlotV3::OutcomeMint(0),
            MarketFoundationSlotV3::OutcomeCustody(0),
        ] {
            assert!(!is_retained_current_foundation_slot_v3(slot));
        }
    }

    #[test]
    fn current_outcome_custody_orders_debit_before_external_postwrite() {
        let source = include_str!("product_series_current.rs");
        let body = source
            .split_once("pub(crate) fn record_next_outcome_custody_v1<'next>(")
            .expect("current outcome custody writer")
            .1
            .split_once("/// Consume one exact family-private physical postwrite")
            .expect("bounded current outcome custody writer")
            .0;
        let transfer = body
            .find("invoke_current_founder_transfer(")
            .expect("FoundationVault debit");
        let allocation = body
            .find("allocate_assign_current_founder_account(")
            .expect("release-selected allocation");
        let initialization = body
            .find("invoke_current_outcome_custody_initialization_v1(")
            .expect("release-selected initialization");
        let acceptance = body
            .find("accept_outcome_custody_founding_step_v1(")
            .expect("hostile custody acceptance");
        let product = body
            .find("self.record_foundation_step(root, postwrite")
            .expect("Product cursor consumption");
        assert!(transfer < allocation);
        assert!(allocation < initialization);
        assert!(initialization < acceptance);
        assert!(acceptance < product);
        assert!(body.contains("claim_binding.binding_id().bytes()"));
        assert!(body.contains("custody_plan.value.deployment.receipt_id()"));
        assert!(body.contains("principal_lamports == rent.minimum_balance"));
    }

    #[test]
    fn current_claim_mint_orders_debit_before_token_postwrite() {
        let source = include_str!("product_series_current.rs");
        let body = source
            .split_once("pub(crate) fn record_next_claim_mint_v2<'next>(")
            .expect("current claim mint writer")
            .1
            .split_once("/// Create one active release-selected outcome custody")
            .expect("bounded current claim mint writer")
            .0;
        let transfer = body
            .find("invoke_current_founder_transfer(")
            .expect("FoundationVault debit");
        let allocation = body
            .find("allocate_assign_current_founder_account(")
            .expect("Token-2022 allocation");
        let initialization = body
            .find("token::initialize_outcome_mint(")
            .expect("Token-2022 mint initialization");
        let admission = body
            .find("token::admit_mint(")
            .expect("hostile Token-2022 admission");
        let acceptance = body
            .find("accept_claim_mint_founding_step_v2(")
            .expect("claim-release acceptance");
        let product = body
            .find("self.record_foundation_step(root, postwrite")
            .expect("Product cursor consumption");
        assert!(transfer < allocation);
        assert!(allocation < initialization);
        assert!(initialization < admission);
        assert!(admission < acceptance);
        assert!(acceptance < product);
        assert!(body.contains("claim_release.token_programdata()"));
        assert!(body.contains("claim_release.loader_receipt_id()"));
        assert!(body.contains("principal_lamports == rent.minimum_balance"));
    }
}
