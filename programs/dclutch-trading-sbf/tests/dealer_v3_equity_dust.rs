//! Adversarial arithmetic for the Dealer junior-equity redemption boundary.
//!
//! `POOL_EQUITY_REDEMPTION_ROUNDING_V3` is the string
//! `floor(burned_shares * scenario_residual / total_shares)`, and it is the one
//! named rounding boundary in the whole multi-LP lifecycle: issuance is an exact
//! cross-multiplication equality and has no rounding at all. Everything a
//! floor-rounded pro-rata pool can be attacked with therefore lands here.
//!
//! The existing corpus (`v3_equity.rs`'s four unit tests and
//! `dealer_v3_multi_lp.rs`'s five) covers a single redemption and the dust it
//! leaves. It does not cover what an LP can do with MANY redemptions, which is
//! the shape that actually drains pools: floor rounding is applied per call, so
//! an LP who slices one exit into pieces gets one truncation per piece against a
//! supply and a residual that both moved in between. Whether that is worth more
//! or less than exiting once is not obvious from the formula and is not implied
//! by any single-step test.
//!
//! These tests state the properties as inequalities over the real planner's
//! outputs. They never re-derive the expected numbers: a test that recomputed
//! `floor(b * r / s)` and asserted the planner agreed would be asserting that
//! one copy of the formula equals another copy of the formula.

use dclutch_trading_sbf::dealer::v3_equity::{
    POOL_EQUITY_REDEMPTION_ROUNDING_V3, PoolEquityActionV3, PoolEquityContributionV3,
    PoolEquityInputV3, PoolEquityPlanV3, PoolEquityRedemptionV3, plan_pool_equity_v3,
};

/// One pool, in the coordinates the planner authenticates.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Pool {
    collateral: u64,
    claims: Vec<u64>,
    obligations: Vec<u64>,
    total_shares: u64,
}

/// Everything one redemption moved out of the pool and into the LP, measured
/// the way the kernel itself measures a pool: **per scenario**.
///
/// Cash and Claims are not independent coordinates. A complete set of Claims —
/// one unit in every scenario — is collateral, which is exactly why the planner
/// decomposes a pro-rata residual vector into `collateral_out` plus a Claims
/// remainder and reports `minimum_complete_sets_to_split`. Comparing the two
/// components separately would call a redemption that returned the same value
/// as cash rather than as Claims an "attack", and would miss a redemption that
/// returned more value in a shape that happened to match. The kernel's own
/// residual is `collateral + Claims_s - obligations_s`, so what an LP got in
/// scenario `s` is `collateral_out + claims_transferred[s]`, and that is what
/// these inequalities are stated over.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Extracted {
    by_scenario: Vec<u64>,
}

impl Extracted {
    fn accumulate(&mut self, collateral: u64, claims: &[u64]) {
        if self.by_scenario.is_empty() {
            self.by_scenario = vec![0; claims.len()];
        }
        for (total, scenario) in self.by_scenario.iter_mut().zip(claims.iter()) {
            *total = total
                .checked_add(collateral)
                .and_then(|value| value.checked_add(*scenario))
                .expect("extracted scenario value fits");
        }
    }

    /// Whether this extraction beat `other` in any scenario.
    fn exceeds(&self, other: &Self) -> bool {
        self.by_scenario
            .iter()
            .zip(other.by_scenario.iter())
            .any(|(mine, theirs)| mine > theirs)
    }
}

/// Redeem `burn` shares and return the plan plus the pool it leaves behind.
fn redeem(pool: &Pool, burn: u64) -> Option<(PoolEquityPlanV3, Pool, Vec<u64>)> {
    let width = pool.claims.len();
    let mut residual_before = vec![0_u64; width];
    let mut residual_after = vec![0_u64; width];
    let mut claims_transferred = vec![0_u64; width];
    let mut claims_after = vec![0_u64; width];
    let plan = plan_pool_equity_v3(
        PoolEquityInputV3 {
            collateral: pool.collateral,
            claims: &pool.claims,
            obligations: &pool.obligations,
            total_shares: pool.total_shares,
            locked_capital_floor: 0,
            action: PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 {
                burned_shares: burn,
            }),
        },
        &mut residual_before,
        &mut residual_after,
        &mut claims_transferred,
        &mut claims_after,
    )
    .ok()?;
    Some((
        plan,
        Pool {
            collateral: plan.collateral_after,
            claims: claims_after,
            obligations: pool.obligations.clone(),
            total_shares: plan.shares_after,
        },
        claims_transferred,
    ))
}

fn corpus() -> Vec<Pool> {
    let mut pools = Vec::new();
    // Deliberately awkward: supplies that divide none of the residuals, and
    // residual vectors whose coordinates have different remainders against the
    // same supply, so every scenario truncates by a different amount.
    for total_shares in [3_u64, 7, 11, 13] {
        for collateral in [0_u64, 1, 5, 17] {
            for claims in [
                vec![0, 1, 2],
                vec![2, 5, 8],
                vec![1, 1, 1],
                vec![0, 0, 31],
                vec![13, 29, 47],
                vec![100, 101, 103],
            ] {
                pools.push(Pool {
                    collateral,
                    claims,
                    obligations: vec![0, 0, 0],
                    total_shares,
                });
            }
        }
    }
    pools
}

#[test]
fn the_named_rounding_boundary_is_the_only_one_and_it_says_floor() {
    // Provenance for every inequality below: the direction of the truncation is
    // what makes "no partition extracts more" the safe direction to assert.
    assert_eq!(
        POOL_EQUITY_REDEMPTION_ROUNDING_V3,
        b"floor(burned_shares * scenario_residual / total_shares)"
    );
}

#[test]
fn slicing_one_exit_into_pieces_never_extracts_more_than_exiting_once() {
    let mut compared = 0_usize;
    for pool in corpus() {
        for total_burn in 1..=pool.total_shares {
            let Some((_, _, single_claims)) = redeem(&pool, total_burn) else {
                continue;
            };
            let (single_plan, _, _) = redeem(&pool, total_burn).expect("planned once already");
            let mut single = Extracted::default();
            single.accumulate(single_plan.collateral_out, &single_claims);

            for first in 1..total_burn {
                let rest = total_burn - first;
                let Some((plan_a, intermediate, claims_a)) = redeem(&pool, first) else {
                    continue;
                };
                let Some((plan_b, _, claims_b)) = redeem(&intermediate, rest) else {
                    continue;
                };
                let mut sliced = Extracted::default();
                sliced.accumulate(plan_a.collateral_out, &claims_a);
                sliced.accumulate(plan_b.collateral_out, &claims_b);
                compared += 1;
                assert!(
                    !sliced.exceeds(&single),
                    "a partitioned exit out-extracted a single one: pool {pool:?}, burn \
                     {total_burn} split {first}+{rest}; sliced {sliced:?} vs single {single:?}. \
                     Floor rounding is applied once per call against a supply and a residual that \
                     both moved in between, so a pool that fails this is drainable by anyone \
                     willing to send more transactions."
                );
            }
        }
    }
    std::eprintln!("dealer-equity-dust: {compared} partitions compared");
    assert!(
        compared > 1_000,
        "the corpus must actually exercise the property; only {compared} partitions compared"
    );
}

#[test]
fn burning_the_whole_supply_leaves_no_value_trapped_behind_the_floor() {
    for pool in corpus() {
        let Some((plan, after, _)) = redeem(&pool, pool.total_shares) else {
            continue;
        };
        assert_eq!(
            plan.shares_after, 0,
            "burning every share must retire the supply: {pool:?}"
        );
        // floor(r * S / S) == r exactly, so a complete exit has no dust at all.
        // If this ever fails, value is stranded in a pool with no owner left to
        // claim it, which is strictly worse than the dust a partial exit leaves.
        assert_eq!(
            after.collateral, 0,
            "collateral stranded after a complete exit: {pool:?} left {after:?}"
        );
        assert!(
            after.claims.iter().all(|value| *value == 0),
            "Claims stranded after a complete exit: {pool:?} left {after:?}"
        );
    }
}

#[test]
fn a_contribution_immediately_redeemed_never_returns_more_than_it_brought() {
    // Issuance is exact and redemption floors, so a round trip must be a loss to
    // the LP or break even. Anything else is free money minted by rounding, and
    // it is the mirror image of the slicing attack: instead of many exits, one
    // entry and one exit.
    let mut round_trips = 0_usize;
    for pool in corpus() {
        for scale in [1_u64, 2, 3] {
            // An exactly proportional basket: every residual coordinate and the
            // collateral scaled by shares/total_shares. Contribute `scale`
            // shares' worth by scaling the whole pool by scale/total_shares
            // only when that division is exact; the planner refuses anything
            // else as dilutive, which is its own tested behaviour.
            let minted = pool.total_shares.checked_mul(scale).expect("shares fit");
            let contribution_collateral =
                pool.collateral.checked_mul(scale).expect("collateral fits");
            let contribution_claims: Vec<u64> = pool
                .claims
                .iter()
                .map(|value| value.checked_mul(scale).expect("claims fit"))
                .collect();
            let width = pool.claims.len();
            let mut residual_before = vec![0_u64; width];
            let mut residual_after = vec![0_u64; width];
            let mut claims_transferred = vec![0_u64; width];
            let mut claims_after = vec![0_u64; width];
            let Ok(plan) = plan_pool_equity_v3(
                PoolEquityInputV3 {
                    collateral: pool.collateral,
                    claims: &pool.claims,
                    obligations: &pool.obligations,
                    total_shares: pool.total_shares,
                    locked_capital_floor: 0,
                    action: PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                        collateral: contribution_collateral,
                        claims: &contribution_claims,
                        minted_shares: minted,
                    }),
                },
                &mut residual_before,
                &mut residual_after,
                &mut claims_transferred,
                &mut claims_after,
            ) else {
                continue;
            };
            let entered = Pool {
                collateral: plan.collateral_after,
                claims: claims_after,
                obligations: pool.obligations.clone(),
                total_shares: plan.shares_after,
            };
            let Some((exit, _, returned_claims)) = redeem(&entered, minted) else {
                continue;
            };
            round_trips += 1;
            let mut brought = Extracted::default();
            brought.accumulate(plan.collateral_in, &claims_transferred);
            let mut returned = Extracted::default();
            returned.accumulate(exit.collateral_out, &returned_claims);
            assert!(
                !returned.exceeds(&brought),
                "a round trip returned more scenario value than it brought: pool {pool:?}, scale \
                 {scale}; brought {brought:?}, returned {returned:?}. Issuance is exact and \
                 redemption floors, so a round trip must break even or lose; anything else is \
                 value minted by rounding."
            );
        }
    }
    std::eprintln!("dealer-equity-dust: {round_trips} round trips completed");
    assert!(
        round_trips > 20,
        "the corpus must actually complete round trips; only {round_trips} did"
    );
}
