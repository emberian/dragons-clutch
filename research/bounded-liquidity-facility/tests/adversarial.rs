use clutch_bounded_liquidity_facility::{
    rounded_quadratic_potential, Error, FacilityPhase, FacilityPolicyV1, FacilityStateV1,
    FacilityTradeV1, MAX_ATOMS, MAX_OUTCOMES,
};

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn vector(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut result = [0; MAX_OUTCOMES];
    result[..values.len()].copy_from_slice(values);
    result
}

fn policy() -> FacilityPolicyV1 {
    FacilityPolicyV1 {
        policy_id: id(1),
        market: id(2),
        terms_digest: id(3),
        instance_id: id(4),
        claim_domain_digest: id(5),
        outcome_count: 3,
        payout_denominator: 2,
        initial_price_denominator: 3,
        initial_price_weights: vector(&[1, 1, 1]),
        depth_atoms: 120,
        max_inventory: vector(&[120, 120, 120]),
        trading_open_slot: 10,
        trading_close_slot: 20,
        maturity_slot: 30,
    }
}

fn small_policy() -> FacilityPolicyV1 {
    FacilityPolicyV1 {
        payout_denominator: 2,
        depth_atoms: 12,
        max_inventory: vector(&[12, 12, 12]),
        ..policy()
    }
}

fn state() -> FacilityStateV1 {
    let policy = policy();
    FacilityStateV1::initialize(
        policy,
        id(6),
        id(7),
        policy.minimum_sponsor_capital().unwrap(),
    )
    .unwrap()
}

fn trade(sell: &[u64], buy: &[u64]) -> FacilityTradeV1 {
    FacilityTradeV1 {
        sell_to_users: vector(sell),
        buy_from_users: vector(buy),
    }
}

#[test]
fn exact_capital_floor_is_required_before_first_quote() {
    let policy = policy();
    assert_eq!(policy.minimum_sponsor_capital(), Ok(40));
    assert_eq!(
        FacilityStateV1::initialize(policy, id(6), id(7), 39),
        Err(Error::InsufficientCapital)
    );

    let admitted = FacilityStateV1::initialize(policy, id(6), id(7), 40).unwrap();
    assert_eq!(admitted.cash_atoms, 40);
    assert_eq!(admitted.rounded_potential(), Ok(0));
    assert_eq!(admitted.liability(), Ok(0));
}

#[test]
fn exhaustive_small_domain_obeys_simplex_prices_and_global_loss_bound() {
    let policy = small_policy();
    let capital = policy.minimum_sponsor_capital().unwrap();
    assert_eq!(capital, 4);

    for q0 in (0..=12).step_by(2) {
        for q1 in (0..=12).step_by(2) {
            for q2 in (0..=12).step_by(2) {
                let q = vector(&[q0, q1, q2]);
                let potential = match rounded_quadratic_potential(&policy, &q) {
                    Ok(value) => value,
                    Err(Error::PriceDomain) => continue,
                    other => panic!("unexpected potential result: {other:?}"),
                };
                let liability = q0.max(q1).max(q2);
                assert!(potential <= liability);
                assert!(liability - potential <= capital);

                let mut candidate =
                    FacilityStateV1::initialize(policy, id(6), id(7), capital).unwrap();
                if q != [0; MAX_OUTCOMES] {
                    candidate
                        .execute_trade(10, trade(&[q0, q1, q2], &[]))
                        .unwrap();
                }
                let prices = candidate.price_vector().unwrap();
                prices.validate().unwrap();
                assert_eq!(
                    prices.numerators[0] + prices.numerators[1] + prices.numerators[2],
                    prices.denominator
                );

                for w0 in 0..=2 {
                    for w1 in 0..=(2 - w0) {
                        let w2 = 2 - w0 - w1;
                        let payout = vector(&[w0, w1, w2]);
                        assert!(candidate.terminal_equity(&payout).unwrap() <= MAX_ATOMS);
                    }
                }
            }
        }
    }
}

#[test]
fn trade_recipe_conserves_cash_hoard_and_every_egg() {
    let mut facility = state();
    let first = facility.execute_trade(10, trade(&[12, 0, 0], &[])).unwrap();
    assert_eq!(first.trader_cash_in_atoms, 5);
    assert_eq!(first.trader_cash_out_atoms, 0);
    assert_eq!(first.split_complete_sets, 12);
    assert_eq!(first.merge_complete_sets, 0);
    assert_eq!(first.new_cash_atoms, 33);
    assert_eq!(&first.new_retained_eggs[..3], &[0, 12, 12]);

    let cross = facility
        .execute_trade(11, trade(&[0, 8, 0], &[4, 0, 0]))
        .unwrap();
    assert_eq!(&cross.new_inventory[..3], &[8, 8, 0]);
    assert_eq!(cross.trader_cash_in_atoms, 1);
    assert_eq!(cross.trader_cash_out_atoms, 0);
    assert_eq!(cross.split_complete_sets, 0);
    assert_eq!(cross.merge_complete_sets, 4);
    assert_eq!(cross.new_cash_atoms, 38);
    assert_eq!(&cross.new_retained_eggs[..3], &[0, 0, 8]);
    facility.validate().unwrap();
}

#[test]
fn endpoint_potential_makes_splitting_and_round_trips_exact() {
    let mut direct = state();
    let direct_receipt = direct.execute_trade(10, trade(&[16, 8, 0], &[])).unwrap();

    let mut split = state();
    let first = split.execute_trade(10, trade(&[8, 0, 0], &[])).unwrap();
    let second = split.execute_trade(11, trade(&[8, 8, 0], &[])).unwrap();
    assert_eq!(direct.inventory, split.inventory);
    assert_eq!(direct.cash_atoms, split.cash_atoms);
    assert_eq!(direct.retained_eggs, split.retained_eggs);
    assert_eq!(direct.hoard_complete_sets, split.hoard_complete_sets);
    assert_eq!(
        direct_receipt.trader_cash_in_atoms,
        first.trader_cash_in_atoms + second.trader_cash_in_atoms
    );

    let original = state();
    let mut round_trip = original;
    round_trip
        .execute_trade(10, trade(&[16, 8, 0], &[]))
        .unwrap();
    round_trip
        .execute_trade(11, trade(&[], &[16, 8, 0]))
        .unwrap();
    assert_eq!(round_trip.cash_atoms, original.cash_atoms);
    assert_eq!(round_trip.inventory, original.inventory);
    assert_eq!(round_trip.retained_eggs, original.retained_eggs);
    assert_eq!(round_trip.hoard_complete_sets, 0);
}

#[test]
fn complete_set_translation_costs_exactly_one_per_set_and_needs_no_more_cash() {
    let policy = policy();
    let capital = policy.minimum_sponsor_capital().unwrap();
    let mut facility = FacilityStateV1::initialize(policy, id(6), id(7), capital).unwrap();
    let receipt = facility
        .execute_trade(10, trade(&[20, 20, 20], &[]))
        .unwrap();
    assert_eq!(receipt.trader_cash_in_atoms, 20);
    assert_eq!(receipt.split_complete_sets, 20);
    assert_eq!(facility.cash_atoms, capital);
    assert_eq!(facility.rounded_potential(), Ok(20));
    assert_eq!(&facility.retained_eggs[..3], &[0, 0, 0]);

    for q0 in (0..=20).step_by(2) {
        for q1 in (0..=20).step_by(2) {
            for q2 in (0..=20).step_by(2) {
                let q = vector(&[q0, q1, q2]);
                let shifted = vector(&[q0 + 2, q1 + 2, q2 + 2]);
                let Ok(base) = rounded_quadratic_potential(&policy, &q) else {
                    continue;
                };
                let Ok(translated) = rounded_quadratic_potential(&policy, &shifted) else {
                    continue;
                };
                assert_eq!(translated, base + 2);
            }
        }
    }
}

#[test]
fn exact_nonuniform_prior_generalizes_the_curve_and_loss_capital() {
    let skewed = FacilityPolicyV1 {
        initial_price_denominator: 10,
        initial_price_weights: vector(&[7, 2, 1]),
        ..policy()
    };
    assert_eq!(skewed.minimum_sponsor_capital(), Ok(81));
    let facility = FacilityStateV1::initialize(skewed, id(6), id(7), 81).unwrap();
    let prices = facility.price_vector().unwrap();
    assert_eq!(prices.denominator, 3_600);
    assert_eq!(&prices.numerators[..3], &[2_520, 720, 360]);

    let mut malformed = skewed;
    malformed.initial_price_weights[2] = 0;
    assert_eq!(malformed.validate(), Err(Error::InvalidPriceVector));
}

#[test]
fn exhaustive_skewed_prior_domain_remains_fully_capitalized() {
    let skewed = FacilityPolicyV1 {
        initial_price_denominator: 10,
        initial_price_weights: vector(&[7, 2, 1]),
        ..policy()
    };
    let capital = skewed.minimum_sponsor_capital().unwrap();
    for q0 in (0..=20).step_by(2) {
        for q1 in (0..=20).step_by(2) {
            for q2 in (0..=20).step_by(2) {
                let q = vector(&[q0, q1, q2]);
                let Ok(potential) = rounded_quadratic_potential(&skewed, &q) else {
                    continue;
                };
                let liability = q0.max(q1).max(q2);
                assert!(potential <= liability);
                assert!(liability - potential <= capital);
                let mut facility =
                    FacilityStateV1::initialize(skewed, id(6), id(7), capital).unwrap();
                if q != [0; MAX_OUTCOMES] {
                    facility
                        .execute_trade(10, trade(&[q0, q1, q2], &[]))
                        .unwrap();
                }
                facility.price_vector().unwrap().validate().unwrap();
                for w0 in 0..=2 {
                    for w1 in 0..=(2 - w0) {
                        facility
                            .terminal_equity(&vector(&[w0, w1, 2 - w0 - w1]))
                            .unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn native_wrapper_coefficients_concentrate_into_the_same_endpoint_inventory() {
    // An admitted Series/wrapper adapter decomposes only to native Egg atoms.
    // The facility therefore prices the aggregate native endpoint, not the
    // wrapper label or the solver's chosen decomposition order.
    let mut aggregate = state();
    aggregate
        .execute_trade(10, trade(&[24, 12, 4], &[]))
        .unwrap();

    let mut decomposed = state();
    decomposed
        .execute_trade(10, trade(&[12, 0, 0], &[]))
        .unwrap();
    decomposed
        .execute_trade(11, trade(&[0, 12, 0], &[]))
        .unwrap();
    decomposed
        .execute_trade(12, trade(&[12, 0, 4], &[]))
        .unwrap();

    assert_eq!(aggregate.inventory, decomposed.inventory);
    assert_eq!(aggregate.cash_atoms, decomposed.cash_atoms);
    assert_eq!(aggregate.retained_eggs, decomposed.retained_eggs);
}

#[test]
fn buyback_only_refuses_new_risk_and_flat_retirement_returns_exact_capital() {
    let mut facility = state();
    facility.execute_trade(10, trade(&[12, 8, 0], &[])).unwrap();
    facility.close_trading(20).unwrap();
    assert_eq!(facility.phase, FacilityPhase::BuybackOnly);
    let before = facility;
    assert_eq!(
        facility.execute_trade(21, trade(&[0, 0, 2], &[])),
        Err(Error::InvalidPhase)
    );
    assert_eq!(facility, before);

    facility.execute_trade(21, trade(&[], &[12, 8, 0])).unwrap();
    assert_eq!(facility.inventory, [0; MAX_OUTCOMES]);
    assert_eq!(facility.cash_atoms, facility.sponsor_capital_atoms);
    let withdrawn = facility.withdraw_and_retire(id(7)).unwrap();
    assert_eq!(withdrawn, 40);
    assert_eq!(facility.phase, FacilityPhase::Retired);
    facility.validate().unwrap();
}

#[test]
fn authenticated_resolution_conserves_user_payout_and_sponsor_equity() {
    let payouts = [vector(&[2, 0, 0]), vector(&[0, 2, 0]), vector(&[1, 1, 0])];
    for payout in payouts {
        let mut facility = state();
        facility.execute_trade(10, trade(&[12, 8, 0], &[])).unwrap();
        let expected_equity = facility.terminal_equity(&payout).unwrap();
        let terminal_cash = facility.resolve(30, payout).unwrap();
        assert_eq!(terminal_cash, expected_equity);
        assert_eq!(facility.phase, FacilityPhase::Resolved);
        assert_eq!(facility.hoard_complete_sets, 0);
        assert_eq!(facility.retained_eggs, [0; MAX_OUTCOMES]);
        facility.validate().unwrap();
        assert_eq!(facility.withdraw_and_retire(id(7)), Ok(expected_equity));
        facility.validate().unwrap();
    }
}

#[test]
fn malformed_or_early_resolution_is_atomic() {
    let mut facility = state();
    facility.execute_trade(10, trade(&[12, 8, 0], &[])).unwrap();
    let before = facility;
    assert_eq!(
        facility.resolve(29, vector(&[2, 0, 0])),
        Err(Error::InvalidSchedule)
    );
    assert_eq!(facility, before);
    assert_eq!(
        facility.resolve(30, vector(&[1, 0, 0])),
        Err(Error::InvalidPayoutVector)
    );
    assert_eq!(facility, before);
    let mut padded = vector(&[2, 0, 0]);
    padded[8] = 1;
    assert_eq!(facility.resolve(30, padded), Err(Error::InvalidBasis));
    assert_eq!(facility, before);
}

#[test]
fn hostile_trades_and_replay_counter_overflow_refuse_without_mutation() {
    let invalid = [
        (trade(&[2, 0, 0], &[2, 0, 0]), Error::NonCanonicalFlow),
        (trade(&[1, 0, 0], &[]), Error::NonIntegralLot),
        (trade(&[], &[2, 0, 0]), Error::InsufficientInventory),
        (trade(&[122, 0, 0], &[]), Error::InventoryLimit),
        (trade(&[120, 2, 0], &[]), Error::PriceDomain),
        (FacilityTradeV1::EMPTY, Error::ZeroValue),
    ];
    for (request, error) in invalid {
        let mut facility = state();
        let before = facility;
        assert_eq!(facility.execute_trade(10, request), Err(error));
        assert_eq!(facility, before);
    }

    let mut bad_padding = trade(&[2, 0, 0], &[]);
    bad_padding.sell_to_users[8] = 2;
    let mut facility = state();
    let before = facility;
    assert_eq!(
        facility.execute_trade(10, bad_padding),
        Err(Error::InvalidBasis)
    );
    assert_eq!(facility, before);

    let mut exhausted = state();
    exhausted.generation = u64::MAX;
    let before = exhausted;
    assert_eq!(
        exhausted.execute_trade(10, trade(&[2, 0, 0], &[])),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(exhausted, before);
}

#[test]
fn schedule_and_sponsor_authority_refusals_are_atomic() {
    let mut facility = state();
    let before = facility;
    assert_eq!(
        facility.execute_trade(9, trade(&[2, 0, 0], &[])),
        Err(Error::InvalidSchedule)
    );
    assert_eq!(facility, before);
    assert_eq!(
        facility.execute_trade(20, trade(&[2, 0, 0], &[])),
        Err(Error::InvalidSchedule)
    );
    assert_eq!(facility, before);
    assert_eq!(
        facility.halt_by_sponsor(id(8)),
        Err(Error::MismatchedSponsor)
    );
    assert_eq!(facility, before);
    assert_eq!(facility.close_trading(19), Err(Error::InvalidSchedule));
    assert_eq!(facility, before);
    assert_eq!(
        facility.withdraw_and_retire(id(7)),
        Err(Error::InvalidPhase)
    );
    assert_eq!(facility, before);
}

#[test]
fn arithmetic_ceiling_and_largest_frozen_domain_are_checked() {
    let mut maximum = FacilityPolicyV1 {
        policy_id: id(1),
        market: id(2),
        terms_digest: id(3),
        instance_id: id(4),
        claim_domain_digest: id(5),
        outcome_count: MAX_OUTCOMES as u8,
        payout_denominator: 1,
        initial_price_denominator: MAX_OUTCOMES as u64,
        initial_price_weights: [1; MAX_OUTCOMES],
        depth_atoms: MAX_ATOMS,
        max_inventory: [MAX_ATOMS; MAX_OUTCOMES],
        trading_open_slot: 1,
        trading_close_slot: 2,
        maturity_slot: 3,
    };
    assert_eq!(maximum.minimum_sponsor_capital(), Ok(468_750_000_000));
    let capital = maximum.minimum_sponsor_capital().unwrap();
    let mut facility = FacilityStateV1::initialize(maximum, id(6), id(7), capital).unwrap();
    let receipt = facility
        .execute_trade(
            1,
            FacilityTradeV1 {
                sell_to_users: [MAX_ATOMS; MAX_OUTCOMES],
                buy_from_users: [0; MAX_OUTCOMES],
            },
        )
        .unwrap();
    assert_eq!(receipt.trader_cash_in_atoms, MAX_ATOMS);
    assert_eq!(facility.cash_atoms, capital);
    facility.validate().unwrap();

    maximum.depth_atoms = MAX_ATOMS + 1;
    assert_eq!(maximum.validate(), Err(Error::ParameterOutOfRange));
}

#[test]
fn cached_backing_mutants_are_detected() {
    let mut facility = state();
    facility.execute_trade(10, trade(&[12, 8, 0], &[])).unwrap();

    let mut cash = facility;
    cash.cash_atoms += 1;
    assert_eq!(cash.validate(), Err(Error::InvariantViolation));

    let mut retained = facility;
    retained.retained_eggs[1] += 2;
    assert_eq!(retained.validate(), Err(Error::InvariantViolation));

    let mut hoard = facility;
    hoard.hoard_complete_sets -= 2;
    assert_eq!(hoard.validate(), Err(Error::InvariantViolation));

    let mut external = facility;
    external.terminal_external_payout_atoms = 1;
    assert_eq!(external.validate(), Err(Error::InvariantViolation));
}
