//! Terminal two-LP pool life, and the no-cross-LP-subsidy property in numbers.
//!
//! The acceptance question is one sentence: **can LP A's capital ever fund LP
//! B's outcome?** These tests answer it over the REAL planner
//! (`plan_pool_equity_v3`) — the same function the accelerator runs on chain —
//! with concrete values, and they never re-derive the planner's arithmetic. A
//! test that recomputed `floor(burn * residual / supply)` and asserted the
//! planner agreed would be asserting that one copy of a formula equals another.
//!
//! WHAT THIS ESTABLISHES AND WHAT IT DOES NOT. It is evidence about the
//! production planner's arithmetic, executed. It is NOT end-to-end on-chain
//! evidence: selector 9's account admission is separately unsatisfiable
//! (`v3_trade_profile.rs:271` requires identity register 116, which only the
//! LATER request pass ever writes), so the physical venue cannot run today. The
//! inventory moves between deposits are applied to the pool directly, because a
//! trade is exogenous to the equity kernel — it is what the venue DOES to the
//! pool, not something this kernel computes. Every value split between the two
//! LPs is the planner's own.
//!
//! Measurement follows the sibling `dealer_v3_equity_dust.rs`: cash and Claims
//! are not independent coordinates, so what an LP moved in scenario `s` is
//! `collateral + claims_transferred[s]`, per scenario.

use dclutch_trading_sbf::dealer::v3_equity::{
    PoolEquityActionV3, PoolEquityContributionV3, PoolEquityInputV3, PoolEquityPlanV3,
    PoolEquityRedemptionV3, plan_pool_equity_v3,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Pool {
    collateral: u64,
    claims: Vec<u64>,
    obligations: Vec<u64>,
    total_shares: u64,
}

impl Pool {
    /// The kernel's own residual, per scenario: `collateral + claims - obligations`.
    fn residual(&self) -> Vec<u64> {
        self.claims
            .iter()
            .zip(self.obligations.iter())
            .map(|(claim, obligation)| {
                self.collateral
                    .checked_add(*claim)
                    .and_then(|value| value.checked_sub(*obligation))
                    .expect("residual is non-negative in these fixtures")
            })
            .collect()
    }
}

/// Per-scenario value an action moved across the pool boundary.
fn moved(collateral: u64, claims: &[u64]) -> Vec<u64> {
    claims
        .iter()
        .map(|claim| collateral.checked_add(*claim).expect("moved value fits"))
        .collect()
}

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

fn contribute(
    pool: &Pool,
    collateral: u64,
    claims: &[u64],
    minted: u64,
) -> Option<(PoolEquityPlanV3, Pool, Vec<u64>)> {
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
            action: PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                collateral,
                claims,
                minted_shares: minted,
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

/// THE ACCEPTANCE PROPERTY, stated directly: a joining LP cannot round-trip
/// out more than it put in, in any scenario.
///
/// If LP B could contribute and immediately redeem for a profit, that profit is
/// LP A's capital — there is nowhere else for it to come from. So this single
/// inequality IS "LP A's capital never funds LP B's outcome", and it is
/// falsifiable: one scenario where B extracts more than B contributed breaks it.
///
/// The basket is the pool's own coordinates scaled by one, which is exactly
/// proportional by construction, so issuance is the planner's exact
/// cross-multiplication and the only rounding in the round trip is the
/// redemption floor.
#[test]
fn a_joining_lp_cannot_round_trip_out_more_than_it_put_in() {
    let mut checked = 0_usize;
    for total_shares in [3_u64, 7, 11, 13, 100] {
        for collateral in [1_u64, 5, 17, 40] {
            for claims in [
                vec![0_u64, 1, 2],
                vec![2, 5, 8],
                vec![13, 29, 47],
                vec![100, 101, 103],
            ] {
                let pool = Pool {
                    collateral,
                    claims: claims.clone(),
                    obligations: vec![0, 0, 0],
                    total_shares,
                };
                // LP B doubles the pool and takes an equal share of it.
                let Some((join, joined, contributed_claims)) =
                    contribute(&pool, pool.collateral, &pool.claims, total_shares)
                else {
                    continue;
                };
                let contributed = moved(join.collateral_in, &contributed_claims);
                let Some((exit, _after, extracted_claims)) = redeem(&joined, total_shares) else {
                    continue;
                };
                let extracted = moved(exit.collateral_out, &extracted_claims);
                for (scenario, (out, put_in)) in
                    extracted.iter().zip(contributed.iter()).enumerate()
                {
                    assert!(
                        out <= put_in,
                        "scenario {scenario}: a joining LP extracted {out} having contributed \
                         {put_in} -- the difference can only be the other LP's capital. \
                         pool={pool:?} join={join:?} exit={exit:?}"
                    );
                }
                checked += 1;
            }
        }
    }
    // Anti-vacuity: an all-`continue` sweep proves nothing.
    assert!(
        checked >= 40,
        "the corpus must actually execute round trips, only {checked} did"
    );
}

/// Terminal two-LP life with time-separated deposits, and exact conservation.
///
/// Every number below is the planner's. The two inventory moves are applied to
/// the pool directly because a trade is exogenous to the equity kernel; every
/// SPLIT of value between the two LPs is computed by the planner.
#[test]
fn two_lp_life_conserves_every_scenario_and_pays_equal_shares_equally() {
    // 1. LP A founds the pool with cash. First capital creates flat residual.
    let empty = Pool {
        collateral: 0,
        claims: vec![0, 0, 0],
        obligations: vec![0, 0, 0],
        total_shares: 0,
    };
    let (a_join, after_a, _) = contribute(&empty, 100, &[0, 0, 0], 100).expect("LP A founds");
    assert_eq!(a_join.shares_after, 100);
    assert_eq!(after_a.collateral, 100);

    // 2. The venue trades: cash becomes inventory. Exogenous to this kernel.
    let traded = Pool {
        collateral: 40,
        claims: vec![0, 60, 120],
        obligations: vec![0, 0, 0],
        total_shares: after_a.total_shares,
    };
    assert_eq!(traded.residual(), vec![40, 100, 160]);

    // 3. LP B joins LATER, at the traded pool's value, taking an equal share.
    let (b_join, after_b, b_claims) =
        contribute(&traded, 40, &[0, 60, 120], 100).expect("LP B joins");
    let b_contributed = moved(b_join.collateral_in, &b_claims);
    assert_eq!(b_join.shares_after, 200);
    assert_eq!(after_b.residual(), vec![80, 200, 320]);

    // 4. The venue trades again, at a profit.
    let grown = Pool {
        collateral: after_b.collateral + 10,
        claims: after_b.claims.clone(),
        obligations: after_b.obligations.clone(),
        total_shares: after_b.total_shares,
    };
    let before_exit = grown.residual();

    // 5. Both LPs exit, in both orders.
    let (a_first, b_second, pool_ab) = {
        let (pa, mid, ca) = redeem(&grown, 100).expect("A exits first");
        let (pb, end, cb) = redeem(&mid, 100).expect("B exits second");
        (
            moved(pa.collateral_out, &ca),
            moved(pb.collateral_out, &cb),
            end,
        )
    };
    let (b_first, a_second, pool_ba) = {
        let (pb, mid, cb) = redeem(&grown, 100).expect("B exits first");
        let (pa, end, ca) = redeem(&mid, 100).expect("A exits second");
        (
            moved(pb.collateral_out, &cb),
            moved(pa.collateral_out, &ca),
            end,
        )
    };

    // CONSERVATION, exact and per scenario: what the two LPs took plus what the
    // pool still holds is exactly what was there. Nothing was created.
    for (scenario, before) in before_exit.iter().enumerate() {
        let left = pool_ab.residual()[scenario];
        let paid = a_first[scenario] + b_second[scenario];
        assert_eq!(
            paid + left,
            *before,
            "scenario {scenario}: {paid} paid + {left} left != {before} before"
        );
    }

    // NO FIRST-MOVER SUBSIDY: exiting first is worth exactly the same as
    // exiting second. If it were worth more, the later LP funded the earlier.
    assert_eq!(a_first, a_second, "LP A's exit must not depend on its order");
    assert_eq!(b_first, b_second, "LP B's exit must not depend on its order");
    assert_eq!(pool_ab.residual(), pool_ba.residual());

    // EQUAL SHARES, EQUAL VALUE: both hold 100 of 200 shares, and B entered at
    // the traded pool's own value, so neither may out-earn the other.
    assert_eq!(a_first, b_second, "equal share counts must redeem equally");

    // AND B DID NOT EXTRACT A's CAPITAL: B took no more than B contributed plus
    // B's half of the 10 the venue actually earned.
    for (scenario, (out, put_in)) in b_second.iter().zip(b_contributed.iter()).enumerate() {
        assert!(
            *out <= put_in + 5,
            "scenario {scenario}: B extracted {out} on {put_in} contributed, \
             above its half of the 10 earned"
        );
    }
}
