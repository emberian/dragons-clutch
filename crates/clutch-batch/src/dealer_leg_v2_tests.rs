//! Adversarial tests for the pure RelationV2 covered-dealer join.

extern crate std;

use crate::dealer_leg_v2::{
    verify_claimed_dealer_allocations_v2, verify_economic_candidate_with_dealer_v2,
    DealerCashPolicyV2, DealerErrorV2, DealerFacilityBindingV2, DealerLegCandidateV2,
    DealerOrderRowV2, DealerReceiptV2, DEALER_LEG_VERSION_V2, EMPTY_DEALER_ORDER_ROW_V2,
    MAX_DEALER_ROWS_V2,
};
use crate::relation_v2::{
    price_semantics_digest_v2, verify_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2,
    EconomicDomainV2, EconomicErrorV2, EconomicOrderV2, PricePreconditionV2,
    ECONOMIC_RELATION_VERSION_V2,
};
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

fn price_with(prices: [u64; 16]) -> PricePreconditionV2 {
    let semantic_price_digest =
        price_semantics_digest_v2(&domain(), &prices).expect("canonical fixture price");
    PricePreconditionV2 {
        policy_digest: id(4),
        semantic_price_digest,
        prices,
    }
}

fn price() -> PricePreconditionV2 {
    let mut prices = [0u64; 16];
    prices[0] = SCALE / 2;
    prices[1] = SCALE / 2;
    price_with(prices)
}

fn coefficients(first: u64, second: u64) -> [u64; 16] {
    let mut result = [0u64; 16];
    result[0] = first;
    result[1] = second;
    result
}

fn order(order_id: u8, side: Side, first: u64, second: u64, quantity: u64) -> EconomicOrderV2 {
    EconomicOrderV2 {
        order_id: id(order_id),
        side,
        coefficients: coefficients(first, second),
        quantity,
        minimum_fill: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: 7,
        limit_value_price_units_per_unit: match side {
            Side::Buy => u128::MAX,
            Side::Sell => 0,
        },
    }
}

fn book_of(orders: &[EconomicOrderV2]) -> EconomicBookV2 {
    let mut book = EconomicBookV2::empty();
    book.orders[..orders.len()].copy_from_slice(orders);
    book.len = u8::try_from(orders.len()).expect("fixture book fits");
    book
}

fn candidate(fills: &[u64]) -> EconomicCandidateV2 {
    let mut candidate = EconomicCandidateV2::EMPTY;
    candidate.fills[..fills.len()].copy_from_slice(fills);
    candidate
}

fn row(
    order_id: u8,
    dealer_fill_units: u64,
    maximum_cash_in_atoms: u64,
    minimum_cash_out_atoms: u64,
    external_fee_atoms: u64,
) -> DealerOrderRowV2 {
    DealerOrderRowV2 {
        order_id: id(order_id),
        dealer_fill_units,
        maximum_cash_in_atoms,
        minimum_cash_out_atoms,
        external_fee_atoms,
    }
}

fn dealer_of(rows: &[DealerOrderRowV2], cash_in: u64, cash_out: u64) -> DealerLegCandidateV2 {
    let mut padded = [EMPTY_DEALER_ORDER_ROW_V2; MAX_DEALER_ROWS_V2];
    padded[..rows.len()].copy_from_slice(rows);
    DealerLegCandidateV2 {
        facility: DealerFacilityBindingV2 {
            version: DEALER_LEG_VERSION_V2,
            facility_semantics_digest: id(20),
            policy_semantics_digest: id(21),
            pre_generation: 9,
        },
        cash_policy: DealerCashPolicyV2::MinimumGrossHamiltonV1,
        receipt: DealerReceiptV2 {
            trader_cash_in_atoms: cash_in,
            trader_cash_out_atoms: cash_out,
        },
        rows: padded,
        row_count: u8::try_from(rows.len()).expect("fixture rows fit"),
    }
}

#[test]
fn dealer_join_is_additive_and_derives_the_unique_aggregate_leg() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1]);
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &book, &price(), &economic),
        Err(EconomicErrorV2::OutcomeConservationMismatch { outcome: 0 })
    );

    let dealer = dealer_of(&[row(1, 1, 5, 0, 2)], 5, 0);
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &dealer)
            .unwrap();
    assert_eq!(verified.trade.sell_to_users[..2], [1, 0]);
    assert_eq!(verified.trade.buy_from_users[..2], [0, 0]);
    assert_eq!(verified.aggregate_buy_flow[..2], [1, 0]);
    assert_eq!(verified.aggregate_sell_flow[..2], [1, 0]);
    assert_eq!(verified.direct_flow[..2], [1, 0]);
    assert_eq!(verified.allocations[0].user_cash_in_atoms, 5);
    assert_eq!(verified.allocations[0].user_cash_out_atoms, 0);
    assert_eq!(verified.total_external_fee_atoms, 2);
}

#[test]
fn mixed_sign_outcomes_and_net_cash_close_exactly() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 2), order(2, Side::Sell, 0, 1, 3)]);
    let economic = candidate(&[2, 3]);
    let dealer = dealer_of(&[row(1, 2, 11, 0, 2), row(2, 3, 0, 7, 3)], 4, 0);
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &dealer)
            .unwrap();

    assert_eq!(verified.trade.sell_to_users[..2], [2, 0]);
    assert_eq!(verified.trade.buy_from_users[..2], [0, 3]);
    assert_eq!(verified.allocations[0].user_cash_in_atoms, 11);
    assert_eq!(verified.allocations[1].user_cash_out_atoms, 7);
    assert_eq!(verified.total_external_fee_atoms, 5);
    assert_eq!(11u128, 7u128 + 4u128);
}

#[test]
fn buyer_and_seller_hamilton_ties_prefer_smaller_immutable_id() {
    let buyers = book_of(&[order(1, Side::Buy, 1, 0, 1), order(2, Side::Buy, 1, 0, 1)]);
    let buy_candidate = candidate(&[1, 1]);
    let buy_dealer = dealer_of(&[row(1, 1, 10, 0, 0), row(2, 1, 10, 0, 0)], 5, 0);
    let bought = verify_economic_candidate_with_dealer_v2(
        &domain(),
        &buyers,
        &price(),
        &buy_candidate,
        &buy_dealer,
    )
    .unwrap();
    assert_eq!(bought.allocations[0].user_cash_in_atoms, 3);
    assert_eq!(bought.allocations[1].user_cash_in_atoms, 2);

    let sellers = book_of(&[order(1, Side::Sell, 1, 0, 1), order(2, Side::Sell, 1, 0, 1)]);
    let sell_candidate = candidate(&[1, 1]);
    let sell_dealer = dealer_of(&[row(1, 1, 0, 0, 0), row(2, 1, 0, 0, 0)], 0, 5);
    let sold = verify_economic_candidate_with_dealer_v2(
        &domain(),
        &sellers,
        &price(),
        &sell_candidate,
        &sell_dealer,
    )
    .unwrap();
    assert_eq!(sold.allocations[0].user_cash_out_atoms, 3);
    assert_eq!(sold.allocations[1].user_cash_out_atoms, 2);
}

#[test]
fn buyer_caps_and_zero_weights_refuse_without_fallbacks() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1), order(2, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1, 1]);
    let at_cap = dealer_of(&[row(1, 1, 2, 0, 0), row(2, 1, 3, 0, 0)], 5, 0);
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &at_cap)
            .unwrap();
    assert_eq!(verified.allocations[0].user_cash_in_atoms, 2);
    assert_eq!(verified.allocations[1].user_cash_in_atoms, 3);

    let insufficient = dealer_of(&[row(1, 1, 2, 0, 0), row(2, 1, 3, 0, 0)], 6, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &insufficient,
        ),
        Err(DealerErrorV2::BuyerCapacityInsufficient)
    );

    let zero_weight = dealer_of(&[row(1, 1, 0, 0, 0), row(2, 1, 0, 0, 0)], 1, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &zero_weight,
        ),
        Err(DealerErrorV2::ZeroAllocationWeight)
    );
}

#[test]
fn row_identity_fill_padding_and_flow_are_canonical() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 2), order(2, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[2, 1]);

    let duplicate = dealer_of(&[row(1, 1, 2, 0, 0), row(1, 1, 2, 0, 0)], 4, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &duplicate,),
        Err(DealerErrorV2::NonCanonicalRowOrder { row: 1 })
    );

    let partial_flow = dealer_of(&[row(1, 1, 3, 0, 0), row(2, 1, 3, 0, 0)], 3, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &partial_flow,
        ),
        Err(DealerErrorV2::DealerFlowMismatch)
    );

    let mut bad_padding = dealer_of(&[row(1, 2, 3, 0, 0), row(2, 1, 3, 0, 0)], 3, 0);
    bad_padding.rows[2] = row(3, 1, 1, 0, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &bad_padding,
        ),
        Err(DealerErrorV2::NonCanonicalRowPadding { row: 2 })
    );
}

#[test]
fn exact_recomputation_is_the_only_allocation_authority() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1), order(2, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1, 1]);
    let dealer = dealer_of(&[row(1, 1, 10, 0, 0), row(2, 1, 10, 0, 0)], 5, 0);
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &dealer)
            .unwrap();
    assert_eq!(
        verify_claimed_dealer_allocations_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &dealer,
            &verified.allocations,
        )
        .unwrap(),
        verified
    );

    let mut forged = verified.allocations;
    forged[0].user_cash_in_atoms -= 1;
    forged[1].user_cash_in_atoms += 1;
    assert_eq!(
        verify_claimed_dealer_allocations_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &dealer,
            &forged,
        ),
        Err(DealerErrorV2::AllocationMismatch { row: 0 })
    );
}

#[test]
fn fees_are_external_and_semantically_bound_but_never_dealer_cash() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1]);
    let first = dealer_of(&[row(1, 1, 5, 0, 7)], 5, 0);
    let second = dealer_of(&[row(1, 1, 5, 0, 9)], 5, 0);
    let first_verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &first)
            .unwrap();
    let second_verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &second)
            .unwrap();
    assert_eq!(first_verified.allocations[0].user_cash_in_atoms, 5);
    assert_eq!(second_verified.allocations[0].user_cash_in_atoms, 5);
    assert_eq!(first_verified.total_external_fee_atoms, 7);
    assert_eq!(second_verified.total_external_fee_atoms, 9);
    assert_ne!(
        first_verified.dealer_economic_candidate_digest,
        second_verified.dealer_economic_candidate_digest
    );
}

#[test]
fn proof_body_is_not_an_economic_or_rank_coordinate() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ExternalProofBody {
        representation_digest: [u8; 32],
        byte_len: u64,
    }

    let proof_a = ExternalProofBody {
        representation_digest: id(90),
        byte_len: 8,
    };
    let proof_b = ExternalProofBody {
        representation_digest: id(91),
        byte_len: 80,
    };
    assert_ne!(proof_a, proof_b);

    let book = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1]);
    let project = |_proof: &ExternalProofBody| dealer_of(&[row(1, 1, 5, 0, 0)], 5, 0);
    let first = verify_economic_candidate_with_dealer_v2(
        &domain(),
        &book,
        &price(),
        &economic,
        &project(&proof_a),
    )
    .unwrap();
    let second = verify_economic_candidate_with_dealer_v2(
        &domain(),
        &book,
        &price(),
        &economic,
        &project(&proof_b),
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.score.digest, first.dealer_economic_candidate_digest);
}

#[test]
fn zero_flow_and_noncanonical_two_way_receipts_refuse() {
    let balanced = book_of(&[order(1, Side::Buy, 1, 0, 1), order(2, Side::Sell, 1, 0, 1)]);
    let economic = candidate(&[1, 1]);
    let dealer = dealer_of(&[row(1, 1, 1, 0, 0)], 1, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &balanced,
            &price(),
            &economic,
            &dealer,
        ),
        Err(DealerErrorV2::ZeroDealerFlow)
    );

    let unbalanced = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let both_directions = dealer_of(&[row(1, 1, 1, 0, 0)], 1, 1);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &unbalanced,
            &price(),
            &candidate(&[1]),
            &both_directions,
        ),
        Err(DealerErrorV2::NonCanonicalReceipt)
    );
}

#[test]
fn every_fixed_width_accumulator_refuses_overflow() {
    let sellers = book_of(&[order(1, Side::Sell, 1, 0, 1), order(2, Side::Sell, 1, 0, 1)]);
    let economic = candidate(&[1, 1]);
    let minimum_overflow = dealer_of(&[row(1, 1, 0, u64::MAX, 0), row(2, 1, 0, 1, 0)], 0, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &sellers,
            &price(),
            &economic,
            &minimum_overflow,
        ),
        Err(DealerErrorV2::CashTotalOverflow)
    );

    let fee_overflow = dealer_of(&[row(1, 1, 0, 0, u64::MAX), row(2, 1, 0, 0, 1)], 0, 0);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &sellers,
            &price(),
            &economic,
            &fee_overflow,
        ),
        Err(DealerErrorV2::CashTotalOverflow)
    );

    let huge_portfolio = book_of(&[order(1, Side::Sell, u64::MAX, 1, 1)]);
    let mut endpoint_prices = [0u64; 16];
    endpoint_prices[1] = SCALE;
    let weight_overflow = dealer_of(&[row(1, 1, 0, 0, 0)], 0, 1);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &huge_portfolio,
            &price_with(endpoint_prices),
            &candidate(&[1]),
            &weight_overflow,
        ),
        Err(DealerErrorV2::AllocationWeightOverflow)
    );
}
