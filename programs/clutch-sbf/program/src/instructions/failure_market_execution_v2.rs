// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared hostile authentication for current callable Market Failure actions.
//!
//! This owner rebuilds the mutable Product root/link, current loader-backed
//! Registry graph, market liveness schedule, immutable Failure admission,
//! mutable runtime, and durable interval capitalization on every call. It
//! accepts only content preimages whose IDs are already committed by those
//! program-owned accounts; no caller ID becomes authority.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v3, AuthenticatedFailureMarketRootV3,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_recovery_quote_v1,
    reopen_failure_market_interval_accounts_v2, AuthenticatedFailureMarketIntervalAccountsV2,
    FailureMarketIntervalFundingPreimageV2,
};
use crate::instructions::failure_market_runtime::{
    authenticate_failure_market_runtime_root_v1, AuthenticatedFailureMarketRuntimeRootV1,
};
use crate::instructions::failure_market_replay_v2::{
    reopen_failure_market_replay_v2, AuthenticatedFailureMarketReplayV2,
    FailureMarketReplayFundingPreimageV2,
};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, authenticate_registry_capability_v3,
    authenticate_series_registry_capability_refs_v2, AuthenticatedProductArtifactV1,
    AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::product_market::{
    authenticate_market_foundation_preallocation_v2, authenticate_market_lifecycle_root_v1,
    authenticate_market_recovery_schedule_v1, authenticate_series_market_link_v1,
    AuthenticatedMarketFoundationPreallocationV2, AuthenticatedMarketLifecycleRootV1,
    AuthenticatedSeriesMarketLinkV1,
};
use crate::instructions::product_series::{
    authenticate_source_product_route_v3, AuthenticatedSourceProductRouteV3,
};
use crate::source_plane_v3::{authenticate_receiver_route, authenticate_route};
use crate::source_plane_v3_actions::authenticate_source_work_schedule_artifact;
use clutch_failure_policy_runtime::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use clutch_product_series::{
    CompiledProductSeriesBundleV5, ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2,
    MarketFoundationAccountGraphV2, NativeClaimBasisV1, PriceMeasurePolicyV1, ProductTemplateV4,
    QuantizedIntervalConsensusContextV1, QuantizedIntervalConsensusProfileV1,
    SeriesFundingQuoteV4, MARKET_FOUNDATION_SLOT_COUNT_V2,
};
use clutch_source_plane_v3::{
    CompiledSourceOccurrenceV3, StatisticKeyV3, StatisticResultV3, SummaryProgramV3, WindowSealV3,
    WindowSpecV3,
};
use clutch_source_plane_v3_runtime::{AuthenticatedSourceRouteV1, SourceWorkScheduleBindingV1};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Complete current Product/Failure authentication reused by actions 10-13.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketExecutionV2<'root, 'link> {
    root: AuthenticatedMarketLifecycleRootV1<'root>,
    link: AuthenticatedSeriesMarketLinkV1<'link>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    funding_quote: AuthenticatedProductArtifactV1<SeriesFundingQuoteV4>,
    admission: AuthenticatedFailureMarketRootV3,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
}

impl<'root, 'link> AuthenticatedFailureMarketExecutionV2<'root, 'link> {
    pub(crate) const fn root(&self) -> AuthenticatedMarketLifecycleRootV1<'root> {
        self.root
    }
    pub(crate) const fn link(&self) -> AuthenticatedSeriesMarketLinkV1<'link> {
        self.link
    }
    pub(crate) const fn registry(&self) -> AuthenticatedRegistryCapabilityV3 {
        self.registry
    }
    pub(crate) const fn bundle(
        &self,
    ) -> &AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5> {
        &self.bundle
    }
    pub(crate) fn into_bundle(
        self,
    ) -> AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5> {
        self.bundle
    }
    pub(crate) const fn funding_quote(
        &self,
    ) -> &AuthenticatedProductArtifactV1<SeriesFundingQuoteV4> {
        &self.funding_quote
    }
    pub(crate) const fn admission(&self) -> AuthenticatedFailureMarketRootV3 {
        self.admission
    }
    pub(crate) const fn runtime(&self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.runtime
    }
    pub(crate) const fn interval(&self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.interval
    }
    pub(crate) const fn quote(&self) -> FailureMarketRecoveryQuoteAdmissionReceiptV1 {
        self.quote
    }

    /// Bind the wire replay sequence to the next exact mutable Failure-runtime
    /// transition. State-specific owners still enforce their finer ordinals.
    pub(crate) fn require_next_sequence(&self, sequence: u64) -> Outcome<()> {
        let expected = self
            .runtime
            .state()
            .transition_sequence()
            .checked_add(1)
            .ok_or(ClutchError::Arithmetic)?;
        require(sequence == expected, ClutchError::Replay)
    }
}

/// Exact current registered Source route and immutable paid-work schedule.
///
/// Both values are reconstructed from program-owned accounts and joined to
/// Product's live Market/link and Failure's immutable policy. The physical
/// account tuple, rather than a caller-supplied route ID, is the authority
/// consumed by action10 through action12.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketSourceRouteV2 {
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
}

impl AuthenticatedFailureMarketSourceRouteV2 {
    pub(crate) const fn route(self) -> AuthenticatedSourceRouteV1 {
        self.route
    }

    pub(crate) const fn schedule(self) -> SourceWorkScheduleBindingV1 {
        self.schedule
    }
}

/// Hostile-authenticate the complete Source route prefix used by current
/// Failure actions and join it to every Market-scoped owner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_source_route_v2(
    program_id: &Pubkey,
    execution: &AuthenticatedFailureMarketExecutionV2<'_, '_>,
    source_release_account: &AccountInfo<'_>,
    adapter_program: &AccountInfo<'_>,
    adapter_programdata: &AccountInfo<'_>,
    parser_program: &AccountInfo<'_>,
    parser_programdata: &AccountInfo<'_>,
    parser_config: &AccountInfo<'_>,
    source_spec_account: &AccountInfo<'_>,
    source_work_schedule_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedFailureMarketSourceRouteV2> {
    let route = authenticate_route(
        program_id,
        source_release_account,
        adapter_program,
        adapter_programdata,
        parser_program,
        parser_programdata,
        parser_config,
        source_spec_account,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule = authenticate_source_work_schedule_artifact(
        program_id,
        route,
        source_work_schedule_account,
    )?;
    let policy = execution.admission().state().binding().facts();
    let root = execution.root().state().binding();
    let link = execution.link().state().binding();
    let bundle = execution.bundle().value();
    require(
        route.release_account().bytes() == source_release_account.key.to_bytes()
            && route.release_manifest_id().bytes() == policy.source_release_manifest_id.bytes()
            && route.release_manifest_id().bytes() == root.source_release_id.bytes()
            && route.release_manifest_id().bytes() == bundle.source_release_manifest_id.bytes()
            && route.release_authentication_id().bytes()
                == policy.source_release_authentication_id.bytes()
            && route.route_id().bytes() == root.source_route_id.bytes()
            && route.route_id().bytes() == link.source_route_id.bytes()
            && route.source_plane_contract_id().bytes()
                == policy.source_plane_contract_id.bytes()
            && route.source_plane_contract_id().bytes() == root.source_plane_contract_id.bytes()
            && route.source_plane_contract_id().bytes() == link.source_plane_contract_id.bytes()
            && route.source_plane_contract_id().bytes()
                == bundle.source_plane_contract_id.bytes()
            && route.source_spec_id().bytes() == policy.source_spec_id.bytes()
            && route.source_spec_id().bytes() == root.source_spec_id.bytes()
            && route.source_spec_id().bytes() == link.source_spec_id.bytes()
            && route.source_spec_id().bytes() == bundle.source_spec_id.bytes()
            && route.clock_policy_id().bytes() == policy.clock_policy_id.bytes()
            && route.clock_policy_id().bytes() == root.clock_policy_id.bytes()
            && route.clock_policy_id().bytes() == link.clock_policy_id.bytes()
            && schedule.source_work_schedule_id() == route.source_work_schedule_id()
            && schedule.generation() == policy.generation
            && schedule.generation() == root.generation
            && schedule.source_compartment_account() == route.source_compartment_account()
            && schedule.source_compartment_owner() == route.source_compartment_owner(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureMarketSourceRouteV2 { route, schedule })
}

/// Current immutable Product bodies and central work profile admitted for a
/// caller-free interval context.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureMarketProductContextV2 {
    template: AuthenticatedProductArtifactV1<ProductTemplateV4>,
    basis: AuthenticatedProductArtifactV1<NativeClaimBasisV1>,
    price: AuthenticatedProductArtifactV1<PriceMeasurePolicyV1>,
    genesis: AuthenticatedProductArtifactV1<MarketGenesisProfileV2>,
    market: AuthenticatedProductArtifactV1<MarketInstancePreimageV2>,
    work_profile: QuantizedIntervalConsensusProfileV1,
    resolved_edge_policy: clutch_product_series::QuantizedEdgePolicyV1,
}

impl AuthenticatedFailureMarketProductContextV2 {
    pub(crate) const fn genesis(
        &self,
    ) -> &AuthenticatedProductArtifactV1<MarketGenesisProfileV2> {
        &self.genesis
    }

    /// Borrow the exact authenticated Product/Source context without
    /// persisting another copy of any body.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn context<'a>(
        &'a self,
        source_occurrence: &'a CompiledSourceOccurrenceV3,
        source_interval: &'a StatisticResultV3,
        statistic_key: &'a StatisticKeyV3,
        summary_program: &'a SummaryProgramV3,
        window_seal: &'a WindowSealV3,
        window: &'a WindowSpecV3,
    ) -> QuantizedIntervalConsensusContextV1<'a> {
        QuantizedIntervalConsensusContextV1 {
            market: self.market.value(),
            product_template: self.template.value(),
            native_claim_basis: self.basis.value(),
            price_measure_policy: self.price.value(),
            market_genesis: self.genesis.value(),
            resolved_edge_policy: self.resolved_edge_policy,
            source_occurrence,
            source_interval,
            statistic_key,
            summary_program,
            window_seal,
            window,
            work_profile: &self.work_profile,
        }
    }
}

/// Reopen the release-selected receiver deployment and mint the sole private
/// Source/Product route needed by Resolution V5.
pub(crate) fn authenticate_failure_market_source_product_route_v3(
    execution: &AuthenticatedFailureMarketExecutionV2<'_, '_>,
    source: AuthenticatedFailureMarketSourceRouteV2,
    product: &AuthenticatedFailureMarketProductContextV2,
    receiver_program: &AccountInfo<'_>,
    receiver_programdata: &AccountInfo<'_>,
    receiver_config: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceProductRouteV3> {
    let receiver = authenticate_receiver_route(
        source.route(),
        receiver_program,
        receiver_programdata,
        receiver_config,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let route = authenticate_source_product_route_v3(
        source.route(),
        receiver,
        execution.registry(),
        execution.bundle(),
        product.genesis(),
    )?;
    require(
        route.source_route_id() == source.route().route_id()
            && route.compiler_bundle_id().content_id() == execution.bundle().semantic_id()
            && route.source_release_manifest_id() == source.route().release_manifest_id()
            && route.source_plane_contract_id() == source.route().source_plane_contract_id()
            && route.source_spec_id() == source.route().source_spec_id(),
        ClutchError::MismatchedState,
    )?;
    Ok(route)
}

/// Authenticate every immutable Product body used by Begin/Advance/Resolve
/// against the current Bundle, Market root, Failure policy, and ProfileV4.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_product_context_v2(
    program_id: &Pubkey,
    execution: &AuthenticatedFailureMarketExecutionV2<'_, '_>,
    product_template_account: &AccountInfo<'_>,
    native_claim_basis_account: &AccountInfo<'_>,
    price_measure_policy_account: &AccountInfo<'_>,
    market_genesis_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedFailureMarketProductContextV2> {
    let bundle = execution.bundle().value();
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        product_template_account,
        bundle.product_template_id.content_id(),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        native_claim_basis_account,
        bundle.native_claim_basis_id.content_id(),
    )?;
    let price = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_measure_policy_account,
        bundle.price_measure_policy_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        market_genesis_account,
        bundle.market_genesis_profile_id.content_id(),
    )?;
    let root_binding = execution.root().state().binding();
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        root_binding.market_instance_id.content_id(),
    )?;
    let work_profile = execution
        .registry()
        .profile()
        .interval_consensus_profile()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy = execution.admission().state().binding().facts();
    let work_profile_id = work_profile
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        template.semantic_id() == bundle.product_template_id.content_id()
            && basis.semantic_id() == bundle.native_claim_basis_id.content_id()
            && price.semantic_id() == bundle.price_measure_policy_id.content_id()
            && genesis.semantic_id() == bundle.market_genesis_profile_id.content_id()
            && market.semantic_id() == root_binding.market_instance_id.content_id()
            && template.semantic_id().bytes() == policy.product_template_id.bytes()
            && basis.semantic_id().bytes() == policy.native_claim_basis_id.bytes()
            && price.semantic_id().bytes() == policy.price_measure_policy_id.bytes()
            && genesis.semantic_id().bytes() == policy.market_genesis_profile_id.bytes()
            && work_profile_id.bytes() == policy.interval_consensus_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureMarketProductContextV2 {
        template,
        basis,
        price,
        genesis,
        market,
        work_profile,
        resolved_edge_policy: execution.registry().resolved_edge_policy(),
    })
}

/// Hostile-decode the canonical 1,544-byte Product foundation graph into
/// request-heap storage and join it to the current QuoteV4 schedule and live
/// root. The bytes are not authority; Product retained-slot authentication
/// must still consume this value.
pub(crate) fn decode_failure_market_foundation_graph_v2(
    execution: &AuthenticatedFailureMarketExecutionV2<'_, '_>,
    input: &[u8],
) -> Outcome<std::boxed::Box<MarketFoundationAccountGraphV2>> {
    require(input.len() == 1_544, ClutchError::WrongDataLength)?;
    let mut account_ids = [ContentId::ZERO; MARKET_FOUNDATION_SLOT_COUNT_V2];
    let mut at = 72usize;
    for account in &mut account_ids {
        let end = at.checked_add(32).ok_or(ClutchError::Arithmetic)?;
        *account = ContentId::from_bytes(
            input[at..end]
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?,
        );
        at = end;
    }
    require(at == input.len(), ClutchError::NonCanonical)?;
    let graph = std::boxed::Box::new(MarketFoundationAccountGraphV2 {
        market_instance_id: clutch_product_series::MarketInstanceV2Id::from_bytes(
            input[..32]
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?,
        ),
        generation: u64::from_le_bytes(
            input[32..40]
                .try_into()
                .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?,
        ),
        foundation_schedule_id:
            clutch_product_series::MarketFoundationScheduleV2Id::from_bytes(
                input[40..72]
                    .try_into()
                    .map_err(|_| Refusal::Adapter(ClutchError::WrongDataLength))?,
            ),
        account_ids,
    });
    let schedule = &execution.funding_quote().value().foundation;
    graph
        .validate(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let root_binding = execution.root().state().binding();
    let graph_id = graph
        .id(schedule)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        graph.market_instance_id == root_binding.market_instance_id
            && graph.generation == root_binding.generation
            && graph_id == root_binding.foundation_account_graph_id,
        ClutchError::MismatchedState,
    )?;
    Ok(graph)
}

/// Exact replay/slot-10 authority needed only by successful Resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketResolutionFoundationV2 {
    replay: AuthenticatedFailureMarketReplayV2,
    resolution: AuthenticatedMarketFoundationPreallocationV2,
}

impl AuthenticatedFailureMarketResolutionFoundationV2 {
    pub(crate) const fn replay(self) -> AuthenticatedFailureMarketReplayV2 {
        self.replay
    }
    pub(crate) const fn resolution(self) -> AuthenticatedMarketFoundationPreallocationV2 {
        self.resolution
    }
}

/// Reopen the permanent replay commitment and the still-zero slot-10
/// Resolution preallocation under one current root/schedule/graph.
pub(crate) fn authenticate_failure_market_resolution_foundation_v2(
    program_id: &Pubkey,
    execution: &AuthenticatedFailureMarketExecutionV2<'_, '_>,
    replay_account: &AccountInfo<'_>,
    resolution_account: &AccountInfo<'_>,
    replay_funding_preimage_body: &[u8],
    foundation_account_graph_body: &[u8],
) -> Outcome<AuthenticatedFailureMarketResolutionFoundationV2> {
    let replay_preimage =
        FailureMarketReplayFundingPreimageV2::decode(replay_funding_preimage_body)?;
    let replay = reopen_failure_market_replay_v2(
        program_id,
        replay_account,
        execution.admission(),
        replay_preimage,
        true,
    )?;
    let graph = decode_failure_market_foundation_graph_v2(
        execution,
        foundation_account_graph_body,
    )?;
    let resolution = authenticate_market_foundation_preallocation_v2(
        execution.root(),
        resolution_account,
        &execution.funding_quote().value().foundation,
        &graph,
        clutch_product_series::MarketFoundationSlotV2::ResolutionV5,
    )?;
    require(
        resolution.root_account() == execution.root().account()
            && resolution.root_authentication_id() == execution.root().authentication_id()
            && resolution.market_instance_id()
                == execution.root().state().binding().market_instance_id
            && resolution.generation() == execution.root().state().binding().generation
            && resolution.account() == *resolution_account.key
            && replay.account() != resolution.account(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureMarketResolutionFoundationV2 { replay, resolution })
}

/// Reopen the full shared authority prefix for one action.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_failure_market_execution_v2<'root, 'link>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    admission_account: &AccountInfo<'_>,
    runtime_account: &AccountInfo<'_>,
    interval_cell_account: &AccountInfo<'_>,
    interval_history_account: &AccountInfo<'_>,
    series_registry_account: &AccountInfo<'_>,
    registry_program: &AccountInfo<'_>,
    registry_programdata: &AccountInfo<'_>,
    registry_release_artifact: &AccountInfo<'_>,
    capability_profile_artifact: &AccountInfo<'_>,
    compiler_bundle_artifact: &AccountInfo<'_>,
    funding_quote_artifact: &AccountInfo<'_>,
    liveness_policy_account: &AccountInfo<'_>,
    recovery_quote_schedule_body: &[u8],
    interval_funding_preimage_body: &[u8],
    root_writable: bool,
    link_writable: bool,
    interval_cell_writable: bool,
    interval_history_writable: bool,
    root_output: &'root mut MarketLifecycleRootAccountV1,
    link_output: &'link mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedFailureMarketExecutionV2<'root, 'link>> {
    let admission = authenticate_failure_market_root_v3(program_id, admission_account, false)?;
    let policy = admission.state().binding().facts();
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        policy.market_instance_id,
        policy.generation,
        root_writable,
        root_output,
    )?;

    // The candidate Series/ordinal is hostile-decoded only to select the
    // canonical link PDA. Registry, Bundle, root, and Failure equalities below
    // independently authenticate every semantic relation.
    {
        let data = link_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV1::decode_into(&data, link_output)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    let candidate = link_output.state.binding();
    let link = authenticate_series_market_link_v1(
        program_id,
        link_account,
        candidate.series_plan_id,
        candidate.ordinal,
        policy.market_instance_id,
        policy.generation,
        *root_account.key,
        link_writable,
        link_output,
    )?;
    let link_binding = link.state().binding();
    let root_binding = root.state().binding();
    require(
        root_binding.market_failure_policy_binding_id.bytes()
            == admission.state().binding().id().bytes()
            && root_binding.failure_liveness_policy_id.bytes()
                == policy.liveness_policy_id.bytes()
            && root_binding.failure_liveness_quote_schedule_id.bytes()
                == policy.recovery_quote_schedule_id.bytes()
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;
    let registry_refs = authenticate_series_registry_capability_refs_v2(
        program_id,
        series_registry_account,
        link_binding.series_plan_id,
    )?;
    let registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        registry_program,
        registry_programdata,
        registry_release_artifact,
        capability_profile_artifact,
    )?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        compiler_bundle_artifact,
        registry.compiler_bundle_id(),
    )?;
    let bundle_value = bundle.value();
    require(
        bundle_value.series_plan_id == link_binding.series_plan_id
            && bundle_value.funding_terms_id == registry.funding_terms_id()
            && bundle_value.funding_terms_id == link_binding.funding_terms_id
            && bundle_value.funding_quote_id == link_binding.funding_quote_id
            && bundle_value.attachment_plan_id == link_binding.attachment_plan_id
            && bundle_value.registry_release_id == registry.registry_release_id()
            && bundle_value.capability_profile_id.bytes()
                == registry.capability_profile_id().bytes()
            && bundle_value.registry_release_id == root_binding.registry_release_id
            && bundle_value.capability_profile_id.bytes()
                == root_binding.capability_profile_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let funding_quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_artifact,
        bundle_value.funding_quote_id.content_id(),
    )?;
    let product_quote = authenticate_market_recovery_schedule_v1(
        program_id,
        root,
        link,
        registry,
        funding_quote_artifact,
        liveness_policy_account,
    )?;
    require(
        product_quote.funding_quote_id() == bundle_value.funding_quote_id.content_id()
            && funding_quote.semantic_id() == product_quote.funding_quote_id()
            && product_quote.market_root_authentication_id() == root.authentication_id()
            && product_quote.series_link_authentication_id() == link.authentication_id(),
        ClutchError::MismatchedState,
    )?;
    let quote = authenticate_failure_market_recovery_quote_v1(
        admission,
        product_quote,
        recovery_quote_schedule_body,
    )?;
    let funding_preimage =
        FailureMarketIntervalFundingPreimageV2::decode(interval_funding_preimage_body)?;
    let interval = reopen_failure_market_interval_accounts_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        admission,
        quote,
        funding_preimage,
        interval_cell_writable,
        interval_history_writable,
    )?;
    let runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_account,
        runtime_account,
        admission,
        true,
    )?;
    require(
        runtime.state().active_interval_funding_receipt_id().is_zero()
            || runtime.state().active_interval_funding_receipt_id().bytes()
                == interval.funding().id().bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureMarketExecutionV2 {
        root,
        link,
        registry,
        bundle,
        funding_quote,
        admission,
        runtime,
        interval,
        quote,
    })
}

#[cfg(test)]
mod adversarial_join_tests {
    #[test]
    fn shared_join_contains_every_current_splice_guard() {
        let source = include_str!("failure_market_execution_v2.rs");
        for predicate in [
            "market_failure_policy_binding_id",
            "failure_liveness_quote_schedule_id",
            "bundle_value.funding_terms_id == link_binding.funding_terms_id",
            "bundle_value.funding_quote_id == link_binding.funding_quote_id",
            "bundle_value.attachment_plan_id == link_binding.attachment_plan_id",
            "product_quote.market_root_authentication_id() == root.authentication_id()",
            "product_quote.series_link_authentication_id() == link.authentication_id()",
            "reopen_failure_market_interval_accounts_v2",
        ] {
            assert!(source.contains(predicate));
        }
    }

    #[test]
    fn caller_preimages_never_bypass_semantic_owners() {
        let source = include_str!("failure_market_execution_v2.rs");
        assert!(source.contains("authenticate_failure_market_recovery_quote_v1"));
        assert!(source.contains("FailureMarketIntervalFundingPreimageV2::decode"));
        assert!(!source.contains("FailureMarketRecoveryQuoteAdmissionReceiptV1 {"));
        assert!(!source.contains("FailureMarketIntervalFundingReceiptV2 {"));
    }

    #[test]
    fn registered_source_route_is_joined_across_all_current_owners() {
        let source = include_str!("failure_market_execution_v2.rs");
        let route = source
            .split("fn authenticate_failure_market_source_route_v2")
            .nth(1)
            .expect("Source route owner");
        for predicate in [
            "authenticate_route(",
            "authenticate_source_work_schedule_artifact(",
            "policy.source_release_manifest_id",
            "root.source_release_id",
            "bundle.source_release_manifest_id",
            "root.source_route_id",
            "link.source_route_id",
            "policy.source_plane_contract_id",
            "root.source_plane_contract_id",
            "link.source_plane_contract_id",
            "bundle.source_plane_contract_id",
            "policy.source_spec_id",
            "root.source_spec_id",
            "link.source_spec_id",
            "bundle.source_spec_id",
            "policy.clock_policy_id",
            "schedule.source_work_schedule_id() == route.source_work_schedule_id()",
            "schedule.generation() == policy.generation",
        ] {
            assert!(route.contains(predicate));
        }
    }

    #[test]
    fn resolution_reopens_the_release_selected_receiver() {
        let source = include_str!("failure_market_execution_v2.rs");
        let route = source
            .split("fn authenticate_failure_market_source_product_route_v3")
            .nth(1)
            .expect("Source/Product route owner");
        assert!(route.contains("authenticate_receiver_route("));
        assert!(route.contains("authenticate_source_product_route_v3("));
        assert!(route.contains("route.compiler_bundle_id().content_id()"));
        assert!(route.contains("route.source_release_manifest_id()"));
        assert!(route.contains("route.source_plane_contract_id()"));
        assert!(route.contains("route.source_spec_id()"));
    }

    #[test]
    fn foundation_graph_is_heap_decoded_and_live_root_joined() {
        let source = include_str!("failure_market_execution_v2.rs");
        let graph = source
            .split("fn decode_failure_market_foundation_graph_v2")
            .nth(1)
            .expect("foundation graph owner");
        assert!(graph.contains("std::boxed::Box::new(MarketFoundationAccountGraphV2"));
        assert!(graph.contains("graph.validate(schedule)"));
        assert!(graph.contains("graph_id == root_binding.foundation_account_graph_id"));
        assert!(graph.contains("require(input.len() == 1_544"));
    }
}
