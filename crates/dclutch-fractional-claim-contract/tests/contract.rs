//! Hostile Fractional request, root, and finalized-artifact boundary tests.

use dclutch_fractional_claim_contract::{
    ArtifactAdmissionV1, FractionalActionV1, FractionalArtifactAdmissionsV1,
    FractionalArtifactBytesV1, FractionalArtifactErrorV1, FractionalArtifactSelectionV1,
    FractionalChildProgramsV1, FractionalFamilyRequestInputV1, FractionalFamilyRequestV1,
    FractionalRequestErrorV1, FractionalRootInputV1, FractionalRootV1,
    authenticate_fractional_artifact_bundle_v1,
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

fn admission(byte: u8, authenticated: bool) -> ArtifactAdmissionV1 {
    ArtifactAdmissionV1 {
        finalized_digest: [byte; 32],
        record_authenticated: authenticated,
    }
}

fn admissions() -> FractionalArtifactAdmissionsV1 {
    FractionalArtifactAdmissionsV1 {
        descriptor: admission(1, true),
        terms: admission(2, true),
        token_behavior: admission(3, true),
        account_profile: admission(4, true),
        lifecycle: admission(5, true),
        request: admission(6, true),
        strategy: admission(7, true),
        transition: admission(8, true),
        effect: admission(9, true),
    }
}

fn selection() -> FractionalArtifactSelectionV1 {
    FractionalArtifactSelectionV1 {
        descriptor_id: [1; 32],
        terms_id: [2; 32],
        market: [3; 32],
        market_realm: [4; 32],
        product_record: [5; 32],
        result_domain: [6; 32],
        outcome_count: 3,
        release_set: [7; 32],
        children: FractionalChildProgramsV1 {
            claims: [8; 32],
            custody: [9; 32],
            token: [10; 32],
            physical_profile: [11; 32],
            release_authenticated: true,
        },
        semantic_selection_authenticated: true,
    }
}

const EMPTY_ARTIFACTS: FractionalArtifactBytesV1<'static> = FractionalArtifactBytesV1 {
    descriptor: b"descriptor",
    terms: b"terms",
    token_behavior: b"token",
    account_profile: b"account",
    lifecycle: b"lifecycle",
    request: b"request",
    strategy: b"strategy",
    transition: b"transition",
    effect: b"effect",
};

#[test]
fn record_and_release_substitution_refuse_before_artifact_decoding() {
    assert_eq!(
        authenticate_fractional_artifact_bundle_v1(
            selection(),
            admissions(),
            EMPTY_ARTIFACTS,
            &request(FractionalActionV1::Wrap).to_bytes(),
        ),
        Err(FractionalArtifactErrorV1::ArtifactIdentity)
    );

    let mut aliased = selection();
    aliased.children.custody = aliased.children.claims;
    assert_eq!(
        authenticate_fractional_artifact_bundle_v1(
            aliased,
            admissions(),
            EMPTY_ARTIFACTS,
            &request(FractionalActionV1::Wrap).to_bytes(),
        ),
        Err(FractionalArtifactErrorV1::SemanticSelection)
    );

    let mut unauthenticated = selection();
    unauthenticated.children.release_authenticated = false;
    assert_eq!(
        authenticate_fractional_artifact_bundle_v1(
            unauthenticated,
            admissions(),
            EMPTY_ARTIFACTS,
            &request(FractionalActionV1::Wrap).to_bytes(),
        ),
        Err(FractionalArtifactErrorV1::SemanticSelection)
    );
}
