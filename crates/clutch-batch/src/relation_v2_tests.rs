//! Adversarial tests for the owner-blind RelationV2 core.

extern crate std;

use crate::relation_v1::{
    canonical_candidate as canonical_candidate_v1, verify as verify_v1, AllocationPolicyV1,
    AonPolicyV1, FeeBaseV1, FrozenPolicyV1, PairingWitnessPolicyV1, PortfolioLotPolicyV1,
    PortfolioOrderV1, RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1,
    SelfCrossPolicyV1, SingleEggOrderV1, TransferPhaseV1, RELATION_VERSION_V1,
};
use crate::relation_v2::{
    price_semantics_digest_v2, sha256_test_vector, verify_economic_candidate_v2, EconomicBookV2,
    EconomicCandidateV2, EconomicDomainV2, EconomicErrorV2, EconomicOrderV2, PricePreconditionV2,
    ECONOMIC_RELATION_VERSION_V2, EMPTY_ECONOMIC_ORDER_V2,
};
use crate::score_v2::RiskObjectiveV2;
use crate::{DustPolicy, PartialPolicy, Side, MAX_ORDERS};

const SCALE: u64 = 10_000;

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn domain() -> EconomicDomainV2 {
    EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: id(1),
        epoch_semantics_digest: id(2),
        relation_policy_digest: id(3),
        price_policy_digest: id(4),
        epoch_index: 7,
        outcome_count: 2,
        price_scale: SCALE,
    }
}

fn price() -> PricePreconditionV2 {
    price_for(&domain())
}

fn price_for(domain: &EconomicDomainV2) -> PricePreconditionV2 {
    let mut prices = [0u64; 16];
    prices[..2].copy_from_slice(&[SCALE / 2, SCALE / 2]);
    let semantic_price_digest =
        price_semantics_digest_v2(domain, &prices).expect("fixture prices are canonical");
    PricePreconditionV2 {
        policy_digest: id(4),
        semantic_price_digest,
        prices,
    }
}

fn coefficients(values: &[u64]) -> [u64; 16] {
    let mut result = [0u64; 16];
    result[..values.len()].copy_from_slice(values);
    result
}

fn order(
    order_id: u8,
    side: Side,
    coefficients: &[u64],
    quantity: u64,
    minimum_fill: u64,
    partial_policy: PartialPolicy,
    limit: u128,
) -> EconomicOrderV2 {
    EconomicOrderV2 {
        order_id: id(order_id),
        side,
        coefficients: self::coefficients(coefficients),
        quantity,
        minimum_fill,
        partial_policy,
        expiry_epoch: 7,
        limit_value_price_units_per_unit: limit,
    }
}

fn book_of(orders: &[EconomicOrderV2]) -> EconomicBookV2 {
    let mut book = EconomicBookV2::empty();
    book.orders[..orders.len()].copy_from_slice(orders);
    book.len = u8::try_from(orders.len()).expect("fixture fits the fixed book");
    book
}

fn candidate(fills: &[u64]) -> EconomicCandidateV2 {
    let mut candidate = EconomicCandidateV2::EMPTY;
    candidate.fills[..fills.len()].copy_from_slice(fills);
    candidate
}

fn simple_cross(quantity: u64) -> EconomicBookV2 {
    book_of(&[
        order(
            1,
            Side::Buy,
            &[1, 0],
            quantity,
            0,
            PartialPolicy::Allow,
            u128::from(SCALE),
        ),
        order(2, Side::Sell, &[1, 0], quantity, 0, PartialPolicy::Allow, 0),
    ])
}

#[test]
fn local_sha256_matches_fips_known_answers() {
    assert_eq!(
        sha256_test_vector(b"").unwrap(),
        [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ]
    );
    assert_eq!(
        sha256_test_vector(b"abc").unwrap(),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );
    assert_eq!(
        sha256_test_vector(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq").unwrap(),
        [
            0x24, 0x8d, 0x6a, 0x61, 0xd2, 0x06, 0x38, 0xb8, 0xe5, 0xc0, 0x26, 0x93, 0x0c, 0x3e,
            0x60, 0x39, 0xa3, 0x3c, 0xe4, 0x59, 0x64, 0xff, 0x21, 0x67, 0xf6, 0xec, 0xed, 0xd4,
            0x19, 0xdb, 0x06, 0xc1,
        ]
    );
}

#[test]
fn owner_or_signer_relabeling_is_not_a_relation_input() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ExternalSettlementBinding {
        owner: [u8; 32],
        signer: [u8; 32],
    }

    let one_controller = [
        ExternalSettlementBinding {
            owner: id(90),
            signer: id(91),
        },
        ExternalSettlementBinding {
            owner: id(90),
            signer: id(91),
        },
    ];
    let split_controllers = [
        ExternalSettlementBinding {
            owner: id(92),
            signer: id(93),
        },
        ExternalSettlementBinding {
            owner: id(94),
            signer: id(95),
        },
    ];
    assert_ne!(one_controller, split_controllers);

    // Neither binding can be passed to the API. The same economic input has
    // exactly one result regardless of which future settlement sidecar exists.
    let book = simple_cross(7);
    let economic = candidate(&[7, 7]);
    let first = verify_economic_candidate_v2(&domain(), &book, &price(), &economic).unwrap();
    let second = verify_economic_candidate_v2(&domain(), &book, &price(), &economic).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.direct_flow[..2], [7, 0]);
}

#[test]
fn complete_set_direct_flow_and_virtual_churn_cannot_improve_score() {
    let complete_set = book_of(&[
        order(
            1,
            Side::Buy,
            &[1, 1],
            7,
            0,
            PartialPolicy::Allow,
            u128::from(SCALE),
        ),
        order(
            2,
            Side::Sell,
            &[1, 1],
            7,
            0,
            PartialPolicy::Allow,
            u128::from(SCALE),
        ),
    ]);
    let wash =
        verify_economic_candidate_v2(&domain(), &complete_set, &price(), &candidate(&[7, 7]))
            .unwrap();
    let empty = verify_economic_candidate_v2(
        &domain(),
        &complete_set,
        &price(),
        &EconomicCandidateV2::EMPTY,
    )
    .unwrap();
    assert_eq!(wash.direct_flow[..2], [7, 7]);
    assert_eq!(
        wash.score.score().risk,
        RiskObjectiveV2 {
            certified_risk_flow_atoms: 0
        }
    );
    assert_eq!(wash.score.score().cash_equivalent_direct_flow_atoms, 7);
    assert_eq!(
        empty.score.total_order_same_domain(&wash.score),
        Ok(core::cmp::Ordering::Greater)
    );

    let split_book = book_of(&[order(
        1,
        Side::Buy,
        &[1, 1],
        5,
        0,
        PartialPolicy::Allow,
        u128::from(SCALE),
    )]);
    let mut split = candidate(&[5]);
    split.virtual_split = 5;
    let churn = verify_economic_candidate_v2(&domain(), &split_book, &price(), &split).unwrap();
    let no_churn = verify_economic_candidate_v2(
        &domain(),
        &split_book,
        &price(),
        &EconomicCandidateV2::EMPTY,
    )
    .unwrap();
    assert_eq!(churn.direct_flow[..2], [0, 0]);
    assert_eq!(churn.score.score().risk.certified_risk_flow_atoms, 0);
    assert_eq!(churn.score.score().virtual_churn_atoms, 5);
    assert_eq!(
        no_churn.score.total_order_same_domain(&churn.score),
        Ok(core::cmp::Ordering::Greater)
    );
}

#[test]
fn price_book_fill_and_mask_padding_refuse_exactly() {
    let book = simple_cross(7);
    let economic = candidate(&[7, 7]);

    let mut bad_price = price();
    bad_price.prices[2] = 1;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &book, &bad_price, &economic),
        Err(EconomicErrorV2::NonCanonicalPricePadding { outcome: 2 })
    );

    let mut bad_coefficient = book;
    bad_coefficient.orders[0].coefficients[2] = 1;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &bad_coefficient, &price(), &economic),
        Err(EconomicErrorV2::NonCanonicalCoefficientPadding {
            order: 0,
            outcome: 2
        })
    );

    let mut bad_order_padding = book;
    bad_order_padding.orders[2] = order(
        3,
        Side::Buy,
        &[1, 0],
        1,
        0,
        PartialPolicy::Allow,
        u128::from(SCALE),
    );
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &bad_order_padding, &price(), &economic),
        Err(EconomicErrorV2::NonCanonicalOrderPadding { order: 2 })
    );

    let mut bad_fill = economic;
    bad_fill.fills[2] = 1;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &book, &price(), &bad_fill),
        Err(EconomicErrorV2::NonCanonicalFillPadding { order: 2 })
    );

    let mut bad_mask = economic;
    bad_mask.honored_aon_mask = 1 << 2;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &book, &price(), &bad_mask),
        Err(EconomicErrorV2::AonMaskNotApplicable { order: 2 })
    );

    let mut oversized_domain = domain();
    oversized_domain.outcome_count = u8::MAX;
    assert_eq!(
        price().validate(&oversized_domain),
        Err(EconomicErrorV2::InvalidOutcomeCount)
    );
}

#[test]
fn aon_minimum_limit_and_order_canonicality_are_enforced() {
    let minimum = book_of(&[
        order(
            1,
            Side::Buy,
            &[1, 0],
            7,
            3,
            PartialPolicy::Allow,
            u128::from(SCALE),
        ),
        order(2, Side::Sell, &[1, 0], 7, 3, PartialPolicy::Allow, 0),
    ]);
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &minimum, &price(), &candidate(&[2, 2])),
        Err(EconomicErrorV2::MinimumFillViolation { order: 0 })
    );

    let aon = book_of(&[
        order(
            1,
            Side::Buy,
            &[1, 0],
            7,
            7,
            PartialPolicy::AllOrNone,
            u128::from(SCALE),
        ),
        order(2, Side::Sell, &[1, 0], 7, 7, PartialPolicy::AllOrNone, 0),
    ]);
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &aon, &price(), &candidate(&[6, 6])),
        Err(EconomicErrorV2::AllOrNoneViolation { order: 0 })
    );
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &aon, &price(), &candidate(&[7, 7])),
        Err(EconomicErrorV2::AonMaskMismatch { order: 0 })
    );
    let mut honored = candidate(&[7, 7]);
    honored.honored_aon_mask = 0b11;
    verify_economic_candidate_v2(&domain(), &aon, &price(), &honored).unwrap();

    let mut ineligible = simple_cross(7);
    ineligible.orders[0].limit_value_price_units_per_unit = 4_999;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &ineligible, &price(), &candidate(&[7, 7])),
        Err(EconomicErrorV2::LimitViolation { order: 0 })
    );

    let mut unordered = simple_cross(7);
    unordered.orders[1].order_id = unordered.orders[0].order_id;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &unordered, &price(), &candidate(&[7, 7])),
        Err(EconomicErrorV2::NonCanonicalOrderOrder { order: 1 })
    );
}

#[test]
fn flow_overflow_and_every_virtual_conservation_failure_refuse() {
    let impossible_order = book_of(&[order(
        1,
        Side::Buy,
        &[u64::MAX, 0],
        2,
        0,
        PartialPolicy::Allow,
        u128::MAX,
    )]);
    assert_eq!(
        impossible_order.validate(&domain()),
        Err(EconomicErrorV2::FlowOverflow {
            order: 0,
            outcome: 0
        })
    );

    let aggregate_overflow = book_of(&[
        order(
            1,
            Side::Buy,
            &[u64::MAX, 0],
            1,
            0,
            PartialPolicy::Allow,
            u128::MAX,
        ),
        order(2, Side::Buy, &[1, 0], 1, 0, PartialPolicy::Allow, u128::MAX),
    ]);
    assert_eq!(
        verify_economic_candidate_v2(
            &domain(),
            &aggregate_overflow,
            &price(),
            &candidate(&[1, 1])
        ),
        Err(EconomicErrorV2::FlowOverflow {
            order: 1,
            outcome: 0
        })
    );

    let buy_only = book_of(&[order(
        1,
        Side::Buy,
        &[1, 0],
        1,
        0,
        PartialPolicy::Allow,
        u128::from(SCALE),
    )]);
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &buy_only, &price(), &candidate(&[1])),
        Err(EconomicErrorV2::OutcomeConservationMismatch { outcome: 0 })
    );

    let mut split_exceeds = candidate(&[1]);
    split_exceeds.virtual_split = 2;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &buy_only, &price(), &split_exceeds),
        Err(EconomicErrorV2::VirtualSplitExceedsBuy { outcome: 0 })
    );

    let mut both = candidate(&[1]);
    both.virtual_split = 1;
    both.virtual_merge = 1;
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &buy_only, &price(), &both),
        Err(EconomicErrorV2::NonCanonicalVirtualConversion)
    );
}

#[test]
fn semantic_price_binding_and_full_digest_are_not_claimed_or_truncated() {
    let book = simple_cross(7);
    let economic = candidate(&[7, 7]);
    let baseline_price = price();
    assert_eq!(
        baseline_price.semantic_price_digest,
        [
            0xf3, 0xf8, 0x73, 0x7e, 0xb1, 0xc5, 0x92, 0x14, 0xf1, 0x21, 0x7e, 0x37, 0x27, 0xf0,
            0x06, 0xb8, 0x6f, 0x30, 0xb2, 0xe9, 0x0a, 0x99, 0xa0, 0xb6, 0x67, 0x46, 0xae, 0x4d,
            0xaf, 0xae, 0xde, 0xb6,
        ]
    );
    let baseline =
        verify_economic_candidate_v2(&domain(), &book, &baseline_price, &economic).unwrap();
    assert_eq!(
        baseline.economic_candidate_digest,
        [
            0x4b, 0x9e, 0x46, 0x17, 0xf6, 0xe2, 0x55, 0x03, 0x99, 0x29, 0x81, 0x8c, 0x26, 0xb3,
            0xac, 0x39, 0x24, 0x9a, 0xf5, 0x3d, 0x3b, 0x3b, 0x51, 0x82, 0xa1, 0xf8, 0xa4, 0xe8,
            0x58, 0xc9, 0x98, 0x87,
        ]
    );
    assert_eq!(
        baseline.score.score().digest,
        baseline.economic_candidate_digest
    );

    let mut forged_semantics = price();
    forged_semantics.semantic_price_digest = id(6);
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &book, &forged_semantics, &economic),
        Err(EconomicErrorV2::PriceSemanticDigestMismatch)
    );

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct AuthenticatedPriceProofSidecar {
        representation_digest: [u8; 32],
    }
    let proof_a = AuthenticatedPriceProofSidecar {
        representation_digest: id(90),
    };
    let proof_b = AuthenticatedPriceProofSidecar {
        representation_digest: id(91),
    };
    assert_ne!(proof_a, proof_b);
    let project_semantics = |_proof: &AuthenticatedPriceProofSidecar| price();
    let proof_a_projection =
        verify_economic_candidate_v2(&domain(), &book, &project_semantics(&proof_a), &economic)
            .unwrap();
    let proof_b_projection =
        verify_economic_candidate_v2(&domain(), &book, &project_semantics(&proof_b), &economic)
            .unwrap();
    assert_eq!(proof_a_projection, proof_b_projection);
    assert_eq!(baseline.score, proof_b_projection.score);

    let mut other_domain = domain();
    other_domain.market_semantics_digest = id(7);
    let changed_domain =
        verify_economic_candidate_v2(&other_domain, &book, &price_for(&other_domain), &economic)
            .unwrap();
    assert_ne!(
        baseline.economic_candidate_digest,
        changed_domain.economic_candidate_digest
    );

    let mut wrong_policy = price();
    wrong_policy.policy_digest = id(8);
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &book, &wrong_policy, &economic),
        Err(EconomicErrorV2::PricePolicyMismatch)
    );
}

#[test]
fn overlap_free_single_and_portfolio_fixtures_match_relation_v1_flows() {
    let policy = FrozenPolicyV1 {
        allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
        self_cross: SelfCrossPolicyV1::RefuseOverlap,
        aon: AonPolicyV1::RefuseAdmission,
        rounding: RoundingBoundaryV1::None,
        residual_settlement: ResidualSettlementV1::FullPairOnly,
        transfer_phase: TransferPhaseV1::ActiveOnly,
        portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
        dust: DustPolicy::AssignCanonical,
        score: ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    };
    let v1_domain = RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 1,
        book_id: 2,
        epoch: 7,
        policy_id: 3,
        order_set_id: 4,
        outcome_count: 2,
        owner_count: 2,
        price_scale: SCALE,
        remainder_seed: 5,
        policy,
    };
    let mut v1_prices = [0u64; 16];
    v1_prices[..2].copy_from_slice(&[SCALE / 2, SCALE / 2]);

    let mut v1_single = crate::relation_v1::BookV1::empty();
    v1_single.orders[0] = crate::relation_v1::OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: 1,
        owner: 0,
        outcome: 0,
        side: Side::Buy,
        quantity: 8,
        limit_price: SCALE,
        minimum_fill: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 7,
    });
    v1_single.orders[1] = crate::relation_v1::OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: 2,
        owner: 1,
        outcome: 0,
        side: Side::Sell,
        quantity: 8,
        limit_price: 0,
        minimum_fill: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 7,
    });
    v1_single.len = 2;
    let v1_candidate = canonical_candidate_v1(&v1_domain, &v1_single, &v1_prices, 0, 0).unwrap();
    let v1_summary = verify_v1(&v1_domain, &v1_single, &v1_candidate, None).unwrap();
    let v2_summary =
        verify_economic_candidate_v2(&domain(), &simple_cross(8), &price(), &candidate(&[8, 8]))
            .unwrap();
    assert_eq!(v2_summary.aggregate_buy_flow, v1_summary.buy_flow);
    assert_eq!(v2_summary.aggregate_sell_flow, v1_summary.sell_flow);
    assert_eq!(v2_summary.direct_flow, v1_summary.direct_flow);

    let mut v1_portfolio = crate::relation_v1::BookV1::empty();
    v1_portfolio.orders[0] = crate::relation_v1::OrderV1::Portfolio(PortfolioOrderV1 {
        canonical_order_id: 1,
        owner: 0,
        side: Side::Buy,
        coefficients: coefficients(&[1, 1]),
        active_len: 2,
        lots: 4,
        limit_collateral_per_lot: 2,
        minimum_fill_lots: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 7,
    });
    v1_portfolio.orders[1] = crate::relation_v1::OrderV1::Portfolio(PortfolioOrderV1 {
        canonical_order_id: 2,
        owner: 1,
        side: Side::Sell,
        coefficients: coefficients(&[1, 1]),
        active_len: 2,
        lots: 4,
        limit_collateral_per_lot: 0,
        minimum_fill_lots: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 7,
    });
    v1_portfolio.len = 2;
    let v1_candidate = canonical_candidate_v1(&v1_domain, &v1_portfolio, &v1_prices, 0, 0).unwrap();
    let v1_summary = verify_v1(&v1_domain, &v1_portfolio, &v1_candidate, None).unwrap();
    let v2_portfolio = book_of(&[
        order(
            1,
            Side::Buy,
            &[1, 1],
            4,
            0,
            PartialPolicy::Allow,
            2 * u128::from(SCALE),
        ),
        order(2, Side::Sell, &[1, 1], 4, 0, PartialPolicy::Allow, 0),
    ]);
    let v2_summary =
        verify_economic_candidate_v2(&domain(), &v2_portfolio, &price(), &candidate(&[4, 4]))
            .unwrap();
    assert_eq!(v2_summary.aggregate_buy_flow, v1_summary.buy_flow);
    assert_eq!(v2_summary.aggregate_sell_flow, v1_summary.sell_flow);
    assert_eq!(v2_summary.direct_flow, v1_summary.direct_flow);
}

#[test]
fn canonical_empty_padding_value_stays_exact() {
    let empty = EconomicBookV2::empty();
    assert_eq!(empty.orders, [EMPTY_ECONOMIC_ORDER_V2; MAX_ORDERS]);
    assert_eq!(empty.len, 0);
    empty.validate(&domain()).unwrap();
}
