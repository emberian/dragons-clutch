//! The scoring Dealer's kernel: Hanson's logarithmic market scoring rule in
//! base two and Q62 fixed point, exactly as `ScoringRuleV1.lean` states it.
//!
//! Lean owns the meaning: `Ê` (`exp2Neg`), `L̂` (`log2Ceil`), the potential
//! `Ŵ(inv) = min − ⌈b·L̂(Σ Ê)/2^62⌉` (`lmsrValue`), the prices `p̂`
//! (`pricesOf`), the founding subsidy (`subsidyOf`), and the participation
//! rule R0–R3 (`Fill.admissible`). The 63-entry root-chain table is Lean data
//! emitted into [`generated`]; every rounding direction is the Lean's. This
//! module is `no_std`, `no_alloc`, total, and every intermediate is bounded
//! under [`parameters_admissible`] so a `u128` never overflows -- and each
//! arithmetic step is still checked, so a violated bound is a named refusal
//! rather than a wrap.
//!
//! What runs on chain is exactly what runs here: `Ê`, `L̂` and one comparison
//! per conjunct. No logarithm is inverted on chain; the inversion a taker
//! needs is [`solver`], host-only.
//!
//! The kernel speaks in CLAIM UNITS. The fund's cash is in collateral atoms
//! and the conversion is the fund's recorded `claim_unit_atoms`
//! (`ProductBasisV3::payout_scale`, decision 0025's refund scale); the routes
//! convert at exactly one place, [`records_v1::DealerFundV1::debit_atoms`].

#![allow(
    clippy::indexing_slicing,
    reason = "every index below is a coordinate under MAX_OUTCOMES, the arrays' own length, or a table row under 63; the kernel is total by construction and a bound is stated at each loop"
)]

#[rustfmt::skip]
pub mod generated_scoring_rule;
/// The three persisted records: the sealed rule, the fund, the quote.
pub mod records_v1;
/// The four request wires, the one receipt, the fill witness, the frames.
pub mod requests_v1;
/// Host-only inversion of the rule for a taker with a target price.
#[cfg(not(target_os = "solana"))]
pub mod solver;

pub use generated_scoring_rule as generated;

use generated::{
    EXP2_NEG_TABLE_Q62, LOG_SLACK, MAX_LIQUIDITY, MAX_OUTCOMES, MIN_OUTCOMES, ONE_Q62,
};

/// One fixed-capacity per-outcome vector; coordinates at or past the rule's
/// `outcome_count` are canonically zero.
pub type Vector = [u64; MAX_OUTCOMES];

/// The zero vector.
pub const ZERO_VECTOR: Vector = [0; MAX_OUTCOMES];

/// Why the rule refused. Each variant is one conjunct of `ScoringRuleV1`, named
/// for the program's sub-band to carry as its own discriminant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoringRefusal {
    /// `outcome_count` outside `2 ..= 16`.
    OutcomeCount,
    /// `liquidity` outside `1 ..= 2^40`.
    Liquidity,
    /// `scale` outside `outcome_count ..= 2^62`.
    Scale,
    /// `tolerance` at or past `scale`.
    Tolerance,
    /// A recorded subsidy that is not `subsidyOf(b, K)`.
    Subsidy,
    /// R0: a vector coordinate at or past `outcome_count` is nonzero.
    Width,
    /// R0: `deliver_i > inventory_i + receive_i`.
    Deliverable,
    /// R0: `receive_i · deliver_i ≠ 0`.
    NonCanonical,
    /// R1: the post-fill inventory holds a complete set.
    NotNormalized,
    /// R2: a price coordinate sits more than `tolerance` from `p̂(inv′)`.
    OffSchedule,
    /// R3: the debit exceeds `Ŵ(inv′) − Ŵ(inv)`.
    Uncovered,
    /// The price vector does not sum to `scale`, or a coordinate is zero.
    PricesNotSimplex,
    /// A checked intermediate left `u128` (unreachable under admitted
    /// parameters; named rather than wrapped).
    Overflow,
    /// A withdrawal above `Φ = cash + Ŵ`.
    WithdrawBelowFloor,
}

/// The sealed parameters of one rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleParameters {
    /// `K`, the ordinary outcome count.
    pub outcome_count: u8,
    /// `b`, claim units.
    pub liquidity: u64,
    /// The price denominator every quote and fill uses.
    pub scale: u64,
    /// `τ`, price units.
    pub tolerance: u64,
}

impl RuleParameters {
    /// `parametersAdmissible`, plus `tolerance < scale`.
    pub fn admit(self) -> Result<Self, ScoringRefusal> {
        let k = usize::from(self.outcome_count);
        if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&k) {
            return Err(ScoringRefusal::OutcomeCount);
        }
        if self.liquidity == 0 || self.liquidity > MAX_LIQUIDITY {
            return Err(ScoringRefusal::Liquidity);
        }
        let scale = u128::from(self.scale);
        if scale < u128::from(self.outcome_count) || scale > ONE_Q62 {
            return Err(ScoringRefusal::Scale);
        }
        if self.tolerance >= self.scale {
            return Err(ScoringRefusal::Tolerance);
        }
        Ok(self)
    }

    /// The active width.
    pub fn width(self) -> usize {
        usize::from(self.outcome_count)
    }
}

/// `parametersAdmissible` as the Lean states it (without the tolerance).
#[must_use]
pub fn parameters_admissible(outcome_count: u8, liquidity: u64, scale: u64) -> bool {
    RuleParameters {
        outcome_count,
        liquidity,
        scale,
        tolerance: 0,
    }
    .admit()
    .is_ok()
}

/// `Ê(d) = max(1, ⌊2^62 · 2^(−d/b)⌋)`, rounded DOWN at every step.
///
/// `d = n·b + r`; `n ≥ 62` floors at one; the fraction `⌈r·2^62/b⌉` is walked
/// most-significant bit first over the root chain, each product floored, then
/// shifted by `n` and floored at one. Total for every `b ≥ 1`.
#[must_use]
pub fn exp2_neg(liquidity: u64, distance: u64) -> u128 {
    if liquidity == 0 {
        return 1;
    }
    let n = distance / liquidity;
    let r = distance % liquidity;
    if n >= 62 {
        return 1;
    }
    let b = u128::from(liquidity);
    // `r < b ≤ 2^64`, so `r · 2^62 + b − 1 < 2^127`.
    let fraction = (u128::from(r) * ONE_Q62 + b - 1) / b;
    let mut product = ONE_Q62;
    let mut j = 0_u32;
    while j < 62 {
        if (fraction >> (61 - j)) & 1 == 1 {
            // `product ≤ 2^62` and `T[k] < 2^62`, so the product is under `2^124`.
            product = product * EXP2_NEG_TABLE_Q62[usize::from(j as u16) + 1] / ONE_Q62;
        }
        j += 1;
    }
    (product >> n).max(1)
}

/// `L̂(s) ≈ 2^62 · log₂(s/2^62)` rounded UP, for `s ≥ 2^62`.
///
/// Integer part from the bit length; 62 floored squarings for the fraction;
/// `LOG_SLACK` pays for the floors (`log2Ceil_above_the_real_value`).
pub fn log2_ceil(sum: u128) -> Result<u128, ScoringRefusal> {
    if sum < ONE_Q62 {
        return Err(ScoringRefusal::Overflow);
    }
    let bits = 128 - sum.leading_zeros();
    let n = bits - 1 - 62;
    let mut x = sum >> n;
    let mut fraction: u128 = 0;
    let mut j = 0_u32;
    while j < 62 {
        let y = x * x / ONE_Q62;
        if y >= 2 * ONE_Q62 {
            fraction += 1_u128 << (61 - j);
            x = y / 2;
        } else {
            x = y;
        }
        j += 1;
    }
    Ok(u128::from(n) * ONE_Q62 + fraction + LOG_SLACK)
}

/// `Ê` at every active coordinate's distance above the minimum, and the minimum.
fn exponentials(rule: RuleParameters, inventory: &Vector) -> ([u128; MAX_OUTCOMES], u64) {
    let width = rule.width();
    let mut minimum = u64::MAX;
    let mut i = 0;
    while i < width {
        if inventory[i] < minimum {
            minimum = inventory[i];
        }
        i += 1;
    }
    let mut values = [0_u128; MAX_OUTCOMES];
    let mut i = 0;
    while i < width {
        values[i] = exp2_neg(rule.liquidity, inventory[i] - minimum);
        i += 1;
    }
    (values, minimum)
}

/// The integer potential as the pair the fund persists: `Ŵ = minimum − cost`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Potential {
    /// `min_i inv_i`, claim units.
    pub minimum: u64,
    /// `⌈b · L̂(Σ Ê) / 2^62⌉`, claim units.
    pub cost: u64,
}

impl Potential {
    /// `Ŵ` as the signed integer the theorems speak about.
    #[must_use]
    pub fn value(self) -> i128 {
        i128::from(self.minimum) - i128::from(self.cost)
    }
}

/// `liquidityCost`: `⌈ b · L̂(Σ_i Ê_i) / 2^62 ⌉`.
pub fn liquidity_cost(rule: RuleParameters, inventory: &Vector) -> Result<u64, ScoringRefusal> {
    let (values, _) = exponentials(rule, inventory);
    let mut sum: u128 = 0;
    let mut i = 0;
    while i < rule.width() {
        sum = sum.checked_add(values[i]).ok_or(ScoringRefusal::Overflow)?;
        i += 1;
    }
    let log = log2_ceil(sum)?;
    let scaled = u128::from(rule.liquidity)
        .checked_mul(log)
        .ok_or(ScoringRefusal::Overflow)?;
    let cost = scaled
        .checked_add(ONE_Q62 - 1)
        .ok_or(ScoringRefusal::Overflow)?
        / ONE_Q62;
    u64::try_from(cost).map_err(|_| ScoringRefusal::Overflow)
}

/// `Ŵ(inv)`.
pub fn potential(rule: RuleParameters, inventory: &Vector) -> Result<Potential, ScoringRefusal> {
    let (_, minimum) = exponentials(rule, inventory);
    Ok(Potential {
        minimum,
        cost: liquidity_cost(rule, inventory)?,
    })
}

/// `subsidyOf b K = liquidityCost b (replicate K 0)`: what the founding must
/// deposit, `b · log₂ K` rounded up by at most one claim unit.
pub fn subsidy_of(rule: RuleParameters) -> Result<u64, ScoringRefusal> {
    liquidity_cost(rule, &ZERO_VECTOR)
}

/// `pricesOf`: `p̂_i = 1 + ⌊Ê_i · (scale − K) / Σ Ê⌋`, the shortfall to `scale`
/// on the lowest index attaining the minimum inventory. Every coordinate is
/// at least one and the active coordinates sum to `scale` (`pricesOf_sum`,
/// `pricesOf_pos`, `pricesOf_lt`).
pub fn prices_of(rule: RuleParameters, inventory: &Vector) -> Result<Vector, ScoringRefusal> {
    let width = rule.width();
    let (values, minimum) = exponentials(rule, inventory);
    let mut sum: u128 = 0;
    let mut i = 0;
    while i < width {
        sum = sum.checked_add(values[i]).ok_or(ScoringRefusal::Overflow)?;
        i += 1;
    }
    let room = u128::from(rule.scale) - u128::from(rule.outcome_count);
    let mut prices = ZERO_VECTOR;
    let mut raw_sum: u128 = 0;
    let mut i = 0;
    while i < width {
        let raw = 1 + values[i]
            .checked_mul(room)
            .ok_or(ScoringRefusal::Overflow)?
            / sum;
        prices[i] = u64::try_from(raw).map_err(|_| ScoringRefusal::Overflow)?;
        raw_sum += raw;
        i += 1;
    }
    let residual = u128::from(rule.scale)
        .checked_sub(raw_sum)
        .ok_or(ScoringRefusal::Overflow)?;
    let mut index_of_min = 0;
    let mut i = 0;
    while i < width {
        if inventory[i] == minimum {
            index_of_min = i;
            break;
        }
        i += 1;
    }
    prices[index_of_min] = prices[index_of_min]
        .checked_add(u64::try_from(residual).map_err(|_| ScoringRefusal::Overflow)?)
        .ok_or(ScoringRefusal::Overflow)?;
    Ok(prices)
}

/// A price vector is a simplex at the rule's scale: every active coordinate
/// positive, the inactive ones zero, the sum exactly `scale`.
pub fn require_simplex(rule: RuleParameters, prices: &Vector) -> Result<(), ScoringRefusal> {
    let width = rule.width();
    let mut sum: u128 = 0;
    let mut i = 0;
    while i < MAX_OUTCOMES {
        if i < width {
            if prices[i] == 0 {
                return Err(ScoringRefusal::PricesNotSimplex);
            }
            sum += u128::from(prices[i]);
        } else if prices[i] != 0 {
            return Err(ScoringRefusal::Width);
        }
        i += 1;
    }
    if sum != u128::from(rule.scale) {
        return Err(ScoringRefusal::PricesNotSimplex);
    }
    Ok(())
}

/// The one canonical net quote for `(receive y, deliver z)` at `p̂`
/// (`roundedQuoteFor`): receipt rounded UP, delivery rounded DOWN, once per
/// fill, in claim units. Returned as the pair `(dealer_pays, dealer_receives)`
/// of which at most one is nonzero.
pub fn rounded_debit(
    rule: RuleParameters,
    prices: &Vector,
    receive: &Vector,
    deliver: &Vector,
) -> Result<(u64, u64), ScoringRefusal> {
    let scale = u128::from(rule.scale);
    let mut receipt: u128 = 0;
    let mut delivery: u128 = 0;
    let mut i = 0;
    while i < rule.width() {
        receipt = receipt
            .checked_add(
                u128::from(receive[i])
                    .checked_mul(u128::from(prices[i]))
                    .ok_or(ScoringRefusal::Overflow)?,
            )
            .ok_or(ScoringRefusal::Overflow)?;
        delivery = delivery
            .checked_add(
                u128::from(deliver[i])
                    .checked_mul(u128::from(prices[i]))
                    .ok_or(ScoringRefusal::Overflow)?,
            )
            .ok_or(ScoringRefusal::Overflow)?;
        i += 1;
    }
    let paid = receipt
        .checked_add(scale - 1)
        .ok_or(ScoringRefusal::Overflow)?
        / scale;
    let earned = delivery / scale;
    let pair = if paid >= earned {
        (paid - earned, 0)
    } else {
        (0, earned - paid)
    };
    Ok((
        u64::try_from(pair.0).map_err(|_| ScoringRefusal::Overflow)?,
        u64::try_from(pair.1).map_err(|_| ScoringRefusal::Overflow)?,
    ))
}

/// What an admitted fill leaves behind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedFill {
    /// `inv′ = inv + y − z`.
    pub next_inventory: Vector,
    /// `Ŵ(inv)`.
    pub potential_before: Potential,
    /// `Ŵ(inv′)`.
    pub potential_after: Potential,
    /// `p̂(inv′)`, the schedule the batch price was held to.
    pub schedule: Vector,
    /// Claim units the Dealer pays for the fill (its debit, if positive).
    pub dealer_pays: u64,
    /// Claim units the Dealer receives (its debit, if negative).
    pub dealer_receives: u64,
}

/// The participation rule, R0–R3, exactly as `Fill.admissible` plus R2.
///
/// The debit is derived here from `(y, z, p̂)` by [`rounded_debit`]; a caller
/// carries no debit on the wire, so there is one author of it.
pub fn admit_fill(
    rule: RuleParameters,
    inventory: &Vector,
    receive: &Vector,
    deliver: &Vector,
    prices: &Vector,
) -> Result<AdmittedFill, ScoringRefusal> {
    let width = rule.width();
    // R0: widths, deliverable, canonical.
    let mut next = ZERO_VECTOR;
    let mut i = 0;
    while i < MAX_OUTCOMES {
        if i >= width {
            if inventory[i] != 0 || receive[i] != 0 || deliver[i] != 0 {
                return Err(ScoringRefusal::Width);
            }
        } else {
            let held = inventory[i]
                .checked_add(receive[i])
                .ok_or(ScoringRefusal::Overflow)?;
            if deliver[i] > held {
                return Err(ScoringRefusal::Deliverable);
            }
            if receive[i] != 0 && deliver[i] != 0 {
                return Err(ScoringRefusal::NonCanonical);
            }
            next[i] = held - deliver[i];
        }
        i += 1;
    }
    // R1: normalized after.
    let mut min_after = u64::MAX;
    let mut i = 0;
    while i < width {
        if next[i] < min_after {
            min_after = next[i];
        }
        i += 1;
    }
    if min_after != 0 {
        return Err(ScoringRefusal::NotNormalized);
    }
    // R2: the batch price is the Dealer's marginal price at its post-fill state.
    require_simplex(rule, prices)?;
    let schedule = prices_of(rule, &next)?;
    let mut i = 0;
    while i < width {
        let gap = prices[i].abs_diff(schedule[i]);
        if gap > rule.tolerance {
            return Err(ScoringRefusal::OffSchedule);
        }
        i += 1;
    }
    // R3: the potential covers the cash.
    let (dealer_pays, dealer_receives) = rounded_debit(rule, prices, receive, deliver)?;
    let potential_before = potential(rule, inventory)?;
    let potential_after = potential(rule, &next)?;
    let debit = i128::from(dealer_pays) - i128::from(dealer_receives);
    if debit > potential_after.value() - potential_before.value() {
        return Err(ScoringRefusal::Uncovered);
    }
    Ok(AdmittedFill {
        next_inventory: next,
        potential_before,
        potential_after,
        schedule,
        dealer_pays,
        dealer_receives,
    })
}

/// `withdraw_floor`: `amount ≤ Φ = cash + Ŵ`, with cash and amount in claim
/// units. The routes convert atoms to claim units before asking.
pub fn withdraw_admissible(
    cash_claims: u64,
    potential: Potential,
    amount_claims: u64,
) -> Result<(), ScoringRefusal> {
    let phi = i128::from(cash_claims) + potential.value();
    if i128::from(amount_claims) > phi {
        return Err(ScoringRefusal::WithdrawBelowFloor);
    }
    Ok(())
}

/// The founding's own arithmetic: the deposit covers the subsidy and the
/// recorded subsidy is `subsidyOf`.
pub fn admit_founding(
    rule: RuleParameters,
    recorded_subsidy: u64,
    deposit_claims: u64,
) -> Result<u64, ScoringRefusal> {
    let rule = rule.admit()?;
    let subsidy = subsidy_of(rule)?;
    if recorded_subsidy != subsidy || deposit_claims < subsidy {
        return Err(ScoringRefusal::Subsidy);
    }
    Ok(subsidy)
}

#[cfg(test)]
mod tests {
    use super::generated::{
        EXP2_NEG_CORPUS, LMSR_VALUE_CORPUS_COST, LMSR_VALUE_CORPUS_INVENTORY,
        LMSR_VALUE_CORPUS_MINIMUM, LOG2_CEIL_CORPUS, PRICES_CORPUS_FIVE, PRICES_CORPUS_SKEWED,
        PRICES_CORPUS_TWO, SUBSIDY_CORPUS,
    };
    use super::*;

    fn rule(k: u8, b: u64) -> RuleParameters {
        RuleParameters {
            outcome_count: k,
            liquidity: b,
            scale: u64::try_from(ONE_Q62).expect("2^62 fits u64"),
            tolerance: 1,
        }
        .admit()
        .expect("admitted")
    }

    fn vector(values: &[u64]) -> Vector {
        let mut out = ZERO_VECTOR;
        out[..values.len()].copy_from_slice(values);
        out
    }

    #[test]
    fn the_table_is_the_root_chain() {
        // `table_is_the_root_chain`, re-checked over the emitted literals: the
        // Lean pins them, and this pins the emission to the Lean.
        assert_eq!(EXP2_NEG_TABLE_Q62[0], 1_u128 << 61);
        for k in 0..62 {
            let square = EXP2_NEG_TABLE_Q62[k] * ONE_Q62;
            let root = EXP2_NEG_TABLE_Q62[k + 1];
            assert!(root * root <= square, "row {k}");
            assert!(square < (root + 1) * (root + 1), "row {k}");
        }
    }

    #[test]
    fn exp2_neg_agrees_with_the_lean_corpus() {
        for (b, d, expected) in EXP2_NEG_CORPUS {
            assert_eq!(exp2_neg(b, d), expected, "b={b} d={d}");
        }
    }

    #[test]
    fn log2_ceil_agrees_with_the_lean_corpus() {
        for (s, expected) in LOG2_CEIL_CORPUS {
            assert_eq!(log2_ceil(s), Ok(expected), "s={s}");
        }
    }

    #[test]
    fn subsidy_agrees_with_the_lean_corpus() {
        for (b, k, expected) in SUBSIDY_CORPUS {
            assert_eq!(subsidy_of(rule(k, b)), Ok(expected), "b={b} k={k}");
        }
    }

    #[test]
    fn potential_agrees_with_the_lean_corpus() {
        let inventory = vector(&LMSR_VALUE_CORPUS_INVENTORY);
        let value = potential(rule(5, 1 << 30), &inventory).expect("potential");
        assert_eq!(value.minimum, LMSR_VALUE_CORPUS_MINIMUM);
        assert_eq!(value.cost, LMSR_VALUE_CORPUS_COST);
    }

    #[test]
    fn prices_agree_with_the_lean_corpus() {
        let two = rule(2, 1 << 30);
        assert_eq!(
            prices_of(two, &vector(&[0, 0])).expect("prices")[..2],
            PRICES_CORPUS_TWO
        );
        assert_eq!(
            prices_of(two, &vector(&[0, 1 << 30])).expect("prices")[..2],
            PRICES_CORPUS_SKEWED
        );
        let five = rule(5, 1 << 30);
        assert_eq!(
            prices_of(five, &vector(&LMSR_VALUE_CORPUS_INVENTORY)).expect("prices")[..5],
            PRICES_CORPUS_FIVE
        );
    }

    #[test]
    fn prices_are_a_simplex_at_every_sampled_state() {
        let five = rule(5, 1 << 20);
        for seed in 0_u64..64 {
            let mut inventory = ZERO_VECTOR;
            for (i, slot) in inventory.iter_mut().take(5).enumerate() {
                *slot = (seed
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(i as u64 * 977))
                    % (1 << 22);
            }
            let prices = prices_of(five, &inventory).expect("prices");
            require_simplex(five, &prices).expect("simplex");
            assert!(prices[..5].iter().all(|price| *price >= 1));
        }
    }

    #[test]
    fn the_null_fill_at_the_schedule_is_admitted() {
        let two = rule(2, 1 << 30);
        let inventory = vector(&[0, 1 << 29]);
        let schedule = prices_of(two, &inventory).expect("prices");
        let admitted = admit_fill(two, &inventory, &ZERO_VECTOR, &ZERO_VECTOR, &schedule)
            .expect("the null fill clears every batch");
        assert_eq!(admitted.dealer_pays, 0);
        assert_eq!(admitted.dealer_receives, 0);
        assert_eq!(admitted.next_inventory, inventory);
    }

    #[test]
    fn a_fill_that_leaves_a_complete_set_is_refused_by_name() {
        let two = rule(2, 1 << 30);
        let inventory = vector(&[0, 5]);
        let receive = vector(&[7, 0]);
        let next = vector(&[7, 5]);
        let schedule = prices_of(two, &next).expect("prices");
        assert_eq!(
            admit_fill(two, &inventory, &receive, &ZERO_VECTOR, &schedule),
            Err(ScoringRefusal::NotNormalized)
        );
    }

    #[test]
    fn a_price_off_the_schedule_is_refused_by_name() {
        let two = rule(2, 1 << 30);
        let inventory = vector(&[0, 1 << 20]);
        let receive = vector(&[0, 1 << 10]);
        let next = vector(&[0, (1 << 20) + (1 << 10)]);
        let mut prices = prices_of(two, &next).expect("prices");
        prices[0] -= 1_000;
        prices[1] += 1_000;
        assert_eq!(
            admit_fill(two, &inventory, &receive, &ZERO_VECTOR, &prices),
            Err(ScoringRefusal::OffSchedule)
        );
    }

    #[test]
    fn a_large_fill_at_the_schedule_is_covered_and_a_shaded_one_is_refused() {
        // The Dealer receives outcome 1 (the taker sells it); at the post-fill
        // schedule the LMSR's own curvature covers the rounding for a fill of
        // more than about sqrt(2b) claims.
        let two = rule(2, 1 << 30);
        let inventory = vector(&[0, 0]);
        let receive = vector(&[0, 1 << 20]);
        let next = vector(&[0, 1 << 20]);
        let schedule = prices_of(two, &next).expect("prices");
        let admitted =
            admit_fill(two, &inventory, &receive, &ZERO_VECTOR, &schedule).expect("covered");
        assert!(admitted.dealer_pays > 0);
        assert_eq!(admitted.dealer_receives, 0);
        let covered = admitted.potential_after.value() - admitted.potential_before.value();
        assert!(i128::from(admitted.dealer_pays) <= covered);
        // Push the price the Dealer pays for outcome 1 UP by more than the
        // curvature buys. The slack is `‖y‖²/(2b) = 2^40/2^31 = 512` claim
        // units; a shade of `2^52` price units on `2^20` claims at scale
        // `2^62` adds `2^10 = 1024` units to the debit, past the slack, so R3
        // refuses even though R2 (at this hostile tolerance) admits the price.
        let hostile = RuleParameters {
            tolerance: 1 << 55,
            ..two
        };
        let mut prices = schedule;
        prices[1] += 1 << 52;
        prices[0] -= 1 << 52;
        assert_eq!(
            admit_fill(hostile, &inventory, &receive, &ZERO_VECTOR, &prices),
            Err(ScoringRefusal::Uncovered)
        );
    }

    #[test]
    fn the_potential_never_falls_along_an_admitted_path() {
        // `potential_life`, sampled: a round trip pays the rounding spread each
        // way and the sponsor keeps it.
        let two = rule(2, 1 << 20);
        let mut inventory = ZERO_VECTOR;
        let mut phi_cash: i128 = i128::from(subsidy_of(two).expect("subsidy"));
        let mut phi = phi_cash + potential(two, &inventory).expect("W").value();
        for step in 0..8_u64 {
            let (receive, deliver) = if step % 2 == 0 {
                (vector(&[0, 4_096]), ZERO_VECTOR)
            } else {
                (ZERO_VECTOR, vector(&[0, 4_096]))
            };
            let mut next = inventory;
            for i in 0..2 {
                next[i] = next[i] + receive[i] - deliver[i];
            }
            let schedule = prices_of(two, &next).expect("prices");
            let admitted = admit_fill(two, &inventory, &receive, &deliver, &schedule)
                .expect("each leg admitted at its own schedule");
            phi_cash -= i128::from(admitted.dealer_pays);
            phi_cash += i128::from(admitted.dealer_receives);
            inventory = admitted.next_inventory;
            let phi_next = phi_cash + admitted.potential_after.value();
            assert!(phi_next >= phi, "step {step}: Φ fell {phi} -> {phi_next}");
            phi = phi_next;
        }
    }

    #[test]
    fn founding_holds_the_deposit_to_the_subsidy() {
        let two = rule(2, 1 << 30);
        let subsidy = subsidy_of(two).expect("subsidy");
        assert_eq!(admit_founding(two, subsidy, subsidy), Ok(subsidy));
        assert_eq!(
            admit_founding(two, subsidy, subsidy - 1),
            Err(ScoringRefusal::Subsidy)
        );
        assert_eq!(
            admit_founding(two, subsidy + 1, subsidy + 1),
            Err(ScoringRefusal::Subsidy)
        );
    }

    #[test]
    fn the_withdraw_floor_is_exactly_phi() {
        let two = rule(2, 1 << 30);
        let inventory = vector(&[0, 1 << 20]);
        let w = potential(two, &inventory).expect("W");
        let cash = 5_000_000_000_u64;
        let phi = i128::from(cash) + w.value();
        let phi = u64::try_from(phi).expect("Φ positive here");
        assert_eq!(withdraw_admissible(cash, w, phi), Ok(()));
        assert_eq!(
            withdraw_admissible(cash, w, phi + 1),
            Err(ScoringRefusal::WithdrawBelowFloor)
        );
    }

    #[test]
    fn hostile_parameters_refuse_by_name() {
        let base = RuleParameters {
            outcome_count: 2,
            liquidity: 1 << 30,
            scale: 1 << 62,
            tolerance: 1,
        };
        assert_eq!(
            RuleParameters {
                outcome_count: 1,
                ..base
            }
            .admit(),
            Err(ScoringRefusal::OutcomeCount)
        );
        assert_eq!(
            RuleParameters {
                outcome_count: 17,
                ..base
            }
            .admit(),
            Err(ScoringRefusal::OutcomeCount)
        );
        assert_eq!(
            RuleParameters {
                liquidity: 0,
                ..base
            }
            .admit(),
            Err(ScoringRefusal::Liquidity)
        );
        assert_eq!(
            RuleParameters {
                liquidity: (1 << 40) + 1,
                ..base
            }
            .admit(),
            Err(ScoringRefusal::Liquidity)
        );
        assert_eq!(
            RuleParameters { scale: 1, ..base }.admit(),
            Err(ScoringRefusal::Scale)
        );
        assert_eq!(
            RuleParameters {
                tolerance: 1 << 62,
                ..base
            }
            .admit(),
            Err(ScoringRefusal::Tolerance)
        );
    }

    /// R0's three conjuncts, each by its own name.
    ///
    /// The build wave's tests named R1, R2 and R3; R0 -- the canonical form
    /// the whole rule reads its vectors under -- had none, so a fill that
    /// asked for a coordinate outside the width, or for more than the Dealer
    /// holds, or for both directions at once, was refused by an unnamed code.
    #[test]
    fn the_three_conjuncts_of_r0_refuse_by_name() {
        let two = rule(2, 1 << 30);
        let inventory = vector(&[0, 1 << 20]);
        let schedule = prices_of(two, &inventory).expect("prices");

        // Width: a coordinate at or past K is not zero.
        let mut wide = ZERO_VECTOR;
        wide[2] = 1;
        assert_eq!(
            admit_fill(two, &inventory, &wide, &ZERO_VECTOR, &schedule),
            Err(ScoringRefusal::Width)
        );
        assert_eq!(
            admit_fill(two, &inventory, &ZERO_VECTOR, &wide, &schedule),
            Err(ScoringRefusal::Width)
        );
        let mut wide_inventory = inventory;
        wide_inventory[5] = 3;
        assert_eq!(
            admit_fill(two, &wide_inventory, &ZERO_VECTOR, &ZERO_VECTOR, &schedule),
            Err(ScoringRefusal::Width)
        );

        // Deliverable: the Dealer cannot deliver what it does not hold.
        let over = vector(&[0, (1 << 20) + 1]);
        assert_eq!(
            admit_fill(two, &inventory, &ZERO_VECTOR, &over, &schedule),
            Err(ScoringRefusal::Deliverable)
        );

        // NonCanonical: one coordinate cannot both receive and deliver. The
        // netting is the caller's to do, so the rule prices exactly one form
        // of every trade.
        let receive = vector(&[0, 8]);
        let deliver = vector(&[0, 3]);
        assert_eq!(
            admit_fill(two, &inventory, &receive, &deliver, &schedule),
            Err(ScoringRefusal::NonCanonical)
        );
    }

    /// A price vector that is not a simplex at the rule's scale is refused
    /// before the schedule is ever computed.
    ///
    /// Distinct from `OffSchedule`, which is about a VALID simplex that is not
    /// this Dealer's price. This one is about a vector that is not a price
    /// vector at all -- the hostile the note's §5 puts first, and the one a
    /// hand-written candidate carries.
    #[test]
    fn a_price_vector_that_is_not_a_simplex_refuses_by_name() {
        let two = rule(2, 1 << 30);
        let inventory = vector(&[0, 1 << 20]);
        let schedule = prices_of(two, &inventory).expect("prices");

        let mut short = schedule;
        short[1] -= 1;
        assert_eq!(
            admit_fill(two, &inventory, &ZERO_VECTOR, &ZERO_VECTOR, &short),
            Err(ScoringRefusal::PricesNotSimplex)
        );

        let mut zeroed = ZERO_VECTOR;
        zeroed[0] = two.scale;
        assert_eq!(
            admit_fill(two, &inventory, &ZERO_VECTOR, &ZERO_VECTOR, &zeroed),
            Err(ScoringRefusal::PricesNotSimplex)
        );
    }

    /// `bounded_loss`, walked: the most damaging path a solver can build with
    /// admissible legs, with the Dealer's cash and inventory re-read at every
    /// boundary.
    ///
    /// The theorem says `deposit − wealth_i ≤ Ŝ` for every ordinary outcome
    /// `i` and every admissible path, where `wealth_i = cash + inv_i`. The
    /// damaging direction is to make the Dealer BUY, repeatedly, on outcomes
    /// it will not be paid on: each leg raises `Ŵ`, which is exactly what
    /// lets R3 admit a debit, and the scenario that never receives a claim is
    /// the one whose wealth falls. So the binding coordinate is outcome 0,
    /// which this path deliberately never touches -- and the assertion is that
    /// its wealth still never falls below `deposit − Ŝ`.
    ///
    /// The control is the last assertion: the path must actually have cost the
    /// sponsor most of the subsidy, or the walk proved nothing about a bound
    /// it never approached.
    #[test]
    fn the_sponsors_loss_never_exceeds_the_subsidy_on_an_adversarial_path() {
        let three = rule(3, 1 << 24);
        let subsidy = subsidy_of(three).expect("subsidy");
        let deposit = subsidy * 3;
        let mut cash = i128::from(deposit);
        let mut inventory = ZERO_VECTOR;
        let mut phi = cash + potential(three, &inventory).expect("W").value();
        let mut admitted_legs = 0_u32;

        // Outcome 0 is never received on, so it stays the minimum and R1 holds
        // for free; outcomes 1 and 2 absorb everything the solver can push.
        for step in 0..512_u64 {
            let outcome = 1 + usize::try_from(step % 2).expect("small");
            // One `b` of claims per leg: the schedule moves the log-odds by
            // exactly `q / b`, so this walks outcome 0's price toward the
            // floor in a few dozen legs, which is the only region where the
            // sponsor's whole subsidy is at risk.
            let quantity = three.liquidity / 4;
            let mut receive = ZERO_VECTOR;
            receive[outcome] = quantity;
            let mut next = inventory;
            next[outcome] += quantity;
            let schedule = prices_of(three, &next).expect("prices");
            let Ok(leg) = admit_fill(three, &inventory, &receive, &ZERO_VECTOR, &schedule) else {
                // The rule stopped covering the debit. That is R3 doing its
                // job, not the end of the walk's evidence.
                continue;
            };
            admitted_legs += 1;
            cash -= i128::from(leg.dealer_pays);
            cash += i128::from(leg.dealer_receives);
            inventory = leg.next_inventory;

            // `solvent`: an admissible fill never asks for credit.
            assert!(cash >= 0, "step {step}: the Dealer went short {cash}");
            // `potential_step` / `potential_life`: Φ does not fall.
            let phi_next = cash + leg.potential_after.value();
            assert!(phi_next >= phi, "step {step}: Φ fell {phi} -> {phi_next}");
            phi = phi_next;
            // `bounded_loss` in every scenario, at every boundary.
            for i in 0..3 {
                let wealth = cash + i128::from(inventory[i]);
                assert!(
                    i128::from(deposit) - wealth <= i128::from(subsidy),
                    "step {step}, outcome {i}: loss {} exceeds Ŝ {subsidy}",
                    i128::from(deposit) - wealth
                );
            }
        }

        assert!(
            admitted_legs >= 8,
            "the walk admitted only {admitted_legs} legs"
        );
        // The control. Measured here: the walk costs the sponsor 24,774,458 of
        // a 26,591,259 subsidy -- 93% -- and the 7% it does not reach is the
        // uniform-price shortfall the note's §3(a) names: the Dealer pays the
        // POST-fill price for the whole leg, which is strictly less than the
        // cost function's increment, and the difference is the sponsor's
        // income. A walk that stayed far below this number would be asserting
        // a bound it never approached.
        let worst = i128::from(deposit) - cash;
        assert!(
            worst > i128::from(subsidy) * 9 / 10,
            "the path cost the sponsor only {worst} of a {subsidy} subsidy, so the bound was never approached"
        );
    }
}
