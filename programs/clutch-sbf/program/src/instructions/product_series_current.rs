//! Current Product/Series account authentication for the 47-slot successor.
//!
//! This module owns only hostile account authentication. Historical RegistryV2,
//! FundingV2, replay V1, root V1, and link V1 helpers remain available to
//! decode old bytes, but no current successor receipt is constructible from
//! them. Mutation remains in event-specific atomic composers; this module does
//! not expose a generic successor writer.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, authenticate_registry_capability_for_registration_v3,
};
use crate::instructions::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV6, AuthenticatedSeriesSourceArtifactsV5,
};
use crate::seeds;
use clutch_product_series::{
    authenticate_market_foundation_account_graph_bytes_v3,
    AuthenticatedMarketFoundationAccountGraphBytesV3, CompiledProductSeriesBundleV6,
    ComponentDebitV1, ContentId, FixedCodec,
    MarketFoundationAccountGraphV3, MarketFoundationScheduleV3, MarketFoundationSlotV3,
    MarketInstanceV2Id,
    AuthenticatedMarketFamilyAuthorityV1, MarketFamilyAggregatorV1, MarketFamilyStatusV1,
    MarketFamilyV1,
    MarketLifecyclePhaseV2, MarketLifecycleRootV2, MarketResolutionActivationV2,
    AuthenticatedSeriesFundingAuthorityV4, SeriesFundingReservationBindingV4,
    SeriesFundingReservationBindingV4Id, SeriesFundingStateV3, SeriesFundingStateV4,
    SeriesFundingStateV4Id,
    AuthenticatedSeriesFundingAuthorityV3, SeriesFundingComponentV2,
    SeriesFundingPhaseV3, SeriesFundingQuoteV5, SeriesFundingTermsV2Id,
    RegistryCapabilityProjectionV2,
    SeriesAttachmentPlanV5, SeriesAttachmentPlanV5Id, SeriesLifecycleReplayBindingV2,
    SeriesLifecycleReplayBindingV2Id,
    SeriesLifecycleAdmissionProjectionV2, SeriesLifecycleReplayPhaseV2,
    SeriesLifecycleReplayV2, SeriesLinkObligationAdmissionProjectionV2,
    SeriesLinkObligationDispositionV2, SeriesLinkObligationStatusV2,
    SeriesLinkObligationTerminalProjectionV2, SeriesLinkObligationV2,
    SeriesMarketDispositionV1, SeriesMarketLinkPhaseV2, SeriesMarketLinkV2,
    SeriesMarketLinkV2Id, SeriesPlanV5, SeriesPlanV5Id, SourceOccurrenceV1Id,
    SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v2, MarketLifecycleRootAccountV2,
    SeriesFundingAccountV3, SeriesFundingAccountV4, SeriesLifecycleReplayAccountV2,
    SeriesMarketLinkAccountV2,
    SeriesRegistryAccountV3, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2,
    SERIES_FUNDING_ACCOUNT_BYTES_V3, SERIES_FUNDING_ACCOUNT_BYTES_V4,
    SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V2, SERIES_REGISTRY_ACCOUNT_BYTES_V3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SERIES_REGISTRY_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-registry-account-authentication/v3\0";
const SERIES_REGISTRY_CAPABILITY_REFS_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-registry-capability-refs/v3\0";
const REGISTRY_CAPABILITY_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/registry-capability-authentication/v4\0";
const SERIES_FUNDING_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-funding-account-authentication/v3\0";
const SERIES_FUNDING_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/series-funding-account-authentication/v4\0";
const SERIES_FUNDING_RESERVATION_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-funding-reservation-postwrite/v4\0";
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
const PRODUCT_CURRENT_LINK_ACTIVATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-current-link-activation/v2\0";
const SERIES_FUNDING_COMPLETION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/series-funding-completion/v3\0";
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

/// Exact current 0x80/version3 funding authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesFundingAccountV3 {
    account: Pubkey,
    value: SeriesFundingAccountV3,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesFundingAccountV3 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> SeriesFundingAccountV3 { self.value }
    pub(crate) const fn state(self) -> SeriesFundingStateV3 { self.value.state }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(self) -> bool { self.writable }
    pub(crate) const fn data_id(self) -> ContentId { self.data_id }
    pub(crate) const fn authentication_id(self) -> ContentId { self.authentication_id }
}

/// Exact current 0x80/version4 acyclic funding authentication.
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

/// Consume the exact current a4/a5/ClaimLedger terminal postwrite and latch the
/// Fractional terminal states in RootV2 before any family rent is disposed.
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
    let binding = authenticated.state().binding();
    require(account.is_writable && *account.key == authenticated.account()
        && account.owner == program_id && successor.binding() == binding,
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

pub(crate) fn authenticate_series_funding_account_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesFundingAccountV3> {
    require(
        !account.is_signer && !account.executable && account.is_writable == require_writable
            && account.owner == program_id && account.data_len() == SERIES_FUNDING_ACCOUNT_BYTES_V3,
        ClutchError::MismatchedState,
    )?;
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesFundingAccountV3::decode(&data)?;
    require(value.state.series_plan_id == expected_series_plan_id, ClutchError::MismatchedState)?;
    let (expected, bump) = seeds::series_funding_pda(program_id, &expected_series_plan_id.bytes());
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let state_id = value.state.id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_lamports = account.lamports();
    require(observed_lamports >= value.rent_principal_lamports, ClutchError::MismatchedState)?;
    let mut vault_rent = [0u8; 40];
    for (index, principal) in value.collateral_vault_rent_principal_lamports.iter().enumerate() {
        let at = index.checked_mul(8).ok_or(ClutchError::Arithmetic)?;
        vault_rent[at..at + 8].copy_from_slice(&principal.to_le_bytes());
    }
    let authentication_id = hashv(&[
        SERIES_FUNDING_AUTHENTICATION_DOMAIN_V3, account.key.as_ref(), program_id.as_ref(),
        &data_id.bytes(), &state_id.bytes(), &value.rent_principal_lamports.to_le_bytes(),
        &vault_rent, &observed_lamports.to_le_bytes(), &[value.stored_bump],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesFundingAccountV3 { account: *account.key, value, observed_lamports,
        writable: account.is_writable, data_id, authentication_id })
}

/// Hostile-authenticate only the exact acyclic FundingV4 account coordinate.
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

/// Private raw FundingV3 writer. Current event composers must derive one exact
/// legal successor and consume it before this semantic-owner boundary writes.
fn write_series_funding_state_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesFundingAccountV3,
    successor: SeriesFundingStateV3,
) -> Outcome<AuthenticatedSeriesFundingAccountV3> {
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
    let live = authenticate_series_funding_account_v3(
        program_id,
        account,
        before.state.series_plan_id,
        true,
    )?;
    require(
        live == authenticated,
        ClutchError::MismatchedState,
    )?;
    let successor_account = SeriesFundingAccountV3 {
        state: successor,
        rent_principal_lamports: before.rent_principal_lamports,
        collateral_vault_rent_principal_lamports: before
            .collateral_vault_rent_principal_lamports,
        stored_bump: before.stored_bump,
    };
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        successor_account.encode(&mut data)?;
    }
    let rebound = authenticate_series_funding_account_v3(
        program_id,
        account,
        before.state.series_plan_id,
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
    let binding = authenticated.state().binding();
    require(account.is_writable && *account.key == authenticated.account()
        && account.owner == program_id && successor.binding() == binding,
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

/// Private final receipt for the inseparable RootV2/LinkV2 activation,
/// FundingV3 completion, and replayV2 admission transition.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductSeriesActivationCompletionV3 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    root_account: Pubkey,
    root_authentication_after: ContentId,
    link_account: Pubkey,
    link_authentication_after: ContentId,
    link_activation_receipt_id: ContentId,
    market_admission_receipt_id: ContentId,
    funding_account: Pubkey,
    funding_authentication_before: ContentId,
    funding_authentication_after: ContentId,
    funding_state_before_id: ContentId,
    funding_state_after_id: ContentId,
    funding_completion_receipt_id: ContentId,
    replay_account: Pubkey,
    replay_authentication_before: ContentId,
    replay_authentication_after: ContentId,
    replay_state_before_id: ContentId,
    replay_state_after_id: ContentId,
    replay_admission_projection_id: ContentId,
}

impl AuthenticatedProductSeriesActivationCompletionV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.ordinal }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn link_activation_receipt_id(&self) -> ContentId {
        self.link_activation_receipt_id
    }
    pub(crate) const fn funding_completion_receipt_id(&self) -> ContentId {
        self.funding_completion_receipt_id
    }
    pub(crate) const fn replay_admission_projection_id(&self) -> ContentId {
        self.replay_admission_projection_id
    }
}

struct ExactSeriesFundingCompletionAuthorityV3 {
    state_before_id: ContentId,
    completion_receipt_id: ContentId,
}

impl AuthenticatedSeriesFundingAuthorityV3 for ExactSeriesFundingCompletionAuthorityV3 {
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

    fn current_bucket(
        &self,
        _series: &SeriesPlanV5,
    ) -> clutch_product_series::Result<u64> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV3,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _series_market_link_id: ContentId,
        _disposition: SeriesMarketDispositionV1,
        _debits: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        state: &SeriesFundingStateV3,
        completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        let state_id = state.id()?.content_id();
        if state_id != self.state_before_id
            || completion_receipt_id != self.completion_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV3,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV3,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        _state: &SeriesFundingStateV3,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

/// Internal current transition. The future founder compositor must call this
/// directly with the exact accepted MarketCore receipt retained by its
/// non-Copy current creation authority; no intermediate half is exported.
#[allow(clippy::too_many_arguments)]
fn activate_complete_and_record_current_series_v3(
    program_id: &Pubkey,
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    graph: &MarketFoundationAccountGraphV3,
    accepted_market_core_receipt_id: ContentId,
    root_account: &AccountInfo<'_>,
    authenticated_root: AuthenticatedMarketLifecycleRootV2<'_>,
    link_account: &AccountInfo<'_>,
    authenticated_link: AuthenticatedSeriesMarketLinkV2<'_>,
    funding_account: &AccountInfo<'_>,
    authenticated_funding: AuthenticatedSeriesFundingAccountV3,
    replay_account: &AccountInfo<'_>,
    authenticated_replay: AuthenticatedSeriesLifecycleReplayV2,
    root_successor_output: &mut MarketLifecycleRootV2,
    root_rebound_output: &mut MarketLifecycleRootAccountV2,
    link_rebound_output: &mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedProductSeriesActivationCompletionV3> {
    require_live(accepted_market_core_receipt_id)?;
    let root = authenticated_root.state();
    let root_binding = root.binding();
    let link = authenticated_link.state();
    let link_binding = link.binding();
    let funding = authenticated_funding.state();
    let replay = authenticated_replay.state();
    let replay_binding: SeriesLifecycleReplayBindingV2 = replay.binding();
    let series = artifacts.series();
    let quote = artifacts.quote();
    let attachment = artifacts.attachment();
    let bundle_value = bundle.bundle();
    let quote_id = quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = attachment
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let series_id = series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = quote
        .foundation
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    graph
        .validate(&quote.foundation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph
        .id(&quote.foundation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_before = link
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_admission = SeriesMarketAdmissionProjectionV2::new(root_binding, *link, 1)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_admission_receipt_id = market_admission.id();
    let expected_root_link_transcript = hashv(&[
        b"dragons-clutch/market-series-link-transcript/v2",
        &ContentId::ZERO.bytes(),
        &market_admission_receipt_id.bytes(),
        &2_u64.to_le_bytes(),
    ]);
    let funding_state_before_id = funding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let replay_state_before_id = replay
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let disposition_byte = match link_binding.disposition {
        SeriesMarketDispositionV1::Founder => 1,
        SeriesMarketDispositionV1::Converger => 2,
    };
    require(
        authenticated_root.is_writable()
            && authenticated_link.is_writable()
            && authenticated_funding.is_writable()
            && authenticated_replay.is_writable()
            && root_account.key != link_account.key
            && root_account.key != funding_account.key
            && root_account.key != replay_account.key
            && link_account.key != funding_account.key
            && link_account.key != replay_account.key
            && funding_account.key != replay_account.key
            && root.phase() == MarketLifecyclePhaseV2::Founding
            && root.admitted_series_links() == 1
            && root.live_series_links() == 1
            && root.retired_series_links() == 0
            && root.series_link_transcript_id() == expected_root_link_transcript
            && link.phase() == SeriesMarketLinkPhaseV2::PendingMarket
            && funding.phase == SeriesFundingPhaseV3::Pending
            && replay.phase() == SeriesLifecycleReplayPhaseV2::Open
            && root_binding.foundation_schedule_id == schedule_id
            && root_binding.foundation_account_graph_id == graph_id
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
            && funding.series_plan_id == series_id
            && funding.funding_terms_id == registry.funding_terms_id()
            && funding.funding_quote_id == quote_id
            && funding.attachment_plan_id == attachment_id
            && funding.compiler_bundle_id == bundle.bundle_id()
            && funding.pending_ordinal == link_binding.ordinal
            && funding.pending_market_instance_id == link_binding.market_instance_id.content_id()
            && funding.pending_source_occurrence_id
                == link_binding.source_occurrence_id.content_id()
            && funding.pending_series_market_link_id == link_semantic_before.content_id()
            && funding.pending_disposition == Some(link_binding.disposition)
            && funding.pending_reservation_receipt_id == link_binding.funding_debit_receipt_id
            && funding.transition_sequence == link_binding.funding_transition_sequence
            && registry.series_plan_id() == series_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && bundle_value.series_plan_id == series_id
            && bundle_value.funding_terms_id == registry.funding_terms_id()
            && bundle_value.funding_quote_id == quote_id
            && bundle_value.attachment_plan_id == attachment_id
            && bundle_value.registry_release_id == registry.registry_release_id()
            && bundle_value.capability_profile_id.content_id()
                == registry.capability_profile_id()
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

    let root_semantic_before = root
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    *root_successor_output = (*root)
        .activate(&quote.foundation, accepted_market_core_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_authentication_before = authenticated_root.authentication_id();
    let rebound_root = write_market_lifecycle_root_v2(
        program_id,
        root_account,
        authenticated_root,
        root_successor_output,
        root_rebound_output,
    )?;
    let root_semantic_after = rebound_root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let link_successor = (*link)
        .activate(1, market_admission_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_authentication_before = authenticated_link.authentication_id();
    let rebound_link = write_series_market_link_v2(
        program_id,
        link_account,
        authenticated_link,
        &link_successor,
        link_rebound_output,
    )?;
    let link_semantic_after = rebound_link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_activation_receipt_id = hashv(&[
        PRODUCT_CURRENT_LINK_ACTIVATION_DOMAIN_V2,
        program_id.as_ref(),
        root_account.key.as_ref(),
        &root_authentication_before.bytes(),
        &rebound_root.authentication_id().bytes(),
        &root_semantic_before.bytes(),
        &root_semantic_after.bytes(),
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

    let funding_completion_receipt_id = hashv(&[
        SERIES_FUNDING_COMPLETION_DOMAIN_V3,
        program_id.as_ref(),
        funding_account.key.as_ref(),
        &authenticated_funding.authentication_id().bytes(),
        &authenticated_funding.data_id().bytes(),
        &funding_state_before_id.bytes(),
        &link_activation_receipt_id.bytes(),
        &market_admission_receipt_id.bytes(),
        &series_id.bytes(),
        &link_binding.ordinal.to_le_bytes(),
        &link_binding.market_instance_id.bytes(),
        &link_binding.generation.to_le_bytes(),
        &link_binding.source_occurrence_id.bytes(),
        &bundle.bundle_id().bytes(),
        &[disposition_byte],
    ]);
    require_live(funding_completion_receipt_id)?;
    let completion_authority = ExactSeriesFundingCompletionAuthorityV3 {
        state_before_id: funding_state_before_id,
        completion_receipt_id: funding_completion_receipt_id,
    };
    let mut funding_successor = funding;
    let completed_ordinal = funding_successor
        .complete_pending(
            &completion_authority,
            series,
            quote,
            attachment,
            funding_completion_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(completed_ordinal == link_binding.ordinal, ClutchError::MismatchedState)?;
    let rebound_funding = write_series_funding_state_v3(
        program_id,
        funding_account,
        authenticated_funding,
        funding_successor,
    )?;
    let funding_state_after_id = rebound_funding
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();

    let admission = SeriesLifecycleAdmissionProjectionV2 {
        binding_id: replay_binding
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        series_plan_id: series_id,
        ordinal: link_binding.ordinal,
        funding_account_id: ContentId::from_bytes(funding_account.key.to_bytes()),
        funding_state_before_id,
        funding_state_after_id,
        occurrence_completion_receipt_id: funding_completion_receipt_id,
        link_account_id: ContentId::from_bytes(link_account.key.to_bytes()),
        link_activation_receipt_id,
        market_admission_receipt_id,
        market_instance_id: link_binding.market_instance_id,
        source_occurrence_id: link_binding.source_occurrence_id,
        compiler_bundle_id: bundle.bundle_id(),
        disposition: link_binding.disposition,
        generation: link_binding.generation,
    };
    let replay_admission_projection_id = admission
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_successor = replay
        .record_admission(admission)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound_replay = write_series_lifecycle_replay_v2(
        program_id,
        replay_account,
        authenticated_replay,
        replay_successor,
    )?;
    let replay_state_after_id = rebound_replay
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let id = hashv(&[
        SERIES_LIFECYCLE_REPLAY_POSTWRITE_DOMAIN_V2,
        program_id.as_ref(),
        replay_account.key.as_ref(),
        &authenticated_replay.authentication_id().bytes(),
        &rebound_replay.authentication_id().bytes(),
        &replay_state_before_id.bytes(),
        &replay_state_after_id.bytes(),
        &replay_admission_projection_id.bytes(),
        &funding_completion_receipt_id.bytes(),
        &link_activation_receipt_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesActivationCompletionV3 {
        id,
        series_plan_id: series_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        root_account: rebound_root.account(),
        root_authentication_after: rebound_root.authentication_id(),
        link_account: rebound_link.account(),
        link_authentication_after: rebound_link.authentication_id(),
        link_activation_receipt_id,
        market_admission_receipt_id,
        funding_account: rebound_funding.account(),
        funding_authentication_before: authenticated_funding.authentication_id(),
        funding_authentication_after: rebound_funding.authentication_id(),
        funding_state_before_id,
        funding_state_after_id,
        funding_completion_receipt_id,
        replay_account: rebound_replay.account(),
        replay_authentication_before: authenticated_replay.authentication_id(),
        replay_authentication_after: rebound_replay.authentication_id(),
        replay_state_before_id,
        replay_state_after_id,
        replay_admission_projection_id,
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
    fn current_activation_completion_and_replay_are_one_private_transition() {
        let source = include_str!("product_series_current.rs");
        assert!(source.contains("fn activate_complete_and_record_current_series_v3("));
        assert!(!source.contains(
            "pub(crate) fn activate_complete_and_record_current_series_v3("
        ));
        assert!(source.contains("fn write_series_funding_state_v3("));
        assert!(source.contains("fn write_series_lifecycle_replay_v2("));
        assert!(!source.contains("pub(crate) fn write_series_funding_state_v3("));
        assert!(!source.contains("pub(crate) fn write_series_lifecycle_replay_v2("));
        assert!(source.contains("root.series_link_transcript_id() == expected_root_link_transcript"));
        assert!(source.contains("funding.pending_series_market_link_id == link_semantic_before.content_id()"));
        assert!(source.contains("funding.pending_reservation_receipt_id == link_binding.funding_debit_receipt_id"));
        assert!(source.contains(".record_admission(admission)"));
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
}
