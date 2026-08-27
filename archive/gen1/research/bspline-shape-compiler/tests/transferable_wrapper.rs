use clutch_accumulator::MAX_VALUE;
use clutch_bspline::{MAX_KNOTS, UNIFORM_SPACING_NONE};
use clutch_bspline_shape_compiler::{
    wrapper::{
        canonical_wrapper_product_id_v1, compile_transferable_shape_v1, flatten_composition_v1,
        realize_native_coefficients_v1, CompositionDispositionV1, CompositionLegV1,
        WrapperCompilerError, WrapperDeploymentBindingV1,
    },
    Shape,
};
use clutch_solana_layout::{
    canonical_outcome_id,
    portfolio_settlement::{canonical_native_portfolio_claim_id, NativePortfolioClaimV1},
    Hash32, MarketAccount, PayoutVectorBytes, TermsAccount, MAX_OUTCOMES, MAX_PAYOUTS,
    PAYOUT_MAP_UNUSED,
};
use num_bigint::BigInt;
use num_rational::BigRational;

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32]).unwrap()
}

fn terms() -> TermsAccount {
    let denominator = 64;
    let mut weights = [0_u64; MAX_OUTCOMES];
    weights[0] = denominator;
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    payouts[0] = PayoutVectorBytes {
        denominator,
        weights,
    };
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..3].copy_from_slice(&[0, MAX_VALUE / 2, MAX_VALUE]);
    let mut value = TermsAccount {
        terms: Hash32::ZERO,
        realm: hash(1),
        profile: hash(2),
        feed: hash(3),
        price_grid: hash(4),
        outcome_count: 3,
        payout_count: 1,
        payouts,
        grid_family_id: 1,
        grid_version: 1,
        bucket_seconds: 60,
        expected_start_bucket: 10,
        expected_end_bucket_exclusive: 20,
        maturity_horizon_buckets: 10,
        coverage_policy_id: 1,
        repair_policy_id: 1,
        failure_policy_id: 1,
        statistic_id: 1,
        ambiguity_policy_id: 1,
        edge_policy_id: 1,
        basis_degree: 1,
        knot_count: 3,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 0,
        source_version: 2,
        evaluator_version: 1,
        source_adapter_id: hash(5),
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
        collateral_cap: 1_000_000,
        stored_bump: 7,
        flags: 0,
    };
    value.terms = value.recomputed_terms_digest().unwrap();
    value.validate().unwrap();
    value
}

fn market_fixture(terms: &TermsAccount, marker: u8) -> MarketAccount {
    let market_id = hash(marker);
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    for (index, destination) in outcomes
        .iter_mut()
        .enumerate()
        .take(usize::from(terms.outcome_count))
    {
        *destination = canonical_outcome_id(market_id, index as u8);
    }
    let value = MarketAccount {
        market: market_id,
        realm: terms.realm,
        profile: terms.profile,
        terms: terms.terms,
        outcome_count: terms.outcome_count,
        lifecycle: 0,
        stored_bump: 1,
        hoard_bump: 2,
        outcomes,
        feed: terms.feed,
        collateral_cap: terms.collateral_cap,
        created_slot: 100,
        reserved: Hash32::ZERO,
    };
    value.validate().unwrap();
    terms.binds_market(&value).unwrap();
    value
}

fn deployment() -> WrapperDeploymentBindingV1 {
    WrapperDeploymentBindingV1 {
        wrapper_program: [11; 32],
        wrapper_program_data: [12; 32],
        wrapper_deployment_slot: 1_000,
        base_program: [13; 32],
        base_program_data: [14; 32],
        base_deployment_slot: 2_000,
        token_2022_program: [15; 32],
        token_2022_program_data: [16; 32],
        token_2022_deployment_slot: 3_000,
    }
}

fn rat(numerator: i64, denominator: i64) -> BigRational {
    BigRational::new(BigInt::from(numerator), BigInt::from(denominator))
}

#[test]
fn rational_coefficients_get_one_minimal_exact_integer_realization() {
    let half = realize_native_coefficients_v1(&[rat(1, 2), rat(1, 1), rat(2, 1)]).unwrap();
    assert_eq!(&half.primitive[..3], &[1, 2, 4]);
    assert_eq!(half.wrapper_atoms_per_display_lot, 1);
    assert_eq!(half.target_units_per_display_lot, 2);
    assert_eq!(half.complete_set_cash_atoms_per_wrapper, 1);
    assert_eq!(&half.residual_eggs_per_wrapper[..3], &[0, 1, 3]);

    let doubled = realize_native_coefficients_v1(&[rat(2, 1), rat(4, 1), rat(6, 1)]).unwrap();
    assert_eq!(&doubled.primitive[..3], &[1, 2, 3]);
    assert_eq!(doubled.wrapper_atoms_per_display_lot, 2);
    assert_eq!(doubled.target_units_per_display_lot, 1);
    assert_eq!(doubled.complete_set_cash_atoms_per_wrapper, 1);
    assert_eq!(&doubled.residual_eggs_per_wrapper[..3], &[0, 1, 2]);

    for (target, realization) in [
        ([rat(1, 2), rat(1, 1), rat(2, 1)], half),
        ([rat(2, 1), rat(4, 1), rat(6, 1)], doubled),
    ] {
        for (coefficient, primitive) in target.iter().zip(realization.primitive) {
            assert_eq!(
                coefficient * BigInt::from(realization.target_units_per_display_lot),
                BigRational::from_integer(
                    BigInt::from(primitive)
                        * BigInt::from(realization.wrapper_atoms_per_display_lot)
                )
            );
        }
    }
}

#[test]
fn live_claim_digest_matches_the_python_golden_vector() {
    let mut coefficients = [0; MAX_OUTCOMES];
    coefficients[..3].copy_from_slice(&[1, 2, 4]);
    assert_eq!(
        hex(canonical_native_portfolio_claim_id(hash(2), hash(3), 2, 6, 3, &coefficients,).0),
        "41885f4a143807479f1e3fa00752c1ce4e1ca7e56834fae084204e3e63831261"
    );
}

#[test]
fn redundant_and_unrepresentable_wrapper_products_refuse() {
    for coefficients in [
        vec![rat(0, 1), rat(0, 1), rat(0, 1)],
        vec![rat(0, 1), rat(7, 1), rat(0, 1)],
        vec![rat(7, 1), rat(7, 1), rat(7, 1)],
        vec![rat(-1, 1), rat(2, 1), rat(3, 1)],
    ] {
        assert_eq!(
            realize_native_coefficients_v1(&coefficients),
            Err(WrapperCompilerError::NoWrapperProductValue)
        );
    }
    let huge = BigInt::from(u64::MAX) + BigInt::from(1_u8);
    assert_eq!(
        realize_native_coefficients_v1(&[BigRational::from_integer(huge), rat(1, 1), rat(0, 1),]),
        Err(WrapperCompilerError::IntegerRealizationOverflow)
    );
}

#[test]
fn shape_compiler_joins_the_live_claim_owner_without_binding_marketing_provenance() {
    let terms = terms();
    let market = market_fixture(&terms, 21);
    let shape = |height| Shape::CappedCall {
        low: 0,
        high: MAX_VALUE,
        height,
    };
    let low = compile_transferable_shape_v1(deployment(), &market, &terms, shape(8)).unwrap();
    let high = compile_transferable_shape_v1(deployment(), &market, &terms, shape(16)).unwrap();
    low.verify(&market, &terms).unwrap();
    high.verify(&market, &terms).unwrap();

    // Scalar copies share the live primitive claim and wrapper mint. The
    // independently recompile-verifiable analytic certificates remain distinct.
    assert_eq!(low.claim, high.claim);
    assert_eq!(low.product, high.product);
    assert_ne!(low.certificate_digest, high.certificate_digest);
    assert_ne!(
        low.realization.wrapper_atoms_per_display_lot,
        high.realization.wrapper_atoms_per_display_lot
    );
    let (live_claim, scale) =
        NativePortfolioClaimV1::compile(market.market, &terms, low.realization.primitive).unwrap();
    assert_eq!(scale, 1);
    assert_eq!(live_claim, low.claim);
}

#[test]
fn deployment_identity_is_load_bearing_but_certificate_is_not() {
    let terms = terms();
    let market = market_fixture(&terms, 21);
    let claim = NativePortfolioClaimV1::compile(market.market, &terms, {
        let mut coefficients = [0; MAX_OUTCOMES];
        coefficients[..3].copy_from_slice(&[1, 2, 4]);
        coefficients
    })
    .unwrap()
    .0;
    let original = canonical_wrapper_product_id_v1(deployment(), claim.claim).unwrap();
    let mut upgraded = deployment();
    upgraded.base_deployment_slot += 1;
    assert_ne!(
        original,
        canonical_wrapper_product_id_v1(upgraded, claim.claim).unwrap()
    );
    let mut aliased = deployment();
    aliased.base_program_data = aliased.base_program;
    assert_eq!(
        canonical_wrapper_product_id_v1(aliased, claim.claim),
        Err(WrapperCompilerError::InvalidDeployment)
    );
}

#[test]
fn composition_flattens_to_native_eggs_and_is_associative() {
    let terms = terms();
    let market = market_fixture(&terms, 21);
    let claim = |active: [u64; 3]| {
        let mut coefficients = [0; MAX_OUTCOMES];
        coefficients[..3].copy_from_slice(&active);
        NativePortfolioClaimV1::compile(market.market, &terms, coefficients)
            .unwrap()
            .0
    };
    let a = claim([1, 2, 4]);
    let b = claim([2, 1, 3]);
    let flat = flatten_composition_v1(
        &market,
        &terms,
        &[
            CompositionLegV1 {
                claim: a,
                wrapper_atoms: 2,
            },
            CompositionLegV1 {
                claim: b,
                wrapper_atoms: 3,
            },
        ],
    )
    .unwrap();
    assert_eq!(&flat.exact_eggs[..3], &[8, 7, 17]);
    assert_eq!(flat.primitive_units, 1);
    assert_eq!(&flat.claim.coefficients[..3], &[8, 7, 17]);
    assert_eq!(flat.input_cash_atoms, 5);
    assert_eq!(flat.additional_complete_sets_to_merge, 2);
    assert_eq!(flat.output_cash_atoms, 7);
    assert_eq!(&flat.output_residual_eggs[..3], &[1, 0, 10]);
    assert_eq!(
        flat.disposition,
        CompositionDispositionV1::TransferableWrapper
    );

    let left = flatten_composition_v1(
        &market,
        &terms,
        &[CompositionLegV1 {
            claim: flat.claim,
            wrapper_atoms: 5,
        }],
    )
    .unwrap();
    let right = flatten_composition_v1(
        &market,
        &terms,
        &[
            CompositionLegV1 {
                claim: a,
                wrapper_atoms: 10,
            },
            CompositionLegV1 {
                claim: b,
                wrapper_atoms: 15,
            },
        ],
    )
    .unwrap();
    assert_eq!(left.exact_eggs, right.exact_eggs);
    assert_eq!(left.claim, right.claim);
    assert_eq!(left.primitive_units, right.primitive_units);
    assert_eq!(left.output_cash_atoms, right.output_cash_atoms);
    assert_eq!(left.output_residual_eggs, right.output_residual_eggs);
}

#[test]
fn composition_that_becomes_a_complete_set_routes_to_cash_not_a_wrapper() {
    let terms = terms();
    let market = market_fixture(&terms, 21);
    let claim = |active: [u64; 3]| {
        let mut coefficients = [0; MAX_OUTCOMES];
        coefficients[..3].copy_from_slice(&active);
        NativePortfolioClaimV1::compile(market.market, &terms, coefficients)
            .unwrap()
            .0
    };
    let flat = flatten_composition_v1(
        &market,
        &terms,
        &[
            CompositionLegV1 {
                claim: claim([1, 2, 3]),
                wrapper_atoms: 1,
            },
            CompositionLegV1 {
                claim: claim([2, 1, 0]),
                wrapper_atoms: 1,
            },
        ],
    )
    .unwrap();
    assert_eq!(&flat.exact_eggs[..3], &[3, 3, 3]);
    assert_eq!(flat.output_cash_atoms, 3);
    assert_eq!(&flat.output_residual_eggs[..3], &[0, 0, 0]);
    assert_eq!(flat.disposition, CompositionDispositionV1::CompleteSetCash);
}

#[test]
fn composition_refuses_zero_cross_market_and_overflow_legs() {
    let terms = terms();
    let market = market_fixture(&terms, 21);
    let other_market = market_fixture(&terms, 22);
    let mut coefficients = [0; MAX_OUTCOMES];
    coefficients[..3].copy_from_slice(&[1, 2, 4]);
    let claim = NativePortfolioClaimV1::compile(market.market, &terms, coefficients)
        .unwrap()
        .0;
    let foreign = NativePortfolioClaimV1::compile(other_market.market, &terms, coefficients)
        .unwrap()
        .0;
    assert_eq!(
        flatten_composition_v1(
            &market,
            &terms,
            &[CompositionLegV1 {
                claim,
                wrapper_atoms: 0,
            }]
        ),
        Err(WrapperCompilerError::InvalidComposition)
    );
    assert_eq!(
        flatten_composition_v1(
            &market,
            &terms,
            &[CompositionLegV1 {
                claim: foreign,
                wrapper_atoms: 1,
            }]
        ),
        Err(WrapperCompilerError::InvalidComposition)
    );
    assert_eq!(
        flatten_composition_v1(
            &market,
            &terms,
            &[CompositionLegV1 {
                claim,
                wrapper_atoms: u64::MAX,
            }]
        ),
        Err(WrapperCompilerError::CompositionOverflow)
    );
}

#[test]
fn plan_refuses_foreign_terms_resolved_creation_and_mutated_fields() {
    let terms = terms();
    let market = market_fixture(&terms, 21);
    let shape = Shape::CappedCall {
        low: 0,
        high: MAX_VALUE,
        height: 8,
    };
    let plan = compile_transferable_shape_v1(deployment(), &market, &terms, shape).unwrap();

    let mut resolved = market;
    resolved.lifecycle = 1;
    assert_eq!(
        compile_transferable_shape_v1(deployment(), &resolved, &terms, shape),
        Err(WrapperCompilerError::MarketNotActive)
    );

    let mut mutated = plan.clone();
    mutated.realization.residual_eggs_per_wrapper[1] += 1;
    assert_eq!(
        mutated.verify(&market, &terms),
        Err(WrapperCompilerError::PlanMismatch)
    );

    let foreign_market = market_fixture(&terms, 22);
    assert_eq!(
        plan.verify(&foreign_market, &terms),
        Err(WrapperCompilerError::PlanMismatch)
    );
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
