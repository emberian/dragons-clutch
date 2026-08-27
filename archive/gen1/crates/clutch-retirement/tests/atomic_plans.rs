use clutch_candidate_lifecycle::{CandidateWindowV4, Id as CandidateId, RankKey};
use clutch_retirement::{
    admit_deletable_rent, admit_initial_rent_split, admit_reopen_rent_split,
    open_general_epoch_root, plan_direct_reservation_close, plan_epoch_root_retirement,
    plan_epoch_root_retirement_v2, plan_general_reservation_close,
    plan_position_replay_retirement,
    register_direct_reservation_v2, register_general_reservation_v2, reopen_position_with_replay,
    terminate_reservation_v2, AdapterDirectEpochProjectionV1, AdapterEpochAccountProjectionV1,
    AdapterMarketAccountProjectionV1, AdapterNeutralSinkBindingProjectionV1,
    AdapterPositionAccountProjectionV1, AdapterReplayAbsenceProjectionV1,
    AdapterReplayAccountProjectionV1, AuthenticatedEpochBudgetDispositionV1,
    CountedEpochChildSlotV2, CountedReservationV2,
    DeletableRentOwnerV1, DirectEpochLifecyclePhaseV1, DirectReservationCloseRequestV1,
    DirectReservationRegistrationAccountsV1, EpochBudgetRootSiblingV1, EpochChildCountsV1,
    EpochRetirementTailV1, EpochRootAccountsV1, EpochRootRecipientBalanceBookV2,
    EpochRootRetirementRequestV1, EpochRootRetirementRequestV2,
    EpochWindowRootSiblingV1, GeneralEpochLifecycleProjectionV2, GeneralEpochPhaseV2,
    GeneralReservationCloseRequestV1, GeneralReservationRegistrationAccountsV1, Identity32V1,
    LiveGeneralEpochProjectionV2, LivePositionV2, LiveReplaySuccessorV1, MarketEpochCursorV1,
    OpenGeneralEpochRootPlanV1, OpenGeneralEpochRootRequestV1, PositionEconomicStateV1,
    PositionLifecycleStateV2, PositionReplayAccountsV1, PositionReplayReopenAccountsV1,
    PositionReplayReopenRequestV1, PositionReplayRetirementPlanV1,
    PositionReplayRetirementRequestV1, PositionRetirementTailV1, PositionTombstoneV1,
    RecipientBalanceBookV1, RecipientBalanceV1, RentSplitV2, ReplayLifecycleStateV1,
    ReservationStateV1, RetirementErrorV2, ValidatedAdmissionLedgerRetiredV1,
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

fn initial_funding(
    target: u8,
    payer: u8,
    live: u64,
    tombstone: u64,
    donation: u64,
    payer_balance: u64,
) -> clutch_retirement::RentSplitAdmissionPlanV2 {
    admit_initial_rent_split(
        id(target),
        id(payer),
        live,
        tombstone,
        donation,
        payer_balance,
        id(250),
    )
    .unwrap()
}

fn open_plan(
    cursor: MarketEpochCursorV1,
    requested_index: u64,
) -> Result<OpenGeneralEpochRootPlanV1, RetirementErrorV2> {
    open_general_epoch_root(open_request(cursor, requested_index))
}

fn open_request(
    cursor: MarketEpochCursorV1,
    requested_index: u64,
) -> OpenGeneralEpochRootRequestV1 {
    OpenGeneralEpochRootRequestV1 {
        cursor,
        requested_index,
        market: id(1),
        market_account: AdapterMarketAccountProjectionV1 {
            account: id(39),
            market: id(1),
        },
        epoch: id(4),
        stored_bump: 9,
        epoch_funding: initial_funding(42, 3, 11, 7, 5, 10_000),
        window_funding: funding(40, 5, 13, 3),
        budget_funding: funding(41, 6, 17, 2),
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        accounts: EpochRootAccountsV1 {
            epoch: AdapterEpochAccountProjectionV1 {
                account: id(42),
                market: id(1),
                epoch: id(4),
                epoch_index: requested_index,
            },
            window: id(40),
            budget: id(41),
        },
    }
}

fn reservation_recipients() -> RecipientBalanceBookV1 {
    RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(3),
                balance_before: 100,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: 200,
            }),
            None,
            None,
        ],
    }
}

fn general_reservation_close(
    epoch: LiveGeneralEpochProjectionV2,
    archive: CountedEpochChildSlotV2,
    reservation: CountedReservationV2,
) -> Result<clutch_retirement::GeneralReservationClosePlanV1, RetirementErrorV2> {
    plan_general_reservation_close(GeneralReservationCloseRequestV1 {
        epoch,
        epoch_account: epoch_account(epoch, 41),
        slot: archive,
        reservation,
        reservation_balance: 20,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        reservation_account: id(45),
        recipient_balances: reservation_recipients(),
    })
}

fn direct_reservation_close(
    direct_epoch: AdapterDirectEpochProjectionV1,
    reservation: CountedReservationV2,
) -> Result<clutch_retirement::DirectReservationClosePlanV1, RetirementErrorV2> {
    plan_direct_reservation_close(DirectReservationCloseRequestV1 {
        direct_epoch,
        reservation,
        reservation_balance: 20,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        reservation_account: id(45),
        recipient_balances: reservation_recipients(),
    })
}

fn rent() -> RentSplitV2 {
    RentSplitV2 {
        payer: id(3),
        refundable_live_principal: 11,
        permanent_tombstone_principal: 7,
        donation_floor: 5,
    }
}

fn deletable(payer: u8, principal: u64, donation: u64) -> DeletableRentOwnerV1 {
    DeletableRentOwnerV1::from_persisted(id(payer), principal, donation).unwrap()
}

fn funding(
    target: u8,
    payer: u8,
    principal: u64,
    donation: u64,
) -> clutch_retirement::DeletableRentAdmissionPlanV1 {
    admit_deletable_rent(id(target), id(payer), principal, donation, 10_000, id(250)).unwrap()
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
    LiveGeneralEpochProjectionV2 {
        market: id(1),
        epoch: id(4),
        epoch_index: 0,
        phase: GeneralEpochPhaseV2::Open,
        stored_bump: 249,
        retirement: EpochRetirementTailV1 {
            epoch_generation: 1,
            children: EpochChildCountsV1::default(),
            rent: rent(),
        },
    }
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
        sequence: 4,
        stored_bump: 248,
        rent: deletable(3, 13, 3),
    })
}

fn position_recipients(payer_balance: u64, sink_balance: u64) -> RecipientBalanceBookV1 {
    RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(3),
                balance_before: payer_balance,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: sink_balance,
            }),
            None,
            None,
        ],
    }
}

fn position_plan(
    live: LivePositionV2,
    recipient_balances: RecipientBalanceBookV1,
) -> Result<PositionReplayRetirementPlanV1, RetirementErrorV2> {
    plan_position_replay_retirement(PositionReplayRetirementRequestV1 {
        position: PositionLifecycleStateV2::Live(live),
        replay: replay(live),
        economic: PositionEconomicStateV1::ZERO,
        position_balance: 29,
        replay_balance: 20,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        accounts: PositionReplayAccountsV1 {
            position: position_account(live, 40),
            replay: replay_account(live, 41),
        },
        recipient_balances,
    })
}

#[test]
fn hostile_prefund_never_discounts_full_principal() {
    let plan = admit_deletable_rent(id(40), id(3), 13, 9, 100, id(250)).unwrap();
    assert_eq!(plan.payer_balance_after(), 87);
    assert_eq!(plan.account_balance_after(), 22);
    assert_eq!(plan.rent().refundable_principal(), 13);
    assert_eq!(plan.rent().donation_floor(), 9);
    assert_eq!(
        admit_deletable_rent(id(40), id(3), 13, 9, 12, id(250)),
        Err(RetirementErrorV2::PayerBalanceShortfall)
    );
    assert_eq!(
        admit_deletable_rent(id(40), id(3), 13, u64::MAX, 100, id(250)),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );
    assert_eq!(
        admit_deletable_rent(id(40), id(250), 13, 9, 100, id(250)),
        Err(RetirementErrorV2::PayerIsNeutralSink)
    );
}

#[test]
fn root_open_coalesces_debits_and_refuses_poisoned_bundle_identities() {
    let cursor = MarketEpochCursorV1 {
        next_general_epoch_index: 7,
    };
    let same_payer_epoch = admit_initial_rent_split(id(42), id(3), 4, 4, 1, 30, id(250)).unwrap();
    let same_payer_window = admit_deletable_rent(id(40), id(3), 8, 1, 30, id(250)).unwrap();
    let same_payer_budget = admit_deletable_rent(id(41), id(3), 8, 2, 30, id(250)).unwrap();
    let mut request = open_request(cursor, 7);
    request.epoch_funding = same_payer_epoch;
    request.window_funding = same_payer_window;
    request.budget_funding = same_payer_budget;
    assert_eq!(
        open_general_epoch_root(request),
        Err(RetirementErrorV2::BudgetFundingUnauthenticated)
    );

    let mut short = request;
    short.epoch_funding = admit_initial_rent_split(id(42), id(3), 4, 4, 1, 20, id(250)).unwrap();
    short.window_funding = admit_deletable_rent(id(40), id(3), 8, 1, 20, id(250)).unwrap();
    short.budget_funding = admit_deletable_rent(id(41), id(3), 8, 2, 20, id(250)).unwrap();
    assert_eq!(
        open_general_epoch_root(short),
        Err(RetirementErrorV2::PayerBalanceShortfall)
    );

    let mut inconsistent = request;
    inconsistent.window_funding = admit_deletable_rent(id(40), id(3), 8, 1, 31, id(250)).unwrap();
    assert_eq!(
        open_general_epoch_root(inconsistent),
        Err(RetirementErrorV2::InconsistentPayerBalance)
    );

    let mut wrong_sink = request;
    wrong_sink.neutral_sink = id(249);
    assert_eq!(
        open_general_epoch_root(wrong_sink),
        Err(RetirementErrorV2::WrongNeutralSink)
    );

    let mut target_is_payer = request;
    target_is_payer.accounts.window = id(3);
    target_is_payer.window_funding = admit_deletable_rent(id(3), id(3), 8, 1, 30, id(250)).unwrap();
    assert_eq!(
        open_general_epoch_root(target_is_payer),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut target_is_sink = request;
    target_is_sink.accounts.window = id(250);
    target_is_sink.window_funding =
        admit_deletable_rent(id(250), id(3), 8, 1, 30, id(250)).unwrap();
    assert_eq!(
        open_general_epoch_root(target_is_sink),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut targets_alias = request;
    targets_alias.accounts.budget = targets_alias.accounts.window;
    targets_alias.budget_funding = admit_deletable_rent(id(40), id(3), 8, 2, 30, id(250)).unwrap();
    assert_eq!(
        open_general_epoch_root(targets_alias),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut semantic_market_is_not_account = request;
    semantic_market_is_not_account.accounts.window = semantic_market_is_not_account.market;
    semantic_market_is_not_account.window_funding =
        admit_deletable_rent(id(1), id(3), 8, 1, 30, id(250)).unwrap();
    assert_eq!(
        open_general_epoch_root(semantic_market_is_not_account),
        Err(RetirementErrorV2::BudgetFundingUnauthenticated)
    );
    let mut target_is_market = request;
    target_is_market.accounts.window = target_is_market.market_account.account;
    target_is_market.window_funding =
        admit_deletable_rent(id(39), id(3), 8, 1, 30, id(250)).unwrap();
    assert_eq!(
        open_general_epoch_root(target_is_market),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut false_epoch_identity = request;
    false_epoch_identity.accounts.epoch.epoch = id(9);
    assert_eq!(
        open_general_epoch_root(false_epoch_identity),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_market_projection = request;
    wrong_market_projection.market_account.market = id(9);
    assert_eq!(
        open_general_epoch_root(wrong_market_projection),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_market = request;
    wrong_epoch_market.accounts.epoch.market = id(9);
    assert_eq!(
        open_general_epoch_root(wrong_epoch_market),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_index = request;
    wrong_epoch_index.accounts.epoch.epoch_index = request.requested_index + 1;
    assert_eq!(
        open_general_epoch_root(wrong_epoch_index),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut swapped_plans = request;
    core::mem::swap(
        &mut swapped_plans.window_funding,
        &mut swapped_plans.budget_funding,
    );
    assert_eq!(
        open_general_epoch_root(swapped_plans),
        Err(RetirementErrorV2::WrongFundingTarget)
    );
}

#[test]
fn position_replay_reopen_coalesces_debits_and_refuses_aliases() {
    let tombstone = PositionLifecycleStateV2::Tombstone(PositionTombstoneV1 {
        market: id(1),
        owner: id(2),
        generation: 1,
        stored_bump: 250,
    });
    let request = PositionReplayReopenRequestV1 {
        position: tombstone,
        prior_replay: AdapterReplayAbsenceProjectionV1 {
            account: id(41),
            market: id(1),
            owner: id(2),
            position_generation: 1,
        },
        position_funding: admit_reopen_rent_split(id(40), id(3), 4, 4, 4, 20, id(250)).unwrap(),
        replay_stored_bump: 248,
        replay_funding: admit_deletable_rent(id(42), id(3), 8, 1, 20, id(250)).unwrap(),
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        accounts: PositionReplayReopenAccountsV1 {
            position: AdapterPositionAccountProjectionV1 {
                account: id(40),
                market: id(1),
                owner: id(2),
            },
            next_replay: AdapterReplayAccountProjectionV1 {
                account: id(42),
                market: id(1),
                owner: id(2),
                position_generation: 2,
            },
        },
    };
    let plan = reopen_position_with_replay(request).unwrap();
    let payer = plan.payer_debits.get(id(3)).unwrap();
    assert_eq!(payer.debit_lamports, 12);
    assert_eq!(payer.balance_after, 8);
    assert_eq!(plan.position_balance_after, 8);
    assert_eq!(plan.replay_balance_after, 9);

    let mut short = request;
    short.position_funding = admit_reopen_rent_split(id(40), id(3), 4, 4, 4, 10, id(250)).unwrap();
    short.replay_funding = admit_deletable_rent(id(42), id(3), 8, 1, 10, id(250)).unwrap();
    assert_eq!(
        reopen_position_with_replay(short),
        Err(RetirementErrorV2::PayerBalanceShortfall)
    );
    let mut inconsistent = request;
    inconsistent.replay_funding = admit_deletable_rent(id(42), id(3), 8, 1, 21, id(250)).unwrap();
    assert_eq!(
        reopen_position_with_replay(inconsistent),
        Err(RetirementErrorV2::InconsistentPayerBalance)
    );
    let mut wrong_sink = request;
    wrong_sink.neutral_sink = id(249);
    assert_eq!(
        reopen_position_with_replay(wrong_sink),
        Err(RetirementErrorV2::WrongNeutralSink)
    );
    let mut wrong_prior = request;
    wrong_prior.prior_replay.market = id(9);
    assert_eq!(
        reopen_position_with_replay(wrong_prior),
        Err(RetirementErrorV2::ReplayMismatch)
    );
    let mut wrong_prior_owner = request;
    wrong_prior_owner.prior_replay.owner = id(9);
    assert_eq!(
        reopen_position_with_replay(wrong_prior_owner),
        Err(RetirementErrorV2::ReplayMismatch)
    );
    let mut wrong_prior_generation = request;
    wrong_prior_generation.prior_replay.position_generation = 9;
    assert_eq!(
        reopen_position_with_replay(wrong_prior_generation),
        Err(RetirementErrorV2::ReplayMismatch)
    );
    let mut wrong_position_projection = request;
    wrong_position_projection.accounts.position.owner = id(9);
    assert_eq!(
        reopen_position_with_replay(wrong_position_projection),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_next_replay = request;
    wrong_next_replay.accounts.next_replay.position_generation = 9;
    assert_eq!(
        reopen_position_with_replay(wrong_next_replay),
        Err(RetirementErrorV2::ReplayMismatch)
    );
    let mut target_is_payer = request;
    target_is_payer.accounts.next_replay.account = id(3);
    target_is_payer.replay_funding = admit_deletable_rent(id(3), id(3), 8, 1, 20, id(250)).unwrap();
    assert_eq!(
        reopen_position_with_replay(target_is_payer),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut target_is_sink = request;
    target_is_sink.accounts.next_replay.account = id(250);
    target_is_sink.replay_funding =
        admit_deletable_rent(id(250), id(3), 8, 1, 20, id(250)).unwrap();
    assert_eq!(
        reopen_position_with_replay(target_is_sink),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut targets_alias = request;
    targets_alias.accounts.next_replay.account = targets_alias.accounts.position.account;
    targets_alias.replay_funding = admit_deletable_rent(id(40), id(3), 8, 1, 20, id(250)).unwrap();
    assert_eq!(
        reopen_position_with_replay(targets_alias),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut replay_addresses_alias = request;
    replay_addresses_alias.accounts.next_replay.account =
        replay_addresses_alias.prior_replay.account;
    replay_addresses_alias.replay_funding =
        admit_deletable_rent(id(41), id(3), 8, 1, 20, id(250)).unwrap();
    assert_eq!(
        reopen_position_with_replay(replay_addresses_alias),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut wrong_absence = request;
    wrong_absence.prior_replay.position_generation = 2;
    assert_eq!(
        reopen_position_with_replay(wrong_absence),
        Err(RetirementErrorV2::ReplayMismatch)
    );
    wrong_absence = request;
    wrong_absence.prior_replay.owner = id(9);
    assert_eq!(
        reopen_position_with_replay(wrong_absence),
        Err(RetirementErrorV2::ReplayMismatch)
    );
    let mut wrong_target = request;
    wrong_target.replay_funding = admit_deletable_rent(id(43), id(3), 8, 1, 20, id(250)).unwrap();
    assert_eq!(
        reopen_position_with_replay(wrong_target),
        Err(RetirementErrorV2::WrongFundingTarget)
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RegistrationBank {
    position: LivePositionV2,
    epoch: LiveGeneralEpochProjectionV2,
    reservation: Option<CountedReservationV2>,
    archive: CountedEpochChildSlotV2,
    position_egg_atoms: u64,
    reservation_egg_atoms: u64,
}

fn staged_general_registration(
    bank: &mut RegistrationBank,
    fail_after_write: Option<u8>,
) -> Result<(), ()> {
    let (position_after, epoch_after, reservation, archive) = register_general_reservation_v2(
        bank.position,
        bank.epoch,
        funding(45, 3, 13, 3),
        id(250),
        neutral_sink_binding(),
        general_registration_accounts(bank.position, bank.epoch),
    )
    .map_err(|_| ())?;
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
fn registration_plan_rolls_back_every_modeled_write() {
    let original = RegistrationBank {
        position: position(),
        epoch: epoch(),
        reservation: None,
        archive: CountedEpochChildSlotV2::Absent,
        position_egg_atoms: 17,
        reservation_egg_atoms: 0,
    };
    for fail_after in 1u8..=5 {
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
    assert_eq!(committed.reservation.unwrap().rent, deletable(3, 13, 3));
}

#[test]
fn reservation_registration_binds_prefund_target_and_every_actual_account() {
    let position = position();
    let epoch = epoch();
    let accounts = general_registration_accounts(position, epoch);

    assert_eq!(
        register_general_reservation_v2(
            position,
            epoch,
            funding(44, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            accounts,
        ),
        Err(RetirementErrorV2::WrongFundingTarget)
    );

    for aliased_target in [
        id(3),
        id(250),
        accounts.position.account,
        accounts.epoch.account,
    ] {
        let mut aliased = accounts;
        aliased.reservation = aliased_target;
        assert_eq!(
            register_general_reservation_v2(
                position,
                epoch,
                admit_deletable_rent(aliased_target, id(3), 13, 3, 100, id(250)).unwrap(),
                id(250),
                neutral_sink_binding(),
                aliased,
            ),
            Err(RetirementErrorV2::AccountAlias)
        );
    }

    let mut wrong_position = accounts;
    wrong_position.position.owner = id(9);
    assert_eq!(
        register_general_reservation_v2(
            position,
            epoch,
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            wrong_position,
        ),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_position_market = accounts;
    wrong_position_market.position.market = id(9);
    assert_eq!(
        register_general_reservation_v2(
            position,
            epoch,
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            wrong_position_market,
        ),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch = accounts;
    wrong_epoch.epoch.epoch = id(9);
    assert_eq!(
        register_general_reservation_v2(
            position,
            epoch,
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            wrong_epoch,
        ),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_market = accounts;
    wrong_epoch_market.epoch.market = id(9);
    assert_eq!(
        register_general_reservation_v2(
            position,
            epoch,
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            wrong_epoch_market,
        ),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_index = accounts;
    wrong_epoch_index.epoch.epoch_index = 9;
    assert_eq!(
        register_general_reservation_v2(
            position,
            epoch,
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            wrong_epoch_index,
        ),
        Err(RetirementErrorV2::WrongParent)
    );
    assert_eq!(
        register_general_reservation_v2(
            position,
            epoch,
            funding(45, 3, 13, 3),
            id(249),
            neutral_sink_binding(),
            accounts,
        ),
        Err(RetirementErrorV2::WrongNeutralSink)
    );

    let direct_epoch = direct_epoch(90);
    let direct_accounts = direct_registration_accounts(position);
    assert_eq!(
        register_direct_reservation_v2(
            position,
            direct_epoch,
            funding(44, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            direct_accounts,
        ),
        Err(RetirementErrorV2::WrongFundingTarget)
    );
    for aliased_target in [
        id(3),
        id(250),
        direct_accounts.position.account,
        direct_epoch.account,
    ] {
        let mut aliased = direct_accounts;
        aliased.reservation = aliased_target;
        assert_eq!(
            register_direct_reservation_v2(
                position,
                direct_epoch,
                admit_deletable_rent(aliased_target, id(3), 13, 3, 100, id(250)).unwrap(),
                id(250),
                neutral_sink_binding(),
                aliased,
            ),
            Err(RetirementErrorV2::AccountAlias)
        );
    }
    let mut wrong_direct_position = direct_accounts;
    wrong_direct_position.position.owner = id(9);
    assert_eq!(
        register_direct_reservation_v2(
            position,
            direct_epoch,
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            wrong_direct_position,
        ),
        Err(RetirementErrorV2::WrongParent)
    );
    assert_eq!(
        register_direct_reservation_v2(
            position,
            direct_epoch,
            funding(45, 3, 13, 3),
            id(249),
            neutral_sink_binding(),
            direct_accounts,
        ),
        Err(RetirementErrorV2::WrongNeutralSink)
    );
}

#[test]
fn position_and_replay_close_are_one_coalesced_alias_safe_plan() {
    let plan = position_plan(position(), position_recipients(100, 200)).unwrap();
    assert_eq!(plan.position_balance_after, 7);
    assert_eq!(plan.replay_balance_after, 0);
    assert_eq!(plan.replay_post_state, ReplayLifecycleStateV1::Absent);
    let payer = plan.recipient_credits.get(id(3)).unwrap();
    assert_eq!(payer.credit_lamports, 24);
    assert_eq!(payer.balance_after, 124);
    let sink = plan.recipient_credits.get(id(250)).unwrap();
    assert_eq!(sink.credit_lamports, 18);
    assert_eq!(sink.balance_after, 218);
    assert_eq!(24 + 18 + 7, 29 + 20);

    assert_eq!(
        position_plan(position(), position_recipients(u64::MAX - 23, 200)),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );
    assert_eq!(
        position_plan(position(), position_recipients(100, u64::MAX - 17)),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );

    let base = PositionReplayRetirementRequestV1 {
        position: PositionLifecycleStateV2::Live(position()),
        replay: replay(position()),
        economic: PositionEconomicStateV1::ZERO,
        position_balance: 29,
        replay_balance: 20,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        accounts: PositionReplayAccountsV1 {
            position: position_account(position(), 40),
            replay: replay_account(position(), 41),
        },
        recipient_balances: position_recipients(100, 200),
    };
    let mut wrong_position = base;
    wrong_position.accounts.position.market = id(9);
    assert_eq!(
        plan_position_replay_retirement(wrong_position),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_owner = base;
    wrong_owner.accounts.position.owner = id(9);
    assert_eq!(
        plan_position_replay_retirement(wrong_owner),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_replay = base;
    wrong_replay.accounts.replay.market = id(9);
    assert_eq!(
        plan_position_replay_retirement(wrong_replay),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_replay_owner = base;
    wrong_replay_owner.accounts.replay.owner = id(9);
    assert_eq!(
        plan_position_replay_retirement(wrong_replay_owner),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_replay_generation = base;
    wrong_replay_generation.accounts.replay.position_generation = 9;
    assert_eq!(
        plan_position_replay_retirement(wrong_replay_generation),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut substituted_sink = base;
    substituted_sink.neutral_sink = id(249);
    assert_eq!(
        plan_position_replay_retirement(substituted_sink),
        Err(RetirementErrorV2::WrongNeutralSink)
    );

    assert_eq!(
        plan_position_replay_retirement(PositionReplayRetirementRequestV1 {
            position: PositionLifecycleStateV2::Live(position()),
            replay: replay(position()),
            economic: PositionEconomicStateV1::ZERO,
            position_balance: 29,
            replay_balance: 20,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: PositionReplayAccountsV1 {
                position: position_account(position(), 40),
                replay: replay_account(position(), 40),
            },
            recipient_balances: position_recipients(100, 200),
        }),
        Err(RetirementErrorV2::AccountAlias)
    );
    assert_eq!(
        plan_position_replay_retirement(PositionReplayRetirementRequestV1 {
            position: PositionLifecycleStateV2::Live(position()),
            replay: replay(position()),
            economic: PositionEconomicStateV1::ZERO,
            position_balance: 29,
            replay_balance: 20,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: PositionReplayAccountsV1 {
                position: position_account(position(), 40),
                replay: replay_account(position(), 41),
            },
            recipient_balances: RecipientBalanceBookV1 {
                entries: [
                    Some(RecipientBalanceV1 {
                        recipient: id(40),
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
        }),
        Err(RetirementErrorV2::AccountAlias)
    );
    assert_eq!(
        plan_position_replay_retirement(PositionReplayRetirementRequestV1 {
            position: PositionLifecycleStateV2::Live(position()),
            replay: ReplayLifecycleStateV1::Absent,
            economic: PositionEconomicStateV1::ZERO,
            position_balance: 29,
            replay_balance: 0,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: PositionReplayAccountsV1 {
                position: position_account(position(), 40),
                replay: replay_account(position(), 41),
            },
            recipient_balances: position_recipients(100, 200),
        }),
        Err(RetirementErrorV2::ReplayMismatch)
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PositionReplayBank {
    position: PositionLifecycleStateV2,
    replay: ReplayLifecycleStateV1,
    position_balance: u64,
    replay_balance: u64,
    payer_balance: u64,
    sink_balance: u64,
}

fn staged_position_replay_close(
    bank: &mut PositionReplayBank,
    fail_after_write: Option<u8>,
) -> Result<(), ()> {
    let live = match bank.position {
        PositionLifecycleStateV2::Live(live) => live,
        PositionLifecycleStateV2::Tombstone(_) => return Err(()),
    };
    let plan = position_plan(
        live,
        position_recipients(bank.payer_balance, bank.sink_balance),
    )
    .map_err(|_| ())?;
    let mut staged = *bank;
    staged.position = plan.position_post_state;
    if fail_after_write == Some(1) {
        return Err(());
    }
    staged.replay = plan.replay_post_state;
    if fail_after_write == Some(2) {
        return Err(());
    }
    staged.payer_balance = plan.recipient_credits.get(id(3)).ok_or(())?.balance_after;
    if fail_after_write == Some(3) {
        return Err(());
    }
    staged.sink_balance = plan.recipient_credits.get(id(250)).ok_or(())?.balance_after;
    if fail_after_write == Some(4) {
        return Err(());
    }
    staged.position_balance = plan.position_balance_after;
    staged.replay_balance = plan.replay_balance_after;
    if fail_after_write == Some(5) {
        return Err(());
    }
    *bank = staged;
    Ok(())
}

#[test]
fn atomic_position_replay_plan_has_byte_identical_modeled_rollback() {
    let original = PositionReplayBank {
        position: PositionLifecycleStateV2::Live(position()),
        replay: replay(position()),
        position_balance: 29,
        replay_balance: 20,
        payer_balance: 100,
        sink_balance: 200,
    };
    for fail_after in 1u8..=5 {
        let mut bank = original;
        assert_eq!(
            staged_position_replay_close(&mut bank, Some(fail_after)),
            Err(())
        );
        assert_eq!(bank, original);
    }
    let mut committed = original;
    staged_position_replay_close(&mut committed, None).unwrap();
    assert_eq!(committed.position_balance, 7);
    assert_eq!(committed.replay_balance, 0);
    assert_eq!(committed.payer_balance, 124);
    assert_eq!(committed.sink_balance, 218);
}

#[test]
fn epoch_root_refuses_without_authoritative_budget_disposition() {
    let mut terminal = epoch();
    terminal.phase = GeneralEpochPhaseV2::Lapsed;
    let window = EpochWindowRootSiblingV1 {
        market: terminal.market,
        epoch: terminal.epoch,
        epoch_generation: 1,
        rent: deletable(3, 13, 3),
    };
    let budget = EpochBudgetRootSiblingV1 {
        market: terminal.market,
        epoch: terminal.epoch,
        epoch_generation: 1,
        rent: deletable(8, 17, 2),
    };
    let recipients = RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(3),
                balance_before: 100,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(8),
                balance_before: 200,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: 300,
            }),
            None,
        ],
    };
    let accounts = EpochRootAccountsV1 {
        epoch: epoch_account(terminal, 40),
        window: id(41),
        budget: id(42),
    };
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::BudgetRetirementUnauthenticated)
    );

    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(249),
            neutral_sink_binding: neutral_sink_binding(),
            accounts,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::WrongNeutralSink)
    );

    let wrong_window_market = EpochWindowRootSiblingV1 {
        market: id(9),
        ..window
    };
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window: wrong_window_market,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_market = accounts;
    wrong_epoch_market.epoch.market = id(9);
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: wrong_epoch_market,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_index = accounts;
    wrong_epoch_index.epoch.epoch_index = terminal.epoch_index + 1;
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: wrong_epoch_index,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::WrongParent)
    );
    let wrong_budget_epoch = EpochBudgetRootSiblingV1 {
        epoch: id(9),
        ..budget
    };
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget: wrong_budget_epoch,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::WrongParent)
    );

    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window: EpochWindowRootSiblingV1 {
                epoch_generation: 2,
                ..window
            },
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::WrongGeneration)
    );

    let mut wrong_identity = accounts;
    wrong_identity.epoch.epoch = id(40);
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: wrong_identity,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::WrongParent)
    );

    let mut source_alias = accounts;
    source_alias.budget = source_alias.window;
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: source_alias,
            recipient_balances: recipients,
        }),
        Err(RetirementErrorV2::AccountAlias)
    );

    let mut aliased_recipient = recipients;
    aliased_recipient.entries[3] = Some(RecipientBalanceV1 {
        recipient: accounts.epoch.account,
        balance_before: 400,
    });
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
            window,
            admission_ledger: admission_ledger(terminal),
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts,
            recipient_balances: aliased_recipient,
        }),
        Err(RetirementErrorV2::AccountAlias)
    );
}

#[test]
fn epoch_root_v2_pays_five_distinct_recipients_and_closes_atomically() {
    let mut terminal = epoch();
    terminal.phase = GeneralEpochPhaseV2::Settled;
    let window = EpochWindowRootSiblingV1 {
        market: terminal.market,
        epoch: terminal.epoch,
        epoch_generation: terminal.retirement.epoch_generation,
        rent: deletable(5, 13, 3),
    };
    let budget = AuthenticatedEpochBudgetDispositionV1::after_semantic_owner_validation(
        id(42),
        terminal.market,
        terminal.epoch,
        terminal.retirement.epoch_generation,
        id(250),
        id(8),
        deletable(6, 17, 2),
        13,
    )
    .unwrap();
    let plan = plan_epoch_root_retirement_v2(EpochRootRetirementRequestV2 {
        epoch: GeneralEpochLifecycleProjectionV2::Live(terminal),
        window,
        admission_ledger: admission_ledger(terminal),
        budget,
        reward_recipient: id(7),
        epoch_balance: 29,
        window_balance: 20,
        budget_balance: 35,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        accounts: EpochRootAccountsV1 {
            epoch: epoch_account(terminal, 40),
            window: id(41),
            budget: id(42),
        },
        recipient_balances: EpochRootRecipientBalanceBookV2 {
            entries: [
                Some(RecipientBalanceV1 {
                    recipient: id(3),
                    balance_before: 100,
                }),
                Some(RecipientBalanceV1 {
                    recipient: id(5),
                    balance_before: 200,
                }),
                Some(RecipientBalanceV1 {
                    recipient: id(6),
                    balance_before: 300,
                }),
                Some(RecipientBalanceV1 {
                    recipient: id(7),
                    balance_before: 400,
                }),
                Some(RecipientBalanceV1 {
                    recipient: id(250),
                    balance_before: 500,
                }),
            ],
        },
    })
    .unwrap();
    assert!(matches!(
        plan.epoch_post_state,
        GeneralEpochLifecycleProjectionV2::Tombstone(_)
    ));
    assert_eq!(plan.epoch_balance_after, 7);
    assert_eq!(plan.window_balance_after, 0);
    assert_eq!(plan.budget_balance_after, 0);
    assert_eq!(plan.root_close_reward_lamports, 13);
    assert_eq!(plan.recipient_credits.get(id(3)).unwrap().credit_lamports, 11);
    assert_eq!(plan.recipient_credits.get(id(5)).unwrap().credit_lamports, 13);
    assert_eq!(plan.recipient_credits.get(id(6)).unwrap().credit_lamports, 17);
    assert_eq!(plan.recipient_credits.get(id(7)).unwrap().credit_lamports, 13);
    assert_eq!(plan.recipient_credits.get(id(250)).unwrap().credit_lamports, 23);
    assert_eq!(11 + 13 + 17 + 13 + 23 + 7, 84);
    assert_eq!(29 + 20 + 35, 84);
}

#[test]
fn candidate_window_retirement_witness_is_terminal_bound_and_generation_scoped() {
    let mut terminal = epoch();
    terminal.phase = GeneralEpochPhaseV2::Lapsed;

    let mut unfinalized = retired_candidate_window(terminal);
    unfinalized.finalized_slot = 0;
    assert_eq!(
        ValidatedAdmissionLedgerRetiredV1::from_candidate_window(unfinalized, terminal,),
        Err(RetirementErrorV2::AdmissionLedgerOutstanding)
    );

    let mut live_head = retired_candidate_window(terminal);
    live_head.admitted_count = 1;
    live_head.live_node_count = 1;
    live_head.admission_head = CandidateId::from_bytes(id(20).bytes());
    assert_eq!(
        ValidatedAdmissionLedgerRetiredV1::from_candidate_window(live_head, terminal,),
        Err(RetirementErrorV2::AdmissionLedgerOutstanding)
    );

    let mut mismatched_counts = retired_candidate_window(terminal);
    mismatched_counts.admitted_count = 1;
    assert_eq!(
        ValidatedAdmissionLedgerRetiredV1::from_candidate_window(mismatched_counts, terminal,),
        Err(RetirementErrorV2::NonCanonicalState)
    );

    let mut wrong_market = retired_candidate_window(terminal);
    wrong_market.market = CandidateId::from_bytes(id(9).bytes());
    assert_eq!(
        ValidatedAdmissionLedgerRetiredV1::from_candidate_window(wrong_market, terminal,),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch = retired_candidate_window(terminal);
    wrong_epoch.epoch = CandidateId::from_bytes(id(9).bytes());
    assert_eq!(
        ValidatedAdmissionLedgerRetiredV1::from_candidate_window(wrong_epoch, terminal,),
        Err(RetirementErrorV2::WrongParent)
    );

    let generation_one = admission_ledger(terminal);
    let mut generation_two = terminal;
    generation_two.epoch_index = 1;
    generation_two.retirement.epoch_generation = 2;
    let window = EpochWindowRootSiblingV1 {
        market: generation_two.market,
        epoch: generation_two.epoch,
        epoch_generation: 2,
        rent: deletable(3, 13, 3),
    };
    let budget = EpochBudgetRootSiblingV1 {
        market: generation_two.market,
        epoch: generation_two.epoch,
        epoch_generation: 2,
        rent: deletable(8, 17, 2),
    };
    assert_eq!(
        plan_epoch_root_retirement(EpochRootRetirementRequestV1 {
            epoch: GeneralEpochLifecycleProjectionV2::Live(generation_two),
            window,
            admission_ledger: generation_one,
            budget,
            epoch_balance: 29,
            window_balance: 20,
            budget_balance: 24,
            neutral_sink: id(250),
            neutral_sink_binding: neutral_sink_binding(),
            accounts: EpochRootAccountsV1 {
                epoch: epoch_account(generation_two, 40),
                window: id(41),
                budget: id(42),
            },
            recipient_balances: RecipientBalanceBookV1 {
                entries: [
                    Some(RecipientBalanceV1 {
                        recipient: id(3),
                        balance_before: 100,
                    }),
                    Some(RecipientBalanceV1 {
                        recipient: id(8),
                        balance_before: 200,
                    }),
                    Some(RecipientBalanceV1 {
                        recipient: id(250),
                        balance_before: 300,
                    }),
                    None,
                ],
            },
        }),
        Err(RetirementErrorV2::AdmissionLedgerOutstanding)
    );
}

#[test]
fn reservation_close_plans_route_donation_and_refuse_caller_chosen_generation() {
    let initial_position = position();
    let initial_epoch = epoch();
    let (general_position, mut general_epoch, general, archive) = register_general_reservation_v2(
        initial_position,
        initial_epoch,
        funding(45, 3, 13, 3),
        id(250),
        neutral_sink_binding(),
        general_registration_accounts(initial_position, initial_epoch),
    )
    .unwrap();
    let mut wrong_owner = general;
    wrong_owner.owner = id(9);
    assert_eq!(
        terminate_reservation_v2(general_position, wrong_owner, ReservationStateV1::Released,),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_market = general;
    wrong_market.market = id(9);
    assert_eq!(
        terminate_reservation_v2(general_position, wrong_market, ReservationStateV1::Released,),
        Err(RetirementErrorV2::WrongParent)
    );
    let (_, general) =
        terminate_reservation_v2(general_position, general, ReservationStateV1::Released).unwrap();
    general_epoch.phase = GeneralEpochPhaseV2::Lapsed;
    let mut wrong_archive_parent = general;
    wrong_archive_parent.epoch = id(9);
    assert_eq!(
        general_reservation_close(general_epoch, archive, wrong_archive_parent),
        Err(RetirementErrorV2::WrongParent)
    );
    wrong_archive_parent = general;
    wrong_archive_parent.market = id(9);
    assert_eq!(
        general_reservation_close(general_epoch, archive, wrong_archive_parent),
        Err(RetirementErrorV2::WrongParent)
    );
    let general_plan = general_reservation_close(general_epoch, archive, general).unwrap();
    assert_eq!(
        general_plan
            .rent_close
            .recipient_credits
            .get(id(3))
            .unwrap()
            .balance_after,
        113
    );
    assert_eq!(
        general_plan
            .rent_close
            .recipient_credits
            .get(id(250))
            .unwrap()
            .balance_after,
        207
    );
    assert_eq!(general_plan.rent_close.account_balance_after, 0);

    let source_alias_request = GeneralReservationCloseRequestV1 {
        epoch: general_epoch,
        epoch_account: epoch_account(general_epoch, 41),
        slot: archive,
        reservation: general,
        reservation_balance: 20,
        neutral_sink: id(250),
        neutral_sink_binding: neutral_sink_binding(),
        reservation_account: id(41),
        recipient_balances: reservation_recipients(),
    };
    assert_eq!(
        plan_general_reservation_close(source_alias_request),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut source_recipient_alias = source_alias_request;
    source_recipient_alias.reservation_account = id(45);
    source_recipient_alias.recipient_balances.entries[2] = Some(RecipientBalanceV1 {
        recipient: source_recipient_alias.epoch_account.account,
        balance_before: 300,
    });
    assert_eq!(
        plan_general_reservation_close(source_recipient_alias),
        Err(RetirementErrorV2::AccountAlias)
    );

    let mut epoch_is_payer = source_alias_request;
    epoch_is_payer.reservation_account = id(45);
    epoch_is_payer.epoch_account.account = id(3);
    assert_eq!(
        plan_general_reservation_close(epoch_is_payer),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut epoch_is_sink = source_alias_request;
    epoch_is_sink.reservation_account = id(45);
    epoch_is_sink.epoch_account.account = id(250);
    assert_eq!(
        plan_general_reservation_close(epoch_is_sink),
        Err(RetirementErrorV2::AccountAlias)
    );
    let mut wrong_epoch_binding = source_alias_request;
    wrong_epoch_binding.reservation_account = id(45);
    wrong_epoch_binding.epoch_account.epoch = id(9);
    assert_eq!(
        plan_general_reservation_close(wrong_epoch_binding),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_market = source_alias_request;
    wrong_epoch_market.reservation_account = id(45);
    wrong_epoch_market.epoch_account.market = id(9);
    assert_eq!(
        plan_general_reservation_close(wrong_epoch_market),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut wrong_epoch_index = source_alias_request;
    wrong_epoch_index.reservation_account = id(45);
    wrong_epoch_index.epoch_account.epoch_index = 9;
    assert_eq!(
        plan_general_reservation_close(wrong_epoch_index),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut substituted_sink = source_alias_request;
    substituted_sink.reservation_account = id(45);
    substituted_sink.neutral_sink = id(249);
    assert_eq!(
        plan_general_reservation_close(substituted_sink),
        Err(RetirementErrorV2::WrongNeutralSink)
    );

    let direct_initial = position();
    let (direct_position, direct) = register_direct_reservation_v2(
        direct_initial,
        direct_epoch(90),
        funding(45, 3, 13, 3),
        id(250),
        neutral_sink_binding(),
        direct_registration_accounts(direct_initial),
    )
    .unwrap();
    assert_eq!(direct.count.epoch_generation, 91);
    let (_, direct) =
        terminate_reservation_v2(direct_position, direct, ReservationStateV1::Consumed).unwrap();
    assert!(direct_reservation_close(direct_epoch(90), direct).is_ok());
    assert_eq!(
        direct_reservation_close(direct_epoch(91), direct),
        Err(RetirementErrorV2::WrongGeneration)
    );
    assert_eq!(
        direct_reservation_close(
            AdapterDirectEpochProjectionV1 {
                epoch: id(9),
                ..direct_epoch(90)
            },
            direct,
        ),
        Err(RetirementErrorV2::WrongParent)
    );
    assert_eq!(
        plan_direct_reservation_close(DirectReservationCloseRequestV1 {
            direct_epoch: AdapterDirectEpochProjectionV1 {
                market: id(9),
                ..direct_epoch(90)
            },
            reservation: direct,
            reservation_balance: 20,
            neutral_sink: id(250),
            neutral_sink_binding: AdapterNeutralSinkBindingProjectionV1 {
                market: id(9),
                neutral_sink: id(250),
            },
            reservation_account: id(45),
            recipient_balances: reservation_recipients(),
        }),
        Err(RetirementErrorV2::WrongParent)
    );
    let mut direct_close = DirectReservationCloseRequestV1 {
        direct_epoch: direct_epoch(90),
        reservation: direct,
        reservation_balance: 20,
        neutral_sink: id(249),
        neutral_sink_binding: neutral_sink_binding(),
        reservation_account: id(45),
        recipient_balances: reservation_recipients(),
    };
    assert_eq!(
        plan_direct_reservation_close(direct_close),
        Err(RetirementErrorV2::WrongNeutralSink)
    );
    direct_close.neutral_sink = id(250);
    direct_close.direct_epoch.epoch_index = 91;
    assert_eq!(
        plan_direct_reservation_close(direct_close),
        Err(RetirementErrorV2::WrongGeneration)
    );
    assert_eq!(
        register_direct_reservation_v2(
            position(),
            direct_epoch(u64::MAX),
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            direct_registration_accounts(position()),
        ),
        Err(RetirementErrorV2::EpochIndexExhausted)
    );
    assert_eq!(
        register_direct_reservation_v2(
            position(),
            AdapterDirectEpochProjectionV1 {
                market: id(9),
                ..direct_epoch(90)
            },
            funding(45, 3, 13, 3),
            id(250),
            neutral_sink_binding(),
            direct_registration_accounts(position()),
        ),
        Err(RetirementErrorV2::WrongParent)
    );

    let poisoned = admit_deletable_rent(id(45), id(250), 13, 3, 100, id(249)).unwrap();
    assert_eq!(
        register_general_reservation_v2(
            position(),
            epoch(),
            poisoned,
            id(250),
            neutral_sink_binding(),
            general_registration_accounts(position(), epoch()),
        ),
        Err(RetirementErrorV2::WrongNeutralSink)
    );
    assert_eq!(
        register_direct_reservation_v2(
            position(),
            direct_epoch(90),
            poisoned,
            id(250),
            neutral_sink_binding(),
            direct_registration_accounts(position()),
        ),
        Err(RetirementErrorV2::WrongNeutralSink)
    );
}

#[test]
fn root_open_is_blocked_before_budget_owner_integration() {
    let cursor = MarketEpochCursorV1 {
        next_general_epoch_index: 7,
    };
    assert_eq!(
        open_plan(cursor, 7),
        Err(RetirementErrorV2::BudgetFundingUnauthenticated)
    );
}
