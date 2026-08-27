//! Emit the cross-language degree-one native artifact fixture as canonical JSON.

use clutch_accumulator::MAX_VALUE;
use clutch_bspline::{MAX_KNOTS, UNIFORM_SPACING_NONE};
use clutch_bspline_shape_compiler::{
    artifact::{build_market_creation_artifacts_v1, render_cross_language_fixture_v1},
    Shape,
};
use clutch_solana_layout::{
    Hash32, PayoutVectorBytes, TermsAccount, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED,
};

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32]).expect("fixture hashes are nonzero")
}

fn terms() -> TermsAccount {
    let mut weights = [0_u64; MAX_OUTCOMES];
    weights[0] = 8;
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    payouts[0] = PayoutVectorBytes {
        denominator: 8,
        weights,
    };
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..2].copy_from_slice(&[0, MAX_VALUE]);
    let mut value = TermsAccount {
        terms: Hash32::ZERO,
        realm: hash(1),
        profile: hash(2),
        feed: hash(3),
        price_grid: hash(4),
        outcome_count: 2,
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
        knot_count: 2,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 0,
        source_version: 1,
        evaluator_version: 1,
        source_adapter_id: hash(5),
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
        collateral_cap: 1_000_000,
        stored_bump: 7,
        flags: 0,
    };
    value.terms = value
        .recomputed_terms_digest()
        .expect("fixture terms body is encodable");
    value.validate().expect("fixture TermsAccount is canonical");
    value
}

fn main() {
    let terms = terms();
    let artifacts = build_market_creation_artifacts_v1(
        &terms,
        42,
        1_000,
        2_000,
        Shape::CappedCall {
            low: 0,
            high: MAX_VALUE,
            height: 8,
        },
    )
    .expect("fixture artifacts compile");
    println!("{}", render_cross_language_fixture_v1(&artifacts, 42));
}
