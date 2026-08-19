use clutch_liquidity_policy_model::{
    allocate_fee_pot, compile_schedule, full_simplex_liability, payout_numerator,
    CoefficientShapeV1, CompiledScheduleV1, Error, FeeAllocationInputV1, FractionalCarry, Id,
    LiquidityPolicyV1, NativeTermsV1, PortfolioQuotePlanV1, QuoteRungV1, QuoteScheduleV1,
    QuoteSide, QuoteStatus, TranchePhase, TrancheStateV1, MAX_FEE_RECIPIENTS, MAX_OUTCOMES,
    MAX_QUOTES,
};

fn id(byte: u8) -> Id {
    [byte; 32]
}

fn vector(active: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut output = [0; MAX_OUTCOMES];
    output[..active.len()].copy_from_slice(active);
    output
}

fn policy(degree: u8, schedule_digest: Id) -> LiquidityPolicyV1 {
    LiquidityPolicyV1 {
        policy_id: id(1),
        terms: NativeTermsV1 {
            market: id(2),
            terms_digest: id(3),
            basis_degree: degree,
            outcome_count: 4,
            payout_denominator: 100,
        },
        payoff_region_digest: id(4),
        quote_schedule_digest: schedule_digest,
        max_inventory: vector(&[1_000, 1_000, 1_000, 1_000]),
        collateral_cap: 10_000,
        batch_start: 10,
        batch_end: 20,
        fee_policy_id: id(6),
        withdrawal_policy_id: id(7),
        compiler_version: 1,
    }
}

fn rung(
    seed: u8,
    side: QuoteSide,
    shape: CoefficientShapeV1,
    lots: u64,
    limit: u64,
    minimum: u64,
    expiry: u64,
) -> QuoteRungV1 {
    QuoteRungV1 {
        quote_id: id(seed),
        side,
        shape,
        lots,
        limit_collateral_per_lot: limit,
        minimum_fill_lots: minimum,
        start_epoch: 10,
        expiry_epoch: expiry,
        generation: 1,
    }
}

fn schedule(digest: Id, rungs: &[QuoteRungV1]) -> QuoteScheduleV1 {
    let mut entries = [None; MAX_QUOTES];
    let mut i = 0;
    while i < rungs.len() {
        entries[i] = Some(rungs[i]);
        i += 1;
    }
    QuoteScheduleV1 {
        schedule_digest: digest,
        rung_count: u8::try_from(rungs.len()).unwrap(),
        rungs: entries,
    }
}

fn compile(policy: &LiquidityPolicyV1, rungs: &[QuoteRungV1]) -> CompiledScheduleV1 {
    compile_schedule(
        policy,
        id(8),
        &schedule(policy.quote_schedule_digest, rungs),
    )
    .unwrap()
}

fn plan(compiled: &CompiledScheduleV1, index: usize) -> PortfolioQuotePlanV1 {
    compiled.plans[index].unwrap()
}

fn funded(policy: LiquidityPolicyV1, reserve: u64) -> TrancheStateV1 {
    let mut state = TrancheStateV1::initialize(policy, id(8), id(9)).unwrap();
    assert_eq!(state.deposit(10, reserve), Ok(reserve));
    state
}

#[test]
fn every_native_degree_is_bound_and_range_triangle_exact_compile_golden() {
    for degree in 0..=3 {
        let p = policy(degree, id(5));
        let exact = vector(&[7, 0, 9, 1]);
        let compiled = compile(
            &p,
            &[
                rung(
                    10,
                    QuoteSide::SellWrite,
                    CoefficientShapeV1::HardRange {
                        first: 1,
                        end: 3,
                        amount: 11,
                    },
                    2,
                    3,
                    1,
                    20,
                ),
                rung(
                    11,
                    QuoteSide::SellWrite,
                    CoefficientShapeV1::Triangle {
                        left: 0,
                        peak: 1,
                        right: 3,
                        height: 10,
                    },
                    1,
                    2,
                    1,
                    20,
                ),
                rung(
                    12,
                    QuoteSide::SellWrite,
                    CoefficientShapeV1::Exact {
                        active_len: 4,
                        coefficients: exact,
                    },
                    1,
                    1,
                    1,
                    20,
                ),
            ],
        );
        assert_eq!(plan(&compiled, 0).coefficients, vector(&[0, 11, 11, 0]));
        assert_eq!(plan(&compiled, 1).coefficients, vector(&[0, 10, 5, 0]));
        assert_eq!(plan(&compiled, 2).coefficients, exact);
        for item in compiled.plans.iter().take(3) {
            let item = item.unwrap();
            assert_eq!(item.basis_degree, degree);
            assert_eq!(item.market, p.terms.market);
            assert_eq!(item.terms_digest, p.terms.terms_digest);
            assert_eq!(item.payoff_region_digest, p.payoff_region_digest);
            assert_eq!(item.quote_schedule_digest, p.quote_schedule_digest);
        }
    }
}

#[test]
fn compiler_refuses_empty_range_padding_duplicate_identity_and_overflow() {
    let p = policy(3, id(5));
    let empty = schedule(
        id(5),
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 2,
                end: 2,
                amount: 1,
            },
            1,
            1,
            1,
            20,
        )],
    );
    assert_eq!(
        compile_schedule(&p, id(8), &empty),
        Err(Error::InvalidRange)
    );

    let one = rung(
        10,
        QuoteSide::SellWrite,
        CoefficientShapeV1::HardRange {
            first: 0,
            end: 1,
            amount: 1,
        },
        1,
        1,
        1,
        20,
    );
    let duplicate = schedule(id(5), &[one, one]);
    assert_eq!(
        compile_schedule(&p, id(8), &duplicate),
        Err(Error::QuoteCapacity)
    );

    let mut noncanonical = schedule(id(5), &[one]);
    noncanonical.rungs[7] = Some(one);
    assert_eq!(
        compile_schedule(&p, id(8), &noncanonical),
        Err(Error::NonCanonicalPadding)
    );

    let wrong_digest = schedule(id(99), &[one]);
    assert_eq!(
        compile_schedule(&p, id(8), &wrong_digest),
        Err(Error::MismatchedBinding)
    );

    let overflow = schedule(
        id(5),
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::Exact {
                active_len: 4,
                coefficients: vector(&[u64::MAX, 0, 0, 0]),
            },
            2,
            1,
            1,
            20,
        )],
    );
    assert_eq!(
        compile_schedule(&p, id(8), &overflow),
        Err(Error::ArithmeticOverflow)
    );

    let mut tight_inventory = p;
    tight_inventory.max_inventory = vector(&[5, 5, 5, 5]);
    let exceeds_inventory = schedule(
        id(5),
        &[rung(
            13,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 0,
                end: 1,
                amount: 6,
            },
            1,
            1,
            1,
            20,
        )],
    );
    assert_eq!(
        compile_schedule(&tight_inventory, id(8), &exceeds_inventory),
        Err(Error::InventoryLimit)
    );

    let mut tight_collateral = p;
    tight_collateral.collateral_cap = 5;
    let exceeds_collateral = schedule(
        id(5),
        &[rung(
            14,
            QuoteSide::BuyOffset,
            CoefficientShapeV1::HardRange {
                first: 0,
                end: 1,
                amount: 1,
            },
            2,
            3,
            1,
            20,
        )],
    );
    assert_eq!(
        compile_schedule(&tight_collateral, id(8), &exceeds_collateral),
        Err(Error::CollateralCap)
    );
}

#[test]
fn full_simplex_maximum_liability_is_exhaustive_and_attained() {
    let denominator = 6u64;
    for first in 0..=4u64 {
        for second in 0..=4u64 {
            for third in 0..=4u64 {
                let inventory = vector(&[first, second, third]);
                let liability = full_simplex_liability(3, &inventory).unwrap();
                for w0 in 0..=denominator {
                    for w1 in 0..=denominator - w0 {
                        let w2 = denominator - w0 - w1;
                        let weights = vector(&[w0, w1, w2]);
                        let numerator = payout_numerator(3, &inventory, &weights).unwrap();
                        assert!(numerator <= u128::from(liability * denominator));
                    }
                }
                let maximum_index = if first == liability {
                    0
                } else if second == liability {
                    1
                } else {
                    2
                };
                let mut vertex = [0; MAX_OUTCOMES];
                vertex[maximum_index] = denominator;
                assert_eq!(
                    payout_numerator(3, &inventory, &vertex).unwrap(),
                    u128::from(liability * denominator)
                );
            }
        }
    }
}

#[test]
fn sell_partial_fill_cancel_and_settlement_conserve_exact_ledgers() {
    let p = policy(3, id(5));
    let compiled = compile(
        &p,
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 0,
                end: 2,
                amount: 10,
            },
            5,
            3,
            2,
            20,
        )],
    );
    let mut state = funded(p, 1_000);
    state.admit_schedule(10, &compiled).unwrap();
    assert_eq!(state.reserve_atoms, 1_000);
    assert_eq!(state.reserved_sell_inventory, vector(&[50, 50, 0, 0]));
    assert_eq!(state.encumbered_collateral(), Ok(50));
    assert_eq!(state.free_collateral(), Ok(950));

    let before_bad_fill = state;
    assert_eq!(
        state.fill_quote(12, id(10), id(30), 1, 4),
        Err(Error::InvalidQuoteState)
    );
    assert_eq!(state, before_bad_fill);

    let receipt = state.fill_quote(12, id(10), id(30), 2, 4).unwrap();
    assert_eq!(receipt.collateral_credit_atoms, 8);
    assert_eq!(receipt.collateral_debit_atoms, 0);
    assert_eq!(receipt.eggs, vector(&[20, 20, 0, 0]));
    assert_eq!(state.reserve_atoms, 1_008);
    assert_eq!(state.inventory, vector(&[20, 20, 0, 0]));
    assert_eq!(state.reserved_sell_inventory, vector(&[30, 30, 0, 0]));
    assert_eq!(state.encumbered_collateral(), Ok(50));
    assert_eq!(state.capital_time_weight, 100);

    state.cancel_quote(13, id(10)).unwrap();
    assert_eq!(state.reserved_sell_inventory, [0; MAX_OUTCOMES]);
    assert_eq!(state.inventory_liability(), Ok(20));
    assert_eq!(state.capital_time_weight, 150);
    let quote = state.quotes[0].unwrap();
    assert_eq!(quote.status, QuoteStatus::Cancelled);
    assert_eq!(quote.remaining_lots, 0);

    let payout = state.settle(21, 100, vector(&[100, 0, 0, 0])).unwrap();
    assert_eq!(payout, 20);
    assert_eq!(state.reserve_atoms, 988);
    assert_eq!(state.inventory, [0; MAX_OUTCOMES]);
    assert_eq!(state.settled_payout_atoms, 20);
    assert_eq!(state.phase, TranchePhase::Resolved);
    state.validate().unwrap();
}

#[test]
fn lapse_is_strictly_after_expiry_and_refusal_is_atomic() {
    let p = policy(1, id(5));
    let compiled = compile(
        &p,
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 1,
                end: 3,
                amount: 4,
            },
            3,
            2,
            1,
            15,
        )],
    );
    let mut state = funded(p, 100);
    state.admit_schedule(10, &compiled).unwrap();
    let before = state;
    assert_eq!(state.lapse_quote(15, id(10)), Err(Error::InvalidEpoch));
    assert_eq!(state, before);
    state.lapse_quote(1_000, id(10)).unwrap();
    assert_eq!(state.quotes[0].unwrap().status, QuoteStatus::Lapsed);
    assert_eq!(state.reserved_sell_inventory, [0; MAX_OUTCOMES]);
    assert_eq!(state.free_collateral(), Ok(100));
    // Exposure 12 is fee-eligible only for epochs [10,16), never to 1,000.
    assert_eq!(state.capital_time_weight, 72);
}

#[test]
fn buy_back_is_inventory_bounded_and_releases_price_improvement() {
    let p = policy(2, id(5));
    let sell = rung(
        10,
        QuoteSide::SellWrite,
        CoefficientShapeV1::Exact {
            active_len: 4,
            coefficients: vector(&[2, 1, 0, 0]),
        },
        10,
        2,
        1,
        20,
    );
    let buy = rung(
        11,
        QuoteSide::BuyOffset,
        CoefficientShapeV1::Exact {
            active_len: 4,
            coefficients: vector(&[2, 1, 0, 0]),
        },
        4,
        3,
        1,
        20,
    );
    let compiled = compile(&p, &[sell, buy]);
    let mut state = funded(p, 500);

    let before_unfunded_buy = state;
    assert_eq!(
        state.admit_plan(10, plan(&compiled, 1)),
        Err(Error::InsufficientInventory)
    );
    assert_eq!(state, before_unfunded_buy);

    state.admit_plan(10, plan(&compiled, 0)).unwrap();
    state.fill_quote(10, id(10), id(30), 10, 2).unwrap();
    assert_eq!(state.inventory, vector(&[20, 10, 0, 0]));
    assert_eq!(state.reserve_atoms, 520);

    state.admit_plan(11, plan(&compiled, 1)).unwrap();
    assert_eq!(state.reserved_buy_cash_atoms, 12);
    assert_eq!(state.reserved_buy_inventory, vector(&[8, 4, 0, 0]));
    let receipt = state.fill_quote(12, id(11), id(31), 3, 2).unwrap();
    assert_eq!(receipt.collateral_debit_atoms, 6);
    assert_eq!(receipt.eggs, vector(&[6, 3, 0, 0]));
    assert_eq!(state.reserve_atoms, 514);
    assert_eq!(state.inventory, vector(&[14, 7, 0, 0]));
    assert_eq!(state.reserved_buy_cash_atoms, 3);
    assert_eq!(state.reserved_buy_inventory, vector(&[2, 1, 0, 0]));
    state.cancel_quote(13, id(11)).unwrap();
    assert_eq!(state.reserved_buy_cash_atoms, 0);
    assert_eq!(state.reserved_buy_inventory, [0; MAX_OUTCOMES]);
    state.validate().unwrap();
}

#[test]
fn simultaneous_sells_aggregate_before_one_max_and_buys_never_net() {
    let p = policy(2, id(5));
    let compiled = compile(
        &p,
        &[
            rung(
                10,
                QuoteSide::SellWrite,
                CoefficientShapeV1::Exact {
                    active_len: 4,
                    coefficients: vector(&[10, 0, 0, 0]),
                },
                2,
                1,
                1,
                20,
            ),
            rung(
                11,
                QuoteSide::SellWrite,
                CoefficientShapeV1::Exact {
                    active_len: 4,
                    coefficients: vector(&[0, 10, 0, 0]),
                },
                2,
                1,
                1,
                20,
            ),
            rung(
                12,
                QuoteSide::BuyOffset,
                CoefficientShapeV1::Exact {
                    active_len: 4,
                    coefficients: vector(&[1, 1, 0, 0]),
                },
                5,
                7,
                1,
                20,
            ),
            rung(
                13,
                QuoteSide::SellWrite,
                CoefficientShapeV1::Exact {
                    active_len: 4,
                    coefficients: vector(&[1, 1, 0, 0]),
                },
                10,
                1,
                1,
                20,
            ),
        ],
    );
    let mut state = funded(p, 100);
    state.admit_plan(10, plan(&compiled, 3)).unwrap();
    state.fill_quote(10, id(13), id(30), 10, 1).unwrap();
    assert_eq!(state.inventory, vector(&[10, 10, 0, 0]));
    state.admit_plan(10, plan(&compiled, 0)).unwrap();
    state.admit_plan(10, plan(&compiled, 1)).unwrap();
    state.admit_plan(10, plan(&compiled, 2)).unwrap();

    assert_eq!(state.reserved_sell_inventory, vector(&[20, 20, 0, 0]));
    assert_eq!(state.reserved_buy_cash_atoms, 35);
    // B + max_i(q_i + sum_r sells[r][i]) = 35 + max(30,30) = 65.
    assert_eq!(state.encumbered_collateral(), Ok(65));
    // Buy Eggs are bounded by q but never subtracted before an actual fill.
    assert_eq!(state.reserved_buy_inventory, vector(&[5, 5, 0, 0]));
    assert_eq!(state.free_collateral(), Ok(45)); // R=110 after seed fill.
}

#[test]
fn fill_splitting_and_range_refinement_preserve_state_and_risk() {
    let p = policy(3, id(5));
    let one_rung = rung(
        10,
        QuoteSide::SellWrite,
        CoefficientShapeV1::HardRange {
            first: 0,
            end: 2,
            amount: 10,
        },
        10,
        2,
        1,
        20,
    );
    let one = compile(&p, &[one_rung]);
    let mut whole_fill = funded(p, 500);
    let mut split_fill = whole_fill;
    whole_fill.admit_schedule(10, &one).unwrap();
    split_fill.admit_schedule(10, &one).unwrap();
    whole_fill.fill_quote(12, id(10), id(30), 10, 3).unwrap();
    split_fill.fill_quote(12, id(10), id(30), 4, 3).unwrap();
    split_fill.fill_quote(12, id(10), id(30), 6, 3).unwrap();
    assert_eq!(whole_fill.reserve_atoms, split_fill.reserve_atoms);
    assert_eq!(whole_fill.inventory, split_fill.inventory);
    assert_eq!(
        whole_fill.reserved_sell_inventory,
        split_fill.reserved_sell_inventory
    );
    assert_eq!(
        whole_fill.capital_time_weight,
        split_fill.capital_time_weight
    );

    let refined = compile(
        &p,
        &[
            rung(
                11,
                QuoteSide::SellWrite,
                CoefficientShapeV1::Exact {
                    active_len: 4,
                    coefficients: vector(&[10, 0, 0, 0]),
                },
                10,
                1,
                1,
                20,
            ),
            rung(
                12,
                QuoteSide::SellWrite,
                CoefficientShapeV1::Exact {
                    active_len: 4,
                    coefficients: vector(&[0, 10, 0, 0]),
                },
                10,
                1,
                1,
                20,
            ),
        ],
    );
    let mut coarse = funded(p, 500);
    let mut fine = funded(p, 500);
    coarse.admit_schedule(10, &one).unwrap();
    fine.admit_schedule(10, &refined).unwrap();
    assert_eq!(coarse.reserved_sell_inventory, fine.reserved_sell_inventory);
    assert_eq!(coarse.encumbered_collateral(), fine.encumbered_collateral());
    assert_eq!(coarse.free_collateral(), fine.free_collateral());
    coarse.accrue_risk(15).unwrap();
    fine.accrue_risk(15).unwrap();
    assert_eq!(coarse.capital_time_weight, fine.capital_time_weight);
    coarse.fill_quote(15, id(10), id(30), 10, 2).unwrap();
    fine.fill_quote(15, id(11), id(30), 10, 1).unwrap();
    fine.fill_quote(15, id(12), id(31), 10, 1).unwrap();
    assert_eq!(coarse.reserve_atoms, fine.reserve_atoms);
    assert_eq!(coarse.inventory, fine.inventory);
    assert_eq!(coarse.reserved_sell_inventory, fine.reserved_sell_inventory);
}

#[test]
fn withdrawal_obeys_bare_liability_pending_quotes_and_pro_rata_equity() {
    let p = policy(0, id(5));
    let compiled = compile(
        &p,
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 0,
                end: 1,
                amount: 20,
            },
            1,
            4,
            1,
            20,
        )],
    );
    let mut state = funded(p, 100);
    state.admit_schedule(10, &compiled).unwrap();
    assert_eq!(state.free_collateral(), Ok(80));
    let before = state;
    assert_eq!(state.withdraw(10, 90, 90), Err(Error::WithdrawalLimit));
    assert_eq!(state, before);
    state.withdraw(10, 50, 50).unwrap();
    assert_eq!(state.reserve_atoms, 50);
    assert_eq!(state.lp_share_supply, 50);
    assert_eq!(state.free_collateral(), Ok(30));

    state.fill_quote(11, id(10), id(30), 1, 4).unwrap();
    assert_eq!(state.inventory_liability(), Ok(20));
    assert_eq!(state.reserve_atoms, 54);
    assert_eq!(state.conservative_equity_numerator(), Ok(34));
    let before_last = state;
    assert_eq!(
        state.withdraw(12, 50, 34),
        Err(Error::FeeCheckpointRequired)
    );
    assert_eq!(state, before_last);
    state.accrue_risk(12).unwrap();
    let inputs = [
        Some(state.fee_input()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let zero_pot = allocate_fee_pot(id(39), 0, 1, &inputs).unwrap();
    assert_eq!(zero_pot.credited_atoms(), 0);
    assert_eq!(zero_pot.retained_carry_escrow_atoms(), 0);
    state
        .apply_fee_allocation(1, zero_pot.output(0).unwrap())
        .unwrap();
    let checkpointed = state;
    assert_eq!(state.withdraw(12, 50, 34), Err(Error::LastShareLocked));
    assert_eq!(state, checkpointed);
}

#[test]
fn exact_pro_rata_deposit_refuses_rounding_transfer() {
    let p = policy(0, id(5));
    let compiled = compile(
        &p,
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 0,
                end: 1,
                amount: 20,
            },
            1,
            4,
            1,
            20,
        )],
    );
    let mut state = funded(p, 100);
    state.admit_schedule(10, &compiled).unwrap();
    state.fill_quote(10, id(10), id(30), 1, 4).unwrap();
    // R=104, H(q)=20, conservative equity=84, S=100.
    let before = state;
    assert_eq!(state.deposit(10, 1), Err(Error::RemainderRequired));
    assert_eq!(state, before);
    assert_eq!(state.deposit(10, 21), Ok(25));
    assert_eq!(state.reserve_atoms, 125);
    assert_eq!(state.lp_share_supply, 125);
    assert_eq!(state.conservative_equity_numerator(), Ok(105));
    state.withdraw(10, 25, 21).unwrap();
    assert_eq!(state.reserve_atoms, 104);
    assert_eq!(state.lp_share_supply, 100);
    state.accrue_risk(11).unwrap();
    let before_historical_fee_theft = state;
    assert_eq!(state.deposit(11, 21), Err(Error::FeeCheckpointRequired));
    assert_eq!(state, before_historical_fee_theft);
}

#[test]
fn self_cross_limit_overflow_fractional_settlement_and_cache_mutants_are_atomic() {
    let p = policy(2, id(5));
    let compiled = compile(
        &p,
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::Exact {
                active_len: 4,
                coefficients: vector(&[1, 0, 0, 0]),
            },
            2,
            3,
            1,
            20,
        )],
    );
    let mut state = funded(p, 100);
    state.admit_schedule(10, &compiled).unwrap();

    for taker in [state.owner, state.tranche_id] {
        let before = state;
        assert_eq!(
            state.fill_quote(11, id(10), taker, 1, 3),
            Err(Error::SelfCross)
        );
        assert_eq!(state, before);
    }
    let before_limit = state;
    assert_eq!(
        state.fill_quote(11, id(10), id(30), 1, 2),
        Err(Error::LimitViolated)
    );
    assert_eq!(state, before_limit);
    let before_overflow = state;
    assert_eq!(
        state.fill_quote(11, id(10), id(30), 2, u64::MAX),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(state, before_overflow);

    state.fill_quote(11, id(10), id(30), 1, 3).unwrap();
    state.cancel_quote(12, id(10)).unwrap();
    let before_fraction = state;
    assert_eq!(
        state.settle(21, 100, vector(&[50, 50, 0, 0])),
        Err(Error::RemainderRequired)
    );
    assert_eq!(state, before_fraction);

    let mut bad_cache = state;
    bad_cache.reserved_buy_cash_atoms = 1;
    assert_eq!(bad_cache.validate(), Err(Error::InvariantViolation));
    let mut bad_policy = state;
    bad_policy.policy.terms.basis_degree = 4;
    assert_eq!(bad_policy.validate(), Err(Error::InvalidBasis));
    let mut fake_prior_settlement = state;
    fake_prior_settlement.settled_payout_atoms = 1;
    assert_eq!(fake_prior_settlement.validate(), Err(Error::InvalidPhase));
}

#[test]
fn exact_fee_carry_conserves_pot_and_split_allocations_telescope() {
    let inputs = [
        Some(FeeAllocationInputV1 {
            tranche_id: id(10),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 100,
            tranche_generation: 1,
            capital_time_weight: 1,
            carry: FractionalCarry::ZERO,
        }),
        Some(FeeAllocationInputV1 {
            tranche_id: id(11),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 100,
            tranche_generation: 1,
            capital_time_weight: 2,
            carry: FractionalCarry::ZERO,
        }),
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let allocation = allocate_fee_pot(id(40), 10, 2, &inputs).unwrap();
    let first = allocation.output(0).unwrap();
    let second = allocation.output(1).unwrap();
    assert_eq!(first.credited_atoms(), 3);
    assert_eq!(
        first.new_carry(),
        FractionalCarry {
            numerator: 1,
            denominator: 3
        }
    );
    assert_eq!(second.credited_atoms(), 6);
    assert_eq!(
        second.new_carry(),
        FractionalCarry {
            numerator: 2,
            denominator: 3
        }
    );
    // 3 + 6 + 1/3 + 2/3 = 10 exactly.
    assert_eq!(first.credited_atoms() + second.credited_atoms() + 1, 10);
    assert_eq!(allocation.prior_carry_escrow_atoms(), 0);
    assert_eq!(allocation.retained_carry_escrow_atoms(), 1);
    assert_eq!(allocation.credited_atoms(), 9);

    let one_input = [
        Some(FeeAllocationInputV1 {
            tranche_id: id(12),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 100,
            tranche_generation: 1,
            capital_time_weight: 1,
            carry: FractionalCarry::ZERO,
        }),
        Some(FeeAllocationInputV1 {
            tranche_id: id(13),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 100,
            tranche_generation: 1,
            capital_time_weight: 1,
            carry: FractionalCarry::ZERO,
        }),
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let whole = allocate_fee_pot(id(41), 10, 2, &one_input).unwrap();
    assert_eq!(whole.output(0).unwrap().credited_atoms(), 5);
    assert_eq!(whole.credited_atoms(), 10);
    assert_eq!(whole.retained_carry_escrow_atoms(), 0);

    let first_half = allocate_fee_pot(id(42), 5, 2, &one_input).unwrap();
    assert_eq!(first_half.credited_atoms(), 4);
    assert_eq!(first_half.prior_carry_escrow_atoms(), 0);
    assert_eq!(first_half.retained_carry_escrow_atoms(), 1);
    let unsplit_input = [
        Some(FeeAllocationInputV1 {
            tranche_id: id(14),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 200,
            tranche_generation: 1,
            capital_time_weight: 2,
            carry: FractionalCarry::ZERO,
        }),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let unsplit = allocate_fee_pot(id(49), 5, 1, &unsplit_input).unwrap();
    assert_eq!(unsplit.credited_atoms(), 5);
    // Split and unsplit aggregate value agree once physical escrow is counted.
    assert_eq!(
        first_half.credited_atoms() + first_half.retained_carry_escrow_atoms(),
        unsplit.credited_atoms()
    );
    let next_inputs = [
        Some(FeeAllocationInputV1 {
            tranche_id: id(12),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 100,
            tranche_generation: 1,
            capital_time_weight: 1,
            carry: first_half.output(0).unwrap().new_carry(),
        }),
        Some(FeeAllocationInputV1 {
            tranche_id: id(13),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 100,
            tranche_generation: 1,
            capital_time_weight: 1,
            carry: first_half.output(1).unwrap().new_carry(),
        }),
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let second_half = allocate_fee_pot(id(43), 5, 2, &next_inputs).unwrap();
    assert_eq!(second_half.prior_carry_escrow_atoms(), 1);
    assert_eq!(second_half.credited_atoms(), 6);
    assert_eq!(second_half.retained_carry_escrow_atoms(), 0);
    for index in 0..2 {
        assert_eq!(
            first_half.output(index).unwrap().credited_atoms()
                + second_half.output(index).unwrap().credited_atoms(),
            whole.output(index).unwrap().credited_atoms()
        );
        assert_eq!(
            second_half.output(index).unwrap().new_carry(),
            whole.output(index).unwrap().new_carry()
        );
    }
}

#[test]
fn risk_weight_is_homogeneous_and_fee_application_is_replay_safe() {
    let p = policy(3, id(5));
    let compiled = compile(
        &p,
        &[rung(
            10,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 0,
                end: 2,
                amount: 5,
            },
            4,
            1,
            1,
            20,
        )],
    );
    let mut state = funded(p, 100);
    state.admit_schedule(10, &compiled).unwrap();
    state.accrue_risk(15).unwrap();
    assert_eq!(state.capital_time_weight, 100); // max 20 * frozen V1 multiplier 1 * 5.

    let inputs = [
        Some(state.fee_input()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let allocation = allocate_fee_pot(id(44), 7, 1, &inputs).unwrap();
    let output = allocation.output(0).unwrap();
    let mut stale_generation = state;
    stale_generation.cancel_quote(15, id(10)).unwrap();
    let before_stale = stale_generation;
    assert_eq!(
        stale_generation.apply_fee_allocation(1, output),
        Err(Error::FeeAllocationMismatch)
    );
    assert_eq!(stale_generation, before_stale);
    state.apply_fee_allocation(1, output).unwrap();
    assert_eq!(state.reserve_atoms, 107);
    assert_eq!(state.capital_time_weight, 0);
    let before_replay = state;
    assert_eq!(
        state.apply_fee_allocation(1, output),
        Err(Error::FeeAllocationMismatch)
    );
    assert_eq!(state, before_replay);

    let doubled = compile(
        &p,
        &[rung(
            11,
            QuoteSide::SellWrite,
            CoefficientShapeV1::HardRange {
                first: 0,
                end: 2,
                amount: 10,
            },
            4,
            1,
            1,
            20,
        )],
    );
    let mut doubled_state = funded(p, 200);
    doubled_state.admit_schedule(10, &doubled).unwrap();
    doubled_state.accrue_risk(15).unwrap();
    assert_eq!(doubled_state.capital_time_weight, 200);
}

#[test]
fn fee_allocation_refuses_zero_weight_duplicates_and_noncanonical_padding() {
    let zero = [
        Some(FeeAllocationInputV1 {
            tranche_id: id(10),
            fee_policy_id: id(6),
            snapshot_epoch: 15,
            lp_share_supply: 100,
            tranche_generation: 1,
            capital_time_weight: 0,
            carry: FractionalCarry::ZERO,
        }),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    assert_eq!(
        allocate_fee_pot(id(45), 1, 1, &zero),
        Err(Error::ZeroWeight)
    );

    let repeated = FeeAllocationInputV1 {
        tranche_id: id(10),
        fee_policy_id: id(6),
        snapshot_epoch: 15,
        lp_share_supply: 100,
        tranche_generation: 1,
        capital_time_weight: 1,
        carry: FractionalCarry::ZERO,
    };
    let duplicate = [
        Some(repeated),
        Some(repeated),
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    assert_eq!(
        allocate_fee_pot(id(46), 1, 2, &duplicate),
        Err(Error::MismatchedBinding)
    );

    let mut padding: [Option<FeeAllocationInputV1>; MAX_FEE_RECIPIENTS] = [None; 8];
    padding[0] = Some(repeated);
    padding[7] = Some(FeeAllocationInputV1 {
        tranche_id: id(11),
        ..repeated
    });
    assert_eq!(
        allocate_fee_pot(id(47), 1, 1, &padding),
        Err(Error::NonCanonicalPadding)
    );

    let unbacked_fraction = [
        Some(FeeAllocationInputV1 {
            carry: FractionalCarry {
                numerator: 1,
                denominator: 2,
            },
            ..repeated
        }),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    assert_eq!(
        allocate_fee_pot(id(48), 1, 1, &unbacked_fraction),
        Err(Error::InvariantViolation)
    );
}

#[test]
fn exhaustive_small_conservation_withdrawal_order_and_fee_escrow_campaign() {
    let p = policy(3, id(5));
    for amount in 1..=3u64 {
        for lots in 1..=3u64 {
            for fill in 1..=lots {
                let compiled = compile(
                    &p,
                    &[rung(
                        10,
                        QuoteSide::SellWrite,
                        CoefficientShapeV1::Exact {
                            active_len: 4,
                            coefficients: vector(&[amount, amount + 1, 0, 0]),
                        },
                        lots,
                        1,
                        1,
                        20,
                    )],
                );
                let mut state = funded(p, 100);
                state.admit_schedule(10, &compiled).unwrap();
                let before_total = [
                    state.inventory[0] + state.reserved_sell_inventory[0],
                    state.inventory[1] + state.reserved_sell_inventory[1],
                ];
                let before_reserve = state.reserve_atoms;
                state.fill_quote(10, id(10), id(30), fill, 2).unwrap();
                assert_eq!(
                    [
                        state.inventory[0] + state.reserved_sell_inventory[0],
                        state.inventory[1] + state.reserved_sell_inventory[1],
                    ],
                    before_total
                );
                assert_eq!(state.reserve_atoms, before_reserve + 2 * fill);
                assert!(state.reserve_atoms >= state.encumbered_collateral().unwrap());
                state.validate().unwrap();
            }
        }
    }

    let withdrawal_orders = [
        [10u64, 30, 60],
        [10, 60, 30],
        [30, 10, 60],
        [30, 60, 10],
        [60, 10, 30],
        [60, 30, 10],
    ];
    for order in withdrawal_orders {
        let mut state = funded(p, 100);
        for shares in order {
            state.withdraw(10, shares, shares).unwrap();
        }
        assert_eq!(state.lp_share_supply, 0);
        assert_eq!(state.reserve_atoms, 0);
        state.validate().unwrap();
    }

    for pot in 0..=8u64 {
        for left_weight in 1..=5u128 {
            for right_weight in 1..=5u128 {
                let inputs = [
                    Some(FeeAllocationInputV1 {
                        tranche_id: id(10),
                        fee_policy_id: id(6),
                        snapshot_epoch: 15,
                        lp_share_supply: 100,
                        tranche_generation: 1,
                        capital_time_weight: left_weight,
                        carry: FractionalCarry::ZERO,
                    }),
                    Some(FeeAllocationInputV1 {
                        tranche_id: id(11),
                        fee_policy_id: id(6),
                        snapshot_epoch: 15,
                        lp_share_supply: 100,
                        tranche_generation: 1,
                        capital_time_weight: right_weight,
                        carry: FractionalCarry::ZERO,
                    }),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ];
                let first = allocate_fee_pot(id(50), pot, 2, &inputs).unwrap();
                assert_eq!(
                    first.fee_pot_atoms() + first.prior_carry_escrow_atoms(),
                    first.credited_atoms() + first.retained_carry_escrow_atoms()
                );
                let next_inputs = [
                    Some(FeeAllocationInputV1 {
                        carry: first.output(0).unwrap().new_carry(),
                        ..inputs[0].unwrap()
                    }),
                    Some(FeeAllocationInputV1 {
                        carry: first.output(1).unwrap().new_carry(),
                        ..inputs[1].unwrap()
                    }),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ];
                let second = allocate_fee_pot(id(51), 3, 2, &next_inputs).unwrap();
                assert_eq!(
                    second.fee_pot_atoms() + second.prior_carry_escrow_atoms(),
                    second.credited_atoms() + second.retained_carry_escrow_atoms()
                );
                assert_eq!(
                    second.prior_carry_escrow_atoms(),
                    first.retained_carry_escrow_atoms()
                );
            }
        }
    }
}
