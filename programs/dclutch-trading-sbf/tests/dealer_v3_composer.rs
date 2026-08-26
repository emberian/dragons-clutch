//! Hostile integration coverage for Dealer V3 scenario-fill composition.

use dclutch_custody_contract::{CompartmentV1, CustodyVaultSeedsV1};
use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
use dclutch_trading_sbf::dealer::{
    v3_composer::{
        ScenarioCollateralFrameV3, ScenarioComposerContextV3, ScenarioFillInputV3,
        ScenarioQuoteDirectionV3, ScenarioQuoteLegV3, prepare_scenario_atomic_v3,
    },
    v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
        DealerObligationProjectionV3,
    },
};
use solana_program::{hash::hash, pubkey::Pubkey};

fn obligation_bytes(revision: u64, values: &[u64]) -> Vec<u8> {
    let mut bytes = vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + values.len() * 8];
    bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
    bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
    bytes[12..16].copy_from_slice(&(values.len() as u32).to_le_bytes());
    bytes[16..24].copy_from_slice(&revision.to_le_bytes());
    for (offset, value) in [
        (24, [1; 32]),
        (56, [2; 32]),
        (88, [3; 32]),
        (120, [4; 32]),
        (152, [5; 32]),
    ] {
        bytes[offset..offset + 32].copy_from_slice(&value);
    }
    for (index, value) in values.iter().enumerate() {
        let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn context_and_frame() -> (ScenarioComposerContextV3, ScenarioCollateralFrameV3) {
    let trading = [9; 32];
    let custody = [10; 32];
    let obligation_account = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &[5; 32]],
        &Pubkey::new_from_array(trading),
    )
    .0
    .to_bytes();
    let context = ScenarioComposerContextV3 {
        trading_program: trading,
        custody_program: custody,
        release_set: [6; 32],
        market: [1; 32],
        realm: [7; 32],
        child_root: [5; 32],
        obligation_account,
        mint: [11; 32],
        token_program: [12; 32],
        parent_request_digest: [13; 32],
        generation: 2,
        custody_replay_revision: 7,
        locked_capital_floor: 0,
    };
    let vault = |vault_context, compartment| {
        Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new([1; 32], [6; 32], vault_context, compartment).as_slices(),
            &Pubkey::new_from_array(custody),
        )
        .0
        .to_bytes()
    };
    let frame = ScenarioCollateralFrameV3 {
        principal_vault: vault([5; 32], CompartmentV1::TradingPrincipal),
        principal_balance: 20,
        fee_vault: vault([5; 32], CompartmentV1::FeeVault),
        fee_balance: 9,
        hoard_vault: vault([1; 32], CompartmentV1::HoardPrincipal),
        hoard_balance: 100,
        counterparty_account: [14; 32],
        counterparty_owner: [15; 32],
        counterparty_balance: 100,
    };
    (context, frame)
}

#[test]
fn incoming_quote_funds_minimum_split_while_fee_stays_segregated() {
    let (context, frame) = context_and_frame();
    let current_bytes = obligation_bytes(9, &[0, 0, 0]);
    let candidate_bytes = obligation_bytes(10, &[0, 0, 0]);
    let current = DealerObligationProjectionV3::decode(&current_bytes).expect("current");
    let candidate = DealerObligationProjectionV3::decode(&candidate_bytes).expect("candidate");
    let mut before = [0; 3];
    let mut after = [0; 3];
    let mut inventory = [0; 3];
    let mut equity = [0; 3];
    let plan = prepare_scenario_atomic_v3(
        context,
        frame,
        current,
        candidate,
        hash(&candidate_bytes).to_bytes(),
        12,
        ScenarioFillInputV3 {
            dealer_position: ClaimsInventoryObservation {
                market_id: [1; 32],
                product_id: [2; 32],
                liability_basis_id: [3; 32],
                position_owner: [4; 32],
                revision: 8,
                inventory: &[2, 10, 0],
            },
            counterparty_position_revision: 3,
            acquired: &[0, 0, 0],
            delivered: &[5, 1, 3],
            quote: ScenarioQuoteLegV3 {
                direction: ScenarioQuoteDirectionV3::CounterpartyPaysDealer,
                principal: 5,
                realized_fee: 1,
            },
        },
        &mut before,
        &mut after,
        &mut inventory,
        &mut equity,
    )
    .expect("atomic plan");
    assert_eq!(plan.scenario.minimum_complete_sets_to_split, 3);
    assert_eq!(plan.scenario.maximum_complete_sets_to_merge, 0);
    assert_eq!(inventory, [0, 12, 0]);
    assert_eq!(plan.principal_after, 22);
    assert_eq!(plan.fee_after, 10);
    assert_eq!(plan.hoard_after, 103);
    assert_eq!(plan.counterparty_after, 94);
    assert_eq!(plan.custody_count, 3);
}

#[test]
fn outgoing_quote_executes_after_merge_and_fee_never_becomes_capital() {
    let (context, frame) = context_and_frame();
    let current_bytes = obligation_bytes(9, &[0, 0, 0]);
    let candidate_bytes = obligation_bytes(10, &[0, 0, 0]);
    let current = DealerObligationProjectionV3::decode(&current_bytes).expect("current");
    let candidate = DealerObligationProjectionV3::decode(&candidate_bytes).expect("candidate");
    let mut before = [0; 3];
    let mut after = [0; 3];
    let mut inventory = [0; 3];
    let mut equity = [0; 3];
    let plan = prepare_scenario_atomic_v3(
        context,
        frame,
        current,
        candidate,
        hash(&candidate_bytes).to_bytes(),
        12,
        ScenarioFillInputV3 {
            dealer_position: ClaimsInventoryObservation {
                market_id: [1; 32],
                product_id: [2; 32],
                liability_basis_id: [3; 32],
                position_owner: [4; 32],
                revision: 8,
                inventory: &[5, 5, 5],
            },
            counterparty_position_revision: 3,
            acquired: &[0, 0, 0],
            delivered: &[1, 1, 1],
            quote: ScenarioQuoteLegV3 {
                direction: ScenarioQuoteDirectionV3::DealerPaysCounterparty,
                principal: 5,
                realized_fee: 1,
            },
        },
        &mut before,
        &mut after,
        &mut inventory,
        &mut equity,
    )
    .expect("atomic plan");
    assert_eq!(plan.scenario.minimum_complete_sets_to_split, 0);
    assert_eq!(plan.scenario.maximum_complete_sets_to_merge, 4);
    assert_eq!(inventory, [0, 0, 0]);
    assert_eq!(plan.principal_after, 19);
    assert_eq!(plan.fee_after, 10);
    assert_eq!(plan.hoard_after, 96);
    assert_eq!(plan.counterparty_after, 104);
    assert_eq!(plan.custody_count, 3);
}

#[test]
fn substituted_candidate_digest_refuses_without_claims_outputs() {
    let (context, frame) = context_and_frame();
    let current_bytes = obligation_bytes(9, &[0, 0, 0]);
    let candidate_bytes = obligation_bytes(10, &[0, 0, 0]);
    let current = DealerObligationProjectionV3::decode(&current_bytes).expect("current");
    let candidate = DealerObligationProjectionV3::decode(&candidate_bytes).expect("candidate");
    let mut before = [77; 3];
    let mut after = [77; 3];
    let mut inventory = [77; 3];
    let mut equity = [77; 3];
    let result = prepare_scenario_atomic_v3(
        context,
        frame,
        current,
        candidate,
        [99; 32],
        12,
        ScenarioFillInputV3 {
            dealer_position: ClaimsInventoryObservation {
                market_id: [1; 32],
                product_id: [2; 32],
                liability_basis_id: [3; 32],
                position_owner: [4; 32],
                revision: 8,
                inventory: &[2, 10, 0],
            },
            counterparty_position_revision: 3,
            acquired: &[0, 0, 0],
            delivered: &[5, 1, 3],
            quote: ScenarioQuoteLegV3 {
                direction: ScenarioQuoteDirectionV3::CounterpartyPaysDealer,
                principal: 5,
                realized_fee: 1,
            },
        },
        &mut before,
        &mut after,
        &mut inventory,
        &mut equity,
    );
    assert!(result.is_err());
    assert_eq!(inventory, [77; 3]);
    assert_eq!(equity, [77; 3]);
}
