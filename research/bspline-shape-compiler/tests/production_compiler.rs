use clutch_bspline::EdgePolicy;
use clutch_bspline_shape_compiler::production::{
    compile_production_payoff_v1, AnalyticSmoothPayoffDefinitionV1,
    ExactCategoricalPayoffDefinitionV1, ExactSmoothPayoffCertificateV1,
    ExactSmoothPayoffDefinitionV1, ProductionCompilerError, ProductionPayoffDefinitionV1,
    ProductionPayoffEvidenceV1, SmoothNativeBasisDefinitionV1,
};
use clutch_bspline_shape_compiler::{Shape, SpanStatus};
use clutch_product_series::{ContentId, FixedCodec, NativeClaimBasisV1, PAYOUT_MAP_UNUSED};
use num_bigint::BigInt;
use num_rational::BigRational;

fn terms_id() -> ContentId {
    ContentId::from_bytes([9; 32])
}

fn rat(numerator: u64, denominator: u64) -> BigRational {
    BigRational::new(BigInt::from(numerator), BigInt::from(denominator))
}

fn categorical() -> ExactCategoricalPayoffDefinitionV1 {
    ExactCategoricalPayoffDefinitionV1 {
        coordinate_domain_min: 0,
        coordinate_domain_max: 30,
        knots: vec![10, 20],
        cell_payouts: vec![
            vec![rat(1, 2), rat(1, 3), rat(1, 6)],
            vec![rat(1, 2), rat(1, 3), rat(1, 6)],
            vec![rat(0, 1), rat(1, 1), rat(0, 1)],
        ],
        ambiguity_policy_registry_value: 3,
        edge_policy_registry_value: 4,
    }
}

fn smooth(degree: u8) -> SmoothNativeBasisDefinitionV1 {
    let knots = match degree {
        1 => vec![0, 2, 4, 6],
        2 => vec![0, 2, 4],
        3 => vec![0, 2],
        _ => unreachable!(),
    };
    let coordinate_domain_max = *knots.last().unwrap();
    SmoothNativeBasisDefinitionV1 {
        degree,
        coordinate_domain_min: 0,
        coordinate_domain_max,
        payout_denominator: 12,
        knots,
        resolved_edge_policy: EdgePolicy::Clamp,
        ambiguity_policy_registry_value: 3,
        edge_policy_registry_value: 4,
    }
}

fn exact_smooth(degree: u8) -> ExactSmoothPayoffDefinitionV1 {
    ExactSmoothPayoffDefinitionV1 {
        basis: smooth(degree),
        control_values: vec![rat(0, 1), rat(1, 3), rat(2, 3), rat(1, 1)],
        maximum_liability: rat(1, 1),
    }
}

#[test]
fn categorical_rationals_emit_minimal_canonical_product_rows() {
    let output = compile_production_payoff_v1(
        terms_id(),
        ProductionPayoffDefinitionV1::ExactCategorical(categorical()),
    )
    .unwrap();
    let basis = output.native_claim_basis;
    assert_eq!(basis.basis_degree, 0);
    assert_eq!(basis.outcome_count, 3);
    assert_eq!(basis.payout_count, 2);
    assert_eq!(basis.denominator, 6);
    assert_eq!(basis.payout_weights[0][..3], [3, 2, 1]);
    assert_eq!(basis.payout_weights[1][..3], [0, 6, 0]);
    assert_eq!(basis.payout_map[..3], [0, 0, 1]);
    assert!(basis.payout_map[3..]
        .iter()
        .all(|value| *value == PAYOUT_MAP_UNUSED));
    assert_eq!(
        NativeClaimBasisV1::decode(&output.native_claim_basis_bytes).unwrap(),
        basis
    );
    assert_eq!(output.native_claim_basis_id, basis.id().unwrap());
    assert_eq!(
        output.evidence,
        ProductionPayoffEvidenceV1::ExactCategoricalBasis
    );
    output.verify(terms_id()).unwrap();
}

#[test]
fn categorical_refuses_non_simplex_negative_and_u64_overflow() {
    let mut bad_sum = categorical();
    bad_sum.cell_payouts[0][0] = rat(1, 3);
    assert_eq!(
        compile_production_payoff_v1(
            terms_id(),
            ProductionPayoffDefinitionV1::ExactCategorical(bad_sum)
        ),
        Err(ProductionCompilerError::InvalidCategoricalSimplex)
    );

    let mut negative = categorical();
    negative.cell_payouts[0][0] = BigRational::from_integer(BigInt::from(-1));
    assert_eq!(
        compile_production_payoff_v1(
            terms_id(),
            ProductionPayoffDefinitionV1::ExactCategorical(negative)
        ),
        Err(ProductionCompilerError::InvalidCategoricalSimplex)
    );

    let huge = BigInt::from(1_u8) << 64_u8;
    let epsilon = BigRational::new(BigInt::from(1_u8), huge.clone());
    let almost_one = BigRational::new(huge - BigInt::from(1_u8), BigInt::from(1_u8) << 64_u8);
    let overflow = ExactCategoricalPayoffDefinitionV1 {
        coordinate_domain_min: 0,
        coordinate_domain_max: 2,
        knots: vec![1],
        cell_payouts: vec![
            vec![epsilon.clone(), almost_one.clone()],
            vec![almost_one, epsilon],
        ],
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
    };
    assert_eq!(
        compile_production_payoff_v1(
            terms_id(),
            ProductionPayoffDefinitionV1::ExactCategorical(overflow)
        ),
        Err(ProductionCompilerError::RationalIntegerizationOverflow)
    );
}

#[test]
fn exact_rational_smooth_payoffs_round_trip_for_every_smooth_degree() {
    for degree in [1, 2, 3] {
        let output = compile_production_payoff_v1(
            terms_id(),
            ProductionPayoffDefinitionV1::ExactSmooth(exact_smooth(degree)),
        )
        .unwrap();
        let ProductionPayoffEvidenceV1::ExactSmooth {
            certificate,
            certificate_bytes,
            certificate_id,
        } = &output.evidence
        else {
            panic!("exact smooth output changed evidence class");
        };
        assert_eq!(certificate.basis_degree, degree);
        assert_eq!(certificate.content_id().unwrap(), *certificate_id);
        assert_eq!(
            ExactSmoothPayoffCertificateV1::decode(certificate_bytes).unwrap(),
            *certificate
        );
        certificate
            .verify(
                terms_id(),
                &output.native_claim_basis,
                output.coordinate_domain_min,
                output.coordinate_domain_max,
            )
            .unwrap();
        output.verify(terms_id()).unwrap();
    }
}

#[test]
fn exact_smooth_certificate_decoder_is_hostile_and_canonical() {
    let output = compile_production_payoff_v1(
        terms_id(),
        ProductionPayoffDefinitionV1::ExactSmooth(exact_smooth(2)),
    )
    .unwrap();
    let ProductionPayoffEvidenceV1::ExactSmooth {
        certificate_bytes, ..
    } = output.evidence
    else {
        unreachable!();
    };

    assert_eq!(
        ExactSmoothPayoffCertificateV1::decode(&certificate_bytes[..127]),
        Err(ProductionCompilerError::Truncated)
    );
    let mut trailing = certificate_bytes.clone();
    trailing.push(0);
    assert_eq!(
        ExactSmoothPayoffCertificateV1::decode(&trailing),
        Err(ProductionCompilerError::TrailingBytes)
    );
    let mut wrong_rounding = certificate_bytes.clone();
    wrong_rounding[14] ^= 1;
    assert_eq!(
        ExactSmoothPayoffCertificateV1::decode(&wrong_rounding),
        Err(ProductionCompilerError::InvalidDiscriminant)
    );
    let mut reserved = certificate_bytes;
    reserved[20] = 1;
    assert_eq!(
        ExactSmoothPayoffCertificateV1::decode(&reserved),
        Err(ProductionCompilerError::NonCanonicalPadding)
    );
}

#[test]
fn exact_smooth_refuses_nonminimal_caps_and_unrepresentable_bases() {
    let mut nonminimal = exact_smooth(2);
    nonminimal.maximum_liability = rat(2, 1);
    assert_eq!(
        compile_production_payoff_v1(
            terms_id(),
            ProductionPayoffDefinitionV1::ExactSmooth(nonminimal)
        ),
        Err(ProductionCompilerError::InvalidDefinition)
    );

    let mut nonuniform = exact_smooth(2);
    nonuniform.basis.knots = vec![0, 2, 5];
    nonuniform.basis.coordinate_domain_max = 5;
    assert_eq!(
        compile_production_payoff_v1(
            terms_id(),
            ProductionPayoffDefinitionV1::ExactSmooth(nonuniform)
        ),
        Err(ProductionCompilerError::UnrepresentableShape)
    );

    let mut incomplete_refusing = exact_smooth(3);
    incomplete_refusing.basis.resolved_edge_policy = EdgePolicy::Refuse;
    incomplete_refusing.basis.coordinate_domain_max += 1;
    assert_eq!(
        compile_production_payoff_v1(
            terms_id(),
            ProductionPayoffDefinitionV1::ExactSmooth(incomplete_refusing)
        ),
        Err(ProductionCompilerError::InvalidDomain)
    );
}

#[test]
fn analytic_exact_and_approximation_statuses_never_alias() {
    let exact = compile_production_payoff_v1(
        terms_id(),
        ProductionPayoffDefinitionV1::AnalyticSmooth(AnalyticSmoothPayoffDefinitionV1 {
            basis: smooth(1),
            shape: Shape::CappedCall {
                low: 0,
                high: 2,
                height: 12,
            },
        }),
    )
    .unwrap();
    assert!(matches!(
        &exact.evidence,
        ProductionPayoffEvidenceV1::AnalyticSmooth {
            status: SpanStatus::ExactInSpan,
            ..
        }
    ));

    let approximation = compile_production_payoff_v1(
        terms_id(),
        ProductionPayoffDefinitionV1::AnalyticSmooth(AnalyticSmoothPayoffDefinitionV1 {
            basis: smooth(2),
            shape: Shape::Triangle {
                left: 0,
                peak: 2,
                right: 4,
                height: 12,
            },
        }),
    )
    .unwrap();
    assert!(matches!(
        &approximation.evidence,
        ProductionPayoffEvidenceV1::AnalyticSmooth {
            status: SpanStatus::CertifiedApproximation,
            ..
        }
    ));
    assert_ne!(exact.evidence, approximation.evidence);
}

#[test]
fn emitted_product_bytes_and_terms_bindings_are_behavioral() {
    let output = compile_production_payoff_v1(
        terms_id(),
        ProductionPayoffDefinitionV1::ExactSmooth(exact_smooth(3)),
    )
    .unwrap();
    assert_eq!(
        output.verify(ContentId::from_bytes([8; 32])),
        Err(ProductionCompilerError::CertificateMismatch)
    );
    let mut changed = output;
    changed.native_claim_basis_bytes[0] ^= 1;
    assert_eq!(
        changed.verify(terms_id()),
        Err(ProductionCompilerError::OutputMismatch)
    );
}
