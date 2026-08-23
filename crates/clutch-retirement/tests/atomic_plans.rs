use clutch_retirement::{
    close_general_reservation_archive, close_position, open_general_epoch, plan_epoch_retirement,
    plan_position_retirement, register_direct_reservation, register_general_reservation,
    terminate_reservation, ChildSlotV1, CountedReservationV1, EpochChildCountsV1,
    EpochLifecycleStateV5, EpochRetirementTailV1, GeneralEpochPhaseV1, Identity32V1, LiveEpochV5,
    LivePositionV2, MarketEpochCursorV1, PositionEconomicStateV1, PositionLifecycleStateV2,
    PositionRetirementTailV1, RentSplitV2, ReservationStateV1, RetirementErrorV1,
};

fn id(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

fn rent() -> RentSplitV2 {
    RentSplitV2 {
        payer: id(3),
        refundable_live_principal: 11,
        permanent_tombstone_principal: 7,
        donation_floor: 5,
    }
}

fn position() -> LivePositionV2 {
    LivePositionV2 {
        market: id(1),
        owner: id(2),
        generation: 1,
        stored_bump: 250,
        retirement: PositionRetirementTailV1 {
            outstanding_reservations: 0,
            rent: rent(),
        },
    }
}

fn epoch() -> LiveEpochV5 {
    LiveEpochV5 {
        market: id(1),
        epoch: id(4),
        epoch_index: 0,
        phase: GeneralEpochPhaseV1::Open,
        stored_bump: 249,
        retirement: EpochRetirementTailV1 {
            epoch_generation: 1,
            children: EpochChildCountsV1::default(),
            rent: rent(),
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegistrationBank {
    position: LivePositionV2,
    epoch: LiveEpochV5,
    reservation: Option<CountedReservationV1>,
    archive: ChildSlotV1,
    position_egg_atoms: u64,
    reservation_egg_atoms: u64,
}

fn staged_general_registration(
    bank: &mut RegistrationBank,
    fail_after_write: Option<u8>,
) -> Result<(), ()> {
    let (position_after, epoch_after, reservation, archive) =
        register_general_reservation(bank.position, bank.epoch).map_err(|_| ())?;
    let mut staged = *bank;
    if staged.position_egg_atoms == 0 || staged.reservation_egg_atoms != 0 {
        return Err(());
    }
    staged.reservation_egg_atoms = staged.position_egg_atoms;
    staged.position_egg_atoms = 0;
    if fail_after_write == Some(1) {
        return Err(());
    }
    staged.position = position_after;
    if fail_after_write == Some(2) {
        return Err(());
    }
    staged.epoch = epoch_after;
    if fail_after_write == Some(3) {
        return Err(());
    }
    staged.reservation = Some(reservation);
    if fail_after_write == Some(4) {
        return Err(());
    }
    staged.archive = archive;
    if fail_after_write == Some(5) {
        return Err(());
    }
    *bank = staged;
    Ok(())
}

#[test]
fn host_staging_model_preserves_prestate_at_every_late_registration_failure() {
    let original = RegistrationBank {
        position: position(),
        epoch: epoch(),
        reservation: None,
        archive: ChildSlotV1::Absent,
        position_egg_atoms: 17,
        reservation_egg_atoms: 0,
    };
    for fail_after in 1..=5 {
        let mut bank = original;
        assert_eq!(
            staged_general_registration(&mut bank, Some(fail_after)),
            Err(())
        );
        assert_eq!(bank, original);
    }

    let mut committed = original;
    staged_general_registration(&mut committed, None).unwrap();
    assert_eq!(committed.position.retirement.outstanding_reservations, 1);
    assert_eq!(committed.epoch.retirement.children.reservation_archives, 1);
    assert!(committed.reservation.is_some());
    assert_ne!(committed.archive, ChildSlotV1::Absent);
    assert_eq!(committed.position_egg_atoms, 0);
    assert_eq!(committed.reservation_egg_atoms, 17);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CloseBank {
    state: PositionLifecycleStateV2,
    payer_balance: u64,
    neutral_balance: u64,
    account_balance: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EpochCloseBank {
    state: EpochLifecycleStateV5,
    payer_balance: u64,
    neutral_balance: u64,
    account_balance: u64,
}

fn staged_position_close(bank: &mut CloseBank, fail_after_write: Option<u8>) -> Result<(), ()> {
    let plan = plan_position_retirement(
        bank.state,
        PositionEconomicStateV1::ZERO,
        bank.account_balance,
        id(250),
        bank.payer_balance,
        bank.neutral_balance,
    )
    .map_err(|_| ())?;
    let mut staged = *bank;
    staged.state = plan.post_state;
    if fail_after_write == Some(1) {
        return Err(());
    }
    staged.payer_balance = plan.payer_balance_after;
    if fail_after_write == Some(2) {
        return Err(());
    }
    staged.neutral_balance = plan.neutral_balance_after;
    if fail_after_write == Some(3) {
        return Err(());
    }
    staged.account_balance = plan.tombstone_balance_after;
    if fail_after_write == Some(4) {
        return Err(());
    }
    *bank = staged;
    Ok(())
}

fn staged_epoch_close(bank: &mut EpochCloseBank, fail_after_write: Option<u8>) -> Result<(), ()> {
    let plan = plan_epoch_retirement(
        bank.state,
        bank.account_balance,
        id(250),
        bank.payer_balance,
        bank.neutral_balance,
    )
    .map_err(|_| ())?;
    let mut staged = *bank;
    staged.state = plan.post_state;
    if fail_after_write == Some(1) {
        return Err(());
    }
    staged.payer_balance = plan.payer_balance_after;
    if fail_after_write == Some(2) {
        return Err(());
    }
    staged.neutral_balance = plan.neutral_balance_after;
    if fail_after_write == Some(3) {
        return Err(());
    }
    staged.account_balance = plan.tombstone_balance_after;
    if fail_after_write == Some(4) {
        return Err(());
    }
    *bank = staged;
    Ok(())
}

#[test]
fn retirement_plan_precomputes_surplus_and_recipient_overflow_before_writes() {
    let state = PositionLifecycleStateV2::Live(position());
    let plan =
        plan_position_retirement(state, PositionEconomicStateV1::ZERO, 29, id(250), 100, 200)
            .unwrap();
    assert_eq!(plan.payer, rent().payer);
    assert_eq!(plan.payer_balance_after, 111);
    assert_eq!(plan.neutral_sink, id(250));
    assert_eq!(plan.neutral_balance_after, 211);
    assert_eq!(plan.tombstone_balance_after, 7);
    assert_eq!((111 - 100) + (211 - 200) + 7, 29);
    assert!(11 >= rent().donation_floor);

    assert_eq!(
        plan_position_retirement(
            state,
            PositionEconomicStateV1::ZERO,
            29,
            id(250),
            u64::MAX - 10,
            200,
        ),
        Err(RetirementErrorV1::ArithmeticOverflow)
    );
    assert_eq!(
        plan_position_retirement(
            state,
            PositionEconomicStateV1::ZERO,
            29,
            id(250),
            100,
            u64::MAX - 10,
        ),
        Err(RetirementErrorV1::ArithmeticOverflow)
    );

    let original = CloseBank {
        state,
        payer_balance: 100,
        neutral_balance: 200,
        account_balance: 29,
    };
    for fail_after in 1..=4 {
        let mut bank = original;
        assert_eq!(staged_position_close(&mut bank, Some(fail_after)), Err(()));
        assert_eq!(bank, original);
    }
    let mut committed = original;
    staged_position_close(&mut committed, None).unwrap();
    assert_eq!(committed.payer_balance, 111);
    assert_eq!(committed.neutral_balance, 211);
    assert_eq!(committed.account_balance, 7);
}

#[test]
fn general_and_direct_all_in_paths_share_one_exact_once_position_count() {
    let (position, epoch, general, archive) =
        register_general_reservation(position(), epoch()).unwrap();
    let (position, direct) = register_direct_reservation(position, 91).unwrap();
    assert_eq!(position.retirement.outstanding_reservations, 2);
    assert_eq!(epoch.retirement.children.reservation_archives, 1);
    assert_eq!(
        close_position(
            PositionLifecycleStateV2::Live(position),
            PositionEconomicStateV1::ZERO,
            23,
            id(250),
        ),
        Err(RetirementErrorV1::ReservationOutstanding)
    );

    let (position, general) =
        terminate_reservation(position, general, ReservationStateV1::Released).unwrap();
    assert_eq!(position.retirement.outstanding_reservations, 1);
    assert_eq!(
        terminate_reservation(position, general, ReservationStateV1::Released),
        Err(RetirementErrorV1::AlreadyTerminal)
    );
    assert_eq!(
        close_position(
            PositionLifecycleStateV2::Live(position),
            PositionEconomicStateV1::ZERO,
            23,
            id(250),
        ),
        Err(RetirementErrorV1::ReservationOutstanding)
    );

    let (position, direct) =
        terminate_reservation(position, direct, ReservationStateV1::Consumed).unwrap();
    assert_eq!(position.retirement.outstanding_reservations, 0);
    assert_eq!(
        terminate_reservation(position, direct, ReservationStateV1::Consumed),
        Err(RetirementErrorV1::AlreadyTerminal)
    );
    assert!(close_position(
        PositionLifecycleStateV2::Live(position),
        PositionEconomicStateV1::ZERO,
        23,
        id(250),
    )
    .is_ok());

    let mut epoch = epoch;
    epoch.phase = GeneralEpochPhaseV1::Lapsed;
    let (epoch, absent) = close_general_reservation_archive(epoch, archive, general).unwrap();
    assert_eq!(absent, ChildSlotV1::Absent);
    assert_eq!(epoch.retirement.children.reservation_archives, 0);
}

#[test]
fn serialized_cursor_race_admits_one_plan_and_refuses_the_stale_replay() {
    let cursor = MarketEpochCursorV1 {
        next_general_epoch_index: 7,
    };
    let (winner_cursor, winner) = open_general_epoch(cursor, 7, id(1), id(4), 9, rent()).unwrap();
    let (stale_snapshot_cursor, stale_snapshot) =
        open_general_epoch(cursor, 7, id(1), id(4), 9, rent()).unwrap();
    assert_eq!(winner_cursor, stale_snapshot_cursor);
    assert_eq!(winner, stale_snapshot);
    assert_eq!(
        open_general_epoch(winner_cursor, 7, id(1), id(4), 9, rent()),
        Err(RetirementErrorV1::NonmonotoneEpoch),
        "the writable Market lock must serialize the loser onto the new cursor"
    );

    let mut terminal = winner;
    terminal.phase = GeneralEpochPhaseV1::Lapsed;
    let plan = plan_epoch_retirement(EpochLifecycleStateV5::Live(terminal), 29, id(250), 100, 200)
        .unwrap();
    assert_eq!(plan.payer_balance_after, 111);
    assert_eq!(plan.neutral_balance_after, 211);
    assert_eq!(plan.tombstone_balance_after, 7);

    let original = EpochCloseBank {
        state: EpochLifecycleStateV5::Live(terminal),
        payer_balance: 100,
        neutral_balance: 200,
        account_balance: 29,
    };
    for fail_after in 1..=4 {
        let mut bank = original;
        assert_eq!(staged_epoch_close(&mut bank, Some(fail_after)), Err(()));
        assert_eq!(bank, original);
    }
    let mut committed = original;
    staged_epoch_close(&mut committed, None).unwrap();
    assert_eq!(committed.payer_balance, 111);
    assert_eq!(committed.neutral_balance, 211);
    assert_eq!(committed.account_balance, 7);
}
