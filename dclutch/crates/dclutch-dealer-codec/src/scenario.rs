//! Runtime-width finite-scenario collateral planning for Dealer V2.
//!
//! This module never creates signed native Positions or persists a parallel
//! liability vector. Inputs are projections of canonical Dealer Claims
//! inventory plus the two nonnegative portfolio legs executed in the same
//! atomic transaction. A shortfall is covered by depositing present Dealer
//! capital into the Market Hoard and minting equal complete sets. An optional
//! release is bounded by the equal residual claims that Claims can merge.

/// Stable refusal from scenario-collateral planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioError {
    /// No Product scenario was supplied.
    EmptyBook,
    /// Inventory, incoming, outgoing, and output widths differed.
    WidthMismatch,
    /// Canonical Claims Position Market or holder identity did not join policy.
    PositionMismatch,
    /// Canonical Claims Position revision did not match the invocation.
    StalePosition,
    /// Checked scenario arithmetic exceeded `u64`.
    ArithmeticOverflow,
    /// Present Dealer capital did not fund the required Hoard deposit.
    UnderfundedReserve,
    /// Requested Hoard release exceeded equal mergeable complete sets.
    ExcessiveRelease,
}

/// Ephemeral projection of the one canonical Dealer Claims Position.
///
/// The adapter constructs this value only through canonical Claims getters;
/// it is not another persisted inventory DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsPositionObservation<'a> {
    /// Logical Core Market referenced by the Claims Position.
    pub market_id: [u8; 32],
    /// Exact Dealer holder identity.
    pub owner: [u8; 32],
    /// Current canonical Claims Position revision.
    pub revision: u64,
    /// Runtime-width native Claims quantities.
    pub native: &'a [u64],
}

/// Runtime-width canonical Claims projections for one atomic portfolio fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioBook<'a> {
    /// Ephemeral canonical Dealer Claims Position projection.
    pub position: ClaimsPositionObservation<'a>,
    /// Immutable policy Market expected by this Dealer capability.
    pub expected_market_id: [u8; 32],
    /// Immutable Dealer holder expected by this capability.
    pub expected_dealer_id: [u8; 32],
    /// Optimistic Claims Position revision bound by the request.
    pub expected_position_revision: u64,
    /// Native Claims acquired from the counterparty in this transaction.
    pub acquired: &'a [u64],
    /// Native Claims delivered to the counterparty in this transaction.
    pub delivered: &'a [u64],
    /// Present Dealer quote capital available for new Hoard principal.
    pub present_reserve_funding: u64,
    /// Equal residual complete sets requested for merge and Hoard release.
    pub requested_release: u64,
}

/// Exact complete-set and collateral plan for one admitted portfolio fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioPlan {
    /// Present collateral moved from Dealer TradingPrincipal to Market Hoard.
    pub reserve_to_hoard: u64,
    /// Equal complete sets minted by Claims into the Dealer Position.
    pub complete_sets_to_mint: u64,
    /// Equal complete sets merged by Claims after portfolio transfers.
    pub complete_sets_to_merge: u64,
    /// Hoard principal returned to Dealer TradingPrincipal after the merge.
    pub release_from_hoard: u64,
    /// Maximum equal complete sets available before the requested merge.
    pub maximum_mergeable: u64,
}

/// Plan one exact finite-scenario portfolio fill without allocation.
///
/// The function validates the complete candidate state in a first pass and
/// mutates `post_inventory` only after every width, arithmetic, present-funding,
/// and merge bound succeeds. Each output coordinate is therefore nonnegative.
/// Claims must execute the nonnegative acquire/deliver vectors and equal-set
/// mint/merge; Custody must execute the two named Hoard transfers. Their own
/// canonical programs remain responsible for supply and principal authority.
pub fn plan_scenario_netting(
    book: ScenarioBook<'_>,
    post_inventory: &mut [u64],
) -> Result<ScenarioPlan, ScenarioError> {
    if book.expected_market_id == [0; 32]
        || book.expected_dealer_id == [0; 32]
        || book.position.market_id != book.expected_market_id
        || book.position.owner != book.expected_dealer_id
    {
        return Err(ScenarioError::PositionMismatch);
    }
    if book.position.revision != book.expected_position_revision {
        return Err(ScenarioError::StalePosition);
    }
    let width = book.position.native.len();
    if width == 0 {
        return Err(ScenarioError::EmptyBook);
    }
    if book.acquired.len() != width
        || book.delivered.len() != width
        || post_inventory.len() != width
    {
        return Err(ScenarioError::WidthMismatch);
    }

    let mut reserve = 0_u64;
    for ((inventory, acquired), delivered) in book
        .position
        .native
        .iter()
        .zip(book.acquired.iter())
        .zip(book.delivered.iter())
    {
        let available = inventory
            .checked_add(*acquired)
            .ok_or(ScenarioError::ArithmeticOverflow)?;
        reserve = reserve.max(delivered.saturating_sub(available));
    }
    if reserve > book.present_reserve_funding {
        return Err(ScenarioError::UnderfundedReserve);
    }

    let mut maximum_mergeable = u64::MAX;
    for ((inventory, acquired), delivered) in book
        .position
        .native
        .iter()
        .zip(book.acquired.iter())
        .zip(book.delivered.iter())
    {
        let available = inventory
            .checked_add(*acquired)
            .and_then(|value| value.checked_add(reserve))
            .ok_or(ScenarioError::ArithmeticOverflow)?;
        let funded = available
            .checked_sub(*delivered)
            .ok_or(ScenarioError::ArithmeticOverflow)?;
        maximum_mergeable = maximum_mergeable.min(funded);
    }
    if book.requested_release > maximum_mergeable {
        return Err(ScenarioError::ExcessiveRelease);
    }

    for (((inventory, acquired), delivered), post) in book
        .position
        .native
        .iter()
        .zip(book.acquired.iter())
        .zip(book.delivered.iter())
        .zip(post_inventory.iter_mut())
    {
        *post = inventory
            .checked_add(*acquired)
            .and_then(|value| value.checked_add(reserve))
            .and_then(|value| value.checked_sub(*delivered))
            .and_then(|value| value.checked_sub(book.requested_release))
            .ok_or(ScenarioError::ArithmeticOverflow)?;
    }

    Ok(ScenarioPlan {
        reserve_to_hoard: reserve,
        complete_sets_to_mint: reserve,
        complete_sets_to_merge: book.requested_release,
        release_from_hoard: book.requested_release,
        maximum_mergeable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKET: [u8; 32] = [1; 32];
    const DEALER: [u8; 32] = [2; 32];
    const REVISION: u64 = 7;

    const fn observed(native: &[u64]) -> ClaimsPositionObservation<'_> {
        ClaimsPositionObservation {
            market_id: MARKET,
            owner: DEALER,
            revision: REVISION,
            native,
        }
    }

    const fn book<'a>(
        inventory: &'a [u64],
        acquired: &'a [u64],
        delivered: &'a [u64],
        present_reserve_funding: u64,
        requested_release: u64,
    ) -> ScenarioBook<'a> {
        ScenarioBook {
            position: observed(inventory),
            expected_market_id: MARKET,
            expected_dealer_id: DEALER,
            expected_position_revision: REVISION,
            acquired,
            delivered,
            present_reserve_funding,
            requested_release,
        }
    }

    #[test]
    fn portfolio_netting_funds_only_the_maximum_terminal_shortfall() {
        let inventory = [2, 10, 0];
        let acquired = [3, 0, 4];
        let delivered = [10, 1, 6];
        let mut post = [u64::MAX; 3];
        let plan = plan_scenario_netting(book(&inventory, &acquired, &delivered, 5, 0), &mut post)
            .expect("present reserve covers every terminal scenario");
        assert_eq!(plan.reserve_to_hoard, 5);
        assert_eq!(plan.complete_sets_to_mint, 5);
        assert_eq!(plan.maximum_mergeable, 0);
        assert_eq!(post, [0, 14, 3]);
        for index in 0..post.len() {
            assert_eq!(
                post[index] + delivered[index],
                inventory[index] + acquired[index] + plan.complete_sets_to_mint
            );
        }
    }

    #[test]
    fn equal_residual_complete_sets_are_the_only_releasable_principal() {
        let inventory = [9, 8, 10];
        let acquired = [3, 4, 0];
        let delivered = [2, 1, 0];
        let mut post = [0; 3];
        let plan = plan_scenario_netting(book(&inventory, &acquired, &delivered, 0, 10), &mut post)
            .expect("ten complete sets remain in every scenario");
        assert_eq!(plan.reserve_to_hoard, 0);
        assert_eq!(plan.maximum_mergeable, 10);
        assert_eq!(plan.complete_sets_to_merge, 10);
        assert_eq!(plan.release_from_hoard, 10);
        assert_eq!(post, [0, 1, 0]);
    }

    #[test]
    fn hostile_width_funding_overflow_and_release_refuse_before_mutation() {
        let unchanged = [0xa5_u64; 3];
        let mut post = unchanged;
        assert_eq!(
            plan_scenario_netting(book(&[1, 2, 3], &[0, 0], &[0, 0, 0], 0, 0), &mut post,),
            Err(ScenarioError::WidthMismatch)
        );
        assert_eq!(post, unchanged);

        assert_eq!(
            plan_scenario_netting(book(&[0, 0, 0], &[0, 0, 0], &[5, 2, 1], 4, 0), &mut post,),
            Err(ScenarioError::UnderfundedReserve)
        );
        assert_eq!(post, unchanged);

        assert_eq!(
            plan_scenario_netting(
                book(&[u64::MAX, 0, 0], &[1, 0, 0], &[0, 0, 0], 0, 0),
                &mut post,
            ),
            Err(ScenarioError::ArithmeticOverflow)
        );
        assert_eq!(post, unchanged);

        assert_eq!(
            plan_scenario_netting(book(&[9, 8, 10], &[3, 4, 0], &[2, 1, 0], 0, 11), &mut post,),
            Err(ScenarioError::ExcessiveRelease)
        );
        assert_eq!(post, unchanged);
    }

    #[test]
    fn empty_book_refuses_without_a_vacuous_solvency_claim() {
        let mut post = [];
        assert_eq!(
            plan_scenario_netting(book(&[], &[], &[], 0, 0), &mut post,),
            Err(ScenarioError::EmptyBook)
        );
    }

    #[test]
    fn stale_and_substituted_claims_positions_refuse_before_mutation() {
        let inventory = [4, 5, 6];
        let acquired = [0, 0, 0];
        let delivered = [1, 2, 3];
        let unchanged = [0xa5_u64; 3];

        let mut substituted_market = book(&inventory, &acquired, &delivered, 0, 0);
        substituted_market.position.market_id = [9; 32];
        let mut post = unchanged;
        assert_eq!(
            plan_scenario_netting(substituted_market, &mut post),
            Err(ScenarioError::PositionMismatch)
        );
        assert_eq!(post, unchanged);

        let mut substituted_holder = book(&inventory, &acquired, &delivered, 0, 0);
        substituted_holder.position.owner = [8; 32];
        assert_eq!(
            plan_scenario_netting(substituted_holder, &mut post),
            Err(ScenarioError::PositionMismatch)
        );
        assert_eq!(post, unchanged);

        let mut stale = book(&inventory, &acquired, &delivered, 0, 0);
        stale.expected_position_revision = REVISION + 1;
        assert_eq!(
            plan_scenario_netting(stale, &mut post),
            Err(ScenarioError::StalePosition)
        );
        assert_eq!(post, unchanged);
    }
}
