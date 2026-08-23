mod common;

use clutch_retirement::{
    ChildGenerationV1, GeneralEpochPhaseV2, RetirementErrorV1, RetirementErrorV2,
    DIRECT_RESERVATION_ACCOUNT_VERSION_V6, DIRECT_RESERVATION_ACCOUNT_VERSION_V8,
    DIRECT_RESERVATION_V2_BYTES, DIRECT_RESERVATION_V6_BYTES, DIRECT_RESERVATION_V8_BYTES,
    EPOCH_ACCOUNT_VERSION_V5, EPOCH_V2_BYTES, EPOCH_V5_BYTES, MARKET_ACCOUNT_VERSION_V2,
    MARKET_V2_BYTES, POSITION_ACCOUNT_VERSION_V2, POSITION_V2_BYTES,
    RESERVATION_ACCOUNT_VERSION_V5, RESERVATION_ACCOUNT_VERSION_V7, RESERVATION_V4_BYTES,
    RESERVATION_V5_BYTES, RESERVATION_V7_BYTES,
};
use clutch_retirement_adapter::{
    decode_counted_child, encode_counted_child_after_base_validation,
    project_general_epoch_phase_v2, project_live_general_epoch_retirement_v2, CountedChildSchemaV1,
    DirectReservationAccountV6, DirectReservationAccountV8, GeneralEpochAccountV5,
    GeneralReservationAccountV5, GeneralReservationAccountV7, MarketAccountV2, PositionAccountV2,
    RetirementAdapterErrorV1, RetirementAdapterErrorV2,
};
use clutch_solana_layout::{
    CodecError, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED, EPOCH_PHASE_OPEN,
    EPOCH_PHASE_SETTLED,
};

#[test]
fn frozen_promoted_versions_do_not_reinterpret_existing_family_schemas() {
    assert_eq!(common::direct_epoch_v4(7).direct.common.epoch_index, 7);
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
    assert_eq!(RESERVATION_ACCOUNT_VERSION_V7, 7);
    assert_eq!(DIRECT_RESERVATION_ACCOUNT_VERSION_V8, 8);
    assert_eq!(EPOCH_V5_BYTES, 429);
    assert_eq!(RESERVATION_V5_BYTES, 627);
    assert_eq!(DIRECT_RESERVATION_V6_BYTES, 627);
    assert_eq!(RESERVATION_V7_BYTES, 675);
    assert_eq!(DIRECT_RESERVATION_V8_BYTES, 675);
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

    let general = common::general_reservation_v7();
    let general_bytes = general.encode().unwrap();
    assert_eq!(general_bytes.len(), RESERVATION_V7_BYTES);
    assert_eq!(general_bytes[1], RESERVATION_ACCOUNT_VERSION_V7);
    assert_eq!(
        GeneralReservationAccountV7::decode(&general_bytes),
        Ok(general)
    );

    let direct = common::direct_reservation_v8();
    let direct_bytes = direct.encode(common::direct_sink()).unwrap();
    assert_eq!(direct_bytes.len(), DIRECT_RESERVATION_V8_BYTES);
    assert_eq!(direct_bytes[1], DIRECT_RESERVATION_ACCOUNT_VERSION_V8);
    assert_eq!(
        DirectReservationAccountV8::decode(&direct_bytes, common::direct_sink()),
        Ok(direct)
    );
}

#[test]
fn frozen_reservation_envelopes_equal_independently_composed_base_and_tail_bytes() {
    let general_v5 = common::general_reservation_v5();
    let mut general_base = [0u8; RESERVATION_V4_BYTES];
    assert_eq!(
        general_v5.base.encode(&mut general_base),
        Ok(RESERVATION_V4_BYTES)
    );
    let mut expected_general_v5 = [0u8; RESERVATION_V5_BYTES];
    expected_general_v5[..RESERVATION_V4_BYTES].copy_from_slice(&general_base);
    expected_general_v5[1] = RESERVATION_ACCOUNT_VERSION_V5;
    expected_general_v5[RESERVATION_V4_BYTES..]
        .copy_from_slice(&general_v5.count.encode().unwrap());
    assert_eq!(general_v5.encode().unwrap(), expected_general_v5);
    assert_eq!(
        GeneralReservationAccountV5::decode(&expected_general_v5),
        Ok(general_v5)
    );

    let direct_v6 = common::direct_reservation_v6();
    let mut direct_base = [0u8; DIRECT_RESERVATION_V2_BYTES];
    assert_eq!(
        direct_v6
            .base
            .encode(common::direct_sink(), &mut direct_base),
        Ok(DIRECT_RESERVATION_V2_BYTES)
    );
    let mut expected_direct_v6 = [0u8; DIRECT_RESERVATION_V6_BYTES];
    expected_direct_v6[..DIRECT_RESERVATION_V2_BYTES].copy_from_slice(&direct_base);
    expected_direct_v6[1] = DIRECT_RESERVATION_ACCOUNT_VERSION_V6;
    expected_direct_v6[DIRECT_RESERVATION_V2_BYTES..]
        .copy_from_slice(&direct_v6.count.encode().unwrap());
    assert_eq!(
        direct_v6.encode(common::direct_sink()).unwrap(),
        expected_direct_v6
    );
    assert_eq!(
        DirectReservationAccountV6::decode(&expected_direct_v6, common::direct_sink()),
        Ok(direct_v6)
    );

    let general_v7 = common::general_reservation_v7();
    let mut expected_general_v7 = [0u8; RESERVATION_V7_BYTES];
    expected_general_v7[..RESERVATION_V4_BYTES].copy_from_slice(&general_base);
    expected_general_v7[1] = RESERVATION_ACCOUNT_VERSION_V7;
    expected_general_v7[RESERVATION_V4_BYTES..]
        .copy_from_slice(&general_v7.retirement.encode().unwrap());
    assert_eq!(general_v7.encode().unwrap(), expected_general_v7);
    assert_eq!(
        GeneralReservationAccountV7::decode(&expected_general_v7),
        Ok(general_v7)
    );

    let direct_v8 = common::direct_reservation_v8();
    let mut expected_direct_v8 = [0u8; DIRECT_RESERVATION_V8_BYTES];
    expected_direct_v8[..DIRECT_RESERVATION_V2_BYTES].copy_from_slice(&direct_base);
    expected_direct_v8[1] = DIRECT_RESERVATION_ACCOUNT_VERSION_V8;
    expected_direct_v8[DIRECT_RESERVATION_V2_BYTES..]
        .copy_from_slice(&direct_v8.retirement.encode().unwrap());
    assert_eq!(
        direct_v8.encode(common::direct_sink()).unwrap(),
        expected_direct_v8
    );
    assert_eq!(
        DirectReservationAccountV8::decode(&expected_direct_v8, common::direct_sink()),
        Ok(direct_v8)
    );
}

#[test]
fn frozen_general_epoch_v5_preserves_its_nonzero_generation_decoder() {
    let mut epoch = common::epoch_v5();
    epoch.retirement.epoch_generation = 9;
    let bytes = epoch.encode().unwrap();
    assert_eq!(GeneralEpochAccountV5::decode(&bytes), Ok(epoch));

    let mut bytes = common::epoch_v5().encode().unwrap();
    bytes[EPOCH_V2_BYTES..EPOCH_V2_BYTES + 8].copy_from_slice(&9u64.to_le_bytes());
    assert_eq!(
        GeneralEpochAccountV5::decode(&bytes)
            .unwrap()
            .retirement
            .epoch_generation,
        9
    );

    let mut zero = common::epoch_v5();
    zero.retirement.epoch_generation = 0;
    assert_eq!(
        zero.encode(),
        Err(RetirementAdapterErrorV1::Retirement(
            RetirementErrorV1::WrongGeneration
        ))
    );
}

#[test]
fn general_epoch_phase_projection_is_exact_and_unknown_bytes_refuse() {
    assert_eq!(EPOCH_PHASE_OPEN, 0);
    assert_eq!(EPOCH_PHASE_FROZEN, 1);
    assert_eq!(EPOCH_PHASE_CLEARED, 2);
    assert_eq!(EPOCH_PHASE_SETTLED, 3);
    assert_eq!(EPOCH_PHASE_LAPSED, 4);
    for (wire, projected) in [
        (EPOCH_PHASE_OPEN, GeneralEpochPhaseV2::Open),
        (EPOCH_PHASE_FROZEN, GeneralEpochPhaseV2::Frozen),
        (EPOCH_PHASE_CLEARED, GeneralEpochPhaseV2::Cleared),
        (EPOCH_PHASE_SETTLED, GeneralEpochPhaseV2::Settled),
        (EPOCH_PHASE_LAPSED, GeneralEpochPhaseV2::Lapsed),
    ] {
        assert_eq!(project_general_epoch_phase_v2(wire), Ok(projected));
    }
    for hostile in 5u8..=u8::MAX {
        assert_eq!(
            project_general_epoch_phase_v2(hostile),
            Err(RetirementAdapterErrorV2::Retirement(
                RetirementErrorV2::InvalidEnum
            ))
        );
    }

    let account = common::epoch_v5();
    let projected = project_live_general_epoch_retirement_v2(account).unwrap();
    assert_eq!(projected.market.bytes(), account.base.market.bytes());
    assert_eq!(projected.epoch.bytes(), account.base.epoch.bytes());
    assert_eq!(projected.epoch_index, account.base.epoch_index);
    assert_eq!(projected.phase, GeneralEpochPhaseV2::Open);
    assert_eq!(projected.stored_bump, account.base.stored_bump);
    assert_eq!(projected.retirement, account.retirement);

    let mut legacy_only = account;
    legacy_only.retirement.epoch_generation = 9;
    assert_eq!(
        project_live_general_epoch_retirement_v2(legacy_only),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::WrongGeneration
        ))
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

    let general = common::general_reservation_v7().encode().unwrap();
    let direct = common::direct_reservation_v8()
        .encode(common::direct_sink())
        .unwrap();
    assert_eq!(general.len(), direct.len());
    assert_eq!(
        GeneralReservationAccountV7::decode(&direct),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::WrongVersion
        ))
    );
    assert_eq!(
        DirectReservationAccountV8::decode(&general, common::direct_sink()),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::WrongVersion
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
fn every_deletable_reservation_requires_its_embedded_funding_owner() {
    let general = common::general_reservation_v7().encode().unwrap();
    let mut zero_payer = general;
    zero_payer[RESERVATION_V4_BYTES + 9..RESERVATION_V4_BYTES + 41].fill(0);
    assert_eq!(
        GeneralReservationAccountV7::decode(&zero_payer),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::ZeroIdentity
        ))
    );
    let mut zero_principal = general;
    zero_principal[RESERVATION_V4_BYTES + 41..RESERVATION_V4_BYTES + 49].fill(0);
    assert_eq!(
        GeneralReservationAccountV7::decode(&zero_principal),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::NonCanonicalState
        ))
    );

    let direct = common::direct_reservation_v8()
        .encode(common::direct_sink())
        .unwrap();
    let mut zero_payer = direct;
    zero_payer[DIRECT_RESERVATION_V2_BYTES + 9..DIRECT_RESERVATION_V2_BYTES + 41].fill(0);
    assert_eq!(
        DirectReservationAccountV8::decode(&zero_payer, common::direct_sink()),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::ZeroIdentity
        ))
    );

    let mut mismatched = common::direct_reservation_v8();
    mismatched.retirement.rent = common::deletable_rent();
    assert_eq!(
        mismatched.encode(common::direct_sink()),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::NonCanonicalState
        ))
    );

    let direct = common::direct_reservation_v8()
        .encode(common::direct_sink())
        .unwrap();
    let mut wrong_payer = direct;
    wrong_payer[DIRECT_RESERVATION_V2_BYTES + 9] ^= 1;
    assert_eq!(
        DirectReservationAccountV8::decode(&wrong_payer, common::direct_sink()),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::NonCanonicalState
        ))
    );

    let mut wrong_principal = direct;
    wrong_principal[DIRECT_RESERVATION_V2_BYTES + 41..DIRECT_RESERVATION_V2_BYTES + 49]
        .copy_from_slice(&1_001u64.to_le_bytes());
    assert_eq!(
        DirectReservationAccountV8::decode(&wrong_principal, common::direct_sink()),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::NonCanonicalState
        ))
    );
    let mut wrong_donation = direct;
    wrong_donation[DIRECT_RESERVATION_V2_BYTES + 49..DIRECT_RESERVATION_V2_BYTES + 57]
        .copy_from_slice(&4u64.to_le_bytes());
    assert_eq!(
        DirectReservationAccountV8::decode(&wrong_donation, common::direct_sink()),
        Err(RetirementAdapterErrorV2::Retirement(
            RetirementErrorV2::NonCanonicalState
        ))
    );
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
        *byte = state.to_le_bytes()[0];
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
    for len in 0..=RESERVATION_V7_BYTES + 1 {
        let _ = GeneralReservationAccountV7::decode(&hostile[..len]);
    }
    for len in 0..=DIRECT_RESERVATION_V8_BYTES + 1 {
        let _ = DirectReservationAccountV8::decode(&hostile[..len], common::direct_sink());
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
