//! Exact runtime-width junior pool-equity kernel for Dealer V3.
//!
//! Pool NAV is the sole scenario-residual vector
//! `E_s = collateral + Claims_s - obligations_s`. Equity shares are never a
//! par liability. The first cash-only contribution creates one share per cash
//! atom from a zero residual vector. Later issuance is admitted only when the
//! contributed scenario basket is exactly proportional to every prestate
//! residual coordinate. Redemption uses one named floor-rounding boundary,
//! returns the pro-rata scenario vector, and leaves all rounding dust in the
//! pool.

/// Named and sole equity-redemption rounding rule.
pub const POOL_EQUITY_REDEMPTION_ROUNDING_V3: &[u8] =
    b"floor(burned_shares * scenario_residual / total_shares)";

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
}

/// Result alias for pool-equity planning.
pub type PoolEquityResultV3<T> = core::result::Result<T, PoolEquityErrorV3>;

/// Exact scenario-basket contribution selected by an LP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolEquityContributionV3<'a> {
    /// Present collateral contributed to TradingPrincipal.
    pub collateral: u64,
    /// Native Claims contributed to the Dealer Position by scenario.
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
    /// Present eligible TradingPrincipal collateral.
    pub collateral: u64,
    /// Canonical Dealer Claims inventory.
    pub claims: &'a [u64],
    /// Canonical external terminal obligations; equity shares are excluded.
    pub obligations: &'a [u64],
    /// Outstanding canonical pool equity-share supply.
    pub total_shares: u64,
    /// Minimum residual required in every scenario after the transition.
    pub locked_capital_floor: u64,
    /// Selected contribution or redemption.
    pub action: PoolEquityActionV3<'a>,
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
    pub minimum_complete_sets_to_split: u64,
    /// Maximum complete sets merged after the Claims transfer.
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
                let transfer = payout.saturating_sub(plan.collateral_out);
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
            .saturating_add(candidate_claims)
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
                .checked_add(*claim)
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
                    .checked_add(*claim)
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
        .checked_add(maximum_merge)
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
        let claim_out = payout
            .checked_sub(collateral_out)
            .ok_or(PoolEquityErrorV3::Arithmetic)?;
        let current_claims = input
            .claims
            .get(index)
            .copied()
            .ok_or(PoolEquityErrorV3::WidthMismatch)?;
        minimum_split = minimum_split.max(claim_out.saturating_sub(current_claims));
    }
    let cash_needed = collateral_out
        .checked_add(minimum_split)
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
        let claim_out = payout
            .checked_sub(collateral_out)
            .ok_or(PoolEquityErrorV3::Arithmetic)?;
        let remaining_claims = input
            .claims
            .get(index)
            .copied()
            .ok_or(PoolEquityErrorV3::WidthMismatch)?
            .checked_add(minimum_split)
            .and_then(|value| value.checked_sub(claim_out))
            .ok_or(PoolEquityErrorV3::InsufficientAssets)?;
        maximum_merge = maximum_merge.min(remaining_claims);
        let after = before
            .checked_sub(payout)
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
        .checked_add(maximum_merge)
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
    input
        .collateral
        .checked_add(
            *input
                .claims
                .get(index)
                .ok_or(PoolEquityErrorV3::WidthMismatch)?,
        )
        .and_then(|value| value.checked_sub(input.obligations.get(index).copied()?))
        .ok_or(PoolEquityErrorV3::NegativeResidual)
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
