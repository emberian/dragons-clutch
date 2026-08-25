//! Hostile physical-codec tests for the Core capability funding-list prefix.

use dclutch_market_core_codec::{
    CAPABILITY_FUNDING_DESCRIPTOR_BYTES_V1, CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1,
    CAPABILITY_FUNDING_LIST_MAX_BYTES_V1, CAPABILITY_FUNDING_MAX_ENTRIES_V1,
    CapabilityFundingDescriptorV1, CapabilityFundingListV1, Error,
    capability_funding_list_bytes_v1,
};

fn descriptor(entry_index: u16, byte: u8) -> CapabilityFundingDescriptorV1 {
    CapabilityFundingDescriptorV1::new(entry_index, [byte; 32]).expect("nonzero descriptor")
}

fn fixture() -> ([u8; 124], [CapabilityFundingDescriptorV1; 3]) {
    let descriptors = [descriptor(1, 11), descriptor(4, 12), descriptor(9, 13)];
    let mut bytes = [0_u8; 124];
    CapabilityFundingListV1::encode_into(&descriptors, 4, &mut bytes).expect("canonical list");
    (bytes, descriptors)
}

#[test]
fn exact_prefix_roundtrips_and_preserves_child_request() {
    let (bytes, descriptors) = fixture();
    let mut composite = [0_u8; 127];
    composite
        .get_mut(..bytes.len())
        .expect("prefix range")
        .copy_from_slice(&bytes);
    composite
        .get_mut(bytes.len()..)
        .expect("tail range")
        .copy_from_slice(&[7, 8, 9]);
    let (decoded, tail) = CapabilityFundingListV1::decode_prefix(&composite).expect("composite");
    assert_eq!(decoded.count(), 3);
    assert_eq!(decoded.selected_entry_index(), 4);
    assert_eq!(decoded.as_bytes(), bytes);
    assert_eq!(tail, [7, 8, 9]);
    for (position, expected) in descriptors.iter().copied().enumerate() {
        assert_eq!(decoded.descriptor(position), Ok(expected));
    }
    assert_eq!(decoded.descriptor(3), Err(Error::InvalidCoordinates));
}

#[test]
fn profile_bound_and_exact_width_are_explicit() {
    assert_eq!(CAPABILITY_FUNDING_MAX_ENTRIES_V1, 16);
    assert_eq!(CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, 16);
    assert_eq!(CAPABILITY_FUNDING_DESCRIPTOR_BYTES_V1, 36);
    assert_eq!(capability_funding_list_bytes_v1(1), Ok(52));
    assert_eq!(capability_funding_list_bytes_v1(16), Ok(592));
    assert_eq!(CAPABILITY_FUNDING_LIST_MAX_BYTES_V1, 592);
    assert_eq!(
        capability_funding_list_bytes_v1(0),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        capability_funding_list_bytes_v1(17),
        Err(Error::InvalidLength)
    );
}

#[test]
fn hostile_header_and_tail_shapes_refuse() {
    let (bytes, _) = fixture();
    assert_eq!(
        CapabilityFundingListV1::decode_prefix(&bytes),
        Err(Error::InvalidLength)
    );
    for (offset, expected) in [
        (0_usize, Error::InvalidMagic),
        (8, Error::UnsupportedVersion),
        (11, Error::NonzeroReserved),
        (14, Error::NonzeroReserved),
    ] {
        let mut hostile = bytes;
        let byte = hostile.get_mut(offset).expect("hostile offset");
        *byte ^= 1;
        assert_eq!(
            CapabilityFundingListV1::decode_exact(&hostile),
            Err(expected)
        );
    }
    let mut zero_count = bytes;
    *zero_count.get_mut(10).expect("count") = 0;
    assert_eq!(
        CapabilityFundingListV1::decode_exact(&zero_count),
        Err(Error::InvalidLength)
    );
    let mut over_count = bytes;
    *over_count.get_mut(10).expect("count") = 17;
    assert_eq!(
        CapabilityFundingListV1::decode_exact(&over_count),
        Err(Error::InvalidLength)
    );
}

#[test]
fn descriptor_order_selection_alias_and_reserved_bytes_refuse() {
    let mut storage = [0_u8; 88];
    assert_eq!(
        CapabilityFundingListV1::encode_into(
            &[descriptor(4, 11), descriptor(1, 12)],
            4,
            &mut storage,
        ),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        CapabilityFundingListV1::encode_into(
            &[descriptor(1, 11), descriptor(1, 12)],
            1,
            &mut storage,
        ),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        CapabilityFundingListV1::encode_into(
            &[descriptor(1, 11), descriptor(4, 11)],
            1,
            &mut storage,
        ),
        Err(Error::InvalidAlias)
    );
    assert_eq!(
        CapabilityFundingListV1::encode_into(
            &[descriptor(1, 11), descriptor(4, 12)],
            9,
            &mut storage,
        ),
        Err(Error::InvalidCoordinates)
    );

    CapabilityFundingListV1::encode_into(&[descriptor(1, 11), descriptor(4, 12)], 1, &mut storage)
        .expect("canonical list");
    *storage.get_mut(18).expect("descriptor reserved") = 1;
    assert_eq!(
        CapabilityFundingListV1::decode_exact(&storage),
        Err(Error::NonzeroReserved)
    );
}

#[test]
fn zero_funding_account_refuses() {
    assert_eq!(
        CapabilityFundingDescriptorV1::new(0, [0; 32]),
        Err(Error::InvalidAccount)
    );
}
