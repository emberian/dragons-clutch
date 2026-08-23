use clutch_retirement::{
    ChildGenerationV1, EpochChildCountsV1, EpochChildKindV1, EpochRetirementTailV1,
    GeneralEpochTombstoneV1, Identity32V1, MarketEpochCursorV1, PositionRetirementTailV1,
    PositionTombstoneV1, RentSplitV2, ReservationCountTailV1, ReservationStateV1,
    CHILD_GENERATION_V1_BYTES, EPOCH_CHILD_COUNTS_V1_BYTES, EPOCH_RETIREMENT_TAIL_V1_BYTES,
    GENERAL_EPOCH_TOMBSTONE_V1_BYTES, MARKET_EPOCH_CURSOR_V1_BYTES,
    POSITION_RETIREMENT_TAIL_V1_BYTES, POSITION_TOMBSTONE_V1_BYTES, RENT_SPLIT_V2_BYTES,
    RESERVATION_COUNT_TAIL_V1_BYTES,
};

fn id(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

fn from_hex<const N: usize>(hex: &str) -> [u8; N] {
    assert_eq!(hex.len(), N * 2);
    let mut out = [0u8; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
    }
    out
}

fn rent() -> RentSplitV2 {
    RentSplitV2 {
        payer: id(0x11),
        refundable_live_principal: 0x0102_0304_0506_0708,
        permanent_tombstone_principal: 0x1112_1314_1516_1718,
        donation_floor: 0x2122_2324_2526_2728,
    }
}

const RENT_HEX: &str = concat!(
    "1111111111111111111111111111111111111111111111111111111111111111",
    "0807060504030201",
    "1817161514131211",
    "2827262524232221"
);

const COUNTS_HEX: &str = concat!(
    "01000000", "02000000", "03000000", "04000000", "05000000", "06000000", "07000000", "08000000",
    "01000000"
);

#[test]
fn retirement_tails_have_frozen_little_endian_vectors() {
    let rent_bytes = from_hex::<RENT_SPLIT_V2_BYTES>(RENT_HEX);
    assert_eq!(rent().encode().unwrap(), rent_bytes);
    assert_eq!(RentSplitV2::decode(&rent_bytes).unwrap(), rent());

    let position = PositionRetirementTailV1 {
        outstanding_reservations: 0x0a0b_0c0d,
        rent: rent(),
    };
    let position_hex = concat!(
        "0d0c0b0a",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "0807060504030201",
        "1817161514131211",
        "2827262524232221"
    );
    let position_bytes = from_hex::<POSITION_RETIREMENT_TAIL_V1_BYTES>(position_hex);
    assert_eq!(position.encode().unwrap(), position_bytes);
    assert_eq!(
        PositionRetirementTailV1::decode(&position_bytes).unwrap(),
        position
    );

    let counts = EpochChildCountsV1 {
        candidate_bundles: 1,
        candidate_index_pages: 2,
        candidate_verdicts: 3,
        candidate_escrows: 4,
        clear_work_bundles: 5,
        order_pages: 6,
        reservation_archives: 7,
        settlement_receipts: 8,
        final_pots: 1,
    };
    let count_bytes = from_hex::<EPOCH_CHILD_COUNTS_V1_BYTES>(COUNTS_HEX);
    assert_eq!(counts.encode().unwrap(), count_bytes);
    assert_eq!(EpochChildCountsV1::decode(&count_bytes).unwrap(), counts);

    let epoch = EpochRetirementTailV1 {
        epoch_generation: 0x0102_0304_0506_0708,
        children: counts,
        rent: rent(),
    };
    let epoch_hex = concat!(
        "0807060504030201",
        "01000000",
        "02000000",
        "03000000",
        "04000000",
        "05000000",
        "06000000",
        "07000000",
        "08000000",
        "01000000",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "0807060504030201",
        "1817161514131211",
        "2827262524232221"
    );
    let epoch_bytes = from_hex::<EPOCH_RETIREMENT_TAIL_V1_BYTES>(epoch_hex);
    assert_eq!(epoch.encode().unwrap(), epoch_bytes);
    assert_eq!(EpochRetirementTailV1::decode(&epoch_bytes).unwrap(), epoch);

    let cursor = MarketEpochCursorV1 {
        next_general_epoch_index: 0x0102_0304_0506_0708,
    };
    let cursor_bytes = from_hex::<MARKET_EPOCH_CURSOR_V1_BYTES>("0807060504030201");
    assert_eq!(cursor.encode(), cursor_bytes);
    assert_eq!(MarketEpochCursorV1::decode(&cursor_bytes).unwrap(), cursor);

    let reservation = ReservationCountTailV1 {
        epoch_generation: 0x0102_0304_0506_0708,
        position_counted: true,
    };
    let reservation_bytes = from_hex::<RESERVATION_COUNT_TAIL_V1_BYTES>("080706050403020101");
    assert_eq!(reservation.encode().unwrap(), reservation_bytes);
    assert_eq!(
        ReservationCountTailV1::decode(&reservation_bytes).unwrap(),
        reservation
    );

    let generation = ChildGenerationV1 {
        epoch_generation: 0x0102_0304_0506_0708,
    };
    let generation_bytes = from_hex::<CHILD_GENERATION_V1_BYTES>("0807060504030201");
    assert_eq!(generation.encode().unwrap(), generation_bytes);
    assert_eq!(
        ChildGenerationV1::decode(&generation_bytes).unwrap(),
        generation
    );
}

#[test]
fn permanent_tombstones_have_frozen_tagged_vectors() {
    let position = PositionTombstoneV1 {
        market: id(0xaa),
        owner: id(0xbb),
        generation: 0x0102_0304_0506_0708,
        stored_bump: 0xfe,
    };
    let position_hex = concat!(
        "7501",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "0807060504030201",
        "01fe"
    );
    let position_bytes = from_hex::<POSITION_TOMBSTONE_V1_BYTES>(position_hex);
    assert_eq!(position.encode().unwrap(), position_bytes);
    assert_eq!(
        PositionTombstoneV1::decode(&position_bytes).unwrap(),
        position
    );
    let founding = PositionTombstoneV1 {
        generation: 0,
        ..position
    };
    let founding_bytes = founding.encode().unwrap();
    assert_eq!(&founding_bytes[66..74], &[0; 8]);
    assert_eq!(PositionTombstoneV1::decode(&founding_bytes), Ok(founding));

    let epoch = GeneralEpochTombstoneV1 {
        epoch: id(0xcc),
        market: id(0xaa),
        epoch_index: 0x1112_1314_1516_1718,
        epoch_generation: 0x0102_0304_0506_0708,
        stored_bump: 0xfd,
    };
    let epoch_hex = concat!(
        "7601",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "1817161514131211",
        "0807060504030201",
        "01fd"
    );
    let epoch_bytes = from_hex::<GENERAL_EPOCH_TOMBSTONE_V1_BYTES>(epoch_hex);
    assert_eq!(epoch.encode().unwrap(), epoch_bytes);
    assert_eq!(
        GeneralEpochTombstoneV1::decode(&epoch_bytes).unwrap(),
        epoch
    );
}

#[test]
fn every_persisted_discriminant_is_frozen() {
    let kinds = [
        EpochChildKindV1::CandidateBundle,
        EpochChildKindV1::CandidateIndexPage,
        EpochChildKindV1::CandidateVerdict,
        EpochChildKindV1::CandidateEscrow,
        EpochChildKindV1::ClearWorkBundle,
        EpochChildKindV1::OrderPage,
        EpochChildKindV1::ReservationArchive,
        EpochChildKindV1::SettlementReceipt,
        EpochChildKindV1::FinalPot,
    ];
    for (byte, kind) in kinds.into_iter().enumerate() {
        assert_eq!(EpochChildKindV1::try_from(byte as u8), Ok(kind));
    }

    for (byte, state) in [
        ReservationStateV1::Active,
        ReservationStateV1::Released,
        ReservationStateV1::Entitled,
        ReservationStateV1::Consumed,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(ReservationStateV1::try_from(byte as u8), Ok(state));
    }
}
