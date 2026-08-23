//! Adversarial tests for the pure RelationV2 covered-dealer join.

extern crate std;

use crate::dealer_leg_v2::{
    dealer_quote_semantics_digest_v2, dealer_upstream_economic_candidate_digest_v2,
    exact_mul_div_rem, verify_claimed_dealer_allocations_v2 as verify_claimed_core,
    verify_economic_candidate_with_dealer_v2 as verify_join_core, AggregateDealerTradeV2,
    DealerCashAllocationV2, DealerCashPolicyV2, DealerErrorV2, DealerFacilityBindingV2,
    DealerFillRowV2, DealerLegCandidateV2, DealerLegVerdictV2, DealerQuotePreconditionV2,
    DealerQuoteRowV2, DealerReceiptV2, DEALER_LEG_VERSION_V2, EMPTY_DEALER_FILL_ROW_V2,
    EMPTY_DEALER_QUOTE_ROW_V2, MAX_DEALER_ROWS_V2,
};
use crate::relation_v2::{
    price_semantics_digest_v2, verify_economic_candidate_v2, EconomicBookV2, EconomicCandidateV2,
    EconomicDomainV2, EconomicErrorV2, EconomicOrderV2, PricePreconditionV2,
    ECONOMIC_RELATION_VERSION_V2,
};
use crate::{PartialPolicy, Side, MAX_ORDERS};

const SCALE: u64 = 10_000;

#[test]
fn exact_mul_div_matches_small_products_and_a_double_width_case() {
    let mut multiplicand = 0u128;
    while multiplicand < 33 {
        let mut multiplier = 0u128;
        while multiplier < 33 {
            let mut denominator = 1u128;
            while denominator < 33 {
                let product = multiplicand * multiplier;
                assert_eq!(
                    exact_mul_div_rem(multiplicand, multiplier, denominator).unwrap(),
                    (product / denominator, product % denominator)
                );
                denominator += 1;
            }
            multiplier += 1;
        }
        multiplicand += 1;
    }

    let weight = u128::from(u64::MAX);
    let denominator = weight * 32;
    let multiplicand = denominator - 1;
    assert!(multiplicand.checked_mul(weight).is_none());
    assert_eq!(
        exact_mul_div_rem(multiplicand, weight, denominator).unwrap(),
        (weight - 1, denominator - weight)
    );
    assert_eq!(
        exact_mul_div_rem(u128::MAX, u128::MAX - 1, u128::MAX).unwrap(),
        (u128::MAX - 1, 0)
    );
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FixtureDealerRow {
    order_id: [u8; 32],
    dealer_fill_units: u64,
    maximum_cash_in_atoms: u64,
    minimum_cash_out_atoms: u64,
    external_fee_atoms: u64,
}

const EMPTY_FIXTURE_DEALER_ROW: FixtureDealerRow = FixtureDealerRow {
    order_id: [0; 32],
    dealer_fill_units: 0,
    maximum_cash_in_atoms: 0,
    minimum_cash_out_atoms: 0,
    external_fee_atoms: 0,
};

fn row(
    order_id: u8,
    dealer_fill_units: u64,
    maximum_cash_in_atoms: u64,
    minimum_cash_out_atoms: u64,
    external_fee_atoms: u64,
) -> FixtureDealerRow {
    FixtureDealerRow {
        order_id: id(order_id),
        dealer_fill_units,
        maximum_cash_in_atoms,
        minimum_cash_out_atoms,
        external_fee_atoms,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DealerFixture {
    candidate: DealerLegCandidateV2,
    quote: DealerQuotePreconditionV2,
}

fn derived_trade(book: &EconomicBookV2, economic: &EconomicCandidateV2) -> AggregateDealerTradeV2 {
    let mut user_buy = [0u64; 16];
    let mut user_sell = [0u64; 16];
    let mut order_index = 0usize;
    while order_index < usize::from(book.len) {
        let order = book.orders[order_index];
        let mut outcome = 0usize;
        while outcome < usize::from(domain().outcome_count) {
            let leg = order.coefficients[outcome]
                .checked_mul(economic.fills[order_index])
                .expect("fixture flow fits");
            let aggregate = match order.side {
                Side::Buy => &mut user_buy[outcome],
                Side::Sell => &mut user_sell[outcome],
            };
            *aggregate = aggregate.checked_add(leg).expect("fixture aggregate fits");
            outcome += 1;
        }
        order_index += 1;
    }
    let mut trade = AggregateDealerTradeV2 {
        sell_to_users: [0; 16],
        buy_from_users: [0; 16],
    };
    let mut outcome = 0usize;
    while outcome < usize::from(domain().outcome_count) {
        let demand = user_buy[outcome]
            .checked_add(economic.virtual_merge)
            .expect("fixture demand fits");
        let supply = user_sell[outcome]
            .checked_add(economic.virtual_split)
            .expect("fixture supply fits");
        if demand > supply {
            trade.sell_to_users[outcome] = demand - supply;
        } else {
            trade.buy_from_users[outcome] = supply - demand;
        }
        outcome += 1;
    }
    trade
}

fn dealer_of(
    book: &EconomicBookV2,
    economic: &EconomicCandidateV2,
    rows: &[FixtureDealerRow],
    cash_in: u64,
    cash_out: u64,
) -> DealerFixture {
    dealer_of_with_price(book, economic, &price(), rows, cash_in, cash_out)
}

fn dealer_of_with_price(
    book: &EconomicBookV2,
    economic: &EconomicCandidateV2,
    quoted_price: &PricePreconditionV2,
    rows: &[FixtureDealerRow],
    cash_in: u64,
    cash_out: u64,
) -> DealerFixture {
    let mut fill_rows = [EMPTY_DEALER_FILL_ROW_V2; MAX_DEALER_ROWS_V2];
    let mut quote_rows = [EMPTY_DEALER_QUOTE_ROW_V2; MAX_DEALER_ROWS_V2];
    let mut index = 0usize;
    while index < rows.len() {
        let supplied = rows[index];
        fill_rows[index] = DealerFillRowV2 {
            order_id: supplied.order_id,
            dealer_fill_units: supplied.dealer_fill_units,
        };
        quote_rows[index] = DealerQuoteRowV2 {
            order_id: supplied.order_id,
            maximum_cash_in_atoms: supplied.maximum_cash_in_atoms,
            minimum_cash_out_atoms: supplied.minimum_cash_out_atoms,
            external_fee_atoms: supplied.external_fee_atoms,
        };
        index += 1;
    }
    let candidate = DealerLegCandidateV2 {
        rows: fill_rows,
        row_count: u8::try_from(rows.len()).expect("fixture rows fit"),
    };
    let mut quote = DealerQuotePreconditionV2 {
        upstream_economic_candidate_digest: dealer_upstream_economic_candidate_digest_v2(
            &domain(),
            book,
            quoted_price,
            economic,
        )
        .expect("fixture RelationV2 projection is canonical"),
        facility: DealerFacilityBindingV2 {
            version: DEALER_LEG_VERSION_V2,
            facility_semantics_digest: id(20),
            policy_semantics_digest: id(21),
            pre_generation: 9,
        },
        cash_policy: DealerCashPolicyV2::MinimumGrossHamiltonV1,
        fee_policy_semantics_digest: id(22),
        trade: derived_trade(book, economic),
        receipt: DealerReceiptV2 {
            dealer_net_cash_in_atoms: cash_in,
            dealer_net_cash_out_atoms: cash_out,
        },
        rows: quote_rows,
        semantic_quote_digest: [0; 32],
    };
    quote.semantic_quote_digest = dealer_quote_semantics_digest_v2(&domain(), &candidate, &quote)
        .expect("fixture quote is canonical");
    DealerFixture { candidate, quote }
}

fn verify_economic_candidate_with_dealer_v2(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    economic: &EconomicCandidateV2,
    dealer: &DealerFixture,
) -> Result<DealerLegVerdictV2, DealerErrorV2> {
    verify_join_core(
        domain,
        book,
        price,
        economic,
        &dealer.candidate,
        &dealer.quote,
    )
}

fn verify_claimed_dealer_allocations_v2(
    domain: &EconomicDomainV2,
    book: &EconomicBookV2,
    price: &PricePreconditionV2,
    economic: &EconomicCandidateV2,
    dealer: &DealerFixture,
    claimed: &[DealerCashAllocationV2; MAX_DEALER_ROWS_V2],
) -> Result<DealerLegVerdictV2, DealerErrorV2> {
    verify_claimed_core(
        domain,
        book,
        price,
        economic,
        &dealer.candidate,
        &dealer.quote,
        claimed,
    )
}

#[test]
fn dealer_join_is_additive_and_derives_the_unique_aggregate_leg() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1]);
    assert_eq!(
        verify_economic_candidate_v2(&domain(), &book, &price(), &economic),
        Err(EconomicErrorV2::OutcomeConservationMismatch { outcome: 0 })
    );

    let dealer = dealer_of(&book, &economic, &[row(1, 1, 5, 0, 2)], 5, 0);
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
    let dealer = dealer_of(
        &book,
        &economic,
        &[row(1, 2, 11, 0, 2), row(2, 3, 0, 7, 3)],
        4,
        0,
    );
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &dealer)
            .unwrap();

    assert_eq!(verified.trade.sell_to_users[..2], [2, 0]);
    assert_eq!(verified.trade.buy_from_users[..2], [0, 3]);
    assert_eq!(verified.allocations[0].user_cash_in_atoms, 11);
    assert_eq!(verified.allocations[1].user_cash_out_atoms, 7);
    assert_eq!(verified.total_external_fee_atoms, 5);
    let user_cash_in = u128::from(verified.allocations[0].user_cash_in_atoms);
    let user_cash_out = u128::from(verified.allocations[1].user_cash_out_atoms);
    assert_eq!(
        user_cash_in + u128::from(dealer.quote.receipt.dealer_net_cash_out_atoms),
        user_cash_out + u128::from(dealer.quote.receipt.dealer_net_cash_in_atoms)
    );
}

#[test]
fn buyer_and_seller_hamilton_ties_prefer_smaller_immutable_id() {
    let buyers = book_of(&[order(1, Side::Buy, 1, 0, 1), order(2, Side::Buy, 1, 0, 1)]);
    let buy_candidate = candidate(&[1, 1]);
    let buy_dealer = dealer_of(
        &buyers,
        &buy_candidate,
        &[row(1, 1, 10, 0, 0), row(2, 1, 10, 0, 0)],
        5,
        0,
    );
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
    let sell_dealer = dealer_of(
        &sellers,
        &sell_candidate,
        &[row(1, 1, 0, 0, 0), row(2, 1, 0, 0, 0)],
        0,
        5,
    );
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
fn full_relation_book_width_is_not_confused_with_an_lp_roster() {
    let mut book = EconomicBookV2::empty();
    let mut economic = EconomicCandidateV2::EMPTY;
    let mut rows = [EMPTY_FIXTURE_DEALER_ROW; MAX_DEALER_ROWS_V2];
    let mut index = 0usize;
    while index < MAX_ORDERS {
        let order_id = u8::try_from(index + 1).expect("RelationV2 capacity fits an identity byte");
        book.orders[index] = order(order_id, Side::Buy, 1, 0, 1);
        economic.fills[index] = 1;
        rows[index] = row(order_id, 1, 1, 0, 0);
        index += 1;
    }
    book.len = u8::try_from(MAX_ORDERS).expect("RelationV2 capacity fits its length field");
    let dealer = dealer_of(
        &book,
        &economic,
        &rows,
        u64::try_from(MAX_ORDERS).expect("RelationV2 capacity fits cash atoms"),
        0,
    );
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &dealer)
            .unwrap();

    assert_eq!(verified.allocation_count, book.len);
    assert_eq!(
        verified.trade.sell_to_users[0],
        u64::try_from(MAX_ORDERS).expect("RelationV2 capacity fits Egg atoms")
    );
    assert_eq!(verified.allocations[MAX_ORDERS - 1].order_id, id(64));
    assert_eq!(verified.allocations[MAX_ORDERS - 1].user_cash_in_atoms, 1);
}

#[test]
fn buyer_caps_and_zero_weights_refuse_without_fallbacks() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1), order(2, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1, 1]);
    let at_cap = dealer_of(
        &book,
        &economic,
        &[row(1, 1, 2, 0, 0), row(2, 1, 3, 0, 0)],
        5,
        0,
    );
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &at_cap)
            .unwrap();
    assert_eq!(verified.allocations[0].user_cash_in_atoms, 2);
    assert_eq!(verified.allocations[1].user_cash_in_atoms, 3);

    let insufficient = dealer_of(
        &book,
        &economic,
        &[row(1, 1, 2, 0, 0), row(2, 1, 3, 0, 0)],
        6,
        0,
    );
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

    let zero_weight = dealer_of(
        &book,
        &economic,
        &[row(1, 1, 0, 0, 0), row(2, 1, 0, 0, 0)],
        1,
        0,
    );
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

    let mut duplicate = dealer_of(
        &book,
        &economic,
        &[row(1, 1, 2, 0, 0), row(2, 1, 2, 0, 0)],
        4,
        0,
    );
    duplicate.candidate.rows[1].order_id = id(1);
    duplicate.quote.rows[1].order_id = id(1);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &duplicate,),
        Err(DealerErrorV2::NonCanonicalRowOrder { row: 1 })
    );

    let partial_flow = dealer_of(
        &book,
        &economic,
        &[row(1, 1, 3, 0, 0), row(2, 1, 3, 0, 0)],
        3,
        0,
    );
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

    let mut bad_padding = dealer_of(
        &book,
        &economic,
        &[row(1, 2, 3, 0, 0), row(2, 1, 3, 0, 0)],
        3,
        0,
    );
    bad_padding.candidate.rows[2] = DealerFillRowV2 {
        order_id: id(3),
        dealer_fill_units: 1,
    };
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
    let dealer = dealer_of(
        &book,
        &economic,
        &[row(1, 1, 10, 0, 0), row(2, 1, 10, 0, 0)],
        5,
        0,
    );
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
    let first = dealer_of(&book, &economic, &[row(1, 1, 5, 0, 7)], 5, 0);
    let second = dealer_of(&book, &economic, &[row(1, 1, 5, 0, 9)], 5, 0);
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
fn fixed_authenticated_quote_identity_refuses_free_cash_and_digest_grinding() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1]);
    let baseline = dealer_of(&book, &economic, &[row(1, 1, 5, 0, 7)], 5, 0);
    let accepted =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &baseline)
            .unwrap();
    assert_eq!(
        accepted.dealer_quote_semantics_digest,
        baseline.quote.semantic_quote_digest
    );

    let mut free_cash = baseline;
    free_cash.quote.receipt.dealer_net_cash_in_atoms = 0;
    assert_ne!(
        dealer_quote_semantics_digest_v2(&domain(), &free_cash.candidate, &free_cash.quote)
            .unwrap(),
        baseline.quote.semantic_quote_digest
    );
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &free_cash,),
        Err(DealerErrorV2::DealerQuoteSemanticDigestMismatch)
    );

    let mut arbitrary_envelope = baseline;
    arbitrary_envelope.quote.rows[0].maximum_cash_in_atoms = 99;
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &arbitrary_envelope,
        ),
        Err(DealerErrorV2::DealerQuoteSemanticDigestMismatch)
    );

    let mut arbitrary_fee = baseline;
    arbitrary_fee.quote.rows[0].external_fee_atoms = 0;
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &arbitrary_fee,
        ),
        Err(DealerErrorV2::DealerQuoteSemanticDigestMismatch)
    );

    let mut generation_grind = baseline;
    generation_grind.quote.facility.pre_generation += 1;
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &generation_grind,
        ),
        Err(DealerErrorV2::DealerQuoteSemanticDigestMismatch)
    );

    let mut fee_policy_grind = baseline;
    fee_policy_grind.quote.fee_policy_semantics_digest = id(23);
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &fee_policy_grind,
        ),
        Err(DealerErrorV2::DealerQuoteSemanticDigestMismatch)
    );

    let mut wrong_trade = baseline;
    wrong_trade.quote.trade.sell_to_users[0] += 1;
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &price(),
            &economic,
            &wrong_trade,
        ),
        Err(DealerErrorV2::QuoteTradeMismatch)
    );
}

#[test]
fn quote_for_same_fill_and_trade_cannot_replay_across_price_semantics() {
    let book = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let economic = candidate(&[1]);
    let baseline_price = price();
    let baseline = dealer_of_with_price(
        &book,
        &economic,
        &baseline_price,
        &[row(1, 1, 5, 0, 0)],
        5,
        0,
    );

    let mut alternate_prices = [0u64; 16];
    alternate_prices[0] = 6_000;
    alternate_prices[1] = 4_000;
    let alternate_price = price_with(alternate_prices);
    assert_eq!(baseline.quote.trade, derived_trade(&book, &economic));
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &book,
            &alternate_price,
            &economic,
            &baseline,
        ),
        Err(DealerErrorV2::UpstreamEconomicCandidateDigestMismatch)
    );

    let alternate = dealer_of_with_price(
        &book,
        &economic,
        &alternate_price,
        &[row(1, 1, 5, 0, 0)],
        5,
        0,
    );
    let accepted = verify_economic_candidate_with_dealer_v2(
        &domain(),
        &book,
        &alternate_price,
        &economic,
        &alternate,
    )
    .unwrap();
    assert_eq!(baseline.quote.trade, alternate.quote.trade);
    assert_ne!(
        baseline.quote.upstream_economic_candidate_digest,
        alternate.quote.upstream_economic_candidate_digest
    );
    assert_ne!(
        baseline.quote.semantic_quote_digest,
        accepted.dealer_quote_semantics_digest
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
    let project =
        |_proof: &ExternalProofBody| dealer_of(&book, &economic, &[row(1, 1, 5, 0, 0)], 5, 0);
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
    assert_eq!(
        first.dealer_quote_semantics_digest,
        second.dealer_quote_semantics_digest
    );
    assert_eq!(first.score.digest, first.dealer_economic_candidate_digest);
}

#[test]
fn zero_flow_and_noncanonical_two_way_receipts_refuse() {
    let balanced = book_of(&[order(1, Side::Buy, 1, 0, 1), order(2, Side::Sell, 1, 0, 1)]);
    let economic = candidate(&[1, 1]);
    let unbalanced = book_of(&[order(1, Side::Buy, 1, 0, 1)]);
    let unbalanced_economic = candidate(&[1]);
    let dealer = dealer_of(
        &unbalanced,
        &unbalanced_economic,
        &[row(1, 1, 1, 0, 0)],
        1,
        0,
    );
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

    let mut both_directions = dealer_of(
        &unbalanced,
        &unbalanced_economic,
        &[row(1, 1, 1, 0, 0)],
        1,
        0,
    );
    both_directions.quote.receipt.dealer_net_cash_out_atoms = 1;
    assert_eq!(
        verify_economic_candidate_with_dealer_v2(
            &domain(),
            &unbalanced,
            &price(),
            &unbalanced_economic,
            &both_directions,
        ),
        Err(DealerErrorV2::NonCanonicalReceipt)
    );
}

#[test]
fn full_width_u64_rows_use_exact_wide_aggregates_and_mul_div() {
    let mut book = EconomicBookV2::empty();
    let mut economic = EconomicCandidateV2::EMPTY;
    let mut rows = [EMPTY_FIXTURE_DEALER_ROW; MAX_DEALER_ROWS_V2];
    let midpoint = MAX_ORDERS / 2;
    let mut index = 0usize;
    while index < MAX_ORDERS {
        let order_id = u8::try_from(index + 1).expect("RelationV2 capacity fits an identity byte");
        let (side, first, second, maximum_in, minimum_out) = if index < midpoint {
            (Side::Buy, 1, 0, u64::MAX, 0)
        } else if index + 1 == MAX_ORDERS {
            (Side::Sell, 0, 1, 0, u64::MAX - 1)
        } else {
            (Side::Sell, 0, 1, 0, u64::MAX)
        };
        book.orders[index] = order(order_id, side, first, second, 1);
        economic.fills[index] = 1;
        rows[index] = row(order_id, 1, maximum_in, minimum_out, u64::MAX);
        index += 1;
    }
    book.len = u8::try_from(MAX_ORDERS).expect("RelationV2 capacity fits its length field");
    let dealer = dealer_of(&book, &economic, &rows, 0, 0);
    let verified =
        verify_economic_candidate_with_dealer_v2(&domain(), &book, &price(), &economic, &dealer)
            .unwrap();

    let buyer_count = u128::try_from(midpoint).expect("fixture count fits");
    let row_max = u128::from(u64::MAX);
    assert_eq!(verified.allocations[0].user_cash_in_atoms, u64::MAX);
    assert_eq!(
        verified.allocations[midpoint - 1].user_cash_in_atoms,
        u64::MAX - 1
    );
    assert_eq!(
        verified.allocations[MAX_ORDERS - 1].user_cash_out_atoms,
        u64::MAX - 1
    );
    assert_eq!(
        verified.total_external_fee_atoms,
        u128::try_from(MAX_ORDERS).expect("fixture count fits") * row_max
    );
    assert_eq!(
        buyer_count * row_max - 1,
        verified.allocations[..midpoint]
            .iter()
            .map(|allocation| u128::from(allocation.user_cash_in_atoms))
            .sum()
    );

    let huge_portfolio = book_of(&[order(1, Side::Sell, u64::MAX, 1, 1)]);
    let huge_economic = candidate(&[1]);
    let mut endpoint_prices = [0u64; 16];
    endpoint_prices[1] = SCALE;
    let endpoint_price = price_with(endpoint_prices);
    let wide_weight = dealer_of_with_price(
        &huge_portfolio,
        &huge_economic,
        &endpoint_price,
        &[row(1, 1, 0, 0, 0)],
        0,
        1,
    );
    let weighted = verify_economic_candidate_with_dealer_v2(
        &domain(),
        &huge_portfolio,
        &endpoint_price,
        &huge_economic,
        &wide_weight,
    )
    .unwrap();
    assert_eq!(weighted.allocations[0].user_cash_out_atoms, 1);
}
