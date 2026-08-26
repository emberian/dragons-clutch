//! Descriptor-bound stateless Dealer scenario planning.
//!
//! [`plan_descriptor_scenario`] is the sole generalized scenario-solvency
//! planner. [`plan_gross_covered_scenario`] is a conservative zero-obligation
//! profile over that same kernel; it owns no arithmetic and cannot diverge into
//! a parallel reserve implementation.

use dclutch_dealer_scenario_kernel::{
    plan_scenario_netting, ClaimsInventoryExpectation, ScenarioTransition,
};
pub use dclutch_dealer_scenario_kernel::{
    ClaimsInventoryObservation, Error as ScenarioError, ScenarioNettingPlan as ScenarioPlan,
};

/// Stable refusal at the Dealer scenario-profile boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioProfileError {
    /// The sole scenario-solvency kernel refused the candidate.
    Kernel(ScenarioError),
    /// The gross-covered profile supplied a nonzero terminal obligation.
    CoveredProfileHasObligation,
}

impl From<ScenarioError> for ScenarioProfileError {
    fn from(value: ScenarioError) -> Self {
        Self::Kernel(value)
    }
}

/// Result alias for descriptor-bound scenario planning.
pub type ScenarioResult<T> = core::result::Result<T, ScenarioProfileError>;

/// Immutable Dealer scenario-solvency descriptor projection.
///
/// A finalized content-addressed Dealer descriptor is the semantic owner of
/// these coordinates. This projection is stateless and contains no inventory,
/// obligations, fees, or anticipated revenue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioSolvencyDescriptor {
    /// Immutable logical Core Market identity.
    pub market_id: [u8; 32],
    /// Immutable stable semantic Product identity.
    pub product_id: [u8; 32],
    /// Immutable semantic LiabilityBasis identity.
    pub liability_basis_id: [u8; 32],
    /// Immutable canonical Dealer Claims Position owner.
    pub position_owner: [u8; 32],
    /// Minimum exact equity required in every terminal scenario.
    pub locked_capital_floor: u64,
}

impl ScenarioSolvencyDescriptor {
    fn expectation(self, position_revision: u64) -> ClaimsInventoryExpectation {
        ClaimsInventoryExpectation {
            market_id: self.market_id,
            product_id: self.product_id,
            liability_basis_id: self.liability_basis_id,
            position_owner: self.position_owner,
            position_revision,
        }
    }
}

/// Borrowed generalized scenario plan input.
///
/// Claims inventory remains borrowed from the canonical Position. Terminal
/// obligations are borrowed from their separately authenticated semantic owner
/// and are not persisted by this planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorScenarioInput<'a> {
    /// Immutable descriptor projection.
    pub descriptor: ScenarioSolvencyDescriptor,
    /// Canonical Claims Position projection.
    pub position: ClaimsInventoryObservation<'a>,
    /// Optimistic Claims Position revision bound by the request.
    pub expected_position_revision: u64,
    /// Present eligible collateral; anticipated fees are excluded.
    pub present_capital: u64,
    /// Exact incoming terminal obligations.
    pub obligations_before: &'a [u64],
    /// Nonnegative native Claims acquired in the atomic fill.
    pub acquired: &'a [u64],
    /// Nonnegative native Claims delivered in the atomic fill.
    pub delivered: &'a [u64],
    /// Exact candidate terminal obligations.
    pub obligations_after: &'a [u64],
}

/// Borrowed conservative gross-covered scenario profile.
///
/// Both obligation projections must be the same runtime width as Claims and
/// contain only zero. This explicitly embeds covered V1 into the generalized
/// planner instead of retaining a second reserve algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrossCoveredScenarioInput<'a> {
    /// General descriptor-bound input with zero terminal obligations.
    pub scenario: DescriptorScenarioInput<'a>,
}

/// Execute the sole descriptor-bound generalized scenario planner.
///
/// Both caller-owned outputs remain unchanged on every refusal.
pub fn plan_descriptor_scenario(
    input: DescriptorScenarioInput<'_>,
    post_inventory: &mut [u64],
    post_equity: &mut [i128],
) -> ScenarioResult<ScenarioPlan> {
    let expected = input
        .descriptor
        .expectation(input.expected_position_revision);
    plan_scenario_netting(
        ScenarioTransition {
            position: input.position,
            expected,
            present_capital: input.present_capital,
            locked_capital_floor: input.descriptor.locked_capital_floor,
            obligations_before: input.obligations_before,
            acquired: input.acquired,
            delivered: input.delivered,
            obligations_after: input.obligations_after,
        },
        post_inventory,
        post_equity,
    )
    .map_err(ScenarioProfileError::Kernel)
}

/// Execute the conservative zero-obligation profile through the sole planner.
///
/// The profile derives the same minimum split and maximum merge as the
/// generalized descriptor route. It does not accept a caller-selected release.
pub fn plan_gross_covered_scenario(
    input: GrossCoveredScenarioInput<'_>,
    post_inventory: &mut [u64],
    post_equity: &mut [i128],
) -> ScenarioResult<ScenarioPlan> {
    if input
        .scenario
        .obligations_before
        .iter()
        .chain(input.scenario.obligations_after.iter())
        .any(|obligation| *obligation != 0)
    {
        return Err(ScenarioProfileError::CoveredProfileHasObligation);
    }
    plan_descriptor_scenario(input.scenario, post_inventory, post_equity)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKET: [u8; 32] = [1; 32];
    const PRODUCT: [u8; 32] = [2; 32];
    const BASIS: [u8; 32] = [3; 32];
    const OWNER: [u8; 32] = [4; 32];

    const fn descriptor(floor: u64) -> ScenarioSolvencyDescriptor {
        ScenarioSolvencyDescriptor {
            market_id: MARKET,
            product_id: PRODUCT,
            liability_basis_id: BASIS,
            position_owner: OWNER,
            locked_capital_floor: floor,
        }
    }

    const fn position(inventory: &[u64]) -> ClaimsInventoryObservation<'_> {
        ClaimsInventoryObservation {
            market_id: MARKET,
            product_id: PRODUCT,
            liability_basis_id: BASIS,
            position_owner: OWNER,
            revision: 7,
            inventory,
        }
    }

    const fn input<'a>(
        inventory: &'a [u64],
        obligations_before: &'a [u64],
        acquired: &'a [u64],
        delivered: &'a [u64],
        obligations_after: &'a [u64],
        capital: u64,
        floor: u64,
    ) -> DescriptorScenarioInput<'a> {
        DescriptorScenarioInput {
            descriptor: descriptor(floor),
            position: position(inventory),
            expected_position_revision: 7,
            present_capital: capital,
            obligations_before,
            acquired,
            delivered,
            obligations_after,
        }
    }

    #[test]
    fn descriptor_plan_enforces_terminal_obligations_and_floor() {
        let mut post_inventory = [99; 3];
        let mut post_equity = [99; 3];
        let plan = plan_descriptor_scenario(
            input(
                &[2, 10, 0],
                &[2, 10, 0],
                &[3, 0, 4],
                &[10, 1, 6],
                &[0, 9, 3],
                10,
                5,
            ),
            &mut post_inventory,
            &mut post_equity,
        )
        .expect("descriptor-bound candidate meets every scenario floor");
        assert_eq!(plan.minimum_complete_sets_to_split, 5);
        assert_eq!(plan.maximum_complete_sets_to_merge, 0);
        assert_eq!(post_inventory, [0, 14, 3]);
        assert_eq!(post_equity, [5, 10, 5]);
    }

    #[test]
    fn gross_covered_profile_is_the_zero_obligation_embedding() {
        let zero = [0, 0, 0];
        let scenario = input(&[9, 8, 10], &zero, &[3, 4, 0], &[2, 1, 0], &zero, 20, 0);
        let mut generalized_inventory = [0; 3];
        let mut generalized_equity = [0; 3];
        let generalized = plan_descriptor_scenario(
            scenario,
            &mut generalized_inventory,
            &mut generalized_equity,
        );
        let mut covered_inventory = [0; 3];
        let mut covered_equity = [0; 3];
        let covered = plan_gross_covered_scenario(
            GrossCoveredScenarioInput { scenario },
            &mut covered_inventory,
            &mut covered_equity,
        );
        assert_eq!(covered, generalized);
        assert_eq!(covered_inventory, generalized_inventory);
        assert_eq!(covered_equity, generalized_equity);
    }

    #[test]
    fn covered_profile_refuses_obligations_without_touching_outputs() {
        let obligations = [0, 1, 0];
        let zero = [0, 0, 0];
        let mut post_inventory = [0xa5; 3];
        let mut post_equity = [0x5a; 3];
        assert_eq!(
            plan_gross_covered_scenario(
                GrossCoveredScenarioInput {
                    scenario: input(&[2, 3, 4], &obligations, &zero, &zero, &zero, 5, 0,),
                },
                &mut post_inventory,
                &mut post_equity,
            ),
            Err(ScenarioProfileError::CoveredProfileHasObligation)
        );
        assert_eq!(post_inventory, [0xa5; 3]);
        assert_eq!(post_equity, [0x5a; 3]);
    }
}
