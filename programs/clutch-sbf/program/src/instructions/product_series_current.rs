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
use crate::seeds;
use clutch_product_series::{
    authenticate_market_foundation_account_graph_bytes_v3,
    AuthenticatedMarketFoundationAccountGraphBytesV3, ContentId, FixedCodec,
    MarketFoundationScheduleV3, MarketFoundationSlotV3, MarketInstanceV2Id,
    MarketLifecyclePhaseV2, MarketLifecycleRootV2, SeriesFundingStateV3,
    SeriesLifecycleReplayBindingV2Id, SeriesLifecycleReplayV2, SeriesMarketLinkPhaseV2,
    SeriesMarketLinkV2, SeriesPlanV5Id,
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
