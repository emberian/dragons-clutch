use clutch_deletion_replay_v2::{
    CandidateState, ChildKind, Error, Fault, PositionPhase, Protocol, ReservationState,
};

fn base_with_epoch(eggs: u64) -> Protocol {
    Protocol::new_v2(1, 2, eggs)
        .unwrap()
        .open_epoch(10, 0, Fault::Never)
        .unwrap()
}

#[test]
fn all_in_seller_is_locally_zero_but_cannot_close() {
    let (model, reservation) = base_with_epoch(9)
        .create_sell_reservation(10, 40, 9, Fault::Never)
        .unwrap();
    assert_eq!(model.position.egg_atoms, 0);
    assert_eq!(model.position.cash_atoms, 0);
    assert_eq!(model.position.reserved_cash_atoms, 0);
    assert_eq!(model.position.outstanding_reservations, 1);
    assert_eq!(
        model.close_position(Fault::Never),
        Err(Error::ReservationOutstanding)
    );

    let model = model
        .terminate_reservation(reservation, ReservationState::Consumed, Fault::Never)
        .unwrap();
    assert_eq!(model.position.outstanding_reservations, 0);
    let closed = model.close_position(Fault::Never).unwrap();
    assert_eq!(closed.position.phase, PositionPhase::Tombstone);
    assert_eq!(
        model.terminate_reservation(reservation, ReservationState::Consumed, Fault::Never),
        Err(Error::AlreadyTerminal)
    );
}

#[test]
fn release_decrements_once_and_returns_to_the_bound_generation() {
    let (model, reservation) = base_with_epoch(7)
        .create_sell_reservation(10, 40, 7, Fault::Never)
        .unwrap();
    let released = model
        .terminate_reservation(reservation, ReservationState::Released, Fault::Never)
        .unwrap();
    assert_eq!(released.position.egg_atoms, 7);
    assert_eq!(released.position.outstanding_reservations, 0);
    assert_eq!(
        released.terminate_reservation(reservation, ReservationState::Released, Fault::Never),
        Err(Error::AlreadyTerminal)
    );
    assert_eq!(released.position.outstanding_reservations, 0);
}

#[test]
fn reservation_create_and_terminal_writes_are_atomic_at_every_checkpoint() {
    let before = base_with_epoch(11);
    for stage in 1..=4 {
        assert_eq!(
            before.create_sell_reservation(10, 40, 11, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(before.position.egg_atoms, 11);
        assert_eq!(before.position.outstanding_reservations, 0);
    }
    let (live, registration) = before
        .create_sell_reservation(10, 40, 11, Fault::Never)
        .unwrap();
    for stage in 1..=4 {
        assert_eq!(
            live.terminate_reservation(registration, ReservationState::Consumed, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(live.position.outstanding_reservations, 1);
        assert_eq!(live.reservation_state(40), Some(ReservationState::Active));
    }
}

#[test]
fn every_candidate_status_is_counted_even_when_not_retained() {
    let states = [
        CandidateState::Submitted,
        CandidateState::Staging,
        CandidateState::SealedUnverified,
        CandidateState::VerifiedValid,
        CandidateState::VerifiedRetained,
        CandidateState::Superseded,
        CandidateState::Refused,
        CandidateState::ExpiredStaging,
        CandidateState::ExpiredUnverified,
        CandidateState::Selected,
    ];
    for (offset, state) in states.into_iter().enumerate() {
        let (model, _) = base_with_epoch(0)
            .create_candidate(10, 20 + offset as u64, state, Fault::Never)
            .unwrap();
        let model = model.mark_epoch_terminal(10).unwrap();
        assert_eq!(
            model.close_epoch(10, Fault::Never),
            Err(Error::ChildOutstanding),
            "status {state:?} escaped the exhaustive count"
        );
    }
}

#[test]
fn clear_work_must_close_before_its_candidate_and_root() {
    let (model, candidate) = base_with_epoch(0)
        .create_candidate(10, 20, CandidateState::Superseded, Fault::Never)
        .unwrap();
    let (model, work) = model
        .create_clear_work(candidate, 21, Fault::Never)
        .unwrap();
    let model = model.mark_epoch_terminal(10).unwrap();
    assert_eq!(
        model.close_candidate(candidate, Fault::Never),
        Err(Error::ClearWorkOutstanding)
    );
    assert_eq!(
        model.close_epoch(10, Fault::Never),
        Err(Error::ChildOutstanding)
    );
    let model = model.close_clear_work(work, Fault::Never).unwrap();
    assert_eq!(
        model.close_clear_work(work, Fault::Never),
        Err(Error::MissingChild)
    );
    let model = model.close_candidate(candidate, Fault::Never).unwrap();
    assert!(model.close_epoch(10, Fault::Never).is_ok());
}

#[test]
fn every_epoch_child_family_blocks_root_retirement() {
    for (offset, kind) in [
        ChildKind::CandidateIndexPage,
        ChildKind::CandidateVerdict,
        ChildKind::CandidateEscrow,
        ChildKind::OrderPage,
        ChildKind::Receipt,
        ChildKind::Pot,
    ]
    .into_iter()
    .enumerate()
    {
        let (model, _) = base_with_epoch(0)
            .create_aux(10, 50 + offset as u64, kind, Fault::Never)
            .unwrap();
        let terminal = model.mark_epoch_terminal(10).unwrap();
        assert_eq!(
            terminal.close_epoch(10, Fault::Never),
            Err(Error::ChildOutstanding)
        );
    }

    let (model, reservation) = base_with_epoch(1)
        .create_sell_reservation(10, 60, 1, Fault::Never)
        .unwrap();
    let model = model
        .terminate_reservation(reservation, ReservationState::Consumed, Fault::Never)
        .unwrap()
        .mark_epoch_terminal(10)
        .unwrap();
    assert_eq!(
        model.close_epoch(10, Fault::Never),
        Err(Error::ChildOutstanding)
    );
    let model = model
        .close_reservation_archive(reservation, Fault::Never)
        .unwrap();
    assert!(model.close_epoch(10, Fault::Never).is_ok());
}

#[test]
fn child_create_and_close_roll_back_at_each_write_boundary() {
    let before = base_with_epoch(0);
    for stage in 1..=2 {
        assert_eq!(
            before.create_candidate(10, 20, CandidateState::SealedUnverified, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(before.epoch(10).unwrap().children.candidate_bundles, 0);
    }
    let (live, candidate) = before
        .create_candidate(10, 20, CandidateState::SealedUnverified, Fault::Never)
        .unwrap();
    let terminal = live.mark_epoch_terminal(10).unwrap();
    for stage in 1..=2 {
        assert_eq!(
            terminal.close_candidate(candidate, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(terminal.epoch(10).unwrap().children.candidate_bundles, 1);
    }
}

#[test]
fn work_aux_and_archive_closes_are_exact_once_and_atomic() {
    let (model, candidate) = base_with_epoch(1)
        .create_candidate(10, 20, CandidateState::Refused, Fault::Never)
        .unwrap();
    for stage in 1..=2 {
        assert_eq!(
            model.create_clear_work(candidate, 21, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(model.epoch(10).unwrap().children.clear_work_bundles, 0);
    }
    let (model, work) = model
        .create_clear_work(candidate, 21, Fault::Never)
        .unwrap();
    let (model, page) = model
        .create_aux(10, 22, ChildKind::OrderPage, Fault::Never)
        .unwrap();
    let (model, reservation) = model
        .create_sell_reservation(10, 23, 1, Fault::Never)
        .unwrap();
    let terminal = model
        .terminate_reservation(reservation, ReservationState::Consumed, Fault::Never)
        .unwrap()
        .mark_epoch_terminal(10)
        .unwrap();

    for stage in 1..=2 {
        assert_eq!(
            terminal.close_clear_work(work, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(terminal.epoch(10).unwrap().children.clear_work_bundles, 1);
        assert_eq!(
            terminal.close_aux(page, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(terminal.epoch(10).unwrap().children.order_pages, 1);
        assert_eq!(
            terminal.close_reservation_archive(reservation, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(terminal.epoch(10).unwrap().children.reservation_archives, 1);
    }

    let terminal = terminal.close_clear_work(work, Fault::Never).unwrap();
    assert_eq!(
        terminal.close_clear_work(work, Fault::Never),
        Err(Error::MissingChild)
    );
    let terminal = terminal.close_aux(page, Fault::Never).unwrap();
    assert_eq!(
        terminal.close_aux(page, Fault::Never),
        Err(Error::MissingChild)
    );
    let terminal = terminal
        .close_reservation_archive(reservation, Fault::Never)
        .unwrap();
    assert_eq!(
        terminal.close_reservation_archive(reservation, Fault::Never),
        Err(Error::MissingChild)
    );
}

#[test]
fn tombstones_and_market_cursor_make_epoch_and_position_replay_monotone() {
    let model = base_with_epoch(0).mark_epoch_terminal(10).unwrap();
    let tombstoned = model.close_epoch(10, Fault::Never).unwrap();
    assert_eq!(
        tombstoned.close_epoch(10, Fault::Never),
        Err(Error::WrongPhase)
    );
    assert_eq!(
        tombstoned.open_epoch(10, 0, Fault::Never),
        Err(Error::NonmonotoneEpoch)
    );
    assert_eq!(
        tombstoned.open_epoch(11, 0, Fault::Never),
        Err(Error::NonmonotoneEpoch)
    );
    let model = tombstoned.open_epoch(11, 1, Fault::Never).unwrap();
    assert_eq!(model.next_epoch_index, 2);

    let closed = Protocol::new_v2(1, 2, 0)
        .unwrap()
        .close_position(Fault::Never)
        .unwrap();
    let reopened = closed.reopen_position(0, Fault::Never).unwrap();
    assert_eq!(reopened.position.generation, 2);
}

#[test]
fn epoch_open_close_and_position_reopen_are_atomic() {
    let before = Protocol::new_v2(1, 2, 0).unwrap();
    for stage in 1..=2 {
        assert_eq!(
            before.open_epoch(10, 0, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(before.next_epoch_index, 0);
        assert_eq!(before.epoch(10), None);
    }

    let terminal = before
        .open_epoch(10, 0, Fault::Never)
        .unwrap()
        .mark_epoch_terminal(10)
        .unwrap();
    assert_eq!(
        terminal.close_epoch(10, Fault::At(1)),
        Err(Error::InjectedCrash)
    );
    assert_eq!(
        terminal.epoch(10).unwrap().phase,
        clutch_deletion_replay_v2::EpochPhase::Terminal
    );

    let closed = before.close_position(Fault::Never).unwrap();
    assert_eq!(
        before.close_position(Fault::At(1)),
        Err(Error::InjectedCrash)
    );
    assert_eq!(before.position.phase, PositionPhase::Open);
    for stage in 1..=2 {
        assert_eq!(
            closed.reopen_position(0, Fault::At(stage)),
            Err(Error::InjectedCrash)
        );
        assert_eq!(closed.position.generation, 1);
        assert_eq!(closed.position.phase, PositionPhase::Tombstone);
    }
}
