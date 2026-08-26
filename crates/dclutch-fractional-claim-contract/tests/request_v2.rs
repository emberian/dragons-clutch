//! Hostile corpus for the exposure-bound Fractional request.

use dclutch_fractional_claim_contract::{
    FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2, FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_ID_V2,
    FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_PREIMAGE_V2, FractionalExposureActionV2,
    FractionalExposureRequestErrorV2, FractionalExposureRequestInputV2,
    FractionalExposureRequestV2, NO_EXPOSURE_COORDINATE_V2,
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
const TOKEN_BEHAVIOR: [u8; 32] = [6; 32];
const EXPOSURE: [u8; 32] = [7; 32];
const OWNER: [u8; 32] = [8; 32];
const SOURCE: [u8; 32] = [9; 32];
const DESTINATION: [u8; 32] = [10; 32];
const TERMINAL: [u8; 32] = [11; 32];
const MINTS: [[u8; 32]; 3] = [[21; 32], [22; 32], [23; 32]];

fn open_input(action: FractionalExposureActionV2) -> FractionalExposureRequestInputV2 {
    FractionalExposureRequestInputV2 {
        release_set: RELEASE,
        market: MARKET,
        product_record: PRODUCT,
        result_domain: DOMAIN,
        terms: TERMS,
        token_behavior: TOKEN_BEHAVIOR,
        exposure: EXPOSURE,
        owner: OWNER,
        source_token_account: if matches!(
            action,
            FractionalExposureActionV2::Transfer | FractionalExposureActionV2::WholeUnwrap
        ) {
            SOURCE
        } else {
            [0; 32]
        },
        destination_token_account: if matches!(
            action,
            FractionalExposureActionV2::Wrap | FractionalExposureActionV2::Transfer
        ) {
            DESTINATION
        } else {
            [0; 32]
        },
        terminal_digest: [0; 32],
        expected_revision: 17,
        quantity: 25,
        representation_coordinate: 2,
    }
}

fn terminal_input(action: FractionalExposureActionV2) -> FractionalExposureRequestInputV2 {
    let actor_bound = matches!(
        action,
        FractionalExposureActionV2::TerminalRedeem | FractionalExposureActionV2::TerminalZeroBurn
    );
    FractionalExposureRequestInputV2 {
        release_set: RELEASE,
        market: MARKET,
        product_record: PRODUCT,
        result_domain: DOMAIN,
        terms: TERMS,
        token_behavior: TOKEN_BEHAVIOR,
        exposure: EXPOSURE,
        owner: if actor_bound { OWNER } else { [0; 32] },
        source_token_account: if actor_bound { SOURCE } else { [0; 32] },
        destination_token_account: [0; 32],
        terminal_digest: TERMINAL,
        expected_revision: 17,
        quantity: if actor_bound { 25 } else { 0 },
        representation_coordinate: if actor_bound {
            2
        } else {
            NO_EXPOSURE_COORDINATE_V2
        },
    }
}

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
            token_program: [31; 32],
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

fn decoded_terms(bytes: &[u8]) -> FractionalExposureTermsV2<'_> {
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

#[test]
fn schema_and_all_action_shapes_round_trip_exactly() {
    assert_eq!(
        Sha256::digest(FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_PREIMAGE_V2).as_slice(),
        FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_ID_V2
    );
    for action in [
        FractionalExposureActionV2::Wrap,
        FractionalExposureActionV2::Transfer,
        FractionalExposureActionV2::WholeUnwrap,
    ] {
        let request =
            FractionalExposureRequestV2::new(action, open_input(action)).expect("open request");
        let bytes = request.to_bytes().expect("encode request");
        assert_eq!(bytes.len(), FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2);
        assert_eq!(FractionalExposureRequestV2::decode(&bytes), Ok(request));
    }
    for action in [
        FractionalExposureActionV2::TerminalRedeem,
        FractionalExposureActionV2::TerminalZeroBurn,
        FractionalExposureActionV2::Terminalize,
        FractionalExposureActionV2::ZeroSupplyRetire,
    ] {
        let request = FractionalExposureRequestV2::new(action, terminal_input(action))
            .expect("terminal request");
        assert_eq!(
            FractionalExposureRequestV2::decode(&request.to_bytes().expect("encode request")),
            Ok(request)
        );
    }
}

#[test]
fn request_binds_distinct_k_and_n_coordinates_to_terms() {
    let terms_bytes = terms_bytes();
    let terms = decoded_terms(&terms_bytes);
    let terminal = FractionalExposureRequestV2::new(
        FractionalExposureActionV2::TerminalRedeem,
        terminal_input(FractionalExposureActionV2::TerminalRedeem),
    )
    .expect("terminal request");
    assert_eq!(terminal.bind_terms(terms), Ok(terminal));

    let mut invalid_k = terminal.input();
    invalid_k.representation_coordinate = 3;
    let invalid_k =
        FractionalExposureRequestV2::new(FractionalExposureActionV2::TerminalRedeem, invalid_k)
            .expect("shape remains canonical");
    assert_eq!(
        invalid_k.bind_terms(terms),
        Err(FractionalExposureRequestErrorV2::InvalidCoordinate)
    );

    let mut substituted = terminal.input();
    substituted.exposure = [99; 32];
    let substituted =
        FractionalExposureRequestV2::new(FractionalExposureActionV2::TerminalRedeem, substituted)
            .expect("shape remains canonical");
    assert_eq!(
        substituted.bind_terms(terms),
        Err(FractionalExposureRequestErrorV2::TermsMismatch)
    );
}

#[test]
fn caller_cannot_supply_payout_or_noncanonical_activity() {
    let transfer = FractionalExposureRequestV2::new(
        FractionalExposureActionV2::Transfer,
        open_input(FractionalExposureActionV2::Transfer),
    )
    .expect("transfer request");
    let mut bytes = transfer.to_bytes().expect("encode transfer");
    *bytes.get_mut(415).expect("reserved byte") = 1;
    assert_eq!(
        FractionalExposureRequestV2::decode(&bytes),
        Err(FractionalExposureRequestErrorV2::NonCanonical)
    );

    let mut aliased = open_input(FractionalExposureActionV2::Transfer);
    aliased.destination_token_account = SOURCE;
    assert_eq!(
        FractionalExposureRequestV2::new(FractionalExposureActionV2::Transfer, aliased),
        Err(FractionalExposureRequestErrorV2::InvalidIdentity)
    );

    let mut terminal = terminal_input(FractionalExposureActionV2::TerminalRedeem);
    terminal.terminal_digest = [0; 32];
    assert_eq!(
        FractionalExposureRequestV2::new(FractionalExposureActionV2::TerminalRedeem, terminal,),
        Err(FractionalExposureRequestErrorV2::InvalidTerminal)
    );

    let mut retire = terminal_input(FractionalExposureActionV2::ZeroSupplyRetire);
    retire.owner = OWNER;
    assert_eq!(
        FractionalExposureRequestV2::new(FractionalExposureActionV2::ZeroSupplyRetire, retire,),
        Err(FractionalExposureRequestErrorV2::InvalidIdentity)
    );
}
