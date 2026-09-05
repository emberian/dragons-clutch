//! Exact runtime-width junior pool-equity kernel for Dealer V3.
//!
//! Pool NAV is the sole scenario-residual vector
//! `E_s = collateral + basis_scale * Claims_s - obligations_s`, in collateral
//! atoms. Collateral and obligations are atoms; the Claims inventory is native
//! claim units, and `basis_scale` is the authenticated
//! `ProductBasisV3::payout_scale` -- atoms per claim unit -- that converts one
//! into the other. Equity shares are never a par liability. The first cash-only contribution creates one share per cash
//! atom from a zero residual vector. Later issuance is admitted only when the
//! contributed scenario basket is exactly proportional to every prestate
//! residual coordinate. Redemption uses one named floor-rounding boundary,
//! returns the pro-rata scenario vector, and leaves all rounding dust in the
//! pool.

/// Named and sole equity-redemption rounding rule.
pub const POOL_EQUITY_REDEMPTION_ROUNDING_V3: &[u8] =
    b"floor(burned_shares * scenario_residual / total_shares)";

/// Named and sole claim-leg rounding rule for equity redemption.
///
/// The pro-rata payout is atoms; the cash leg is the uniform minimum across
/// scenarios; the remainder is delivered as native claim units, which exist
/// only in whole multiples of `basis_scale` atoms. Dust stays in the pool
/// exactly like the first boundary, so an LP is never paid above pro rata.
pub const POOL_EQUITY_CLAIM_LEG_ROUNDING_V3: &[u8] =
    b"floor((scenario_payout - collateral_out) / basis_scale)";

/// Stable refusal from the exact junior-equity boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolEquityErrorV3 {
    /// Runtime scenario slices were empty or had different widths.
    WidthMismatch,
    /// A share, collateral, Claim, or revision arithmetic operation failed.
    Arithmetic,
    /// Existing assets did not cover external obligations in one scenario.
    NegativeResidual,
    /// First issuance did not start from zero residual with cash-only 1:1 shares.
    InvalidFirstContribution,
    /// Later issuance would dilute or subsidize existing equity holders.
    DilutiveContribution,
    /// Burn exceeded the LP or global supply, or supply coordinates disagreed.
    InvalidShareSupply,
    /// Available collateral and Claims could not physically deliver redemption.
    InsufficientAssets,
    /// The candidate residual vector violated the immutable scenario floor.
    Insolvent,
    /// The authenticated payout scale was absent; there is no default of one.
    InvalidBasisScale,
}

/// Result alias for pool-equity planning.
pub type PoolEquityResultV3<T> = core::result::Result<T, PoolEquityErrorV3>;

/// Exact scenario-basket contribution selected by an LP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolEquityContributionV3<'a> {
    /// Present collateral contributed to TradingPrincipal.
    pub collateral: u64,
    /// Native Claims contributed to the Dealer Position by scenario, in claim
    /// units.
    pub claims: &'a [u64],
    /// Exact equity shares requested in exchange.
    pub minted_shares: u64,
}

/// Exact pro-rata redemption selected by an LP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolEquityRedemptionV3 {
    /// Exact equity shares burned.
    pub burned_shares: u64,
}

/// Selected junior-equity transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolEquityActionV3<'a> {
    /// Contribute an exactly proportional scenario basket and mint shares.
    Contribute(PoolEquityContributionV3<'a>),
    /// Burn shares and receive the floor-rounded pro-rata scenario residual.
    Redeem(PoolEquityRedemptionV3),
}

/// Borrowed authenticated pool state and selected transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolEquityInputV3<'a> {
    /// Present eligible TradingPrincipal collateral, in collateral atoms.
    pub collateral: u64,
    /// Canonical Dealer Claims inventory, in native claim units.
    pub claims: &'a [u64],
    /// Canonical external terminal obligations, in collateral atoms; equity
    /// shares are excluded.
    pub obligations: &'a [u64],
    /// Outstanding canonical pool equity-share supply.
    pub total_shares: u64,
    /// Minimum residual required in every scenario after the transition, in
    /// collateral atoms.
    pub locked_capital_floor: u64,
    /// Selected contribution or redemption.
    pub action: PoolEquityActionV3<'a>,
    /// Collateral atoms per native claim unit.
    ///
    /// This is the authenticated `ProductBasisV3::payout_scale`. It has one
    /// semantic owner -- the payoff basis -- and every consumer obtains it by
    /// authenticating that basis record against the market identity. Zero is
    /// refused, so no call site can reach the gate without stating it.
    pub basis_scale: u64,
}

/// Exact atomic pool-equity candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolEquityPlanV3 {
    /// Outstanding share supply before the action.
    pub shares_before: u64,
    /// Outstanding share supply after the action.
    pub shares_after: u64,
    /// Shares minted or burned by the action.
    pub share_delta: u64,
    /// Cash transferred from the LP into TradingPrincipal.
    pub collateral_in: u64,
    /// Cash transferred from TradingPrincipal to the LP.
    pub collateral_out: u64,
    /// Minimum complete sets split to make the outgoing Claims basket physical.
    ///
    /// A count of complete sets, never atoms: a Custody transfer funding this
    /// split moves `minimum_complete_sets_to_split * basis_scale` atoms.
    pub minimum_complete_sets_to_split: u64,
    /// Maximum complete sets merged after the Claims transfer.
    ///
    /// A count of complete sets, never atoms, on the same conversion.
    pub maximum_complete_sets_to_merge: u64,
    /// Exact TradingPrincipal balance after split/merge and LP transfer.
    pub collateral_after: u64,
    /// Minimum incoming scenario residual.
    pub minimum_residual_before: u64,
    /// First scenario attaining the incoming minimum.
    pub minimum_scenario_before: usize,
    /// Minimum candidate scenario residual.
    pub minimum_residual_after: u64,
    /// First scenario attaining the candidate minimum.
    pub minimum_scenario_after: usize,
}

/// Plan one exact contribution or redemption without mutating authority.
///
/// `residual_before`, `residual_after`, `claims_transferred`, and
/// `claims_after` remain byte-for-byte unchanged on every refusal. For a
/// contribution, `claims_transferred` moves LP→Dealer. For a redemption it
/// moves Dealer→LP.
pub fn plan_pool_equity_v3(
    input: PoolEquityInputV3<'_>,
    residual_before: &mut [u64],
    residual_after: &mut [u64],
    claims_transferred: &mut [u64],
    claims_after: &mut [u64],
) -> PoolEquityResultV3<PoolEquityPlanV3> {
    let width = input.claims.len();
    if width == 0
        || input.obligations.len() != width
        || residual_before.len() != width
        || residual_after.len() != width
        || claims_transferred.len() != width
        || claims_after.len() != width
    {
        return Err(PoolEquityErrorV3::WidthMismatch);
    }
    let plan = preflight_pool_equity_v3(input)?;

    // All candidate arithmetic and invariants were checked in the read-only
    // passes above. These exact operations cannot fail under those bounds.
    for index in 0..width {
        let before = residual_at(input, index).unwrap_or(0);
        let current_claims = input
            .claims
            .get(index)
            .copied()
            .ok_or(PoolEquityErrorV3::WidthMismatch)?;
        let obligation = input
            .obligations
            .get(index)
            .copied()
            .ok_or(PoolEquityErrorV3::WidthMismatch)?;
        let (transfer, candidate_claims) = match input.action {
            PoolEquityActionV3::Contribute(contribution) => {
                let transfer = contribution
                    .claims
                    .get(index)
                    .copied()
                    .ok_or(PoolEquityErrorV3::WidthMismatch)?;
                let preliminary = current_claims.saturating_add(transfer);
                (
                    transfer,
                    preliminary.saturating_sub(plan.maximum_complete_sets_to_merge),
                )
            }
            PoolEquityActionV3::Redeem(redemption) => {
                let payout = pro_rata_floor(before, redemption.burned_shares, input.total_shares)
                    .unwrap_or(0);
                let transfer =
                    claim_units_out(payout, plan.collateral_out, input.basis_scale).unwrap_or(0);
                let preliminary = current_claims
                    .saturating_add(plan.minimum_complete_sets_to_split)
                    .saturating_sub(transfer);
                (
                    transfer,
                    preliminary.saturating_sub(plan.maximum_complete_sets_to_merge),
                )
            }
        };
        *residual_before
            .get_mut(index)
            .ok_or(PoolEquityErrorV3::WidthMismatch)? = before;
        *residual_after
            .get_mut(index)
            .ok_or(PoolEquityErrorV3::WidthMismatch)? = plan
            .collateral_after
            .saturating_add(candidate_claims.saturating_mul(input.basis_scale))
            .saturating_sub(obligation);
        *claims_transferred
            .get_mut(index)
            .ok_or(PoolEquityErrorV3::WidthMismatch)? = transfer;
        *claims_after
            .get_mut(index)
            .ok_or(PoolEquityErrorV3::WidthMismatch)? = candidate_claims;
    }
    Ok(plan)
}

/// Read-only preflight used by operator construction and physical composition.
pub fn preflight_pool_equity_v3(
    input: PoolEquityInputV3<'_>,
) -> PoolEquityResultV3<PoolEquityPlanV3> {
    if input.claims.is_empty() || input.obligations.len() != input.claims.len() {
        return Err(PoolEquityErrorV3::WidthMismatch);
    }
    if input.basis_scale == 0 {
        return Err(PoolEquityErrorV3::InvalidBasisScale);
    }
    let (minimum_before, minimum_scenario_before) = minimum_residual(input)?;
    match input.action {
        PoolEquityActionV3::Contribute(contribution) => {
            plan_contribution(input, contribution, minimum_before, minimum_scenario_before)
        }
        PoolEquityActionV3::Redeem(redemption) => {
            plan_redemption(input, redemption, minimum_before, minimum_scenario_before)
        }
    }
}

fn plan_contribution(
    input: PoolEquityInputV3<'_>,
    contribution: PoolEquityContributionV3<'_>,
    minimum_before: u64,
    minimum_scenario_before: usize,
) -> PoolEquityResultV3<PoolEquityPlanV3> {
    if contribution.claims.len() != input.claims.len() || contribution.minted_shares == 0 {
        return Err(PoolEquityErrorV3::WidthMismatch);
    }
    if input.total_shares == 0 {
        if contribution.collateral == 0
            || contribution.minted_shares != contribution.collateral
            || contribution.claims.iter().any(|value| *value != 0)
            || (0..input.claims.len()).any(|index| residual_at(input, index) != Ok(0))
        {
            return Err(PoolEquityErrorV3::InvalidFirstContribution);
        }
    } else {
        let mut any_contribution = false;
        for (index, claim) in contribution.claims.iter().enumerate() {
            let scenario_contribution = contribution
                .collateral
                .checked_add(claims_atoms(*claim, input.basis_scale)?)
                .ok_or(PoolEquityErrorV3::Arithmetic)?;
            any_contribution |= scenario_contribution != 0;
            let residual = residual_at(input, index)?;
            let left = u128::from(scenario_contribution)
                .checked_mul(u128::from(input.total_shares))
                .ok_or(PoolEquityErrorV3::Arithmetic)?;
            let right = u128::from(contribution.minted_shares)
                .checked_mul(u128::from(residual))
                .ok_or(PoolEquityErrorV3::Arithmetic)?;
            if left != right {
                return Err(PoolEquityErrorV3::DilutiveContribution);
            }
        }
        if !any_contribution {
            return Err(PoolEquityErrorV3::DilutiveContribution);
        }
    }

    let preliminary_collateral = input
        .collateral
        .checked_add(contribution.collateral)
        .ok_or(PoolEquityErrorV3::Arithmetic)?;
    let mut maximum_merge = u64::MAX;
    let mut minimum_after = u64::MAX;
    let mut minimum_scenario_after = 0;
    for (index, claim) in contribution.claims.iter().enumerate() {
        let preliminary_claims = input
            .claims
            .get(index)
            .copied()
            .ok_or(PoolEquityErrorV3::WidthMismatch)?
            .checked_add(*claim)
            .ok_or(PoolEquityErrorV3::Arithmetic)?;
        maximum_merge = maximum_merge.min(preliminary_claims);
        let after = residual_at(input, index)?
            .checked_add(
                contribution
                    .collateral
                    .checked_add(claims_atoms(*claim, input.basis_scale)?)
                    .ok_or(PoolEquityErrorV3::Arithmetic)?,
            )
            .ok_or(PoolEquityErrorV3::Arithmetic)?;
        if after < input.locked_capital_floor {
            return Err(PoolEquityErrorV3::Insolvent);
        }
        if after < minimum_after {
            minimum_after = after;
            minimum_scenario_after = index;
        }
    }
    let collateral_after = preliminary_collateral
        .checked_add(claims_atoms(maximum_merge, input.basis_scale)?)
        .ok_or(PoolEquityErrorV3::Arithmetic)?;
    Ok(PoolEquityPlanV3 {
        shares_before: input.total_shares,
        shares_after: input
            .total_shares
            .checked_add(contribution.minted_shares)
            .ok_or(PoolEquityErrorV3::Arithmetic)?,
        share_delta: contribution.minted_shares,
        collateral_in: contribution.collateral,
        collateral_out: 0,
        minimum_complete_sets_to_split: 0,
        maximum_complete_sets_to_merge: maximum_merge,
        collateral_after,
        minimum_residual_before: minimum_before,
        minimum_scenario_before,
        minimum_residual_after: minimum_after,
        minimum_scenario_after,
    })
}

fn plan_redemption(
    input: PoolEquityInputV3<'_>,
    redemption: PoolEquityRedemptionV3,
    minimum_before: u64,
    minimum_scenario_before: usize,
) -> PoolEquityResultV3<PoolEquityPlanV3> {
    if input.total_shares == 0
        || redemption.burned_shares == 0
        || redemption.burned_shares > input.total_shares
    {
        return Err(PoolEquityErrorV3::InvalidShareSupply);
    }
    let mut collateral_out = u64::MAX;
    for index in 0..input.claims.len() {
        let payout = pro_rata_floor(
            residual_at(input, index)?,
            redemption.burned_shares,
            input.total_shares,
        )?;
        collateral_out = collateral_out.min(payout);
    }

    let mut minimum_split = 0_u64;
    for index in 0..input.claims.len() {
        let payout = pro_rata_floor(
            residual_at(input, index)?,
            redemption.burned_shares,
            input.total_shares,
        )?;
        let claim_out = claim_units_out(payout, collateral_out, input.basis_scale)?;
        let current_claims = input
            .claims
            .get(index)
            .copied()
            .ok_or(PoolEquityErrorV3::WidthMismatch)?;
        minimum_split = minimum_split.max(claim_out.saturating_sub(current_claims));
    }
    // Splitting one complete set consumes `basis_scale` atoms of collateral,
    // not one. Funding the split with the set COUNT is the direction that
    // under-collateralizes the Hoard.
    let cash_needed = collateral_out
        .checked_add(claims_atoms(minimum_split, input.basis_scale)?)
        .ok_or(PoolEquityErrorV3::Arithmetic)?;
    let collateral_after_split = input
        .collateral
        .checked_sub(cash_needed)
        .ok_or(PoolEquityErrorV3::InsufficientAssets)?;
    let mut maximum_merge = u64::MAX;
    let mut minimum_after = u64::MAX;
    let mut minimum_scenario_after = 0;
    for index in 0..input.claims.len() {
        let before = residual_at(input, index)?;
        let payout = pro_rata_floor(before, redemption.burned_shares, input.total_shares)?;
        let claim_out = claim_units_out(payout, collateral_out, input.basis_scale)?;
        let remaining_claims = input
            .claims
            .get(index)
            .copied()
            .ok_or(PoolEquityErrorV3::WidthMismatch)?
            .checked_add(minimum_split)
            .and_then(|value| value.checked_sub(claim_out))
            .ok_or(PoolEquityErrorV3::InsufficientAssets)?;
        maximum_merge = maximum_merge.min(remaining_claims);
        // What the LP is actually handed is the cash leg plus whole claim
        // units, which is the pro-rata payout minus its claim-leg rounding
        // dust. At `basis_scale` one this is exactly `before - payout`.
        let after = before
            .checked_sub(collateral_out)
            .ok_or(PoolEquityErrorV3::Arithmetic)?
            .checked_sub(claims_atoms(claim_out, input.basis_scale)?)
            .ok_or(PoolEquityErrorV3::Arithmetic)?;
        if after < input.locked_capital_floor {
            return Err(PoolEquityErrorV3::Insolvent);
        }
        if after < minimum_after {
            minimum_after = after;
            minimum_scenario_after = index;
        }
    }
    let collateral_after = collateral_after_split
        .checked_add(claims_atoms(maximum_merge, input.basis_scale)?)
        .ok_or(PoolEquityErrorV3::Arithmetic)?;
    Ok(PoolEquityPlanV3 {
        shares_before: input.total_shares,
        shares_after: input
            .total_shares
            .checked_sub(redemption.burned_shares)
            .ok_or(PoolEquityErrorV3::InvalidShareSupply)?,
        share_delta: redemption.burned_shares,
        collateral_in: 0,
        collateral_out,
        minimum_complete_sets_to_split: minimum_split,
        maximum_complete_sets_to_merge: maximum_merge,
        collateral_after,
        minimum_residual_before: minimum_before,
        minimum_scenario_before,
        minimum_residual_after: minimum_after,
        minimum_scenario_after,
    })
}

fn minimum_residual(input: PoolEquityInputV3<'_>) -> PoolEquityResultV3<(u64, usize)> {
    let mut minimum = u64::MAX;
    let mut scenario = 0;
    for index in 0..input.claims.len() {
        let value = residual_at(input, index)?;
        if value < minimum {
            minimum = value;
            scenario = index;
        }
    }
    Ok((minimum, scenario))
}

fn residual_at(input: PoolEquityInputV3<'_>, index: usize) -> PoolEquityResultV3<u64> {
    let claims = *input
        .claims
        .get(index)
        .ok_or(PoolEquityErrorV3::WidthMismatch)?;
    let obligation = *input
        .obligations
        .get(index)
        .ok_or(PoolEquityErrorV3::WidthMismatch)?;
    input
        .collateral
        .checked_add(claims_atoms(claims, input.basis_scale)?)
        .and_then(|value| value.checked_sub(obligation))
        .ok_or(PoolEquityErrorV3::NegativeResidual)
}

/// Convert native claim units into collateral atoms at the authenticated scale.
///
/// A zero scale is refused rather than defaulted: the whole defect this
/// function exists to close is a scale nothing states behaving as one.
fn claims_atoms(units: u64, basis_scale: u64) -> PoolEquityResultV3<u64> {
    if basis_scale == 0 {
        return Err(PoolEquityErrorV3::InvalidBasisScale);
    }
    units
        .checked_mul(basis_scale)
        .ok_or(PoolEquityErrorV3::Arithmetic)
}

/// Split one scenario's pro-rata atom payout into its whole claim-unit leg.
///
/// `POOL_EQUITY_CLAIM_LEG_ROUNDING_V3` is the boundary applied here.
fn claim_units_out(payout: u64, collateral_out: u64, basis_scale: u64) -> PoolEquityResultV3<u64> {
    if basis_scale == 0 {
        return Err(PoolEquityErrorV3::InvalidBasisScale);
    }
    Ok(payout
        .checked_sub(collateral_out)
        .ok_or(PoolEquityErrorV3::Arithmetic)?
        / basis_scale)
}

fn pro_rata_floor(residual: u64, burn: u64, supply: u64) -> PoolEquityResultV3<u64> {
    if supply == 0 || burn > supply {
        return Err(PoolEquityErrorV3::InvalidShareSupply);
    }
    let quotient = u128::from(residual)
        .checked_mul(u128::from(burn))
        .ok_or(PoolEquityErrorV3::Arithmetic)?
        / u128::from(supply);
    u64::try_from(quotient).map_err(|_| PoolEquityErrorV3::Arithmetic)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The solvency gate no longer adds atoms to claim units.
    ///
    /// Until `basis_scale` landed, `residual_at` computed
    /// `collateral + claims[s] - obligations[s]` in one `u64` and that scalar
    /// was the SOLE `Insolvent` gate, legal only if one claim unit IS one atom.
    /// Nothing pinned that, and every in-tree fixture used one, which is the
    /// only reason it was invisible.
    ///
    /// The property asserted here is the weakest one a solvency gate must have:
    /// its verdict is a fact about the POSITION, not about the units the claim
    /// leg happens to be denominated in. Each case below describes one physical
    /// pool twice and demands one answer.
    ///
    /// The ambiguity the red version named -- whether `obligations` is atom- or
    /// claim-denominated, which the type did not say -- is now settled by the
    /// type, and settled the way `dclutch-trading::dealer_scenario` already
    /// documented it: obligations are collateral atoms, collateral is atoms,
    /// and ONLY the Claims inventory is claim units. So the redescriptions
    /// below hold `obligations` fixed and move only the claim leg.
    #[test]
    fn the_solvency_gate_must_not_change_its_verdict_with_the_claim_unit() {
        // Redeem a real slice: a zero burn refuses `InvalidShareSupply` before
        // the solvency gate, and an earlier draft of this test compared two such
        // refusals and passed while proving nothing.
        let redeem = PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares: 10 });

        // One pool described twice. The claim leg is worth 20 atoms either way:
        // 20 units at scale 1, or 10 units at scale 2.
        // Collateral is ample so the pro-rata payout is physically deliverable
        // and the run reaches the solvency gate rather than `InsufficientAssets`.
        let at_scale_one = preflight_pool_equity_v3(PoolEquityInputV3 {
            collateral: 100,
            claims: &[20],
            obligations: &[0],
            total_shares: 100,
            locked_capital_floor: 100,
            action: redeem,
            basis_scale: 1,
        });
        let at_scale_two = preflight_pool_equity_v3(PoolEquityInputV3 {
            collateral: 100,
            claims: &[10],
            obligations: &[0],
            total_shares: 100,
            locked_capital_floor: 100,
            action: redeem,
            basis_scale: 2,
        });
        reached_the_gate("scale one", at_scale_one);
        reached_the_gate("scale two", at_scale_two);
        assert_eq!(
            at_scale_one.is_ok(),
            at_scale_two.is_ok(),
            "the same 20 atoms of claim value must give one solvency verdict, \
             but the gate reads the unit count: {at_scale_one:?} vs {at_scale_two:?}"
        );

        // A live obligation, and a floor between the two answers so the
        // verdict actually flips if the claim leg is read as atoms. True
        // residual 40 + 20 - 30 = 30 clears a floor of 25 after a payout of 3;
        // reading 10 claim units as 10 atoms gives 20, a payout of 2, and a
        // candidate of 18 -- `Insolvent` on a pool that is solvent. The pool is
        // refused a redemption it can physically make, and every payout it does
        // make is sized off the wrong residual.
        let truth = preflight_pool_equity_v3(PoolEquityInputV3 {
            collateral: 40,
            claims: &[20],
            obligations: &[30],
            total_shares: 100,
            locked_capital_floor: 25,
            action: redeem,
            basis_scale: 1,
        });
        let same_pool_at_scale_two = preflight_pool_equity_v3(PoolEquityInputV3 {
            collateral: 40,
            claims: &[10],
            obligations: &[30],
            total_shares: 100,
            locked_capital_floor: 25,
            action: redeem,
            basis_scale: 2,
        });
        reached_the_gate("atoms", truth);
        reached_the_gate("scale two", same_pool_at_scale_two);
        assert_eq!(
            truth.is_ok(),
            same_pool_at_scale_two.is_ok(),
            "one pool, one verdict: {truth:?} vs {same_pool_at_scale_two:?}"
        );
        assert_eq!(
            truth.map(|plan| plan.collateral_out),
            same_pool_at_scale_two.map(|plan| plan.collateral_out),
            "one pool, one payout"
        );
    }

    /// One pool, described at `basis_scale` 1 and at 97.
    ///
    /// 97 is prime, coprime to every other number in the fixture, and not a
    /// power of two, so neither a coincidence of the residual arithmetic nor a
    /// shift standing in for the multiply can make the two descriptions agree
    /// by accident. Width is two so the claim leg is genuinely exercised:
    /// scenario 1 is paid partly in claims, not only in cash.
    #[test]
    fn one_pool_gives_one_plan_at_scale_one_and_at_scale_ninety_seven() {
        let redeem = PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares: 10 });
        let pool = |claims: &'static [u64], basis_scale, locked_capital_floor| {
            preflight_pool_equity_v3(PoolEquityInputV3 {
                collateral: 970,
                claims,
                obligations: &[0, 0],
                total_shares: 100,
                locked_capital_floor,
                action: redeem,
                basis_scale,
            })
        };
        // 1940 and 3880 atoms of claim value, written both ways.
        let at_ninety_seven = pool(&[20, 40], 97, 2000);
        let at_one = pool(&[1940, 3880], 1, 2000);
        reached_the_gate("scale ninety-seven", at_ninety_seven);
        reached_the_gate("scale one", at_one);

        let wide = at_ninety_seven.expect("the pool is solvent at 97");
        let narrow = at_one.expect("and at 1, because it is the same pool");
        assert_eq!(
            (
                wide.minimum_residual_before,
                wide.minimum_residual_after,
                wide.collateral_out,
                wide.collateral_after,
                wide.shares_after,
            ),
            (2910, 2619, 291, 2619, 90),
            "the atom-denominated plan, stated once and not re-derived"
        );
        assert_eq!(
            (
                narrow.minimum_residual_before,
                narrow.minimum_residual_after,
                narrow.collateral_out,
                narrow.collateral_after,
                narrow.shares_after,
            ),
            (
                wide.minimum_residual_before,
                wide.minimum_residual_after,
                wide.collateral_out,
                wide.collateral_after,
                wide.shares_after,
            ),
            "one pool, one plan in atoms"
        );
        // Set COUNTS are the one thing that legitimately differs, and they
        // differ by exactly the scale -- which is what makes the Custody
        // transfer `count * basis_scale` and not `count`.
        assert_eq!(wide.maximum_complete_sets_to_merge, 20);
        assert_eq!(narrow.maximum_complete_sets_to_merge, 1940);
        assert_eq!(
            wide.maximum_complete_sets_to_merge * 97,
            narrow.maximum_complete_sets_to_merge * 1
        );

        // Teeth: raise the floor one atom above the candidate residual and both
        // descriptions must refuse. A gate that cannot say no is not a gate.
        assert_eq!(pool(&[20, 40], 97, 2620), Err(PoolEquityErrorV3::Insolvent));
        assert_eq!(
            pool(&[1940, 3880], 1, 2620),
            Err(PoolEquityErrorV3::Insolvent)
        );
        assert!(pool(&[20, 40], 97, 2619).is_ok(), "the bound is exact");
    }

    /// A scale nobody stated is refused, not defaulted to one.
    #[test]
    fn an_unstated_payout_scale_is_refused_rather_than_assumed() {
        assert_eq!(
            preflight_pool_equity_v3(PoolEquityInputV3 {
                collateral: 100,
                claims: &[20],
                obligations: &[0],
                total_shares: 100,
                locked_capital_floor: 0,
                action: PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares: 10 }),
                basis_scale: 0,
            }),
            Err(PoolEquityErrorV3::InvalidBasisScale)
        );
    }

    /// Refuse to draw a conclusion from a refusal that never reached the gate.
    ///
    /// Without this, two `InvalidShareSupply`s compare equal and the invariance
    /// above "holds" vacuously. An absent signal is evidence only once something
    /// present proves the channel works.
    fn reached_the_gate(label: &str, outcome: PoolEquityResultV3<PoolEquityPlanV3>) {
        assert!(
            matches!(outcome, Ok(_) | Err(PoolEquityErrorV3::Insolvent)),
            "{label}: this case must reach the solvency gate for the comparison \
             to mean anything, got {outcome:?}"
        );
    }

    #[test]
    fn first_cash_contribution_creates_real_risk_capacity() {
        let mut before = [99; 3];
        let mut after = [99; 3];
        let mut transfer = [99; 3];
        let mut claims_after = [99; 3];
        let plan = plan_pool_equity_v3(
            PoolEquityInputV3 {
                collateral: 0,
                claims: &[0, 0, 0],
                obligations: &[0, 0, 0],
                total_shares: 0,
                locked_capital_floor: 5,
                action: PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                    collateral: 10,
                    claims: &[0, 0, 0],
                    minted_shares: 10,
                }),
                basis_scale: 1,
            },
            &mut before,
            &mut after,
            &mut transfer,
            &mut claims_after,
        )
        .expect("first capital creates flat residual");
        assert_eq!(before, [0, 0, 0]);
        assert_eq!(after, [10, 10, 10]);
        assert_eq!(plan.shares_after, 10);
        assert_eq!(plan.collateral_after, 10);
    }

    #[test]
    fn proportional_basket_issues_without_dilution_and_merges_sets() {
        let mut before = [0; 3];
        let mut after = [0; 3];
        let mut transfer = [0; 3];
        let mut claims_after = [0; 3];
        let plan = plan_pool_equity_v3(
            PoolEquityInputV3 {
                collateral: 10,
                claims: &[0, 10, 20],
                obligations: &[0, 0, 0],
                total_shares: 10,
                locked_capital_floor: 0,
                action: PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                    collateral: 5,
                    claims: &[0, 5, 10],
                    minted_shares: 5,
                }),
                basis_scale: 1,
            },
            &mut before,
            &mut after,
            &mut transfer,
            &mut claims_after,
        )
        .expect("exact half-size scenario basket");
        assert_eq!(before, [10, 20, 30]);
        assert_eq!(after, [15, 30, 45]);
        assert_eq!(transfer, [0, 5, 10]);
        assert_eq!(claims_after, [0, 15, 30]);
        assert_eq!(plan.shares_after, 15);

        let mut untouched = [77; 3];
        assert_eq!(
            plan_pool_equity_v3(
                PoolEquityInputV3 {
                    collateral: 10,
                    claims: &[0, 10, 20],
                    obligations: &[0, 0, 0],
                    total_shares: 10,
                    locked_capital_floor: 0,
                    action: PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                        collateral: 5,
                        claims: &[0, 4, 10],
                        minted_shares: 5,
                    }),
                    basis_scale: 1,
                },
                &mut untouched,
                &mut [77; 3],
                &mut [77; 3],
                &mut [77; 3],
            ),
            Err(PoolEquityErrorV3::DilutiveContribution)
        );
        assert_eq!(untouched, [77; 3]);
    }

    #[test]
    fn redemption_floor_rounds_once_and_leaves_dust() {
        let mut before = [0; 3];
        let mut after = [0; 3];
        let mut transfer = [0; 3];
        let mut claims_after = [0; 3];
        let plan = plan_pool_equity_v3(
            PoolEquityInputV3 {
                collateral: 2,
                claims: &[0, 3, 6],
                obligations: &[0, 0, 0],
                total_shares: 3,
                locked_capital_floor: 0,
                action: PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares: 1 }),
                basis_scale: 1,
            },
            &mut before,
            &mut after,
            &mut transfer,
            &mut claims_after,
        )
        .expect("one-third residual redemption");
        assert_eq!(before, [2, 5, 8]);
        assert_eq!(transfer, [0, 1, 2]);
        assert_eq!(after, [2, 4, 6]);
        assert_eq!(claims_after, [0, 2, 4]);
        assert_eq!(plan.collateral_out, 0);
        assert_eq!(plan.shares_after, 2);
        assert_eq!(
            POOL_EQUITY_REDEMPTION_ROUNDING_V3,
            b"floor(burned_shares * scenario_residual / total_shares)"
        );
    }

    #[test]
    fn floor_and_physical_assets_refuse_atomically() {
        let input = PoolEquityInputV3 {
            collateral: 5,
            claims: &[0, 0],
            obligations: &[0, 4],
            total_shares: 5,
            locked_capital_floor: 1,
            action: PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares: 5 }),
            basis_scale: 1,
        };
        let mut before = [44; 2];
        let mut after = [44; 2];
        let mut transfer = [44; 2];
        let mut claims_after = [44; 2];
        assert_eq!(
            plan_pool_equity_v3(
                input,
                &mut before,
                &mut after,
                &mut transfer,
                &mut claims_after,
            ),
            Err(PoolEquityErrorV3::Insolvent)
        );
        assert_eq!(before, [44; 2]);
        assert_eq!(after, [44; 2]);
        assert_eq!(transfer, [44; 2]);
        assert_eq!(claims_after, [44; 2]);
    }
}
