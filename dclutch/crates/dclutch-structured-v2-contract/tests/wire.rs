//! Round trip and hostile-decode evidence for the Structured V2 wire records.

mod support;

use dclutch_structured_v2_contract::{
    StructuredActionV2, StructuredRequestErrorV2, StructuredRequestInputV2, StructuredRequestV2,
    StructuredRootInputV2, StructuredRootV2,
};
use dclutch_structured_v2_kernel::{STRUCTURED_REQUEST_BYTES_V2, STRUCTURED_ROOT_BYTES_V2};
use support::{
    DENOMINATOR_FIXTURE, MARKET, OWNER, PRODUCT_RECORD, RECEIPT_DESTINATION, RECEIPT_SOURCE,
    RECEIPT_TOKEN_BEHAVIOR, RELEASE_SET, RENT_BENEFICIARY, RESULT_DOMAIN, SHARD_EXPOSURE,
    TERMINAL_DIGEST, digest, identity, root_bytes, shard_terms_bytes, structured_terms_bytes,
};

fn request_input() -> StructuredRequestInputV2 {
    let terms_bytes = structured_terms_bytes(&[1, 3], DENOMINATOR_FIXTURE);
    StructuredRequestInputV2 {
        release_set: identity(RELEASE_SET),
        market: identity(MARKET),
        product_record: identity(PRODUCT_RECORD),
        result_domain: identity(RESULT_DOMAIN),
        terms: digest(&terms_bytes),
        token_behavior: identity(RECEIPT_TOKEN_BEHAVIOR),
        shard_terms: digest(&shard_terms_bytes(2, DENOMINATOR_FIXTURE)),
        shard_exposure: identity(SHARD_EXPOSURE),
        owner: identity(OWNER),
        receipt_source: [0; 32],
        receipt_destination: identity(RECEIPT_DESTINATION),
        terminal_digest: [0; 32],
        expected_revision: 7,
        quantity: 4,
    }
}

#[test]
fn issue_request_round_trips_exactly() {
    let request = StructuredRequestV2::new(StructuredActionV2::Issue, request_input())
        .expect("issue request");
    let bytes = request.to_bytes().expect("encode");
    assert_eq!(bytes.len(), STRUCTURED_REQUEST_BYTES_V2);
    assert_eq!(StructuredRequestV2::decode(&bytes), Ok(request));
}

#[test]
fn terminal_redeem_request_round_trips_exactly() {
    let request = StructuredRequestV2::new(
        StructuredActionV2::TerminalRedeem,
        StructuredRequestInputV2 {
            receipt_source: identity(RECEIPT_SOURCE),
            receipt_destination: [0; 32],
            terminal_digest: identity(TERMINAL_DIGEST),
            ..request_input()
        },
    )
    .expect("terminal request");
    let bytes = request.to_bytes().expect("encode");
    assert_eq!(StructuredRequestV2::decode(&bytes), Ok(request));
}

#[test]
fn retire_request_carries_no_quantity_and_no_owner() {
    let request = StructuredRequestV2::new(
        StructuredActionV2::ZeroSupplyRetire,
        StructuredRequestInputV2 {
            owner: [0; 32],
            receipt_source: [0; 32],
            receipt_destination: [0; 32],
            terminal_digest: identity(TERMINAL_DIGEST),
            quantity: 0,
            ..request_input()
        },
    )
    .expect("retire request");
    let bytes = request.to_bytes().expect("encode");
    assert_eq!(StructuredRequestV2::decode(&bytes), Ok(request));
}

#[test]
fn request_shape_refusals_are_explicit() {
    // Quantity presence must match the action.
    assert_eq!(
        StructuredRequestV2::new(
            StructuredActionV2::Issue,
            StructuredRequestInputV2 {
                quantity: 0,
                ..request_input()
            }
        ),
        Err(StructuredRequestErrorV2::InvalidQuantity)
    );
    assert_eq!(
        StructuredRequestV2::new(
            StructuredActionV2::ZeroSupplyRetire,
            StructuredRequestInputV2 {
                owner: [0; 32],
                receipt_destination: [0; 32],
                terminal_digest: identity(TERMINAL_DIGEST),
                ..request_input()
            }
        ),
        Err(StructuredRequestErrorV2::InvalidQuantity)
    );
    // Terminal evidence presence must match the action.
    assert_eq!(
        StructuredRequestV2::new(
            StructuredActionV2::Issue,
            StructuredRequestInputV2 {
                terminal_digest: identity(TERMINAL_DIGEST),
                ..request_input()
            }
        ),
        Err(StructuredRequestErrorV2::InvalidTerminal)
    );
    assert_eq!(
        StructuredRequestV2::new(
            StructuredActionV2::TerminalRedeem,
            StructuredRequestInputV2 {
                receipt_source: identity(RECEIPT_SOURCE),
                receipt_destination: [0; 32],
                ..request_input()
            }
        ),
        Err(StructuredRequestErrorV2::InvalidTerminal)
    );
    // A zero identity refuses.
    assert_eq!(
        StructuredRequestV2::new(
            StructuredActionV2::Issue,
            StructuredRequestInputV2 {
                shard_terms: [0; 32],
                ..request_input()
            }
        ),
        Err(StructuredRequestErrorV2::InvalidIdentity)
    );
    // The receipt account required by the action must be present.
    assert_eq!(
        StructuredRequestV2::new(
            StructuredActionV2::Issue,
            StructuredRequestInputV2 {
                receipt_destination: [0; 32],
                ..request_input()
            }
        ),
        Err(StructuredRequestErrorV2::InvalidIdentity)
    );
    assert_eq!(
        StructuredRequestV2::new(
            StructuredActionV2::Unwrap,
            StructuredRequestInputV2 {
                receipt_source: [0; 32],
                receipt_destination: [0; 32],
                ..request_input()
            }
        ),
        Err(StructuredRequestErrorV2::InvalidIdentity)
    );
}

#[test]
fn hostile_request_bytes_refuse() {
    let request = StructuredRequestV2::new(StructuredActionV2::Issue, request_input())
        .expect("issue request");
    let accepted = request.to_bytes().expect("encode");

    let mut wrong_magic = accepted;
    if let Some(byte) = wrong_magic.first_mut() {
        *byte ^= 0xff;
    }
    assert_eq!(
        StructuredRequestV2::decode(&wrong_magic),
        Err(StructuredRequestErrorV2::InvalidHeader)
    );

    let mut wrong_version = accepted;
    if let Some(byte) = wrong_version.get_mut(8) {
        *byte = 9;
    }
    assert_eq!(
        StructuredRequestV2::decode(&wrong_version),
        Err(StructuredRequestErrorV2::InvalidHeader)
    );

    let mut unknown_action = accepted;
    if let Some(byte) = unknown_action.get_mut(10) {
        *byte = 9;
    }
    assert_eq!(
        StructuredRequestV2::decode(&unknown_action),
        Err(StructuredRequestErrorV2::UnknownAction)
    );

    let mut dirty_header = accepted;
    if let Some(byte) = dirty_header.get_mut(12) {
        *byte = 1;
    }
    assert_eq!(
        StructuredRequestV2::decode(&dirty_header),
        Err(StructuredRequestErrorV2::NonCanonical)
    );

    let mut dirty_tail = accepted;
    if let Some(byte) = dirty_tail.get_mut(STRUCTURED_REQUEST_BYTES_V2 - 1) {
        *byte = 1;
    }
    assert_eq!(
        StructuredRequestV2::decode(&dirty_tail),
        Err(StructuredRequestErrorV2::NonCanonical)
    );

    assert_eq!(
        StructuredRequestV2::decode(accepted.get(..431).expect("truncate")),
        Err(StructuredRequestErrorV2::InvalidLength)
    );
    let mut extended = accepted.to_vec();
    extended.push(0);
    assert_eq!(
        StructuredRequestV2::decode(&extended),
        Err(StructuredRequestErrorV2::InvalidLength)
    );
}

#[test]
fn root_round_trips_and_advances_by_exactly_one() {
    let terms_bytes = structured_terms_bytes(&[1, 3], DENOMINATOR_FIXTURE);
    let bytes = root_bytes(digest(&terms_bytes), 7);
    assert_eq!(bytes.len(), STRUCTURED_ROOT_BYTES_V2);
    let root = StructuredRootV2::decode(&bytes).expect("root");
    assert_eq!(root.input().revision, 7);
    assert_eq!(root.input().rent_beneficiary, identity(RENT_BENEFICIARY));
    assert_eq!(root.to_bytes().to_vec(), bytes);
    let advanced = root.advanced().expect("advance");
    assert_eq!(advanced.input().revision, 8);
    assert_eq!(
        StructuredRootInputV2 {
            revision: 7,
            ..advanced.input()
        },
        root.input()
    );
}

#[test]
fn hostile_root_bytes_refuse() {
    let terms_bytes = structured_terms_bytes(&[1, 3], DENOMINATOR_FIXTURE);
    let accepted = root_bytes(digest(&terms_bytes), 7);

    let mut wrong_magic = accepted.clone();
    if let Some(byte) = wrong_magic.first_mut() {
        *byte ^= 0xff;
    }
    assert_eq!(StructuredRootV2::decode(&wrong_magic), None);

    let mut wrong_version = accepted.clone();
    if let Some(byte) = wrong_version.get_mut(8) {
        *byte = 9;
    }
    assert_eq!(StructuredRootV2::decode(&wrong_version), None);

    let mut dirty_reserved = accepted.clone();
    if let Some(byte) = dirty_reserved.get_mut(12) {
        *byte = 1;
    }
    assert_eq!(StructuredRootV2::decode(&dirty_reserved), None);

    let mut zero_beneficiary = accepted.clone();
    if let Some(span) = zero_beneficiary.get_mut(80..112) {
        span.fill(0);
    }
    assert_eq!(StructuredRootV2::decode(&zero_beneficiary), None);

    let mut zero_principal = accepted.clone();
    if let Some(span) = zero_principal.get_mut(120..128) {
        span.fill(0);
    }
    assert_eq!(StructuredRootV2::decode(&zero_principal), None);

    assert_eq!(
        StructuredRootV2::decode(accepted.get(..127).expect("truncate")),
        None
    );
    let mut extended = accepted;
    extended.push(0);
    assert_eq!(StructuredRootV2::decode(&extended), None);
}
