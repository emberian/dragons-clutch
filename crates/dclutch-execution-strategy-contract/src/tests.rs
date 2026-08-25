extern crate std;

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use std::vec;

use super::*;

fn content(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero content")
}

fn artifact(byte: u8) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new([byte; 32]).expect("nonzero artifact")
}

fn certificate() -> ExecutionStrategyCertificateV1 {
    ExecutionStrategyCertificateV1::new(
        content(1),
        content(2),
        content(3),
        artifact(4),
        content(5),
        content(6),
        content(7),
    )
}

fn input_bank(scalars: u16, identities: u16) -> std::vec::Vec<u8> {
    vec![9_u8; register_bank_bytes(scalars, identities).expect("register bank width")]
}

fn request<'a>(bank: &'a [u8]) -> AcceleratorRequestV1<'a> {
    AcceleratorRequestV1::new(content(8), content(1), content(9), 41, 4, bank).expect("request")
}

#[test]
fn certificate_roundtrip_and_hostile_fields() {
    let certificate = certificate();
    let bytes = certificate.to_bytes();
    assert_eq!(
        ExecutionStrategyCertificateV1::decode(&bytes),
        Ok(certificate)
    );
    assert_eq!(bytes.len(), 240);

    for offset in [0, 8, 10, 12, 13] {
        let mut hostile = bytes;
        *hostile.get_mut(offset).expect("hostile offset") ^= 1;
        assert!(ExecutionStrategyCertificateV1::decode(&hostile).is_err());
    }
    for offset in [16, 48, 80, 112, 144, 176, 208] {
        let mut hostile = bytes;
        hostile
            .get_mut(offset..offset + 32)
            .expect("identity field")
            .fill(0);
        assert_eq!(
            ExecutionStrategyCertificateV1::decode(&hostile),
            Err(Error::ZeroIdentity)
        );
    }
    assert_eq!(
        ExecutionStrategyCertificateV1::decode(&bytes[..239]),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        certificate.require_aot_only_admitted(),
        Err(Error::AotOnlyUnavailable)
    );
}

#[test]
fn request_roundtrip_is_count_derived_and_runtime_width() {
    for (scalars, identities) in [(0, 1), (41, 4), (73, 19)] {
        let bank = input_bank(scalars, identities);
        let request = AcceleratorRequestV1::new(
            content(8),
            content(1),
            content(9),
            scalars,
            identities,
            &bank,
        )
        .expect("request");
        let mut bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V1 + bank.len()];
        request.encode_into(&mut bytes).expect("encode request");
        assert_eq!(AcceleratorRequestV1::decode(&bytes), Ok(request));

        let mut wrong_count = bytes;
        let count = wrong_count
            .get_mut(REQUEST_SCALAR_COUNT_OFFSET..REQUEST_SCALAR_COUNT_OFFSET + 2)
            .expect("count bytes");
        count.copy_from_slice(&scalars.wrapping_add(1).to_le_bytes());
        assert_eq!(
            AcceleratorRequestV1::decode(&wrong_count),
            Err(Error::InvalidLength)
        );
    }
}

#[test]
fn accepted_ack_roundtrip_and_exact_shadow_equivalence() {
    let bank = input_bank(41, 4);
    assert_eq!(bank.len(), 456);
    let request = request(&bank);
    let mut output_bank = bank.clone();
    *output_bank.get_mut(0).expect("first scalar byte") = 10;
    let ack = AcceleratorAckV1::accepted(request, content(10), content(11), &output_bank)
        .expect("accepted ack");
    let mut bytes = vec![0_u8; ACCELERATOR_ACK_HEADER_BYTES_V1 + output_bank.len()];
    ack.encode_into(&mut bytes).expect("encode ack");
    assert_eq!(bytes.len(), 616);
    let decoded = AcceleratorAckV1::decode(&bytes).expect("decode ack");
    assert_eq!(decoded, ack);
    assert_eq!(
        compare_execution_v1(
            request,
            content(10),
            ExecutionDispositionV1::Accepted,
            Some(content(11)),
            &output_bank,
            decoded,
        ),
        Ok(())
    );

    let mut divergent = output_bank;
    *divergent.get_mut(1).expect("second scalar byte") ^= 1;
    assert_eq!(
        compare_execution_v1(
            request,
            content(10),
            ExecutionDispositionV1::Accepted,
            Some(content(11)),
            &divergent,
            decoded,
        ),
        Err(Error::StrategyDivergence)
    );
}

#[test]
fn semantic_refusal_has_no_candidate_bank() {
    let bank = input_bank(41, 4);
    let request = request(&bank);
    let ack = AcceleratorAckV1::refused(request, content(10));
    let mut bytes = [0_u8; ACCELERATOR_ACK_HEADER_BYTES_V1];
    ack.encode_into(&mut bytes).expect("encode refusal");
    assert_eq!(AcceleratorAckV1::decode(&bytes), Ok(ack));
    assert_eq!(
        compare_execution_v1(
            request,
            content(10),
            ExecutionDispositionV1::Refused,
            None,
            &[],
            ack,
        ),
        Ok(())
    );
    assert_eq!(
        compare_execution_v1(
            request,
            content(10),
            ExecutionDispositionV1::Accepted,
            Some(content(11)),
            &bank,
            ack,
        ),
        Err(Error::StrategyDivergence)
    );

    let mut mixed = bytes;
    *mixed.get_mut(12).expect("disposition") = 1;
    assert!(AcceleratorAckV1::decode(&mixed).is_err());
}

#[test]
fn register_bank_codec_is_exact_little_endian_and_atomic_on_bad_width() {
    let scalars = [1_u64, 0x0102_0304_0506_0708, u64::MAX];
    let identities = [[0x11_u8; 32], [0x22_u8; 32]];
    let mut bytes = [0_u8; 88];
    encode_register_bank_into(&scalars, &identities, &mut bytes).expect("encode bank");
    assert_eq!(bytes.get(..8), Some(1_u64.to_le_bytes().as_slice()));
    assert_eq!(
        bytes.get(8..16),
        Some(0x0102_0304_0506_0708_u64.to_le_bytes().as_slice())
    );
    assert_eq!(bytes.get(24..56), Some(identities[0].as_slice()));

    let mut decoded_scalars = [0_u64; 3];
    let mut decoded_identities = [[0_u8; 32]; 2];
    decode_register_bank_into(&bytes, &mut decoded_scalars, &mut decoded_identities)
        .expect("decode bank");
    assert_eq!(decoded_scalars, scalars);
    assert_eq!(decoded_identities, identities);

    let scalar_before = decoded_scalars;
    let identities_before = decoded_identities;
    assert_eq!(
        decode_register_bank_into(
            bytes.get(..87).expect("short bank"),
            &mut decoded_scalars,
            &mut decoded_identities,
        ),
        Err(Error::InvalidLength)
    );
    assert_eq!(decoded_scalars, scalar_before);
    assert_eq!(decoded_identities, identities_before);
}

#[test]
fn accepted_ack_digest_substitution_refuses_comparison() {
    let bank = input_bank(41, 4);
    let request = request(&bank);
    let ack =
        AcceleratorAckV1::accepted(request, content(10), content(11), &bank).expect("accepted ack");
    assert_eq!(
        compare_execution_v1(
            request,
            content(10),
            ExecutionDispositionV1::Accepted,
            Some(content(12)),
            &bank,
            ack,
        ),
        Err(Error::StrategyDivergence)
    );
}

#[test]
fn substitutions_and_transport_overflow_refuse() {
    let bank = input_bank(41, 4);
    let request = request(&bank);
    assert_eq!(
        request.validate_certificate(content(99), certificate()),
        Err(Error::BindingMismatch)
    );
    assert_eq!(
        certificate().validate_artifact(artifact(99)),
        Err(Error::ArtifactMismatch)
    );

    let too_wide_bank = input_bank(105, 1);
    assert!(too_wide_bank.len() > ACCELERATOR_ACK_MAX_BANK_BYTES_V1);
    let too_wide_request =
        AcceleratorRequestV1::new(content(8), content(1), content(9), 105, 1, &too_wide_bank)
            .expect("input transport is runtime width");
    assert_eq!(
        AcceleratorAckV1::accepted(too_wide_request, content(10), content(11), &too_wide_bank,),
        Err(Error::ResultCapacityExceeded)
    );
}
