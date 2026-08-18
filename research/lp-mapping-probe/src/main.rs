//! PROPOSED research probe for `docs/implementation/OPTIMALITY_CERTIFICATE_MAPPING.md`.
//!
//! Nothing here is a shipped artifact, a policy selection, or an evidence claim.
//! It exists to make three mapping claims falsifiable on real code rather than
//! on prose, using only the public API of `clutch-batch`.
//!
//! E1  the per-tick flow closed form `B_i = min(D_i, S_i + c)` equals the LP
//!     maximum of score component 1 over the whole conservation polytope
//!     (brute force over every integer fill vector of a small book);
//! E2  the canonical allocation is NOT the argmax of the frozen `ScoreV1`
//!     order inside a single tick (component 4, distinct owners);
//! E3  allocation policy A's `StrictUnderfill` refusal removes the
//!     component-1-maximal tick from the searched grid, so the grid argmax
//!     under A is strictly worse than under B.

use clutch_batch::relation_v1::*;
use clutch_batch::{DustPolicy, PartialPolicy, Side, MAX_ORDERS};

const SCALE: u64 = PRICE_SCALE;

fn policy(alloc: AllocationPolicyV1, cross: SelfCrossPolicyV1) -> FrozenPolicyV1 {
    FrozenPolicyV1 {
        allocation: alloc,
        self_cross: cross,
        aon: AonPolicyV1::RefuseAdmission,
        rounding: RoundingBoundaryV1::TerminalOwnerFloor,
        residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
        transfer_phase: TransferPhaseV1::ActiveOrResolved,
        portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
        dust: DustPolicy::AssignCanonical,
        score: ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    }
}

fn domain(alloc: AllocationPolicyV1, cross: SelfCrossPolicyV1, owners: u16) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 11,
        book_id: 22,
        epoch: 7,
        policy_id: 33,
        order_set_id: 44,
        outcome_count: 2,
        owner_count: owners,
        price_scale: SCALE,
        remainder_seed: 7,
        policy: policy(alloc, cross),
    }
}

fn single(id: u64, owner: u16, outcome: u8, side: Side, quantity: u64, limit: u64) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity,
        limit_price: limit,
        minimum_fill: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: u64::MAX,
    })
}

fn book_of(orders: &[OrderV1]) -> BookV1 {
    let mut book = BookV1::empty();
    for (i, o) in orders.iter().enumerate() {
        book.orders[i] = *o;
    }
    book.len = orders.len() as u8;
    book
}

fn prices(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut v = [0u64; MAX_OUTCOMES];
    v[..values.len()].copy_from_slice(values);
    v
}

// ---------------------------------------------------------------- E1

/// Component 1 of `ScoreV1`, recomputed from a raw fill vector, for a book of
/// single-Egg orders on one outcome set, under `N-a` (overlap term is zero).
fn component_one(book: &BookV1, p: &[u64; MAX_OUTCOMES], fills: &[u64], outcomes: usize) -> i128 {
    let mut total = 0i128;
    for i in 0..outcomes {
        let (mut b, mut e) = (0i128, 0i128);
        for j in 0..book.len as usize {
            if let OrderV1::SingleEgg(o) = book.orders[j] {
                if o.outcome as usize != i {
                    continue;
                }
                match o.side {
                    Side::Buy => b += fills[j] as i128,
                    Side::Sell => e += fills[j] as i128,
                }
            }
        }
        let direct = b.min(e);
        let price = p[i] as i128;
        total += price * (SCALE as i128 - price) * direct;
    }
    total
}

/// Brute force the LP polytope over integer fill vectors: every `f` with
/// `0 <= f_j <= cap_j` (cap 0 for ineligible), one constant net imbalance on
/// every active outcome.  Reports the maximum of component 1.
fn brute_force_component_one(
    domain: &RelationDomainV1,
    book: &BookV1,
    p: &[u64; MAX_OUTCOMES],
) -> Option<(i128, Vec<u64>)> {
    let n = book.len as usize;
    let outcomes = domain.outcome_count as usize;
    let normalized = normalize(domain, book).ok()?;
    let mut caps = vec![0u64; n];
    for j in 0..n {
        let class = classify_order(domain, &book.orders[j], p).ok()?;
        caps[j] = if class == EligibilityV1::Ineligible {
            0
        } else {
            normalized.effective_quantity(j)
        };
    }
    let mut best: Option<(i128, Vec<u64>)> = None;
    let mut f = vec![0u64; n];
    loop {
        // conservation: B_i - E_i must be the same constant on every outcome
        // that carries any capacity (empty outcomes force c = 0, see below).
        let mut imbalances = Vec::new();
        for i in 0..outcomes {
            let (mut b, mut e) = (0i128, 0i128);
            for j in 0..n {
                if let OrderV1::SingleEgg(o) = book.orders[j] {
                    if o.outcome as usize != i {
                        continue;
                    }
                    match o.side {
                        Side::Buy => b += f[j] as i128,
                        Side::Sell => e += f[j] as i128,
                    }
                }
            }
            imbalances.push(b - e);
        }
        if imbalances.iter().all(|c| *c == imbalances[0]) {
            let value = component_one(book, p, &f, outcomes);
            if best.as_ref().map(|(v, _)| value > *v).unwrap_or(true) {
                best = Some((value, f.clone()));
            }
        }
        // odometer
        let mut k = 0usize;
        loop {
            if k == n {
                return best;
            }
            if f[k] < caps[k] {
                f[k] += 1;
                break;
            }
            f[k] = 0;
            k += 1;
        }
    }
}

fn e1() {
    println!("== E1  per-tick closed form vs brute-forced LP maximum of component 1 ==");
    let d = domain(
        AllocationPolicyV1::FullProRata,
        SelfCrossPolicyV1::RefuseOverlap,
        8,
    );
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 6, 7000),
        single(2, 1, 0, Side::Buy, 4, 5000),
        single(3, 2, 0, Side::Sell, 5, 3000),
        single(4, 3, 0, Side::Sell, 3, 5000),
        single(5, 4, 1, Side::Buy, 4, 6000),
        single(6, 5, 1, Side::Sell, 4, 2000),
    ]);
    let mut agree = 0usize;
    let mut disagree = 0usize;
    for a in (0..=SCALE).step_by(1000) {
        let p = prices(&[a, SCALE - a]);
        let brute = brute_force_component_one(&d, &book, &p);
        // the relation's own answer at the best imbalance it admits
        let mut relation_best: Option<i128> = None;
        for c in -4i64..=4 {
            if let Ok(cand) = canonical_candidate(&d, &book, &p, c, 0) {
                if verify(&d, &book, &cand, None).is_ok() {
                    let v = cand.claimed_score.weighted_direct_volume;
                    if relation_best.map(|b| v > b).unwrap_or(true) {
                        relation_best = Some(v);
                    }
                }
            }
        }
        match (brute, relation_best) {
            (Some((bv, _)), Some(rv)) => {
                if bv == rv {
                    agree += 1;
                } else {
                    disagree += 1;
                    println!("  p0={a:5}  brute={bv}  relation={rv}   <-- DISAGREE");
                }
            }
            (b, r) => println!("  p0={a:5}  brute={b:?} relation={r:?}  (one side empty)"),
        }
    }
    println!("  agree={agree}  disagree={disagree}");
    println!();
}

// ---------------------------------------------------------------- E2

fn e2() {
    println!("== E2  canonical allocation vs the frozen ScoreV1 argmax inside one tick ==");
    let d = domain(
        AllocationPolicyV1::PricePriorityMarginalProRata,
        SelfCrossPolicyV1::RefuseOverlap,
        8,
    );
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 10, SCALE / 2),
        single(2, 1, 0, Side::Buy, 1, SCALE / 2),
        single(3, 2, 0, Side::Buy, 1, SCALE / 2),
        single(4, 3, 0, Side::Sell, 3, 0),
    ]);
    let p = prices(&[SCALE / 2, SCALE / 2]);
    let canonical = canonical_candidate(&d, &book, &p, 0, 0).expect("canonical candidate");
    let summary = verify(&d, &book, &canonical, None).expect("canonical verifies");
    println!("  canonical fills      = {:?}", &canonical.fills[..4]);
    println!(
        "  canonical score      = c1={} c3={} owners={} churn={}",
        summary.score.weighted_direct_volume,
        summary.score.limit_surplus_price_units,
        summary.score.distinct_owners,
        summary.score.churn
    );

    // the rival: same tick, same conservation, same caps, spread across owners
    let mut rival = canonical;
    rival.fills[0] = 1;
    rival.fills[1] = 1;
    rival.fills[2] = 1;
    let rival_verdict = verify(&d, &book, &rival, None);
    println!("  rival fills          = {:?}", &rival.fills[..4]);
    println!("  rival verdict        = {rival_verdict:?}");

    // score the rival with the relation's own recomputation, bypassing the
    // canonical-equality gate only (nothing else is relaxed).
    let normalized = normalize(&d, &book).unwrap();
    let flows = flows_from_fills(&d, &normalized, &rival.fills).unwrap();
    let mut seen = [false; MAX_ORDERS];
    for j in 0..normalized.len as usize {
        if rival.fills[j] != 0 {
            seen[normalized.owner_slot[j] as usize] = true;
        }
    }
    let owners = seen.iter().filter(|b| **b).count() as u16;
    let mut c1 = 0i128;
    for i in 0..(d.outcome_count as usize) {
        let direct = (flows.buy[i].min(flows.sell[i])) as i128;
        let price = p[i] as i128;
        c1 += price * (SCALE as i128 - price) * direct;
    }
    println!("  rival c1={c1}  rival distinct_owners={owners}");
    let strictly_better = c1 == summary.score.weighted_direct_volume
        && owners > summary.score.distinct_owners;
    println!("  rival ties c1 and strictly beats component 4 : {strictly_better}");
    println!();
}

// ---------------------------------------------------------------- E3

fn e3() {
    println!("== E3  allocation A's StrictUnderfill removes the best tick from the grid ==");
    let book = book_of(&[
        single(1, 0, 0, Side::Buy, 10, 6000),
        single(2, 1, 0, Side::Sell, 5, 4000),
    ]);
    let bounds = SearchBoundsV1 {
        price_step: 500,
        max_imbalance: 2,
        max_visits: 200_000,
    };
    for (name, alloc) in [
        ("A price-priority", AllocationPolicyV1::PricePriorityMarginalProRata),
        ("B full pro-rata", AllocationPolicyV1::FullProRata),
    ] {
        let d = domain(alloc, SelfCrossPolicyV1::RefuseOverlap, 8);
        match propose_best_valid(&d, &book, &bounds) {
            Ok(best) => {
                println!(
                    "  {name:18}  best p0={:5}  c1={}  fills={:?}",
                    best.prices[0],
                    best.claimed_score.weighted_direct_volume,
                    &best.fills[..2]
                );
            }
            Err(e) => println!("  {name:18}  {e:?}"),
        }
        // which ticks does this policy admit at all?
        let mut admitted = Vec::new();
        for a in (0..=SCALE).step_by(500) {
            let p = prices(&[a, SCALE - a]);
            if canonical_candidate(&d, &book, &p, 0, 0).is_ok() {
                admitted.push(a);
            }
        }
        println!("  {name:18}  admitted ticks (c=0): {admitted:?}");
    }
    println!();
}

// ---------------------------------------------------------------- E4

fn e4() {
    println!("== E4  largest-remainder deviation from the fractional pro-rata point ==");
    // pool of n marginal orders, target T; measure ||f - x*||_1 and ||.||_inf
    let d = domain(
        AllocationPolicyV1::PricePriorityMarginalProRata,
        SelfCrossPolicyV1::RefuseOverlap,
        64,
    );
    let mut worst_l1 = 0f64;
    let mut worst_inf = 0f64;
    let mut worst_shape = String::new();
    for n in 2usize..=6 {
        for quantities in shapes(n, 1, 6) {
            let total: u64 = quantities.iter().sum();
            for target in 1..=total.min(12) {
                let mut orders = Vec::new();
                for (k, q) in quantities.iter().enumerate() {
                    orders.push(single(k as u64 + 1, k as u16, 0, Side::Buy, *q, SCALE / 2));
                }
                orders.push(single(99, 60, 0, Side::Sell, target, 0));
                let book = book_of(&orders);
                let p = prices(&[SCALE / 2, SCALE / 2]);
                let Ok(cand) = canonical_candidate(&d, &book, &p, 0, 0) else {
                    continue;
                };
                if verify(&d, &book, &cand, None).is_err() {
                    continue;
                }
                let (mut l1, mut linf) = (0f64, 0f64);
                for (k, q) in quantities.iter().enumerate() {
                    let ideal = (*q as f64) * (target as f64) / (total as f64);
                    let dev = (cand.fills[k] as f64 - ideal).abs();
                    l1 += dev;
                    linf = linf.max(dev);
                }
                if l1 > worst_l1 {
                    worst_l1 = l1;
                    worst_shape = format!("n={n} q={quantities:?} T={target} fills={:?}", &cand.fills[..n]);
                }
                worst_inf = worst_inf.max(linf);
            }
        }
    }
    println!("  worst ||f - x*||_1   = {worst_l1:.4}   at {worst_shape}");
    println!("  worst ||f - x*||_inf = {worst_inf:.4}");
    println!("  predicted bounds: ||.||_inf < 1 ; ||.||_1 <= 2 D (n-D) / n <= n/2");
    println!();
}

fn shapes(n: usize, lo: u64, hi: u64) -> Vec<Vec<u64>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for tail in shapes(n - 1, lo, hi) {
        for q in lo..=hi {
            let mut v = vec![q];
            v.extend(tail.iter().copied());
            out.push(v);
        }
    }
    out
}

fn main() {
    e1();
    e2();
    e3();
    e4();
}
