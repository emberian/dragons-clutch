//! Hostile coverage for the Found-only Series Core acknowledgment successor.

use dclutch_market_core_codec::{
    Error, Identity, SERIES_CORE_FOUND_ACK_BYTES_V2,
    SERIES_CORE_FOUND_ACK_FUNDING_LIST_ID_OFFSET_V2, SERIES_CORE_FOUND_ACK_SCHEMA_RELEASE_ID_V2,
    SeriesCoreActionV1, SeriesCoreFoundAckV2, SeriesCoreRequestV1,
};

fn id(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("nonzero identity")
}

fn request() -> SeriesCoreRequestV1 {
    SeriesCoreRequestV1::occurrence(
        SeriesCoreActionV1::Consume,
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
    .expect("valid Consume request")
}

#[test]
fn found_ack_binds_authenticated_funding_span_and_permit() {
    let request = request();
    let ack = SeriesCoreFoundAckV2::new(request, id(51), id(52), id(53), 3, id(54), id(55))
        .expect("valid Found acknowledgment");
    let bytes = ack.encode().expect("encode");
    assert_eq!(bytes.len(), SERIES_CORE_FOUND_ACK_BYTES_V2);
    assert_eq!(SeriesCoreFoundAckV2::decode(&bytes), Ok(ack));
    assert_eq!(ack.funding_count(), 3);
    assert_eq!(ack.core_program(), id(51));
    assert_eq!(ack.release_set(), id(30));
    assert_eq!(ack.template(), id(31));
    assert_eq!(ack.ticket(), id(32));
    assert_eq!(ack.market(), id(33));
    assert_eq!(ack.permit(), id(52));
    assert_eq!(ack.request_digest(), id(53));
    assert_eq!(ack.funding_list_id(), id(54));
    assert_eq!(ack.post_resource_digest(), id(55));
    assert_eq!(ack.market_generation(), 39);
    assert_eq!(ack.expected_series_revision(), 39);
    assert_eq!(ack.expected_ticket_revision(), 40);
    assert_eq!(
        ack.validate_for(request, id(51), id(52), id(53), 3, id(54), id(55)),
        Ok(())
    );
    assert_eq!(
        ack.validate_for(request, id(51), id(52), id(53), 2, id(54), id(55)),
        Err(Error::InvalidRelease)
    );
    assert_eq!(
        ack.validate_for(request, id(51), id(52), id(53), 3, id(56), id(55)),
        Err(Error::InvalidRelease)
    );
    assert_ne!(SERIES_CORE_FOUND_ACK_SCHEMA_RELEASE_ID_V2, [0; 32]);
}

#[test]
fn hostile_width_header_funding_and_identity_substitution_refuse() {
    let request = request();
    let bytes = SeriesCoreFoundAckV2::new(request, id(51), id(52), id(53), 3, id(54), id(55))
        .expect("ack")
        .encode()
        .expect("encode");
    assert_eq!(
        SeriesCoreFoundAckV2::decode(&bytes[..SERIES_CORE_FOUND_ACK_BYTES_V2 - 1]),
        Err(Error::InvalidLength)
    );
    let mut hostile = bytes;
    hostile[0] ^= 1;
    assert_eq!(
        SeriesCoreFoundAckV2::decode(&hostile),
        Err(Error::InvalidMagic)
    );
    let mut hostile = bytes;
    hostile[8] = 1;
    assert_eq!(
        SeriesCoreFoundAckV2::decode(&hostile),
        Err(Error::UnsupportedVersion)
    );
    let mut hostile = bytes;
    hostile[10] = 0;
    assert_eq!(
        SeriesCoreFoundAckV2::decode(&hostile),
        Err(Error::InvalidCoordinates)
    );
    let mut hostile = bytes;
    hostile[11] = 1;
    assert_eq!(
        SeriesCoreFoundAckV2::decode(&hostile),
        Err(Error::NonzeroReserved)
    );
    let mut hostile = bytes;
    hostile
        .get_mut(
            SERIES_CORE_FOUND_ACK_FUNDING_LIST_ID_OFFSET_V2
                ..SERIES_CORE_FOUND_ACK_FUNDING_LIST_ID_OFFSET_V2 + 32,
        )
        .expect("funding-list field")
        .fill(0);
    assert_eq!(
        SeriesCoreFoundAckV2::decode(&hostile),
        Err(Error::InvalidIdentity)
    );

    let close = SeriesCoreRequestV1::close(id(30), id(31), id(36), 39, 0).expect("close");
    assert_eq!(
        SeriesCoreFoundAckV2::new(close, id(51), id(52), id(53), 3, id(54), id(55)),
        Err(Error::InvalidCoordinates)
    );
}
