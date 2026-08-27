use clutch_batch::dealer_leg_v2::{
    dealer_quote_semantics_digest_v2, dealer_upstream_economic_candidate_digest_v2,
    verify_economic_candidate_with_dealer_v2, AggregateDealerTradeV2, DealerCashPolicyV2,
    DealerFacilityBindingV2, DealerFillRowV2, DealerLegCandidateV2, DealerQuotePreconditionV2,
    DealerQuoteRowV2, DealerReceiptV2, VerifiedDealerLegV2, DEALER_LEG_VERSION_V2,
    EMPTY_DEALER_FILL_ROW_V2, EMPTY_DEALER_QUOTE_ROW_V2, MAX_DEALER_ROWS_V2,
};
use clutch_batch::relation_v2::{
    price_semantics_digest_v2, EconomicBookV2, EconomicCandidateV2, EconomicDomainV2,
    EconomicOrderV2, PricePreconditionV2, ECONOMIC_RELATION_VERSION_V2,
};
use clutch_batch::{PartialPolicy, Side};
use clutch_bounded_liquidity_facility::signed_dealer::{
    signed_quadratic_price_vector, signed_rounded_quadratic_potential, DealerError, LpPositionV1,
    SignedDealerPhase, SignedDealerPolicyV1, SignedDealerStateV1, SignedDealerTradeV1,
    MAX_LIVE_POOL_ATOMS, MAX_LPS, MAX_TERMINAL_POOL_ATOMS,
};
use clutch_bounded_liquidity_facility::{MAX_ATOMS, MAX_OUTCOMES};

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn uvec(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut result = [0; MAX_OUTCOMES];
    result[..values.len()].copy_from_slice(values);
    result
}

fn ivec(values: &[i64]) -> [i64; MAX_OUTCOMES] {
    let mut result = [0; MAX_OUTCOMES];
    result[..values.len()].copy_from_slice(values);
    result
}

fn policy() -> SignedDealerPolicyV1 {
    SignedDealerPolicyV1 {
        policy_id: id(1),
        market: id(2),
        terms_digest: id(3),
        instance_id: id(4),
        claim_domain_digest: id(5),
        outcome_count: 3,
        payout_denominator: 2,
        initial_price_denominator: 3,
        initial_price_weights: uvec(&[1, 1, 1]),
        depth_atoms: 120,
        max_net_buy: uvec(&[12, 12, 12]),
        max_net_sell: uvec(&[12, 12, 12]),
        capital_unit_cash_atoms: 4,
        capital_unit_eggs: uvec(&[4, 4, 4]),
        minimum_lp_shares: 3,
        maximum_lp_shares: 10,
        shutdown_queue_numerator: 1,
        shutdown_queue_denominator: 2,
        funding_deadline_slot: 5,
        trading_open_slot: 10,
        trading_close_slot: 20,
        maturity_slot: 30,
    }
}

fn trade_for_endpoint(q: &[i64; MAX_OUTCOMES]) -> SignedDealerTradeV1 {
    let mut sell = [0; MAX_OUTCOMES];
    let mut buy = [0; MAX_OUTCOMES];
    for i in 0..MAX_OUTCOMES {
        if q[i] >= 0 {
            sell[i] = q[i] as u64;
        } else {
            buy[i] = (-i128::from(q[i])) as u64;
        }
    }
    SignedDealerTradeV1 {
        sell_to_users: sell,
        buy_from_users: buy,
    }
}

fn trade(sell: &[u64], buy: &[u64]) -> SignedDealerTradeV1 {
    SignedDealerTradeV1 {
        sell_to_users: uvec(sell),
        buy_from_users: uvec(buy),
    }
}

fn funded_state() -> SignedDealerStateV1 {
    let policy = policy();
    let subsidy = policy.minimum_sponsor_subsidy().unwrap();
    let mut state = SignedDealerStateV1::initialize(policy, id(6), id(7), subsidy).unwrap();
    state.contribute(1, id(8), 1).unwrap();
    state.contribute(2, id(9), 2).unwrap();
    state.activate(5).unwrap();
    state
}

#[derive(Clone, Copy)]
struct DealerJoinFixture {
    domain: EconomicDomainV2,
    book: EconomicBookV2,
    price: PricePreconditionV2,
    candidate: EconomicCandidateV2,
    dealer: DealerLegCandidateV2,
    quote: DealerQuotePreconditionV2,
}

fn dealer_join_fixture(state: &SignedDealerStateV1) -> DealerJoinFixture {
    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: state.policy.market,
        epoch_semantics_digest: state.policy.instance_id,
        relation_policy_digest: id(30),
        price_policy_digest: id(31),
        epoch_index: 1,
        outcome_count: state.policy.outcome_count,
        price_scale: 3,
    };
    let prices = uvec(&[1, 1, 1]);
    let price = PricePreconditionV2 {
        policy_digest: domain.price_policy_digest,
        semantic_price_digest: price_semantics_digest_v2(&domain, &prices).unwrap(),
        prices,
    };
    let mut book = EconomicBookV2::empty();
    book.orders[0] = EconomicOrderV2 {
        order_id: id(20),
        side: Side::Buy,
        coefficients: uvec(&[1, 0, 0]),
        quantity: 6,
        minimum_fill: 0,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: domain.epoch_index,
        limit_value_price_units_per_unit: 1,
    };
    book.len = 1;
    let mut candidate = EconomicCandidateV2::EMPTY;
    candidate.fills[0] = 6;
    let mut dealer_rows = [EMPTY_DEALER_FILL_ROW_V2; MAX_DEALER_ROWS_V2];
    dealer_rows[0] = DealerFillRowV2 {
        order_id: id(20),
        dealer_fill_units: 6,
    };
    let dealer = DealerLegCandidateV2 {
        rows: dealer_rows,
        row_count: 1,
    };
    let mut quote_rows = [EMPTY_DEALER_QUOTE_ROW_V2; MAX_DEALER_ROWS_V2];
    quote_rows[0] = DealerQuoteRowV2 {
        order_id: id(20),
        maximum_cash_in_atoms: 3,
        minimum_cash_out_atoms: 0,
        external_fee_atoms: 1,
    };
    let expected = state.quote_trade(10, trade(&[6, 0, 0], &[])).unwrap();
    let mut quote = DealerQuotePreconditionV2 {
        upstream_economic_candidate_digest: dealer_upstream_economic_candidate_digest_v2(
            &domain, &book, &price, &candidate,
        )
        .unwrap(),
        facility: DealerFacilityBindingV2 {
            version: DEALER_LEG_VERSION_V2,
            facility_semantics_digest: state.facility_id,
            policy_semantics_digest: state.policy.policy_id,
            pre_generation: state.generation,
        },
        cash_policy: DealerCashPolicyV2::MinimumGrossHamiltonV1,
        fee_policy_semantics_digest: id(32),
        trade: AggregateDealerTradeV2 {
            sell_to_users: expected.trade.sell_to_users,
            buy_from_users: expected.trade.buy_from_users,
        },
        receipt: DealerReceiptV2 {
            dealer_net_cash_in_atoms: expected.trader_cash_in_atoms,
            dealer_net_cash_out_atoms: expected.trader_cash_out_atoms,
        },
        rows: quote_rows,
        semantic_quote_digest: [0; 32],
    };
    quote.semantic_quote_digest =
        dealer_quote_semantics_digest_v2(&domain, &dealer, &quote).unwrap();
    DealerJoinFixture {
        domain,
        book,
        price,
        candidate,
        dealer,
        quote,
    }
}

fn verified_dealer_leg(fixture: &DealerJoinFixture) -> VerifiedDealerLegV2 {
    verify_economic_candidate_with_dealer_v2(
        &fixture.domain,
        &fixture.book,
        &fixture.price,
        &fixture.candidate,
        &fixture.dealer,
        &fixture.quote,
    )
    .unwrap()
}

fn refresh_quote(fixture: &mut DealerJoinFixture) {
    fixture.quote.semantic_quote_digest = [0; 32];
    fixture.quote.semantic_quote_digest =
        dealer_quote_semantics_digest_v2(&fixture.domain, &fixture.dealer, &fixture.quote).unwrap();
}

#[test]
fn sponsor_subsidy_and_lp_assets_are_distinct_before_activation() {
    let policy = policy();
    assert_eq!(policy.minimum_sponsor_subsidy(), Ok(40));
    assert_eq!(policy.minimum_sponsor_capital(), Ok(40));
    assert_eq!(
        SignedDealerStateV1::initialize(policy, id(6), id(7), 39),
        Err(DealerError::InsufficientSubsidy)
    );
    let mut state = SignedDealerStateV1::initialize(policy, id(6), id(7), 40).unwrap();
    assert_eq!(state.pool_cash_atoms, 40);
    assert_eq!(state.pool_eggs, [0; MAX_OUTCOMES]);
    let receipt = state.contribute(1, id(8), 2).unwrap();
    assert_eq!(receipt.cash_atoms, 8);
    assert_eq!(&receipt.eggs[..3], &[8, 8, 8]);
    assert_eq!(state.pool_cash_atoms, 48);
    assert_eq!(&state.pool_eggs[..3], &[8, 8, 8]);
    assert_eq!(state.sponsor_capital_atoms, 40);
}

#[test]
fn exact_lower_corner_financing_is_separate_from_loss_capital() {
    let financed = SignedDealerPolicyV1 {
        policy_id: id(1),
        market: id(2),
        terms_digest: id(3),
        instance_id: id(4),
        claim_domain_digest: id(5),
        outcome_count: 2,
        payout_denominator: 2,
        initial_price_denominator: 2,
        initial_price_weights: uvec(&[1, 1]),
        depth_atoms: 100,
        max_net_buy: uvec(&[100, 100]),
        max_net_sell: uvec(&[0, 0]),
        capital_unit_cash_atoms: 75,
        capital_unit_eggs: uvec(&[2, 2]),
        minimum_lp_shares: 1,
        maximum_lp_shares: 1,
        shutdown_queue_numerator: 1,
        shutdown_queue_denominator: 1,
        funding_deadline_slot: 1,
        trading_open_slot: 2,
        trading_close_slot: 3,
        maturity_slot: 4,
    };
    assert_eq!(financed.minimum_sponsor_subsidy(), Ok(25));
    assert_eq!(financed.minimum_sponsor_capital(), Ok(25));
    assert_eq!(
        signed_rounded_quadratic_potential(&financed, &ivec(&[-100, -100])),
        Ok(-100)
    );

    let mut underfunded = financed;
    underfunded.capital_unit_cash_atoms = 0;
    underfunded.validate().unwrap();
    assert_eq!(underfunded.minimum_sponsor_subsidy(), Ok(25));
    assert_eq!(underfunded.minimum_sponsor_capital(), Ok(100));
    assert_eq!(
        signed_rounded_quadratic_potential(&underfunded, &ivec(&[-100, -100])),
        Ok(-100)
    );
    assert_eq!(
        SignedDealerStateV1::initialize(underfunded, id(6), id(7), 24),
        Err(DealerError::InsufficientSubsidy)
    );
    assert_eq!(
        SignedDealerStateV1::initialize(underfunded, id(6), id(7), 99),
        Err(DealerError::InsufficientCoverage)
    );
    let state = SignedDealerStateV1::initialize(underfunded, id(6), id(7), 100).unwrap();
    assert_eq!(state.pool_cash_atoms, 100);
}

#[test]
fn full_box_validation_checks_mixed_adverse_price_corners() {
    let mut hostile = policy();
    hostile.depth_atoms = 30;
    assert_eq!(hostile.validate(), Err(DealerError::PriceDomain));

    // The two diagonal corners preserve the initial price, but a mixed corner
    // makes one outcome negative. Public helpers refuse the malformed policy
    // even when the requested point itself is benign.
    assert_eq!(
        signed_quadratic_price_vector(&hostile, &ivec(&[-12, -12, -12])),
        Err(DealerError::PriceDomain)
    );
    assert_eq!(
        signed_quadratic_price_vector(&hostile, &ivec(&[12, 12, 12])),
        Err(DealerError::PriceDomain)
    );
    assert_eq!(
        signed_quadratic_price_vector(&hostile, &ivec(&[-12, 12, 12])),
        Err(DealerError::PriceDomain)
    );
}

#[test]
fn dealer_buys_before_any_sale_and_conserves_custodied_assets() {
    let mut state = funded_state();
    assert_eq!(state.pool_cash_atoms, 52);
    assert_eq!(&state.pool_eggs[..3], &[12, 12, 12]);

    let buy = state.execute_trade(10, trade(&[], &[6, 0, 0])).unwrap();
    assert_eq!(&buy.new_net_sold[..3], &[-6, 0, 0]);
    assert_eq!(buy.trader_cash_out_atoms, 1);
    assert_eq!(buy.trader_cash_in_atoms, 0);
    assert_eq!(buy.new_pool_cash_atoms, 51);
    assert_eq!(&buy.new_pool_eggs[..3], &[18, 12, 12]);

    let rotate = state.execute_trade(11, trade(&[0, 4, 0], &[])).unwrap();
    assert_eq!(&rotate.new_net_sold[..3], &[-6, 4, 0]);
    assert_eq!(rotate.trader_cash_in_atoms, 1);
    assert_eq!(rotate.new_pool_cash_atoms, 52);
    assert_eq!(&rotate.new_pool_eggs[..3], &[18, 8, 12]);
    state.validate().unwrap();
}

#[test]
fn signed_endpoint_round_trips_and_common_set_purchases_are_exact() {
    let original = funded_state();
    let mut state = original;
    let first = state
        .execute_trade(10, trade_for_endpoint(&ivec(&[-4, -4, -4])))
        .unwrap();
    assert_eq!(first.trader_cash_out_atoms, 4);
    assert_eq!(&state.pool_eggs[..3], &[16, 16, 16]);
    assert_eq!(state.pool_cash_atoms, 48);
    let back = state.execute_trade(11, trade(&[4, 4, 4], &[])).unwrap();
    assert_eq!(back.trader_cash_in_atoms, 4);
    assert_eq!(state.pool_cash_atoms, original.pool_cash_atoms);
    assert_eq!(state.pool_eggs, original.pool_eggs);
    assert_eq!(state.net_sold, original.net_sold);

    let mut direct = funded_state();
    direct
        .execute_trade(10, trade_for_endpoint(&ivec(&[-6, 4, 2])))
        .unwrap();
    let mut split = funded_state();
    split.execute_trade(10, trade(&[], &[6, 0, 0])).unwrap();
    split.execute_trade(11, trade(&[0, 4, 2], &[])).unwrap();
    assert_eq!(direct.net_sold, split.net_sold);
    assert_eq!(direct.pool_cash_atoms, split.pool_cash_atoms);
    assert_eq!(direct.pool_eggs, split.pool_eggs);
}

#[test]
fn funding_withdrawal_and_failed_activation_refunds_are_exact_in_any_order() {
    let policy = policy();
    let mut first = SignedDealerStateV1::initialize(policy, id(6), id(7), 40).unwrap();
    first.contribute(1, id(8), 1).unwrap();
    first.cancel_funding(5).unwrap();
    assert_eq!(first.refund_cancelled_sponsor_capital(id(7)), Ok(40));
    let basket = first.withdraw_funding(99, id(8), 1).unwrap();
    assert_eq!(basket.cash_atoms, 4);
    assert_eq!(&basket.eggs[..3], &[4, 4, 4]);
    assert_eq!(first.pool_cash_atoms, 0);
    assert_eq!(first.pool_eggs, [0; MAX_OUTCOMES]);

    let mut second = SignedDealerStateV1::initialize(policy, id(6), id(7), 40).unwrap();
    second.contribute(1, id(8), 1).unwrap();
    second.cancel_funding(5).unwrap();
    second.withdraw_funding(99, id(8), 1).unwrap();
    assert_eq!(second.refund_cancelled_sponsor_capital(id(7)), Ok(40));
    assert_eq!(second.pool_cash_atoms, 0);
    assert_eq!(second, first);

    let mut funded = SignedDealerStateV1::initialize(policy, id(6), id(7), 40).unwrap();
    funded.contribute(1, id(8), 3).unwrap();
    let before = funded;
    assert_eq!(funded.cancel_funding(5), Err(DealerError::InvalidSchedule));
    assert_eq!(funded, before);
    funded.cancel_funding(20).unwrap();
}

#[test]
fn fixed_capacity_positions_aggregate_same_owner_and_refuse_the_ninth() {
    let mut expanded = policy();
    expanded.maximum_lp_shares = 9;
    expanded.validate().unwrap();
    let mut state = SignedDealerStateV1::initialize(expanded, id(6), id(7), 40).unwrap();
    for owner in 10..(10 + MAX_LPS as u8) {
        state.contribute(1, id(owner), 1).unwrap();
    }
    assert_eq!(
        state.positions.iter().filter(|p| p.shares != 0).count(),
        MAX_LPS
    );
    let before = state;
    assert_eq!(
        state.contribute(1, id(30), 1),
        Err(DealerError::PositionLimit)
    );
    assert_eq!(state, before);
    state.contribute(1, id(10), 1).unwrap();
    assert_eq!(state.positions[0].shares, 2);
}

#[test]
fn share_queue_is_an_irrevocable_risk_stop_not_a_withdrawal_promise() {
    let mut state = funded_state();
    state
        .execute_trade(10, trade_for_endpoint(&ivec(&[-6, 4, 0])))
        .unwrap();
    assert_eq!(state.queue_exit(11, id(8), 1), Ok(false));
    assert_eq!(state.phase, SignedDealerPhase::Trading);
    assert_eq!(state.queue_exit(12, id(9), 1), Ok(true));
    assert_eq!(state.phase, SignedDealerPhase::UnwindOnly);

    let before = state;
    assert_eq!(
        state.execute_trade(13, trade(&[], &[2, 0, 0])),
        Err(DealerError::IncreasesExposure)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.execute_trade(13, trade(&[8, 0, 0], &[0, 4, 0])),
        Err(DealerError::IncreasesExposure)
    );
    assert_eq!(state, before);

    state
        .execute_trade(13, trade(&[6, 0, 0], &[0, 4, 0]))
        .unwrap();
    assert_eq!(state.net_sold, [0; MAX_OUTCOMES]);
    assert_eq!(state.pool_eggs, uvec(&[12, 12, 12]));
    assert_eq!(state.pool_cash_atoms, 52);
}

#[test]
fn exhaustive_signed_box_preserves_lp_principal_under_every_payout() {
    for q0 in (-12..=12).step_by(4) {
        for q1 in (-12..=12).step_by(4) {
            for q2 in (-12..=12).step_by(4) {
                let q = ivec(&[q0, q1, q2]);
                let mut state = funded_state();
                if q != [0; MAX_OUTCOMES] {
                    state.execute_trade(10, trade_for_endpoint(&q)).unwrap();
                }
                let prices = signed_quadratic_price_vector(&state.policy, &q).unwrap();
                prices.validate().unwrap();
                assert_eq!(
                    prices.numerators[0] + prices.numerators[1] + prices.numerators[2],
                    prices.denominator
                );
                for w0 in 0..=2 {
                    for w1 in 0..=(2 - w0) {
                        let payout = uvec(&[w0, w1, 2 - w0 - w1]);
                        let principal = state.terminal_lp_principal(&payout).unwrap();
                        let yield_atoms = state.terminal_pool_yield(&payout).unwrap();
                        assert_eq!(principal, 24);
                        let mut resolved = state;
                        let terminal = resolved.resolve(30, payout).unwrap();
                        assert_eq!(terminal, principal + yield_atoms);
                        let lp_one = resolved
                            .positions
                            .iter()
                            .find(|position| position.owner == id(8))
                            .unwrap();
                        let lp_two = resolved
                            .positions
                            .iter()
                            .find(|position| position.owner == id(9))
                            .unwrap();
                        assert!(lp_one.terminal_claim_atoms >= 8);
                        assert!(lp_two.terminal_claim_atoms >= 16);
                        resolved.validate().unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn signed_potential_matches_an_independent_rational_oracle() {
    let policy = policy();
    for q0 in (-12..=12).step_by(2) {
        for q1 in (-12..=12).step_by(2) {
            for q2 in (-12..=12).step_by(2) {
                let q = [i128::from(q0), i128::from(q1), i128::from(q2)];
                let sum = q[0] + q[1] + q[2];
                let sum_squares = q[0] * q[0] + q[1] * q[1] + q[2] * q[2];
                let initial_dot = q[0] + q[1] + q[2];
                let numerator = 2 * 120 * 3 * initial_dot + 3 * (3 * sum_squares - sum * sum);
                let denominator = 2 * 120 * 3 * 3;
                let oracle = if numerator >= 0 {
                    (numerator + denominator - 1) / denominator
                } else {
                    numerator / denominator
                };
                assert_eq!(
                    signed_rounded_quadratic_potential(&policy, &ivec(&[q0, q1, q2])),
                    Ok(oracle as i64)
                );
            }
        }
    }
}

#[test]
fn terminal_hamilton_allocation_is_claim_order_independent_and_exact() {
    let mut resolved = funded_state();
    resolved
        .execute_trade(10, trade_for_endpoint(&ivec(&[-6, 4, 0])))
        .unwrap();
    resolved.resolve(30, uvec(&[1, 1, 0])).unwrap();
    let total = resolved.terminal_pool_atoms;
    let claim_a = resolved
        .positions
        .iter()
        .find(|position| position.owner == id(8))
        .unwrap()
        .terminal_claim_atoms;
    let claim_b = resolved
        .positions
        .iter()
        .find(|position| position.owner == id(9))
        .unwrap()
        .terminal_claim_atoms;
    assert_eq!(claim_a + claim_b, total);

    let mut a_then_b = resolved;
    assert_eq!(a_then_b.claim_terminal(id(8)), Ok(claim_a));
    assert_eq!(a_then_b.claim_terminal(id(9)), Ok(claim_b));
    assert_eq!(a_then_b.pool_cash_atoms, 0);
    assert_eq!(
        a_then_b.claim_terminal(id(8)),
        Err(DealerError::AlreadyClaimed)
    );

    let mut b_then_a = resolved;
    assert_eq!(b_then_a.claim_terminal(id(9)), Ok(claim_b));
    assert_eq!(b_then_a.claim_terminal(id(8)), Ok(claim_a));
    assert_eq!(b_then_a.pool_cash_atoms, 0);
}

#[test]
fn skewed_prior_signed_prices_and_subsidy_are_exact() {
    let mut skewed = policy();
    skewed.initial_price_denominator = 10;
    skewed.initial_price_weights = uvec(&[7, 2, 1]);
    skewed.max_net_buy = uvec(&[6, 6, 6]);
    skewed.max_net_sell = uvec(&[6, 6, 6]);
    skewed.validate().unwrap();
    assert_eq!(skewed.minimum_sponsor_subsidy(), Ok(81));
    let prices = signed_quadratic_price_vector(&skewed, &[0; MAX_OUTCOMES]).unwrap();
    assert_eq!(prices.denominator, 3_600);
    assert_eq!(&prices.numerators[..3], &[2_520, 720, 360]);
    assert_eq!(
        signed_rounded_quadratic_potential(&skewed, &ivec(&[-6, -6, -6])),
        Ok(-6)
    );
}

#[test]
fn hostile_flows_schedules_and_generation_overflow_are_atomic() {
    let requests = [
        (trade(&[2, 0, 0], &[2, 0, 0]), DealerError::NonCanonicalFlow),
        (trade(&[1, 0, 0], &[]), DealerError::NonIntegralLot),
        (trade(&[14, 0, 0], &[]), DealerError::InventoryLimit),
        (trade(&[], &[14, 0, 0]), DealerError::InventoryLimit),
        (SignedDealerTradeV1::EMPTY, DealerError::NonCanonicalFlow),
    ];
    for (request, error) in requests {
        let mut state = funded_state();
        let before = state;
        assert_eq!(state.execute_trade(10, request), Err(error));
        assert_eq!(state, before);
    }

    let mut state = funded_state();
    let before = state;
    assert_eq!(
        state.execute_trade(9, trade(&[2, 0, 0], &[])),
        Err(DealerError::InvalidSchedule)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.contribute(1, id(10), 1),
        Err(DealerError::InvalidPhase)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.withdraw_funding(1, id(8), 1),
        Err(DealerError::InvalidPhase)
    );
    assert_eq!(state, before);

    state.generation = u64::MAX;
    let before = state;
    assert_eq!(
        state.execute_trade(10, trade(&[2, 0, 0], &[])),
        Err(DealerError::ArithmeticOverflow)
    );
    assert_eq!(state, before);
}

#[test]
fn cached_custody_share_and_terminal_mutants_are_detected() {
    let state = funded_state();
    let mut cash = state;
    cash.pool_cash_atoms += 1;
    assert_eq!(cash.validate(), Err(DealerError::InvariantViolation));

    let mut eggs = state;
    eggs.pool_eggs[0] += 2;
    assert_eq!(eggs.validate(), Err(DealerError::InvariantViolation));

    let mut signed = state;
    signed.net_sold[0] = 2;
    assert_eq!(signed.validate(), Err(DealerError::InvariantViolation));

    let mut duplicate = state;
    duplicate.positions[1].owner = duplicate.positions[0].owner;
    assert_eq!(duplicate.validate(), Err(DealerError::InvariantViolation));

    let mut empty = state;
    empty.positions[MAX_LPS - 1] = LpPositionV1 {
        owner: id(40),
        ..LpPositionV1::EMPTY
    };
    assert_eq!(empty.validate(), Err(DealerError::InvariantViolation));

    let mut resolved = state;
    resolved.resolve(30, uvec(&[2, 0, 0])).unwrap();
    let mut allocation = resolved;
    allocation.positions[0].terminal_claim_atoms += 1;
    assert_eq!(allocation.validate(), Err(DealerError::InvariantViolation));
}

#[test]
fn maximum_arithmetic_policy_remains_checked_without_allocation() {
    let mut bounded = policy();
    bounded.capital_unit_cash_atoms = MAX_ATOMS;
    bounded.capital_unit_eggs = uvec(&[MAX_ATOMS, MAX_ATOMS, MAX_ATOMS]);
    bounded.minimum_lp_shares = 1;
    bounded.maximum_lp_shares = 2;
    assert_eq!(bounded.validate(), Err(DealerError::ParameterOutOfRange));
}

#[test]
fn valid_aggregate_pool_values_may_exceed_each_single_source_bound() {
    let maximal_sources = SignedDealerPolicyV1 {
        policy_id: id(1),
        market: id(2),
        terms_digest: id(3),
        instance_id: id(4),
        claim_domain_digest: id(5),
        outcome_count: 2,
        payout_denominator: 1,
        initial_price_denominator: 2,
        initial_price_weights: uvec(&[1, 1]),
        depth_atoms: MAX_ATOMS,
        max_net_buy: uvec(&[1, 1]),
        max_net_sell: uvec(&[1, 1]),
        capital_unit_cash_atoms: MAX_ATOMS,
        capital_unit_eggs: uvec(&[MAX_ATOMS - 1, MAX_ATOMS - 1]),
        minimum_lp_shares: 1,
        maximum_lp_shares: 1,
        shutdown_queue_numerator: 1,
        shutdown_queue_denominator: 1,
        funding_deadline_slot: 5,
        trading_open_slot: 10,
        trading_close_slot: 20,
        maturity_slot: 30,
    };
    let subsidy = MAX_ATOMS / 4;
    assert_eq!(maximal_sources.minimum_sponsor_subsidy(), Ok(subsidy));
    let mut state =
        SignedDealerStateV1::initialize(maximal_sources, id(6), id(7), subsidy).unwrap();
    state.contribute(1, id(8), 1).unwrap();
    assert!(state.pool_cash_atoms > MAX_ATOMS);
    assert!(state.pool_cash_atoms <= MAX_LIVE_POOL_ATOMS);
    state.activate(5).unwrap();
    state.resolve(30, uvec(&[1, 0])).unwrap();
    assert!(state.terminal_pool_atoms > MAX_ATOMS);
    assert!(state.terminal_pool_atoms <= MAX_TERMINAL_POOL_ATOMS);
    state.validate().unwrap();
}

#[test]
fn authenticated_dealer_leg_is_reconciled_and_committed_as_one_aggregate() {
    let state = funded_state();
    let fixture = dealer_join_fixture(&state);
    let verified = verified_dealer_leg(&fixture);
    assert_eq!(verified.allocation_count(), 1);
    assert_eq!(verified.allocations()[0].user_cash_in_atoms, 3);
    assert_eq!(verified.total_external_fee_atoms(), 1);

    let receipt = state
        .reconcile_authenticated_dealer_leg_v2(10, &fixture.quote, &verified)
        .unwrap();
    assert_eq!(receipt.trader_cash_in_atoms, 3);
    assert_eq!(receipt.trader_cash_out_atoms, 0);

    let mut through_join = state;
    through_join
        .execute_authenticated_dealer_leg_v2(10, &fixture.quote, &verified)
        .unwrap();
    let mut direct = state;
    direct.execute_trade(10, trade(&[6, 0, 0], &[])).unwrap();
    assert_eq!(through_join, direct);
}

#[test]
fn relation_valid_but_false_curve_receipt_is_refused_without_mutation() {
    let state = funded_state();
    let mut fixture = dealer_join_fixture(&state);
    fixture.quote.receipt.dealer_net_cash_in_atoms = 2;
    refresh_quote(&mut fixture);
    let verified = verified_dealer_leg(&fixture);
    assert_eq!(verified.allocations()[0].user_cash_in_atoms, 2);

    let mut attempted = state;
    assert_eq!(
        attempted.execute_authenticated_dealer_leg_v2(10, &fixture.quote, &verified),
        Err(DealerError::DealerLegReceiptMismatch)
    );
    assert_eq!(attempted, state);
}

#[test]
fn stale_or_misdirected_authenticated_quote_is_refused_without_mutation() {
    let state = funded_state();
    let mut stale = dealer_join_fixture(&state);
    stale.quote.facility.pre_generation += 1;
    refresh_quote(&mut stale);
    let stale_verified = verified_dealer_leg(&stale);
    let mut attempted = state;
    assert_eq!(
        attempted.execute_authenticated_dealer_leg_v2(10, &stale.quote, &stale_verified),
        Err(DealerError::DealerLegBindingMismatch)
    );
    assert_eq!(attempted, state);

    let mut misdirected = dealer_join_fixture(&state);
    misdirected.quote.facility.facility_semantics_digest = id(99);
    refresh_quote(&mut misdirected);
    let misdirected_verified = verified_dealer_leg(&misdirected);
    assert_eq!(
        state.reconcile_authenticated_dealer_leg_v2(10, &misdirected.quote, &misdirected_verified,),
        Err(DealerError::DealerLegBindingMismatch)
    );
}

#[test]
fn detached_public_verdict_cannot_forge_or_mix_the_verified_capability() {
    let state = funded_state();
    let fixture = dealer_join_fixture(&state);
    let verified = verified_dealer_leg(&fixture);
    let mut forged_dto = *verified.verdict();
    forged_dto.trade.sell_to_users[0] -= 1;
    assert_ne!(&forged_dto, verified.verdict());

    // The facility accepts only `VerifiedDealerLegV2`; the public DTO above
    // cannot be passed to either reconciliation method or wrapped back into a
    // capability by safe downstream code. The compile-fail contract on the
    // capability type enforces that boundary.
    state
        .reconcile_authenticated_dealer_leg_v2(10, &fixture.quote, &verified)
        .unwrap();

    let mut other_quote = fixture.quote;
    other_quote.fee_policy_semantics_digest = id(77);
    other_quote.semantic_quote_digest = [0; 32];
    other_quote.semantic_quote_digest =
        dealer_quote_semantics_digest_v2(&fixture.domain, &fixture.dealer, &other_quote).unwrap();
    assert_eq!(
        state.reconcile_authenticated_dealer_leg_v2(10, &other_quote, &verified),
        Err(DealerError::DealerLegBindingMismatch)
    );

    // The same book, fills, and aggregate dealer trade under another valid
    // price projection have another upstream economic identity. Re-signing
    // only the quote projection cannot make the old verdict reusable.
    let mut other_price = fixture.price;
    other_price.prices = uvec(&[0, 2, 1]);
    other_price.semantic_price_digest =
        price_semantics_digest_v2(&fixture.domain, &other_price.prices).unwrap();
    let mut other_price_quote = fixture.quote;
    other_price_quote.upstream_economic_candidate_digest =
        dealer_upstream_economic_candidate_digest_v2(
            &fixture.domain,
            &fixture.book,
            &other_price,
            &fixture.candidate,
        )
        .unwrap();
    other_price_quote.semantic_quote_digest = [0; 32];
    other_price_quote.semantic_quote_digest =
        dealer_quote_semantics_digest_v2(&fixture.domain, &fixture.dealer, &other_price_quote)
            .unwrap();
    assert_ne!(
        other_price_quote.upstream_economic_candidate_digest,
        fixture.quote.upstream_economic_candidate_digest
    );
    assert_eq!(other_price_quote.trade, fixture.quote.trade);
    let other_price_verified = verify_economic_candidate_with_dealer_v2(
        &fixture.domain,
        &fixture.book,
        &other_price,
        &fixture.candidate,
        &fixture.dealer,
        &other_price_quote,
    )
    .unwrap();
    state
        .reconcile_authenticated_dealer_leg_v2(10, &other_price_quote, &other_price_verified)
        .unwrap();
    assert_eq!(
        state.reconcile_authenticated_dealer_leg_v2(10, &other_price_quote, &verified),
        Err(DealerError::DealerLegBindingMismatch)
    );
}

#[test]
fn tight_loss_boundary_and_one_atom_expense_are_not_handwaved() {
    let tight = SignedDealerPolicyV1 {
        policy_id: id(1),
        market: id(2),
        terms_digest: id(3),
        instance_id: id(4),
        claim_domain_digest: id(5),
        outcome_count: 2,
        payout_denominator: 2,
        initial_price_denominator: 2,
        initial_price_weights: uvec(&[1, 1]),
        depth_atoms: 100,
        max_net_buy: uvec(&[50, 50]),
        max_net_sell: uvec(&[50, 50]),
        capital_unit_cash_atoms: 25,
        capital_unit_eggs: uvec(&[50, 50]),
        minimum_lp_shares: 1,
        maximum_lp_shares: 1,
        shutdown_queue_numerator: 1,
        shutdown_queue_denominator: 1,
        funding_deadline_slot: 5,
        trading_open_slot: 10,
        trading_close_slot: 20,
        maturity_slot: 30,
    };
    assert_eq!(tight.minimum_sponsor_subsidy(), Ok(25));
    tight.validate().unwrap();
    let mut state = SignedDealerStateV1::initialize(tight, id(6), id(7), 25).unwrap();
    state.contribute(1, id(8), 1).unwrap();
    state.activate(5).unwrap();
    state
        .execute_trade(10, trade_for_endpoint(&ivec(&[50, -50])))
        .unwrap();
    let payout = uvec(&[2, 0]);
    assert_eq!(state.terminal_pool_yield(&payout), Ok(0));
    assert_eq!(state.terminal_lp_principal(&payout), Ok(75));

    let mut stolen_expense = state;
    stolen_expense.pool_cash_atoms -= 1;
    assert_eq!(
        stolen_expense.validate(),
        Err(DealerError::InvariantViolation)
    );
    assert_eq!(state.resolve(30, payout), Ok(75));
    assert_eq!(state.positions[0].terminal_claim_atoms, 75);
}

#[test]
fn terminal_owner_allocation_is_permutation_invariant() {
    let policy = policy();
    let mut forward = SignedDealerStateV1::initialize(policy, id(6), id(7), 40).unwrap();
    forward.contribute(1, id(8), 1).unwrap();
    forward.contribute(2, id(9), 2).unwrap();
    forward.activate(5).unwrap();
    forward
        .execute_trade(10, trade_for_endpoint(&ivec(&[-6, 4, 0])))
        .unwrap();
    forward.resolve(30, uvec(&[1, 1, 0])).unwrap();

    let mut reverse = SignedDealerStateV1::initialize(policy, id(6), id(7), 40).unwrap();
    reverse.contribute(1, id(9), 2).unwrap();
    reverse.contribute(2, id(8), 1).unwrap();
    reverse.activate(5).unwrap();
    reverse
        .execute_trade(10, trade_for_endpoint(&ivec(&[-6, 4, 0])))
        .unwrap();
    reverse.resolve(30, uvec(&[1, 1, 0])).unwrap();

    for owner in [id(8), id(9)] {
        let left = forward
            .positions
            .iter()
            .find(|position| position.owner == owner)
            .unwrap()
            .terminal_claim_atoms;
        let right = reverse
            .positions
            .iter()
            .find(|position| position.owner == owner)
            .unwrap()
            .terminal_claim_atoms;
        assert_eq!(left, right);
    }
}

#[test]
fn public_signed_potential_refuses_zero_settlement_denominator() {
    let mut malformed = policy();
    malformed.payout_denominator = 0;
    assert_eq!(
        signed_rounded_quadratic_potential(&malformed, &[0; MAX_OUTCOMES]),
        Err(DealerError::ZeroValue)
    );
}

#[test]
fn public_curve_helpers_refuse_malformed_box_padding_and_lots() {
    let mut bad_padding = policy();
    bad_padding.max_net_buy[15] = 2;
    assert_eq!(
        signed_rounded_quadratic_potential(&bad_padding, &[0; MAX_OUTCOMES]),
        Err(DealerError::InvalidBasis)
    );
    assert_eq!(
        bad_padding.minimum_sponsor_subsidy(),
        Err(DealerError::InvalidBasis)
    );

    let mut bad_lot = policy();
    bad_lot.max_net_buy[0] = 1;
    assert_eq!(
        signed_quadratic_price_vector(&bad_lot, &[0; MAX_OUTCOMES]),
        Err(DealerError::NonIntegralLot)
    );
    assert_eq!(
        bad_lot.minimum_sponsor_subsidy(),
        Err(DealerError::NonIntegralLot)
    );
}

#[test]
fn one_share_basket_must_settle_exactly_under_every_payout() {
    let mut fractional = policy();
    fractional.capital_unit_eggs[0] = 3;
    assert_eq!(fractional.validate(), Err(DealerError::NonIntegralLot));
}
