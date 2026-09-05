use super::*;

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

fn id(fill: u8) -> [u8; 32] {
    [fill; 32]
}

fn key() -> CapabilitySealKeyV1 {
    CapabilitySealKeyV1::new(id(0x11), id(0x22), 7, id(0x33), id(0x44)).expect("canonical seal key")
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

/// A bump these bodies are written under. Nothing here derives an address, so
/// the only property that matters is that it is one a search could return.
const CANONICAL_BUMP: u8 = 254;

fn canonical() -> Vec<u8> {
    let mut bytes = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(key(), rows(), CANONICAL_BUMP, &mut bytes)
        .expect("canonical seal");
    bytes
}

#[test]
fn canonical_seal_round_trips_and_pins_its_width() {
    assert_eq!(CAPABILITY_SEAL_BYTES_V1, 968);
    let bytes = canonical();
    let decoded = SealedDescriptorClosureV1::decode(&bytes).expect("decode");
    assert_eq!(decoded.key(), Ok(key()));
    assert_eq!(decoded.bump(), Ok(CANONICAL_BUMP));
    // The bump took the FIRST of what were four reserved bytes, so the width
    // and every offset after it are the ones already deployed.
    assert_eq!(CAPABILITY_SEAL_BUMP_OFFSET_V1, 20);
    assert_eq!(CAPABILITY_SEAL_RESERVED_OFFSET_V1, 21);
    assert_eq!(CAPABILITY_SEAL_DESCRIPTOR_SCHEMA_OFFSET_V1, 24);
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
    let cases: [(usize, u8, Error); 7] = [
        (CAPABILITY_SEAL_BUMP_OFFSET_V1, 0x00, Error::ZeroBump),
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
            CapabilitySealKeyV1::new(id(0x11), id(0x23), 7, id(0x33), id(0x44)).expect("key"),
            Error::DescriptorMismatch,
        ),
        (
            CapabilitySealKeyV1::new(id(0x12), id(0x22), 7, id(0x33), id(0x44)).expect("key"),
            Error::DescriptorMismatch,
        ),
        (
            CapabilitySealKeyV1::new(id(0x11), id(0x22), 8, id(0x33), id(0x44)).expect("key"),
            Error::ActionMismatch,
        ),
        (
            CapabilitySealKeyV1::new(id(0x11), id(0x22), 7, id(0x34), id(0x44)).expect("key"),
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
        CapabilitySealKeyV1::new([0; 32], id(0x22), 7, id(0x33), id(0x44)),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        CapabilitySealKeyV1::new(id(0x11), [0; 32], 7, id(0x33), id(0x44)),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        CapabilitySealKeyV1::new(id(0x11), id(0x22), 7, [0; 32], id(0x44)),
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
    assert_eq!(slices[5], id(0x44));
    for seed in slices {
        assert!(seed.len() <= 32);
    }
}

fn body_for(seal: SealedDescriptorClosureV1, role: SealedRoleV1) -> Vec<u8> {
    let row = seal.row(role).expect("row");
    vec![
        row.role().tag().to_le_bytes()[0];
        usize::try_from(row.exact_data_length()).expect("row width")
    ]
}

fn token<'a>(
    seal: SealedDescriptorClosureV1,
    role: SealedRoleV1,
    body: &'a [u8],
) -> SealedArtifactV1<'a> {
    let row = seal.row(role).expect("row");
    seal.authenticate_artifact(role, row.schema(), row.content_digest(), body)
        .expect("token")
}

#[test]
fn a_join_cannot_be_assembled_out_of_two_seals() {
    let first_bytes = canonical();
    let first = SealedDescriptorClosureV1::decode(&first_bytes).expect("decode");

    // A second, entirely legitimate seal for a different descriptor closure.
    let other_key = CapabilitySealKeyV1::new(id(0x11), id(0x88), 7, id(0x33), id(0x44))
        .expect("second seal key");
    let mut other_rows = rows();
    if let Some(row) = other_rows.get_mut(SealedRoleV1::Descriptor.ordinal()) {
        *row = SealedRecordRowV1::new(
            SealedRoleV1::Descriptor,
            64,
            id(0x11),
            id(0x88),
            id(0x61),
            id(0x71),
        )
        .expect("descriptor row");
    }
    let mut other_bytes = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(other_key, other_rows, CANONICAL_BUMP, &mut other_bytes)
        .expect("second seal");
    let second = SealedDescriptorClosureV1::decode(&other_bytes).expect("decode");

    let policy_body = body_for(first, SealedRoleV1::LifecyclePolicy);
    let profile_body = body_for(second, SealedRoleV1::AccountProfile);
    let policy = token(first, SealedRoleV1::LifecyclePolicy, &policy_body);
    let foreign_profile = token(second, SealedRoleV1::AccountProfile, &profile_body);

    assert_eq!(
        first
            .authenticate_profile_join(policy, foreign_profile)
            .map(|_| ()),
        Err(Error::DescriptorMismatch)
    );

    let own_profile_body = body_for(first, SealedRoleV1::AccountProfile);
    let own_profile = token(first, SealedRoleV1::AccountProfile, &own_profile_body);
    first
        .authenticate_profile_join(policy, own_profile)
        .expect("own join");
    assert_eq!(
        first
            .authenticate_profile_join(own_profile, policy)
            .map(|_| ()),
        Err(Error::TokenRoleMismatch)
    );
}

#[test]
fn the_ownership_verdict_covers_only_its_four_artifacts_and_its_own_action() {
    let bytes = canonical();
    let seal = SealedDescriptorClosureV1::decode(&bytes).expect("decode");
    let profile_body = body_for(seal, SealedRoleV1::AccountProfile);
    let policy_body = body_for(seal, SealedRoleV1::LifecyclePolicy);
    let request_body = body_for(seal, SealedRoleV1::RequestProfile);
    let transition_body = body_for(seal, SealedRoleV1::TransitionProgram);
    let verdict = seal
        .authenticate_static_ownership(
            token(seal, SealedRoleV1::AccountProfile, &profile_body),
            token(seal, SealedRoleV1::LifecyclePolicy, &policy_body),
            token(seal, SealedRoleV1::RequestProfile, &request_body),
            token(seal, SealedRoleV1::TransitionProgram, &transition_body),
        )
        .expect("ownership verdict");
    verdict
        .require(
            7,
            &profile_body,
            &policy_body,
            &request_body,
            &transition_body,
        )
        .expect("exact coverage");
    assert_eq!(
        verdict.require(
            8,
            &profile_body,
            &policy_body,
            &request_body,
            &transition_body
        ),
        Err(Error::ActionMismatch)
    );
    let twin = profile_body.clone();
    assert_eq!(
        verdict.require(7, &twin, &policy_body, &request_body, &transition_body),
        Err(Error::TokenRangeMismatch)
    );
}

#[test]
fn the_close_request_is_a_discriminator_and_carries_one_bump_candidate() {
    let canonical =
        CapabilitySealCloseRequestV1::new(CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1).to_bytes();
    assert_eq!(canonical.len(), CAPABILITY_SEAL_CLOSE_REQUEST_BYTES_V1);
    assert!(is_capability_seal_close_request_v1(&canonical));
    assert_eq!(
        CapabilitySealCloseRequestV1::decode(&canonical),
        Ok(CapabilitySealCloseRequestV1::new(
            CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1
        ))
    );

    // The two seal outers must never be reachable from one another's bytes.
    // Both dispatch predicates key on magic and width, and both are checked
    // here rather than trusted, because the whole separation between "write a
    // verdict" and "delete an account and pay a stranger" rests on them.
    let write = CapabilitySealRequestV1::new(7, id(0x22))
        .expect("canonical seal request")
        .to_bytes();
    assert!(!is_capability_seal_close_request_v1(&write));
    assert!(!is_capability_seal_request_v1(&canonical));
    assert_eq!(
        CapabilitySealCloseRequestV1::decode(&write),
        Err(Error::InvalidLength)
    );

    // Hostile bytes at the exact width: wrong magic, wrong schema, wrong
    // profile, and a non-zero reserved byte.
    let mut wrong_magic = canonical;
    wrong_magic[0] ^= 0xff;
    assert!(!is_capability_seal_close_request_v1(&wrong_magic));
    assert_eq!(
        CapabilitySealCloseRequestV1::decode(&wrong_magic),
        Err(Error::InvalidMagic)
    );

    let mut wrong_schema = canonical;
    wrong_schema[8] = 2;
    assert_eq!(
        CapabilitySealCloseRequestV1::decode(&wrong_schema),
        Err(Error::UnsupportedSchema)
    );

    // The profile field is the CAP. A future seal class that carries lamports
    // beyond rent exemption is a different profile byte, and this route must
    // refuse it rather than pay those lamports to a closer who has never heard
    // of them.
    let mut wrong_profile = canonical;
    wrong_profile[10] = 2;
    assert_eq!(
        CapabilitySealCloseRequestV1::decode(&wrong_profile),
        Err(Error::UnsupportedArtifactProfile)
    );

    for offset in CAPABILITY_SEAL_CLOSE_RESERVED_OFFSET_V1
        ..CAPABILITY_SEAL_CLOSE_RESERVED_OFFSET_V1 + CAPABILITY_SEAL_CLOSE_RESERVED_BYTES_V1
    {
        let mut dirty = canonical;
        *dirty.get_mut(offset).expect("reserved byte") = 1;
        assert_eq!(
            CapabilitySealCloseRequestV1::decode(&dirty),
            Err(Error::NonCanonicalReserved),
            "reserved byte {offset} was not enforced"
        );
    }

    let mut short = Vec::from(canonical);
    short.pop();
    assert!(!is_capability_seal_close_request_v1(&short));
    assert_eq!(
        CapabilitySealCloseRequestV1::decode(&short),
        Err(Error::InvalidLength)
    );
}

/// The same 968 bytes with byte 20 zeroed, which is what the pre-bump layout
/// left behind.
fn defunct() -> Vec<u8> {
    let mut bytes = canonical();
    *bytes
        .get_mut(CAPABILITY_SEAL_BUMP_OFFSET_V1)
        .expect("sealed canonical bump") = 0;
    bytes
}

/// The disjointness witness, and it is one byte wide.
///
/// The same body is accepted by exactly one of the two arms depending on the
/// value at [`CAPABILITY_SEAL_BUMP_OFFSET_V1`], and the swap runs both ways in
/// one test so neither direction can be satisfied by a decoder that just got
/// looser. No byte string reaches both arms, which is the partition the close
/// route's two arms rest on.
#[test]
fn one_byte_separates_a_defunct_body_from_a_well_formed_one() {
    let mut bytes = canonical();
    SealedDescriptorClosureV1::decode(&bytes).expect("the canonical body decodes");
    assert_eq!(
        SealedDescriptorClosureV1::decode_defunct(&bytes),
        Err(Error::NotDefunct)
    );

    *bytes
        .get_mut(CAPABILITY_SEAL_BUMP_OFFSET_V1)
        .expect("sealed canonical bump") = 0;
    assert_eq!(
        SealedDescriptorClosureV1::decode(&bytes),
        Err(Error::ZeroBump)
    );
    let defunct = SealedDescriptorClosureV1::decode_defunct(&bytes).expect("the defunct body");
    // Every other coordinate is intact: this is a real seal that cannot state
    // the bump reproducing its own address, not a damaged one.
    assert_eq!(defunct.key(), Ok(key()));
    assert_eq!(defunct.bump(), Ok(0));
    for role in SealedRoleV1::canonical_order() {
        assert_eq!(defunct.row(role).map(|row| row.role()), Ok(role));
    }

    *bytes
        .get_mut(CAPABILITY_SEAL_BUMP_OFFSET_V1)
        .expect("sealed canonical bump") = CANONICAL_BUMP;
    SealedDescriptorClosureV1::decode(&bytes).expect("the restored body decodes");
    assert_eq!(
        SealedDescriptorClosureV1::decode_defunct(&bytes),
        Err(Error::NotDefunct)
    );

    // Not just the one bump a search happened to return: every value a
    // derivation can produce is refused by the defunct arm.
    for bump in [1_u8, 0x7f, 0x80, 0xfe, 0xff] {
        let mut bytes = canonical();
        *bytes
            .get_mut(CAPABILITY_SEAL_BUMP_OFFSET_V1)
            .expect("sealed canonical bump") = bump;
        assert_eq!(
            SealedDescriptorClosureV1::decode_defunct(&bytes),
            Err(Error::NotDefunct),
            "bump {bump} reached the defunct arm"
        );
    }

    // Route 2 of the disjointness argument, checked rather than asserted: no
    // writer in this crate can produce the zero the defunct arm reads.
    let mut written = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
    assert_eq!(
        SealedDescriptorClosureV1::encode(key(), rows(), 0, &mut written),
        Err(Error::ZeroBump)
    );
}

/// The defunct arm relaxes the bump byte and nothing else.
///
/// Each case mutates the defunct body at one field the canonical decoder pins
/// and requires the SAME named refusal, so a caller cannot smuggle a
/// non-canonical body past the close by zeroing byte 20. The controls are the
/// unmutated `decode_defunct` in the test above and the identical case table in
/// `every_header_field_is_canonicality_checked`.
#[test]
fn the_defunct_arm_pins_every_field_the_canonical_arm_pins() {
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
        let mut bytes = defunct();
        *bytes.get_mut(offset).expect("header byte") = value;
        assert_eq!(
            SealedDescriptorClosureV1::decode_defunct(&bytes),
            Err(expected),
            "header byte {offset} was not enforced on the defunct arm"
        );
    }

    for width in [CAPABILITY_SEAL_BYTES_V1 - 1, CAPABILITY_SEAL_BYTES_V1 + 1] {
        let mut bytes = defunct();
        bytes.resize(width, 0);
        assert_eq!(
            SealedDescriptorClosureV1::decode_defunct(&bytes),
            Err(Error::InvalidLength)
        );
    }

    // The descriptor row still has to be the key the body is filed under: this
    // is the conjunct that stops a defunct body naming somebody else's
    // artifacts under its own address.
    let mut bytes = defunct();
    let digest = CAPABILITY_SEAL_HEADER_BYTES_V1 + CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1;
    bytes
        .get_mut(digest..digest + 32)
        .expect("descriptor row digest")
        .copy_from_slice(&id(0x99));
    assert_eq!(
        SealedDescriptorClosureV1::decode_defunct(&bytes),
        Err(Error::DescriptorMismatch)
    );

    let role = CAPABILITY_SEAL_HEADER_BYTES_V1
        + CAPABILITY_SEAL_ROW_BYTES_V1
        + CAPABILITY_SEAL_ROW_ROLE_OFFSET_V1;
    let mut bytes = defunct();
    *bytes.get_mut(role).expect("row role") =
        u8::try_from(SealedRoleV1::EffectProgram.tag()).expect("role tag");
    assert_eq!(
        SealedDescriptorClosureV1::decode_defunct(&bytes),
        Err(Error::NonCanonicalRowOrder)
    );
    let mut bytes = defunct();
    *bytes.get_mut(role).expect("row role") = 0x7f;
    assert_eq!(
        SealedDescriptorClosureV1::decode_defunct(&bytes),
        Err(Error::UnknownRole)
    );
    let mut bytes = defunct();
    let reserved = CAPABILITY_SEAL_HEADER_BYTES_V1 + CAPABILITY_SEAL_ROW_RESERVED_OFFSET_V1;
    *bytes.get_mut(reserved).expect("row reserved") = 1;
    assert_eq!(
        SealedDescriptorClosureV1::decode_defunct(&bytes),
        Err(Error::NonCanonicalReserved)
    );
}

/// The bump candidate rides a byte the wire already reserved, and the ordinary
/// request is byte-for-byte the one already deployed.
#[test]
fn the_close_request_bump_candidate_spends_one_reserved_byte() {
    // The deployed wire, written out rather than rebuilt, so a change to the
    // builder cannot quietly move it.
    let deployed: [u8; CAPABILITY_SEAL_CLOSE_REQUEST_BYTES_V1] = [
        b'D', b'C', b'L', b'T', b'C', b'S', b'X', b'1', 1, 0, 1, 0, 0, 0, 0, 0,
    ];
    assert_eq!(
        CapabilitySealCloseRequestV1::new(CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1).to_bytes(),
        deployed,
        "the ordinary close request stopped being the wire already deployed"
    );
    assert_eq!(CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1, 0);

    // Every candidate a bump search can return, and the whole u8 besides.
    for candidate in 0..=u8::MAX {
        let wire = CapabilitySealCloseRequestV1::new(candidate).to_bytes();
        assert_eq!(
            wire.get(CAPABILITY_SEAL_CLOSE_BUMP_CANDIDATE_OFFSET_V1),
            Some(&candidate)
        );
        assert!(is_capability_seal_close_request_v1(&wire));
        assert_eq!(
            CapabilitySealCloseRequestV1::decode(&wire).map(|request| request.bump_candidate()),
            Ok(candidate)
        );
        // The candidate rides byte 12 and nothing else moves with it.
        let mut expected = deployed;
        *expected
            .get_mut(CAPABILITY_SEAL_CLOSE_BUMP_CANDIDATE_OFFSET_V1)
            .expect("bump candidate") = candidate;
        assert_eq!(wire, expected);
    }
}
