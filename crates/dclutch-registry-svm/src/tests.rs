extern crate std;

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{ArtifactReleaseIdV1, ExecutionRoleV1, ProgramIdentityV1};
use sha2::{Digest, Sha256};

use crate::{
    AUTHENTICATED_ROLE_RECEIPT_BYTES_V1, AuthenticatedRoleReceiptV1, Error,
    LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataV3View,
    ProgramV3View, REGISTRY_INSTRUCTION_BYTES_V1, RegistryInstructionV1,
};

fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    let end = offset.checked_add(source.len()).expect("fixture range");
    output
        .get_mut(offset..end)
        .expect("fixture range in bounds")
        .copy_from_slice(source);
}

fn flip(output: &mut [u8], offset: usize) {
    let byte = output.get_mut(offset).expect("fixture offset in bounds");
    *byte ^= 0xff;
}

#[test]
fn instructions_are_exact_and_hostile_decoded() {
    let cases = [
        RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Core),
        RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Claims),
        RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Trading),
        RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Resolution),
        RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Custody),
        RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Core),
        RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Claims),
        RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Trading),
        RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Resolution),
        RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Custody),
    ];
    for value in cases {
        let encoded = value.to_bytes();
        assert_eq!(encoded.len(), REGISTRY_INSTRUCTION_BYTES_V1);
        assert_eq!(RegistryInstructionV1::decode(&encoded), Ok(value));
    }

    let valid = RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Core).to_bytes();
    assert_eq!(
        RegistryInstructionV1::decode(&valid[..15]),
        Err(Error::InvalidLength)
    );
    for (offset, expected) in [
        (0, Error::InvalidMagic),
        (8, Error::UnsupportedSchema),
        (10, Error::UnknownAction),
        (12, Error::NonCanonicalReservedBytes),
    ] {
        let mut hostile = valid;
        flip(&mut hostile, offset);
        assert_eq!(RegistryInstructionV1::decode(&hostile), Err(expected));
    }
    for action in [2_u8, 3, 4, 0xff] {
        let mut hostile = valid;
        hostile[10] = action;
        assert_eq!(
            RegistryInstructionV1::decode(&hostile),
            Err(Error::UnknownAction),
            "action {action} belongs to the record family or nothing at all"
        );
    }
    // The retired five-role wire was action 0 with role 0. It now names exactly
    // one role, which is strictly less authority; the ten-account activation
    // route refuses its 26-account frame.
    assert_eq!(
        RegistryInstructionV1::decode(&valid),
        Ok(RegistryInstructionV1::ActivateRole(ExecutionRoleV1::Core))
    );
    for action in [0_u8, 1] {
        let mut unknown_role = valid;
        unknown_role[10] = action;
        unknown_role[11] = 5;
        assert_eq!(
            RegistryInstructionV1::decode(&unknown_role),
            Err(Error::UnknownRole)
        );
    }
}

#[test]
fn authenticated_receipt_roundtrips_every_coordinate() {
    let receipt = AuthenticatedRoleReceiptV1::new(
        ExecutionRoleV1::Resolution,
        ContentId::new(bytes(1)).expect("content"),
        ProgramIdentityV1::new(bytes(2)).expect("program"),
        ArtifactReleaseIdV1::new(bytes(3)).expect("artifact"),
        ContentId::new(bytes(4)).expect("semantic"),
    );
    let encoded = receipt.to_bytes();
    assert_eq!(encoded.len(), AUTHENTICATED_ROLE_RECEIPT_BYTES_V1);
    assert_eq!(AuthenticatedRoleReceiptV1::decode(&encoded), Ok(receipt));
    assert_eq!(receipt.role(), ExecutionRoleV1::Resolution);
    assert_eq!(receipt.execution_release_set_id().to_bytes(), bytes(1));
    assert_eq!(receipt.program().to_bytes(), bytes(2));
    assert_eq!(receipt.artifact_release_id().to_bytes(), bytes(3));
    assert_eq!(receipt.semantic_release_id().to_bytes(), bytes(4));

    for (offset, expected) in [
        (0, Error::InvalidMagic),
        (8, Error::UnsupportedSchema),
        (11, Error::NonCanonicalReservedBytes),
    ] {
        let mut hostile = encoded;
        flip(&mut hostile, offset);
        assert_eq!(AuthenticatedRoleReceiptV1::decode(&hostile), Err(expected));
    }
    let mut zero = encoded;
    zero[16..48].fill(0);
    assert!(matches!(
        AuthenticatedRoleReceiptV1::decode(&zero),
        Err(Error::Content(_))
    ));
}

#[test]
fn loader_program_requires_exact_variant_two_and_link() {
    let mut encoded = [0_u8; LOADER_V3_PROGRAM_BYTES];
    put(&mut encoded, 0, &2_u32.to_le_bytes());
    put(&mut encoded, 4, &bytes(7));
    let view = ProgramV3View::parse(&encoded).expect("valid Loader Program");
    assert_eq!(view.programdata(), bytes(7));
    assert_eq!(
        ProgramV3View::parse(&encoded[..35]),
        Err(Error::InvalidLength)
    );
    put(&mut encoded, 0, &3_u32.to_le_bytes());
    assert_eq!(
        ProgramV3View::parse(&encoded),
        Err(Error::InvalidLoaderVariant)
    );
}

#[test]
fn loader_programdata_uses_fixed_offset_and_tag_owned_authority_semantics() {
    let elf = [0xa5_u8; 64];
    let mut immutable = [0_u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES + 64];
    put(&mut immutable, 0, &3_u32.to_le_bytes());
    put(&mut immutable, 4, &77_u64.to_le_bytes());
    put(&mut immutable, LOADER_V3_PROGRAMDATA_METADATA_BYTES, &elf);
    let view = ProgramDataV3View::parse(&immutable).expect("valid immutable ProgramData");
    assert_eq!(view.deployment_slot(), 77);
    assert_eq!(view.upgrade_authority(), None);
    assert_eq!(view.elf(), elf);
    assert_eq!(Sha256::digest(view.elf()), Sha256::digest(elf));
    assert_ne!(Sha256::digest(&immutable[13..]), Sha256::digest(view.elf()));

    let mut upgradeable = immutable;
    upgradeable[12] = 1;
    put(&mut upgradeable, 13, &bytes(9));
    let view = ProgramDataV3View::parse(&upgradeable).expect("valid authority");
    assert_eq!(view.upgrade_authority(), Some(bytes(9)));
    assert_eq!(view.elf(), elf);

    let mut default_authority = immutable;
    default_authority[12] = 1;
    let view = ProgramDataV3View::parse(&default_authority).expect("default authority shape");
    assert_eq!(view.upgrade_authority(), Some([0; 32]));
    assert_eq!(view.elf(), elf);

    let mut retained_prior_authority = immutable;
    put(&mut retained_prior_authority, 13, &bytes(44));
    let view = ProgramDataV3View::parse(&retained_prior_authority)
        .expect("Loader None tag owns retained bytes as inactive storage");
    assert_eq!(view.deployment_slot(), 77);
    assert_eq!(view.upgrade_authority(), None);
    assert_eq!(view.elf(), elf);
    assert_ne!(&retained_prior_authority[13..45], &immutable[13..45]);
    let mut bad_tag = immutable;
    bad_tag[12] = 2;
    assert_eq!(
        ProgramDataV3View::parse(&bad_tag),
        Err(Error::InvalidUpgradeAuthorityTag)
    );
    put(&mut immutable, 0, &2_u32.to_le_bytes());
    assert_eq!(
        ProgramDataV3View::parse(&immutable),
        Err(Error::InvalidLoaderVariant)
    );
    let short = [0_u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES];
    for length in 0..=LOADER_V3_PROGRAMDATA_METADATA_BYTES {
        let prefix = short.get(..length).expect("bounded short prefix");
        assert_eq!(
            ProgramDataV3View::parse(prefix),
            Err(Error::EmptyElf),
            "short ProgramData length {length}"
        );
    }
}
