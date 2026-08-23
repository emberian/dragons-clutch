// SPDX-License-Identifier: AGPL-3.0-or-later
//! Product-owned current compiler authority for a Failure interval Begin.
//!
//! This module is deliberately not routed by dispatch and persists no schedule
//! artifact. It hostile-authenticates the current Product graph, recompiles one
//! exact V5 Series ordinal, and returns a private receipt over the complete
//! canonical schedule body and its compiler provenance. Possession of a caller
//! supplied schedule or digest is never authority.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedProductArtifactV1,
    AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    AuthenticatedMarketLifecycleRootV1, AuthenticatedSeriesMarketLinkV1,
};
use clutch_product_series::{
    compile_ordinal_v5, derive_product_failure_begin_schedule_projection_v1,
    CompiledProductSeriesBundleV5, CompiledScheduleV1, ContentId, EvidenceOnlyRecoveryPolicyV1,
    MarketGenesisProfileV2, MarketInstancePreimageV2, MarketInstanceV2Id, MarketLifecyclePhaseV1,
    NativeClaimBasisV1, PriceMeasurePolicyV1, ProductFailureBeginCompilerProvenanceV1,
    ProductFailureBeginScheduleProjectionV1Id, ProductTemplateV4, SeriesAttachmentPlanV4,
    SeriesMarketLinkPhaseV1, SeriesPlanV5, SeriesPlanV5Id, SourceOccurrenceV1Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_FAILURE_BEGIN_SCHEDULE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-failure-begin-schedule-authentication/v1\0";

/// Testable exact equality partition for the current mutable/immutable graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductFailureBeginGraphJoinV1 {
    registry_terms: ContentId,
    bundle_terms: ContentId,
    link_terms: ContentId,
    bundle_quote: ContentId,
    link_quote: ContentId,
    attachment_quote: ContentId,
    bundle_attachment: ContentId,
    link_attachment: ContentId,
    attachment_semantic: ContentId,
    registry_release: ContentId,
    projection_release: ContentId,
    bundle_release: ContentId,
    root_release: ContentId,
    registry_profile: ContentId,
    projection_profile: ContentId,
    bundle_profile: ContentId,
    link_profile: ContentId,
    root_profile: ContentId,
    bundle_source_release: ContentId,
    link_source_release: ContentId,
    root_source_release: ContentId,
    bundle_source_plane: ContentId,
    link_source_plane: ContentId,
    root_source_plane: ContentId,
    bundle_source_spec: ContentId,
    link_source_spec: ContentId,
    root_source_spec: ContentId,
    link_source_route: ContentId,
    root_source_route: ContentId,
    link_clock_policy: ContentId,
    root_clock_policy: ContentId,
}

impl ProductFailureBeginGraphJoinV1 {
    fn validate(self) -> Outcome<()> {
        require(
            self.registry_terms == self.bundle_terms
                && self.registry_terms == self.link_terms
                && self.bundle_quote == self.link_quote
                && self.bundle_quote == self.attachment_quote
                && self.bundle_attachment == self.link_attachment
                && self.bundle_attachment == self.attachment_semantic
                && self.registry_release == self.projection_release
                && self.registry_release == self.bundle_release
                && self.registry_release == self.root_release
                && self.registry_profile == self.projection_profile
                && self.registry_profile == self.bundle_profile
                && self.registry_profile == self.link_profile
                && self.registry_profile == self.root_profile
                && self.bundle_source_release == self.link_source_release
                && self.bundle_source_release == self.root_source_release
                && self.bundle_source_plane == self.link_source_plane
                && self.bundle_source_plane == self.root_source_plane
                && self.bundle_source_spec == self.link_source_spec
                && self.bundle_source_spec == self.root_source_spec
                && self.link_source_route == self.root_source_route
                && self.link_clock_policy == self.root_clock_policy,
            ClutchError::MismatchedState,
        )
    }
}

/// Product-private exact current schedule projection for one subordinate Begin.
///
/// The receipt is intentionally not decodable and its constructor is private to
/// this module. Failure may consume its crate getters only inside the atomic
/// Begin composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductFailureBeginScheduleV1 {
    id: ContentId,
    schedule_projection_id: ProductFailureBeginScheduleProjectionV1Id,
    schedule: CompiledScheduleV1,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    compiler_bundle_id: ContentId,
}

impl AuthenticatedProductFailureBeginScheduleV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn schedule_projection_id(self) -> ProductFailureBeginScheduleProjectionV1Id {
        self.schedule_projection_id
    }

    pub(crate) const fn schedule(self) -> CompiledScheduleV1 {
        self.schedule
    }

    pub(crate) const fn root_account(self) -> Pubkey {
        self.root_account
    }

    pub(crate) const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }

    pub(crate) const fn link_account(self) -> Pubkey {
        self.link_account
    }

    pub(crate) const fn link_authentication_id(self) -> ContentId {
        self.link_authentication_id
    }

    pub(crate) const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn ordinal(self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    pub(crate) const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn source_occurrence_id(self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }

    pub(crate) const fn registry_release_id(self) -> ContentId {
        self.registry_release_id
    }

    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }

    pub(crate) const fn compiler_bundle_id(self) -> ContentId {
        self.compiler_bundle_id
    }
}

/// Authenticate and deterministically recompile one current V5 ordinal.
///
/// Both mutable lifecycle receipts are hostile-reopened before their facts are
/// committed. The link remains unmodified; the atomic Failure Begin composer
/// consumes this receipt and performs the sole cell-activation/link-pin batch.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_product_failure_begin_schedule_v1<'root, 'link>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV1<'_>,
    link_before: AuthenticatedSeriesMarketLinkV1<'_>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle_account: &AccountInfo<'_>,
    series_account: &AccountInfo<'_>,
    template_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    attachment_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    root_decode: &'root mut MarketLifecycleRootAccountV1,
    link_decode: &'link mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedProductFailureBeginScheduleV1> {
    let expected_root_binding = root_before.state().binding();
    let expected_link_binding = link_before.state().binding();
    let root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        expected_root_binding.market_instance_id,
        expected_root_binding.generation,
        false,
        root_decode,
    )?;
    let link = authenticate_series_market_link_v1(
        program_id,
        link_account,
        expected_link_binding.series_plan_id,
        expected_link_binding.ordinal,
        expected_link_binding.market_instance_id,
        expected_link_binding.generation,
        *root_account.key,
        true,
        link_decode,
    )?;
    require_cached_root_and_link(root_before, root, link_before, link)?;
    require_distinct_product_failure_begin_accounts(
        registry,
        [
            *root_account.key,
            *link_account.key,
            *bundle_account.key,
            *series_account.key,
            *template_account.key,
            *basis_account.key,
            *recovery_account.key,
            *price_policy_account.key,
            *genesis_account.key,
            *attachment_account.key,
            *market_account.key,
        ],
    )?;

    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        bundle_account,
        registry.compiler_bundle_id(),
    )?;
    let bundle_value = bundle.value();
    let series = authenticate_product_artifact_v1::<SeriesPlanV5>(
        program_id,
        series_account,
        bundle_value.series_plan_id.content_id(),
    )?;
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        template_account,
        bundle_value.product_template_id.content_id(),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        basis_account,
        bundle_value.native_claim_basis_id.content_id(),
    )?;
    let recovery = authenticate_product_artifact_v1::<EvidenceOnlyRecoveryPolicyV1>(
        program_id,
        recovery_account,
        bundle_value.evidence_only_recovery_policy_id.content_id(),
    )?;
    let price = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_policy_account,
        bundle_value.price_measure_policy_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle_value.market_genesis_profile_id.content_id(),
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV4>(
        program_id,
        attachment_account,
        bundle_value.attachment_plan_id.content_id(),
    )?;
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_account,
        root_binding.market_instance_id.content_id(),
    )?;

    require_current_product_failure_begin_graph_v1(
        root,
        link,
        registry,
        &bundle,
        &series,
        &template,
        &basis,
        &recovery,
        &price,
        &genesis,
        &attachment,
    )?;
    let compiled = compile_ordinal_v5(
        series.value(),
        template.value(),
        basis.value(),
        recovery.value(),
        price.value(),
        genesis.value(),
        attachment.value(),
        &registry.projection(),
        link_binding.ordinal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        compiled.series_plan_id == link_binding.series_plan_id
            && compiled.ordinal == link_binding.ordinal
            && compiled.market_instance_id == root_binding.market_instance_id
            && compiled.market_instance_id == link_binding.market_instance_id
            && compiled.market == *market.value()
            && compiled.attachment_plan_id.bytes() == bundle_value.attachment_plan_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    compiled
        .schedule
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let provenance = ProductFailureBeginCompilerProvenanceV1 {
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        compiler_bundle_id: bundle.semantic_id(),
        series_plan_id: compiled.series_plan_id,
        ordinal: compiled.ordinal,
        market_instance_id: compiled.market_instance_id,
        product_template_id: template.semantic_id(),
        native_claim_basis_id: basis.semantic_id(),
        recovery_policy_id: recovery.semantic_id(),
        price_measure_policy_id: price.semantic_id(),
        market_genesis_profile_id: genesis.semantic_id(),
        attachment_plan_id: attachment.semantic_id(),
    };
    let schedule_projection_id =
        derive_product_failure_begin_schedule_projection_v1(compiled.schedule, provenance)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_FAILURE_BEGIN_SCHEDULE_AUTHENTICATION_DOMAIN_V1,
            &schedule_projection_id.bytes(),
            root.account().as_ref(),
            &root.authentication_id().bytes(),
            link.account().as_ref(),
            &link.authentication_id().bytes(),
            registry.series_registry_account().as_ref(),
            registry.program_account().as_ref(),
            registry.programdata_account().as_ref(),
            registry.release_artifact_account().as_ref(),
            registry.profile_artifact_account().as_ref(),
            bundle.account().as_ref(),
            series.account().as_ref(),
            template.account().as_ref(),
            basis.account().as_ref(),
            recovery.account().as_ref(),
            price.account().as_ref(),
            genesis.account().as_ref(),
            attachment.account().as_ref(),
            market.account().as_ref(),
            &link_binding.source_occurrence_id.bytes(),
            &root_binding.generation.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedProductFailureBeginScheduleV1 {
        id,
        schedule_projection_id,
        schedule: compiled.schedule,
        root_account: root.account(),
        root_authentication_id: root.authentication_id(),
        link_account: link.account(),
        link_authentication_id: link.authentication_id(),
        series_plan_id: compiled.series_plan_id,
        ordinal: compiled.ordinal,
        market_instance_id: compiled.market_instance_id,
        generation: root_binding.generation,
        source_occurrence_id: link_binding.source_occurrence_id,
        registry_release_id: provenance.registry_release_id,
        capability_profile_id: provenance.capability_profile_id,
        compiler_bundle_id: provenance.compiler_bundle_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_current_product_failure_begin_graph_v1(
    root: AuthenticatedMarketLifecycleRootV1<'_>,
    link: AuthenticatedSeriesMarketLinkV1<'_>,
    registry: AuthenticatedRegistryCapabilityV3,
    bundle: &AuthenticatedProductArtifactV1<CompiledProductSeriesBundleV5>,
    series: &AuthenticatedProductArtifactV1<SeriesPlanV5>,
    template: &AuthenticatedProductArtifactV1<ProductTemplateV4>,
    basis: &AuthenticatedProductArtifactV1<NativeClaimBasisV1>,
    recovery: &AuthenticatedProductArtifactV1<EvidenceOnlyRecoveryPolicyV1>,
    price: &AuthenticatedProductArtifactV1<PriceMeasurePolicyV1>,
    genesis: &AuthenticatedProductArtifactV1<MarketGenesisProfileV2>,
    attachment: &AuthenticatedProductArtifactV1<SeriesAttachmentPlanV4>,
) -> Outcome<()> {
    let root_state = root.state();
    let root_binding = root_state.binding();
    let root_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_state = *link.state();
    let link_binding = link_state.binding();
    let bundle_value = bundle.value();
    let projection = registry.projection();
    ProductFailureBeginGraphJoinV1 {
        registry_terms: registry.funding_terms_id().content_id(),
        bundle_terms: bundle_value.funding_terms_id.content_id(),
        link_terms: link_binding.funding_terms_id.content_id(),
        bundle_quote: bundle_value.funding_quote_id.content_id(),
        link_quote: link_binding.funding_quote_id.content_id(),
        attachment_quote: attachment.value().funding_quote_id.content_id(),
        bundle_attachment: bundle_value.attachment_plan_id.content_id(),
        link_attachment: link_binding.attachment_plan_id,
        attachment_semantic: attachment.semantic_id(),
        registry_release: registry.registry_release_id(),
        projection_release: projection.registry_release_id,
        bundle_release: bundle_value.registry_release_id,
        root_release: root_binding.registry_release_id,
        registry_profile: registry.capability_profile_id(),
        projection_profile: projection.capability_profile_id,
        bundle_profile: bundle_value.capability_profile_id.content_id(),
        link_profile: link_binding.capability_profile_id,
        root_profile: root_binding.capability_profile_id,
        bundle_source_release: bundle_value.source_release_manifest_id,
        link_source_release: link_binding.source_release_id,
        root_source_release: root_binding.source_release_id,
        bundle_source_plane: bundle_value.source_plane_contract_id,
        link_source_plane: link_binding.source_plane_contract_id,
        root_source_plane: root_binding.source_plane_contract_id,
        bundle_source_spec: bundle_value.source_spec_id,
        link_source_spec: link_binding.source_spec_id,
        root_source_spec: root_binding.source_spec_id,
        link_source_route: link_binding.source_route_id,
        root_source_route: root_binding.source_route_id,
        link_clock_policy: link_binding.clock_policy_id,
        root_clock_policy: root_binding.clock_policy_id,
    }
    .validate()?;
    require(
        !root.is_writable()
            && link.is_writable()
            && root_state.phase() == MarketLifecyclePhaseV1::Active
            && root_state.resolution_semantic_id() == ContentId::ZERO
            && root_state.resolution_data_id() == ContentId::ZERO
            && root_state.resolution_activation_receipt_id() == ContentId::ZERO
            && link_state.phase() == SeriesMarketLinkPhaseV1::Active
            && link_state.active_failure_sessions() == 0
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && registry.series_plan_id() == link_binding.series_plan_id
            && registry.compiler_bundle_id() == bundle.semantic_id()
            && bundle_value.series_plan_id == link_binding.series_plan_id
            && series.semantic_id() == bundle_value.series_plan_id.content_id()
            && template.semantic_id() == bundle_value.product_template_id.content_id()
            && basis.semantic_id() == bundle_value.native_claim_basis_id.content_id()
            && recovery.semantic_id() == bundle_value.evidence_only_recovery_policy_id.content_id()
            && price.semantic_id() == bundle_value.price_measure_policy_id.content_id()
            && genesis.semantic_id() == bundle_value.market_genesis_profile_id.content_id()
            && attachment.semantic_id() == bundle_value.attachment_plan_id.content_id()
            && bundle_value.product_template_id.content_id() == root_binding.product_template_id
            && bundle_value.native_claim_basis_id.content_id()
                == root_binding.native_claim_basis_id
            && bundle_value.evidence_only_recovery_policy_id.content_id()
                == root_binding.recovery_policy_id
            && bundle_value.price_measure_policy_id.content_id()
                == root_binding.price_measure_policy_id
            && bundle_value.market_genesis_profile_id.content_id()
                == root_binding.market_genesis_profile_id
            && link_binding.compiler_output_id == bundle.semantic_id(),
        ClutchError::MismatchedState,
    )
}

fn require_cached_root_and_link(
    expected_root: AuthenticatedMarketLifecycleRootV1<'_>,
    live_root: AuthenticatedMarketLifecycleRootV1<'_>,
    expected_link: AuthenticatedSeriesMarketLinkV1<'_>,
    live_link: AuthenticatedSeriesMarketLinkV1<'_>,
) -> Outcome<()> {
    require(
        expected_root.account() == live_root.account()
            && expected_root.owner_program() == live_root.owner_program()
            && expected_root.state() == live_root.state()
            && expected_root.observed_lamports() == live_root.observed_lamports()
            && expected_root.data_id() == live_root.data_id()
            && expected_root.authentication_id() == live_root.authentication_id()
            && expected_link.account() == live_link.account()
            && expected_link.owner_program() == live_link.owner_program()
            && expected_link.state() == live_link.state()
            && expected_link.observed_lamports() == live_link.observed_lamports()
            && expected_link.data_id() == live_link.data_id()
            && expected_link.authentication_id() == live_link.authentication_id(),
        ClutchError::MismatchedState,
    )
}

fn require_distinct_product_failure_begin_accounts(
    registry: AuthenticatedRegistryCapabilityV3,
    operation_accounts: [Pubkey; 11],
) -> Outcome<()> {
    let authority_accounts = [
        registry.series_registry_account(),
        registry.program_account(),
        registry.programdata_account(),
        registry.release_artifact_account(),
        registry.profile_artifact_account(),
    ];
    let mut operation_index = 0_usize;
    while operation_index < operation_accounts.len() {
        let mut other_operation_index = operation_index + 1;
        while other_operation_index < operation_accounts.len() {
            require(
                operation_accounts[operation_index] != operation_accounts[other_operation_index],
                ClutchError::AccountAlias,
            )?;
            other_operation_index += 1;
        }
        let mut authority_index = 0_usize;
        while authority_index < authority_accounts.len() {
            require(
                operation_accounts[operation_index] != authority_accounts[authority_index],
                ClutchError::AccountAlias,
            )?;
            authority_index += 1;
        }
        operation_index += 1;
    }
    let mut authority_index = 0_usize;
    while authority_index < authority_accounts.len() {
        let mut other_authority_index = authority_index + 1;
        while other_authority_index < authority_accounts.len() {
            require(
                authority_accounts[authority_index] != authority_accounts[other_authority_index],
                ClutchError::AccountAlias,
            )?;
            other_authority_index += 1;
        }
        authority_index += 1;
    }
    Ok(())
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    id.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_product_series::{AbsoluteRecoveryAttemptV1, MAX_RECOVERY_ATTEMPTS};

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn schedule() -> CompiledScheduleV1 {
        let mut attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = AbsoluteRecoveryAttemptV1 {
            repair_generation: 7,
            opens_at_bucket: 30,
            closes_at_bucket: 40,
        };
        CompiledScheduleV1 {
            start_bucket: 10,
            end_bucket_exclusive: 20,
            primary_maturity_bucket_exclusive: 25,
            recovery_attempt_count: 1,
            recovery_attempts: attempts,
        }
    }

    fn provenance() -> ProductFailureBeginCompilerProvenanceV1 {
        ProductFailureBeginCompilerProvenanceV1 {
            registry_release_id: id(1),
            capability_profile_id: id(2),
            compiler_bundle_id: id(3),
            series_plan_id: SeriesPlanV5Id::from_bytes([4; 32]),
            ordinal: 5,
            market_instance_id: MarketInstanceV2Id::from_bytes([6; 32]),
            product_template_id: id(7),
            native_claim_basis_id: id(8),
            recovery_policy_id: id(9),
            price_measure_policy_id: id(10),
            market_genesis_profile_id: id(11),
            attachment_plan_id: id(12),
        }
    }

    fn graph() -> ProductFailureBeginGraphJoinV1 {
        ProductFailureBeginGraphJoinV1 {
            registry_terms: id(1),
            bundle_terms: id(1),
            link_terms: id(1),
            bundle_quote: id(2),
            link_quote: id(2),
            attachment_quote: id(2),
            bundle_attachment: id(3),
            link_attachment: id(3),
            attachment_semantic: id(3),
            registry_release: id(4),
            projection_release: id(4),
            bundle_release: id(4),
            root_release: id(4),
            registry_profile: id(5),
            projection_profile: id(5),
            bundle_profile: id(5),
            link_profile: id(5),
            root_profile: id(5),
            bundle_source_release: id(6),
            link_source_release: id(6),
            root_source_release: id(6),
            bundle_source_plane: id(7),
            link_source_plane: id(7),
            root_source_plane: id(7),
            bundle_source_spec: id(8),
            link_source_spec: id(8),
            root_source_spec: id(8),
            link_source_route: id(9),
            root_source_route: id(9),
            link_clock_policy: id(10),
            root_clock_policy: id(10),
        }
    }

    #[test]
    fn full_schedule_body_and_every_provenance_role_change_projection() {
        let original =
            derive_product_failure_begin_schedule_projection_v1(schedule(), provenance()).unwrap();

        let mut altered_schedule = schedule();
        altered_schedule.recovery_attempts[0].closes_at_bucket = 41;
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(altered_schedule, provenance())
                .unwrap(),
            original
        );

        let mut altered = provenance();
        altered.registry_release_id = id(13);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.capability_profile_id = id(13);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.compiler_bundle_id = id(13);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.series_plan_id = SeriesPlanV5Id::from_bytes([13; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.ordinal = 13;
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        let mut altered = provenance();
        altered.market_instance_id = MarketInstanceV2Id::from_bytes([13; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
            original
        );
        for role in 0_u8..6_u8 {
            let mut altered = provenance();
            match role {
                0 => altered.product_template_id = id(13),
                1 => altered.native_claim_basis_id = id(13),
                2 => altered.recovery_policy_id = id(13),
                3 => altered.price_measure_policy_id = id(13),
                4 => altered.market_genesis_profile_id = id(13),
                5 => altered.attachment_plan_id = id(13),
                _ => unreachable!(),
            }
            assert_ne!(
                derive_product_failure_begin_schedule_projection_v1(schedule(), altered).unwrap(),
                original
            );
        }
    }

    #[test]
    fn noncanonical_schedule_is_never_projected() {
        let mut invalid = schedule();
        invalid.recovery_attempts[1] = invalid.recovery_attempts[0];
        assert!(
            derive_product_failure_begin_schedule_projection_v1(invalid, provenance()).is_err()
        );
    }

    #[test]
    fn registry_bundle_link_and_root_splices_refuse() {
        assert!(graph().validate().is_ok());

        let mut altered = graph();
        altered.link_terms = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_quote = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.attachment_quote = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_attachment = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.root_release = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.bundle_profile = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.root_source_release = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_source_plane = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.bundle_source_spec = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_source_route = id(20);
        assert!(altered.validate().is_err());
        let mut altered = graph();
        altered.link_clock_policy = id(20);
        assert!(altered.validate().is_err());
    }
}
