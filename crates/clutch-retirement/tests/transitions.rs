use clutch_candidate_lifecycle::{CandidateWindowV4, Id as CandidateId, RankKey};
use clutch_retirement::{
    admit_deletable_rent, admit_reopen_rent_split, close_epoch, close_epoch_child_v2,
    close_general_reservation_archive, close_position, close_registered_candidate_v2,
    create_epoch_child, create_epoch_child_v2, create_registered_candidate_after_validation_v2,
    entitle_reservation, entitle_reservation_v2, open_general_epoch, plan_epoch_retirement,
    plan_epoch_root_retirement, plan_general_reservation_close, plan_position_replay_retirement,
    plan_position_retirement, register_direct_reservation, register_direct_reservation_v2,
    register_general_reservation, register_general_reservation_v2, reopen_position,
    reopen_position_with_replay, terminate_reservation, terminate_reservation_v2,
    update_registered_candidate_status_after_validation_v2, AdapterDirectEpochProjectionV1,
    AdapterEpochAccountProjectionV1, AdapterNeutralSinkBindingProjectionV1,
    AdapterPositionAccountProjectionV1, AdapterReplayAbsenceProjectionV1,
    AdapterReplayAccountProjectionV1, CandidateStatusWitnessV1, ChildSlotV1,
    CountedEpochChildProjectionV2, CountedEpochChildSlotV2, CountedReservationV1,
    DeletableRentOwnerV1, DirectEpochLifecyclePhaseV1, DirectReservationRegistrationAccountsV1,
    EpochBudgetRootSiblingV1, EpochChildCountsV1, EpochChildKindV1, EpochChildProjectionV1,
    EpochLifecycleStateV5, EpochRetirementTailV1, EpochRootAccountsV1,
    EpochRootRetirementRequestV1, EpochWindowRootSiblingV1, GeneralEpochLifecycleProjectionV2,
    GeneralEpochPhaseV1, GeneralEpochPhaseV2, GeneralReservationCloseRequestV1,
    GeneralReservationRegistrationAccountsV1, Identity32V1, LiveEpochV5,
    LiveGeneralEpochProjectionV2, LivePositionV2, LiveReplaySuccessorV1, MarketEpochCursorV1,
    PositionEconomicStateV1, PositionLifecycleStateV2, PositionReplayAccountsV1,
    PositionReplayReopenAccountsV1, PositionReplayReopenRequestV1,
    PositionReplayRetirementRequestV1, PositionRetirementTailV1, RecipientBalanceBookV1,
    RecipientBalanceV1, RentSplitV2, ReplayLifecycleStateV1, ReservationStateV1, RetirementErrorV1,
    RetirementErrorV2, ValidatedAdmissionLedgerRetiredV1,
};

fn id(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

fn neutral_sink_binding() -> AdapterNeutralSinkBindingProjectionV1 {
    AdapterNeutralSinkBindingProjectionV1 {
        market: id(1),
        neutral_sink: id(250),
    }
}

fn rent() -> RentSplitV2 {
    RentSplitV2 {
        payer: id(3),
        refundable_live_principal: 11,
        permanent_tombstone_principal: 7,
        donation_floor: 5,
    }
}

fn deletable_rent() -> DeletableRentOwnerV1 {
    DeletableRentOwnerV1::from_persisted(id(5), 13, 3).unwrap()
}

fn reservation_funding(target: u8) -> clutch_retirement::DeletableRentAdmissionPlanV1 {
    admit_deletable_rent(id(target), id(5), 13, 3, 100, id(250)).unwrap()
}

fn reopen_rent_funding() -> clutch_retirement::RentSplitAdmissionPlanV2 {
    admit_reopen_rent_split(id(40), id(3), 11, 7, 7, 100, id(250)).unwrap()
}

fn open_epoch(
    cursor: MarketEpochCursorV1,
    requested_index: u64,
) -> Result<(MarketEpochCursorV1, LiveGeneralEpochProjectionV2), RetirementErrorV2> {
    if requested_index != cursor.next_general_epoch_index {
        return Err(RetirementErrorV2::NonmonotoneEpoch);
    }
    let next = requested_index
        .checked_add(1)
        .ok_or(RetirementErrorV2::EpochIndexExhausted)?;
    Ok((
        MarketEpochCursorV1 {
            next_general_epoch_index: next,
        },
        LiveGeneralEpochProjectionV2 {
            market: id(1),
            epoch: id(4),
            epoch_index: requested_index,
            phase: GeneralEpochPhaseV2::Open,
            stored_bump: 249,
            retirement: EpochRetirementTailV1 {
                epoch_generation: next,
                children: EpochChildCountsV1::default(),
                rent: rent(),
            },
        },
    ))
}

fn direct_epoch(index: u64) -> AdapterDirectEpochProjectionV1 {
    AdapterDirectEpochProjectionV1 {
        account: id(46),
        market: id(1),
        epoch: id(6),
        epoch_index: index,
        lifecycle_phase: DirectEpochLifecyclePhaseV1::PrefreezeOpen,
    }
}

#[test]
fn direct_successor_registration_requires_prefreeze_open_lifecycle() {
    let refused = [
        DirectEpochLifecyclePhaseV1::FrozenEmpty,
        DirectEpochLifecyclePhaseV1::WindowOpen,
        DirectEpochLifecyclePhaseV1::Verifying,
        DirectEpochLifecyclePhaseV1::Selected,
        DirectEpochLifecyclePhaseV1::Terminal,
    ];
    for lifecycle_phase in refused {
        assert_eq!(
            register_direct_reservation_v2(
                position(),
                AdapterDirectEpochProjectionV1 {
                    lifecycle_phase,
                    ..direct_epoch(7)
                },
                admit_deletable_rent(id(45), id(3), 13, 3, 100, id(250)).unwrap(),
                id(250),
                neutral_sink_binding(),
                DirectReservationRegistrationAccountsV1 {
                    position: AdapterPositionAccountProjectionV1 {
                        account: id(44),
                        market: id(1),
                        owner: id(2),
                    },
                    reservation: id(45),
                },
            ),
            Err(RetirementErrorV2::WrongPhase)
        );
    }
}

fn position_account(position: LivePositionV2, account: u8) -> AdapterPositionAccountProjectionV1 {
    AdapterPositionAccountProjectionV1 {
        account: id(account),
        market: position.market,
        owner: position.owner,
    }
}

fn replay_account(position: LivePositionV2, account: u8) -> AdapterReplayAccountProjectionV1 {
    AdapterReplayAccountProjectionV1 {
        account: id(account),
        market: position.market,
        owner: position.owner,
        position_generation: position.generation,
    }
}

fn epoch_account(
    epoch: LiveGeneralEpochProjectionV2,
    account: u8,
) -> AdapterEpochAccountProjectionV1 {
    AdapterEpochAccountProjectionV1 {
        account: id(account),
        market: epoch.market,
        epoch: epoch.epoch,
        epoch_index: epoch.epoch_index,
    }
}

fn general_registration_accounts(
    position: LivePositionV2,
    epoch: LiveGeneralEpochProjectionV2,
) -> GeneralReservationRegistrationAccountsV1 {
    GeneralReservationRegistrationAccountsV1 {
        position: position_account(position, 40),
        epoch: epoch_account(epoch, 41),
        reservation: id(45),
    }
}

fn direct_registration_accounts(
    position: LivePositionV2,
) -> DirectReservationRegistrationAccountsV1 {
    DirectReservationRegistrationAccountsV1 {
        position: position_account(position, 40),
        reservation: id(45),
    }
}

fn retired_candidate_window(epoch: LiveGeneralEpochProjectionV2) -> CandidateWindowV4 {
    CandidateWindowV4 {
        epoch: CandidateId::from_bytes(epoch.epoch.bytes()),
        market: CandidateId::from_bytes(epoch.market.bytes()),
        relation_policy_id: CandidateId::from_bytes(id(10).bytes()),
        admission_policy_id: CandidateId::from_bytes(id(11).bytes()),
        score_policy_id: CandidateId::from_bytes(id(12).bytes()),
        freeze_deadline_slot: 9,
        frozen_slot: 10,
        reveal_opens_slot: 11,
        submission_closes_slot: 12,
        verification_closes_slot: 13,
        finalized_slot: 13,
        admission_head: CandidateId::ZERO,
        best_candidate_node: CandidateId::ZERO,
        selected_candidate_node: CandidateId::ZERO,
        best_rank_key: RankKey::EMPTY,
        admitted_count: 0,
        revealed_count: 0,
        verdict_count: 0,
        valid_verdict_count: 0,
        expired_commitment_count: 0,
        expired_unverified_count: 0,
        live_node_count: 0,
        closed_node_count: 0,
        rank_key_len: 32,
        stored_bump: 8,
        flags: 0,
    }
}

fn admission_ledger(epoch: LiveGeneralEpochProjectionV2) -> ValidatedAdmissionLedgerRetiredV1 {
    ValidatedAdmissionLedgerRetiredV1::from_candidate_window(retired_candidate_window(epoch), epoch)
        .unwrap()
}

fn replay(position: LivePositionV2) -> ReplayLifecycleStateV1 {
    ReplayLifecycleStateV1::Live(LiveReplaySuccessorV1 {
        market: position.market,
        owner: position.owner,
        position_generation: position.generation,
        sequence: 7,
        stored_bump: 248,
        rent: deletable_rent(),
    })
}

fn recipients() -> RecipientBalanceBookV1 {
    RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: rent().payer,
                balance_before: 100,
            }),
            Some(RecipientBalanceV1 {
                recipient: deletable_rent().payer(),
                balance_before: 200,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: 300,
            }),
            None,
        ],
    }
}

fn plan_position_close(
    position: LivePositionV2,
    economic: PositionEconomicStateV1,
    position_balance: u64,
    replay_balance: u64,
    sink: Identity32V1,
) -> Result<clutch_retirement::PositionReplayRetirementPlanV1, RetirementErrorV2> {
    plan_position_replay_retirement(PositionReplayRetirementRequestV1 {
        position: PositionLifecycleStateV2::Live(position),
        replay: replay(position),
        economic,
        position_balance,
        replay_balance,
        neutral_sink: sink,
        neutral_sink_binding: neutral_sink_binding(),
        accounts: PositionReplayAccountsV1 {
            position: position_account(position, 40),
            replay: replay_account(position, 41),
        },
        recipient_balances: recipients(),
    })
}

fn plan_epoch_close(
    epoch: LiveGeneralEpochProjectionV2,
) -> Result<clutch_retirement::EpochRootRetirementPlanV1, RetirementErrorV2> {
    let mut witness_epoch = epoch;
    if witness_epoch.retirement.epoch_generation == 0 {
        witness_epoch.retirement.epoch_generation = 1;
    }
    let window = EpochWindowRootSiblingV1 {
        market: epoch.market,
        epoch: epoch.epoch,
        epoch_generation: epoch.retirement.epoch_generation,
        rent: deletable_rent(),
    };
    let budget = EpochBudgetRootSiblingV1 {
        market: epoch.market,
        epoch: epoch.epoch,
        epoch_generation: epoch.retirement.epoch_generation,
        rent: deletable_rent(),
    };
    plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
        epoch: GeneralEpochLifecycleProjectionV2::Live(epoch),
        window,
        admission_ledger: admission_ledger(witness_epoch),
        budget,
        epoch_balance: 29,
        window_balance: 18,
        budget_balance: 19,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        accounts: EpochRootAccountsV1 {
            epoch: epoch_account(epoch, 42),
            window: id(43),
            budget: id(44),
        },
        recipient_balances: recipients(),
    })
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

fn epoch() -> LiveGeneralEpochProjectionV2 {
    open_epoch(
        MarketEpochCursorV1 {
            next_general_epoch_index: 0,
        },
        0,
    )
    .unwrap()
    .1
}

fn terminal(mut epoch: LiveGeneralEpochProjectionV2) -> LiveGeneralEpochProjectionV2 {
    epoch.phase = GeneralEpochPhaseV2::Settled;
    epoch
}

fn creation_phase(
    mut epoch: LiveGeneralEpochProjectionV2,
    kind: EpochChildKindV1,
) -> LiveGeneralEpochProjectionV2 {
    epoch.phase = match kind {
        EpochChildKindV1::OrderPage => GeneralEpochPhaseV2::Open,
        EpochChildKindV1::CandidateBundle
        | EpochChildKindV1::CandidateIndexPage
        | EpochChildKindV1::CandidateVerdict
        | EpochChildKindV1::CandidateEscrow
        | EpochChildKindV1::ClearWorkBundle => GeneralEpochPhaseV2::Frozen,
        EpochChildKindV1::SettlementReceipt | EpochChildKindV1::FinalPot => {
            GeneralEpochPhaseV2::Cleared
        }
        EpochChildKindV1::ReservationArchive => GeneralEpochPhaseV2::Open,
    };
    epoch
}

fn general_close(
    epoch: LiveGeneralEpochProjectionV2,
    slot: CountedEpochChildSlotV2,
    reservation: clutch_retirement::CountedReservationV2,
) -> Result<clutch_retirement::GeneralReservationClosePlanV1, RetirementErrorV2> {
    plan_general_reservation_close(GeneralReservationCloseRequestV1 {
        epoch,
        epoch_account: epoch_account(epoch, 41),
        slot,
        reservation,
        reservation_balance: 16,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        reservation_account: id(45),
        recipient_balances: RecipientBalanceBookV1 {
            entries: [
                Some(RecipientBalanceV1 {
                    recipient: id(5),
                    balance_before: 100,
                }),
                Some(RecipientBalanceV1 {
                    recipient: id(250),
                    balance_before: 200,
                }),
                None,
                None,
            ],
        },
    })
}

fn reopen_request(position: PositionLifecycleStateV2) -> PositionReplayReopenRequestV1 {
    let tombstone = match position {
        PositionLifecycleStateV2::Tombstone(tombstone) => tombstone,
        PositionLifecycleStateV2::Live(live) => clutch_retirement::PositionTombstoneV1 {
            market: live.market,
            owner: live.owner,
            generation: live.generation,
            stored_bump: live.stored_bump,
        },
    };
    PositionReplayReopenRequestV1 {
        position,
        prior_replay: AdapterReplayAbsenceProjectionV1 {
            account: id(41),
            market: tombstone.market,
            owner: tombstone.owner,
            position_generation: tombstone.generation,
        },
        position_funding: reopen_rent_funding(),
        replay_stored_bump: 7,
        replay_funding: reservation_funding(42),
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        accounts: PositionReplayReopenAccountsV1 {
            position: position_account(
                LivePositionV2 {
                    market: tombstone.market,
                    owner: tombstone.owner,
                    generation: tombstone.generation,
                    stored_bump: tombstone.stored_bump,
                    retirement: PositionRetirementTailV1 {
                        outstanding_reservations: 0,
                        rent: rent(),
                    },
                },
                40,
            ),
            next_replay: AdapterReplayAccountProjectionV1 {
                account: id(42),
                market: tombstone.market,
                owner: tombstone.owner,
                position_generation: tombstone.generation.saturating_add(1),
            },
        },
    }
}

#[test]
fn all_in_local_zero_still_refuses_until_exact_once_terminal_debit() {
    let original_position = position();
    let original_epoch = epoch();
    let (counted_position, counted_epoch, reservation, archive) = register_general_reservation_v2(
        original_position,
        original_epoch,
        reservation_funding(45),
        id(250),
        neutral_sink_binding(),
        general_registration_accounts(original_position, original_epoch),
    )
    .unwrap();
    assert_eq!(counted_position.retirement.outstanding_reservations, 1);
    assert_eq!(counted_epoch.retirement.children.reservation_archives, 1);
    assert!(PositionEconomicStateV1::ZERO.is_zero());
    assert_eq!(
        plan_position_close(
            counted_position,
            PositionEconomicStateV1::ZERO,
            23,
            16,
            id(250)
        ),
        Err(RetirementErrorV2::ReservationOutstanding)
    );

    let entitled = entitle_reservation_v2(reservation).unwrap();
    assert_eq!(counted_position.retirement.outstanding_reservations, 1);
    assert!(entitled.count.position_counted);
    let (zero_position, consumed) =
        terminate_reservation_v2(counted_position, entitled, ReservationStateV1::Consumed).unwrap();
    assert_eq!(zero_position.retirement.outstanding_reservations, 0);
    assert!(!consumed.count.position_counted);
    assert_eq!(
        terminate_reservation_v2(zero_position, consumed, ReservationStateV1::Consumed),
        Err(RetirementErrorV2::AlreadyTerminal)
    );
    assert_eq!(zero_position.retirement.outstanding_reservations, 0);

    let closed = plan_position_close(
        zero_position,
        PositionEconomicStateV1::ZERO,
        29,
        16,
        id(250),
    )
    .unwrap();
    assert_eq!(closed.position_balance_after, 7);
    assert_eq!(closed.replay_balance_after, 0);
    assert_eq!(
        plan_position_replay_retirement(PositionReplayRetirementRequestV1 {
            position: closed.position_post_state,
            replay: closed.replay_post_state,
            economic: PositionEconomicStateV1::ZERO,
            position_balance: 7,
            replay_balance: 0,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: PositionReplayAccountsV1 {
                position: position_account(zero_position, 40),
                replay: replay_account(zero_position, 41),
            },
            recipient_balances: recipients(),
        }),
        Err(RetirementErrorV2::AlreadyTerminal),
    );

    let terminal_epoch = terminal(counted_epoch);
    assert_eq!(
        close_epoch_child_v2(terminal_epoch, archive),
        Err(RetirementErrorV2::WrongChildKind),
        "generic close cannot bypass terminal reservation validation"
    );
    let close = general_close(terminal_epoch, archive, consumed).unwrap();
    let terminal_epoch = close.epoch_post_state;
    let absent = close.reservation_post_slot;
    assert_eq!(absent, CountedEpochChildSlotV2::Absent);
    assert_eq!(terminal_epoch.retirement.children.reservation_archives, 0);
    assert_eq!(
        general_close(terminal_epoch, absent, consumed),
        Err(RetirementErrorV2::ChildAbsent)
    );
}

#[test]
fn released_and_direct_reservations_share_the_position_counter() {
    let initial = position();
    let (position, direct) = register_direct_reservation_v2(
        initial,
        direct_epoch(76),
        reservation_funding(45),
        id(250),
        neutral_sink_binding(),
        direct_registration_accounts(initial),
    )
    .unwrap();
    assert_eq!(position.retirement.outstanding_reservations, 1);
    let (position, released) =
        terminate_reservation_v2(position, direct, ReservationStateV1::Released).unwrap();
    assert_eq!(position.retirement.outstanding_reservations, 0);
    assert_eq!(released.state, ReservationStateV1::Released);
    assert_eq!(
        terminate_reservation_v2(position, released, ReservationStateV1::Released),
        Err(RetirementErrorV2::AlreadyTerminal)
    );
}

#[test]
fn cursor_is_exact_monotone_and_root_retirement_is_budget_blocked() {
    let cursor = MarketEpochCursorV1 {
        next_general_epoch_index: 41,
    };
    for requested in [0, 40, 42, u64::MAX] {
        assert_eq!(
            open_epoch(cursor, requested),
            Err(RetirementErrorV2::NonmonotoneEpoch)
        );
    }
    let (next, mut epoch) = open_epoch(cursor, 41).unwrap();
    assert_eq!(next.next_general_epoch_index, 42);
    assert_eq!(epoch.retirement.epoch_generation, 42);
    epoch.phase = GeneralEpochPhaseV2::Settled;
    assert_eq!(
        plan_epoch_close(epoch),
        Err(RetirementErrorV2::BudgetRetirementUnauthenticated)
    );
    assert_eq!(next.next_general_epoch_index, 42);
    assert_eq!(
        open_epoch(next, 41),
        Err(RetirementErrorV2::NonmonotoneEpoch),
        "a closed Epoch index cannot be replayed behind the Market cursor"
    );

    let exhausted = MarketEpochCursorV1 {
        next_general_epoch_index: u64::MAX,
    };
    assert_eq!(
        open_epoch(exhausted, u64::MAX),
        Err(RetirementErrorV2::EpochIndexExhausted)
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
        let initial = creation_phase(epoch(), kind);
        let (live, child) =
            create_epoch_child_v2(initial, CountedEpochChildSlotV2::Absent, kind).unwrap();
        assert_eq!(live.retirement.children.get(kind), 1);
        let terminal = terminal(live);
        assert_eq!(
            plan_epoch_close(terminal),
            Err(RetirementErrorV2::ChildOutstanding)
        );
        let (empty, absent) = close_epoch_child_v2(terminal, child).unwrap();
        assert_eq!(empty.retirement.children.get(kind), 0);
        assert_eq!(
            close_epoch_child_v2(empty, absent),
            Err(RetirementErrorV2::ChildAbsent)
        );
        assert_eq!(
            plan_epoch_close(empty),
            Err(RetirementErrorV2::BudgetRetirementUnauthenticated)
        );
    }
}

#[test]
fn epoch_child_creation_and_cleanup_are_phase_exact() {
    let cases = [
        (EpochChildKindV1::OrderPage, GeneralEpochPhaseV2::Open),
        (
            EpochChildKindV1::CandidateIndexPage,
            GeneralEpochPhaseV2::Frozen,
        ),
        (
            EpochChildKindV1::CandidateVerdict,
            GeneralEpochPhaseV2::Frozen,
        ),
        (
            EpochChildKindV1::CandidateEscrow,
            GeneralEpochPhaseV2::Frozen,
        ),
        (
            EpochChildKindV1::ClearWorkBundle,
            GeneralEpochPhaseV2::Frozen,
        ),
        (
            EpochChildKindV1::SettlementReceipt,
            GeneralEpochPhaseV2::Cleared,
        ),
        (EpochChildKindV1::FinalPot, GeneralEpochPhaseV2::Cleared),
    ];
    let phases = [
        GeneralEpochPhaseV2::Open,
        GeneralEpochPhaseV2::Frozen,
        GeneralEpochPhaseV2::Cleared,
        GeneralEpochPhaseV2::Settled,
        GeneralEpochPhaseV2::Lapsed,
    ];
    for (kind, admitted_phase) in cases {
        for phase in phases {
            let mut state = epoch();
            state.phase = phase;
            let result = create_epoch_child_v2(state, CountedEpochChildSlotV2::Absent, kind);
            if phase == admitted_phase {
                assert!(result.is_ok());
            } else {
                assert_eq!(result, Err(RetirementErrorV2::WrongPhase));
            }
        }
    }

    let witness = CandidateStatusWitnessV1::from_validated_account(13, 3, 0);
    for phase in phases {
        let mut state = epoch();
        state.phase = phase;
        let result = create_registered_candidate_after_validation_v2(
            state,
            CountedEpochChildSlotV2::Absent,
            witness,
        );
        if phase == GeneralEpochPhaseV2::Frozen {
            assert!(result.is_ok());
        } else {
            assert_eq!(result, Err(RetirementErrorV2::WrongPhase));
        }
    }

    let mut cleared = epoch();
    cleared.phase = GeneralEpochPhaseV2::Cleared;
    let (cleared, receipt) = create_epoch_child_v2(
        cleared,
        CountedEpochChildSlotV2::Absent,
        EpochChildKindV1::SettlementReceipt,
    )
    .unwrap();
    assert_eq!(
        close_epoch_child_v2(cleared, receipt),
        Err(RetirementErrorV2::WrongPhase)
    );
    assert_eq!(
        plan_epoch_close(cleared),
        Err(RetirementErrorV2::WrongPhase)
    );
    let mut settled = cleared;
    settled.phase = GeneralEpochPhaseV2::Settled;
    assert!(close_epoch_child_v2(settled, receipt).is_ok());
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
            let frozen = creation_phase(epoch(), EpochChildKindV1::CandidateBundle);
            let (live, candidate) = create_registered_candidate_after_validation_v2(
                frozen,
                CountedEpochChildSlotV2::Absent,
                initial,
            )
            .unwrap();
            let candidate =
                update_registered_candidate_status_after_validation_v2(candidate, status).unwrap();
            assert_eq!(live.retirement.children.candidate_bundles, 1);
            let terminal = terminal(live);
            assert_eq!(
                plan_epoch_close(terminal),
                Err(RetirementErrorV2::ChildOutstanding)
            );
            assert_eq!(
                close_registered_candidate_v2(terminal, candidate, true),
                Err(RetirementErrorV2::ClearWorkOutstanding)
            );
            let (empty, absent) =
                close_registered_candidate_v2(terminal, candidate, false).unwrap();
            assert_eq!(empty.retirement.children.candidate_bundles, 0);
            assert_eq!(
                close_registered_candidate_v2(empty, absent, false),
                Err(RetirementErrorV2::ChildAbsent)
            );
        }
    }
}

#[test]
fn candidate_and_clear_work_have_independent_exact_counts_and_ordering() {
    let initial = CandidateStatusWitnessV1::from_validated_account(3, 2, 0);
    let frozen = creation_phase(epoch(), EpochChildKindV1::CandidateBundle);
    let (epoch, candidate) = create_registered_candidate_after_validation_v2(
        frozen,
        CountedEpochChildSlotV2::Absent,
        initial,
    )
    .unwrap();
    let candidate = update_registered_candidate_status_after_validation_v2(
        candidate,
        CandidateStatusWitnessV1::from_validated_account(3, 2, 4),
    )
    .unwrap();
    let (epoch, work) = create_epoch_child_v2(
        epoch,
        CountedEpochChildSlotV2::Absent,
        EpochChildKindV1::ClearWorkBundle,
    )
    .unwrap();
    let terminal = terminal(epoch);
    assert_eq!(
        close_registered_candidate_v2(terminal, candidate, true),
        Err(RetirementErrorV2::ClearWorkOutstanding)
    );
    let (terminal, _) = close_epoch_child_v2(terminal, work).unwrap();
    let (terminal, _) = close_registered_candidate_v2(terminal, candidate, false).unwrap();
    assert!(terminal.retirement.children.is_zero());
}

#[test]
fn candidate_status_updates_cannot_switch_the_registered_schema() {
    let initial = CandidateStatusWitnessV1::from_validated_account(3, 2, 0);
    let frozen = creation_phase(epoch(), EpochChildKindV1::CandidateBundle);
    let (live, candidate) = create_registered_candidate_after_validation_v2(
        frozen,
        CountedEpochChildSlotV2::Absent,
        initial,
    )
    .unwrap();
    assert_eq!(
        update_registered_candidate_status_after_validation_v2(
            candidate,
            CandidateStatusWitnessV1::from_validated_account(4, 2, 1),
        ),
        Err(RetirementErrorV2::WrongTag)
    );
    assert_eq!(
        update_registered_candidate_status_after_validation_v2(
            candidate,
            CandidateStatusWitnessV1::from_validated_account(3, 3, 1),
        ),
        Err(RetirementErrorV2::WrongVersion)
    );
    assert_eq!(live.retirement.children.candidate_bundles, 1);
    let CountedEpochChildSlotV2::Present(unchanged) = candidate else {
        panic!("registered candidate disappeared")
    };
    assert_eq!(unchanged.candidate_status, Some(initial));
}

#[test]
fn wrong_generation_overflow_underflow_and_singleton_pot_refuse_without_poststate() {
    let initial = epoch();
    assert_eq!(
        create_epoch_child_v2(
            initial,
            CountedEpochChildSlotV2::Absent,
            EpochChildKindV1::ReservationArchive
        ),
        Err(RetirementErrorV2::WrongChildKind),
        "generic create cannot bypass Position registration"
    );
    let wrong = CountedEpochChildSlotV2::Present(CountedEpochChildProjectionV2 {
        epoch_generation: initial.retirement.epoch_generation + 1,
        kind: EpochChildKindV1::OrderPage,
        candidate_status: None,
    });
    assert_eq!(
        close_epoch_child_v2(terminal(initial), wrong),
        Err(RetirementErrorV2::WrongGeneration)
    );
    assert_eq!(initial.retirement.children, EpochChildCountsV1::default());

    let forged = CountedEpochChildSlotV2::Present(CountedEpochChildProjectionV2 {
        epoch_generation: initial.retirement.epoch_generation,
        kind: EpochChildKindV1::OrderPage,
        candidate_status: None,
    });
    assert_eq!(
        close_epoch_child_v2(terminal(initial), forged),
        Err(RetirementErrorV2::CounterUnderflow)
    );

    let mut overflow = initial;
    overflow.retirement.children.order_pages = u32::MAX;
    assert_eq!(
        create_epoch_child_v2(
            overflow,
            CountedEpochChildSlotV2::Absent,
            EpochChildKindV1::OrderPage
        ),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );

    let cleared = creation_phase(initial, EpochChildKindV1::FinalPot);
    let (one_pot, _) = create_epoch_child_v2(
        cleared,
        CountedEpochChildSlotV2::Absent,
        EpochChildKindV1::FinalPot,
    )
    .unwrap();
    assert_eq!(
        create_epoch_child_v2(
            one_pot,
            CountedEpochChildSlotV2::Absent,
            EpochChildKindV1::FinalPot
        ),
        Err(RetirementErrorV2::NonCanonicalState)
    );
}

#[test]
fn multi_parent_registration_precomputes_both_counters_before_commit() {
    let initial_position = position();
    let mut full_epoch = epoch();
    full_epoch.retirement.children.reservation_archives = u32::MAX;
    assert_eq!(
        register_general_reservation_v2(
            initial_position,
            full_epoch,
            reservation_funding(45),
            id(250),
            neutral_sink_binding(),
            general_registration_accounts(initial_position, full_epoch),
        ),
        Err(RetirementErrorV2::ArithmeticOverflow)
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
        register_general_reservation_v2(
            full_position,
            initial_epoch,
            reservation_funding(45),
            id(250),
            neutral_sink_binding(),
            general_registration_accounts(full_position, initial_epoch),
        ),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );
    assert_eq!(
        initial_epoch.retirement.children.reservation_archives, 0,
        "Epoch cannot increment when Position overflow refuses"
    );
}

#[test]
fn malformed_or_stale_reservation_markers_never_debit_position() {
    let initial = position();
    let (_, reservation) = register_direct_reservation_v2(
        initial,
        direct_epoch(8),
        reservation_funding(45),
        id(250),
        neutral_sink_binding(),
        direct_registration_accounts(initial),
    )
    .unwrap();

    let mut uncounted_active = reservation;
    uncounted_active.count.position_counted = false;
    assert_eq!(
        terminate_reservation_v2(initial, uncounted_active, ReservationStateV1::Released),
        Err(RetirementErrorV2::NonCanonicalState)
    );

    let mut stale = reservation;
    stale.position_generation = initial.generation + 1;
    assert_eq!(
        terminate_reservation_v2(initial, stale, ReservationStateV1::Released),
        Err(RetirementErrorV2::WrongGeneration)
    );

    assert_eq!(
        terminate_reservation_v2(initial, reservation, ReservationStateV1::Released),
        Err(RetirementErrorV2::CounterUnderflow)
    );
    assert_eq!(initial.retirement.outstanding_reservations, 0);
}

#[test]
fn close_refusals_preserve_inputs_and_exact_rent_compartments() {
    let live = position();
    let state = PositionLifecycleStateV2::Live(live);
    for balance in 0..23u64 {
        assert_eq!(
            plan_position_close(live, PositionEconomicStateV1::ZERO, balance, 16, id(250)),
            Err(RetirementErrorV2::AccountBalanceShortfall)
        );
        assert_eq!(state, PositionLifecycleStateV2::Live(live));
    }
    assert_eq!(
        plan_position_close(live, PositionEconomicStateV1::ZERO, 23, 16, rent().payer),
        Err(RetirementErrorV2::WrongNeutralSink)
    );
    assert_eq!(state, PositionLifecycleStateV2::Live(live));

    let mut nonzero = PositionEconomicStateV1::ZERO;
    nonzero.internal_atoms[15] = 1;
    assert_eq!(
        plan_position_close(live, nonzero, 23, 16, id(250)),
        Err(RetirementErrorV2::EconomicBalanceOutstanding)
    );
}

#[test]
fn frozen_v1_v5_v6_api_retains_its_exact_committed_semantics() {
    let legacy_epoch = LiveEpochV5 {
        market: id(1),
        epoch: id(4),
        epoch_index: 7,
        phase: GeneralEpochPhaseV1::Open,
        stored_bump: 9,
        retirement: EpochRetirementTailV1 {
            epoch_generation: 8,
            children: EpochChildCountsV1::default(),
            rent: rent(),
        },
    };
    let (cursor, opened) = open_general_epoch(
        MarketEpochCursorV1 {
            next_general_epoch_index: 7,
        },
        7,
        id(1),
        id(4),
        9,
        rent(),
    )
    .unwrap();
    assert_eq!(cursor.next_general_epoch_index, 8);
    assert_eq!(opened.retirement.epoch_generation, 8);
    let terminal_epoch = LiveEpochV5 {
        phase: GeneralEpochPhaseV1::Lapsed,
        ..legacy_epoch
    };
    assert!(close_epoch(EpochLifecycleStateV5::Live(terminal_epoch), 23, id(250),).is_ok());
    assert!(plan_epoch_retirement(
        EpochLifecycleStateV5::Live(terminal_epoch),
        23,
        id(250),
        100,
        200,
    )
    .is_ok());
    assert!(close_position(
        PositionLifecycleStateV2::Live(position()),
        PositionEconomicStateV1::ZERO,
        23,
        id(250),
    )
    .is_ok());
    assert!(plan_position_retirement(
        PositionLifecycleStateV2::Live(position()),
        PositionEconomicStateV1::ZERO,
        23,
        id(250),
        100,
        200,
    )
    .is_ok());
    let tombstone = PositionLifecycleStateV2::Tombstone(clutch_retirement::PositionTombstoneV1 {
        market: id(1),
        owner: id(2),
        generation: 1,
        stored_bump: 9,
    });
    assert!(reopen_position(tombstone, rent(), id(250)).is_ok());

    let (counted_position, direct) = register_direct_reservation(position(), 8).unwrap();
    let entitled = entitle_reservation(direct).unwrap();
    let (_, terminal) =
        terminate_reservation(counted_position, entitled, ReservationStateV1::Consumed).unwrap();
    assert!(!terminal.count.position_counted);

    let (counted_position, counted_epoch, general, archive) =
        register_general_reservation(position(), legacy_epoch).unwrap();
    let (_, general) =
        terminate_reservation(counted_position, general, ReservationStateV1::Released).unwrap();
    let counted_epoch = LiveEpochV5 {
        phase: GeneralEpochPhaseV1::Lapsed,
        ..counted_epoch
    };
    assert!(close_general_reservation_archive(counted_epoch, archive, general).is_ok());

    let (with_page, page) = create_epoch_child(
        legacy_epoch,
        ChildSlotV1::Absent,
        EpochChildKindV1::OrderPage,
    )
    .unwrap();
    let terminal_epoch = LiveEpochV5 {
        phase: GeneralEpochPhaseV1::Lapsed,
        ..with_page
    };
    assert!(matches!(
        page,
        ChildSlotV1::Present(EpochChildProjectionV1 {
            kind: EpochChildKindV1::OrderPage,
            ..
        })
    ));
    assert_eq!(
        clutch_retirement::close_epoch_child(terminal_epoch, page)
            .unwrap()
            .0
            .retirement
            .children
            .order_pages,
        0
    );

    let malformed = CountedReservationV1 {
        position_generation: 1,
        state: ReservationStateV1::Active,
        count: clutch_retirement::ReservationCountTailV1 {
            epoch_generation: 8,
            position_counted: false,
        },
    };
    assert_eq!(
        entitle_reservation(malformed),
        Err(RetirementErrorV1::NonCanonicalState)
    );
}

#[test]
fn reopen_is_monotone_and_generation_overflow_is_permanent_stop() {
    let mut founding = position();
    founding.generation = 0;
    let (founding, reservation) = register_direct_reservation_v2(
        founding,
        direct_epoch(8),
        reservation_funding(45),
        id(250),
        neutral_sink_binding(),
        direct_registration_accounts(founding),
    )
    .unwrap();
    assert_eq!(reservation.position_generation, 0);
    let (founding, _) =
        terminate_reservation_v2(founding, reservation, ReservationStateV1::Consumed).unwrap();
    let founding_closed =
        plan_position_close(founding, PositionEconomicStateV1::ZERO, 23, 16, id(250)).unwrap();
    assert_eq!(
        founding_closed.replay_post_state,
        ReplayLifecycleStateV1::Absent
    );
    let founding_reopened =
        reopen_position_with_replay(reopen_request(founding_closed.position_post_state))
            .unwrap()
            .position_post_state;
    let PositionLifecycleStateV2::Live(founding_reopened) = founding_reopened else {
        panic!("founding Position did not reopen")
    };
    assert_eq!(founding_reopened.generation, 1);

    let closed =
        plan_position_close(position(), PositionEconomicStateV1::ZERO, 23, 16, id(250)).unwrap();
    assert_eq!(closed.replay_post_state, ReplayLifecycleStateV1::Absent);
    let reopened_plan =
        reopen_position_with_replay(reopen_request(closed.position_post_state)).unwrap();
    let reopened = reopened_plan.position_post_state;
    match reopened {
        PositionLifecycleStateV2::Live(live) => {
            assert_eq!(live.generation, 2);
            assert_eq!(live.retirement.outstanding_reservations, 0);
        }
        PositionLifecycleStateV2::Tombstone(_) => panic!("reopen stayed tombstoned"),
    }
    assert_eq!(
        reopen_position_with_replay(reopen_request(reopened)),
        Err(RetirementErrorV2::WrongPhase),
    );

    let overflow = PositionLifecycleStateV2::Tombstone(clutch_retirement::PositionTombstoneV1 {
        market: id(1),
        owner: id(2),
        generation: u64::MAX,
        stored_bump: 1,
    });
    assert_eq!(
        reopen_position_with_replay(reopen_request(overflow)),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );
}

#[test]
fn epoch_generation_and_counts_have_frozen_transition_vectors() {
    let mut epoch = epoch();
    let initial_position = position();
    let (position, next_epoch, reservation, archive) = register_general_reservation_v2(
        initial_position,
        epoch,
        reservation_funding(45),
        id(250),
        neutral_sink_binding(),
        general_registration_accounts(initial_position, epoch),
    )
    .unwrap();
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
    let mut slots = [CountedEpochChildSlotV2::Absent; 7];
    for (index, kind) in kinds.into_iter().enumerate() {
        epoch = creation_phase(epoch, kind);
        (epoch, slots[index]) = create_epoch_child_v2(epoch, slots[index], kind).unwrap();
    }
    epoch = creation_phase(epoch, EpochChildKindV1::CandidateBundle);
    let (next, candidate) = create_registered_candidate_after_validation_v2(
        epoch,
        CountedEpochChildSlotV2::Absent,
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
        terminate_reservation_v2(position, reservation, ReservationStateV1::Consumed).unwrap();
    epoch = general_close(epoch, archive, reservation)
        .unwrap()
        .epoch_post_state;
    for slot in slots {
        (epoch, _) = close_epoch_child_v2(epoch, slot).unwrap();
    }
    (epoch, _) = close_registered_candidate_v2(epoch, candidate, false).unwrap();
    assert!(epoch.retirement.children.is_zero());
}

#[test]
fn malformed_live_epoch_tail_never_authorizes_root_close() {
    let mut corrupt = terminal(epoch());
    corrupt.retirement.children.candidate_bundles = 1;
    assert_eq!(
        plan_epoch_close(corrupt),
        Err(RetirementErrorV2::ChildOutstanding)
    );

    let mut invalid = terminal(epoch());
    invalid.retirement = EpochRetirementTailV1 {
        epoch_generation: 0,
        children: EpochChildCountsV1::default(),
        rent: rent(),
    };
    assert_eq!(
        plan_epoch_close(invalid),
        Err(RetirementErrorV2::WrongGeneration)
    );
}
