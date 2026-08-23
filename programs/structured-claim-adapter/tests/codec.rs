use clutch_structured_claim_adapter::{
    Action, Error, RequestV1, StructuredClaimDescriptorV1, WrapperReplayV1, DESCRIPTOR_BYTES,
    REPLAY_BYTES, REQUEST_BYTES,
};
use sha2::{Digest, Sha256};

fn descriptor() -> StructuredClaimDescriptorV1 {
    let mut primitive = [0; 16];
    primitive[0] = 1;
    primitive[1] = 2;
    StructuredClaimDescriptorV1 {
        base_program: [1; 32],
        base_program_data: [2; 32],
        base_deployment_slot: 3,
        wrapper_program_data: [4; 32],
        wrapper_deployment_slot: 5,
        token_2022_program: [6; 32],
        token_2022_program_data: [7; 32],
        token_2022_deployment_slot: 8,
        market: [9; 32],
        terms: [10; 32],
        primitive,
        state: 0,
        descriptor_bump: 11,
        mint_bump: 12,
        vault_owner_bump: 13,
    }
}

fn request() -> RequestV1 {
    RequestV1 {
        action: Action::WrapFull,
        wrapper_sequence: 1,
        source_base_sequence: 2,
        vault_base_sequence: 3,
        quantity: 4,
        expected_mint_supply: 5,
        expected_holder_amount: 6,
        source_generation: 7,
        vault_generation: 8,
    }
}

#[test]
fn frozen_exact_codecs_round_trip() {
    let descriptor = descriptor();
    let mut descriptor_bytes = [0; DESCRIPTOR_BYTES];
    descriptor.encode(&mut descriptor_bytes).unwrap();
    assert_eq!(
        StructuredClaimDescriptorV1::decode(&descriptor_bytes),
        Ok(descriptor)
    );

    let replay = WrapperReplayV1 {
        descriptor: [21; 32],
        actor: [22; 32],
        sequence: 23,
        stored_bump: 24,
    };
    let mut replay_bytes = [0; REPLAY_BYTES];
    replay.encode(&mut replay_bytes).unwrap();
    assert_eq!(WrapperReplayV1::decode(&replay_bytes), Ok(replay));

    let request = request();
    let mut request_bytes = [0; REQUEST_BYTES];
    request.encode(&mut request_bytes).unwrap();
    assert_eq!(RequestV1::decode(&request_bytes), Ok(request));

    let digest: [u8; 32] = Sha256::digest(descriptor_bytes).into();
    assert_eq!(
        digest,
        [
            0xb6, 0x04, 0x58, 0xf9, 0x4d, 0xee, 0x18, 0xeb, 0x4d, 0xcd, 0x0c, 0x3e, 0x9f, 0x00,
            0x26, 0x07, 0x07, 0x71, 0x65, 0x7d, 0x1a, 0x7c, 0x3a, 0x75, 0xcf, 0x9f, 0xc5, 0x57,
            0x58, 0xd9, 0xb8, 0x19,
        ]
    );
}

#[test]
fn hostile_lengths_headers_flags_padding_and_enums_refuse() {
    let mut bytes = [0; DESCRIPTOR_BYTES];
    descriptor().encode(&mut bytes).unwrap();
    assert_eq!(
        StructuredClaimDescriptorV1::decode(&bytes[..DESCRIPTOR_BYTES - 1]),
        Err(Error::Truncated)
    );
    let mut long = [0; DESCRIPTOR_BYTES + 1];
    long[..DESCRIPTOR_BYTES].copy_from_slice(&bytes);
    assert_eq!(
        StructuredClaimDescriptorV1::decode(&long),
        Err(Error::TrailingBytes)
    );
    for (offset, expected) in [
        (0, Error::WrongTag),
        (1, Error::WrongVersion),
        (2, Error::NonCanonical),
    ] {
        let mut hostile = bytes;
        hostile[offset] ^= 1;
        assert_eq!(StructuredClaimDescriptorV1::decode(&hostile), Err(expected));
    }
    let mut hostile = bytes;
    hostile[DESCRIPTOR_BYTES - 4] = 2;
    assert_eq!(
        StructuredClaimDescriptorV1::decode(&hostile),
        Err(Error::NonCanonical)
    );

    let replay = WrapperReplayV1 {
        descriptor: [21; 32],
        actor: [22; 32],
        sequence: 23,
        stored_bump: 24,
    };
    let mut replay_bytes = [0; REPLAY_BYTES];
    replay.encode(&mut replay_bytes).unwrap();
    replay_bytes[REPLAY_BYTES - 1] = 1;
    assert_eq!(
        WrapperReplayV1::decode(&replay_bytes),
        Err(Error::NonCanonical)
    );

    let mut request_bytes = [0; REQUEST_BYTES];
    request().encode(&mut request_bytes).unwrap();
    request_bytes[4] = 99;
    assert_eq!(RequestV1::decode(&request_bytes), Err(Error::NonCanonical));
    request().encode(&mut request_bytes).unwrap();
    request_bytes[5] = 1;
    assert_eq!(RequestV1::decode(&request_bytes), Err(Error::NonCanonical));
}

#[test]
fn operation_specific_quantity_rules_are_part_of_the_wire() {
    let mut value = request();
    value.quantity = 0;
    assert_eq!(value.validate(), Err(Error::NonCanonical));
    value.action = Action::CompactDonation;
    assert_eq!(value.validate(), Ok(()));
    value.quantity = 1;
    assert_eq!(value.validate(), Err(Error::NonCanonical));
}
