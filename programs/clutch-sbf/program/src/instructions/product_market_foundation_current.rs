// SPDX-License-Identifier: AGPL-3.0-or-later

//! Acyclic current Product founder authority.
//!
//! The dependency order is part of the protocol, not an implementation detail:
//!
//! 1. hostile-reopen RegistryV3 and **Active** FundingV4 and authenticate the
//!    exact BundleV6/QuoteV5/AttachmentV5/Genesis/FoundationV3 graph;
//! 2. mint this module's non-copy preauthorization for the exact next ordinal
//!    and deterministic future Source coordinates;
//! 3. capitalize `0xba/v2`, whose immutable binding commits that preauthorization;
//! 4. derive the final RootV2 binding and reserve FundingV4 against the exact
//!    acyclic pre-Source reservation binding and hostile Clock receipt;
//! 5. create the Source postwrite, derive LinkV2, and join it with the `0xba`
//!    postwrite here before handing the unique FundingV4 reservation to the
//!    sole concrete RootV2/LinkV2/FundingV4/replayV2 compositor;
//! 6. hostile-reopen RootV2 and only then change `0xba` Founding to Active with
//!    `activated_market_binding_id == MarketLifecycleBindingV2::id()`.
//!
//! FundingV4 breaks the historical FundingV3 cycle: Pending commits an acyclic
//! reservation binding, while completion joins the later Source and LinkV2
//! postwrites. No historical RootV1/FundingV3 capability is accepted here.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;
use crate::source_plane_v3::authenticate_route_clock_bucket;
use clutch_product_series::{
    ComponentDebitV1, ContentId, MarketFamilyAggregatorV1,
    MarketFoundationAccountGraphV3, MarketFoundationCapitalV2, MarketFoundationScheduleV3,
    MarketFoundationSlotV3, MarketInstanceV2Id, MarketLifecycleBindingV2,
    SeriesFundingPhaseV4, SeriesLinkObligationConfigurationV2, SeriesMarketDispositionV1,
    SeriesMarketLinkBindingV2, SeriesMarketLinkV2, SeriesMarketLinkV2Id, SeriesPlanV5Id,
    SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_source_plane_v3_runtime::AuthenticatedSourceRouteV1;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_direct_global_liveness::{
    AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
};
use super::product_series_current::{
    authenticate_series_funding_account_v4, authenticate_series_registry_account_v3,
    AuthenticatedProductSeriesFundingReservationV4, AuthenticatedRegistryCapabilityV4,
    AuthenticatedSeriesFundingAccountV4,
    AuthenticatedSeriesRegistryAccountV3,
};
use super::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV6, AuthenticatedSeriesSourceArtifactsV5,
    AuthenticatedSourceSemanticPublicationV2,
};
use super::source_occurrence_foundation_v1::{
    AuthenticatedPreRootSourceOccurrencePostwriteV3, AuthenticatedPreRootSourceOccurrenceV3,
    AuthenticatedSourceOccurrenceFoundationAuthorityV3, SourceWorkCapitalizationFactsV3,
};

const PRODUCT_CURRENT_FOUNDER_PREAUTH_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-current-founder-preauthorization/v3\0";
const PRODUCT_CURRENT_FOUNDER_CREATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-current-founder-creation/v3\0";

/// Concrete Foundation/Collateral owner consumed by the current Product join.
///
/// This is deliberately default-refusing. It is not a caller DTO: the eventual
/// Collateral composer must retain its physical FoundationVault, Recovery,
/// liability, claim-mint, claim-issuance, General-policy, and rent transcript.
pub(crate) trait AuthenticatedProductMarketFoundationCurrentOwnerV2 {
    fn authentication_id(&self) -> ContentId;
    fn market_instance_id(&self) -> MarketInstanceV2Id;
    fn generation(&self) -> u64;
    fn foundation_vault_account(&self) -> Pubkey;
    fn failure_liveness_policy_account(&self) -> Pubkey;
    fn accepted_market_core_receipt_id(&self) -> ContentId;
    fn general_founding_capability_id(&self) -> ContentId;
    fn market_liability_founding_id(&self) -> ContentId;
    fn claim_mint_founding_plan_id(&self) -> ContentId;
    fn claim_issuance_binding_id(&self) -> ContentId;
    fn product_families(&self) -> MarketFamilyAggregatorV1;
    fn obligation_configuration(&self) -> SeriesLinkObligationConfigurationV2;
    fn founder_link_rent_principal_lamports(&self) -> u64;
    fn founder_link_donation_lamports(&self) -> u64;

    /// Construct the exact final binding only after `0xba` has minted its
    /// immutable bundle ID. General 0x79/V4 does not exist at this boundary.
    fn market_lifecycle_binding_v2(
        &self,
        _direct_global_liveness_binding_id: ContentId,
    ) -> Outcome<MarketLifecycleBindingV2> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }

    fn market_foundation_capital_v2(
        &self,
        _founder_link_id: SeriesMarketLinkV2Id,
    ) -> Outcome<MarketFoundationCapitalV2> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_market_founder_preauthorization_v3(
        &self,
        _program_id: &Pubkey,
        _registry_authentication_id: ContentId,
        _funding_authentication_id: ContentId,
        _funding_state_id: ContentId,
        _compiler_bundle_id: ContentId,
        _source_publication_id: ContentId,
        _clock_receipt_id: ContentId,
        _clock_bucket: u64,
        _foundation_schedule_id: ContentId,
        _foundation_graph_id: ContentId,
        _root_account: Pubkey,
        _link_account: Pubkey,
        _replay_account: Pubkey,
        _direct_global_liveness_account: Pubkey,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_market_founder_completion_v3(
        &self,
        _preauthorization_id: ContentId,
        _direct_global_liveness_binding_id: ContentId,
        _funding_reservation_receipt_id: ContentId,
        _source_receipt_id: ContentId,
        _market_binding_id: ContentId,
        _link_semantic_id: SeriesMarketLinkV2Id,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Acyclic stage-one authority. It commits Active FundingV4 and deterministic
/// future Source coordinates, never Pending Funding or an `0xba` postwrite.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductMarketFounderFoundationPreauthorizationV3 {
    id: ContentId,
    foundation_owner_authentication_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    lifecycle_root_account: Pubkey,
    founder_link_account: Pubkey,
    lifecycle_replay_account: Pubkey,
    direct_global_liveness_account: Pubkey,
    failure_liveness_policy_account: Pubkey,
    foundation_vault_account: Pubkey,
    principal_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    registry_account: Pubkey,
    registry_data_id: ContentId,
    registry_authentication_id: ContentId,
    funding_account: Pubkey,
    funding_data_id: ContentId,
    funding_authentication_id: ContentId,
    funding_state_id: ContentId,
    funding_transition_sequence: u64,
    compiler_bundle_id: ContentId,
    funding_terms_id: ContentId,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    market_genesis_profile_id: ContentId,
    realm_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    product_template_id: ContentId,
    native_claim_basis_id: ContentId,
    recovery_policy_id: ContentId,
    price_measure_policy_id: ContentId,
    candidate_lifecycle_policy_id: ContentId,
    candidate_liveness_policy_id: ContentId,
    failure_liveness_policy_id: ContentId,
    failure_recovery_quote_schedule_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    source_publication_id: ContentId,
    source_occurrence_id: ContentId,
    source_window_id: ContentId,
    statistic_key_id: ContentId,
    source_route_id: ContentId,
    source_release_id: ContentId,
    clock_policy_id: ContentId,
    clock_receipt_id: ContentId,
    clock_slot: u64,
    clock_unix_timestamp: u64,
    clock_bucket: u64,
    source_plane_contract_id: ContentId,
    source_spec_id: ContentId,
    source_work_funding: ComponentDebitV1,
    funding_debits: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
}

impl AuthenticatedProductMarketFounderFoundationPreauthorizationV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.ordinal }
    pub(crate) const fn lifecycle_root_account(&self) -> Pubkey { self.lifecycle_root_account }
    pub(crate) const fn founder_link_account(&self) -> Pubkey { self.founder_link_account }
    pub(crate) const fn lifecycle_replay_account(&self) -> Pubkey {
        self.lifecycle_replay_account
    }
    pub(crate) const fn direct_global_liveness_account(&self) -> Pubkey {
        self.direct_global_liveness_account
    }
    pub(crate) const fn failure_liveness_policy_account(&self) -> Pubkey {
        self.failure_liveness_policy_account
    }
    pub(crate) const fn principal_refund_owner(&self) -> Pubkey { self.principal_refund_owner }
    pub(crate) const fn neutral_lamport_sink(&self) -> Pubkey { self.neutral_lamport_sink }
    pub(crate) const fn candidate_lifecycle_policy_id(&self) -> ContentId {
        self.candidate_lifecycle_policy_id
    }
    pub(crate) const fn candidate_liveness_policy_id(&self) -> ContentId {
        self.candidate_liveness_policy_id
    }
    pub(crate) const fn failure_liveness_policy_id(&self) -> ContentId {
        self.failure_liveness_policy_id
    }
    pub(crate) const fn failure_recovery_quote_schedule_id(&self) -> ContentId {
        self.failure_recovery_quote_schedule_id
    }
    pub(crate) const fn liveness_realm_id(&self) -> ContentId {
        self.realm_id
    }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.funding_account }
    pub(crate) const fn funding_authentication_id(&self) -> ContentId {
        self.funding_authentication_id
    }
    pub(crate) const fn funding_data_id(&self) -> ContentId { self.funding_data_id }
    pub(crate) const fn funding_state_id(&self) -> ContentId { self.funding_state_id }
    pub(crate) const fn funding_transition_sequence(&self) -> u64 {
        self.funding_transition_sequence
    }
    pub(crate) const fn source_occurrence_id(&self) -> ContentId { self.source_occurrence_id }
    pub(crate) const fn compiler_bundle_id(&self) -> ContentId { self.compiler_bundle_id }
    pub(crate) const fn funding_quote_id(&self) -> ContentId { self.funding_quote_id }
    pub(crate) const fn funding_terms_id(&self) -> ContentId { self.funding_terms_id }
    pub(crate) const fn attachment_plan_id(&self) -> ContentId { self.attachment_plan_id }
    pub(crate) const fn source_publication_id(&self) -> ContentId {
        self.source_publication_id
    }
    pub(crate) const fn funding_debits(
        &self,
    ) -> &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2] {
        &self.funding_debits
    }
    pub(crate) const fn clock_policy_id(&self) -> ContentId { self.clock_policy_id }
    pub(crate) const fn clock_receipt_id(&self) -> ContentId { self.clock_receipt_id }
    pub(crate) const fn clock_slot(&self) -> u64 { self.clock_slot }
    pub(crate) const fn clock_unix_timestamp(&self) -> u64 { self.clock_unix_timestamp }
    pub(crate) const fn clock_bucket(&self) -> u64 { self.clock_bucket }
}

impl AuthenticatedSourceOccurrenceFoundationAuthorityV3
    for AuthenticatedProductMarketFounderFoundationPreauthorizationV3
{
    fn authenticate_source_occurrence_foundation_v3(
        &self,
        facts: &SourceWorkCapitalizationFactsV3,
    ) -> Outcome<ContentId> {
        require(
            facts.series_plan_id == self.series_plan_id.content_id()
                && facts.funding_quote_id == self.funding_quote_id
                && facts.ordinal == self.ordinal
                && facts.market_instance_id == self.market_instance_id.content_id()
                && facts.generation == self.generation
                && facts.registry_release_id == self.registry_release_id
                && facts.capability_profile_id == self.capability_profile_id
                && facts.compiler_bundle_id == self.compiler_bundle_id
                && facts.funding_terms_id == self.funding_terms_id
                && facts.attachment_plan_id == self.attachment_plan_id
                && facts.funding_account == self.funding_account
                && facts.funding_state_id != self.funding_state_id
                && facts.funding_account_data_id != self.funding_data_id
                && facts.funding_account_authentication_id != self.funding_authentication_id
                && facts.funding_transition_sequence
                    == self.funding_transition_sequence
                        .checked_add(1).ok_or(ClutchError::Arithmetic)?
                && !facts.funding_reservation_postwrite_id.is_zero()
                && !facts.pending_pre_source_reservation_binding_id.is_zero()
                && !facts.pending_reservation_receipt_id.is_zero()
                && facts.pending_clock_receipt_id == self.clock_receipt_id
                && facts.pending_clock_bucket == self.clock_bucket
                && facts.source_route_id == self.source_route_id
                && facts.source_release_manifest_id == self.source_release_id
                && facts.source_plane_contract_id == self.source_plane_contract_id
                && facts.source_spec_id == self.source_spec_id
                && facts.source_principal_refund.bytes()
                    == self.principal_refund_owner.to_bytes()
                && facts.pending_source_work == self.source_work_funding,
            ClutchError::MismatchedState,
        )?;
        Ok(self.id)
    }
}

/// Final non-copy authority handed to the sole current Product physical writer.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductMarketFounderCurrentCreationV3 {
    id: ContentId,
    preauthorization: AuthenticatedProductMarketFounderFoundationPreauthorizationV3,
    funding_reservation: AuthenticatedProductSeriesFundingReservationV4,
    source: AuthenticatedPreRootSourceOccurrencePostwriteV3,
    direct_capitalization: AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
    market_binding: Box<MarketLifecycleBindingV2>,
    founder_link_binding: Box<SeriesMarketLinkBindingV2>,
    foundation_capital: Box<MarketFoundationCapitalV2>,
    product_families: Box<MarketFamilyAggregatorV1>,
    obligation_configuration: SeriesLinkObligationConfigurationV2,
    founder_link_semantic_id: SeriesMarketLinkV2Id,
    accepted_market_core_receipt_id: ContentId,
}

impl AuthenticatedProductMarketFounderCurrentCreationV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn preauthorization_id(&self) -> ContentId { self.preauthorization.id }
    pub(crate) const fn preauthorization(
        &self,
    ) -> &AuthenticatedProductMarketFounderFoundationPreauthorizationV3 {
        &self.preauthorization
    }
    pub(crate) fn market_binding(&self) -> &MarketLifecycleBindingV2 { &self.market_binding }
    pub(crate) fn founder_link_binding(&self) -> &SeriesMarketLinkBindingV2 {
        &self.founder_link_binding
    }
    pub(crate) fn foundation_capital(&self) -> &MarketFoundationCapitalV2 {
        &self.foundation_capital
    }
    pub(crate) fn product_families(&self) -> &MarketFamilyAggregatorV1 {
        &self.product_families
    }
    pub(crate) const fn obligation_configuration(
        &self,
    ) -> SeriesLinkObligationConfigurationV2 {
        self.obligation_configuration
    }
    pub(crate) const fn founder_link_semantic_id(&self) -> SeriesMarketLinkV2Id {
        self.founder_link_semantic_id
    }
    pub(crate) const fn accepted_market_core_receipt_id(&self) -> ContentId {
        self.accepted_market_core_receipt_id
    }
    pub(crate) const fn direct_global_liveness_binding_id(&self) -> ContentId {
        self.direct_capitalization.global_bundle_binding_id()
    }
    pub(crate) const fn direct_global_liveness_account_data_id(&self) -> ContentId {
        self.direct_capitalization.account_data_id()
    }
    pub(crate) const fn direct_global_liveness_account_authentication_id(&self) -> ContentId {
        self.direct_capitalization.account_authentication_id()
    }
    pub(crate) const fn source_receipt_id(&self) -> ContentId { self.source.id() }
    pub(crate) const fn funding_reservation(
        &self,
    ) -> &AuthenticatedProductSeriesFundingReservationV4 {
        &self.funding_reservation
    }
    pub(crate) const fn source_postwrite(
        &self,
    ) -> &AuthenticatedPreRootSourceOccurrencePostwriteV3 {
        &self.source
    }

    pub(super) fn into_direct_activation_parts(
        self,
    ) -> (
        ContentId,
        Pubkey,
        ContentId,
        AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
    ) {
        (
            self.id,
            self.preauthorization.lifecycle_root_account,
            self.preauthorization.id,
            self.direct_capitalization,
        )
    }
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

fn require_distinct_pubkeys(accounts: &[Pubkey]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        require(accounts[left] != Pubkey::default(), ClutchError::MismatchedState)?;
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left] != accounts[right], ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Mint the acyclic current preauthorization from a hostile-reopened Active
/// FundingV4 account and one exact deterministic Source publication.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn authenticate_product_market_founder_foundation_preauthorization_v3<O>(
    program_id: &Pubkey,
    owner: &O,
    registry: AuthenticatedSeriesRegistryAccountV3,
    registry_account: &AccountInfo<'_>,
    capability: &AuthenticatedRegistryCapabilityV4,
    funding: AuthenticatedSeriesFundingAccountV4,
    funding_account: &AccountInfo<'_>,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    publication: AuthenticatedSourceSemanticPublicationV2,
    source_route: AuthenticatedSourceRouteV1,
    clock_account: &AccountInfo<'_>,
    schedule: &MarketFoundationScheduleV3,
    graph: &MarketFoundationAccountGraphV3,
    lifecycle_root_account: Pubkey,
    founder_link_account: Pubkey,
    lifecycle_replay_account: Pubkey,
    direct_global_liveness_account: Pubkey,
    failure_liveness_policy_account: Pubkey,
    foundation_vault_account: Pubkey,
    principal_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
) -> Outcome<AuthenticatedProductMarketFounderFoundationPreauthorizationV3>
where
    O: AuthenticatedProductMarketFoundationCurrentOwnerV2 + ?Sized,
{
    let series = artifacts.series();
    let funding_terms = artifacts.funding_terms();
    let quote = artifacts.quote();
    let attachment = artifacts.attachment();
    let genesis = artifacts.genesis();
    artifacts.validate_registry_projection(&capability.projection())?;
    let series_id = series.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = funding_terms.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let quote_id = quote.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = attachment.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let genesis_id = genesis.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = schedule.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let graph_id = graph.id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let state = funding.state();
    let funding_state_id = state.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let occurrence = publication.occurrence();
    let occurrence_id = occurrence.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let route = publication.route();
    let clock = authenticate_route_clock_bucket(source_route, clock_account)
        .map_err(Refusal::from)?;
    let clock_snapshot = clock.snapshot();

    let live_registry = authenticate_series_registry_account_v3(
        program_id, registry_account, series_id, false)?;
    let live_funding = authenticate_series_funding_account_v4(
        program_id, funding_account, series_id, true)?;
    require(
        live_registry == registry
            && live_funding.account() == funding.account()
            && live_funding.value() == funding.value()
            && live_funding.observed_lamports() == funding.observed_lamports()
            && live_funding.is_writable() == funding.is_writable()
            && live_funding.data_id() == funding.data_id()
            && live_funding.authentication_id() == funding.authentication_id()
            && registry.value().activation_consumed
            && state.phase == SeriesFundingPhaseV4::Active
            && state.next_ordinal < state.instance_count
            && state.pending_market_instance_id == ContentId::ZERO
            && state.pending_source_occurrence_id == ContentId::ZERO
            && state.pending_pre_source_reservation_binding_id == ContentId::ZERO
            && state.pending_reservation_receipt_id == ContentId::ZERO
            && state.pending_clock_receipt_id == ContentId::ZERO
            && state.pending_clock_bucket == 0
            && state.pending_debits
                == [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2]
            && state.series_plan_id == series_id
            && state.funding_terms_id == funding_terms_id
            && state.funding_quote_id == quote_id
            && state.attachment_plan_id == attachment_id
            && state.compiler_bundle_id == bundle.bundle_id()
            && registry.value().funding_terms_id == funding_terms_id
            && registry.value().compiler_bundle_id == bundle.bundle_id()
            && capability.series_registry_account() == registry.account()
            && capability.series_registry_authentication_id() == registry.authentication_id()
            && capability.compiler_bundle_id() == bundle.bundle_id()
            && bundle.bundle().series_plan_id == series_id
            && bundle.bundle().funding_terms_id == funding_terms_id
            && bundle.bundle().funding_quote_id == quote_id
            && bundle.bundle().attachment_plan_id == attachment_id
            && bundle.bundle().market_genesis_profile_id == genesis_id
            && quote.foundation == *schedule
            && occurrence.series_plan_id == series_id
            && occurrence.ordinal == state.next_ordinal
            && occurrence.market_instance_id == owner.market_instance_id()
            && occurrence.attachment_plan_id.bytes() == attachment_id.bytes()
            && publication.source_work_funding()
                == quote.components[clutch_product_series::SeriesFundingComponentV2::SourceWork.index()]
            && route.compiler_bundle_id() == bundle.bundle_id()
            && route.registry_release_id() == capability.registry_release_id()
            && route.capability_profile_id() == capability.capability_profile_id()
            && route.source_release_manifest_id() == bundle.bundle().source_release_manifest_id
            && route.source_route_id().bytes() == source_route.route_id().bytes()
            && route.source_release_manifest_id().bytes()
                == source_route.release_manifest_id().bytes()
            && route.source_release_authentication_id().bytes()
                == source_route.release_authentication_id().bytes()
            && route.clock_policy_id().bytes() == source_route.clock_policy_id().bytes()
            && clock.policy_id().bytes() == route.clock_policy_id().bytes()
            && series
                .is_creation_eligible(state.next_ordinal, clock.bucket())
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && route.source_plane_contract_id() == bundle.bundle().source_plane_contract_id
            && route.source_spec_id() == bundle.bundle().source_spec_id
            && graph.market_instance_id == owner.market_instance_id()
            && graph.generation == owner.generation()
            && graph.account(MarketFoundationSlotV3::LifecycleRoot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes() == lifecycle_root_account.to_bytes()
            && owner.foundation_vault_account() == foundation_vault_account
            && owner.failure_liveness_policy_account() == failure_liveness_policy_account
            && funding_terms.lamport_principal_refund.bytes() == principal_refund_owner.to_bytes()
            && funding_terms.neutral_lamport_sink.bytes() == neutral_lamport_sink.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let market = owner.market_instance_id().bytes();
    require(
        seeds::product_market_lifecycle_root_pda(program_id, &market, owner.generation()).0
            == lifecycle_root_account
            && seeds::product_series_market_link_pda(
                program_id, &series_id.bytes(), state.next_ordinal).0 == founder_link_account
            && seeds::product_series_lifecycle_replay_pda(program_id, &series_id.bytes()).0
                == lifecycle_replay_account
            && seeds::product_direct_global_liveness_pda(
                program_id, &market, owner.generation()).0 == direct_global_liveness_account
            && seeds::product_market_foundation_vault_pda(
                program_id, &market, owner.generation()).0 == foundation_vault_account,
        ClutchError::WrongPda,
    )?;
    require_distinct_pubkeys(&[
        *registry_account.key, *funding_account.key, lifecycle_root_account,
        founder_link_account, lifecycle_replay_account, direct_global_liveness_account,
        failure_liveness_policy_account, foundation_vault_account, principal_refund_owner,
        neutral_lamport_sink,
    ])?;
    for account in [lifecycle_root_account, founder_link_account, direct_global_liveness_account] {
        require(account != SYSTEM_PROGRAM_ID, ClutchError::AccountAlias)?;
    }

    let foundation_owner_authentication_id = owner.authentication_id();
    let compiler_bundle_id = bundle.bundle_id().content_id();
    owner.authenticate_product_market_founder_preauthorization_v3(
        program_id, registry.authentication_id(), funding.authentication_id(), funding_state_id,
        compiler_bundle_id, publication.id(), ContentId::from_bytes(clock.id().bytes()),
        clock.bucket(), schedule_id.content_id(), graph_id.content_id(),
        lifecycle_root_account, founder_link_account, lifecycle_replay_account,
        direct_global_liveness_account,
    )?;
    for id in [
        foundation_owner_authentication_id, owner.accepted_market_core_receipt_id(),
        owner.general_founding_capability_id(), owner.market_liability_founding_id(),
        owner.claim_mint_founding_plan_id(), owner.claim_issuance_binding_id(),
    ] {
        require_live(id)?;
    }

    let id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[
        PRODUCT_CURRENT_FOUNDER_PREAUTH_DOMAIN_V3,
        program_id.as_ref(),
        &foundation_owner_authentication_id.bytes(),
        &owner.market_instance_id().bytes(),
        &owner.generation().to_le_bytes(),
        &series_id.bytes(),
        &state.next_ordinal.to_le_bytes(),
        registry_account.key.as_ref(),
        &registry.data_id().bytes(),
        &registry.authentication_id().bytes(),
        funding_account.key.as_ref(),
        &funding.data_id().bytes(),
        &funding.authentication_id().bytes(),
        &funding_state_id.bytes(),
        &state.transition_sequence.to_le_bytes(),
        &compiler_bundle_id.bytes(),
        &quote_id.bytes(),
        &attachment_id.bytes(),
        &genesis_id.bytes(),
        &publication.id().bytes(),
        &clock.id().bytes(),
        &clock_snapshot.slot.to_le_bytes(),
        &clock_snapshot.unix_timestamp.to_le_bytes(),
        &clock.bucket().to_le_bytes(),
        &occurrence_id.bytes(),
        &occurrence.source_window_id.bytes(),
        &occurrence.statistic_key_id.bytes(),
        &route.id().bytes(),
        &route.clock_policy_id().bytes(),
        &schedule_id.bytes(),
        &graph_id.bytes(),
        lifecycle_root_account.as_ref(), founder_link_account.as_ref(),
        lifecycle_replay_account.as_ref(), direct_global_liveness_account.as_ref(),
        failure_liveness_policy_account.as_ref(), foundation_vault_account.as_ref(),
        principal_refund_owner.as_ref(), neutral_lamport_sink.as_ref(),
        &owner.general_founding_capability_id().bytes(),
        &owner.market_liability_founding_id().bytes(),
        &owner.claim_mint_founding_plan_id().bytes(),
        &owner.claim_issuance_binding_id().bytes(),
    ]).to_bytes());
    require_live(id)?;
    Ok(AuthenticatedProductMarketFounderFoundationPreauthorizationV3 {
        id, foundation_owner_authentication_id,
        market_instance_id: owner.market_instance_id(), generation: owner.generation(),
        series_plan_id: series_id, ordinal: state.next_ordinal,
        lifecycle_root_account, founder_link_account, lifecycle_replay_account,
        direct_global_liveness_account, failure_liveness_policy_account,
        foundation_vault_account, principal_refund_owner, neutral_lamport_sink,
        registry_account: *registry_account.key, registry_data_id: registry.data_id(),
        registry_authentication_id: registry.authentication_id(),
        funding_account: *funding_account.key, funding_data_id: funding.data_id(),
        funding_authentication_id: funding.authentication_id(), funding_state_id,
        funding_transition_sequence: state.transition_sequence, compiler_bundle_id,
        funding_terms_id: funding_terms_id.content_id(),
        funding_quote_id: quote_id.content_id(), attachment_plan_id: attachment_id.content_id(),
        market_genesis_profile_id: genesis_id.content_id(), realm_id: genesis.realm_id,
        registry_release_id: capability.registry_release_id(),
        capability_profile_id: capability.capability_profile_id(),
        product_template_id: bundle.bundle().product_template_id.content_id(),
        native_claim_basis_id: bundle.bundle().native_claim_basis_id.content_id(),
        recovery_policy_id: bundle.bundle().evidence_only_recovery_policy_id.content_id(),
        price_measure_policy_id: bundle.bundle().price_measure_policy_id.content_id(),
        candidate_lifecycle_policy_id: genesis.candidate_lifecycle_policy_id,
        candidate_liveness_policy_id: genesis.candidate_liveness_policy_id,
        failure_liveness_policy_id: quote.failure_liveness_policy_id,
        failure_recovery_quote_schedule_id: quote.failure_recovery_quote_schedule_id,
        foundation_schedule_id: schedule_id.content_id(), foundation_graph_id: graph_id.content_id(),
        source_publication_id: publication.id(), source_occurrence_id: occurrence_id.content_id(),
        source_window_id: occurrence.source_window_id, statistic_key_id: occurrence.statistic_key_id,
        source_route_id: route.source_route_id(), source_release_id: route.source_release_manifest_id(),
        clock_policy_id: route.clock_policy_id(),
        clock_receipt_id: ContentId::from_bytes(clock.id().bytes()),
        clock_slot: clock_snapshot.slot,
        clock_unix_timestamp: clock_snapshot.unix_timestamp,
        clock_bucket: clock.bucket(),
        source_plane_contract_id: route.source_plane_contract_id(), source_spec_id: route.source_spec_id(),
        source_work_funding: publication.source_work_funding(),
        funding_debits: quote.components,
    })
}

/// Join the exact Pending successor, Source postwrite, and `0xba` postwrite into
/// the sole current creation authority. The Funding reservation itself must
/// have occurred earlier in the same instruction from this preauthorization.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn complete_product_market_founder_current_creation_v3<O>(
    owner: &O,
    preauthorization: AuthenticatedProductMarketFounderFoundationPreauthorizationV3,
    source: AuthenticatedPreRootSourceOccurrenceV3,
    direct_capitalization: AuthenticatedProductDirectGlobalLivenessCapitalizationV2,
) -> Outcome<AuthenticatedProductMarketFounderCurrentCreationV3>
where
    O: AuthenticatedProductMarketFoundationCurrentOwnerV2 + ?Sized,
{
    let source_id = source.id();
    let source_preauthorization_id = source.product_preauthorization_id();
    let occurrence = source.occurrence();
    let (funding_reservation, source) = source.into_product_founder_parts();
    let funding = funding_reservation.pending();
    let state = funding.state();
    let reservation_binding = funding_reservation.binding();
    let reservation_binding_id = reservation_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_facts = source.capitalization().facts();
    let funding_state_pending_id = funding.state().id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        owner.authentication_id() == preauthorization.foundation_owner_authentication_id
            && funding.account() == preauthorization.funding_account
            && state.phase == SeriesFundingPhaseV4::Pending
            && state.series_plan_id == preauthorization.series_plan_id
            && state.pending_ordinal == preauthorization.ordinal
            && state.pending_market_instance_id == preauthorization.market_instance_id.content_id()
            && state.pending_source_occurrence_id == preauthorization.source_occurrence_id
            && state.pending_disposition == Some(SeriesMarketDispositionV1::Founder)
            && state.transition_sequence
                == preauthorization.funding_transition_sequence
                    .checked_add(1).ok_or(ClutchError::Arithmetic)?
            && state.pending_pre_source_reservation_binding_id
                == reservation_binding_id.content_id()
            && state.pending_clock_receipt_id == preauthorization.clock_receipt_id
            && state.pending_clock_bucket == preauthorization.clock_bucket
            && state.pending_debits == preauthorization.funding_debits
            && funding_reservation.funding_state_before_id().content_id()
                == preauthorization.funding_state_id
            && funding_reservation.funding_data_before_id()
                == preauthorization.funding_data_id
            && funding_reservation.funding_authentication_before_id()
                == preauthorization.funding_authentication_id
            && source_preauthorization_id == preauthorization.id
            && source.product_preauthorization_id() == preauthorization.id
            && source_facts.funding_reservation_postwrite_id == funding_reservation.id()
            && source_facts.series_plan_id == preauthorization.series_plan_id.content_id()
            && source_facts.funding_terms_id == preauthorization.funding_terms_id
            && source_facts.funding_quote_id == preauthorization.funding_quote_id
            && source_facts.attachment_plan_id == preauthorization.attachment_plan_id
            && source_facts.compiler_bundle_id == preauthorization.compiler_bundle_id
            && source_facts.registry_release_id == preauthorization.registry_release_id
            && source_facts.capability_profile_id == preauthorization.capability_profile_id
            && source_facts.funding_account == preauthorization.funding_account
            && source_facts.funding_state_id == funding_state_pending_id.content_id()
            && source_facts.funding_account_data_id == funding.data_id()
            && source_facts.funding_account_authentication_id == funding.authentication_id()
            && source_facts.funding_transition_sequence == state.transition_sequence
            && source_facts.pending_pre_source_reservation_binding_id
                == reservation_binding_id.content_id()
            && source_facts.pending_reservation_receipt_id
                == funding_reservation.reservation_receipt_id()
            && source_facts.pending_clock_receipt_id == preauthorization.clock_receipt_id
            && source_facts.pending_clock_bucket == preauthorization.clock_bucket
            && source_facts.pending_source_work == preauthorization.source_work_funding
            && source_facts.source_route_id == preauthorization.source_route_id
            && source_facts.source_release_manifest_id == preauthorization.source_release_id
            && source_facts.source_plane_contract_id == preauthorization.source_plane_contract_id
            && source_facts.source_spec_id == preauthorization.source_spec_id
            && source_facts.source_principal_refund.bytes()
                == preauthorization.principal_refund_owner.to_bytes()
            && occurrence.occurrence_record_id() == preauthorization.source_occurrence_id
            && occurrence.series_plan_id() == preauthorization.series_plan_id.content_id()
            && occurrence.ordinal() == preauthorization.ordinal
            && occurrence.market_instance_id() == preauthorization.market_instance_id.content_id()
            && occurrence.attachment_plan_id() == preauthorization.attachment_plan_id
            && occurrence.route_id() == preauthorization.source_route_id
            && occurrence.clock_policy_id() == preauthorization.clock_policy_id
            && occurrence.source_plane_contract_id() == preauthorization.source_plane_contract_id
            && occurrence.source_spec_id() == preauthorization.source_spec_id
            && occurrence.window_id() == preauthorization.source_window_id
            && occurrence.statistic_key_id() == preauthorization.statistic_key_id
            && direct_capitalization.global_bundle_binding_id() != ContentId::ZERO
            && reservation_binding.funding_account_id.bytes()
                == preauthorization.funding_account.to_bytes()
            && reservation_binding.funding_account_authentication_before_id
                == preauthorization.funding_authentication_id
            && reservation_binding.funding_state_before_id.content_id()
                == preauthorization.funding_state_id
            && reservation_binding.series_plan_id == preauthorization.series_plan_id
            && reservation_binding.funding_terms_id.content_id()
                == preauthorization.funding_terms_id
            && reservation_binding.funding_quote_id.content_id()
                == preauthorization.funding_quote_id
            && reservation_binding.attachment_plan_id.content_id()
                == preauthorization.attachment_plan_id
            && reservation_binding.compiler_bundle_id.content_id()
                == preauthorization.compiler_bundle_id
            && reservation_binding.ordinal == preauthorization.ordinal
            && reservation_binding.market_instance_id == preauthorization.market_instance_id
            && reservation_binding.source_occurrence_id.content_id()
                == preauthorization.source_occurrence_id
            && reservation_binding.disposition == SeriesMarketDispositionV1::Founder
            && reservation_binding.debits == preauthorization.funding_debits
            && reservation_binding.market_root_account_id.bytes()
                == preauthorization.lifecycle_root_account.to_bytes()
            && reservation_binding.series_market_link_account_id.bytes()
                == preauthorization.founder_link_account.to_bytes()
            && reservation_binding.product_founder_preauthorization_id == preauthorization.id
            && reservation_binding.direct_global_liveness_capitalization_id
                == direct_capitalization.global_capitalization_receipt_id()
            && reservation_binding.source_publication_id == preauthorization.source_publication_id
            && reservation_binding.clock_policy_id == preauthorization.clock_policy_id
            && reservation_binding.clock_receipt_id == preauthorization.clock_receipt_id
            && reservation_binding.funding_transition_sequence_before
                == preauthorization.funding_transition_sequence
            && reservation_binding.clock_slot == preauthorization.clock_slot
            && reservation_binding.clock_unix_timestamp == preauthorization.clock_unix_timestamp
            && reservation_binding.clock_bucket == preauthorization.clock_bucket,
        ClutchError::MismatchedState,
    )?;

    let market_binding = owner.market_lifecycle_binding_v2(
        direct_capitalization.global_bundle_binding_id())?;
    let market_binding_id = market_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        reservation_binding.market_binding_id == market_binding_id
            && market_binding.market_instance_id == preauthorization.market_instance_id
            && market_binding.generation == preauthorization.generation
            && market_binding.outcome_count != 0
            && market_binding.product_template_id == preauthorization.product_template_id
            && market_binding.native_claim_basis_id == preauthorization.native_claim_basis_id
            && market_binding.recovery_policy_id == preauthorization.recovery_policy_id
            && market_binding.price_measure_policy_id == preauthorization.price_measure_policy_id
            && market_binding.market_genesis_profile_id
                == preauthorization.market_genesis_profile_id
            && market_binding.registry_release_id == preauthorization.registry_release_id
            && market_binding.capability_profile_id == preauthorization.capability_profile_id
            && market_binding.realm_id == preauthorization.realm_id
            && market_binding.source_release_id == preauthorization.source_release_id
            && market_binding.source_route_id == preauthorization.source_route_id
            && market_binding.clock_policy_id == preauthorization.clock_policy_id
            && market_binding.source_plane_contract_id
                == preauthorization.source_plane_contract_id
            && market_binding.source_spec_id == preauthorization.source_spec_id
            && market_binding.primary_window_id == preauthorization.source_window_id
            && market_binding.statistic_key_id == preauthorization.statistic_key_id
            && market_binding.foundation_schedule_id.content_id()
                == preauthorization.foundation_schedule_id
            && market_binding.foundation_account_graph_id.content_id()
                == preauthorization.foundation_graph_id
            && market_binding.direct_global_liveness_binding_id
                == direct_capitalization.global_bundle_binding_id()
            && market_binding.failure_liveness_policy_id
                == preauthorization.failure_liveness_policy_id
            && market_binding.failure_liveness_quote_schedule_id
                == preauthorization.failure_recovery_quote_schedule_id
            && market_binding.foundation_vault_id.bytes()
                == preauthorization.foundation_vault_account.to_bytes()
            && market_binding.general_founding_capability_id
                == owner.general_founding_capability_id()
            && market_binding.market_liability_founding_id
                == owner.market_liability_founding_id()
            && market_binding.claim_mint_founding_plan_id
                == owner.claim_mint_founding_plan_id()
            && market_binding.claim_issuance_binding_id
                == owner.claim_issuance_binding_id(),
        ClutchError::MismatchedState,
    )?;
    let obligation_configuration = owner.obligation_configuration();
    let founder_link_binding = SeriesMarketLinkBindingV2 {
        series_plan_id: preauthorization.series_plan_id,
        ordinal: preauthorization.ordinal,
        market_instance_id: preauthorization.market_instance_id,
        market_root_account_id: ContentId::from_bytes(
            preauthorization.lifecycle_root_account.to_bytes()),
        market_binding_id,
        disposition: SeriesMarketDispositionV1::Founder,
        funding_terms_id: state.funding_terms_id,
        funding_quote_id: state.funding_quote_id,
        attachment_plan_id: state.attachment_plan_id,
        capability_profile_id: market_binding.capability_profile_id,
        obligation_configuration_id: obligation_configuration.id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        compiler_bundle_id: state.compiler_bundle_id,
        source_occurrence_id: clutch_product_series::SourceOccurrenceV1Id::from_bytes(
            occurrence.occurrence_record_id().bytes()),
        source_occurrence_account_id: ContentId::from_bytes(occurrence.occurrence_account().bytes()),
        source_occurrence_account_authentication_id:
            occurrence.occurrence_account_authentication_id(),
        source_occurrence_receipt_id: source.id(),
        source_release_id: preauthorization.source_release_id,
        source_route_id: occurrence.route_id(),
        clock_policy_id: occurrence.clock_policy_id(),
        source_plane_contract_id: occurrence.source_plane_contract_id(),
        source_spec_id: occurrence.source_spec_id(),
        window_spec_id: occurrence.window_id(),
        statistic_key_id: occurrence.statistic_key_id(),
        funding_state_account_id: ContentId::from_bytes(funding.account().to_bytes()),
        funding_debit_receipt_id: state.pending_reservation_receipt_id,
        rent_refund_owner: ContentId::from_bytes(preauthorization.principal_refund_owner.to_bytes()),
        neutral_lamport_sink: ContentId::from_bytes(preauthorization.neutral_lamport_sink.to_bytes()),
        generation: preauthorization.generation,
        source_repair_generation: occurrence.repair_generation(),
        funding_transition_sequence: state.transition_sequence,
    };
    let pending_link = SeriesMarketLinkV2::initialize_pending(
        founder_link_binding, obligation_configuration,
        owner.founder_link_rent_principal_lamports(),
        owner.founder_link_donation_lamports(),
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        owner.founder_link_rent_principal_lamports()
            == state.pending_debits[
                clutch_product_series::SeriesFundingComponentV2::SeriesAdmission.index()
            ].lamports,
        ClutchError::MismatchedState,
    )?;
    let founder_link_semantic_id = pending_link.semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        ContentId::from_bytes(preauthorization.founder_link_account.to_bytes())
            == reservation_binding.series_market_link_account_id
            && founder_link_binding.market_binding_id == reservation_binding.market_binding_id
            && founder_link_binding.source_occurrence_id
                == reservation_binding.source_occurrence_id
            && founder_link_binding.funding_transition_sequence == state.transition_sequence,
        ClutchError::MismatchedState,
    )?;
    let foundation_capital = owner.market_foundation_capital_v2(founder_link_semantic_id)?;
    let accepted_market_core_receipt_id = owner.accepted_market_core_receipt_id();
    let product_families = owner.product_families();
    owner.authenticate_product_market_founder_completion_v3(
        preauthorization.id, direct_capitalization.global_bundle_binding_id(),
        state.pending_reservation_receipt_id, source_id, market_binding_id,
        founder_link_semantic_id,
    )?;
    let id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[
        PRODUCT_CURRENT_FOUNDER_CREATION_DOMAIN_V3,
        &preauthorization.id.bytes(),
        &direct_capitalization.account_authentication_id().bytes(),
        &direct_capitalization.global_bundle_binding_id().bytes(),
        &funding.authentication_id().bytes(),
        &funding_reservation.id().bytes(),
        &reservation_binding_id.bytes(),
        &state.pending_reservation_receipt_id.bytes(),
        &source_id.bytes(),
        &market_binding_id.bytes(),
        &founder_link_semantic_id.bytes(),
        &accepted_market_core_receipt_id.bytes(),
        &owner.authentication_id().bytes(),
    ]).to_bytes());
    require_live(id)?;
    Ok(AuthenticatedProductMarketFounderCurrentCreationV3 {
        id, preauthorization, funding_reservation, source, direct_capitalization,
        market_binding: Box::new(market_binding),
        founder_link_binding: Box::new(founder_link_binding),
        foundation_capital: Box::new(foundation_capital),
        product_families: Box::new(product_families),
        obligation_configuration, founder_link_semantic_id,
        accepted_market_core_receipt_id,
    })
}
