//! Hostile Fractional request and root boundary tests.

use dclutch_fractional_claim_contract::{
    FractionalActionV1, FractionalFamilyRequestInputV1, FractionalFamilyRequestV1,
    FractionalRequestErrorV1, FractionalRootInputV1, FractionalRootV1,
};

const ABSENT: u32 = u32::MAX;

fn base() -> FractionalFamilyRequestInputV1 {
    FractionalFamilyRequestInputV1 {
        release_set: [1; 32],
        market: [2; 32],
        product_record: [3; 32],
        result_domain: [4; 32],
        terms: [5; 32],
        token_behavior: [6; 32],
        owner: [7; 32],
        source_token_account: [0; 32],
        destination_token_account: [8; 32],
        terminal_digest: [0; 32],
        expected_revision: 9,
        quantity: 3,
        outcome: 1,
        terminal_outcome: ABSENT,
    }
}

fn request(action: FractionalActionV1) -> FractionalFamilyRequestV1 {
    let mut input = base();
    match action {
        FractionalActionV1::Wrap => {}
        FractionalActionV1::Transfer => {
            input.source_token_account = [8; 32];
            input.destination_token_account = [9; 32];
        }
        FractionalActionV1::WholeUnwrap => {
            input.source_token_account = [8; 32];
            input.destination_token_account = [0; 32];
        }
        FractionalActionV1::WinningRedeem => {
            input.source_token_account = [8; 32];
            input.destination_token_account = [0; 32];
            input.terminal_digest = [10; 32];
            input.terminal_outcome = 1;
        }
        FractionalActionV1::LosingZeroBurn => {
            input.source_token_account = [8; 32];
            input.destination_token_account = [0; 32];
            input.terminal_digest = [10; 32];
            input.terminal_outcome = 2;
        }
        FractionalActionV1::Terminalize => {
            input.owner = [0; 32];
            input.destination_token_account = [0; 32];
            input.terminal_digest = [10; 32];
            input.terminal_outcome = 1;
            input.quantity = 0;
        }
        FractionalActionV1::ZeroSupplyRetire => {
            input.owner = [0; 32];
            input.destination_token_account = [0; 32];
            input.terminal_digest = [10; 32];
            input.terminal_outcome = 1;
            input.quantity = 0;
            input.outcome = ABSENT;
        }
    }
    FractionalFamilyRequestV1::new(action, input).expect("canonical request")
}

#[test]
fn every_action_round_trips_without_hidden_change_fields() {
    for action in [
        FractionalActionV1::Wrap,
        FractionalActionV1::Transfer,
        FractionalActionV1::WholeUnwrap,
        FractionalActionV1::WinningRedeem,
        FractionalActionV1::LosingZeroBurn,
        FractionalActionV1::Terminalize,
        FractionalActionV1::ZeroSupplyRetire,
    ] {
        let value = request(action);
        assert_eq!(
            FractionalFamilyRequestV1::decode(&value.to_bytes()),
            Ok(value)
        );
    }
    let unwrap = request(FractionalActionV1::WholeUnwrap).input();
    assert_eq!(unwrap.destination_token_account, [0; 32]);
    assert_eq!(unwrap.source_token_account, [8; 32]);
}

#[test]
fn terminal_and_same_mint_canonicality_refuse_hostile_inputs() {
    let mut winning = request(FractionalActionV1::WinningRedeem).input();
    winning.outcome = 0;
    assert_eq!(
        FractionalFamilyRequestV1::new(FractionalActionV1::WinningRedeem, winning),
        Err(FractionalRequestErrorV1::InvalidTerminal)
    );

    let mut losing = request(FractionalActionV1::LosingZeroBurn).input();
    losing.outcome = losing.terminal_outcome;
    assert_eq!(
        FractionalFamilyRequestV1::new(FractionalActionV1::LosingZeroBurn, losing),
        Err(FractionalRequestErrorV1::InvalidTerminal)
    );

    let mut unwrap = request(FractionalActionV1::WholeUnwrap).input();
    unwrap.destination_token_account = [99; 32];
    assert_eq!(
        FractionalFamilyRequestV1::new(FractionalActionV1::WholeUnwrap, unwrap),
        Err(FractionalRequestErrorV1::NonCanonical)
    );

    let mut hostile = request(FractionalActionV1::Transfer).to_bytes();
    hostile[383] = 1;
    assert_eq!(
        FractionalFamilyRequestV1::decode(&hostile),
        Err(FractionalRequestErrorV1::NonCanonical)
    );
}

#[test]
fn root_persists_replay_and_rent_but_no_supply_or_remainder() {
    let root = FractionalRootV1::new(FractionalRootInputV1 {
        bump: 254,
        terms: [1; 32],
        market: [2; 32],
        rent_beneficiary: [3; 32],
        revision: u64::MAX,
        historical_rent_principal: 99,
    })
    .expect("root");
    assert_eq!(FractionalRootV1::decode(&root.to_bytes()), Some(root));
    let mut hostile = root.to_bytes();
    hostile[15] = 1;
    assert_eq!(FractionalRootV1::decode(&hostile), None);
}
