use crate::rational_kernel::{
    DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
    DescriptorAdmissionV2,
};

use super::*;

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn descriptor_bytes() -> std::vec::Vec<u8> {
    let mut output = std::vec![0; DESCRIPTOR_HEADER_BYTES + 3 * DESCRIPTOR_COEFFICIENT_BYTES];
    put(&mut output, 0, &DESCRIPTOR_MAGIC_V3).expect("magic");
    put(&mut output, 8, &3_u16.to_le_bytes()).expect("version");
    put(&mut output, 16, &id(3)).expect("graph id");
    put(&mut output, 48, &id(4)).expect("graph digest");
    put(&mut output, 80, &id(5)).expect("root");
    put(&mut output, 112, &id(6)).expect("market");
    put(&mut output, 144, &id(7)).expect("release");
    put(&mut output, 176, &id(8)).expect("receipt");
    put(&mut output, 208, &TOKEN_2022_PROGRAM_ID).expect("token");
    put(&mut output, 240, &3_u32.to_le_bytes()).expect("width");
    put(&mut output, 248, &10_u64.to_le_bytes()).expect("denominator");
    for (index, value) in [3_u64, 0, 7].iter().enumerate() {
        put(
            &mut output,
            DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
            &value.to_le_bytes(),
        )
        .expect("coefficient");
    }
    output
}

fn descriptor<'a>(bytes: &'a [u8]) -> RepresentationDescriptorV2<'a> {
    let digest = id(9);
    RepresentationDescriptorV2::decode(
        bytes,
        DescriptorAdmissionV2 {
            selected_descriptor_id: digest,
            finalized_descriptor_id: digest,
            recomputed_descriptor_digest: digest,
            finalized_descriptor_digest: digest,
            record_authenticated: true,
            derived_representation_authority: id(10),
            authority_derivation_authenticated: true,
        },
    )
    .expect("descriptor")
}

fn header(action: LifecycleActionV2, coordinate_count: u32) -> LifecycleHeaderV2 {
    let retirement = action.retires();
    let receipt_close = action == LifecycleActionV2::RetireReceipt;
    LifecycleHeaderV2 {
        action,
        release_set: id(7),
        market: id(6),
        graph_id: id(3),
        descriptor_id: id(9),
        parent_context: id(11),
        representation_authority: id(10),
        receipt_mint: id(8),
        token_program: TOKEN_2022_PROGRAM_ID,
        rent_credit: id(12),
        rent_program: id(13),
        generation: 14,
        expected_claims_market_revision: 15,
        observed_receipt_lamports: 120,
        receipt_rent_principal: 100,
        expected_receipt_supply: 0,
        outcome_count: 3,
        coordinate_count,
        rent_credit_before: 1_000,
        rent_credit_after: if receipt_close {
            1_120
        } else if retirement {
            1_460
        } else {
            1_000
        },
    }
}

fn coordinate(outcome: u32, coefficient: u64, vacancy: bool) -> LifecycleCoordinateV2 {
    let seed = u8::try_from(outcome).expect("small outcome") * 8;
    LifecycleCoordinateV2 {
        outcome,
        coefficient,
        shard_mint: id(20 + seed),
        structured_custody_account: id(21 + seed),
        claims_custody_owner: id(22 + seed),
        claims_custody_position: id(23 + seed),
        position_admission: id(24 + seed),
        observed_shard_lamports: if vacancy { 0 } else { 110 },
        observed_structured_lamports: if vacancy { 0 } else { 120 },
        observed_position_lamports: if vacancy { 0 } else { 130 },
        observed_admission_lamports: if vacancy { 0 } else { 100 },
        shard_rent_principal: if vacancy { 0 } else { 100 },
        structured_rent_principal: if vacancy { 0 } else { 100 },
        position_rent_principal: if vacancy { 0 } else { 100 },
        admission_rent_principal: if vacancy { 0 } else { 100 },
        expected_shard_supply: 0,
        expected_structured_amount: 0,
        expected_position_revision: if vacancy {
            ABSENT_POSITION_REVISION_V2
        } else {
            0
        },
    }
}

fn rows(values: &[LifecycleCoordinateV2]) -> std::vec::Vec<u8> {
    let mut output = std::vec![0; values.len() * LIFECYCLE_COORDINATE_BYTES_V2];
    for (index, value) in values.iter().copied().enumerate() {
        let start = index * LIFECYCLE_COORDINATE_BYTES_V2;
        let end = start + LIFECYCLE_COORDINATE_BYTES_V2;
        value
            .encode_into(output.get_mut(start..end).expect("row"))
            .expect("coordinate");
    }
    output
}

fn request_bytes(header: LifecycleHeaderV2, rows: &[LifecycleCoordinateV2]) -> std::vec::Vec<u8> {
    let encoded_rows = self::rows(rows);
    let request = LifecycleRequestV2::new(header, &encoded_rows).expect("request");
    let mut output = std::vec![0; LIFECYCLE_HEADER_BYTES_V2 + encoded_rows.len()];
    request.encode_into(&mut output).expect("request bytes");
    output
}

fn completion(prepared: PreparedLifecycleV2) -> LifecycleCompletionEvidenceV2 {
    let coordinate_action = matches!(
        prepared.action(),
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    );
    let before = 1_000;
    LifecycleCompletionEvidenceV2 {
        request_digest: id(50),
        descriptor_digest: id(9),
        post_resource_digest: id(51),
        position_lifecycle_receipt_digest: if coordinate_action { id(52) } else { [0; 32] },
        rent_credit_before: before,
        rent_credit_after: before + prepared.expected_credit(),
        caller_authenticated: true,
        descriptor_and_resources_authenticated: true,
        physical_effects_committed: true,
    }
}

#[test]
fn all_actions_roundtrip_and_finalize_state_last() {
    let descriptor_bytes = descriptor_bytes();
    let descriptor = descriptor(&descriptor_bytes);
    for (action, coordinates, expected_credit, selected) in [
        (
            LifecycleActionV2::ActivateReceipt,
            std::vec![],
            0,
            ABSENT_OUTCOME_V2,
        ),
        (
            LifecycleActionV2::ActivateCoordinate,
            std::vec![coordinate(2, 7, false)],
            0,
            2,
        ),
        (
            LifecycleActionV2::RetireCoordinate,
            std::vec![coordinate(2, 7, false)],
            460,
            2,
        ),
        (
            LifecycleActionV2::RetireReceipt,
            std::vec![coordinate(0, 3, true), coordinate(2, 7, true)],
            120,
            ABSENT_OUTCOME_V2,
        ),
    ] {
        let bytes = request_bytes(
            header(
                action,
                u32::try_from(coordinates.len()).expect("coordinate count"),
            ),
            &coordinates,
        );
        let request = LifecycleRequestV2::decode(&bytes).expect("roundtrip request");
        let prepared = prepare(request, descriptor).expect("prepare");
        assert_eq!(prepared.action(), action);
        assert_eq!(prepared.expected_credit(), expected_credit);
        assert_eq!(prepared.selected_outcome(), selected);
        let evidence = completion(prepared);
        let receipt = finalize(prepared, evidence).expect("completion");
        assert_eq!(receipt.action(), action);
        assert_eq!(receipt.outcome(), selected);
        assert_eq!(receipt.credited_lamports(), expected_credit);
        assert_eq!(
            LifecycleReceiptV2::decode(&receipt.to_bytes().expect("receipt bytes")),
            Ok(receipt)
        );
    }
}

#[test]
fn exact_nonzero_support_refuses_missing_extra_reordered_and_zero_rows() {
    let descriptor_bytes = descriptor_bytes();
    let descriptor = descriptor(&descriptor_bytes);
    let canonical = [coordinate(0, 3, true), coordinate(2, 7, true)];
    let canonical_bytes = request_bytes(header(LifecycleActionV2::RetireReceipt, 2), &canonical);
    assert!(
        prepare(
            LifecycleRequestV2::decode(&canonical_bytes).expect("canonical request"),
            descriptor,
        )
        .is_ok()
    );

    for hostile in [
        std::vec![coordinate(0, 3, true)],
        std::vec![
            coordinate(0, 3, true),
            coordinate(2, 7, true),
            coordinate(2, 7, true),
        ],
        std::vec![coordinate(2, 7, true), coordinate(0, 3, true)],
        std::vec![coordinate(0, 3, true), coordinate(1, 0, true)],
    ] {
        let hostile_bytes = request_bytes(
            header(
                LifecycleActionV2::RetireReceipt,
                u32::try_from(hostile.len()).expect("hostile count"),
            ),
            &hostile,
        );
        assert_eq!(
            prepare(
                LifecycleRequestV2::decode(&hostile_bytes).expect("structural request"),
                descriptor,
            ),
            Err(Error::InvalidSupport)
        );
    }
}

#[test]
fn rent_supply_vacancy_token_and_completion_refusals_are_exact() {
    let descriptor_bytes = descriptor_bytes();
    let descriptor = descriptor(&descriptor_bytes);
    let mut bad_token = header(LifecycleActionV2::ActivateReceipt, 0);
    bad_token.token_program = id(99);
    let bad_token_rows = rows(&[]);
    assert_eq!(
        LifecycleRequestV2::new(bad_token, &bad_token_rows),
        Err(Error::UnsupportedTokenProgram)
    );

    let mut underfunded = header(LifecycleActionV2::ActivateCoordinate, 1);
    let mut underfunded_coordinate = coordinate(2, 7, false);
    underfunded_coordinate.observed_shard_lamports =
        underfunded_coordinate.shard_rent_principal - 1;
    let underfunded_bytes = request_bytes(underfunded, &[underfunded_coordinate]);
    assert_eq!(
        prepare(
            LifecycleRequestV2::decode(&underfunded_bytes).expect("request"),
            descriptor,
        ),
        Err(Error::InvalidPhysicalState)
    );

    underfunded.rent_credit_after = underfunded.rent_credit_before + 1;
    let noncanonical_rows = rows(&[coordinate(2, 7, false)]);
    assert_eq!(
        LifecycleRequestV2::new(underfunded, &noncanonical_rows),
        Err(Error::NonCanonical)
    );

    let retire_bytes = request_bytes(
        header(LifecycleActionV2::RetireCoordinate, 1),
        &[coordinate(2, 7, false)],
    );
    let prepared = prepare(
        LifecycleRequestV2::decode(&retire_bytes).expect("retire request"),
        descriptor,
    )
    .expect("retire plan");
    let mut evidence = completion(prepared);
    evidence.physical_effects_committed = false;
    assert_eq!(finalize(prepared, evidence), Err(Error::InvalidCompletion));
}

#[test]
fn hostile_headers_reserved_bytes_and_truncation_refuse() {
    let canonical = request_bytes(header(LifecycleActionV2::ActivateReceipt, 0), &[]);
    for offset in [0_usize, 8, 11] {
        let mut hostile = canonical.clone();
        *hostile.get_mut(offset).expect("hostile offset") ^= 1;
        assert!(LifecycleRequestV2::decode(&hostile).is_err());
    }
    let mut unknown = canonical.clone();
    *unknown.get_mut(ACTION_OFFSET).expect("action") = 255;
    assert_eq!(
        LifecycleRequestV2::decode(&unknown),
        Err(Error::UnknownAction)
    );
    assert_eq!(
        LifecycleRequestV2::decode(
            canonical
                .get(..canonical.len() - 1)
                .expect("truncated request"),
        ),
        Err(Error::InvalidLength)
    );
}

/// The kind identity is the SHA-256 of its own preimage, not a chosen literal.
///
/// A capability kind is a Market-manifest key: an entry carrying the wrong 32
/// bytes selects nothing, or worse, collides with another family's entry. The
/// literal therefore has to be derivable by anyone from the domain string
/// beside it, and this recomputes it rather than trusting that it was pasted
/// correctly. Same control General keeps for its own kind.
#[test]
fn the_rational_lifecycle_capability_kind_is_the_digest_of_its_preimage() {
    use sha2::{Digest as _, Sha256};

    assert_eq!(
        Sha256::digest(RATIONAL_LIFECYCLE_CAPABILITY_KIND_PREIMAGE_V1).as_slice(),
        RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1.as_slice()
    );
    // A kind of all zeroes is the reserved absent value every manifest entry
    // constructor refuses, so the domain must not hash to it.
    assert_ne!(RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1, [0; 32]);
}

#[test]
fn compact_dynamic_retirement_derives_sparse_rows_and_preserves_output_on_wrong_geometry() {
    use super::compact_hot_v4::RationalLifecycleCompactHotRequestV4;
    let descriptor_bytes = descriptor_bytes();
    let descriptor = descriptor(&descriptor_bytes);
    let mut family = [0; LIFECYCLE_HEADER_BYTES_V2];
    let compact = RationalLifecycleCompactHotRequestV4::from_header_into(
        header(LifecycleActionV2::RetireReceipt, 0),
        &mut family,
    )
    .expect("compact");
    let mut transport = [0; hot_v6::DYNAMIC_RETIREMENT_PREFIX_BYTES_V1 + LIFECYCLE_HEADER_BYTES_V2];
    hot_v6::encode_dynamic_retirement_v1(compact, 2, &mut transport).expect("transport");
    assert_eq!(
        hot_v6::validate_dynamic_retirement_v1(&transport),
        Ok(compact)
    );
    for (offset, bytes, expected) in [
        (12, 0_u32.to_le_bytes(), Error::InvalidLength),
        (12, 9_u32.to_le_bytes(), Error::InvalidLength),
        (16, 399_u32.to_le_bytes(), Error::InvalidLength),
        (20, 1_u32.to_le_bytes(), Error::InvalidHeader),
    ] {
        let mut hostile = transport;
        hostile[offset..offset + 4].copy_from_slice(&bytes);
        assert_eq!(
            hot_v6::validate_dynamic_retirement_v1(&hostile),
            Err(expected)
        );
    }
    let mut wrong_action = transport;
    wrong_action[10] =
        u8::try_from(hot_v6::STRUCTURED_RETIRE_COORDINATE_SELECTOR_V1).expect("selector");
    assert_eq!(
        hot_v6::validate_dynamic_retirement_v1(&wrong_action),
        Err(Error::InvalidHeader)
    );
    let rows = [coordinate(0, 3, true), coordinate(2, 7, true)];
    let keys = rows
        .iter()
        .flat_map(|row| {
            [
                row.shard_mint,
                row.structured_custody_account,
                row.claims_custody_owner,
                row.claims_custody_position,
                row.position_admission,
            ]
        })
        .collect::<std::vec::Vec<_>>();
    let width = LIFECYCLE_HEADER_BYTES_V2 + rows.len() * LIFECYCLE_COORDINATE_BYTES_V2;
    let mut scratch = std::vec![0; width];
    let mut output = std::vec![0xa5; width];
    let request = compact
        .specialize_child_into(id(11), descriptor, &keys, &mut scratch, &mut output)
        .expect("complete sparse support");
    assert_eq!(request.header().coordinate_count, 2);
    assert_eq!(
        request
            .coordinates()
            .collect::<Result<std::vec::Vec<_>>>()
            .expect("rows"),
        rows
    );
    let accepted = output.clone();
    for bad in [&keys[..5], &keys[..9]] {
        assert_eq!(
            compact.specialize_child_into(id(11), descriptor, bad, &mut scratch, &mut output),
            Err(Error::InvalidSupport)
        );
        assert_eq!(output, accepted);
    }
    let mut extra = keys.clone();
    extra.extend_from_slice(&keys[..5]);
    assert_eq!(
        compact.specialize_child_into(id(11), descriptor, &extra, &mut scratch, &mut output),
        Err(Error::InvalidSupport)
    );
    assert_eq!(output, accepted);
    let mut nonzero_header = header(LifecycleActionV2::RetireReceipt, 0);
    nonzero_header.expected_receipt_supply = 1;
    let mut nonzero_bytes = [0; LIFECYCLE_HEADER_BYTES_V2];
    let before_header = nonzero_bytes;
    assert_eq!(
        RationalLifecycleCompactHotRequestV4::from_header_into(nonzero_header, &mut nonzero_bytes),
        Err(Error::InvalidPhysicalState)
    );
    assert_eq!(nonzero_bytes, before_header);
    assert_eq!(output, accepted);
}

#[test]
fn nested_position_projection_preserves_exact_admit_and_close_bytes() {
    use crate::protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2,
    };
    for (action, position_action, presence) in [
        (
            LifecycleActionV2::ActivateCoordinate,
            ProtocolPositionActionV2::Admit,
            ProtocolPositionPresenceV2::Vacant,
        ),
        (
            LifecycleActionV2::RetireCoordinate,
            ProtocolPositionActionV2::Close,
            ProtocolPositionPresenceV2::Existing,
        ),
    ] {
        let mut row = coordinate(2, 7, false);
        row.position_rent_principal = 91;
        row.admission_rent_principal = 92;
        let wire = request_bytes(header(action, 1), &[row]);
        let request = LifecycleRequestV2::decode(&wire).expect("lifecycle");
        // Independent expected wire, with unequal observed/principal coordinates
        // so a swapped rent field cannot preserve this equality.
        let expected = ProtocolPositionRequestV2 {
            action: position_action,
            owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
            presence,
            release_set: id(7),
            market: id(6),
            position_owner: id(38),
            parent_request_digest: id(50),
            rent_credit: id(12),
            rent_program: id(13),
            generation: 14,
            expected_market_revision: 15,
            expected_position_revision: 0,
            observed_position_lamports: 130,
            observed_admission_lamports: 100,
            position_rent_principal: 91,
            admission_rent_principal: 92,
            capability_descriptor: id(9),
            capability_outcome: 2,
        }
        .new()
        .expect("independent expected request")
        .to_bytes()
        .expect("expected bytes");
        assert_eq!(
            request
                .protocol_position_request(id(50))
                .expect("projection")
                .to_bytes()
                .expect("projected bytes"),
            expected
        );
        let changed_digest = request
            .protocol_position_request(id(51))
            .expect("new digest");
        assert_eq!(changed_digest.parent_request_digest, id(51));
        assert_ne!(changed_digest.to_bytes().expect("changed bytes"), expected);
    }
}

#[test]
fn nested_position_projection_refuses_receipt_actions_and_retains_position_causes() {
    use crate::protocol_position_v2::ProtocolPositionErrorV2;
    for action in [
        LifecycleActionV2::ActivateReceipt,
        LifecycleActionV2::RetireReceipt,
    ] {
        let wire = request_bytes(header(action, 0), &[]);
        assert_eq!(
            LifecycleRequestV2::decode(&wire)
                .expect("receipt")
                .protocol_position_request(id(50)),
            Err(CoordinatePositionRequestErrorV2::Action)
        );
    }
    let row = coordinate(2, 7, false);
    let wire = request_bytes(header(LifecycleActionV2::ActivateCoordinate, 1), &[row]);
    assert_eq!(
        LifecycleRequestV2::decode(&wire)
            .expect("coordinate")
            .protocol_position_request([0; 32]),
        Err(CoordinatePositionRequestErrorV2::Position(
            ProtocolPositionErrorV2::InvalidIdentity
        ))
    );
    let mut short_rent = row;
    short_rent.position_rent_principal = row.observed_position_lamports + 1;
    let wire = request_bytes(
        header(LifecycleActionV2::ActivateCoordinate, 1),
        &[short_rent],
    );
    assert_eq!(
        LifecycleRequestV2::decode(&wire)
            .expect("structural lifecycle")
            .protocol_position_request(id(50)),
        Err(CoordinatePositionRequestErrorV2::Position(
            ProtocolPositionErrorV2::InvalidRent
        ))
    );
}
