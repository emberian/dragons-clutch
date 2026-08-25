//! Hostile physical-codec tests for the fixed Core capability funding header.

use dclutch_market_core_codec::{
    CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CAPABILITY_FUNDING_MAX_ENTRIES_V1,
    CapabilityFundingHeaderV1, Error,
};

#[test]
fn exact_header_roundtrips_at_both_profile_boundaries() {
    assert_eq!(CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, 16);
    assert_eq!(CAPABILITY_FUNDING_MAX_ENTRIES_V1, 16);
    for count in [1_u8, 16] {
        let value = CapabilityFundingHeaderV1::new(count).expect("profile count");
        let bytes = value.encode();
        assert_eq!(bytes.len(), 16);
        assert_eq!(CapabilityFundingHeaderV1::decode(&bytes), Ok(value));
        assert_eq!(value.funding_count(), count);
    }
}

#[test]
fn empty_oversized_and_nonexact_headers_refuse() {
    assert_eq!(CapabilityFundingHeaderV1::new(0), Err(Error::InvalidLength));
    assert_eq!(
        CapabilityFundingHeaderV1::new(17),
        Err(Error::InvalidLength)
    );
    let bytes = CapabilityFundingHeaderV1::new(3)
        .expect("profile count")
        .encode();
    assert_eq!(
        CapabilityFundingHeaderV1::decode(bytes.get(..15).expect("short header")),
        Err(Error::InvalidLength)
    );
    let mut long = [0_u8; 17];
    long.get_mut(..16)
        .expect("header range")
        .copy_from_slice(&bytes);
    assert_eq!(
        CapabilityFundingHeaderV1::decode(&long),
        Err(Error::InvalidLength)
    );
}

#[test]
fn hostile_magic_version_reserved_and_count_refuse() {
    let bytes = CapabilityFundingHeaderV1::new(3)
        .expect("profile count")
        .encode();
    for (offset, expected) in [
        (0_usize, Error::InvalidMagic),
        (8, Error::UnsupportedVersion),
        (11, Error::NonzeroReserved),
        (15, Error::NonzeroReserved),
    ] {
        let mut hostile = bytes;
        *hostile.get_mut(offset).expect("hostile byte") ^= 1;
        assert_eq!(CapabilityFundingHeaderV1::decode(&hostile), Err(expected));
    }
    for count in [0_u8, 17] {
        let mut hostile = bytes;
        *hostile.get_mut(10).expect("count byte") = count;
        assert_eq!(
            CapabilityFundingHeaderV1::decode(&hostile),
            Err(Error::InvalidLength)
        );
    }
}
