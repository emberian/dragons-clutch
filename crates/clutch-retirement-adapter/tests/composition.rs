mod common;

use clutch_retirement::{
    ChildGenerationV1, RetirementErrorV1, DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
    DIRECT_RESERVATION_V2_BYTES, DIRECT_RESERVATION_V6_BYTES, EPOCH_ACCOUNT_VERSION_V5,
    EPOCH_V5_BYTES, MARKET_ACCOUNT_VERSION_V2, MARKET_V2_BYTES, POSITION_ACCOUNT_VERSION_V2,
    POSITION_V2_BYTES, RESERVATION_ACCOUNT_VERSION_V5, RESERVATION_V4_BYTES, RESERVATION_V5_BYTES,
};
use clutch_retirement_adapter::{
    decode_counted_child, encode_counted_child_after_base_validation, CountedChildSchemaV1,
    DirectReservationAccountV6, GeneralEpochAccountV5, GeneralReservationAccountV5,
    MarketAccountV2, PositionAccountV2, RetirementAdapterErrorV1,
};
use clutch_solana_layout::CodecError;

#[test]
fn frozen_promoted_versions_do_not_reinterpret_existing_family_schemas() {
    assert_eq!(
        clutch_solana_layout::direct_selection::DIRECT_EPOCH_VERSION,
        3
    );
    assert_eq!(
        clutch_solana_layout::direct_selection_v3::DIRECT_EPOCH_V4_VERSION,
        4
    );
    assert_eq!(EPOCH_ACCOUNT_VERSION_V5, 5);
    assert_eq!(
        clutch_solana_layout::direct_selection_v3::DIRECT_RESERVATION_V2_VERSION,
        2
    );
    assert_eq!(
        clutch_solana_layout::reservation::RESERVATION_ACCOUNT_VERSION,
        4
    );
    assert_eq!(RESERVATION_ACCOUNT_VERSION_V5, 5);
    assert_eq!(DIRECT_RESERVATION_ACCOUNT_VERSION_V6, 6);
}

#[test]
fn all_authoritative_base_plus_tail_compositions_round_trip_exactly() {
    let position = common::position_v2();
    let position_bytes = position.encode().unwrap();
    assert_eq!(position_bytes.len(), POSITION_V2_BYTES);
    assert_eq!(position_bytes[1], POSITION_ACCOUNT_VERSION_V2);
    assert_eq!(PositionAccountV2::decode(&position_bytes), Ok(position));

    let market = common::market_v2();
    let market_bytes = market.encode().unwrap();
    assert_eq!(market_bytes.len(), MARKET_V2_BYTES);
    assert_eq!(market_bytes[1], MARKET_ACCOUNT_VERSION_V2);
    assert_eq!(MarketAccountV2::decode(&market_bytes), Ok(market));

    let epoch = common::epoch_v5();
    let epoch_bytes = epoch.encode().unwrap();
    assert_eq!(epoch_bytes.len(), EPOCH_V5_BYTES);
    assert_eq!(epoch_bytes[1], EPOCH_ACCOUNT_VERSION_V5);
    assert_eq!(GeneralEpochAccountV5::decode(&epoch_bytes), Ok(epoch));

    let general = common::general_reservation_v5();
    let general_bytes = general.encode().unwrap();
    assert_eq!(general_bytes.len(), RESERVATION_V5_BYTES);
    assert_eq!(general_bytes[1], RESERVATION_ACCOUNT_VERSION_V5);
    assert_eq!(
        GeneralReservationAccountV5::decode(&general_bytes),
        Ok(general)
    );

    let direct = common::direct_reservation_v6();
    let direct_bytes = direct.encode(common::direct_sink()).unwrap();
    assert_eq!(direct_bytes.len(), DIRECT_RESERVATION_V6_BYTES);
    assert_eq!(direct_bytes[1], DIRECT_RESERVATION_ACCOUNT_VERSION_V6);
    assert_eq!(
        DirectReservationAccountV6::decode(&direct_bytes, common::direct_sink()),
        Ok(direct)
    );
}

#[test]
fn exact_envelopes_refuse_lengths_headers_cross_family_and_hostile_base_fields() {
    let position = common::position_v2().encode().unwrap();
    assert_eq!(
        PositionAccountV2::decode(&position[..POSITION_V2_BYTES - 1]),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::Truncated
        ))
    );
    let mut long = position.to_vec();
    long.push(0);
    assert_eq!(
        PositionAccountV2::decode(&long),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::TrailingBytes
        ))
    );
    let mut wrong = position;
    wrong[0] = wrong[0].wrapping_add(1);
    assert_eq!(
        PositionAccountV2::decode(&wrong),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::WrongTag
        ))
    );
    wrong = position;
    wrong[1] = wrong[1].wrapping_add(1);
    assert_eq!(
        PositionAccountV2::decode(&wrong),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::WrongVersion
        ))
    );
    wrong = position;
    wrong[2..34].fill(0);
    assert_eq!(
        PositionAccountV2::decode(&wrong),
        Err(RetirementAdapterErrorV1::BaseCodec(
            CodecError::ZeroIdentity
        ))
    );

    let general = common::general_reservation_v5().encode().unwrap();
    let direct = common::direct_reservation_v6()
        .encode(common::direct_sink())
        .unwrap();
    assert_eq!(general.len(), direct.len());
    assert_eq!(
        GeneralReservationAccountV5::decode(&direct),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::WrongVersion
        ))
    );
    assert_eq!(
        DirectReservationAccountV6::decode(&general, common::direct_sink()),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::WrongVersion
        ))
    );
}

#[test]
fn reservation_base_phase_and_position_count_marker_are_one_canonical_state() {
    let mut general = common::general_reservation_v5();
    general.count.position_counted = false;
    assert_eq!(
        general.encode(),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::NonCanonicalState
        ))
    );
    general.count.position_counted = true;
    let mut general_bytes = general.encode().unwrap();
    general_bytes[RESERVATION_V4_BYTES + 8] = 0;
    assert_eq!(
        GeneralReservationAccountV5::decode(&general_bytes),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::NonCanonicalState
        ))
    );
    let mut general_terminal = general;
    general_terminal.base = general_terminal.base.released(9).unwrap();
    assert_eq!(
        general_terminal.encode(),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::NonCanonicalState
        ))
    );
    general_terminal.count.position_counted = false;
    assert!(general_terminal.encode().is_ok());

    let mut direct = common::direct_reservation_v6();
    direct.count.position_counted = false;
    assert_eq!(
        direct.encode(common::direct_sink()),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::NonCanonicalState
        ))
    );
    direct.count.position_counted = true;
    let mut direct_bytes = direct.encode(common::direct_sink()).unwrap();
    direct_bytes[DIRECT_RESERVATION_V2_BYTES + 8] = 0;
    assert_eq!(
        DirectReservationAccountV6::decode(&direct_bytes, common::direct_sink()),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::NonCanonicalState
        ))
    );
    let mut direct_terminal = direct;
    direct_terminal.base.reservation = direct_terminal.base.reservation.released(9).unwrap();
    assert_eq!(
        direct_terminal.encode(common::direct_sink()),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::NonCanonicalState
        ))
    );
    direct_terminal.count.position_counted = false;
    assert!(direct_terminal.encode(common::direct_sink()).is_ok());
}

#[test]
fn every_full_composition_decoder_is_total_for_hostile_lengths_and_bytes() {
    const MAX_BYTES: usize = MARKET_V2_BYTES + 1;
    let mut hostile = [0u8; MAX_BYTES];
    let mut state = 0x4d59_5df4_d0f3_3173u64;
    for byte in &mut hostile {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = state as u8;
    }

    for len in 0..=POSITION_V2_BYTES + 1 {
        let _ = PositionAccountV2::decode(&hostile[..len]);
    }
    for len in 0..=MARKET_V2_BYTES + 1 {
        let _ = MarketAccountV2::decode(&hostile[..len]);
    }
    for len in 0..=EPOCH_V5_BYTES + 1 {
        let _ = GeneralEpochAccountV5::decode(&hostile[..len]);
    }
    for len in 0..=RESERVATION_V5_BYTES + 1 {
        let _ = GeneralReservationAccountV5::decode(&hostile[..len]);
    }
    for len in 0..=DIRECT_RESERVATION_V6_BYTES + 1 {
        let _ = DirectReservationAccountV6::decode(&hostile[..len], common::direct_sink());
    }
}

#[test]
fn generic_counted_child_codec_is_exact_total_and_round_trips_legacy_bytes() {
    let schema = CountedChildSchemaV1::after_registry_allocation(33, 4, 5, 12, 10).unwrap();
    let mut base = [0u8; 12];
    base[0] = 33;
    base[1] = 4;
    base[2..10].copy_from_slice(&99u64.to_le_bytes());
    base[10] = 7;
    let generation = ChildGenerationV1 {
        epoch_generation: 88,
    };
    let mut counted = [0u8; 20];
    encode_counted_child_after_base_validation(schema, &base, generation, &mut counted).unwrap();
    assert_eq!(&counted[..2], &[33, 5]);
    assert_eq!(&counted[12..], &88u64.to_le_bytes());

    let mut decoded_base = [0u8; 12];
    assert_eq!(
        decode_counted_child(schema, &counted, &mut decoded_base),
        Ok(generation)
    );
    assert_eq!(decoded_base, base);

    for bad_len in [0usize, 1, 11, 13, 19] {
        let mut output = [0u8; 12];
        assert!(decode_counted_child(schema, &counted[..bad_len], &mut output).is_err());
    }
    let mut bad_generation = counted;
    bad_generation[12..].fill(0);
    let mut output = [0u8; 12];
    assert_eq!(
        decode_counted_child(schema, &bad_generation, &mut output),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::WrongGeneration
        ))
    );

    assert_eq!(RESERVATION_V4_BYTES, DIRECT_RESERVATION_V2_BYTES);
}
