use clutch_bspline::EdgePolicy;
use clutch_bspline_shape_compiler::exact_market::{
    bind_exact_market_bundle_v7, compile_exact_market_v1, ExactMarketBundleSidecarV3,
    ExactMarketCompilerErrorV1, ExactMarketCompilerRequestV1,
    ExactMarketCoordinateCoverageV1, ExactMarketManifestErrorV1, ExactMarketSearchOutcomeV1,
    ExactMarketWorkManifestV1, COMPILED_PRODUCT_SERIES_BUNDLE_V7_ARTIFACT_KIND,
    EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V3, EXACT_MARKET_WORK_MANIFEST_BYTES_V1,
};
use clutch_bspline_shape_compiler::production::{
    compile_production_payoff_v1, ExactCategoricalPayoffDefinitionV1,
    ExactSmoothPayoffDefinitionV1, ProductionPayoffDefinitionV1,
    SmoothNativeBasisDefinitionV1,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV7, ContentId, EvidenceOnlyRecoveryPolicyId,
    MarketGenesisProfileV2Id, PriceMeasurePolicyV1Id, ProductTemplateId,
    RegistryCapabilityProfileV4Id, SeriesAttachmentPlanV6Id, SeriesFundingQuoteV6Id,
    SeriesFundingTermsV2Id, SeriesPlanV5Id, PAYOUT_MAP_UNUSED,
};
use num_bigint::BigInt;
use num_rational::BigRational;

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn rat(numerator: u64, denominator: u64) -> BigRational {
    BigRational::new(BigInt::from(numerator), BigInt::from(denominator))
}

fn smooth_definition(degree: u8) -> ExactSmoothPayoffDefinitionV1 {
    let (knots, controls) = match degree {
        1 => (
            vec![0, 2, 4, 6],
            vec![rat(0, 1), rat(1, 4), rat(1, 2), rat(1, 1)],
        ),
        3 => (
            vec![0, 4, 8],
            vec![
                rat(0, 1),
                rat(1, 4),
                rat(1, 2),
                rat(3, 4),
                rat(1, 1),
            ],
        ),
        _ => unreachable!(),
    };
    ExactSmoothPayoffDefinitionV1 {
        basis: SmoothNativeBasisDefinitionV1 {
            degree,
            coordinate_domain_min: 0,
            coordinate_domain_max: *knots.last().unwrap(),
            payout_denominator: 32,
            knots,
            resolved_edge_policy: EdgePolicy::Refuse,
            ambiguity_policy_registry_value: 1,
            edge_policy_registry_value: 1,
        },
        control_values: controls,
        maximum_liability: rat(1, 1),
    }
}

fn compiled_degree_three(
) -> clutch_bspline_shape_compiler::production::CompiledProductionPayoffV1 {
    compile_production_payoff_v1(
        id(2),
        ProductionPayoffDefinitionV1::ExactSmooth(smooth_definition(3)),
    )
    .unwrap()
}

fn bundle_v7(
    compiled: &clutch_bspline_shape_compiler::production::CompiledProductionPayoffV1,
) -> CompiledProductSeriesBundleV7 {
    CompiledProductSeriesBundleV7 {
        registry_release_id: id(10),
        capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes([11; 32]),
        source_release_manifest_id: id(12),
        source_plane_contract_id: id(13),
        source_spec_id: id(14),
        summary_program_id: id(15),
        product_compiler_release_id: id(16),
        native_claim_basis_id: compiled.native_claim_basis_id,
        evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes([17; 32]),
        product_template_id: ProductTemplateId::from_bytes([18; 32]),
        price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes([19; 32]),
        market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes(id(2).bytes()),
        funding_quote_id: SeriesFundingQuoteV6Id::from_bytes([21; 32]),
        attachment_plan_id: SeriesAttachmentPlanV6Id::from_bytes([22; 32]),
        series_plan_id: SeriesPlanV5Id::from_bytes([23; 32]),
        funding_terms_id: SeriesFundingTermsV2Id::from_bytes([24; 32]),
    }
}

fn request(
    prices: &[u64],
    coordinates: &[u128],
    budget: u64,
) -> ExactMarketCompilerRequestV1 {
    ExactMarketCompilerRequestV1::new(id(1), id(2), id(4), prices, coordinates, budget)
        .unwrap()
}

#[test]
fn all_support_solution_emits_canonical_certificate_and_work_manifest() {
    let compiled = compiled_degree_three();
    let output = compile_exact_market_v1(
        &compiled,
        request(&[7, 6, 6, 6, 7], &[0, 2, 4, 6, 8], 10),
    )
    .unwrap();
    assert_eq!(output.manifest.outcome(), ExactMarketSearchOutcomeV1::Solved);
    assert_eq!(
        output.manifest.coverage(),
        ExactMarketCoordinateCoverageV1::DeclaredCoordinateSubset
    );
    assert_eq!(output.manifest.solution_support(), 5);
    assert_eq!(output.manifest.exhausted_through_support(), 4);
    assert_eq!(output.manifest.evaluations_for_support(1), Some(5));
    assert_eq!(output.manifest.evaluations_for_support(2), Some(10));
    assert_eq!(output.manifest.evaluations_for_support(3), Some(10));
    assert_eq!(output.manifest.evaluations_for_support(4), Some(5));
    assert_eq!(output.manifest.evaluations_for_support(5), Some(1));
    assert!(!output.manifest.is_complete_full_domain_negative());
    assert!(output.certificate_bytes.is_some());
    assert_eq!(
        ExactMarketWorkManifestV1::decode(&output.manifest_bytes).unwrap(),
        output.manifest
    );
    assert_eq!(output.manifest.content_id().unwrap(), output.manifest_id);
    output.verify(&compiled).unwrap();
}

#[test]
fn declared_subset_negative_never_claims_complete_terms_domain() {
    let compiled = compiled_degree_three();
    let output = compile_exact_market_v1(
        &compiled,
        request(&[0, 32, 0, 0, 0], &[0, 8], 100),
    )
    .unwrap();
    assert_eq!(
        output.manifest.outcome(),
        ExactMarketSearchOutcomeV1::Unsupported
    );
    assert_eq!(
        output.manifest.coverage(),
        ExactMarketCoordinateCoverageV1::DeclaredCoordinateSubset
    );
    assert_eq!(output.manifest.exhausted_through_support(), 5);
    assert!(!output.manifest.is_complete_full_domain_negative());
    assert!(output.certificate_bytes.is_none());
}

#[test]
fn complete_integer_domain_negative_is_explicit_and_narrow() {
    let compiled = compiled_degree_three();
    let output = compile_exact_market_v1(
        &compiled,
        request(
            &[0, 32, 0, 0, 0],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8],
            1_000,
        ),
    )
    .unwrap();
    assert_eq!(
        output.manifest.outcome(),
        ExactMarketSearchOutcomeV1::Unsupported
    );
    assert_eq!(
        output.manifest.coverage(),
        ExactMarketCoordinateCoverageV1::FullIntegerDomain
    );
    assert!(output.manifest.is_complete_full_domain_negative());
}

#[test]
fn work_budget_is_a_terminal_fact_not_an_exhaustive_negative() {
    let compiled = compiled_degree_three();
    let output = compile_exact_market_v1(
        &compiled,
        request(&[0, 32, 0, 0, 0], &[0, 1, 2, 3, 4, 8], 1),
    )
    .unwrap();
    assert_eq!(
        output.manifest.outcome(),
        ExactMarketSearchOutcomeV1::WorkTruncated
    );
    assert_eq!(output.manifest.exhausted_through_support(), 1);
    assert_eq!(output.manifest.truncated_support(), 2);
    assert_eq!(output.manifest.evaluations_for_support(2), Some(1));
    assert!(!output.manifest.is_complete_full_domain_negative());
    assert!(output.certificate_bytes.is_none());
}

#[test]
fn compiler_refuses_unjoined_widths_and_unsupported_product_bases() {
    let compiled = compiled_degree_three();
    assert_eq!(
        compile_exact_market_v1(
            &compiled,
            request(&[16, 16, 0, 0], &[0, 8], 10),
        ),
        Err(ExactMarketCompilerErrorV1::InvalidPriceWidth)
    );

    let degree_one = compile_production_payoff_v1(
        id(2),
        ProductionPayoffDefinitionV1::ExactSmooth(smooth_definition(1)),
    )
    .unwrap();
    assert_eq!(
        compile_exact_market_v1(
            &degree_one,
            request(&[8, 8, 8, 8], &[0, 2, 4, 6], 10),
        ),
        Err(ExactMarketCompilerErrorV1::UnsupportedBasisProfile)
    );

    let categorical = ExactCategoricalPayoffDefinitionV1 {
        coordinate_domain_min: 0,
        coordinate_domain_max: 2,
        knots: vec![1],
        cell_payouts: vec![vec![rat(1, 1), rat(0, 1)], vec![rat(0, 1), rat(1, 1)]],
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
    };
    let categorical = compile_production_payoff_v1(
        id(2),
        ProductionPayoffDefinitionV1::ExactCategorical(categorical),
    )
    .unwrap();
    assert_eq!(categorical.native_claim_basis.payout_map[2], PAYOUT_MAP_UNUSED);
    assert_eq!(
        compile_exact_market_v1(
            &categorical,
            request(&[16, 16], &[0, 1, 2], 10),
        ),
        Err(ExactMarketCompilerErrorV1::UnsupportedBasisProfile)
    );
}

#[test]
fn hostile_manifest_refuses_reserved_padding_coverage_and_report_fabrication() {
    let output = compile_exact_market_v1(
        &compiled_degree_three(),
        request(&[7, 6, 6, 6, 7], &[0, 2, 4, 6, 8], 10),
    )
    .unwrap();

    assert_eq!(
        ExactMarketWorkManifestV1::decode(
            &output.manifest_bytes[..EXACT_MARKET_WORK_MANIFEST_BYTES_V1 - 1]
        ),
        Err(ExactMarketManifestErrorV1::InvalidLength)
    );

    let mut reserved = output.manifest_bytes;
    reserved[20] = 1;
    assert_eq!(
        ExactMarketWorkManifestV1::decode(&reserved),
        Err(ExactMarketManifestErrorV1::NonCanonicalPadding)
    );

    let mut fabricated_full_domain = output.manifest_bytes;
    fabricated_full_domain[13] = 1;
    assert_eq!(
        ExactMarketWorkManifestV1::decode(&fabricated_full_domain),
        Err(ExactMarketManifestErrorV1::CoverageMismatch)
    );

    let mut fabricated_exhaustion = output.manifest_bytes;
    fabricated_exhaustion[18] = 5;
    assert_eq!(
        ExactMarketWorkManifestV1::decode(&fabricated_exhaustion),
        Err(ExactMarketManifestErrorV1::InvalidReport)
    );
}

#[test]
fn certificate_and_terms_tampering_fail_closed() {
    let compiled = compiled_degree_three();
    let mut output = compile_exact_market_v1(
        &compiled,
        request(&[7, 6, 6, 6, 7], &[0, 2, 4, 6, 8], 10),
    )
    .unwrap();
    output.certificate_bytes.as_mut().unwrap()[0] ^= 1;
    assert_eq!(
        output.verify(&compiled),
        Err(ExactMarketCompilerErrorV1::OutputMismatch)
    );

    let different_terms = compile_production_payoff_v1(
        id(9),
        ProductionPayoffDefinitionV1::ExactSmooth(smooth_definition(3)),
    )
    .unwrap();
    assert!(output.verify(&different_terms).is_err());
}

#[test]
fn current_bundle_sidecar_is_fixed_canonical_and_reopens_every_join() {
    let compiled = compiled_degree_three();
    let bundle = bundle_v7(&compiled);
    let exact_market = compile_exact_market_v1(
        &compiled,
        request(&[7, 6, 6, 6, 7], &[0, 2, 4, 6, 8], 10),
    )
    .unwrap();
    let sidecar = bind_exact_market_bundle_v7(&compiled, &bundle, &exact_market).unwrap();
    assert_eq!(
        sidecar.bundle_artifact_kind(),
        COMPILED_PRODUCT_SERIES_BUNDLE_V7_ARTIFACT_KIND
    );
    assert!(sidecar.bundle_artifact_context().is_zero());
    assert_eq!(sidecar.bundle_v7_id(), bundle.id().unwrap());
    assert_eq!(sidecar.work_manifest_id(), exact_market.manifest_id);
    assert_eq!(
        sidecar.certificate_output_id(),
        exact_market.manifest.certificate_output_id()
    );
    let mut bytes = [0_u8; EXACT_MARKET_BUNDLE_SIDECAR_BYTES_V3];
    sidecar.encode_into(&mut bytes).unwrap();
    assert_eq!(ExactMarketBundleSidecarV3::decode(&bytes), Ok(sidecar));
    sidecar.verify(&compiled, &bundle, &exact_market).unwrap();

    let mut wrong_kind = bytes;
    wrong_kind[12] = 56;
    assert_eq!(
        ExactMarketBundleSidecarV3::decode(&wrong_kind),
        Err(ExactMarketManifestErrorV1::InvalidDiscriminant)
    );
    let mut nonzero_context = bytes;
    nonzero_context[16] = 1;
    assert_eq!(
        ExactMarketBundleSidecarV3::decode(&nonzero_context),
        Err(ExactMarketManifestErrorV1::NonCanonicalPadding)
    );
}

#[test]
fn bundle_sidecar_refuses_a_parallel_basis_truth() {
    let compiled = compiled_degree_three();
    let mut bundle = bundle_v7(&compiled);
    bundle.native_claim_basis_id =
        clutch_product_series::NativeClaimBasisId::from_bytes([99; 32]);
    let exact_market = compile_exact_market_v1(
        &compiled,
        request(&[7, 6, 6, 6, 7], &[0, 2, 4, 6, 8], 10),
    )
    .unwrap();
    assert_eq!(
        bind_exact_market_bundle_v7(&compiled, &bundle, &exact_market),
        Err(ExactMarketCompilerErrorV1::OutputMismatch)
    );
}
