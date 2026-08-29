//! Fractional V3 conservation/topology and ordered-retirement hostile corpus.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use dclutch_fractional_claim_contract::{
    FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3, FRACTIONAL_RETIREMENT_CURSOR_SCHEMA_ID_V3,
    FRACTIONAL_RETIREMENT_CURSOR_SCHEMA_PREIMAGE_V3, FRACTIONAL_RETIREMENT_REQUEST_SCHEMA_ID_V3,
    FRACTIONAL_RETIREMENT_REQUEST_SCHEMA_PREIMAGE_V3, FractionalChildRouteV3,
    FractionalExposureActionV2, FractionalExposureRequestInputV2, FractionalExposureRequestV2,
    FractionalPhysicalErrorV3, FractionalRetireCoordinateObservationV3,
    FractionalRetirementActionV3, FractionalRetirementCursorInputV3, FractionalRetirementCursorV3,
    FractionalRetirementErrorV3, FractionalRetirementRequestInputV3, FractionalRetirementRequestV3,
    FractionalSignerRoleV3, NO_EXPOSURE_COORDINATE_V2, NO_RETIREMENT_COORDINATE_V3,
    plan_fractional_physical_v3,
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
const BEHAVIOR: [u8; 32] = [7; 32];
const EXPOSURE: [u8; 32] = [8; 32];
const ROOT: [u8; 32] = [9; 32];
const RENT: [u8; 32] = [10; 32];
const OWNER: [u8; 32] = [11; 32];
const SOURCE: [u8; 32] = [12; 32];
const DESTINATION: [u8; 32] = [13; 32];
const TERMINAL: [u8; 32] = [14; 32];
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
            token_program: TOKEN,
            token_behavior: BEHAVIOR,
            exposure_id: EXPOSURE,
            product_basis: [31; 32],
            representation_basis: [32; 32],
            graph_id: [33; 32],
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

fn request(action: FractionalExposureActionV2, quantity: u64) -> FractionalExposureRequestV2 {
    let carries = action.carries_quantity();
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
            token_behavior: BEHAVIOR,
            exposure: EXPOSURE,
            owner: if carries { OWNER } else { [0; 32] },
            source_token_account: if source { SOURCE } else { [0; 32] },
            destination_token_account: if destination { DESTINATION } else { [0; 32] },
            terminal_digest: if action.requires_terminal() {
                TERMINAL
            } else {
                [0; 32]
            },
            expected_revision: 7,
            quantity,
            representation_coordinate: if carries {
                1
            } else {
                NO_EXPOSURE_COORDINATE_V2
            },
        },
    )
    .unwrap()
}

fn retirement_request(
    action: FractionalRetirementActionV3,
    revision: u64,
    coordinate: u32,
) -> FractionalRetirementRequestV3 {
    FractionalRetirementRequestV3::new(
        action,
        FractionalRetirementRequestInputV3 {
            release_set: RELEASE,
            market: MARKET,
            terms: TERMS,
            token_program: TOKEN,
            token_behavior: BEHAVIOR,
            exposure: EXPOSURE,
            root: ROOT,
            rent_credit: RENT,
            expected_revision: revision,
            representation_coordinate: coordinate,
        },
    )
    .unwrap()
}

fn observation(coordinate: usize) -> FractionalRetireCoordinateObservationV3 {
    FractionalRetireCoordinateObservationV3 {
        shard_mint: MINTS[coordinate],
        shard_supply: 0,
        reserve_claims: 0,
        mint_authenticated: true,
        reserve_authenticated: true,
    }
}

#[test]
fn retirement_schema_identities_match_exact_preimages() {
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(
            FRACTIONAL_RETIREMENT_CURSOR_SCHEMA_PREIMAGE_V3
        )),
        FRACTIONAL_RETIREMENT_CURSOR_SCHEMA_ID_V3
    );
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(
            FRACTIONAL_RETIREMENT_REQUEST_SCHEMA_PREIMAGE_V3
        )),
        FRACTIONAL_RETIREMENT_REQUEST_SCHEMA_ID_V3
    );
}

#[test]
fn selected_topology_preserves_exact_denominator_and_same_mint_remainder() {
    let bytes = encoded_terms();
    let terms = terms(&bytes);
    let wrap =
        plan_fractional_physical_v3(terms, request(FractionalExposureActionV2::Wrap, 3)).unwrap();
    assert_eq!(
        (wrap.whole_claims, wrap.consumed_shards, wrap.change_shards),
        (3, 30, 0)
    );
    assert_eq!(wrap.route, FractionalChildRouteV3::ClaimsTokenAtomic);
    assert_eq!(wrap.signer, FractionalSignerRoleV3::Holder);

    let unwrap =
        plan_fractional_physical_v3(terms, request(FractionalExposureActionV2::WholeUnwrap, 37))
            .unwrap();
    assert_eq!(
        (
            unwrap.whole_claims,
            unwrap.consumed_shards,
            unwrap.change_shards
        ),
        (3, 30, 7)
    );
    assert_eq!(
        unwrap.consumed_shards + unwrap.change_shards,
        37,
        "sole exact conservation boundary"
    );
    assert_eq!(unwrap.shard_mint, Some(MINTS[1]));

    let redeem = plan_fractional_physical_v3(
        terms,
        request(FractionalExposureActionV2::TerminalRedeem, 29),
    )
    .unwrap();
    assert_eq!(
        redeem.route,
        FractionalChildRouteV3::ClaimsTerminalTokenCustodyAtomic
    );
    assert_eq!(
        (
            redeem.whole_claims,
            redeem.consumed_shards,
            redeem.change_shards
        ),
        (2, 20, 9)
    );

    let transfer =
        plan_fractional_physical_v3(terms, request(FractionalExposureActionV2::Transfer, 7))
            .unwrap();
    assert_eq!(
        transfer.route,
        FractionalChildRouteV3::Token2022DirectTransfer
    );
    assert_eq!((transfer.whole_claims, transfer.consumed_shards), (0, 7));
}

#[test]
fn sub_denominator_burn_and_old_all_k_retirement_refuse() {
    let bytes = encoded_terms();
    let terms = terms(&bytes);
    assert_eq!(
        plan_fractional_physical_v3(terms, request(FractionalExposureActionV2::WholeUnwrap, 9),),
        Err(FractionalPhysicalErrorV3::Division)
    );
    assert_eq!(
        plan_fractional_physical_v3(
            terms,
            request(FractionalExposureActionV2::ZeroSupplyRetire, 0),
        ),
        Err(FractionalPhysicalErrorV3::UseOrderedRetirement)
    );
}

#[test]
fn ordered_retirement_roundtrips_advances_k_steps_and_finishes_fixed_width() {
    let bytes = encoded_terms();
    let terms = terms(&bytes);
    let begin = retirement_request(
        FractionalRetirementActionV3::Begin,
        7,
        NO_RETIREMENT_COORDINATE_V3,
    );
    let mut cursor = FractionalRetirementCursorV3::begin(
        terms,
        begin,
        FractionalRetirementCursorInputV3 {
            bump: 251,
            pre_revision: 7,
            historical_rent_principal: 2_039_280,
        },
    )
    .unwrap();
    assert_eq!((cursor.next_coordinate(), cursor.revision()), (0, 8));
    let encoded = cursor.to_bytes().unwrap();
    assert_eq!(encoded.len(), FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3);
    assert_eq!(FractionalRetirementCursorV3::decode(&encoded), Ok(cursor));

    for coordinate in 0..3_u32 {
        cursor = cursor
            .advance(
                terms,
                retirement_request(
                    FractionalRetirementActionV3::RetireCoordinate,
                    8 + u64::from(coordinate),
                    coordinate,
                ),
                observation(usize::try_from(coordinate).unwrap()),
            )
            .unwrap();
    }
    assert_eq!((cursor.next_coordinate(), cursor.revision()), (3, 11));
    let finish = cursor
        .finish(
            terms,
            retirement_request(
                FractionalRetirementActionV3::Finish,
                11,
                NO_RETIREMENT_COORDINATE_V3,
            ),
        )
        .unwrap();
    assert_eq!((finish.coordinate_count, finish.terminal_revision), (3, 12));
    assert_eq!(finish.cursor_rent_principal, 2_039_280);
}

#[test]
fn replay_skip_substitution_nonzero_and_stale_revision_refuse() {
    let bytes = encoded_terms();
    let terms = terms(&bytes);
    let cursor = FractionalRetirementCursorV3::begin(
        terms,
        retirement_request(
            FractionalRetirementActionV3::Begin,
            7,
            NO_RETIREMENT_COORDINATE_V3,
        ),
        FractionalRetirementCursorInputV3 {
            bump: 1,
            pre_revision: 7,
            historical_rent_principal: 1,
        },
    )
    .unwrap();
    let step0 = retirement_request(FractionalRetirementActionV3::RetireCoordinate, 8, 0);
    let advanced = cursor.advance(terms, step0, observation(0)).unwrap();
    assert_eq!(
        advanced.advance(terms, step0, observation(0)),
        Err(FractionalRetirementErrorV3::InvalidTransition)
    );
    assert_eq!(
        cursor.advance(
            terms,
            retirement_request(FractionalRetirementActionV3::RetireCoordinate, 8, 1),
            observation(1),
        ),
        Err(FractionalRetirementErrorV3::InvalidTransition)
    );
    for hostile in [
        FractionalRetireCoordinateObservationV3 {
            shard_mint: MINTS[1],
            ..observation(0)
        },
        FractionalRetireCoordinateObservationV3 {
            shard_supply: 1,
            ..observation(0)
        },
        FractionalRetireCoordinateObservationV3 {
            reserve_claims: 1,
            ..observation(0)
        },
        FractionalRetireCoordinateObservationV3 {
            mint_authenticated: false,
            ..observation(0)
        },
        FractionalRetireCoordinateObservationV3 {
            reserve_authenticated: false,
            ..observation(0)
        },
    ] {
        assert_eq!(
            cursor.advance(terms, step0, hostile),
            Err(FractionalRetirementErrorV3::InvalidTransition)
        );
    }
    assert_eq!(
        cursor.advance(
            terms,
            retirement_request(FractionalRetirementActionV3::RetireCoordinate, 9, 0),
            observation(0),
        ),
        Err(FractionalRetirementErrorV3::InvalidTransition)
    );
    assert_eq!(
        cursor.finish(
            terms,
            retirement_request(
                FractionalRetirementActionV3::Finish,
                8,
                NO_RETIREMENT_COORDINATE_V3,
            ),
        ),
        Err(FractionalRetirementErrorV3::InvalidTransition)
    );
}

#[test]
fn request_and_cursor_reserved_bytes_and_terms_substitution_refuse() {
    let bytes = encoded_terms();
    let terms = terms(&bytes);
    let request = retirement_request(
        FractionalRetirementActionV3::Begin,
        7,
        NO_RETIREMENT_COORDINATE_V3,
    );
    let mut request_bytes = request.to_bytes().unwrap();
    request_bytes[15] = 1;
    assert_eq!(
        FractionalRetirementRequestV3::decode(&request_bytes),
        Err(FractionalRetirementErrorV3::InvalidEncoding)
    );
    let cursor = FractionalRetirementCursorV3::begin(
        terms,
        request,
        FractionalRetirementCursorInputV3 {
            bump: 1,
            pre_revision: 7,
            historical_rent_principal: 1,
        },
    )
    .unwrap();
    let mut cursor_bytes = cursor.to_bytes().unwrap();
    cursor_bytes[12] = 1;
    assert_eq!(
        FractionalRetirementCursorV3::decode(&cursor_bytes),
        Err(FractionalRetirementErrorV3::InvalidEncoding)
    );

    let mut request_input = request.input();
    request_input.exposure = [99; 32];
    let substituted =
        FractionalRetirementRequestV3::new(FractionalRetirementActionV3::Begin, request_input)
            .unwrap();
    assert_eq!(
        FractionalRetirementCursorV3::begin(
            terms,
            substituted,
            FractionalRetirementCursorInputV3 {
                bump: 1,
                pre_revision: 7,
                historical_rent_principal: 1,
            },
        ),
        Err(FractionalRetirementErrorV3::IdentityMismatch)
    );
}
