//! Hostile and generator-freshness tests for Capability Funding Header V2.

#![allow(clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

use dclutch_market_core_codec::{
    CAPABILITY_FUNDING_HEADER_BYTES_V2, CapabilityFundingHeaderV2, Error,
};

#[test]
fn exact_header_roundtrips_at_logical_boundaries() {
    assert_eq!(CAPABILITY_FUNDING_HEADER_BYTES_V2, 16);
    for (physical_count, logical_count, selected_mask) in
        [(1_u8, 1_u8, 0x8000_u16), (16, 16, u16::MAX)]
    {
        let header = CapabilityFundingHeaderV2::new(physical_count, logical_count, selected_mask)
            .expect("canonical funding header");
        let bytes = header.encode();
        assert_eq!(bytes.len(), CAPABILITY_FUNDING_HEADER_BYTES_V2);
        assert_eq!(CapabilityFundingHeaderV2::decode(&bytes), Ok(header));
        assert_eq!(header.physical_count(), physical_count);
        assert_eq!(header.logical_count(), logical_count);
        assert_eq!(header.selected_mask(), selected_mask);
    }
}

#[test]
fn exact_wire_pins_magic_version_physical_count_mask_and_reserved() {
    let bytes = CapabilityFundingHeaderV2::new(2, 3, 0x8005)
        .expect("canonical funding header")
        .encode();
    assert_eq!(&bytes[..8], b"DCLTCFL2");
    assert_eq!(&bytes[8..10], &2_u16.to_le_bytes());
    assert_eq!(bytes[10], 2);
    assert_eq!(bytes[11], 3);
    assert_eq!(&bytes[12..14], &0x8005_u16.to_le_bytes());
    assert_eq!(&bytes[14..16], &[0, 0]);
}

#[test]
fn constructor_refuses_empty_oversized_and_out_of_range_selections() {
    assert_eq!(
        CapabilityFundingHeaderV2::new(0, 1, 1),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        CapabilityFundingHeaderV2::new(17, 1, 1),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        CapabilityFundingHeaderV2::new(1, 0, 1),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        CapabilityFundingHeaderV2::new(1, 17, 1),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        CapabilityFundingHeaderV2::new(4, 3, 0x8005),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        CapabilityFundingHeaderV2::new(1, 3, 0),
        Err(Error::InvalidFunding)
    );
    assert_eq!(
        CapabilityFundingHeaderV2::new(1, 3, 0b1000),
        Err(Error::InvalidFunding)
    );
    assert_eq!(
        CapabilityFundingHeaderV2::new(1, 3, 0b011),
        Err(Error::InvalidFunding)
    );
    assert!(CapabilityFundingHeaderV2::new(16, 16, u16::MAX).is_ok());
}

#[test]
fn decoder_refuses_nonexact_and_each_hostile_fixed_field() {
    let bytes = CapabilityFundingHeaderV2::new(2, 3, 0x8005)
        .expect("canonical funding header")
        .encode();
    assert_eq!(
        CapabilityFundingHeaderV2::decode(&bytes[..15]),
        Err(Error::InvalidLength)
    );
    let mut long = [0_u8; 17];
    long[..16].copy_from_slice(&bytes);
    assert_eq!(
        CapabilityFundingHeaderV2::decode(&long),
        Err(Error::InvalidLength)
    );
    for (offset, expected) in [
        (0_usize, Error::InvalidMagic),
        (8, Error::UnsupportedVersion),
        (14, Error::NonzeroReserved),
        (15, Error::NonzeroReserved),
    ] {
        let mut hostile = bytes;
        *hostile.get_mut(offset).expect("hostile fixed-field byte") ^= 1;
        assert_eq!(CapabilityFundingHeaderV2::decode(&hostile), Err(expected));
    }
    for (offset, value, expected) in [
        (10_usize, 0_u8, Error::InvalidLength),
        (10, 4, Error::InvalidLength),
        (10, 17, Error::InvalidLength),
        (11_usize, 0_u8, Error::InvalidLength),
        (11, 17, Error::InvalidLength),
        (12, 0, Error::InvalidFunding),
        (13, 0, Error::InvalidFunding),
    ] {
        let mut hostile = bytes;
        *hostile.get_mut(offset).expect("hostile count or mask byte") = value;
        if offset == 12 {
            *hostile.get_mut(13).expect("mask high byte") = 0;
        }
        assert_eq!(CapabilityFundingHeaderV2::decode(&hostile), Err(expected));
    }
}

#[test]
fn selection_query_uses_the_sparse_sixteen_bit_domain() {
    let header = CapabilityFundingHeaderV2::new(2, 3, 0x8005).expect("canonical funding header");
    assert!(header.selects(0));
    assert!(!header.selects(1));
    assert!(header.selects(2));
    assert!(!header.selects(3));
    assert!(header.selects(15));
    assert!(!header.selects(16));
    assert!(!header.selects(u8::MAX));

    let sparse_singleton =
        CapabilityFundingHeaderV2::new(1, 1, 0b1000).expect("sparse singleton selection");
    assert!(!sparse_singleton.selects(0));
    assert!(sparse_singleton.selects(3));
}

#[test]
fn checked_in_header_constants_are_exact_lean_output() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let formal = manifest.join("../../formal/dclutch-semantics");
    let build = Command::new("lake")
        .args(["build", "DClutchSemantics.CapabilityFundingHeaderV2Abi"])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean build: {error}"));
    assert!(
        build.status.success(),
        "Capability Funding Header V2 build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let generated = Command::new("lake")
        .args([
            "env",
            "lean",
            "--run",
            "EmitCapabilityFundingHeaderV2Rust.lean",
        ])
        .current_dir(&formal)
        .output()
        .unwrap_or_else(|error| panic!("launch Lean generator: {error}"));
    assert!(
        generated.status.success(),
        "Capability Funding Header V2 generator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let checked_in = std::fs::read(manifest.join("src/generated_capability_funding_header_v2.rs"))
        .unwrap_or_else(|error| panic!("read generated Rust: {error}"));
    assert_eq!(generated.stdout, checked_in);
}
