//! Adversarial tests for fixed-domain best-valid-submitted ranking.

extern crate std;

use crate::relation_v2::{
    price_semantics_digest_v2, EconomicBookV2, EconomicCandidateV2, EconomicDomainV2,
    EconomicErrorV2, EconomicOrderV2, PricePreconditionV2, ECONOMIC_RELATION_VERSION_V2,
};
use crate::relation_v2_ranking::BestValidSubmittedCandidateV2;
use crate::score_v2::SelectionUpdateV2;
use crate::{PartialPolicy, Side};

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

fn price(domain: &EconomicDomainV2) -> PricePreconditionV2 {
    let mut prices = [0u64; 16];
    prices[..2].copy_from_slice(&[SCALE / 2, SCALE / 2]);
    PricePreconditionV2 {
        policy_digest: domain.price_policy_digest,
        semantic_price_digest: price_semantics_digest_v2(domain, &prices).unwrap(),
        prices,
    }
}

fn coefficients(outcome: usize) -> [u64; 16] {
    let mut coefficients = [0u64; 16];
    coefficients[outcome] = 1;
    coefficients
}

fn order(order_id: u8, side: Side, outcome: usize) -> EconomicOrderV2 {
    EconomicOrderV2 {
        order_id: id(order_id),
        side,
        coefficients: coefficients(outcome),
        quantity: 10,
        minimum_fill: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 7,
        limit_value_price_units_per_unit: match side {
            Side::Buy => u128::from(SCALE),
            Side::Sell => 0,
        },
    }
}

fn book() -> EconomicBookV2 {
    let orders = [
        order(1, Side::Buy, 0),
        order(2, Side::Sell, 0),
        order(3, Side::Buy, 1),
        order(4, Side::Sell, 1),
    ];
    let mut book = EconomicBookV2::empty();
    book.orders[..orders.len()].copy_from_slice(&orders);
    book.len = u8::try_from(orders.len()).unwrap();
    book
}

fn candidate(fills: [u64; 4]) -> EconomicCandidateV2 {
    let mut candidate = EconomicCandidateV2::EMPTY;
    candidate.fills[..fills.len()].copy_from_slice(&fills);
    candidate
}

#[test]
fn relation_fold_retains_best_valid_submitted_not_complete_set_wash() {
    let domain = domain();
    let book = book();
    let price = price(&domain);
    let wash = candidate([7, 7, 7, 7]);
    let mut ranking =
        BestValidSubmittedCandidateV2::begin(domain, book, price, wash).unwrap();
    assert_eq!(ranking.valid_submission_count(), 1);
    assert_eq!(ranking.best_economics().direct_flow[..2], [7, 7]);
    assert_eq!(
        ranking.best_economics().score.domain().market_semantics_digest(),
        domain.market_semantics_digest
    );

    assert_eq!(
        ranking.submit(EconomicCandidateV2::EMPTY),
        Ok(SelectionUpdateV2::ReplacedBest)
    );
    assert_eq!(ranking.best_economics().direct_flow[..2], [0, 0]);

    let contingent = candidate([9, 9, 1, 1]);
    assert_eq!(
        ranking.submit(contingent),
        Ok(SelectionUpdateV2::ReplacedBest)
    );
    assert_eq!(ranking.best_candidate(), &contingent);
    assert_eq!(ranking.best_economics().direct_flow[..2], [9, 1]);
    assert_eq!(
        ranking
            .best_economics()
            .score
            .score()
            .risk
            .certified_risk_flow_atoms,
        8
    );
    assert_eq!(ranking.valid_submission_count(), 3);
}

#[test]
fn refused_submission_cannot_mutate_retained_candidate_or_count() {
    let domain = domain();
    let book = book();
    let price = price(&domain);
    let first = candidate([3, 3, 0, 0]);
    let mut ranking =
        BestValidSubmittedCandidateV2::begin(domain, book, price, first).unwrap();
    let before = ranking;
    let invalid = candidate([11, 11, 0, 0]);
    assert_eq!(
        ranking.submit(invalid),
        Err(EconomicErrorV2::FillExceedsQuantity { order: 0 })
    );
    assert_eq!(ranking, before);
}

#[test]
fn deterministic_digest_tie_break_is_used_only_after_equal_economics() {
    let domain = domain();
    let book = book();
    let price = price(&domain);
    let left = candidate([9, 9, 1, 1]);
    let right = candidate([1, 1, 9, 9]);
    let left_score = crate::relation_v2::verify_economic_candidate_v2(
        &domain,
        &book,
        &price,
        &left,
    )
    .unwrap()
    .score;
    let right_score = crate::relation_v2::verify_economic_candidate_v2(
        &domain,
        &book,
        &price,
        &right,
    )
    .unwrap()
    .score;
    assert_eq!(
        left_score.score().risk.certified_risk_flow_atoms,
        right_score.score().risk.certified_risk_flow_atoms
    );
    assert_eq!(
        left_score.score().cash_equivalent_direct_flow_atoms,
        right_score.score().cash_equivalent_direct_flow_atoms
    );
    assert_eq!(
        left_score.score().virtual_churn_atoms,
        right_score.score().virtual_churn_atoms
    );

    let expected = if left_score.score().digest < right_score.score().digest {
        left
    } else {
        right
    };
    let mut ranking =
        BestValidSubmittedCandidateV2::begin(domain, book, price, left).unwrap();
    ranking.submit(right).unwrap();
    assert_eq!(ranking.best_candidate(), &expected);
}
