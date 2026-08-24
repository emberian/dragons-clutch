//! Product-owned current compiler authority for one Failure attempt.
//!
//! The module persists no parallel schedule. It hostile-authenticates the
//! current RegistryV3/BundleV6/QuoteV5 graph, recompiles one ordinal with
//! `compile_ordinal_v6`, and returns a private typed receipt consumed by the
//! sole Failure begin composer for success, absence, or refusal.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedProductArtifactV1,
};
use crate::instructions::product_series_current::{
    authenticate_market_lifecycle_root_v2, authenticate_series_market_link_v2,
    AuthenticatedMarketLifecycleRootV2, AuthenticatedRegistryCapabilityV4,
    AuthenticatedSeriesMarketLinkV2,
};
use clutch_product_series::{
    compile_ordinal_v6, derive_product_failure_begin_schedule_projection_v2,
    CompiledProductSeriesBundleV6, CompiledScheduleV1, ContentId,
    EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV2, MarketInstancePreimageV2,
    MarketInstanceV2Id, MarketLifecyclePhaseV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
    ProductFailureBeginCompilerProvenanceV2, ProductFailureBeginScheduleProjectionV2Id,
    ProductTemplateV4, SeriesAttachmentPlanV5, SeriesFundingQuoteV5,
    SeriesMarketLinkPhaseV2, SeriesPlanV5, SeriesPlanV5Id, SourceOccurrenceV1Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV2, SeriesMarketLinkAccountV2,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_FAILURE_BEGIN_SCHEDULE_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/product-failure-begin-schedule-authentication/v2\0";

/// Default-refusing exact Failure recovery-quote owner.
pub(crate) trait AuthenticatedProductFailureBeginQuoteV2 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_failure_begin_quote_v2(
        &self,
        _expected_quote_schedule_id: ContentId,
        _expected_attempt_count: u8,
        _attempt_index: u8,
        _source_repair_generation: u64,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Product-private current schedule/attempt receipt.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFailureBeginScheduleV2 {
    id: ContentId,
    schedule_projection_id: ProductFailureBeginScheduleProjectionV2Id,
    schedule: CompiledScheduleV1,
    attempt_index: u8,
    source_repair_generation: u64,
    failure_quote_receipt_id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    registry_capability_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV6Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteV5Id,
    attachment_plan_id: clutch_product_series::SeriesAttachmentPlanV5Id,
}

impl AuthenticatedProductFailureBeginScheduleV2 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn schedule_projection_id(
        &self,
    ) -> ProductFailureBeginScheduleProjectionV2Id {
        self.schedule_projection_id
    }
    pub(crate) const fn schedule(&self) -> CompiledScheduleV1 { self.schedule }
    pub(crate) const fn attempt_index(&self) -> u8 { self.attempt_index }
    pub(crate) const fn source_repair_generation(&self) -> u64 {
        self.source_repair_generation
    }
    pub(crate) const fn failure_quote_receipt_id(&self) -> ContentId {
        self.failure_quote_receipt_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_id(&self) -> ContentId {
        self.link_authentication_id
    }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.ordinal }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn source_occurrence_id(&self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }
    pub(crate) const fn registry_capability_id(&self) -> ContentId {
        self.registry_capability_id
    }
    pub(crate) const fn registry_release_id(&self) -> ContentId {
        self.registry_release_id
    }
    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.capability_profile_id
    }
    pub(crate) const fn compiler_bundle_id(
        &self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV6Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn funding_quote_id(
        &self,
    ) -> clutch_product_series::SeriesFundingQuoteV5Id {
        self.funding_quote_id
    }
    pub(crate) const fn attachment_plan_id(
        &self,
    ) -> clutch_product_series::SeriesAttachmentPlanV5Id {
        self.attachment_plan_id
    }
}

/// Hostile-authenticate and deterministically recompile one current attempt.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_product_failure_begin_schedule_v2<'root, 'link, Q>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV2<'_>,
    link_before: AuthenticatedSeriesMarketLinkV2<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle_account: &AccountInfo<'_>,
    quote_account: &AccountInfo<'_>,
    series_account: &AccountInfo<'_>,
    template_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    failure_quote: &Q,
    attempt_index: u8,
    root_decode: &'root mut MarketLifecycleRootAccountV2,
    link_decode: &'link mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedProductFailureBeginScheduleV2>
where
    Q: AuthenticatedProductFailureBeginQuoteV2 + ?Sized,
{
    authenticate_product_failure_schedule_v2(
        program_id, root_account, link_account, root_before, link_before, registry,
        bundle_account, quote_account, series_account, template_account, basis_account,
        recovery_account, price_policy_account, genesis_account, attachment_account,
        market_account, failure_quote, attempt_index, true, 0, root_decode, link_decode,
    )
}

/// Reauthenticate the exact persisted schedule for one already-pinned active
/// session. This is read-only authority; it cannot pin or begin another
/// Product session.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_product_failure_active_schedule_v2<'root, 'link, Q>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV2<'_>,
    link_before: AuthenticatedSeriesMarketLinkV2<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle_account: &AccountInfo<'_>,
    quote_account: &AccountInfo<'_>,
    series_account: &AccountInfo<'_>,
    template_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    failure_quote: &Q,
    attempt_index: u8,
    root_decode: &'root mut MarketLifecycleRootAccountV2,
    link_decode: &'link mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedProductFailureBeginScheduleV2>
where
    Q: AuthenticatedProductFailureBeginQuoteV2 + ?Sized,
{
    authenticate_product_failure_schedule_v2(
        program_id, root_account, link_account, root_before, link_before, registry,
        bundle_account, quote_account, series_account, template_account, basis_account,
        recovery_account, price_policy_account, genesis_account, attachment_account,
        market_account, failure_quote, attempt_index, false, 1, root_decode, link_decode,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_product_failure_schedule_v2<'root, 'link, Q>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV2<'_>,
    link_before: AuthenticatedSeriesMarketLinkV2<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle_account: &AccountInfo<'_>,
    quote_account: &AccountInfo<'_>,
    series_account: &AccountInfo<'_>,
    template_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    failure_quote: &Q,
    attempt_index: u8,
    expected_link_writable: bool,
    expected_active_failure_sessions: u32,
    root_decode: &'root mut MarketLifecycleRootAccountV2,
    link_decode: &'link mut SeriesMarketLinkAccountV2,
) -> Outcome<AuthenticatedProductFailureBeginScheduleV2>
where
    Q: AuthenticatedProductFailureBeginQuoteV2 + ?Sized,
{
    let expected_root_binding = root_before.state().binding();
    let expected_link_binding = link_before.state().binding();
    let root = authenticate_market_lifecycle_root_v2(
        program_id, root_account, expected_root_binding.market_instance_id,
        expected_root_binding.generation, false, root_decode)?;
    let link = authenticate_series_market_link_v2(
        program_id, link_account, expected_link_binding.series_plan_id,
        expected_link_binding.ordinal, expected_link_binding.market_instance_id,
        expected_link_binding.generation, *root_account.key, expected_link_writable, link_decode)?;
    require_cached_current_root_and_link(root_before, root, link_before, link)?;
    require_distinct_product_failure_begin_accounts_v2(
        registry,
        [*root_account.key, *link_account.key, *bundle_account.key, *quote_account.key,
            *series_account.key, *template_account.key, *basis_account.key,
            *recovery_account.key, *price_policy_account.key, *genesis_account.key,
            *attachment_account.key, *market_account.key])?;
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id, bundle_account, registry.compiler_bundle_id().content_id())?;
    let bundle_value = bundle.value();
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV5>(
        program_id, quote_account, bundle_value.funding_quote_id.content_id())?;
    let series = authenticate_product_artifact_v1::<SeriesPlanV5>(
        program_id, series_account, bundle_value.series_plan_id.content_id())?;
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id, template_account, bundle_value.product_template_id.content_id())?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id, basis_account, bundle_value.native_claim_basis_id.content_id())?;
    let recovery = authenticate_product_artifact_v1::<EvidenceOnlyRecoveryPolicyV1>(
        program_id, recovery_account,
        bundle_value.evidence_only_recovery_policy_id.content_id())?;
    let price = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id, price_policy_account, bundle_value.price_measure_policy_id.content_id())?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id, genesis_account, bundle_value.market_genesis_profile_id.content_id())?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV5>(
        program_id, attachment_account, bundle_value.attachment_plan_id.content_id())?;
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id, market_account, root_binding.market_instance_id.content_id())?;
    require_current_product_failure_begin_graph_v2(
        root, link, registry, &bundle, &quote, &series, &template, &basis,
        &recovery, &price, &genesis, &attachment, expected_link_writable,
        expected_active_failure_sessions)?;
    let compiled = compile_ordinal_v6(
        series.value(), template.value(), basis.value(), recovery.value(), price.value(),
        genesis.value(), attachment.value(), &registry.projection(), link_binding.ordinal)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(compiled.series_plan_id == link_binding.series_plan_id
        && compiled.ordinal == link_binding.ordinal
        && compiled.market_instance_id == root_binding.market_instance_id
        && compiled.market_instance_id == link_binding.market_instance_id
        && compiled.market == *market.value()
        && compiled.attachment_plan_id.bytes() == bundle_value.attachment_plan_id.bytes(),
        ClutchError::MismatchedState)?;
    compiled.schedule.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let index = usize::from(attempt_index);
    require(index < usize::from(compiled.schedule.recovery_attempt_count),
        ClutchError::MismatchedState)?;
    let source_repair_generation = compiled.schedule.recovery_attempts[index].repair_generation;
    let failure_quote_receipt_id = failure_quote.authenticate_product_failure_begin_quote_v2(
        quote.value().failure_recovery_quote_schedule_id,
        compiled.schedule.recovery_attempt_count, attempt_index, source_repair_generation)?;
    require_live(failure_quote_receipt_id)?;
    let provenance = ProductFailureBeginCompilerProvenanceV2 {
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        compiler_bundle_id: registry.compiler_bundle_id(),
        series_plan_id: compiled.series_plan_id, ordinal: compiled.ordinal,
        market_instance_id: compiled.market_instance_id,
        product_template_id: bundle_value.product_template_id,
        native_claim_basis_id: bundle_value.native_claim_basis_id,
        recovery_policy_id: bundle_value.evidence_only_recovery_policy_id,
        price_measure_policy_id: bundle_value.price_measure_policy_id,
        market_genesis_profile_id: bundle_value.market_genesis_profile_id,
        funding_terms_id: bundle_value.funding_terms_id,
        funding_quote_id: bundle_value.funding_quote_id,
        foundation_schedule_id: quote.value().foundation.id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        attachment_plan_id: bundle_value.attachment_plan_id,
        failure_recovery_quote_schedule_id: quote.value().failure_recovery_quote_schedule_id,
        attempt_index, source_repair_generation,
    };
    let schedule_projection_id = derive_product_failure_begin_schedule_projection_v2(
        compiled.schedule, provenance)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = hashv(&[
        PRODUCT_FAILURE_BEGIN_SCHEDULE_AUTHENTICATION_DOMAIN_V2,
        &schedule_projection_id.bytes(), &registry.id().bytes(),
        root.account().as_ref(), &root.authentication_id().bytes(),
        link.account().as_ref(), &link.authentication_id().bytes(),
        bundle.account().as_ref(), quote.account().as_ref(), series.account().as_ref(),
        template.account().as_ref(), basis.account().as_ref(), recovery.account().as_ref(),
        price.account().as_ref(), genesis.account().as_ref(), attachment.account().as_ref(),
        market.account().as_ref(), &failure_quote_receipt_id.bytes(),
        &link_binding.source_occurrence_id.bytes(), &root_binding.generation.to_le_bytes(),
        &[attempt_index], &source_repair_generation.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductFailureBeginScheduleV2 {
        id, schedule_projection_id, schedule: compiled.schedule, attempt_index,
        source_repair_generation, failure_quote_receipt_id, root_account: root.account(),
        root_authentication_id: root.authentication_id(), link_account: link.account(),
        link_authentication_id: link.authentication_id(), series_plan_id: compiled.series_plan_id,
        ordinal: compiled.ordinal, market_instance_id: compiled.market_instance_id,
        generation: root_binding.generation, source_occurrence_id: link_binding.source_occurrence_id,
        registry_capability_id: registry.id(), registry_release_id: provenance.registry_release_id,
        capability_profile_id: provenance.capability_profile_id,
        compiler_bundle_id: provenance.compiler_bundle_id,
        funding_quote_id: provenance.funding_quote_id,
        attachment_plan_id: provenance.attachment_plan_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_current_product_failure_begin_graph_v2(
    root: AuthenticatedMarketLifecycleRootV2<'_>,
    link: AuthenticatedSeriesMarketLinkV2<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle: &AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV6>,
    quote: &AuthenticatedProductArtifactV1<SeriesFundingQuoteV5>,
    series: &AuthenticatedProductArtifactV1<SeriesPlanV5>,
    template: &AuthenticatedProductArtifactV1<ProductTemplateV4>,
    basis: &AuthenticatedProductArtifactV1<NativeClaimBasisV1>,
    recovery: &AuthenticatedProductArtifactV1<EvidenceOnlyRecoveryPolicyV1>,
    price: &AuthenticatedProductArtifactV1<PriceMeasurePolicyV1>,
    genesis: &AuthenticatedProductArtifactV1<MarketGenesisProfileV2>,
    attachment: &AuthenticatedProductArtifactV1<SeriesAttachmentPlanV5>,
    expected_link_writable: bool,
    expected_active_failure_sessions: u32,
) -> Outcome<()> {
    let root_state = root.state();
    let root_binding = root_state.binding();
    let root_binding_id = root_binding.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_state = link.state();
    let link_binding = link_state.binding();
    let bundle_value = bundle.value();
    let quote_value = quote.value();
    let foundation_schedule_id = quote_value.foundation.id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(!root.is_writable() && link.is_writable() == expected_link_writable
        && root_state.phase() == MarketLifecyclePhaseV2::Active
        && root_state.resolution_semantic_id() == ContentId::ZERO
        && root_state.resolution_data_id() == ContentId::ZERO
        && root_state.resolution_activation_receipt_id() == ContentId::ZERO
        && link_state.phase() == SeriesMarketLinkPhaseV2::Active
        && link_state.active_failure_sessions() == expected_active_failure_sessions
        && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
        && link_binding.market_binding_id == root_binding_id
        && link_binding.market_instance_id == root_binding.market_instance_id
        && link_binding.generation == root_binding.generation
        && registry.series_plan_id() == link_binding.series_plan_id
        && registry.funding_terms_id() == bundle_value.funding_terms_id
        && registry.funding_terms_id() == link_binding.funding_terms_id
        && registry.compiler_bundle_id() == bundle_value.id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        && registry.compiler_bundle_id() == link_binding.compiler_bundle_id
        && registry.registry_release_id() == bundle_value.registry_release_id
        && registry.registry_release_id() == root_binding.registry_release_id
        && registry.capability_profile_id() == bundle_value.capability_profile_id.content_id()
        && registry.capability_profile_id() == link_binding.capability_profile_id
        && registry.capability_profile_id() == root_binding.capability_profile_id
        && bundle_value.series_plan_id == link_binding.series_plan_id
        && series.semantic_id() == bundle_value.series_plan_id.content_id()
        && template.semantic_id() == bundle_value.product_template_id.content_id()
        && basis.semantic_id() == bundle_value.native_claim_basis_id.content_id()
        && recovery.semantic_id() == bundle_value.evidence_only_recovery_policy_id.content_id()
        && price.semantic_id() == bundle_value.price_measure_policy_id.content_id()
        && genesis.semantic_id() == bundle_value.market_genesis_profile_id.content_id()
        && quote.semantic_id() == bundle_value.funding_quote_id.content_id()
        && bundle_value.funding_quote_id == link_binding.funding_quote_id
        && attachment.semantic_id() == bundle_value.attachment_plan_id.content_id()
        && bundle_value.attachment_plan_id == link_binding.attachment_plan_id
        && attachment.value().funding_quote_id == bundle_value.funding_quote_id
        && foundation_schedule_id == root_binding.foundation_schedule_id
        && quote_value.failure_liveness_policy_id == root_binding.failure_liveness_policy_id
        && quote_value.failure_recovery_quote_schedule_id
            == root_binding.failure_liveness_quote_schedule_id
        && bundle_value.product_template_id.content_id() == root_binding.product_template_id
        && bundle_value.native_claim_basis_id.content_id() == root_binding.native_claim_basis_id
        && bundle_value.evidence_only_recovery_policy_id.content_id()
            == root_binding.recovery_policy_id
        && bundle_value.price_measure_policy_id.content_id() == root_binding.price_measure_policy_id
        && bundle_value.market_genesis_profile_id.content_id()
            == root_binding.market_genesis_profile_id
        && bundle_value.source_release_manifest_id == link_binding.source_release_id
        && bundle_value.source_release_manifest_id == root_binding.source_release_id
        && bundle_value.source_plane_contract_id == link_binding.source_plane_contract_id
        && bundle_value.source_plane_contract_id == root_binding.source_plane_contract_id
        && bundle_value.source_spec_id == link_binding.source_spec_id
        && bundle_value.source_spec_id == root_binding.source_spec_id
        && link_binding.source_route_id == root_binding.source_route_id
        && link_binding.clock_policy_id == root_binding.clock_policy_id,
        ClutchError::MismatchedState)
}

fn require_cached_current_root_and_link(
    expected_root: AuthenticatedMarketLifecycleRootV2<'_>,
    live_root: AuthenticatedMarketLifecycleRootV2<'_>,
    expected_link: AuthenticatedSeriesMarketLinkV2<'_>,
    live_link: AuthenticatedSeriesMarketLinkV2<'_>,
) -> Outcome<()> {
    require(expected_root.account() == live_root.account()
        && expected_root.owner_program() == live_root.owner_program()
        && expected_root.value() == live_root.value()
        && expected_root.observed_lamports() == live_root.observed_lamports()
        && expected_root.data_id() == live_root.data_id()
        && expected_root.authentication_id() == live_root.authentication_id()
        && expected_link.account() == live_link.account()
        && expected_link.owner_program() == live_link.owner_program()
        && expected_link.value() == live_link.value()
        && expected_link.observed_lamports() == live_link.observed_lamports()
        && expected_link.data_id() == live_link.data_id()
        && expected_link.authentication_id() == live_link.authentication_id(),
        ClutchError::MismatchedState)
}

fn require_distinct_product_failure_begin_accounts_v2(
    registry: &AuthenticatedRegistryCapabilityV4,
    operation_accounts: [Pubkey; 12],
) -> Outcome<()> {
    let authority_accounts = [registry.series_registry_account(), registry.program_account(),
        registry.programdata_account(), registry.release_artifact_account(),
        registry.profile_artifact_account()];
    let mut index = 0usize;
    while index < operation_accounts.len() {
        let mut other = index + 1;
        while other < operation_accounts.len() {
            require(operation_accounts[index] != operation_accounts[other],
                ClutchError::AccountAlias)?;
            other += 1;
        }
        let mut authority = 0usize;
        while authority < authority_accounts.len() {
            require(operation_accounts[index] != authority_accounts[authority],
                ClutchError::AccountAlias)?;
            authority += 1;
        }
        index += 1;
    }
    let mut authority = 0usize;
    while authority < authority_accounts.len() {
        let mut other = authority + 1;
        while other < authority_accounts.len() {
            require(authority_accounts[authority] != authority_accounts[other],
                ClutchError::AccountAlias)?;
            other += 1;
        }
        authority += 1;
    }
    Ok(())
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
    fn current_graph_and_attempt_are_non_substitutable() {
        let source = include_str!("product_failure_begin_current.rs");
        for join in [
            "registry.funding_terms_id() == bundle_value.funding_terms_id",
            "registry.compiler_bundle_id() == link_binding.compiler_bundle_id",
            "quote.semantic_id() == bundle_value.funding_quote_id.content_id()",
            "bundle_value.attachment_plan_id == link_binding.attachment_plan_id",
            "foundation_schedule_id == root_binding.foundation_schedule_id",
            "quote_value.failure_recovery_quote_schedule_id",
            "compiled.schedule.recovery_attempts[index].repair_generation",
        ] {
            assert!(source.contains(join), "missing current join: {join}");
        }
        assert!(source.contains("failure_quote.authenticate_product_failure_begin_quote_v2("));
    }

    #[test]
    fn active_schedule_reauthentication_cannot_mint_a_second_pin() {
        let source = include_str!("product_failure_begin_current.rs");
        let begin = source
            .split("pub(crate) fn authenticate_product_failure_begin_schedule_v2")
            .nth(1)
            .and_then(|value| {
                value
                    .split("pub(crate) fn authenticate_product_failure_active_schedule_v2")
                    .next()
            })
            .expect("current begin schedule owner");
        let active = source
            .split("pub(crate) fn authenticate_product_failure_active_schedule_v2")
            .nth(1)
            .and_then(|value| {
                value
                    .split("fn authenticate_product_failure_schedule_v2")
                    .next()
            })
            .expect("current active schedule owner");
        assert!(begin.contains("attempt_index, true, 0"));
        assert!(active.contains("attempt_index, false, 1"));
        assert!(source.contains("require_cached_current_root_and_link("));
        assert!(source.contains("link_state.active_failure_sessions() == expected_active_failure_sessions"));
        assert!(source.contains("root_state.resolution_activation_receipt_id() == ContentId::ZERO"));
    }
}
