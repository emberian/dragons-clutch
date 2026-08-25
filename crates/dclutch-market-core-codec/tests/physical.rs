//! Hostile coverage for the Lean-owned cross-program physical ABI.

use dclutch_market_core_codec::{
    CORE_EFFECT_ACK_BYTES_V1, CORE_EFFECT_ENVELOPE_BYTES_V1, CoreEffectAckV1, CoreEffectActionV1,
    CoreEffectEnvelopeV1, Error, Identity, Role, SERIES_CORE_REQUEST_BYTES_V1, SeriesCoreActionV1,
    SeriesCoreRequestV1,
};
use dclutch_release_set_contract::{CALLER_AUTHORITY_PDA_DOMAIN_V1, ExecutionRoleV1};

fn id(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("fixture identity is nonzero")
}

fn envelope() -> CoreEffectEnvelopeV1 {
    CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::CreateFund,
        Role::Resolution,
        id(1),
        id(2),
        id(3),
        id(4),
        id(5),
        id(6),
        id(7),
        8,
        9,
        10,
        416,
    )
    .expect("valid envelope fixture")
}

fn ack() -> CoreEffectAckV1 {
    CoreEffectAckV1::new(
        CoreEffectActionV1::CreateFund,
        Role::Resolution,
        id(20),
        id(3),
        id(4),
        id(5),
        id(21),
        id(22),
        9,
        10,
        10,
        12,
    )
    .expect("valid acknowledgment fixture")
}

#[test]
fn effect_envelope_is_exact_role_bound_and_round_trips() {
    let envelope = envelope();
    let bytes = envelope.encode().expect("envelope encodes");
    assert_eq!(bytes.len(), CORE_EFFECT_ENVELOPE_BYTES_V1);
    assert_eq!(CoreEffectEnvelopeV1::decode(&bytes), Ok(envelope));
    assert_eq!(envelope.action(), CoreEffectActionV1::CreateFund);
    assert_eq!(envelope.target_role(), Role::Resolution);
    assert_eq!(envelope.caller_program(), id(1));
    assert_eq!(envelope.caller_authority(), id(2));
    assert_eq!(envelope.release_set(), id(3));
    assert_eq!(envelope.market(), id(4));
    assert_eq!(envelope.context(), id(5));
    assert_eq!(envelope.parent_state_digest(), id(6));
    assert_eq!(envelope.role_request_digest(), id(7));
    assert_eq!(envelope.generation(), 8);
    assert_eq!(envelope.expected_resource_a_revision(), 9);
    assert_eq!(envelope.expected_resource_b_revision(), 10);
    assert_eq!(envelope.role_request_bytes(), 416);
    let release_set = id(3).to_bytes();
    let market = id(4).to_bytes();
    let caller_role = [ExecutionRoleV1::Core as u8];
    let context = id(5).to_bytes();
    let request_digest = id(7).to_bytes();
    let expected_seeds: [&[u8]; 6] = [
        CALLER_AUTHORITY_PDA_DOMAIN_V1,
        release_set.as_slice(),
        market.as_slice(),
        caller_role.as_slice(),
        context.as_slice(),
        request_digest.as_slice(),
    ];
    assert_eq!(
        envelope
            .caller_authority_seeds()
            .expect("valid Core caller authority")
            .as_slices(),
        expected_seeds
    );
    assert_eq!(envelope.validate_role_request(416, id(7)), Ok(()));
    assert_eq!(
        CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::CreateFund,
            Role::Custody,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            8,
            9,
            10,
            416,
        ),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        envelope.validate_role_request(415, id(7)),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(
        envelope.validate_role_request(416, id(8)),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn effect_envelope_hostile_bytes_are_refused() {
    let bytes = envelope().encode().expect("fixture encodes");
    let short = bytes
        .get(..bytes.len().saturating_sub(1))
        .expect("fixture has a shorter prefix");
    assert_eq!(
        CoreEffectEnvelopeV1::decode(short),
        Err(Error::InvalidLength)
    );
    let mut long = bytes.to_vec();
    long.push(0);
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&long),
        Err(Error::InvalidLength)
    );

    let mut hostile = bytes;
    hostile[0] ^= 1;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidMagic)
    );
    let mut hostile = bytes;
    hostile[8] = 2;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::UnsupportedVersion)
    );
    let mut hostile = bytes;
    hostile[10] = u8::MAX;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidTag)
    );
    let mut hostile = bytes;
    hostile[11] = Role::Custody as u8;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
    let mut hostile = bytes;
    hostile[12] = 1;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[16..48].fill(0);
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::InvalidIdentity)
    );
    let mut hostile = bytes;
    hostile[268] = 1;
    assert_eq!(
        CoreEffectEnvelopeV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
}

#[test]
fn acknowledgement_binds_full_effect_and_monotonic_revisions() {
    let envelope = envelope();
    let ack = ack();
    let bytes = ack.encode().expect("acknowledgment encodes");
    assert_eq!(bytes.len(), CORE_EFFECT_ACK_BYTES_V1);
    assert_eq!(CoreEffectAckV1::decode(&bytes), Ok(ack));
    assert_eq!(ack.validate_for(envelope, id(20), id(21)), Ok(()));
    assert_eq!(ack.action(), CoreEffectActionV1::CreateFund);
    assert_eq!(ack.target_role(), Role::Resolution);
    assert_eq!(ack.role_program(), id(20));
    assert_eq!(ack.release_set(), id(3));
    assert_eq!(ack.market(), id(4));
    assert_eq!(ack.context(), id(5));
    assert_eq!(ack.effect_digest(), id(21));
    assert_eq!(ack.post_resource_digest(), id(22));
    assert_eq!(ack.pre_resource_a_revision(), 9);
    assert_eq!(ack.post_resource_a_revision(), 10);
    assert_eq!(ack.pre_resource_b_revision(), 10);
    assert_eq!(ack.post_resource_b_revision(), 12);
    assert_eq!(
        ack.validate_for(envelope, id(23), id(21)),
        Err(Error::InvalidRelease)
    );
    assert_eq!(
        ack.validate_for(envelope, id(20), id(23)),
        Err(Error::InvalidRelease)
    );
    assert_eq!(
        CoreEffectAckV1::new(
            CoreEffectActionV1::CreateFund,
            Role::Resolution,
            id(20),
            id(3),
            id(4),
            id(5),
            id(21),
            id(22),
            9,
            8,
            10,
            12,
        ),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn acknowledgement_hostile_bytes_are_refused() {
    let bytes = ack().encode().expect("fixture encodes");
    let short = bytes
        .get(..bytes.len().saturating_sub(1))
        .expect("fixture has a shorter prefix");
    assert_eq!(CoreEffectAckV1::decode(short), Err(Error::InvalidLength));
    let mut hostile = bytes;
    hostile[12] = 1;
    assert_eq!(
        CoreEffectAckV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[16..48].fill(0);
    assert_eq!(
        CoreEffectAckV1::decode(&hostile),
        Err(Error::InvalidIdentity)
    );
    let mut hostile = bytes;
    hostile[216..224].copy_from_slice(&8_u64.to_le_bytes());
    assert_eq!(
        CoreEffectAckV1::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
}

fn occurrence(action: SeriesCoreActionV1) -> SeriesCoreRequestV1 {
    SeriesCoreRequestV1::occurrence(
        action,
        id(30),
        id(31),
        id(32),
        id(33),
        id(34),
        id(35),
        id(36),
        id(37),
        38,
        39,
        40,
        41,
        42,
        43,
        44,
    )
    .expect("valid occurrence fixture")
}

#[test]
fn series_occurrence_request_is_exact_and_round_trips() {
    for action in [
        SeriesCoreActionV1::Prepare,
        SeriesCoreActionV1::Consume,
        SeriesCoreActionV1::Expire,
    ] {
        let request = occurrence(action);
        let bytes = request.encode().expect("request encodes");
        assert_eq!(bytes.len(), SERIES_CORE_REQUEST_BYTES_V1);
        assert_eq!(SeriesCoreRequestV1::decode(&bytes), Ok(request));
        assert_eq!(request.action(), action);
        assert_eq!(request.release_set(), id(30));
        assert_eq!(request.template(), id(31));
        assert_eq!(request.ticket(), Some(id(32)));
        assert_eq!(request.market(), Some(id(33)));
        assert_eq!(request.realm(), Some(id(34)));
        assert_eq!(request.product(), Some(id(35)));
        assert_eq!(request.beneficiary(), id(36));
        assert_eq!(request.founder(), Some(id(37)));
        assert_eq!(request.occurrence_index(), 38);
        assert_eq!(request.expected_series_revision(), 39);
        assert_eq!(request.expected_ticket_revision(), 40);
        assert_eq!(request.market_rent(), 41);
        assert_eq!(request.capability_rent(), 42);
        assert_eq!(request.work(), 43);
        assert_eq!(request.hoard_principal(), 44);
        assert_eq!(request.series_close_rent(), 0);
    }
}

#[test]
fn series_close_is_disjoint_from_occurrence_shape() {
    let close =
        SeriesCoreRequestV1::close(id(30), id(31), id(36), 39, 44).expect("valid close fixture");
    let bytes = close.encode().expect("close encodes");
    assert_eq!(SeriesCoreRequestV1::decode(&bytes), Ok(close));
    assert_eq!(close.action(), SeriesCoreActionV1::Close);
    assert_eq!(close.ticket(), None);
    assert_eq!(close.market(), None);
    assert_eq!(close.realm(), None);
    assert_eq!(close.product(), None);
    assert_eq!(close.founder(), None);
    assert_eq!(close.series_close_rent(), 44);

    let mut hostile = bytes;
    hostile[80] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[240] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[272] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[288] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
}

#[test]
fn series_hostile_tags_and_inactive_fields_are_refused_without_invented_minima() {
    assert_eq!(
        SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Close,
            id(30),
            id(31),
            id(32),
            id(33),
            id(34),
            id(35),
            id(36),
            id(37),
            38,
            39,
            40,
            41,
            42,
            43,
            44,
        ),
        Err(Error::InvalidCoordinates)
    );
    let zero_close = SeriesCoreRequestV1::close(id(30), id(31), id(36), 39, 0)
        .expect("Series owns the exact zero close-rent fact");
    assert_eq!(
        SeriesCoreRequestV1::decode(&zero_close.encode().expect("zero close encodes")),
        Ok(zero_close)
    );
    let zero_occurrence = SeriesCoreRequestV1::occurrence(
        SeriesCoreActionV1::Prepare,
        id(30),
        id(31),
        id(32),
        id(33),
        id(34),
        id(35),
        id(36),
        id(37),
        38,
        39,
        40,
        0,
        0,
        0,
        44,
    )
    .expect("Series owns exact zero occurrence compartments");
    assert_eq!(
        SeriesCoreRequestV1::decode(&zero_occurrence.encode().expect("zero request encodes")),
        Ok(zero_occurrence)
    );
    let bytes = occurrence(SeriesCoreActionV1::Consume)
        .encode()
        .expect("fixture encodes");
    let mut hostile = bytes;
    hostile[10] = u8::MAX;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::InvalidTag)
    );
    let mut hostile = bytes;
    hostile[11] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile[328] = 1;
    assert_eq!(
        SeriesCoreRequestV1::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
}
