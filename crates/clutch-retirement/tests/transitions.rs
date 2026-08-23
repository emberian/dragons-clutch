use clutch_retirement::{
    close_epoch, close_epoch_child, close_general_reservation_archive, close_position,
    close_registered_candidate, create_epoch_child, create_registered_candidate_after_validation,
    entitle_reservation, open_general_epoch, register_direct_reservation,
    register_general_reservation, reopen_position, terminate_reservation,
    update_registered_candidate_status_after_validation, AuthenticatedEpochChildV1,
    CandidateStatusWitnessV1, ChildSlotV1, EpochChildCountsV1, EpochChildKindV1,
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
    open_general_epoch(
        MarketEpochCursorV1 {
            next_general_epoch_index: 0,
        },
        0,
        id(1),
        id(4),
        249,
        rent(),
    )
    .unwrap()
    .1
}

fn terminal(mut epoch: LiveEpochV5) -> LiveEpochV5 {
    epoch.phase = GeneralEpochPhaseV1::Lapsed;
    epoch
}

#[test]
fn all_in_local_zero_still_refuses_until_exact_once_terminal_debit() {
    let original_position = position();
    let original_epoch = epoch();
    let (counted_position, counted_epoch, reservation, archive) =
        register_general_reservation(original_position, original_epoch).unwrap();
    assert_eq!(counted_position.retirement.outstanding_reservations, 1);
    assert_eq!(counted_epoch.retirement.children.reservation_archives, 1);
    assert!(PositionEconomicStateV1::ZERO.is_zero());
    assert_eq!(
        close_position(
            PositionLifecycleStateV2::Live(counted_position),
            PositionEconomicStateV1::ZERO,
            23,
            id(250)
        ),
        Err(RetirementErrorV1::ReservationOutstanding)
    );

    let entitled = entitle_reservation(reservation).unwrap();
    assert_eq!(counted_position.retirement.outstanding_reservations, 1);
    assert!(entitled.count.position_counted);
    let (zero_position, consumed) =
        terminate_reservation(counted_position, entitled, ReservationStateV1::Consumed).unwrap();
    assert_eq!(zero_position.retirement.outstanding_reservations, 0);
    assert!(!consumed.count.position_counted);
    assert_eq!(
        terminate_reservation(zero_position, consumed, ReservationStateV1::Consumed),
        Err(RetirementErrorV1::AlreadyTerminal)
    );
    assert_eq!(zero_position.retirement.outstanding_reservations, 0);

    let (closed, disposition) = close_position(
        PositionLifecycleStateV2::Live(zero_position),
        PositionEconomicStateV1::ZERO,
        29,
        id(250),
    )
    .unwrap();
    assert_eq!(disposition.payer_refund_lamports, 11);
    assert_eq!(disposition.tombstone_lamports, 7);
    assert_eq!(disposition.neutral_lamports, 11);
    assert_eq!(11 + 7 + 11, 29);
    assert_eq!(
        close_position(closed, PositionEconomicStateV1::ZERO, 29, id(250)),
        Err(RetirementErrorV1::AlreadyTerminal)
    );

    let terminal_epoch = terminal(counted_epoch);
    assert_eq!(
        close_epoch_child(terminal_epoch, archive),
        Err(RetirementErrorV1::WrongChildKind),
        "generic close cannot bypass terminal reservation validation"
    );
    let (terminal_epoch, absent) =
        close_general_reservation_archive(terminal_epoch, archive, consumed).unwrap();
    assert_eq!(absent, ChildSlotV1::Absent);
    assert_eq!(terminal_epoch.retirement.children.reservation_archives, 0);
    assert_eq!(
        close_general_reservation_archive(terminal_epoch, absent, consumed),
        Err(RetirementErrorV1::ChildAbsent)
    );
}

#[test]
fn released_and_direct_reservations_share_the_position_counter() {
    let (position, direct) = register_direct_reservation(position(), 77).unwrap();
    assert_eq!(position.retirement.outstanding_reservations, 1);
    let (position, released) =
        terminate_reservation(position, direct, ReservationStateV1::Released).unwrap();
    assert_eq!(position.retirement.outstanding_reservations, 0);
    assert_eq!(released.state, ReservationStateV1::Released);
    assert_eq!(
        terminate_reservation(position, released, ReservationStateV1::Released),
        Err(RetirementErrorV1::AlreadyTerminal)
    );
}

#[test]
fn cursor_is_exact_monotone_and_retirement_never_reopens_identity() {
    let cursor = MarketEpochCursorV1 {
        next_general_epoch_index: 41,
    };
    for requested in [0, 40, 42, u64::MAX] {
        assert_eq!(
            open_general_epoch(cursor, requested, id(1), id(4), 9, rent()),
            Err(RetirementErrorV1::NonmonotoneEpoch)
        );
    }
    let (next, mut epoch) = open_general_epoch(cursor, 41, id(1), id(4), 9, rent()).unwrap();
    assert_eq!(next.next_general_epoch_index, 42);
    assert_eq!(epoch.retirement.epoch_generation, 42);
    epoch.phase = GeneralEpochPhaseV1::Cleared;
    let (closed, disposition) =
        close_epoch(EpochLifecycleStateV5::Live(epoch), 31, id(250)).unwrap();
    assert_eq!(disposition.payer_refund_lamports, 11);
    assert_eq!(disposition.tombstone_lamports, 7);
    assert_eq!(disposition.neutral_lamports, 13);
    assert_eq!(
        close_epoch(closed, 31, id(250)),
        Err(RetirementErrorV1::AlreadyTerminal)
    );
    assert_eq!(next.next_general_epoch_index, 42);
    assert_eq!(
        open_general_epoch(next, 41, id(1), id(4), 9, rent()),
        Err(RetirementErrorV1::NonmonotoneEpoch),
        "a closed Epoch index cannot be replayed behind the Market cursor"
    );

    let exhausted = MarketEpochCursorV1 {
        next_general_epoch_index: u64::MAX,
    };
    assert_eq!(
        open_general_epoch(exhausted, u64::MAX, id(1), id(4), 9, rent()),
        Err(RetirementErrorV1::EpochIndexExhausted)
    );
}

#[test]
fn every_generic_child_class_blocks_root_and_closes_once() {
    let kinds = [
        EpochChildKindV1::CandidateIndexPage,
        EpochChildKindV1::CandidateVerdict,
        EpochChildKindV1::CandidateEscrow,
        EpochChildKindV1::ClearWorkBundle,
        EpochChildKindV1::OrderPage,
        EpochChildKindV1::SettlementReceipt,
        EpochChildKindV1::FinalPot,
    ];
    for kind in kinds {
        let initial = epoch();
        let (live, child) = create_epoch_child(initial, ChildSlotV1::Absent, kind).unwrap();
        assert_eq!(live.retirement.children.get(kind), 1);
        let terminal = terminal(live);
        assert_eq!(
            close_epoch(EpochLifecycleStateV5::Live(terminal), 23, id(250)),
            Err(RetirementErrorV1::ChildOutstanding)
        );
        let (empty, absent) = close_epoch_child(terminal, child).unwrap();
        assert_eq!(empty.retirement.children.get(kind), 0);
        assert_eq!(
            close_epoch_child(empty, absent),
            Err(RetirementErrorV1::ChildAbsent)
        );
        assert!(close_epoch(EpochLifecycleStateV5::Live(empty), 23, id(250)).is_ok());
    }
}

#[test]
fn admitted_candidate_statuses_are_exhaustively_counted_without_semantic_duplication() {
    // Frozen adapter fixtures only: retirement treats each triple as opaque.
    // Current CandidateRecord V3: tag 13/version 3, statuses 0..=4.
    // Candidate lifecycle V2: tag 3/version 2, statuses 0..=4.
    let schemas = [(13u8, 3u8), (3u8, 2u8)];
    for (tag, version) in schemas {
        let initial = CandidateStatusWitnessV1::from_validated_account(tag, version, 0);
        for status_byte in 0u8..=4 {
            let status =
                CandidateStatusWitnessV1::from_validated_account(tag, version, status_byte);
            let (live, candidate) =
                create_registered_candidate_after_validation(epoch(), ChildSlotV1::Absent, initial)
                    .unwrap();
            let candidate =
                update_registered_candidate_status_after_validation(candidate, status).unwrap();
            assert_eq!(live.retirement.children.candidate_bundles, 1);
            let terminal = terminal(live);
            assert_eq!(
                close_epoch(EpochLifecycleStateV5::Live(terminal), 23, id(250)),
                Err(RetirementErrorV1::ChildOutstanding)
            );
            assert_eq!(
                close_registered_candidate(terminal, candidate, true),
                Err(RetirementErrorV1::ClearWorkOutstanding)
            );
            let (empty, absent) = close_registered_candidate(terminal, candidate, false).unwrap();
            assert_eq!(empty.retirement.children.candidate_bundles, 0);
            assert_eq!(
                close_registered_candidate(empty, absent, false),
                Err(RetirementErrorV1::ChildAbsent)
            );
        }
    }
}

#[test]
fn candidate_and_clear_work_have_independent_exact_counts_and_ordering() {
    let initial = CandidateStatusWitnessV1::from_validated_account(3, 2, 0);
    let (epoch, candidate) =
        create_registered_candidate_after_validation(epoch(), ChildSlotV1::Absent, initial)
            .unwrap();
    let candidate = update_registered_candidate_status_after_validation(
        candidate,
        CandidateStatusWitnessV1::from_validated_account(3, 2, 4),
    )
    .unwrap();
    let (epoch, work) = create_epoch_child(
        epoch,
        ChildSlotV1::Absent,
        EpochChildKindV1::ClearWorkBundle,
    )
    .unwrap();
    let terminal = terminal(epoch);
    assert_eq!(
        close_registered_candidate(terminal, candidate, true),
        Err(RetirementErrorV1::ClearWorkOutstanding)
    );
    let (terminal, _) = close_epoch_child(terminal, work).unwrap();
    let (terminal, _) = close_registered_candidate(terminal, candidate, false).unwrap();
    assert!(terminal.retirement.children.is_zero());
}

#[test]
fn candidate_status_updates_cannot_switch_the_registered_schema() {
    let initial = CandidateStatusWitnessV1::from_validated_account(3, 2, 0);
    let (live, candidate) =
        create_registered_candidate_after_validation(epoch(), ChildSlotV1::Absent, initial)
            .unwrap();
    assert_eq!(
        update_registered_candidate_status_after_validation(
            candidate,
            CandidateStatusWitnessV1::from_validated_account(4, 2, 1),
        ),
        Err(RetirementErrorV1::WrongTag)
    );
    assert_eq!(
        update_registered_candidate_status_after_validation(
            candidate,
            CandidateStatusWitnessV1::from_validated_account(3, 3, 1),
        ),
        Err(RetirementErrorV1::WrongVersion)
    );
    assert_eq!(live.retirement.children.candidate_bundles, 1);
    let ChildSlotV1::Present(unchanged) = candidate else {
        panic!("registered candidate disappeared")
    };
    assert_eq!(unchanged.candidate_status, Some(initial));
}

#[test]
fn wrong_generation_overflow_underflow_and_singleton_pot_refuse_without_poststate() {
    let initial = epoch();
    assert_eq!(
        create_epoch_child(
            initial,
            ChildSlotV1::Absent,
            EpochChildKindV1::ReservationArchive
        ),
        Err(RetirementErrorV1::WrongChildKind),
        "generic create cannot bypass Position registration"
    );
    let wrong = ChildSlotV1::Present(AuthenticatedEpochChildV1 {
        epoch_generation: initial.retirement.epoch_generation + 1,
        kind: EpochChildKindV1::OrderPage,
        candidate_status: None,
    });
    assert_eq!(
        close_epoch_child(terminal(initial), wrong),
        Err(RetirementErrorV1::WrongGeneration)
    );
    assert_eq!(initial.retirement.children, EpochChildCountsV1::default());

    let forged = ChildSlotV1::Present(AuthenticatedEpochChildV1 {
        epoch_generation: initial.retirement.epoch_generation,
        kind: EpochChildKindV1::OrderPage,
        candidate_status: None,
    });
    assert_eq!(
        close_epoch_child(terminal(initial), forged),
        Err(RetirementErrorV1::CounterUnderflow)
    );

    let mut overflow = initial;
    overflow.retirement.children.order_pages = u32::MAX;
    assert_eq!(
        create_epoch_child(overflow, ChildSlotV1::Absent, EpochChildKindV1::OrderPage),
        Err(RetirementErrorV1::ArithmeticOverflow)
    );

    let (one_pot, _) =
        create_epoch_child(initial, ChildSlotV1::Absent, EpochChildKindV1::FinalPot).unwrap();
    assert_eq!(
        create_epoch_child(one_pot, ChildSlotV1::Absent, EpochChildKindV1::FinalPot),
        Err(RetirementErrorV1::NonCanonicalState)
    );
}

#[test]
fn multi_parent_registration_precomputes_both_counters_before_commit() {
    let initial_position = position();
    let mut full_epoch = epoch();
    full_epoch.retirement.children.reservation_archives = u32::MAX;
    assert_eq!(
        register_general_reservation(initial_position, full_epoch),
        Err(RetirementErrorV1::ArithmeticOverflow)
    );
    assert_eq!(
        initial_position.retirement.outstanding_reservations, 0,
        "Position cannot expose a half increment"
    );
    assert_eq!(
        full_epoch.retirement.children.reservation_archives,
        u32::MAX,
        "Epoch cannot expose a half increment"
    );

    let mut full_position = position();
    full_position.retirement.outstanding_reservations = u32::MAX;
    let initial_epoch = epoch();
    assert_eq!(
        register_general_reservation(full_position, initial_epoch),
        Err(RetirementErrorV1::ArithmeticOverflow)
    );
    assert_eq!(
        initial_epoch.retirement.children.reservation_archives, 0,
        "Epoch cannot increment when Position overflow refuses"
    );
}

#[test]
fn malformed_or_stale_reservation_markers_never_debit_position() {
    let initial = position();
    let (_, reservation) = register_direct_reservation(initial, 9).unwrap();

    let mut uncounted_active = reservation;
    uncounted_active.count.position_counted = false;
    assert_eq!(
        terminate_reservation(initial, uncounted_active, ReservationStateV1::Released),
        Err(RetirementErrorV1::NonCanonicalState)
    );

    let mut stale = reservation;
    stale.position_generation = initial.generation + 1;
    assert_eq!(
        terminate_reservation(initial, stale, ReservationStateV1::Released),
        Err(RetirementErrorV1::WrongGeneration)
    );

    assert_eq!(
        terminate_reservation(initial, reservation, ReservationStateV1::Released),
        Err(RetirementErrorV1::CounterUnderflow)
    );
    assert_eq!(initial.retirement.outstanding_reservations, 0);
}

#[test]
fn close_refusals_preserve_inputs_and_exact_rent_compartments() {
    let live = position();
    let state = PositionLifecycleStateV2::Live(live);
    for balance in 0..23u64 {
        assert_eq!(
            close_position(state, PositionEconomicStateV1::ZERO, balance, id(250)),
            Err(RetirementErrorV1::AccountBalanceShortfall)
        );
        assert_eq!(state, PositionLifecycleStateV2::Live(live));
    }
    assert_eq!(
        close_position(state, PositionEconomicStateV1::ZERO, 23, rent().payer),
        Err(RetirementErrorV1::PayerIsNeutralSink)
    );
    assert_eq!(state, PositionLifecycleStateV2::Live(live));

    let mut nonzero = PositionEconomicStateV1::ZERO;
    nonzero.internal_atoms[15] = 1;
    assert_eq!(
        close_position(state, nonzero, 23, id(250)),
        Err(RetirementErrorV1::EconomicBalanceOutstanding)
    );
}

#[test]
fn reopen_is_monotone_and_generation_overflow_is_permanent_stop() {
    let mut founding = position();
    founding.generation = 0;
    let (founding, reservation) = register_direct_reservation(founding, 9).unwrap();
    assert_eq!(reservation.position_generation, 0);
    let (founding, _) =
        terminate_reservation(founding, reservation, ReservationStateV1::Consumed).unwrap();
    let (founding_tombstone, _) = close_position(
        PositionLifecycleStateV2::Live(founding),
        PositionEconomicStateV1::ZERO,
        23,
        id(250),
    )
    .unwrap();
    let founding_reopened = reopen_position(founding_tombstone, rent(), id(250)).unwrap();
    let PositionLifecycleStateV2::Live(founding_reopened) = founding_reopened else {
        panic!("founding Position did not reopen")
    };
    assert_eq!(founding_reopened.generation, 1);

    let (closed, _) = close_position(
        PositionLifecycleStateV2::Live(position()),
        PositionEconomicStateV1::ZERO,
        23,
        id(250),
    )
    .unwrap();
    let reopened = reopen_position(closed, rent(), id(250)).unwrap();
    match reopened {
        PositionLifecycleStateV2::Live(live) => {
            assert_eq!(live.generation, 2);
            assert_eq!(live.retirement.outstanding_reservations, 0);
        }
        PositionLifecycleStateV2::Tombstone(_) => panic!("reopen stayed tombstoned"),
    }
    assert_eq!(
        reopen_position(reopened, rent(), id(250)),
        Err(RetirementErrorV1::WrongPhase)
    );

    let overflow = PositionLifecycleStateV2::Tombstone(clutch_retirement::PositionTombstoneV1 {
        market: id(1),
        owner: id(2),
        generation: u64::MAX,
        stored_bump: 1,
    });
    assert_eq!(
        reopen_position(overflow, rent(), id(250)),
        Err(RetirementErrorV1::ArithmeticOverflow)
    );
}

#[test]
fn epoch_generation_and_counts_have_frozen_transition_vectors() {
    let mut epoch = epoch();
    let (position, next_epoch, reservation, archive) =
        register_general_reservation(position(), epoch).unwrap();
    epoch = next_epoch;
    let kinds = [
        EpochChildKindV1::CandidateIndexPage,
        EpochChildKindV1::CandidateVerdict,
        EpochChildKindV1::CandidateEscrow,
        EpochChildKindV1::ClearWorkBundle,
        EpochChildKindV1::OrderPage,
        EpochChildKindV1::SettlementReceipt,
        EpochChildKindV1::FinalPot,
    ];
    let mut slots = [ChildSlotV1::Absent; 7];
    for (index, kind) in kinds.into_iter().enumerate() {
        (epoch, slots[index]) = create_epoch_child(epoch, slots[index], kind).unwrap();
    }
    let (next, candidate) = create_registered_candidate_after_validation(
        epoch,
        ChildSlotV1::Absent,
        CandidateStatusWitnessV1::from_validated_account(13, 3, 0),
    )
    .unwrap();
    epoch = next;
    assert_eq!(
        epoch.retirement.children.encode().unwrap(),
        [
            1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1,
            0, 0, 0, 1, 0, 0, 0,
        ]
    );

    epoch = terminal(epoch);
    let (_, reservation) =
        terminate_reservation(position, reservation, ReservationStateV1::Consumed).unwrap();
    (epoch, _) = close_general_reservation_archive(epoch, archive, reservation).unwrap();
    for slot in slots {
        (epoch, _) = close_epoch_child(epoch, slot).unwrap();
    }
    (epoch, _) = close_registered_candidate(epoch, candidate, false).unwrap();
    assert!(epoch.retirement.children.is_zero());
}

#[test]
fn malformed_live_epoch_tail_never_authorizes_root_close() {
    let mut corrupt = terminal(epoch());
    corrupt.retirement.children.candidate_bundles = 1;
    assert_eq!(
        close_epoch(EpochLifecycleStateV5::Live(corrupt), 23, id(250)),
        Err(RetirementErrorV1::ChildOutstanding)
    );

    let mut invalid = terminal(epoch());
    invalid.retirement = EpochRetirementTailV1 {
        epoch_generation: 0,
        children: EpochChildCountsV1::default(),
        rent: rent(),
    };
    assert_eq!(
        close_epoch(EpochLifecycleStateV5::Live(invalid), 23, id(250)),
        Err(RetirementErrorV1::WrongGeneration)
    );
}
