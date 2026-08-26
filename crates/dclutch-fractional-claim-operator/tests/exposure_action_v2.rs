//! Exposure-bound TokenBehavior and exact Token-2022 effect corpus.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_contract::{
    FractionalExposureActionV2, FractionalExposureRequestInputV2, FractionalExposureRequestV2,
    NO_EXPOSURE_COORDINATE_V2,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsInputV2, FractionalExposureTermsV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use dclutch_fractional_claim_operator::{
    Error, FractionalExposureMintSnapshotV2, FractionalExposureRetirementContextV2,
    FractionalExposureTokenEffectV2, FractionalExposureTokenObservationV2,
    FractionalTokenAccountSnapshotV1, FractionalTokenBehaviorRecordAdmissionV2,
    authenticate_fractional_token_behavior_v2, plan_fractional_exposure_retirement_v2,
    plan_fractional_exposure_token_effect_v2,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};
use solana_program::pubkey::Pubkey;

const RELEASE: [u8; 32] = [1; 32];
const MARKET: [u8; 32] = [2; 32];
const PRODUCT: [u8; 32] = [3; 32];
const DOMAIN: [u8; 32] = [4; 32];
const TERMS: [u8; 32] = [5; 32];
const TOKEN_BEHAVIOR: [u8; 32] = [6; 32];
const EXPOSURE: [u8; 32] = [7; 32];
const REALM: [u8; 32] = [8; 32];
const OWNER: [u8; 32] = [9; 32];
const SOURCE: [u8; 32] = [10; 32];
const DESTINATION: [u8; 32] = [11; 32];
const TERMINAL: [u8; 32] = [12; 32];
const MINTS: [[u8; 32]; 3] = [[21; 32], [22; 32], [23; 32]];
const ROOT: [u8; 32] = [31; 32];

fn terms_bytes() -> Vec<u8> {
    let length = fractional_exposure_terms_bytes_v2(MINTS.len()).expect("terms width");
    let mut scratch = vec![0_u8; length];
    let mut output = vec![0_u8; length];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: MARKET,
            product_record: PRODUCT,
            result_domain: DOMAIN,
            release_set: RELEASE,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: TOKEN_BEHAVIOR,
            exposure_id: EXPOSURE,
            product_basis: [32; 32],
            representation_basis: [33; 32],
            graph_id: [34; 32],
            product_width: 258,
            denominator: 10,
            shard_mints: &MINTS,
        },
        &mut scratch,
        &mut output,
    )
    .expect("canonical terms");
    output
}

fn terms(bytes: &[u8]) -> FractionalExposureTermsV2<'_> {
    FractionalExposureTermsV2::decode(
        bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: TERMS,
            finalized_terms_id: TERMS,
            recomputed_terms_digest: TERMS,
            finalized_terms_digest: TERMS,
            record_authenticated: true,
        },
    )
    .expect("admitted terms")
}

fn behavior_admission() -> FractionalTokenBehaviorRecordAdmissionV2 {
    FractionalTokenBehaviorRecordAdmissionV2 {
        selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        selected_content_digest: TOKEN_BEHAVIOR,
        finalized_content_digest: TOKEN_BEHAVIOR,
        recomputed_content_digest: TOKEN_BEHAVIOR,
        record_authenticated: true,
        market_realm_authenticated: true,
    }
}

fn request(action: FractionalExposureActionV2, quantity: u64) -> FractionalExposureRequestV2 {
    let terminal = matches!(
        action,
        FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn
            | FractionalExposureActionV2::Terminalize
            | FractionalExposureActionV2::ZeroSupplyRetire
    );
    let actor = matches!(
        action,
        FractionalExposureActionV2::Wrap
            | FractionalExposureActionV2::Transfer
            | FractionalExposureActionV2::WholeUnwrap
            | FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn
    );
    let source = matches!(
        action,
        FractionalExposureActionV2::Transfer
            | FractionalExposureActionV2::WholeUnwrap
            | FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn
    );
    let destination = matches!(
        action,
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::Transfer
    );
    FractionalExposureRequestV2::new(
        action,
        FractionalExposureRequestInputV2 {
            release_set: RELEASE,
            market: MARKET,
            product_record: PRODUCT,
            result_domain: DOMAIN,
            terms: TERMS,
            token_behavior: TOKEN_BEHAVIOR,
            exposure: EXPOSURE,
            owner: if actor { OWNER } else { [0; 32] },
            source_token_account: if source { SOURCE } else { [0; 32] },
            destination_token_account: if destination { DESTINATION } else { [0; 32] },
            terminal_digest: if terminal { TERMINAL } else { [0; 32] },
            expected_revision: 7,
            quantity,
            representation_coordinate: if actor { 1 } else { NO_EXPOSURE_COORDINATE_V2 },
        },
    )
    .expect("canonical request")
}

fn token_account(mint: Pubkey, owner: Pubkey, amount: u64) -> [u8; 165] {
    let mut data = [0_u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

fn behavior_mint(controller: Pubkey, supply: u64, decimals: u8) -> Vec<u8> {
    let mut data = vec![0_u8; 238];
    data[0..4].copy_from_slice(&1_u32.to_le_bytes());
    data[4..36].copy_from_slice(controller.as_ref());
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = decimals;
    data[45] = 1;
    data[165] = 1;
    data[166..168].copy_from_slice(&3_u16.to_le_bytes());
    data[168..170].copy_from_slice(&32_u16.to_le_bytes());
    data[170..202].copy_from_slice(controller.as_ref());
    data[202..204].copy_from_slice(&28_u16.to_le_bytes());
    data[204..206].copy_from_slice(&32_u16.to_le_bytes());
    data[206..238].copy_from_slice(controller.as_ref());
    data
}

fn snapshot<'a>(key: [u8; 32], data: &'a [u8]) -> FractionalTokenAccountSnapshotV1<'a> {
    FractionalTokenAccountSnapshotV1 {
        key: Pubkey::new_from_array(key),
        program_owner: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
        data,
    }
}

#[test]
fn token_behavior_record_is_terms_and_market_selected() {
    let encoded_terms = terms_bytes();
    let terms = terms(&encoded_terms);
    let selection = TokenBehaviorSelectionV2::new(REALM, RELEASE)
        .expect("selection")
        .to_bytes();
    let checked =
        authenticate_fractional_token_behavior_v2(terms, REALM, &selection, behavior_admission())
            .expect("authenticated behavior");
    assert_eq!(checked.content_digest(), TOKEN_BEHAVIOR);
    assert_eq!(checked.selection().realm(), REALM);

    let mut wrong = behavior_admission();
    wrong.finalized_content_digest = [99; 32];
    assert_eq!(
        authenticate_fractional_token_behavior_v2(terms, REALM, &selection, wrong),
        Err(Error::Token)
    );
    assert_eq!(
        authenticate_fractional_token_behavior_v2(
            terms,
            [98; 32],
            &selection,
            behavior_admission(),
        ),
        Err(Error::Token)
    );
}

#[test]
fn wrap_transfer_and_whole_burn_rederive_exact_token_effects() {
    let encoded_terms = terms_bytes();
    let terms = terms(&encoded_terms);
    let selection = TokenBehaviorSelectionV2::new(REALM, RELEASE)
        .expect("selection")
        .to_bytes();
    let behavior =
        authenticate_fractional_token_behavior_v2(terms, REALM, &selection, behavior_admission())
            .expect("behavior");
    let root = Pubkey::new_from_array(ROOT);
    let mint_key = Pubkey::new_from_array(MINTS[1]);
    let mint = behavior_mint(root, 30, 9);
    let source = token_account(mint_key, Pubkey::new_from_array(OWNER), 23);
    let destination = token_account(mint_key, Pubkey::new_from_array([55; 32]), 3);

    let wrap_destination = token_account(mint_key, Pubkey::new_from_array(OWNER), 3);
    let wrap = plan_fractional_exposure_token_effect_v2(
        terms,
        request(FractionalExposureActionV2::Wrap, 2),
        behavior,
        FractionalExposureTokenObservationV2 {
            root_controller: root,
            mint: Some(snapshot(MINTS[1], &mint)),
            source: None,
            destination: Some(snapshot(DESTINATION, &wrap_destination)),
            pre_supply: 30,
            pre_source: 0,
            pre_destination: 3,
        },
    )
    .expect("wrap");
    assert!(matches!(
        wrap.effect(),
        FractionalExposureTokenEffectV2::Mint(_)
    ));
    assert_eq!(wrap.consumed_shards(), 20);
    assert_eq!((wrap.pre_supply(), wrap.post_supply()), (30, 50));
    assert_eq!((wrap.pre_destination(), wrap.post_destination()), (3, 23));

    let transfer = plan_fractional_exposure_token_effect_v2(
        terms,
        request(FractionalExposureActionV2::Transfer, 7),
        behavior,
        FractionalExposureTokenObservationV2 {
            root_controller: root,
            mint: Some(snapshot(MINTS[1], &mint)),
            source: Some(snapshot(SOURCE, &source)),
            destination: Some(snapshot(DESTINATION, &destination)),
            pre_supply: 30,
            pre_source: 23,
            pre_destination: 3,
        },
    )
    .expect("transfer");
    assert!(matches!(
        transfer.effect(),
        FractionalExposureTokenEffectV2::Transfer(_)
    ));
    assert_eq!((transfer.pre_source(), transfer.post_source()), (23, 16));
    assert_eq!(
        (transfer.pre_destination(), transfer.post_destination()),
        (3, 10)
    );

    let unwrap = plan_fractional_exposure_token_effect_v2(
        terms,
        request(FractionalExposureActionV2::WholeUnwrap, 23),
        behavior,
        FractionalExposureTokenObservationV2 {
            root_controller: root,
            mint: Some(snapshot(MINTS[1], &mint)),
            source: Some(snapshot(SOURCE, &source)),
            destination: None,
            pre_supply: 30,
            pre_source: 23,
            pre_destination: 0,
        },
    )
    .expect("whole unwrap");
    assert!(matches!(
        unwrap.effect(),
        FractionalExposureTokenEffectV2::Burn(_)
    ));
    assert_eq!(unwrap.consumed_shards(), 20);
    assert_eq!(unwrap.change_shards(), 3);
    assert_eq!(unwrap.division().expect("division").whole_claims, 2);
    assert_eq!((unwrap.pre_source(), unwrap.post_source()), (23, 3));
    assert_eq!((unwrap.pre_supply(), unwrap.post_supply()), (30, 10));
}

#[test]
fn terminal_burn_is_exact_but_settlement_remains_a_separate_claims_gate() {
    let encoded_terms = terms_bytes();
    let terms = terms(&encoded_terms);
    let selection = TokenBehaviorSelectionV2::new(REALM, RELEASE)
        .expect("selection")
        .to_bytes();
    let behavior =
        authenticate_fractional_token_behavior_v2(terms, REALM, &selection, behavior_admission())
            .expect("behavior");
    let root = Pubkey::new_from_array(ROOT);
    let mint = behavior_mint(root, 30, 0);
    let source = token_account(
        Pubkey::new_from_array(MINTS[1]),
        Pubkey::new_from_array(OWNER),
        23,
    );
    for action in [
        FractionalExposureActionV2::TerminalRedeem,
        FractionalExposureActionV2::TerminalZeroBurn,
    ] {
        let plan = plan_fractional_exposure_token_effect_v2(
            terms,
            request(action, 23),
            behavior,
            FractionalExposureTokenObservationV2 {
                root_controller: root,
                mint: Some(snapshot(MINTS[1], &mint)),
                source: Some(snapshot(SOURCE, &source)),
                destination: None,
                pre_supply: 30,
                pre_source: 23,
                pre_destination: 0,
            },
        )
        .expect("terminal burn candidate");
        assert_eq!(plan.consumed_shards(), 20);
        assert_eq!(plan.change_shards(), 3);
    }

    let mut wrong_mint = snapshot(MINTS[1], &mint);
    wrong_mint.key = Pubkey::new_from_array([99; 32]);
    assert_eq!(
        plan_fractional_exposure_token_effect_v2(
            terms,
            request(FractionalExposureActionV2::TerminalRedeem, 23),
            behavior,
            FractionalExposureTokenObservationV2 {
                root_controller: root,
                mint: Some(wrong_mint),
                source: Some(snapshot(SOURCE, &source)),
                destination: None,
                pre_supply: 30,
                pre_source: 23,
                pre_destination: 0,
            },
        ),
        Err(Error::Token)
    );
}

#[test]
fn retirement_closes_all_k_mints_in_terms_order_only_at_zero_supply() {
    let encoded_terms = terms_bytes();
    let terms = terms(&encoded_terms);
    let selection = TokenBehaviorSelectionV2::new(REALM, RELEASE)
        .expect("selection")
        .to_bytes();
    let behavior =
        authenticate_fractional_token_behavior_v2(terms, REALM, &selection, behavior_admission())
            .expect("behavior");
    let root = Pubkey::new_from_array(ROOT);
    let rent_credit = Pubkey::new_from_array([61; 32]);
    let context = FractionalExposureRetirementContextV2 {
        root_controller: root,
        rent_credit,
        current_core_program: Pubkey::new_from_array([62; 32]),
    };
    let mint_data = [
        behavior_mint(root, 0, 0),
        behavior_mint(root, 0, 1),
        behavior_mint(root, 0, 2),
    ];
    let mints = [
        FractionalExposureMintSnapshotV2 {
            representation_coordinate: 0,
            mint: snapshot(MINTS[0], &mint_data[0]),
        },
        FractionalExposureMintSnapshotV2 {
            representation_coordinate: 1,
            mint: snapshot(MINTS[1], &mint_data[1]),
        },
        FractionalExposureMintSnapshotV2 {
            representation_coordinate: 2,
            mint: snapshot(MINTS[2], &mint_data[2]),
        },
    ];
    let plan = plan_fractional_exposure_retirement_v2(
        terms,
        request(FractionalExposureActionV2::ZeroSupplyRetire, 0),
        behavior,
        context,
        &mints,
    )
    .expect("zero-supply retirement");
    assert_eq!(plan.instructions().len(), 3);
    assert_eq!(plan.post_revision(), 8);
    for (instruction, expected_mint) in plan.instructions().iter().zip(MINTS) {
        assert_eq!(instruction.program_id.to_bytes(), TOKEN_2022_PROGRAM_ID);
        assert_eq!(instruction.accounts[0].pubkey.to_bytes(), expected_mint);
        assert_eq!(instruction.accounts[1].pubkey, rent_credit);
        assert_eq!(instruction.accounts[2].pubkey, root);
    }

    let hostile_data = [
        behavior_mint(root, 0, 0),
        behavior_mint(root, 1, 1),
        behavior_mint(root, 0, 2),
    ];
    let hostile = [
        FractionalExposureMintSnapshotV2 {
            representation_coordinate: 0,
            mint: snapshot(MINTS[0], &hostile_data[0]),
        },
        FractionalExposureMintSnapshotV2 {
            representation_coordinate: 1,
            mint: snapshot(MINTS[1], &hostile_data[1]),
        },
        FractionalExposureMintSnapshotV2 {
            representation_coordinate: 2,
            mint: snapshot(MINTS[2], &hostile_data[2]),
        },
    ];
    assert_eq!(
        plan_fractional_exposure_retirement_v2(
            terms,
            request(FractionalExposureActionV2::ZeroSupplyRetire, 0),
            behavior,
            context,
            &hostile,
        ),
        Err(Error::Token)
    );
}
