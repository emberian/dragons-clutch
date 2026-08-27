use super::*;

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

fn id(fill: u8) -> [u8; 32] {
    [fill; 32]
}

fn key() -> CapabilitySealKeyV1 {
    CapabilitySealKeyV1::new(id(0x11), id(0x22), 7, id(0x33)).expect("canonical seal key")
}

fn rows() -> [SealedRecordRowV1; CAPABILITY_SEAL_ROW_COUNT_V1] {
    let mut ordinal = 0_u8;
    SealedRoleV1::canonical_order().map(|role| {
        ordinal = ordinal.saturating_add(1);
        let (schema, digest) = if role == SealedRoleV1::Descriptor {
            (id(0x11), id(0x22))
        } else {
            (id(0x40 + ordinal), id(0x50 + ordinal))
        };
        SealedRecordRowV1::new(
            role,
            u32::from(ordinal).saturating_mul(64).max(1),
            schema,
            digest,
            id(0x60 + ordinal),
            id(0x70 + ordinal),
        )
        .expect("canonical row")
    })
}

fn canonical() -> Vec<u8> {
    let mut bytes = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(key(), rows(), &mut bytes).expect("canonical seal");
    bytes
}

#[test]
fn canonical_seal_round_trips_and_pins_its_width() {
    assert_eq!(CAPABILITY_SEAL_BYTES_V1, 936);
    let bytes = canonical();
    let decoded = SealedDescriptorClosureV1::decode(&bytes).expect("decode");
    assert_eq!(decoded.key(), key());
    decoded.require_key(key()).expect("same key");
    for (ordinal, role) in SealedRoleV1::canonical_order().into_iter().enumerate() {
        let row = decoded.row(role).expect("row");
        assert_eq!(row.role(), role);
        assert_eq!(role.ordinal(), ordinal);
        assert_eq!(
            rows()
                .get(ordinal)
                .map(|expected| expected.content_digest()),
            Some(row.content_digest())
        );
    }
}

#[test]
fn the_descriptor_row_must_be_the_key_it_is_filed_under() {
    let mut bytes = canonical();
    let offset = CAPABILITY_SEAL_HEADER_BYTES_V1 + CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1;
    bytes
        .get_mut(offset..offset + 32)
        .expect("descriptor row digest")
        .copy_from_slice(&id(0x99));
    assert_eq!(
        SealedDescriptorClosureV1::decode(&bytes),
        Err(Error::DescriptorMismatch)
    );
}

#[test]
fn every_header_field_is_canonicality_checked() {
    let cases: [(usize, u8, Error); 6] = [
        (CAPABILITY_SEAL_MAGIC_OFFSET_V1, 0xff, Error::InvalidMagic),
        (
            CAPABILITY_SEAL_SCHEMA_VERSION_OFFSET_V1,
            0x09,
            Error::UnsupportedSchema,
        ),
        (
            CAPABILITY_SEAL_PROFILE_OFFSET_V1,
            0x09,
            Error::UnsupportedArtifactProfile,
        ),
        (
            CAPABILITY_SEAL_ROW_COUNT_OFFSET_V1,
            0x09,
            Error::InvalidRowCount,
        ),
        (
            CAPABILITY_SEAL_VERDICTS_OFFSET_V1,
            0x09,
            Error::InvalidVerdicts,
        ),
        (
            CAPABILITY_SEAL_RESERVED_OFFSET_V1,
            0x01,
            Error::NonCanonicalReserved,
        ),
    ];
    for (offset, value, expected) in cases {
        let mut bytes = canonical();
        *bytes.get_mut(offset).expect("header byte") = value;
        assert_eq!(SealedDescriptorClosureV1::decode(&bytes), Err(expected));
    }
    for width in [CAPABILITY_SEAL_BYTES_V1 - 1, CAPABILITY_SEAL_BYTES_V1 + 1] {
        let mut bytes = canonical();
        bytes.resize(width, 0);
        assert_eq!(
            SealedDescriptorClosureV1::decode(&bytes),
            Err(Error::InvalidLength)
        );
    }
}

#[test]
fn rows_must_carry_their_canonical_role_in_their_canonical_position() {
    let mut bytes = canonical();
    let offset = CAPABILITY_SEAL_HEADER_BYTES_V1
        + CAPABILITY_SEAL_ROW_BYTES_V1
        + CAPABILITY_SEAL_ROW_ROLE_OFFSET_V1;
    *bytes.get_mut(offset).expect("row role") =
        u8::try_from(SealedRoleV1::EffectProgram.tag()).expect("role tag");
    assert_eq!(
        SealedDescriptorClosureV1::decode(&bytes),
        Err(Error::NonCanonicalRowOrder)
    );
    let mut bytes = canonical();
    *bytes.get_mut(offset).expect("row role") = 0x7f;
    assert_eq!(
        SealedDescriptorClosureV1::decode(&bytes),
        Err(Error::UnknownRole)
    );
    let mut bytes = canonical();
    let reserved = CAPABILITY_SEAL_HEADER_BYTES_V1 + CAPABILITY_SEAL_ROW_RESERVED_OFFSET_V1;
    *bytes.get_mut(reserved).expect("row reserved") = 1;
    assert_eq!(
        SealedDescriptorClosureV1::decode(&bytes),
        Err(Error::NonCanonicalReserved)
    );
}

#[test]
fn a_permuted_row_body_refuses_even_with_correct_role_tags() {
    // Swap the account-profile and request-profile bodies but keep both role
    // tags in place: the artifact identities then no longer match what a
    // descriptor names for either role, and `authenticate_artifact` refuses.
    let bytes = canonical();
    let decoded = SealedDescriptorClosureV1::decode(&bytes).expect("decode");
    let account = decoded
        .row(SealedRoleV1::AccountProfile)
        .expect("account row");
    let request = decoded
        .row(SealedRoleV1::RequestProfile)
        .expect("request row");
    let body = vec![0_u8; usize::try_from(request.exact_data_length()).expect("row width")];
    assert_eq!(
        decoded
            .authenticate_artifact(
                SealedRoleV1::AccountProfile,
                request.schema(),
                request.content_digest(),
                &body,
            )
            .map(|_| ()),
        Err(Error::ArtifactIdentityMismatch)
    );
    assert_eq!(account.role(), SealedRoleV1::AccountProfile);
}

#[test]
fn a_seal_for_another_key_refuses_at_every_coordinate() {
    let bytes = canonical();
    let decoded = SealedDescriptorClosureV1::decode(&bytes).expect("decode");
    let cases = [
        (
            CapabilitySealKeyV1::new(id(0x11), id(0x23), 7, id(0x33)).expect("key"),
            Error::DescriptorMismatch,
        ),
        (
            CapabilitySealKeyV1::new(id(0x12), id(0x22), 7, id(0x33)).expect("key"),
            Error::DescriptorMismatch,
        ),
        (
            CapabilitySealKeyV1::new(id(0x11), id(0x22), 8, id(0x33)).expect("key"),
            Error::ActionMismatch,
        ),
        (
            CapabilitySealKeyV1::new(id(0x11), id(0x22), 7, id(0x34)).expect("key"),
            Error::InterpreterReleaseMismatch,
        ),
    ];
    for (other, expected) in cases {
        assert_eq!(decoded.require_key(other), Err(expected));
    }
}

#[test]
fn zero_identities_and_zero_widths_are_refused() {
    assert_eq!(
        CapabilitySealKeyV1::new([0; 32], id(0x22), 7, id(0x33)),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        CapabilitySealKeyV1::new(id(0x11), [0; 32], 7, id(0x33)),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        CapabilitySealKeyV1::new(id(0x11), id(0x22), 7, [0; 32]),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        SealedRecordRowV1::new(SealedRoleV1::EffectProgram, 0, id(1), id(2), id(3), id(4)),
        Err(Error::ZeroRecordWidth)
    );
    assert_eq!(
        SealedRecordRowV1::new(SealedRoleV1::EffectProgram, 8, id(1), id(2), id(3), id(3)),
        Err(Error::ZeroIdentity)
    );
}

#[test]
fn a_token_covers_only_the_exact_range_and_role_it_was_minted_for() {
    let bytes = canonical();
    let decoded = SealedDescriptorClosureV1::decode(&bytes).expect("decode");
    let row = decoded
        .row(SealedRoleV1::EffectProgram)
        .expect("effect row");
    let body = vec![0xab_u8; usize::try_from(row.exact_data_length()).expect("row width")];
    let token = decoded
        .authenticate_artifact(
            SealedRoleV1::EffectProgram,
            row.schema(),
            row.content_digest(),
            &body,
        )
        .expect("token");
    token
        .require(SealedRoleV1::EffectProgram, &body)
        .expect("same range, same role");
    assert_eq!(
        token.require(SealedRoleV1::AccountProfile, &body),
        Err(Error::TokenRoleMismatch)
    );

    // A byte-identical artifact at another address is a different artifact.
    let twin = body.clone();
    assert_eq!(
        token.require(SealedRoleV1::EffectProgram, &twin),
        Err(Error::TokenRangeMismatch)
    );
    let short = body.get(..body.len() - 1).expect("prefix");
    assert_eq!(
        token.require(SealedRoleV1::EffectProgram, short),
        Err(Error::TokenRangeMismatch)
    );
}

#[test]
fn a_body_of_the_wrong_width_cannot_mint_a_token() {
    let bytes = canonical();
    let decoded = SealedDescriptorClosureV1::decode(&bytes).expect("decode");
    let row = decoded
        .row(SealedRoleV1::TransitionProgram)
        .expect("transition row");
    let body = vec![
        0_u8;
        usize::try_from(row.exact_data_length())
            .expect("row width")
            .saturating_add(1)
    ];
    assert_eq!(
        decoded
            .authenticate_artifact(
                SealedRoleV1::TransitionProgram,
                row.schema(),
                row.content_digest(),
                &body,
            )
            .map(|_| ()),
        Err(Error::RecordWidthMismatch)
    );
}

#[test]
fn the_seed_projection_is_the_four_key_coordinates_under_one_domain() {
    let seeds = key().seeds();
    let slices = seeds.as_slices();
    assert_eq!(slices[0], CAPABILITY_SEAL_PDA_DOMAIN_V1);
    assert_eq!(slices[1], id(0x11));
    assert_eq!(slices[2], id(0x22));
    assert_eq!(slices[3], 7_u32.to_le_bytes());
    assert_eq!(slices[4], id(0x33));
    for seed in slices {
        assert!(seed.len() <= 32);
    }
}
