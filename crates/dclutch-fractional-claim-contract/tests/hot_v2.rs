//! Onchain-safe Fractional V2 candidate corpus.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use dclutch_claims_svm::{
    CallerRole,
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
        SignedDeltaPlanV3, SignedDeltaPositionV3, SignedDeltaV3, plan_bytes,
    },
};
use dclutch_fractional_claim_contract::{
    FractionalExposureActionV2, FractionalExposureRequestInputV2, FractionalExposureRequestV2,
    FractionalHotAccountRefV2, FractionalHotCandidateInputV2, FractionalHotCandidateV2,
    FractionalHotClaimsEffectV2, FractionalHotErrorV2, FractionalHotTokenEffectV2,
    FractionalHotTokenKindV2, FractionalHotTokenPostV2, FractionalRootInputV1, FractionalRootV1,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsInputV2, FractionalExposureTermsV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use sha2::{Digest, Sha256};

const RELEASE: [u8; 32] = [1; 32];
const MARKET: [u8; 32] = [2; 32];
const PRODUCT: [u8; 32] = [3; 32];
const DOMAIN: [u8; 32] = [4; 32];
const TERMS: [u8; 32] = [5; 32];
const TOKEN: [u8; 32] = [6; 32];
const TOKEN_BEHAVIOR: [u8; 32] = [7; 32];
const EXPOSURE: [u8; 32] = [8; 32];
const BASIS: [u8; 32] = [9; 32];
const LINKED_BASIS: [u8; 32] = [10; 32];
const OWNER: [u8; 32] = [11; 32];
const ROOT: [u8; 32] = [31; 32];
const DESTINATION: [u8; 32] = [12; 32];
const MINTS: [[u8; 32]; 3] = [[21; 32], [22; 32], [23; 32]];

fn account(coordinate: u16, key: [u8; 32]) -> FractionalHotAccountRefV2 {
    FractionalHotAccountRefV2::new(coordinate, key).unwrap()
}

fn terms_bytes() -> Vec<u8> {
    let length = fractional_exposure_terms_bytes_v2(MINTS.len()).unwrap();
    let mut scratch = vec![0; length];
    let mut output = vec![0; length];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: MARKET,
            product_record: PRODUCT,
            result_domain: DOMAIN,
            release_set: RELEASE,
            token_program: TOKEN,
            token_behavior: TOKEN_BEHAVIOR,
            exposure_id: EXPOSURE,
            product_basis: LINKED_BASIS,
            representation_basis: BASIS,
            graph_id: [13; 32],
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

fn root_bytes() -> [u8; 128] {
    FractionalRootV1::new(FractionalRootInputV1 {
        bump: 4,
        terms: TERMS,
        market: MARKET,
        rent_beneficiary: [14; 32],
        revision: 7,
        historical_rent_principal: 99,
    })
    .unwrap()
    .to_bytes()
}

fn token_effect() -> FractionalHotTokenEffectV2 {
    FractionalHotTokenEffectV2 {
        kind: FractionalHotTokenKindV2::Mint,
        representation_coordinate: 1,
        token_program: account(2, TOKEN),
        mint: account(3, MINTS[1]),
        source: None,
        destination: Some(account(4, DESTINATION)),
        authority: account(0, ROOT),
        amount: 20,
        pre_supply: 30,
        post_supply: 50,
        pre_source: 0,
        post_source: 0,
        pre_destination: 3,
        post_destination: 23,
    }
}

fn packet() -> Vec<u8> {
    let request = request();
    let request_digest: [u8; 32] = Sha256::digest(request.to_bytes().unwrap()).into();
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).unwrap();
    let debit = SignedDeltaV3::new(DeltaDirectionV3::Debit, 2).unwrap();
    let credit = SignedDeltaV3::new(DeltaDirectionV3::Credit, 2).unwrap();
    let positions = [
        SignedDeltaPositionV3::new(OWNER, 5).unwrap(),
        SignedDeltaPositionV3::new(ROOT, 6).unwrap(),
    ];
    let rows = [
        PositionDeltaV3::new(
            PositionDeltaInputV3 {
                position_index: 0,
                outcome: 1,
                delta: debit,
            },
            2,
            3,
        )
        .unwrap(),
        PositionDeltaV3::new(
            PositionDeltaInputV3 {
                position_index: 1,
                outcome: 1,
                delta: credit,
            },
            2,
            3,
        )
        .unwrap(),
    ];
    let mut output = vec![0; plan_bytes(3, 2, 2).unwrap()];
    SignedDeltaPlanV3::encode_into(
        SignedDeltaPlanInputV3 {
            caller_role: CallerRole::Trading,
            release_set: RELEASE,
            market: MARKET,
            request_id: request_digest,
            product_record_digest: PRODUCT,
            semantic_basis_id: BASIS,
            linked_basis_record_digest: LINKED_BASIS,
            expected_market_revision: 8,
            claim_count: 3,
        },
        &positions,
        &[neutral; 3],
        &rows,
        &mut output,
    )
    .unwrap();
    output
}

#[test]
fn n258_k3_candidate_binds_token_claims_and_root_last() {
    let terms_bytes = terms_bytes();
    let terms = terms(&terms_bytes);
    let request = request();
    let token = [token_effect()];
    let packet = packet();
    let root_bytes = root_bytes();
    let candidate = FractionalHotCandidateV2::prepare(FractionalHotCandidateInputV2 {
        request,
        terms,
        root_bytes: &root_bytes,
        root: account(0, ROOT),
        token_effects: &token,
        claims: FractionalHotClaimsEffectV2::SignedDelta {
            claims_program: account(5, [15; 32]),
            route_base: 20,
            packet: &packet,
        },
        rent_close: None,
    })
    .unwrap();
    assert_eq!(
        (candidate.pre_revision(), candidate.post_revision()),
        (7, Some(8))
    );
    let root = FractionalRootV1::decode(&candidate.root_candidate_bytes().unwrap()).unwrap();
    assert_eq!(root.input().revision, 8);
    assert_eq!(root.input().historical_rent_principal, 99);
    assert_eq!(
        candidate.validate_token_poststate(&[FractionalHotTokenPostV2 {
            representation_coordinate: 1,
            mint: MINTS[1],
            supply: 50,
            source_amount: 0,
            destination_amount: 23,
        }]),
        Ok(())
    );
    assert_eq!(
        candidate.validate_root_poststate(Some(&root.to_bytes())),
        Ok(())
    );
}

#[test]
fn account_amount_direction_poststate_and_stale_root_substitutions_refuse() {
    let terms_bytes = terms_bytes();
    let terms = terms(&terms_bytes);
    let request = request();
    let packet = packet();
    let mut token = [token_effect()];
    token[0].amount = 19;
    assert_eq!(
        FractionalHotCandidateV2::prepare(FractionalHotCandidateInputV2 {
            request,
            terms,
            root_bytes: &root_bytes(),
            root: account(0, ROOT),
            token_effects: &token,
            claims: FractionalHotClaimsEffectV2::SignedDelta {
                claims_program: account(5, [15; 32]),
                route_base: 20,
                packet: &packet,
            },
            rent_close: None,
        }),
        Err(FractionalHotErrorV2::TokenMismatch)
    );

    let mut wrong_packet = packet.clone();
    let row_offset = wrong_packet.len() - 24;
    wrong_packet[row_offset + 8] = DeltaDirectionV3::Debit as u8;
    assert!(
        FractionalHotCandidateV2::prepare(FractionalHotCandidateInputV2 {
            request,
            terms,
            root_bytes: &root_bytes(),
            root: account(0, ROOT),
            token_effects: &[token_effect()],
            claims: FractionalHotClaimsEffectV2::SignedDelta {
                claims_program: account(5, [15; 32]),
                route_base: 20,
                packet: &wrong_packet,
            },
            rent_close: None,
        })
        .is_err()
    );

    let mut stale = root_bytes();
    stale[112..120].copy_from_slice(&6_u64.to_le_bytes());
    assert_eq!(
        FractionalHotCandidateV2::prepare(FractionalHotCandidateInputV2 {
            request,
            terms,
            root_bytes: &stale,
            root: account(0, ROOT),
            token_effects: &[token_effect()],
            claims: FractionalHotClaimsEffectV2::SignedDelta {
                claims_program: account(5, [15; 32]),
                route_base: 20,
                packet: &packet,
            },
            rent_close: None,
        }),
        Err(FractionalHotErrorV2::IdentityMismatch)
    );
}
