//! The taker's side of the rule, host-only: invert `p̂` for a target price
//! and construct the fill the chain will admit.
//!
//! **No logarithm runs on chain.** The chain evaluates `Ê` and `L̂` and
//! compares; the inversion `inv′_i = b · log₂(p_max / p_i)` is the solver's
//! cost, and it is done here with the kernel's own `log2_ceil` over the price
//! ratio -- integers only -- then rounded to the lattice, so the solver never
//! asserts a price the kernel would not derive. The fill it returns carries
//! `p̂ := p̂(inv′)` exactly, which is why R2 passes at `τ = 0` for every fill
//! this solver constructs, and R3 passes because the Dealer's debit is what
//! the potential's increment covers up to one unit of rounding (`τ` lets the
//! solver shade by that unit toward the Dealer when it does not).

use super::generated::{MAX_OUTCOMES, ONE_Q62};
use super::{
    AdmittedFill, RuleParameters, ScoringRefusal, Vector, ZERO_VECTOR, admit_fill, log2_ceil,
    prices_of,
};

/// What the taker wants: to end the fill holding `target_prices` as the
/// Dealer's marginal price. Coordinates past the width are zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillTargetV1 {
    /// The price vector the taker wants the Dealer to quote after the fill,
    /// at the rule's scale; it need not be a simplex, the solver normalizes.
    pub target_prices: Vector,
}

/// A fill the chain will admit, with the accounting the taker needs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolvedFillV1 {
    /// Complete sets minted at par into the fill.
    pub mint: u64,
    /// `p̂(inv′)`.
    pub prices: Vector,
    /// The Dealer's receipt vector.
    pub receive: Vector,
    /// The Dealer's delivery vector.
    pub deliver: Vector,
    /// The post-fill inventory.
    pub next_inventory: Vector,
    /// The kernel's own verdict, carried so a caller reads the debit it will pay.
    pub admitted: AdmittedFill,
}

/// `⌊ b · log₂(numerator / denominator) ⌋` for `numerator ≥ denominator > 0`,
/// through the kernel's `L̂` (an upper bound, so the floor here is honest to
/// within the slack the kernel already carries).
fn scaled_log2_ratio(b: u64, numerator: u64, denominator: u64) -> Result<u64, ScoringRefusal> {
    if denominator == 0 || numerator < denominator {
        return Err(ScoringRefusal::PricesNotSimplex);
    }
    // `numerator · 2^62 / denominator ≥ 2^62`, and under `2^126`.
    let ratio = u128::from(numerator)
        .checked_mul(ONE_Q62)
        .ok_or(ScoringRefusal::Overflow)?
        / u128::from(denominator);
    let log = log2_ceil(ratio)?;
    let scaled = u128::from(b)
        .checked_mul(log)
        .ok_or(ScoringRefusal::Overflow)?
        / ONE_Q62;
    u64::try_from(scaled).map_err(|_| ScoringRefusal::Overflow)
}

/// The inventory at which the Dealer quotes `target`: `inv′_i = b · log₂(p_max/p_i)`
/// with the most expensive outcome at zero (normalized).
pub fn invert_prices(rule: RuleParameters, target: &Vector) -> Result<Vector, ScoringRefusal> {
    let width = rule.width();
    let mut p_max = 0_u64;
    for (index, price) in target.iter().enumerate() {
        if index >= width {
            if *price != 0 {
                return Err(ScoringRefusal::Width);
            }
        } else {
            if *price == 0 {
                return Err(ScoringRefusal::PricesNotSimplex);
            }
            p_max = p_max.max(*price);
        }
    }
    let mut inventory = ZERO_VECTOR;
    for (index, slot) in inventory.iter_mut().enumerate().take(width) {
        *slot = scaled_log2_ratio(rule.liquidity, p_max, target[index])?;
    }
    Ok(inventory)
}

/// Solve the fill that takes the Dealer from `inventory` to the inventory
/// quoting `target`, given the taker may supply `taker_holdings` of its own
/// claims (what it does not hold is minted at par).
///
/// The decomposition: with `inv′ = invert(target)`, the Dealer must end
/// holding `inv′`. Coordinates where `inv′_i > inv_i` are receipts; where
/// `inv′_i < inv_i`, deliveries. Receipts come from the taker's holdings
/// first and from a mint of `m = max_i (receive_i − holdings_i)` complete sets
/// otherwise, the taker keeping the rest of every minted set.
pub fn solve_fill(
    rule: RuleParameters,
    inventory: &Vector,
    taker_holdings: &Vector,
    target: FillTargetV1,
) -> Result<SolvedFillV1, ScoringRefusal> {
    let width = rule.width();
    let next = invert_prices(rule, &target.target_prices)?;
    let mut receive = ZERO_VECTOR;
    let mut deliver = ZERO_VECTOR;
    let mut mint = 0_u64;
    for index in 0..width {
        if next[index] > inventory[index] {
            receive[index] = next[index] - inventory[index];
            let short = receive[index].saturating_sub(taker_holdings[index]);
            mint = mint.max(short);
        } else {
            deliver[index] = inventory[index] - next[index];
        }
    }
    let prices = prices_of(rule, &next)?;
    let admitted = admit_fill(rule, inventory, &receive, &deliver, &prices)?;
    Ok(SolvedFillV1 {
        mint,
        prices,
        receive,
        deliver,
        next_inventory: admitted.next_inventory,
        admitted,
    })
}

/// The simplest taker: buy `claims` of `outcome` from a Dealer at inventory
/// `inventory`, minting what the Dealer does not hold. The taker ends up
/// holding `claims` of `outcome` more than before; the Dealer's inventory
/// moves so that its marginal price of `outcome` rises.
///
/// The fill is found by walking the lattice: the Dealer delivers from
/// `inventory[outcome]` first, and beyond that the taker mints `t` sets, of
/// which the Dealer keeps every coordinate but `outcome`. Both legs are one
/// `admit_fill`, and the solver returns the first admitted point.
pub fn solve_buy(
    rule: RuleParameters,
    inventory: &Vector,
    outcome: usize,
    claims: u64,
) -> Result<SolvedFillV1, ScoringRefusal> {
    let width = rule.width();
    if outcome >= width || outcome >= MAX_OUTCOMES {
        return Err(ScoringRefusal::Width);
    }
    let from_inventory = claims.min(inventory[outcome]);
    let minted = claims - from_inventory;
    let mut receive = ZERO_VECTOR;
    let mut deliver = ZERO_VECTOR;
    deliver[outcome] = from_inventory;
    for (index, slot) in receive.iter_mut().enumerate().take(width) {
        if index != outcome {
            *slot = minted;
        }
    }
    // R1: the post-fill inventory must have a zero coordinate. Delivering all
    // of `outcome` and receiving every other coordinate keeps the minimum at
    // `outcome` only if the Dealer held nothing else at zero; otherwise the
    // minimum is still zero at that coordinate. Either way normalized, unless
    // the Dealer still holds `outcome` after the delivery and every other
    // coordinate gained -- then the walk shifts one complete set out.
    let mut next = ZERO_VECTOR;
    let mut minimum = u64::MAX;
    for index in 0..width {
        next[index] = inventory[index] + receive[index] - deliver[index];
        minimum = minimum.min(next[index]);
    }
    if minimum > 0 {
        // Shift `minimum` complete sets to the taker: the Dealer delivers them
        // (par, `lmsrValue_shift`), which the canonical form expresses as a
        // smaller receipt on every coordinate and a larger delivery on `outcome`.
        for index in 0..width {
            if receive[index] >= minimum {
                receive[index] -= minimum;
            } else {
                deliver[index] += minimum - receive[index];
                receive[index] = 0;
            }
        }
        for index in 0..width {
            next[index] = inventory[index] + receive[index] - deliver[index];
        }
    }
    let prices = prices_of(rule, &next)?;
    let admitted = admit_fill(rule, inventory, &receive, &deliver, &prices)?;
    Ok(SolvedFillV1 {
        mint: minted,
        prices,
        receive,
        deliver,
        next_inventory: admitted.next_inventory,
        admitted,
    })
}

#[cfg(test)]
mod tests {
    use super::super::potential;
    use super::*;

    fn rule(k: u8, b: u64) -> RuleParameters {
        RuleParameters {
            outcome_count: k,
            liquidity: b,
            scale: 1 << 62,
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
    fn inverting_the_schedule_is_the_identity_on_the_lattice() {
        let two = rule(2, 1 << 20);
        for skew in [0_u64, 1 << 10, 1 << 18, 3 << 19] {
            let inventory = vector(&[0, skew]);
            let prices = prices_of(two, &inventory).expect("prices");
            let inverted = invert_prices(two, &prices).expect("invert");
            // Within one claim unit of rounding at every coordinate.
            assert!(inverted[0].abs_diff(inventory[0]) <= 1, "skew {skew}");
            assert!(inverted[1].abs_diff(inventory[1]) <= 1, "skew {skew}");
        }
    }

    #[test]
    fn a_buy_from_an_empty_dealer_mints_and_is_admitted() {
        let three = rule(3, 1 << 20);
        let inventory = ZERO_VECTOR;
        let solved = solve_buy(three, &inventory, 1, 4_096).expect("solved");
        assert_eq!(solved.mint, 4_096);
        assert_eq!(solved.deliver, ZERO_VECTOR);
        assert_eq!(solved.receive[..3], [4_096, 0, 4_096]);
        assert_eq!(solved.next_inventory[..3], [4_096, 0, 4_096]);
        // The Dealer pays for what it keeps; the potential rose by at least that.
        assert!(solved.admitted.dealer_pays > 0);
        let before = potential(three, &inventory).expect("W").value();
        let after = potential(three, &solved.next_inventory).expect("W").value();
        assert!(i128::from(solved.admitted.dealer_pays) <= after - before);
        // The price of the bought outcome rose.
        let flat = prices_of(three, &inventory).expect("flat");
        assert!(solved.prices[1] > flat[1]);
    }

    #[test]
    fn a_buy_from_inventory_delivers_before_it_mints() {
        let two = rule(2, 1 << 20);
        let inventory = vector(&[0, 10_000]);
        let solved = solve_buy(two, &inventory, 1, 4_000).expect("solved");
        assert_eq!(solved.mint, 0);
        assert_eq!(solved.deliver[..2], [0, 4_000]);
        assert_eq!(solved.next_inventory[..2], [0, 6_000]);
        assert!(solved.admitted.dealer_receives > 0);
    }

    #[test]
    fn a_target_price_is_reached_by_a_solved_fill() {
        let two = rule(2, 1 << 20);
        let inventory = vector(&[0, 1 << 16]);
        let mut target = ZERO_VECTOR;
        target[0] = 3 << 60;
        target[1] = 1 << 60;
        let solved = solve_fill(
            two,
            &inventory,
            &ZERO_VECTOR,
            FillTargetV1 {
                target_prices: target,
            },
        )
        .expect("solved");
        // The struck price is the schedule at the solved inventory, and it is
        // within the lattice's resolution of the target.
        let ratio_target = target[0] as f64 / target[1] as f64;
        let ratio_struck = solved.prices[0] as f64 / solved.prices[1] as f64;
        assert!((ratio_struck / ratio_target - 1.0).abs() < 1e-3);
    }
}
