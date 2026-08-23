use clutch_retirement::{
    ChildGenerationV1, EpochChildCountsV1, EpochChildKindV1, EpochRetirementTailV1,
    GeneralEpochTombstoneV1, Identity32V1, MarketEpochCursorV1, PositionRetirementTailV1,
    PositionTombstoneV1, RentSplitV2, ReservationCountTailV1, ReservationStateV1,
    RetirementErrorV1,
};

fn id(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

fn rent() -> RentSplitV2 {
    RentSplitV2 {
        payer: id(1),
        refundable_live_principal: 11,
        permanent_tombstone_principal: 7,
        donation_floor: 3,
    }
}

macro_rules! exact_lengths {
    ($bytes:expr, $decode:expr) => {{
        let bytes = $bytes;
        for len in 0..bytes.len() {
            assert_eq!($decode(&bytes[..len]), Err(RetirementErrorV1::Truncated));
        }
        for extra in 1..=4usize {
            let mut long = bytes.to_vec();
            long.resize(bytes.len() + extra, 0xa5);
            assert_eq!($decode(&long), Err(RetirementErrorV1::TrailingBytes));
        }
    }};
}

#[test]
fn every_decoder_refuses_truncation_and_trailing_bytes() {
    let rent_bytes = rent().encode().unwrap();
    exact_lengths!(rent_bytes, RentSplitV2::decode);

    let position_bytes = PositionRetirementTailV1 {
        outstanding_reservations: 1,
        rent: rent(),
    }
    .encode()
    .unwrap();
    exact_lengths!(position_bytes, PositionRetirementTailV1::decode);

    let counts_bytes = EpochChildCountsV1::default().encode().unwrap();
    exact_lengths!(counts_bytes, EpochChildCountsV1::decode);

    let epoch_bytes = EpochRetirementTailV1 {
        epoch_generation: 1,
        children: EpochChildCountsV1::default(),
        rent: rent(),
    }
    .encode()
    .unwrap();
    exact_lengths!(epoch_bytes, EpochRetirementTailV1::decode);

    let cursor_bytes = MarketEpochCursorV1 {
        next_general_epoch_index: 0,
    }
    .encode();
    exact_lengths!(cursor_bytes, MarketEpochCursorV1::decode);

    let reservation_bytes = ReservationCountTailV1 {
        epoch_generation: 1,
        position_counted: true,
    }
    .encode()
    .unwrap();
    exact_lengths!(reservation_bytes, ReservationCountTailV1::decode);

    let generation_bytes = ChildGenerationV1 {
        epoch_generation: 1,
    }
    .encode()
    .unwrap();
    exact_lengths!(generation_bytes, ChildGenerationV1::decode);

    let position_tombstone = PositionTombstoneV1 {
        market: id(2),
        owner: id(3),
        generation: 1,
        stored_bump: 4,
    }
    .encode()
    .unwrap();
    exact_lengths!(position_tombstone, PositionTombstoneV1::decode);

    let epoch_tombstone = GeneralEpochTombstoneV1 {
        epoch: id(5),
        market: id(2),
        epoch_index: 0,
        epoch_generation: 1,
        stored_bump: 6,
    }
    .encode()
    .unwrap();
    exact_lengths!(epoch_tombstone, GeneralEpochTombstoneV1::decode);
}

#[test]
fn hostile_fields_fail_closed() {
    let mut rent_bytes = rent().encode().unwrap();
    rent_bytes[..32].fill(0);
    assert_eq!(
        RentSplitV2::decode(&rent_bytes),
        Err(RetirementErrorV1::ZeroIdentity)
    );
    let mut rent_bytes = rent().encode().unwrap();
    rent_bytes[32..40].fill(0);
    assert_eq!(
        RentSplitV2::decode(&rent_bytes),
        Err(RetirementErrorV1::NonCanonicalState)
    );
    let mut rent_bytes = rent().encode().unwrap();
    rent_bytes[40..48].fill(0);
    assert_eq!(
        RentSplitV2::decode(&rent_bytes),
        Err(RetirementErrorV1::NonCanonicalState)
    );
    let overflowing = RentSplitV2 {
        payer: id(1),
        refundable_live_principal: u64::MAX,
        permanent_tombstone_principal: 1,
        donation_floor: 0,
    };
    assert_eq!(
        overflowing.encode(),
        Err(RetirementErrorV1::ArithmeticOverflow)
    );

    let mut counts = EpochChildCountsV1::default().encode().unwrap();
    counts[32..36].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        EpochChildCountsV1::decode(&counts),
        Err(RetirementErrorV1::NonCanonicalState)
    );

    let mut reservation = ReservationCountTailV1 {
        epoch_generation: 1,
        position_counted: true,
    }
    .encode()
    .unwrap();
    reservation[8] = 2;
    assert_eq!(
        ReservationCountTailV1::decode(&reservation),
        Err(RetirementErrorV1::InvalidEnum)
    );
    reservation[8] = 1;
    reservation[..8].fill(0);
    assert_eq!(
        ReservationCountTailV1::decode(&reservation),
        Err(RetirementErrorV1::WrongGeneration)
    );

    let mut position = PositionTombstoneV1 {
        market: id(2),
        owner: id(3),
        generation: 1,
        stored_bump: 4,
    }
    .encode()
    .unwrap();
    for (offset, expected) in [
        (0usize, RetirementErrorV1::WrongTag),
        (1, RetirementErrorV1::WrongVersion),
        (74, RetirementErrorV1::InvalidEnum),
    ] {
        let mut bad = position;
        bad[offset] = bad[offset].wrapping_add(1);
        assert_eq!(PositionTombstoneV1::decode(&bad), Err(expected));
    }
    position[2..34].fill(0);
    assert_eq!(
        PositionTombstoneV1::decode(&position),
        Err(RetirementErrorV1::ZeroIdentity)
    );

    let epoch = GeneralEpochTombstoneV1 {
        epoch: id(5),
        market: id(2),
        epoch_index: 0,
        epoch_generation: 1,
        stored_bump: 6,
    }
    .encode()
    .unwrap();
    for (offset, expected) in [
        (0usize, RetirementErrorV1::WrongTag),
        (1, RetirementErrorV1::WrongVersion),
        (82, RetirementErrorV1::InvalidEnum),
    ] {
        let mut bad = epoch;
        bad[offset] = bad[offset].wrapping_add(1);
        assert_eq!(GeneralEpochTombstoneV1::decode(&bad), Err(expected));
    }
}

#[test]
fn every_unknown_discriminant_refuses() {
    for byte in 0u8..=u8::MAX {
        assert_eq!(
            EpochChildKindV1::try_from(byte).is_ok(),
            byte <= EpochChildKindV1::FinalPot as u8
        );
        assert_eq!(
            ReservationStateV1::try_from(byte).is_ok(),
            byte <= ReservationStateV1::Consumed as u8
        );
    }
}

#[test]
fn arbitrary_bytes_and_lengths_never_panic_or_accept_noncanonical_state() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for len in 0usize..=140 {
        for _ in 0..64 {
            let mut bytes = vec![0u8; len];
            for byte in &mut bytes {
                seed ^= seed << 7;
                seed ^= seed >> 9;
                seed ^= seed << 8;
                *byte = seed as u8;
            }
            if let Ok(value) = RentSplitV2::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = PositionRetirementTailV1::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = EpochChildCountsV1::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = EpochRetirementTailV1::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = MarketEpochCursorV1::decode(&bytes) {
                assert_eq!(value.encode().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = ReservationCountTailV1::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = ChildGenerationV1::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = PositionTombstoneV1::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
            if let Ok(value) = GeneralEpochTombstoneV1::decode(&bytes) {
                assert_eq!(value.encode().unwrap().as_slice(), bytes.as_slice());
            }
        }
    }
}

#[test]
fn tombstone_families_cannot_cross_decode() {
    let position = PositionTombstoneV1 {
        market: id(2),
        owner: id(3),
        generation: 1,
        stored_bump: 4,
    }
    .encode()
    .unwrap();
    let epoch = GeneralEpochTombstoneV1 {
        epoch: id(5),
        market: id(2),
        epoch_index: 0,
        epoch_generation: 1,
        stored_bump: 6,
    }
    .encode()
    .unwrap();
    assert_eq!(
        GeneralEpochTombstoneV1::decode(&position),
        Err(RetirementErrorV1::Truncated)
    );
    assert_eq!(
        PositionTombstoneV1::decode(&epoch),
        Err(RetirementErrorV1::TrailingBytes)
    );
}
