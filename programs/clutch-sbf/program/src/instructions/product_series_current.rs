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
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::seeds;
use clutch_product_series::{
    authenticate_market_foundation_account_graph_bytes_v3,
    AuthenticatedMarketFoundationAccountGraphBytesV3, CompiledProductSeriesBundleV6, ContentId,
    FixedCodec,
    MarketFoundationScheduleV3, MarketFoundationSlotV3, MarketInstanceV2Id,
    MarketLifecyclePhaseV2, MarketLifecycleRootV2, SeriesFundingStateV3,
    SeriesAttachmentPlanV5, SeriesAttachmentPlanV5Id, SeriesLifecycleReplayBindingV2Id,
    SeriesLifecycleReplayV2, SeriesLinkObligationAdmissionProjectionV2,
    SeriesLinkObligationDispositionV2, SeriesLinkObligationStatusV2,
    SeriesLinkObligationTerminalProjectionV2, SeriesLinkObligationV2,
    SeriesMarketLinkPhaseV2, SeriesMarketLinkV2, SeriesMarketLinkV2Id, SeriesPlanV5Id,
};
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v2, MarketLifecycleRootAccountV2,
    SeriesFundingAccountV3, SeriesLifecycleReplayAccountV2, SeriesMarketLinkAccountV2,
    SeriesRegistryAccountV3, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V2,
    SERIES_FUNDING_ACCOUNT_BYTES_V3, SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V2,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V2, SERIES_REGISTRY_ACCOUNT_BYTES_V3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SERIES_REGISTRY_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-registry-account-authentication/v3\0";
const SERIES_FUNDING_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-funding-account-authentication/v3\0";
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
}
