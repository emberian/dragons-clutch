use super::*;

const MARKET: [u8; 32] = [1; 32];
const PRODUCT: [u8; 32] = [2; 32];
const BASIS: [u8; 32] = [3; 32];
const OWNER: [u8; 32] = [4; 32];
const REVISION: u64 = 7;

const fn observation(inventory: &[u64]) -> ClaimsInventoryObservation<'_> {
    ClaimsInventoryObservation {
        market_id: MARKET,
        product_id: PRODUCT,
        liability_basis_id: BASIS,
        position_owner: OWNER,
        revision: REVISION,
        inventory,
    }
}

const fn expectation() -> ClaimsInventoryExpectation {
    ClaimsInventoryExpectation {
        market_id: MARKET,
        product_id: PRODUCT,
        liability_basis_id: BASIS,
        position_owner: OWNER,
        position_revision: REVISION,
    }
}

const fn snapshot<'a>(
    inventory: &'a [u64],
    obligations: &'a [u64],
    present_capital: u64,
    locked_capital_floor: u64,
) -> ScenarioSolvencySnapshot<'a> {
    ScenarioSolvencySnapshot {
        position: observation(inventory),
        expected: expectation(),
        present_capital,
        obligations,
        locked_capital_floor,
    }
}

const fn transition<'a>(
    inventory: &'a [u64],
    obligations_before: &'a [u64],
    acquired: &'a [u64],
    delivered: &'a [u64],
    obligations_after: &'a [u64],
    present_capital: u64,
    locked_capital_floor: u64,
) -> ScenarioTransition<'a> {
    ScenarioTransition {
        position: observation(inventory),
        expected: expectation(),
        present_capital,
        locked_capital_floor,
        obligations_before,
        acquired,
        delivered,
        obligations_after,
    }
}

#[test]
fn exact_signed_equity_and_first_minimum_are_reported() {
    assert_eq!(scenario_equity(3, 2, 10), -5);
    let mut equity = [99_i128; 3];
    let report = assess_scenario_solvency(snapshot(&[2, 10, 0], &[5, 14, 3], 8, 4), &mut equity)
        .expect("every exact terminal equity meets four");
    assert_eq!(equity, [5, 4, 5]);
    assert_eq!(report.minimum_equity, 4);
    assert_eq!(report.minimum_scenario, 1);
    assert_eq!(report.locked_capital_floor, 4);
}

#[test]
fn anticipated_fees_have_no_capital_coordinate() {
    let anticipated_fees = u64::MAX;
    let unchanged = [77_i128; 2];
    let mut equity = unchanged;
    assert_eq!(
        assess_scenario_solvency(snapshot(&[0, 0], &[1, 1], 0, 0), &mut equity),
        Err(Error::CandidateBelowLockedFloor)
    );
    assert_eq!(anticipated_fees, u64::MAX);
    assert_eq!(equity, unchanged);
}

#[test]
fn minimum_split_nets_incoming_claims_and_preserves_floor() {
    let inventory = [2, 10, 0];
    let acquired = [3, 0, 4];
    let delivered = [10, 1, 6];
    let obligations_before = [2, 10, 0];
    let obligations_after = [0, 9, 3];
    let mut post_inventory = [u64::MAX; 3];
    let mut post_equity = [i128::MIN; 3];
    let plan = plan_scenario_netting(
        transition(
            &inventory,
            &obligations_before,
            &acquired,
            &delivered,
            &obligations_after,
            10,
            5,
        ),
        &mut post_inventory,
        &mut post_equity,
    )
    .expect("five present atoms fund the maximum terminal shortfall");
    assert_eq!(plan.minimum_complete_sets_to_split, 5);
    assert_eq!(plan.maximum_complete_sets_to_merge, 0);
    assert_eq!(plan.capital_after, 5);
    assert_eq!(post_inventory, [0, 14, 3]);
    assert_eq!(post_equity, [5, 10, 5]);
    assert_eq!(plan.minimum_equity_after, 5);
}

#[test]
fn maximum_equal_residual_merge_is_derived_not_requested() {
    let inventory = [9, 8, 10];
    let acquired = [3, 4, 0];
    let delivered = [2, 1, 0];
    let obligations = [0, 0, 0];
    let mut post_inventory = [0; 3];
    let mut post_equity = [0; 3];
    let plan = plan_scenario_netting(
        transition(
            &inventory,
            &obligations,
            &acquired,
            &delivered,
            &obligations,
            20,
            20,
        ),
        &mut post_inventory,
        &mut post_equity,
    )
    .expect("all ten equal residual sets can merge");
    assert_eq!(plan.minimum_complete_sets_to_split, 0);
    assert_eq!(plan.maximum_complete_sets_to_merge, 10);
    assert_eq!(plan.capital_after, 30);
    assert_eq!(post_inventory, [0, 1, 0]);
    assert_eq!(post_equity, [30, 31, 30]);
}

#[test]
fn runtime_width_258_and_full_u64_inventory_are_exact() {
    let mut inventory = [0_u64; 258];
    let mut obligations = [0_u64; 258];
    let acquired = [0_u64; 258];
    let delivered = [0_u64; 258];
    *inventory.first_mut().expect("nonempty") = u64::MAX;
    *inventory.last_mut().expect("nonempty") = u64::MAX;
    *obligations.first_mut().expect("nonempty") = u64::MAX;
    *obligations.last_mut().expect("nonempty") = u64::MAX;
    let mut equity = [i128::MIN; 258];
    let report = assess_scenario_solvency(
        snapshot(&inventory, &obligations, u64::MAX, u64::MAX),
        &mut equity,
    )
    .expect("full-width exact signed arithmetic remains in i128 range");
    assert_eq!(report.minimum_equity, u64::MAX.into());
    assert!(equity.iter().all(|value| *value >= i128::from(u64::MAX)));

    let mut post_inventory = [7_u64; 258];
    let mut post_equity = [7_i128; 258];
    let plan = plan_scenario_netting(
        transition(
            &inventory,
            &obligations,
            &acquired,
            &delivered,
            &obligations,
            u64::MAX,
            0,
        ),
        &mut post_inventory,
        &mut post_equity,
    )
    .expect("258-wide no-op netting is exact");
    assert_eq!(plan.minimum_complete_sets_to_split, 0);
    assert_eq!(plan.maximum_complete_sets_to_merge, 0);
    assert_eq!(post_inventory, inventory);
    assert_eq!(post_equity, equity);

    let mut overflow_acquired = [0_u64; 258];
    *overflow_acquired.first_mut().expect("nonempty") = 1;
    post_inventory.fill(7);
    post_equity.fill(7);
    assert_eq!(
        plan_scenario_netting(
            transition(
                &inventory,
                &obligations,
                &overflow_acquired,
                &delivered,
                &obligations,
                u64::MAX,
                0,
            ),
            &mut post_inventory,
            &mut post_equity,
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(post_inventory, [7; 258]);
    assert_eq!(post_equity, [7; 258]);
}

#[test]
fn hostile_identity_width_revision_funding_floor_and_overflow_are_atomic() {
    let inventory = [2, 3, 4];
    let zero = [0, 0, 0];
    let deliveries = [8, 3, 4];
    let unchanged_inventory = [0xa5_u64; 3];
    let unchanged_equity = [0x5a_i128; 3];

    let mut cases = [
        transition(&inventory, &zero, &zero, &deliveries, &zero, 5, 0),
        transition(&inventory, &zero, &zero, &zero, &[9, 9, 9], 5, 1),
        transition(&[u64::MAX, 0, 0], &zero, &[1, 0, 0], &zero, &zero, 0, 0),
    ];
    let expected_errors = [
        Error::InsufficientPresentCapital,
        Error::CandidateBelowLockedFloor,
        Error::ArithmeticOverflow,
    ];
    for (case, expected_error) in cases.iter_mut().zip(expected_errors) {
        let mut post_inventory = unchanged_inventory;
        let mut post_equity = unchanged_equity;
        assert_eq!(
            plan_scenario_netting(*case, &mut post_inventory, &mut post_equity),
            Err(expected_error)
        );
        assert_eq!(post_inventory, unchanged_inventory);
        assert_eq!(post_equity, unchanged_equity);
    }

    let mut stale = transition(&inventory, &zero, &zero, &zero, &zero, 5, 0);
    stale.expected.position_revision = REVISION + 1;
    let mut post_inventory = unchanged_inventory;
    let mut post_equity = unchanged_equity;
    assert_eq!(
        plan_scenario_netting(stale, &mut post_inventory, &mut post_equity),
        Err(Error::StalePosition)
    );
    assert_eq!(post_inventory, unchanged_inventory);
    assert_eq!(post_equity, unchanged_equity);

    let mut substituted = transition(&inventory, &zero, &zero, &zero, &zero, 5, 0);
    substituted.position.product_id = [9; 32];
    assert_eq!(
        plan_scenario_netting(substituted, &mut post_inventory, &mut post_equity),
        Err(Error::PositionMismatch)
    );
    assert_eq!(post_inventory, unchanged_inventory);
    assert_eq!(post_equity, unchanged_equity);

    let wrong_width = transition(&inventory, &zero, &[0, 0], &zero, &zero, 5, 0);
    assert_eq!(
        plan_scenario_netting(wrong_width, &mut post_inventory, &mut post_equity),
        Err(Error::WidthMismatch)
    );
    assert_eq!(post_inventory, unchanged_inventory);
    assert_eq!(post_equity, unchanged_equity);
}

#[test]
fn empty_scenarios_and_revision_overflow_refuse_without_mutation() {
    let mut empty_inventory = [];
    let mut empty_equity = [];
    assert_eq!(
        plan_scenario_netting(
            transition(&[], &[], &[], &[], &[], 0, 0),
            &mut empty_inventory,
            &mut empty_equity,
        ),
        Err(Error::EmptyScenarios)
    );

    let inventory = [0];
    let obligations = [0];
    let mut overflow = transition(
        &inventory,
        &obligations,
        &obligations,
        &obligations,
        &obligations,
        0,
        0,
    );
    overflow.position.revision = u64::MAX;
    overflow.expected.position_revision = u64::MAX;
    let mut post_inventory = [9];
    let mut post_equity = [9];
    assert_eq!(
        plan_scenario_netting(overflow, &mut post_inventory, &mut post_equity),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(post_inventory, [9]);
    assert_eq!(post_equity, [9]);
}
