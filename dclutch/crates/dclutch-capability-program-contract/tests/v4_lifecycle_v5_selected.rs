//! Release-boundary tests for the sole Capability Program V4 lifecycle schema.

#![allow(clippy::panic)]

use dclutch_account_profile_contract::lifecycle_v3::{
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5, HEADER_BYTES, SUCCESSOR_SCHEMA_RELEASE_ID,
    StateLifecyclePolicyV5, encode::encode_lifecycle_policy_v5_atomic,
};
use dclutch_capability_program_contract::Error;
use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
    SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
};
use dclutch_core_contract::ContentId;

fn id(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("nonzero identity")
}

fn lifecycle_schema() -> ContentId {
    ContentId::new(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5).expect("selected lifecycle schema")
}

fn artifacts(lifecycle: ArtifactReferenceV4) -> CapabilityArtifactsV4 {
    CapabilityArtifactsV4 {
        account_profile: ArtifactReferenceV4::new(id(7), id(8)),
        request_profile: ArtifactReferenceV4::new(id(9), id(10)),
        lifecycle,
        strategy: ArtifactReferenceV4::new(id(13), id(14)),
        transition: ArtifactReferenceV4::new(id(15), id(16)),
        effect: ArtifactReferenceV4::new(id(17), id(18)),
    }
}

#[test]
fn exact_600_byte_wire_selects_only_lifecycle_v5() {
    assert_eq!(
        SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5
    );
    let descriptor = CapabilityProgramV4::new(
        id(1),
        id(2),
        id(3),
        id(4),
        id(5),
        id(6),
        artifacts(ArtifactReferenceV4::new(lifecycle_schema(), id(12))),
        128,
    )
    .expect("V5-selected descriptor");
    let encoded = descriptor.encode();
    assert_eq!(encoded.len(), CAPABILITY_PROGRAM_V4_BYTES);
    assert_eq!(CAPABILITY_PROGRAM_V4_BYTES, 600);
    assert_eq!(CapabilityProgramV4::decode(&encoded), Ok(descriptor));

    let legacy = ContentId::new(SUCCESSOR_SCHEMA_RELEASE_ID).expect("legacy schema");
    assert_eq!(
        CapabilityProgramV4::new(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            artifacts(ArtifactReferenceV4::new(legacy, id(12))),
            128,
        ),
        Err(Error::UnsupportedSchema)
    );
}

#[test]
fn no_lifecycle_route_uses_canonical_empty_v5() {
    let mut scratch = [0_u8; HEADER_BYTES];
    let mut output = [0_u8; HEADER_BYTES];
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
        .expect("canonical empty V5 encode");
    let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &output)
        .expect("canonical empty V5 decode");
    assert!(policy.is_empty());
}
