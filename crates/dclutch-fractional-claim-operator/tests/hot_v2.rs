//! Chain-derived AccountProfile projection into the Fractional Hot contract.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_contract::{
    FractionalExposureActionV2, FractionalExposureRequestInputV2, FractionalExposureRequestV2,
    FractionalHotAccountRefV2, FractionalHotTokenKindV2,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsInputV2, FractionalExposureTermsV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use dclutch_fractional_claim_operator::{
    Error, FractionalExposureTokenObservationV2, FractionalHotProfileV2,
    FractionalHotTokenCoordinatesV2, FractionalTokenAccountSnapshotV1,
    FractionalTokenBehaviorRecordAdmissionV2, authenticate_fractional_token_behavior_v2,
    lower_fractional_hot_token_effect_v2, plan_fractional_exposure_token_effect_v2,
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
const DESTINATION: [u8; 32] = [11; 32];
const ROOT: [u8; 32] = [31; 32];
const MINTS: [[u8; 32]; 3] = [[21; 32], [22; 32], [23; 32]];

fn encoded_terms() -> Vec<u8> {
    let width = fractional_exposure_terms_bytes_v2(MINTS.len()).unwrap();
    let mut scratch = vec![0; width];
    let mut output = vec![0; width];
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
    .unwrap();
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
    .unwrap()
}

fn request() -> FractionalExposureRequestV2 {
    FractionalExposureRequestV2::new(
        FractionalExposureActionV2::Wrap,
        FractionalExposureRequestInputV2 {
            release_set: RELEASE,
            market: MARKET,
            product_record: PRODUCT,
            result_domain: DOMAIN,
            terms: TERMS,
            token_behavior: TOKEN_BEHAVIOR,
            exposure: EXPOSURE,
            owner: OWNER,
            source_token_account: [0; 32],
            destination_token_account: DESTINATION,
            terminal_digest: [0; 32],
            expected_revision: 7,
            quantity: 2,
            representation_coordinate: 1,
        },
    )
    .unwrap()
}

fn mint_bytes(controller: [u8; 32], supply: u64) -> Vec<u8> {
    let mut data = vec![0; 238];
    data[0..4].copy_from_slice(&1_u32.to_le_bytes());
    data[4..36].copy_from_slice(&controller);
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = 9;
    data[45] = 1;
    data[165] = 1;
    data[166..168].copy_from_slice(&3_u16.to_le_bytes());
    data[168..170].copy_from_slice(&32_u16.to_le_bytes());
    data[170..202].copy_from_slice(&controller);
    data[202..204].copy_from_slice(&28_u16.to_le_bytes());
    data[204..206].copy_from_slice(&32_u16.to_le_bytes());
    data[206..238].copy_from_slice(&controller);
    data
}

fn token_account(mint: [u8; 32], owner: [u8; 32], amount: u64) -> [u8; 165] {
    let mut data = [0; 165];
    data[0..32].copy_from_slice(&mint);
    data[32..64].copy_from_slice(&owner);
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    data
}

#[test]
fn n258_k3_wrap_projects_exact_account_coordinates_and_poststate() {
    let encoded = encoded_terms();
    let terms = terms(&encoded);
    let behavior_bytes = TokenBehaviorSelectionV2::new(REALM, RELEASE)
        .unwrap()
        .to_bytes();
    let behavior = authenticate_fractional_token_behavior_v2(
        terms,
        REALM,
        &behavior_bytes,
        FractionalTokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: TOKEN_BEHAVIOR,
            finalized_content_digest: TOKEN_BEHAVIOR,
            recomputed_content_digest: TOKEN_BEHAVIOR,
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )
    .unwrap();
    let mint = mint_bytes(ROOT, 30);
    let destination = token_account(MINTS[1], OWNER, 3);
    let plan = plan_fractional_exposure_token_effect_v2(
        terms,
        request(),
        behavior,
        FractionalExposureTokenObservationV2 {
            root_controller: Pubkey::new_from_array(ROOT),
            mint: Some(FractionalTokenAccountSnapshotV1 {
                key: Pubkey::new_from_array(MINTS[1]),
                program_owner: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
                data: &mint,
            }),
            source: None,
            destination: Some(FractionalTokenAccountSnapshotV1 {
                key: Pubkey::new_from_array(DESTINATION),
                program_owner: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
                data: &destination,
            }),
            pre_supply: 30,
            pre_source: 0,
            pre_destination: 3,
        },
    )
    .unwrap();
    let keys = [ROOT, TOKEN_2022_PROGRAM_ID, MINTS[1], DESTINATION];
    let profile = FractionalHotProfileV2::new(&keys).unwrap();
    let effect = lower_fractional_hot_token_effect_v2(
        profile,
        terms,
        request(),
        FractionalHotAccountRefV2::new(0, ROOT).unwrap(),
        &plan,
        FractionalHotTokenCoordinatesV2 {
            token_program: 1,
            mint: 2,
            source: None,
            destination: Some(3),
            authority: 0,
        },
    )
    .unwrap();
    assert_eq!(effect.kind, FractionalHotTokenKindV2::Mint);
    assert_eq!(
        (effect.amount, effect.pre_supply, effect.post_supply),
        (20, 30, 50)
    );
    assert_eq!((effect.pre_destination, effect.post_destination), (3, 23));

    assert_eq!(
        lower_fractional_hot_token_effect_v2(
            profile,
            terms,
            request(),
            FractionalHotAccountRefV2::new(0, ROOT).unwrap(),
            &plan,
            FractionalHotTokenCoordinatesV2 {
                token_program: 1,
                mint: 3,
                source: None,
                destination: Some(2),
                authority: 0,
            },
        ),
        Err(Error::AccountFrame)
    );
}
