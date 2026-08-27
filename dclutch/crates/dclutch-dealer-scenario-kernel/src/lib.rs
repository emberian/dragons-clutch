#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Runtime-width Dealer scenario solvency and complete-set netting.
//!
//! The kernel consumes borrowed projections of the canonical Claims Position
//! and terminal obligations. It never persists an inventory or obligation
//! mirror. Eligible capital is present collateral already authenticated by an
//! adapter; anticipated fees, future order flow, and liquidation proceeds have
//! no field and cannot contribute to equity.
//!
//! For every terminal scenario `s`, exact equity is:
//!
//! `present_capital + canonical_claim_inventory[s] - obligations[s]`.
//!
//! A transition derives the least equal complete-set split needed to execute
//! its outgoing Claims basket, followed by the greatest equal residual merge.
//! Both transformations preserve every scenario's assets. Outputs are written
//! only after identity, width, arithmetic, funding, and floor checks succeed.

/// Stable refusal from scenario solvency or complete-set netting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// No terminal scenario was supplied.
    EmptyScenarios,
    /// Runtime-width inventory, obligation, transfer, or output slices differed.
    WidthMismatch,
    /// An immutable expected identity was zero.
    InvalidIdentity,
    /// Market, Product, liability basis, or Position owner did not join.
    PositionMismatch,
    /// The canonical Claims Position revision was stale.
    StalePosition,
    /// Checked native-claim, collateral, or revision arithmetic overflowed.
    ArithmeticOverflow,
    /// Present eligible capital could not fund the minimum complete-set split.
    InsufficientPresentCapital,
    /// The incoming canonical state was already below its locked capital floor.
    IncomingBelowLockedFloor,
    /// The candidate state would fall below its locked capital floor.
    CandidateBelowLockedFloor,
}

/// Result alias for the Dealer scenario kernel.
pub type Result<T> = core::result::Result<T, Error>;

/// Ephemeral projection of the one canonical Dealer Claims Position.
///
/// The Solana adapter must authenticate the raw Claims account, exact Product
/// graph, LiabilityBasis record, Market, release, owner, width, and revision
/// before constructing this value. This borrowed projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsInventoryObservation<'a> {
    /// Logical Core Market identity.
    pub market_id: [u8; 32],
    /// Stable semantic Product identity selecting the terminal scenarios.
    pub product_id: [u8; 32],
    /// Semantic LiabilityBasis identity selecting the native Claims vector.
    pub liability_basis_id: [u8; 32],
    /// Sole owner of this canonical Claims Position.
    pub position_owner: [u8; 32],
    /// Current canonical Claims Position revision.
    pub revision: u64,
    /// Runtime-width native Claims inventory.
    pub inventory: &'a [u64],
}

/// Immutable and optimistic coordinates expected by one Dealer request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsInventoryExpectation {
    /// Immutable logical Core Market identity.
    pub market_id: [u8; 32],
    /// Immutable stable semantic Product identity.
    pub product_id: [u8; 32],
    /// Immutable semantic LiabilityBasis identity.
    pub liability_basis_id: [u8; 32],
    /// Immutable canonical Dealer Position owner.
    pub position_owner: [u8; 32],
    /// Optimistic Claims Position revision bound by the request.
    pub position_revision: u64,
}

/// One immutable terminal-scenario solvency snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioSolvencySnapshot<'a> {
    /// Borrowed canonical Claims Position projection.
    pub position: ClaimsInventoryObservation<'a>,
    /// Immutable and optimistic Dealer request coordinates.
    pub expected: ClaimsInventoryExpectation,
    /// Present eligible collateral atoms; anticipated fees are excluded.
    pub present_capital: u64,
    /// Exact terminal obligations in collateral atoms.
    pub obligations: &'a [u64],
    /// Minimum admitted equity in every terminal scenario.
    pub locked_capital_floor: u64,
}

/// Exact admitted solvency summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioSolvencyReport {
    /// Exact minimum signed terminal equity.
    pub minimum_equity: i128,
    /// First terminal scenario attaining the minimum equity.
    pub minimum_scenario: usize,
    /// Present eligible collateral used by the calculation.
    pub present_capital: u64,
    /// Immutable locked capital floor applied to every scenario.
    pub locked_capital_floor: u64,
}

/// Borrowed inputs for one atomic Claims basket transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioTransition<'a> {
    /// Borrowed canonical Claims Position projection before the transition.
    pub position: ClaimsInventoryObservation<'a>,
    /// Immutable and optimistic Dealer request coordinates.
    pub expected: ClaimsInventoryExpectation,
    /// Present eligible collateral atoms before split or merge.
    pub present_capital: u64,
    /// Immutable locked capital floor applied before and after the transition.
    pub locked_capital_floor: u64,
    /// Exact incoming terminal obligations.
    pub obligations_before: &'a [u64],
    /// Nonnegative native Claims acquired in the same atomic transaction.
    pub acquired: &'a [u64],
    /// Nonnegative native Claims delivered in the same atomic transaction.
    pub delivered: &'a [u64],
    /// Exact candidate terminal obligations after the atomic transaction.
    pub obligations_after: &'a [u64],
}

/// Exact complete-set netting and scenario-solvency candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioNettingPlan {
    /// Least equal complete-set quantity that makes every delivery possible.
    pub minimum_complete_sets_to_split: u64,
    /// Greatest equal residual complete-set quantity available for merge.
    pub maximum_complete_sets_to_merge: u64,
    /// Present eligible collateral before netting.
    pub capital_before: u64,
    /// Candidate eligible collateral after split then merge.
    pub capital_after: u64,
    /// Exact minimum incoming terminal equity.
    pub minimum_equity_before: i128,
    /// First incoming scenario attaining the minimum equity.
    pub minimum_scenario_before: usize,
    /// Exact minimum candidate terminal equity.
    pub minimum_equity_after: i128,
    /// First candidate scenario attaining the minimum equity.
    pub minimum_scenario_after: usize,
    /// Canonical Claims Position revision consumed by the plan.
    pub position_revision_before: u64,
    /// Exact successor Claims Position revision expected after the batch.
    pub position_revision_after: u64,
}

/// Compute exact signed equity for one terminal scenario.
///
/// Conversion to `i128` is exact for every `u64` input: the greatest asset sum
/// is less than `2^65`, and subtracting one `u64` obligation remains in range.
pub fn scenario_equity(
    present_capital: u64,
    canonical_claim_inventory: u64,
    obligation: u64,
) -> i128 {
    i128::from(present_capital) + i128::from(canonical_claim_inventory) - i128::from(obligation)
}

/// Require exact terminal-scenario equity to meet the locked capital floor.
///
/// `equity_by_scenario` is left byte-for-byte unchanged on every refusal.
pub fn assess_scenario_solvency(
    snapshot: ScenarioSolvencySnapshot<'_>,
    equity_by_scenario: &mut [i128],
) -> Result<ScenarioSolvencyReport> {
    authenticate_position(snapshot.position, snapshot.expected)?;
    let width = snapshot.position.inventory.len();
    require_nonempty_width(width)?;
    require_width(width, snapshot.obligations.len())?;
    require_width(width, equity_by_scenario.len())?;

    let (minimum_equity, minimum_scenario) = minimum_equity(
        snapshot.present_capital,
        snapshot.position.inventory,
        snapshot.obligations,
    )?;
    require_floor(
        minimum_equity,
        snapshot.locked_capital_floor,
        Error::CandidateBelowLockedFloor,
    )?;

    for ((inventory, obligation), output) in snapshot
        .position
        .inventory
        .iter()
        .zip(snapshot.obligations.iter())
        .zip(equity_by_scenario.iter_mut())
    {
        *output = scenario_equity(snapshot.present_capital, *inventory, *obligation);
    }

    Ok(ScenarioSolvencyReport {
        minimum_equity,
        minimum_scenario,
        present_capital: snapshot.present_capital,
        locked_capital_floor: snapshot.locked_capital_floor,
    })
}

/// Derive the canonical minimum-split/maximum-merge plan for one atomic fill.
///
/// The incoming state and candidate state must both satisfy the locked floor.
/// The function checks all arithmetic in read-only passes. `post_inventory` and
/// `post_equity` remain byte-for-byte unchanged on any refusal, and are written
/// only after the complete candidate is admitted.
pub fn plan_scenario_netting(
    transition: ScenarioTransition<'_>,
    post_inventory: &mut [u64],
    post_equity: &mut [i128],
) -> Result<ScenarioNettingPlan> {
    authenticate_position(transition.position, transition.expected)?;
    let width = transition.position.inventory.len();
    require_nonempty_width(width)?;
    for observed in [
        transition.obligations_before.len(),
        transition.acquired.len(),
        transition.delivered.len(),
        transition.obligations_after.len(),
        post_inventory.len(),
        post_equity.len(),
    ] {
        require_width(width, observed)?;
    }

    let (minimum_equity_before, minimum_scenario_before) = minimum_equity(
        transition.present_capital,
        transition.position.inventory,
        transition.obligations_before,
    )?;
    require_floor(
        minimum_equity_before,
        transition.locked_capital_floor,
        Error::IncomingBelowLockedFloor,
    )?;

    let position_revision_after = transition
        .position
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;

    let mut minimum_split = 0_u64;
    for ((inventory, acquired), delivered) in transition
        .position
        .inventory
        .iter()
        .zip(transition.acquired.iter())
        .zip(transition.delivered.iter())
    {
        let available = inventory
            .checked_add(*acquired)
            .ok_or(Error::ArithmeticOverflow)?;
        minimum_split = minimum_split.max(delivered.saturating_sub(available));
    }
    if minimum_split > transition.present_capital {
        return Err(Error::InsufficientPresentCapital);
    }

    let mut maximum_merge = u64::MAX;
    for ((inventory, acquired), delivered) in transition
        .position
        .inventory
        .iter()
        .zip(transition.acquired.iter())
        .zip(transition.delivered.iter())
    {
        let funded = inventory
            .checked_add(*acquired)
            .and_then(|available| available.checked_add(minimum_split))
            .and_then(|available| available.checked_sub(*delivered))
            .ok_or(Error::ArithmeticOverflow)?;
        maximum_merge = maximum_merge.min(funded);
    }

    let capital_after = transition
        .present_capital
        .checked_sub(minimum_split)
        .and_then(|capital| capital.checked_add(maximum_merge))
        .ok_or(Error::ArithmeticOverflow)?;

    let mut minimum_equity_after = i128::MAX;
    let mut minimum_scenario_after = 0_usize;
    for (scenario, (((inventory, acquired), delivered), obligation)) in transition
        .position
        .inventory
        .iter()
        .zip(transition.acquired.iter())
        .zip(transition.delivered.iter())
        .zip(transition.obligations_after.iter())
        .enumerate()
    {
        let funded = inventory
            .checked_add(*acquired)
            .and_then(|available| available.checked_add(minimum_split))
            .and_then(|available| available.checked_sub(*delivered))
            .ok_or(Error::ArithmeticOverflow)?;
        let candidate = funded
            .checked_sub(maximum_merge)
            .ok_or(Error::ArithmeticOverflow)?;
        let equity = scenario_equity(capital_after, candidate, *obligation);
        if equity < minimum_equity_after {
            minimum_equity_after = equity;
            minimum_scenario_after = scenario;
        }
    }
    require_floor(
        minimum_equity_after,
        transition.locked_capital_floor,
        Error::CandidateBelowLockedFloor,
    )?;

    // Every operation below was checked for every coordinate above. Saturating
    // arithmetic is exact under those admitted bounds and cannot introduce a
    // late refusal after a caller-owned output has been touched.
    for (((((inventory, acquired), delivered), obligation), inventory_output), equity_output) in
        transition
            .position
            .inventory
            .iter()
            .zip(transition.acquired.iter())
            .zip(transition.delivered.iter())
            .zip(transition.obligations_after.iter())
            .zip(post_inventory.iter_mut())
            .zip(post_equity.iter_mut())
    {
        let candidate = inventory
            .saturating_add(*acquired)
            .saturating_add(minimum_split)
            .saturating_sub(*delivered)
            .saturating_sub(maximum_merge);
        *inventory_output = candidate;
        *equity_output = scenario_equity(capital_after, candidate, *obligation);
    }

    Ok(ScenarioNettingPlan {
        minimum_complete_sets_to_split: minimum_split,
        maximum_complete_sets_to_merge: maximum_merge,
        capital_before: transition.present_capital,
        capital_after,
        minimum_equity_before,
        minimum_scenario_before,
        minimum_equity_after,
        minimum_scenario_after,
        position_revision_before: transition.position.revision,
        position_revision_after,
    })
}

fn authenticate_position(
    observed: ClaimsInventoryObservation<'_>,
    expected: ClaimsInventoryExpectation,
) -> Result<()> {
    if expected.market_id == [0; 32]
        || expected.product_id == [0; 32]
        || expected.liability_basis_id == [0; 32]
        || expected.position_owner == [0; 32]
    {
        return Err(Error::InvalidIdentity);
    }
    if observed.market_id != expected.market_id
        || observed.product_id != expected.product_id
        || observed.liability_basis_id != expected.liability_basis_id
        || observed.position_owner != expected.position_owner
    {
        return Err(Error::PositionMismatch);
    }
    if observed.revision != expected.position_revision {
        return Err(Error::StalePosition);
    }
    Ok(())
}

fn minimum_equity(
    present_capital: u64,
    inventory: &[u64],
    obligations: &[u64],
) -> Result<(i128, usize)> {
    let mut values = inventory.iter().zip(obligations.iter()).enumerate();
    let (first_scenario, (first_inventory, first_obligation)) =
        values.next().ok_or(Error::EmptyScenarios)?;
    let mut minimum = scenario_equity(present_capital, *first_inventory, *first_obligation);
    let mut scenario = first_scenario;
    for (candidate_scenario, (candidate_inventory, candidate_obligation)) in values {
        let candidate =
            scenario_equity(present_capital, *candidate_inventory, *candidate_obligation);
        if candidate < minimum {
            minimum = candidate;
            scenario = candidate_scenario;
        }
    }
    Ok((minimum, scenario))
}

fn require_floor(minimum: i128, floor: u64, refusal: Error) -> Result<()> {
    if minimum < i128::from(floor) {
        return Err(refusal);
    }
    Ok(())
}

const fn require_nonempty_width(width: usize) -> Result<()> {
    if width == 0 {
        return Err(Error::EmptyScenarios);
    }
    Ok(())
}

const fn require_width(expected: usize, observed: usize) -> Result<()> {
    if expected != observed {
        return Err(Error::WidthMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
