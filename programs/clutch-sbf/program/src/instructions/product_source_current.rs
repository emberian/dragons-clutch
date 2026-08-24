//! Current Product V3/V5/V6 authority for Source occurrence publication.
//!
//! This module is the sole current successor join from hostile-authenticated
//! Product artifacts and Registry/Profile authority into liability-free Source
//! semantic inputs. Historical BundleV5/QuoteV4 publication remains decode-only
//! for successor builds and cannot construct these private receipts.

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::authenticate_product_artifact_v1;
use crate::instructions::product_series_current::AuthenticatedRegistryCapabilityV4;
use crate::instructions::product_series::{
    IX_SERIES_ARTIFACT_ATTACHMENT, IX_SERIES_ARTIFACT_BASIS,
    IX_SERIES_ARTIFACT_FUNDING_TERMS,
    IX_SERIES_ARTIFACT_GENESIS, IX_SERIES_ARTIFACT_PLAN,
    IX_SERIES_ARTIFACT_PRICE_POLICY, IX_SERIES_ARTIFACT_QUOTE,
    IX_SERIES_ARTIFACT_RECOVERY, IX_SERIES_ARTIFACT_TEMPLATE,
    SERIES_ARTIFACT_ACCOUNT_COUNT_V1,
};
use clutch_product_series::{
    assemble_compiled_product_series_bundle_v6, compile_source_semantic_inputs_v2,
    AuthenticatedSourceSeriesAuthorityV3, CompiledProductSeriesBundleV6,
    CompiledProductSeriesBundleV6Id,
    CompiledSourceOccurrenceV3, ComponentDebitV1, ContentId,
    EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductSeriesBundleInputsV6, ProductTemplateV4,
    RegistryCapabilityProjectionV2, SeriesAttachmentPlanV5, SeriesFundingComponentV2,
    SeriesFundingQuoteV5, SeriesFundingTermsV2, SeriesFundingTermsV2Id, SeriesPlanV5,
    SeriesPlanV5Id,
};
use clutch_source_plane_v3::{
    SourcePlaneProgramV3, StatisticKeyV3, StatisticKindV3, SummaryProgramV3, WindowSpecV3,
};
use clutch_source_plane_v3_runtime::{
    AuthenticatedPersistedSourcePolicyHandoffV1, AuthenticatedReceiverRouteV2,
    AuthenticatedSourceReleaseV1, AuthenticatedSourceRouteV1, RuntimeKey,
    SourcePolicyHandoffJoinV1, SuccessfulEvaluationHandoffV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SOURCE_PRODUCT_ROUTE_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/source-product-route-authentication/v4";
const SOURCE_SEMANTIC_PUBLICATION_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/source-semantic-publication-authentication/v2";

/// Current Product/Source compiler authority over one exact RegistryV4 and
/// Source ReleaseV2 join.
///
/// Its fields and constructor remain in this semantic-owner module. Current
/// Source publication cannot pair a valid RegistryV4 projection with either a
/// historical Product authority or a different authenticated Source release.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedProductSourceAuthorityV2 {
    registry_projection: RegistryCapabilityProjectionV2,
    summary_program: SummaryProgramV3,
    resolved_statistic: StatisticKindV3,
    resolved_coverage_policy_value: u16,
    source_release: AuthenticatedSourceReleaseV1,
}

impl AuthenticatedProductSourceAuthorityV2 {
    fn new(
        registry: &AuthenticatedRegistryCapabilityV4,
        source_release: AuthenticatedSourceReleaseV1,
    ) -> Outcome<Self> {
        let projection = registry.projection();
        let manifest = source_release.manifest();
        let source_plane_id = source_release
            .source_plane()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            source_plane_id.bytes() == projection.semantic_owners.source_plane_contract_id.bytes()
                && manifest.base.source_spec_id.bytes()
                    == projection.semantic_owners.source_spec_id.bytes()
                && registry.statistic_registry_value()
                    == projection.statistic_registry_value
                && registry.coverage_policy_registry_value()
                    == projection.coverage_policy_registry_value
                && registry.profile().rules.resolved_coverage_policy_value
                    == projection.coverage_policy_registry_value,
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            registry_projection: projection,
            summary_program: registry.profile().rules.summary_program,
            resolved_statistic: registry.resolved_statistic(),
            resolved_coverage_policy_value: registry
                .profile()
                .rules
                .resolved_coverage_policy_value,
            source_release,
        })
    }

    fn require_route(&self, route: AuthenticatedSourceProductRouteV4) -> Outcome<()> {
        require(
            self.source_release.manifest_id().bytes()
                == route.source_release_manifest_id.bytes()
                && self.source_release.id().bytes()
                    == route.source_release_authentication_id.bytes()
                && self
                    .source_release
                    .source_plane()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .bytes()
                    == route.source_plane_contract_id.bytes()
                && self.source_release.manifest().base.source_spec_id.bytes()
                    == route.source_spec_id.bytes(),
            ClutchError::MismatchedState,
        )
    }
}

impl AuthenticatedSourceSeriesAuthorityV3 for AuthenticatedProductSourceAuthorityV2 {
    fn authenticate_registry_projection(
        &self,
        projection: &RegistryCapabilityProjectionV2,
    ) -> clutch_product_series::Result<()> {
        if projection != &self.registry_projection {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticated_source_plane(
        &self,
        expected_contract_id: ContentId,
    ) -> clutch_product_series::Result<SourcePlaneProgramV3> {
        let source_plane = self.source_release.source_plane();
        if source_plane
            .id()
            .map_err(|_| clutch_product_series::Error::MismatchedArtifact)?
            .bytes()
            != expected_contract_id.bytes()
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(source_plane)
    }

    fn authenticate_source_spec(
        &self,
        expected_source_spec_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if self.source_release.manifest().base.source_spec_id.bytes()
            != expected_source_spec_id.bytes()
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn authenticated_summary_program(
        &self,
        expected_summary_program_id: ContentId,
    ) -> clutch_product_series::Result<SummaryProgramV3> {
        if self
            .summary_program
            .id()
            .map_err(|_| clutch_product_series::Error::MismatchedArtifact)?
            .bytes()
            != expected_summary_program_id.bytes()
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(self.summary_program)
    }

    fn resolve_statistic(
        &self,
        registry_release_id: ContentId,
        capability_profile_id: ContentId,
        statistic_registry_value: u16,
    ) -> clutch_product_series::Result<StatisticKindV3> {
        let projection = self.registry_projection;
        if registry_release_id != projection.registry_release_id
            || capability_profile_id != projection.capability_profile_id
            || statistic_registry_value != projection.statistic_registry_value
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(self.resolved_statistic)
    }

    fn resolve_coverage_policy(
        &self,
        registry_release_id: ContentId,
        capability_profile_id: ContentId,
        coverage_policy_registry_value: u16,
    ) -> clutch_product_series::Result<u16> {
        let projection = self.registry_projection;
        if registry_release_id != projection.registry_release_id
            || capability_profile_id != projection.capability_profile_id
            || coverage_policy_registry_value != projection.coverage_policy_registry_value
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(self.resolved_coverage_policy_value)
    }
}

/// Exact nine-account QuoteV5/AttachmentV5 artifact graph.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesSourceArtifactsV5 {
    series: Box<SeriesPlanV5>,
    funding_terms: Box<SeriesFundingTermsV2>,
    template: Box<ProductTemplateV4>,
    basis: Box<NativeClaimBasisV1>,
    recovery: Box<EvidenceOnlyRecoveryPolicyV1>,
    price_policy: Box<PriceMeasurePolicyV1>,
    genesis: Box<MarketGenesisProfileV2>,
    quote: Box<SeriesFundingQuoteV5>,
    attachment: Box<SeriesAttachmentPlanV5>,
}

impl AuthenticatedSeriesSourceArtifactsV5 {
    pub(crate) fn validate_registry_projection(
        &self,
        projection: &RegistryCapabilityProjectionV2,
    ) -> Outcome<()> {
        self.series
            .validate_bindings_v5(
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                &self.attachment,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.funding_terms
            .validate_bindings(
                &self.series,
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.quote
            .validate()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            self.quote.evidence_only_recovery_policy_id
                == self
                    .recovery
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .content_id()
                && self.attachment.funding_quote_id
                    == self
                        .quote
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            ClutchError::MismatchedState,
        )
    }

    pub(crate) fn series(&self) -> &SeriesPlanV5 {
        &self.series
    }

    pub(crate) fn funding_terms(&self) -> &SeriesFundingTermsV2 {
        &self.funding_terms
    }

    pub(crate) fn quote(&self) -> &SeriesFundingQuoteV5 {
        &self.quote
    }

    pub(crate) fn attachment(&self) -> &SeriesAttachmentPlanV5 {
        &self.attachment
    }
}

/// Hostile-authenticated current BundleV6 reconstructed from all semantic
/// owners rather than accepted as an independent caller claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedCompiledProductSeriesBundleV6 {
    artifact_account: Pubkey,
    bundle: CompiledProductSeriesBundleV6,
    bundle_id: CompiledProductSeriesBundleV6Id,
}

impl AuthenticatedCompiledProductSeriesBundleV6 {
    pub(crate) const fn artifact_account(self) -> Pubkey {
        self.artifact_account
    }

    pub(crate) const fn bundle(self) -> CompiledProductSeriesBundleV6 {
        self.bundle
    }

    pub(crate) const fn bundle_id(self) -> CompiledProductSeriesBundleV6Id {
        self.bundle_id
    }
}

/// Private Source/ProfileV4/BundleV6 route selected by current Product state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceProductRouteV4 {
    id: ContentId,
    source_route_id: ContentId,
    receiver_route_id: ContentId,
    source_release_manifest_id: ContentId,
    source_release_authentication_id: ContentId,
    source_plane_contract_id: ContentId,
    source_spec_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    market_genesis_profile_id: clutch_product_series::MarketGenesisProfileV2Id,
    realm_id: ContentId,
    profile_id: ContentId,
    collateral_mint: ContentId,
    collateral_token_program: ContentId,
}

impl AuthenticatedSourceProductRouteV4 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn source_route_id(self) -> ContentId {
        self.source_route_id
    }

    pub(crate) const fn source_release_manifest_id(self) -> ContentId {
        self.source_release_manifest_id
    }

    pub(crate) const fn source_release_authentication_id(self) -> ContentId {
        self.source_release_authentication_id
    }

    pub(crate) const fn source_plane_contract_id(self) -> ContentId {
        self.source_plane_contract_id
    }

    pub(crate) const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    pub(crate) const fn compiler_bundle_id(self) -> CompiledProductSeriesBundleV6Id {
        self.compiler_bundle_id
    }

    pub(crate) const fn registry_release_id(self) -> ContentId {
        self.registry_release_id
    }

    pub(crate) const fn capability_profile_id(self) -> ContentId {
        self.capability_profile_id
    }

    pub(crate) const fn realm_id(self) -> ContentId {
        self.realm_id
    }

    pub(crate) const fn profile_id(self) -> ContentId {
        self.profile_id
    }
}

/// Private authority to create or authenticate one exact ordinal's immutable
/// Source semantic accounts from BundleV6/QuoteV5/AttachmentV5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceSemanticPublicationV2 {
    id: ContentId,
    route: AuthenticatedSourceProductRouteV4,
    occurrence: CompiledSourceOccurrenceV3,
    window: WindowSpecV3,
    statistic_key: StatisticKeyV3,
    summary_program: SummaryProgramV3,
    source_work_funding: ComponentDebitV1,
}

impl AuthenticatedSourceSemanticPublicationV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn route(self) -> AuthenticatedSourceProductRouteV4 {
        self.route
    }

    pub(crate) const fn occurrence(self) -> CompiledSourceOccurrenceV3 {
        self.occurrence
    }

    pub(crate) const fn window(self) -> WindowSpecV3 {
        self.window
    }

    pub(crate) const fn statistic_key(self) -> StatisticKeyV3 {
        self.statistic_key
    }

    pub(crate) const fn summary_program(self) -> SummaryProgramV3 {
        self.summary_program
    }

    pub(crate) const fn source_work_funding(self) -> ComponentDebitV1 {
        self.source_work_funding
    }
}

/// Hostile-authenticate the current nine immutable Product artifacts.
pub(crate) fn authenticate_series_source_artifacts_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_series: SeriesPlanV5Id,
    expected_funding_terms: SeriesFundingTermsV2Id,
) -> Outcome<AuthenticatedSeriesSourceArtifactsV5> {
    require_count(accounts, SERIES_ARTIFACT_ACCOUNT_COUNT_V1)?;
    expected_series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expected_funding_terms
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let series = authenticate_product_artifact_v1::<SeriesPlanV5>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PLAN],
        expected_series.content_id(),
    )?
    .into_value();
    let funding_terms = authenticate_product_artifact_v1::<SeriesFundingTermsV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_FUNDING_TERMS],
        expected_funding_terms.content_id(),
    )?
    .into_value();
    require(
        series
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected_series
            && funding_terms
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == expected_funding_terms
            && funding_terms.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    let template = authenticate_product_artifact_v1::<ProductTemplateV4>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_TEMPLATE],
        series.product_template_id.content_id(),
    )?
    .into_value();
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_GENESIS],
        series.market_genesis_profile_id.content_id(),
    )?
    .into_value();
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV5>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_ATTACHMENT],
        series.attachment_plan_id.content_id(),
    )?
    .into_value();
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_BASIS],
        template.native_claim_basis_id.content_id(),
    )?
    .into_value();
    let recovery = authenticate_product_artifact_v1::<EvidenceOnlyRecoveryPolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_RECOVERY],
        template.evidence_only_recovery_policy_id.content_id(),
    )?
    .into_value();
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PRICE_POLICY],
        genesis.price_measure_policy_id.content_id(),
    )?
    .into_value();
    let quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV5>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_QUOTE],
        attachment.funding_quote_id.content_id(),
    )?
    .into_value();
    require(
        template
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.product_template_id
            && genesis
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == series.market_genesis_profile_id
            && attachment
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .content_id()
                == series.attachment_plan_id.content_id()
            && recovery
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == template.evidence_only_recovery_policy_id
            && price_policy
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == genesis.price_measure_policy_id
            && quote
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == attachment.funding_quote_id
            && quote.evidence_only_recovery_policy_id
                == template.evidence_only_recovery_policy_id.content_id(),
        ClutchError::MismatchedState,
    )?;
    template
        .validate_bindings(&basis, &recovery)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    genesis
        .validate_bindings(&basis, &price_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    quote
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedSeriesSourceArtifactsV5 {
        series,
        funding_terms,
        template,
        basis,
        recovery,
        price_policy,
        genesis,
        quote,
        attachment,
    })
}

/// Reconstruct and hostile-authenticate the sole current BundleV6 graph.
pub(crate) fn authenticate_compiled_product_series_bundle_v6(
    program_id: &Pubkey,
    bundle_account: &AccountInfo<'_>,
    registry: &AuthenticatedRegistryCapabilityV4,
    source_release: AuthenticatedSourceReleaseV1,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
) -> Outcome<AuthenticatedCompiledProductSeriesBundleV6> {
    artifacts.validate_registry_projection(&registry.projection())?;
    let expected = assemble_compiled_product_series_bundle_v6(ProductSeriesBundleInputsV6 {
        registry: &registry.projection(),
        source_release_manifest_id: ContentId::from_bytes(source_release.manifest_id().bytes()),
        basis: &artifacts.basis,
        recovery: &artifacts.recovery,
        template: &artifacts.template,
        price_policy: &artifacts.price_policy,
        genesis: &artifacts.genesis,
        funding_quote: &artifacts.quote,
        attachment: &artifacts.attachment,
        series: &artifacts.series,
        funding_terms: &artifacts.funding_terms,
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_id = expected
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let decoded = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id,
        bundle_account,
        expected_id.content_id(),
    )?;
    require(*decoded.value() == expected, ClutchError::MismatchedState)?;
    Ok(AuthenticatedCompiledProductSeriesBundleV6 {
        artifact_account: *bundle_account.key,
        bundle: expected,
        bundle_id: expected_id,
    })
}

/// Join exact Source deployment/receiver authority to current Product V6.
pub(crate) fn authenticate_source_product_route_v4(
    route: AuthenticatedSourceRouteV1,
    receiver: AuthenticatedReceiverRouteV2,
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
) -> Outcome<AuthenticatedSourceProductRouteV4> {
    let bundle_value = bundle.bundle();
    let genesis_id = artifacts
        .genesis
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projection = registry.projection();
    let owners = registry.semantic_owners();
    let collateral = registry.realm_collateral();
    require(
        receiver.route_id() == route.route_id()
            && bundle_value.source_release_manifest_id.bytes()
                == route.release_manifest_id().bytes()
            && bundle_value.source_plane_contract_id.bytes()
                == route.source_plane_contract_id().bytes()
            && bundle_value.source_spec_id.bytes() == route.source_spec_id().bytes()
            && bundle_value.registry_release_id == registry.registry_release_id()
            && bundle_value.capability_profile_id.content_id()
                == registry.capability_profile_id()
            && bundle_value.source_plane_contract_id == owners.source_plane_contract_id
            && bundle_value.source_spec_id == owners.source_spec_id
            && bundle_value.market_genesis_profile_id == genesis_id
            && artifacts.genesis.capability_profile_id == registry.capability_profile_id()
            && artifacts.genesis.realm_id == collateral.realm_id
            && artifacts.genesis.profile_id == collateral.profile_id
            && projection.realm_collateral == collateral,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_PRODUCT_ROUTE_AUTHENTICATION_DOMAIN_V4,
            &route.route_id().bytes(),
            &receiver.id().bytes(),
            &route.release_manifest_id().bytes(),
            &route.release_authentication_id().bytes(),
            &route.source_plane_contract_id().bytes(),
            &route.source_spec_id().bytes(),
            &registry.registry_release_id().bytes(),
            &registry.capability_profile_id().bytes(),
            &bundle.bundle_id().bytes(),
            &genesis_id.bytes(),
            &collateral.realm_id.bytes(),
            &collateral.profile_id.bytes(),
            &collateral.collateral_mint.bytes(),
            &collateral.token_program.bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceProductRouteV4 {
        id,
        source_route_id: ContentId::from_bytes(route.route_id().bytes()),
        receiver_route_id: ContentId::from_bytes(receiver.id().bytes()),
        source_release_manifest_id: ContentId::from_bytes(route.release_manifest_id().bytes()),
        source_release_authentication_id: ContentId::from_bytes(
            route.release_authentication_id().bytes(),
        ),
        source_plane_contract_id: ContentId::from_bytes(route.source_plane_contract_id().bytes()),
        source_spec_id: ContentId::from_bytes(route.source_spec_id().bytes()),
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        compiler_bundle_id: bundle.bundle_id(),
        market_genesis_profile_id: genesis_id,
        realm_id: collateral.realm_id,
        profile_id: collateral.profile_id,
        collateral_mint: collateral.collateral_mint,
        collateral_token_program: collateral.token_program,
    })
}

/// Recompile the sole Source semantic graph for one V6 Series ordinal.
pub(crate) fn authenticate_source_semantic_publication_v2(
    route: AuthenticatedSourceProductRouteV4,
    source_release: AuthenticatedSourceReleaseV1,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    registry: &AuthenticatedRegistryCapabilityV4,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV6,
    ordinal: u32,
) -> Outcome<AuthenticatedSourceSemanticPublicationV2> {
    let projection = registry.projection();
    let authority = AuthenticatedProductSourceAuthorityV2::new(registry, source_release)?;
    authority.require_route(route)?;
    require(
        route.registry_release_id == registry.registry_release_id()
            && route.capability_profile_id == registry.capability_profile_id()
            && route.compiler_bundle_id == registry.compiler_bundle_id()
            && compiler_bundle.bundle_id() == registry.compiler_bundle_id(),
        ClutchError::MismatchedState,
    )?;
    artifacts.validate_registry_projection(&projection)?;
    let compiled = compile_source_semantic_inputs_v2(
        &authority,
        &artifacts.series,
        &artifacts.template,
        &artifacts.basis,
        &artifacts.recovery,
        &artifacts.price_policy,
        &artifacts.genesis,
        &artifacts.attachment,
        &projection,
        ordinal,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let occurrence_id = compiled
        .occurrence
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let window_id = compiled
        .window
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let statistic_key_id = compiled
        .statistic_key
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let summary_program_id = compiled
        .summary_program
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_work_funding =
        artifacts.quote.components[SeriesFundingComponentV2::SourceWork.index()];
    let bundle = compiler_bundle.bundle();
    require(
        compiler_bundle.bundle_id() == route.compiler_bundle_id
            && bundle.capability_profile_id.content_id() == route.capability_profile_id
            && bundle.source_release_manifest_id == route.source_release_manifest_id
            && bundle.source_plane_contract_id == route.source_plane_contract_id
            && bundle.source_spec_id == route.source_spec_id
            && artifacts.genesis.realm_id == route.realm_id
            && artifacts.genesis.profile_id == route.profile_id
            && compiled.window.source_plane_program_id.bytes()
                == route.source_plane_contract_id.bytes()
            && compiled.window.source_spec_id.bytes() == route.source_spec_id.bytes()
            && compiled.occurrence.source_window_id.bytes() == window_id.bytes()
            && compiled.occurrence.statistic_key_id.bytes() == statistic_key_id.bytes()
            && compiled.statistic_key.window_id == window_id
            && compiled.statistic_key.summary_program_id == summary_program_id,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_SEMANTIC_PUBLICATION_AUTHENTICATION_DOMAIN_V2,
            &route.id.bytes(),
            &occurrence_id.bytes(),
            &window_id.bytes(),
            &statistic_key_id.bytes(),
            &summary_program_id.bytes(),
            &source_work_funding.lamports.to_le_bytes(),
            &source_work_funding.collateral_atoms.to_le_bytes(),
            &ordinal.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceSemanticPublicationV2 {
        id,
        route,
        occurrence: compiled.occurrence,
        window: compiled.window,
        statistic_key: compiled.statistic_key,
        summary_program: compiled.summary_program,
        source_work_funding,
    })
}

/// Create or authenticate the exact content-addressed current Source inputs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_source_semantic_inputs_v2(
    program_id: &Pubkey,
    publication: AuthenticatedSourceSemanticPublicationV2,
    custody: crate::source_plane_v3_actions::AuthenticatedSourceFundingCustodyV1,
    custody_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    summary_account: &AccountInfo<'_>,
    statistic_key_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<crate::source_plane_v3_actions::PublishedSourceSemanticInputsV1> {
    crate::source_plane_v3_actions::publish_authenticated_source_semantic_inputs(
        program_id,
        publication.id,
        publication.window,
        publication.summary_program,
        publication.statistic_key,
        custody,
        custody_account,
        window_account,
        summary_account,
        statistic_key_account,
        system_program,
        rent_sysvar,
    )
}

/// Successful Source handoff bound to the current BundleV6 Product route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceResolutionInputV4 {
    id: ContentId,
    route: AuthenticatedSourceProductRouteV4,
    source_handoff_authentication_id: ContentId,
    persisted_handoff_authentication_id: ContentId,
    persisted_handoff_account: RuntimeKey,
    successful_evaluation_handoff_id: ContentId,
    occurrence_account: RuntimeKey,
    result_account: RuntimeKey,
    result_account_data_id: ContentId,
    result_account_authentication_id: ContentId,
    work_receipt_authentication_id: ContentId,
    failure_policy_binding_id: ContentId,
    market_instance_id: ContentId,
    source_repair_generation: u64,
    window_id: ContentId,
    statistic_key_id: ContentId,
}

impl AuthenticatedSourceResolutionInputV4 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn route(self) -> AuthenticatedSourceProductRouteV4 {
        self.route
    }

    pub(crate) const fn source_handoff_authentication_id(self) -> ContentId {
        self.source_handoff_authentication_id
    }

    pub(crate) const fn persisted_handoff_authentication_id(self) -> ContentId {
        self.persisted_handoff_authentication_id
    }

    pub(crate) const fn persisted_handoff_account(self) -> RuntimeKey {
        self.persisted_handoff_account
    }

    pub(crate) const fn successful_evaluation_handoff_id(self) -> ContentId {
        self.successful_evaluation_handoff_id
    }

    pub(crate) const fn occurrence_account(self) -> RuntimeKey {
        self.occurrence_account
    }

    pub(crate) const fn result_account(self) -> RuntimeKey {
        self.result_account
    }

    pub(crate) const fn result_account_data_id(self) -> ContentId {
        self.result_account_data_id
    }

    pub(crate) const fn result_account_authentication_id(self) -> ContentId {
        self.result_account_authentication_id
    }

    pub(crate) const fn work_receipt_authentication_id(self) -> ContentId {
        self.work_receipt_authentication_id
    }

    pub(crate) const fn failure_policy_binding_id(self) -> ContentId {
        self.failure_policy_binding_id
    }

    pub(crate) const fn market_instance_id(self) -> ContentId {
        self.market_instance_id
    }

    pub(crate) const fn source_repair_generation(self) -> u64 {
        self.source_repair_generation
    }

    pub(crate) const fn window_id(self) -> ContentId {
        self.window_id
    }

    pub(crate) const fn statistic_key_id(self) -> ContentId {
        self.statistic_key_id
    }
}

/// Bind Source's successful action-10 postwrite to BundleV6 without accepting
/// any caller-selected result, Market, Window, or Statistic identity.
pub(crate) fn authenticate_source_resolution_input_v4(
    route: AuthenticatedSourceProductRouteV4,
    handoff: SuccessfulEvaluationHandoffV1,
    source: SourcePolicyHandoffJoinV1,
    persisted: AuthenticatedPersistedSourcePolicyHandoffV1,
) -> Outcome<AuthenticatedSourceResolutionInputV4> {
    let occurrence = handoff.occurrence();
    require(
        source.handoff_id() == handoff.id()
            && source.release_authentication_id().bytes()
                == route.source_release_authentication_id.bytes()
            && source.route_id().bytes() == route.source_route_id.bytes()
            && source.source_spec_id().bytes() == route.source_spec_id.bytes()
            && occurrence.route_id().bytes() == route.source_route_id.bytes()
            && occurrence.source_plane_contract_id().bytes()
                == route.source_plane_contract_id.bytes()
            && occurrence.source_spec_id().bytes() == route.source_spec_id.bytes()
            && source.occurrence_account() == occurrence.occurrence_account()
            && source.source_fact_authentication_id()
                == handoff.result_account_authentication_id()
            && source.failure_policy_binding_id() == handoff.failure_policy_binding_id()
            && source.window_id() == occurrence.window_id()
            && source.statistic_key_id() == occurrence.statistic_key_id()
            && source.clock_policy_id() == handoff.clock_policy_id()
            && source.clock() == handoff.clock()
            && !handoff.result_account_data_id().is_zero()
            && persisted.source_policy_handoff_join_id() == source.id(),
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/source-resolution-input/v4",
            &route.id.bytes(),
            &source.id().bytes(),
            &persisted.id().bytes(),
            &persisted.account().bytes(),
            &handoff.id().bytes(),
            &occurrence.id().bytes(),
            &source.occurrence_account().bytes(),
            &source.result_or_absence_account().bytes(),
            &handoff.result_account_data_id().bytes(),
            &handoff.result_account_authentication_id().bytes(),
            &source.work_receipt_authentication_id().bytes(),
            &handoff.failure_policy_binding_id().bytes(),
            &occurrence.market_instance_id().bytes(),
            &occurrence.repair_generation().to_le_bytes(),
            &occurrence.window_id().bytes(),
            &occurrence.statistic_key_id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceResolutionInputV4 {
        id,
        route,
        source_handoff_authentication_id: ContentId::from_bytes(source.id().bytes()),
        persisted_handoff_authentication_id: ContentId::from_bytes(persisted.id().bytes()),
        persisted_handoff_account: persisted.account(),
        successful_evaluation_handoff_id: ContentId::from_bytes(handoff.id().bytes()),
        occurrence_account: source.occurrence_account(),
        result_account: source.result_or_absence_account(),
        result_account_data_id: ContentId::from_bytes(handoff.result_account_data_id().bytes()),
        result_account_authentication_id: ContentId::from_bytes(
            handoff.result_account_authentication_id().bytes(),
        ),
        work_receipt_authentication_id: ContentId::from_bytes(
            source.work_receipt_authentication_id().bytes(),
        ),
        failure_policy_binding_id: ContentId::from_bytes(
            handoff.failure_policy_binding_id().bytes(),
        ),
        market_instance_id: ContentId::from_bytes(occurrence.market_instance_id().bytes()),
        source_repair_generation: occurrence.repair_generation(),
        window_id: ContentId::from_bytes(occurrence.window_id().bytes()),
        statistic_key_id: ContentId::from_bytes(occurrence.statistic_key_id().bytes()),
    })
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    #[test]
    fn current_publication_has_no_bundle_v5_or_quote_v4_authority() {
        let source = include_str!("product_source_current.rs");
        assert!(source.contains("CompiledProductSeriesBundleV6"));
        assert!(source.contains("SeriesFundingQuoteV5"));
        assert!(source.contains("SeriesAttachmentPlanV5"));
        assert!(source.contains("compile_source_semantic_inputs_v2"));
        assert!(source.contains("AuthenticatedRegistryCapabilityV4"));
        assert!(source.contains("AuthenticatedProductSourceAuthorityV2"));
        assert!(source.contains("authority.require_route(route)?"));
        assert!(!source.contains("CompiledProductSeriesBundleV5"));
        assert!(!source.contains("SeriesFundingQuoteV4"));
        assert!(!source.contains("SeriesAttachmentPlanV4"));
        assert!(!source.contains("AuthenticatedRegistryCapabilityV3"));
        assert!(!source.contains("AuthenticatedProductSourceAuthorityV1"));
    }
}
