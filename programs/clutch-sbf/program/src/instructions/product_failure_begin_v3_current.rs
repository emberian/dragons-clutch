//! Product-owned BundleV7 schedule authority for current Failure sessions.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::instructions::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
    AuthenticatedMarketLifecycleRootV3, AuthenticatedSeriesMarketLinkV3,
};
use crate::instructions::product_series_current::{
    AuthenticatedRegistryCapabilityV5, AuthenticatedSeriesFundingAccountV5,
};
use crate::instructions::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV7, AuthenticatedSeriesSourceArtifactsV6,
};
use clutch_product_series::{
    compile_ordinal_v7, derive_product_failure_begin_schedule_projection_v3, CompiledScheduleV1,
    ContentId, MarketInstancePreimageV2, MarketInstanceV2Id, MarketLifecyclePhaseV3,
    ProductFailureBeginCompilerProvenanceV3, ProductFailureBeginScheduleProjectionV3Id,
    SeriesAttachmentPlanV6Id, SeriesFundingPhaseV5, SeriesFundingQuoteV6Id,
    SeriesMarketLinkPhaseV3, SeriesPlanV5Id, SourceOccurrenceV1Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_FAILURE_SCHEDULE_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-failure-schedule-authentication/v3";

/// Default-refusing current Recovery-quote authority.
pub(crate) trait AuthenticatedProductFailureBeginQuoteV3 {
    fn authenticate_product_failure_begin_quote_v3(
        &self,
        _expected_quote_schedule_id: ContentId,
        _expected_attempt_count: u8,
        _attempt_index: u8,
        _source_repair_generation: u64,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Move-only current Product schedule/attempt receipt.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductFailureScheduleV3 {
    id: ContentId,
    schedule_projection_id: ProductFailureBeginScheduleProjectionV3Id,
    schedule: CompiledScheduleV1,
    attempt_index: u8,
    source_repair_generation: u64,
    failure_quote_receipt_id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_semantic_id: ContentId,
    funding_account: Pubkey,
    funding_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    registry_capability_id: ContentId,
    compiler_bundle_id: clutch_product_series::CompiledProductSeriesBundleV7Id,
    funding_quote_id: SeriesFundingQuoteV6Id,
    attachment_plan_id: SeriesAttachmentPlanV6Id,
}

impl AuthenticatedProductFailureScheduleV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn schedule(&self) -> CompiledScheduleV1 { self.schedule }
    pub(crate) const fn attempt_index(&self) -> u8 { self.attempt_index }
    pub(crate) const fn source_repair_generation(&self) -> u64 {
        self.source_repair_generation
    }
    pub(crate) const fn failure_quote_receipt_id(&self) -> ContentId {
        self.failure_quote_receipt_id
    }
    pub(crate) const fn schedule_projection_id(
        &self,
    ) -> ProductFailureBeginScheduleProjectionV3Id {
        self.schedule_projection_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_id(&self) -> ContentId {
        self.link_authentication_id
    }
    pub(crate) const fn link_semantic_id(&self) -> ContentId { self.link_semantic_id }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.funding_account }
    pub(crate) const fn funding_authentication_id(&self) -> ContentId {
        self.funding_authentication_id
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
    pub(crate) const fn compiler_bundle_id(
        &self,
    ) -> clutch_product_series::CompiledProductSeriesBundleV7Id {
        self.compiler_bundle_id
    }
    pub(crate) const fn funding_quote_id(&self) -> SeriesFundingQuoteV6Id {
        self.funding_quote_id
    }
    pub(crate) const fn attachment_plan_id(&self) -> SeriesAttachmentPlanV6Id {
        self.attachment_plan_id
    }
}

/// Authenticate and compile the exact current schedule before a LinkV3 pin.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_product_failure_begin_schedule_v3<'root, 'link, Q>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: &AuthenticatedMarketLifecycleRootV3<'_>,
    link_before: &AuthenticatedSeriesMarketLinkV3<'_>,
    registry: &AuthenticatedRegistryCapabilityV5,
    funding: &AuthenticatedSeriesFundingAccountV5,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    market_account: &AccountInfo<'_>,
    failure_quote: &Q,
    attempt_index: u8,
    root_decode: &'root mut MarketLifecycleRootAccountV3,
    link_decode: &'link mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedProductFailureScheduleV3>
where
    Q: AuthenticatedProductFailureBeginQuoteV3 + ?Sized,
{
    authenticate_product_failure_schedule_v3(
        program_id, root_account, link_account, root_before, link_before, registry, funding,
        artifacts, bundle, market_account, failure_quote, attempt_index, true, 0,
        root_decode, link_decode,
    )
}

/// Reauthenticate and deterministically compile one already-pinned current session.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_product_failure_active_schedule_v3<'root, 'link, Q>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: &AuthenticatedMarketLifecycleRootV3<'_>,
    link_before: &AuthenticatedSeriesMarketLinkV3<'_>,
    registry: &AuthenticatedRegistryCapabilityV5,
    funding: &AuthenticatedSeriesFundingAccountV5,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    market_account: &AccountInfo<'_>,
    failure_quote: &Q,
    attempt_index: u8,
    root_decode: &'root mut MarketLifecycleRootAccountV3,
    link_decode: &'link mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedProductFailureScheduleV3>
where
    Q: AuthenticatedProductFailureBeginQuoteV3 + ?Sized,
{
    authenticate_product_failure_schedule_v3(
        program_id, root_account, link_account, root_before, link_before, registry, funding,
        artifacts, bundle, market_account, failure_quote, attempt_index, false, 1,
        root_decode, link_decode,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_product_failure_schedule_v3<'root, 'link, Q>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: &AuthenticatedMarketLifecycleRootV3<'_>,
    link_before: &AuthenticatedSeriesMarketLinkV3<'_>,
    registry: &AuthenticatedRegistryCapabilityV5,
    funding: &AuthenticatedSeriesFundingAccountV5,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    market_account: &AccountInfo<'_>,
    failure_quote: &Q,
    attempt_index: u8,
    expected_link_writable: bool,
    expected_active_failure_sessions: u32,
    root_decode: &'root mut MarketLifecycleRootAccountV3,
    link_decode: &'link mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedProductFailureScheduleV3>
where
    Q: AuthenticatedProductFailureBeginQuoteV3 + ?Sized,
{
    let root_binding = root_before.binding();
    let link_binding = link_before.binding();
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        root_binding.market_instance_id,
        root_binding.generation,
        expected_link_writable,
        root_decode,
    )?;
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        *root_account.key,
        false,
        link_decode,
    )?;
    require_cached_root_and_link_v3(root_before, &root, link_before, &link)?;
    let root_state = root.state();
    let link_state = link.state();
    let root_binding_id = root.binding_id();
    let link_semantic_id = link.semantic_id().content_id();
    let funding_state = funding.state();
    let bundle_value = bundle.bundle();
    require(
        !funding.is_writable()
            && root_state.phase() == MarketLifecyclePhaseV3::Active
            && root_state.resolution_semantic_id() == ContentId::ZERO
            && root_state.resolution_data_id() == ContentId::ZERO
            && root_state.resolution_activation_receipt_id() == ContentId::ZERO
            && link_state.phase() == SeriesMarketLinkPhaseV3::Active
            && link_state.active_failure_sessions() == expected_active_failure_sessions
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && funding.account().to_bytes() == link_binding.funding_state_account_id.bytes()
            && funding_state.series_plan_id == link_binding.series_plan_id
            && funding_state.funding_terms_id == link_binding.funding_terms_id
            && funding_state.funding_quote_id == link_binding.funding_quote_id
            && funding_state.attachment_plan_id == link_binding.attachment_plan_id
            && funding_state.compiler_bundle_id == link_binding.compiler_bundle_id
            && funding_state.phase != SeriesFundingPhaseV5::Pending
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.funding_terms_id() == link_binding.funding_terms_id
            && registry.compiler_bundle_id() == link_binding.compiler_bundle_id
            && registry.registry_release_id() == root_binding.registry_release_id
            && registry.capability_profile_id() == root_binding.capability_profile_id
            && bundle.bundle_id() == link_binding.compiler_bundle_id
            && bundle_value.series_plan_id == link_binding.series_plan_id
            && bundle_value.funding_terms_id == link_binding.funding_terms_id
            && bundle_value.funding_quote_id == link_binding.funding_quote_id
            && bundle_value.attachment_plan_id == link_binding.attachment_plan_id,
        ClutchError::MismatchedState,
    )?;
    artifacts.validate_registry_projection(&registry.projection())?;
    let compiled = compile_ordinal_v7(
        artifacts.series(),
        artifacts.template(),
        artifacts.basis(),
        artifacts.recovery(),
        artifacts.price_policy(),
        artifacts.genesis(),
        artifacts.attachment(),
        &registry.projection(),
        link_binding.ordinal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_account,
        root_binding.market_instance_id.content_id(),
    )?;
    require(
        compiled.series_plan_id == link_binding.series_plan_id
            && compiled.ordinal == link_binding.ordinal
            && compiled.market_instance_id == root_binding.market_instance_id
            && compiled.market_instance_id == link_binding.market_instance_id
            && compiled.market == *market.value()
            && compiled.attachment_plan_id.bytes() == link_binding.attachment_plan_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let index = usize::from(attempt_index);
    require(
        index < usize::from(compiled.schedule.recovery_attempt_count),
        ClutchError::MismatchedState,
    )?;
    let source_repair_generation = compiled.schedule.recovery_attempts[index].repair_generation;
    let failure_quote_receipt_id = failure_quote.authenticate_product_failure_begin_quote_v3(
        artifacts.quote().failure_recovery_quote_schedule_id,
        compiled.schedule.recovery_attempt_count,
        attempt_index,
        source_repair_generation,
    )?;
    let provenance = ProductFailureBeginCompilerProvenanceV3 {
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        compiler_bundle_id: bundle.bundle_id(),
        series_plan_id: compiled.series_plan_id,
        ordinal: compiled.ordinal,
        market_instance_id: compiled.market_instance_id,
        product_template_id: bundle_value.product_template_id,
        native_claim_basis_id: bundle_value.native_claim_basis_id,
        recovery_policy_id: bundle_value.evidence_only_recovery_policy_id,
        price_measure_policy_id: bundle_value.price_measure_policy_id,
        market_genesis_profile_id: bundle_value.market_genesis_profile_id,
        funding_terms_id: bundle_value.funding_terms_id,
        funding_quote_id: bundle_value.funding_quote_id,
        foundation_schedule_id: artifacts
            .quote()
            .foundation
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        attachment_plan_id: bundle_value.attachment_plan_id,
        failure_recovery_quote_schedule_id: artifacts.quote().failure_recovery_quote_schedule_id,
        attempt_index,
        source_repair_generation,
    };
    let schedule_projection_id = derive_product_failure_begin_schedule_projection_v3(
        compiled.schedule,
        provenance,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FAILURE_SCHEDULE_AUTHENTICATION_DOMAIN_V3,
            &schedule_projection_id.bytes(),
            &registry.id().bytes(),
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            &link_semantic_id.bytes(),
            funding.account().as_ref(),
            &funding.authentication_id().bytes(),
            market.account().as_ref(),
            &failure_quote_receipt_id.bytes(),
            &[attempt_index],
            &source_repair_generation.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedProductFailureScheduleV3 {
        id,
        schedule_projection_id,
        schedule: compiled.schedule,
        attempt_index,
        source_repair_generation,
        failure_quote_receipt_id,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        link_account: link.account(),
        link_authentication_id: link.authentication_id(),
        link_semantic_id,
        funding_account: funding.account(),
        funding_authentication_id: funding.authentication_id(),
        series_plan_id: compiled.series_plan_id,
        ordinal: compiled.ordinal,
        market_instance_id: compiled.market_instance_id,
        generation: root_binding.generation,
        source_occurrence_id: link_binding.source_occurrence_id,
        registry_capability_id: registry.id(),
        compiler_bundle_id: provenance.compiler_bundle_id,
        funding_quote_id: provenance.funding_quote_id,
        attachment_plan_id: provenance.attachment_plan_id,
    })
}

fn require_cached_root_and_link_v3(
    expected_root: &AuthenticatedMarketLifecycleRootV3<'_>,
    live_root: &AuthenticatedMarketLifecycleRootV3<'_>,
    expected_link: &AuthenticatedSeriesMarketLinkV3<'_>,
    live_link: &AuthenticatedSeriesMarketLinkV3<'_>,
) -> Outcome<()> {
    require(
        expected_root.account() == live_root.account()
            && expected_root.owner_program() == live_root.owner_program()
            && expected_root.value() == live_root.value()
            && expected_root.observed_lamports() == live_root.observed_lamports()
            && expected_root.data_id() == live_root.data_id()
            && expected_root.semantic_id() == live_root.semantic_id()
            && expected_root.authentication_id() == live_root.authentication_id()
            && expected_link.account() == live_link.account()
            && expected_link.owner_program() == live_link.owner_program()
            && expected_link.value() == live_link.value()
            && expected_link.observed_lamports() == live_link.observed_lamports()
            && expected_link.data_id() == live_link.data_id()
            && expected_link.semantic_id() == live_link.semantic_id()
            && expected_link.authentication_id() == live_link.authentication_id(),
        ClutchError::MismatchedState,
    )
}
