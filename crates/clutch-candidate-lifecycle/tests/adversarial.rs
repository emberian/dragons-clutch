// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_candidate_lifecycle::*;

const F: u64 = 100;
const S: u64 = 110;
const V: u64 = 130;

fn id(byte: u8) -> Id {
    Id::from_bytes([byte; 32])
}

fn lifecycle(max_candidates: u16) -> CandidateLifecyclePolicyV2 {
    CandidateLifecyclePolicyV2 {
        policy_id: id(4),
        submission_span_slots: S - F,
        verification_span_slots: V - S,
        max_feed_bytes: 1_024,
        max_begun_candidates: max_candidates,
        max_verification_units: 3,
        stored_bump: 1,
        flags: 0,
    }
}

fn score() -> ScorePolicyBindingV1 {
    ScorePolicyBindingV1 {
        policy_id: id(5),
        rank_key_len: 33,
    }
}

fn liveness() -> CandidateLivenessPolicyV2 {
    CandidateLivenessPolicyV2 {
        policy_id: id(6),
        neutral_sink: id(7),
        progress_reward_per_unit: 2,
        completion_reward: 5,
        expiry_reward: 7,
        candidate_close_reward: 11,
        freeze_reward: 13,
        finalizer_reward: 17,
        index_page_close_reward: 19,
        bond_lamports: 100,
        invalidity_penalty: 30,
        abandonment_penalty: 20,
        solver_prize: 23,
        stored_bump: 1,
        flags: 0,
    }
}

fn rank(candidate: Id, score_byte: u8) -> RankKey {
    let mut bytes = [0u8; RANK_KEY_CAPACITY];
    bytes[0] = score_byte;
    let candidate_bytes = candidate.bytes();
    let mut index = 0usize;
    while index < 32 {
        bytes[index + 1] = !candidate_bytes[index];
        index += 1;
    }
    RankKey::new(33, bytes).unwrap()
}

fn open(max_candidates: u16) -> CandidateWindowV3 {
    CandidateWindowV3::open(
        id(1),
        id(2),
        id(3),
        lifecycle(max_candidates),
        score(),
        liveness(),
        F,
        1,
    )
    .unwrap()
}

fn budget() -> EpochCandidateBudgetV2 {
    admit_epoch_budget(
        EpochBudgetAdmissionV2 {
            epoch: id(1),
            sponsor: id(8),
            refund_destination: id(9),
            account_rent_principal: 1_000,
            index_page_rent_principal: 2_000,
            budget_bump: 1,
        },
        liveness(),
    )
    .unwrap()
}

fn with_all_index_pages_closed(mut budget: EpochCandidateBudgetV2) -> EpochCandidateBudgetV2 {
    budget.index_cleanup_remaining = 0;
    budget.index_cleanup_paid = budget.index_cleanup_initial;
    budget.index_pages_owed = 0;
    budget
}

fn frozen(max_candidates: u16) -> (CandidateWindowV3, EpochCandidateBudgetV2) {
    let frozen = freeze_window(
        open(max_candidates),
        budget(),
        lifecycle(max_candidates),
        score(),
        liveness(),
        F,
    )
    .unwrap();
    assert_eq!(frozen.disposition.keeper_reward, 13);
    (frozen.window, frozen.budget)
}

fn begin_input(candidate_byte: u8) -> BeginCandidateInputV2 {
    BeginCandidateInputV2 {
        candidate: id(candidate_byte),
        solver: id(candidate_byte.wrapping_add(40)),
        solver_reward_destination: id(candidate_byte.wrapping_add(80)),
        feed: id(candidate_byte.wrapping_add(120)),
        payer: id(candidate_byte.wrapping_add(160)),
        refund_destination: id(candidate_byte.wrapping_add(200)),
        expected_feed_bytes: 256,
        verification_units: 2,
        staging_rent_principal: 700,
        candidate_bump: 1,
        escrow_bump: 1,
    }
}

fn begin_one(
    window: CandidateWindowV3,
    candidate_byte: u8,
    slot: u64,
) -> BeginCandidateTransitionV2 {
    begin_candidate(
        window,
        CandidateIndexPageV1::empty(id(1), 0, 1).unwrap(),
        begin_input(candidate_byte),
        lifecycle(4),
        score(),
        liveness(),
        slot,
    )
    .unwrap()
}

fn seal_one(
    window: CandidateWindowV3,
    index_page: CandidateIndexPageV1,
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
    slot: u64,
) -> SealCandidateTransitionV2 {
    seal_candidate(
        window,
        index_page,
        candidate,
        escrow,
        FeedSealV1 {
            candidate: candidate.candidate,
            epoch: candidate.epoch,
            feed: candidate.feed,
            content_digest: id(250),
            exact_bytes: candidate.expected_feed_bytes,
            written_bytes: candidate.expected_feed_bytes,
            canonical_padding: 1,
        },
        SealFundingV1 {
            verification_rent_principal: 900,
            work_reward_deposit: 9,
        },
        lifecycle(4),
        score(),
        liveness(),
        slot,
    )
    .unwrap()
}

fn finish_progress(
    window: CandidateWindowV3,
    candidate: CandidateRecordV2,
    escrow: CandidateEscrowV2,
) -> CandidateEscrowV2 {
    pay_verification_progress(window, candidate, escrow, liveness(), 0, 2, S)
        .unwrap()
        .escrow
}

#[test]
fn half_open_boundaries_are_exclusive_and_freeze_is_not_an_expiring_gate() {
    assert_eq!(
        freeze_window(open(4), budget(), lifecycle(4), score(), liveness(), F - 1),
        Err(Error::NotActive)
    );
    let (window, _) = frozen(4);
    let begun = begin_one(window, 20, F);
    let refused = begin_candidate(
        begun.window,
        begun.index_page,
        begin_input(21),
        lifecycle(4),
        score(),
        liveness(),
        S,
    );
    assert_eq!(refused, Err(Error::NotActive));

    let sealed = seal_one(
        begun.window,
        begun.index_page,
        begun.candidate,
        begun.escrow,
        S - 1,
    );
    let at_s = pay_verification_progress(
        sealed.window,
        sealed.candidate,
        sealed.escrow,
        liveness(),
        0,
        1,
        S,
    )
    .unwrap();
    assert_eq!(at_s.disposition.keeper_reward, 2);
    assert_eq!(
        pay_verification_progress(
            sealed.window,
            sealed.candidate,
            sealed.escrow,
            liveness(),
            0,
            1,
            V
        ),
        Err(Error::NotActive)
    );
}

#[test]
fn begin_is_bounded_and_enumerated_before_seal() {
    let (window, _) = frozen(2);
    let first = begin_one(window, 20, F);
    assert_eq!(first.window.begun_candidate_count, 1);
    assert_eq!(first.window.sealed_candidate_count, 0);
    assert_eq!(first.index_page.candidates[0], id(20));
    let second = begin_candidate(
        first.window,
        first.index_page,
        begin_input(21),
        lifecycle(2),
        score(),
        liveness(),
        F + 1,
    )
    .unwrap();
    assert_eq!(second.index_page.candidates[1], id(21));
    let snapshot = second;
    assert_eq!(
        begin_candidate(
            second.window,
            second.index_page,
            begin_input(22),
            lifecycle(2),
            score(),
            liveness(),
            F + 2
        ),
        Err(Error::CapacityReached)
    );
    assert_eq!(second, snapshot);
}

#[test]
fn seal_requires_exact_complete_feed_and_exact_present_work_funding() {
    let (window, _) = frozen(4);
    let begun = begin_one(window, 20, F);
    let mut wrong_page = begun.index_page;
    wrong_page.candidates[0] = id(21);
    assert_eq!(
        seal_candidate(
            begun.window,
            wrong_page,
            begun.candidate,
            begun.escrow,
            FeedSealV1 {
                candidate: begun.candidate.candidate,
                epoch: begun.candidate.epoch,
                feed: begun.candidate.feed,
                content_digest: id(250),
                exact_bytes: 256,
                written_bytes: 256,
                canonical_padding: 1,
            },
            SealFundingV1 {
                verification_rent_principal: 900,
                work_reward_deposit: 9,
            },
            lifecycle(4),
            score(),
            liveness(),
            F
        ),
        Err(Error::MismatchedBinding)
    );
    let mut incomplete = FeedSealV1 {
        candidate: begun.candidate.candidate,
        epoch: begun.candidate.epoch,
        feed: begun.candidate.feed,
        content_digest: id(250),
        exact_bytes: 256,
        written_bytes: 255,
        canonical_padding: 1,
    };
    assert_eq!(
        seal_candidate(
            begun.window,
            begun.index_page,
            begun.candidate,
            begun.escrow,
            incomplete,
            SealFundingV1 {
                verification_rent_principal: 900,
                work_reward_deposit: 9,
            },
            lifecycle(4),
            score(),
            liveness(),
            F
        ),
        Err(Error::MismatchedBinding)
    );
    incomplete.written_bytes = 256;
    assert_eq!(
        seal_candidate(
            begun.window,
            begun.index_page,
            begun.candidate,
            begun.escrow,
            incomplete,
            SealFundingV1 {
                verification_rent_principal: 900,
                work_reward_deposit: 8,
            },
            lifecycle(4),
            score(),
            liveness(),
            F
        ),
        Err(Error::Underfunded)
    );
    assert_eq!(begun.candidate.status, CandidateStatus::Staging);
    assert_eq!(begun.escrow.work_initial, 0);
}

#[test]
fn valid_completion_early_finalization_and_split_claims_conserve_funds() {
    let (window, budget) = frozen(4);
    let begun = begin_one(window, 20, F);
    let sealed = seal_one(
        begun.window,
        begun.index_page,
        begun.candidate,
        begun.escrow,
        F + 1,
    );
    let escrow = finish_progress(sealed.window, sealed.candidate, sealed.escrow);
    let completed = complete_verification(
        sealed.window,
        sealed.candidate,
        escrow,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        AdapterVerifiedOutcomeV1 {
            verdict: id(30),
            relation_digest: id(31),
            kind: VerdictKind::Valid,
            refusal_code: 0,
            rank_key: rank(id(20), 9),
            verdict_bump: 1,
        },
        liveness(),
        S,
    )
    .unwrap();
    assert_eq!(
        completed.window.top_candidates,
        [id(20), Id::ZERO, Id::ZERO]
    );
    assert_eq!(completed.disposition.keeper_reward, 5);
    let finalized = finalize_selection(
        completed.window,
        budget,
        [
            completed.verdict,
            CandidateVerdictV1::EMPTY,
            CandidateVerdictV1::EMPTY,
        ],
        Some(WinnerFundingV2 {
            candidate: completed.candidate,
            escrow: completed.escrow,
        }),
        liveness(),
        S + 1,
    )
    .unwrap();
    assert_eq!(finalized.window.selected_candidate, id(20));
    assert_eq!(finalized.disposition.solver_escrow_credit, 23);

    let mut selected_but_uncredited = with_all_index_pages_closed(finalized.budget);
    selected_but_uncredited.solver_remaining = selected_but_uncredited.solver_initial;
    selected_but_uncredited.solver_credited = 0;
    assert_eq!(selected_but_uncredited.validate(), Ok(()));
    assert_eq!(
        claim_epoch_unused(
            finalized.window,
            selected_but_uncredited,
            liveness(),
            selected_but_uncredited.accounted_lamports().unwrap(),
        ),
        Err(Error::MismatchedBinding)
    );

    let bond = claim_bond_refund(
        finalized.window,
        completed.candidate,
        Some(completed.verdict),
        finalized.winner_escrow.unwrap(),
        liveness(),
    )
    .unwrap();
    assert_eq!(bond.disposition.refund_destination_credit, 100);
    let closed_work = mark_work_closed(completed.candidate, bond.escrow, liveness(), 2).unwrap();
    let work = claim_work_refund(completed.candidate, closed_work, liveness()).unwrap();
    assert_eq!(work.disposition.refund_destination_credit, 0);
    let settlement = Some(AdapterVerifiedSettlementTerminalV1 {
        epoch: finalized.window.epoch,
        candidate: completed.candidate.candidate,
        terminal_slot: finalized.window.finalized_slot,
        flags: 0,
    });
    let mut selected_without_credit = work.escrow;
    selected_without_credit.solver_credited = 0;
    selected_without_credit.solver_remaining = 0;
    assert_eq!(selected_without_credit.validate(), Ok(()));
    assert_eq!(
        finish_candidate_cleanup(
            finalized.window,
            begun.index_page,
            completed.candidate,
            Some(completed.verdict),
            settlement,
            selected_without_credit,
            liveness(),
            selected_without_credit.accounted_lamports().unwrap(),
        ),
        Err(Error::MismatchedBinding)
    );
    let solver = claim_solver_credit(
        finalized.window,
        completed.candidate,
        completed.verdict,
        work.escrow,
        liveness(),
    )
    .unwrap();
    assert_eq!(solver.disposition.solver_payout, 23);
    let forged_refused = CandidateVerdictV1 {
        rank_key: RankKey::EMPTY,
        refusal_code: 7,
        kind: VerdictKind::Refused,
        ..completed.verdict
    };
    let mut selected_with_refused_verdict = solver.escrow;
    selected_with_refused_verdict.bond_slashed = liveness().invalidity_penalty;
    selected_with_refused_verdict.bond_refunded =
        liveness().bond_lamports - liveness().invalidity_penalty;
    assert_eq!(selected_with_refused_verdict.validate(), Ok(()));
    assert_eq!(
        finish_candidate_cleanup(
            finalized.window,
            begun.index_page,
            completed.candidate,
            Some(forged_refused),
            settlement,
            selected_with_refused_verdict,
            liveness(),
            selected_with_refused_verdict.accounted_lamports().unwrap(),
        ),
        Err(Error::MismatchedBinding)
    );
    let expected_owned = solver.escrow.accounted_lamports().unwrap();
    assert_eq!(
        finish_candidate_cleanup(
            finalized.window,
            begun.index_page,
            completed.candidate,
            Some(completed.verdict),
            settlement,
            solver.escrow,
            liveness(),
            expected_owned - 1,
        ),
        Err(Error::Underfunded)
    );
    let cleanup = finish_candidate_cleanup(
        finalized.window,
        begun.index_page,
        completed.candidate,
        Some(completed.verdict),
        settlement,
        solver.escrow,
        liveness(),
        expected_owned + 3,
    )
    .unwrap();
    assert_eq!(cleanup.disposition.keeper_reward, 11);
    assert_eq!(cleanup.disposition.refund_destination_credit, 7);
    assert_eq!(cleanup.disposition.rent_principal_refund, 1_600);
    assert_eq!(cleanup.disposition.neutral_sink, 3);
    assert_eq!(cleanup.escrow.surplus_routed, 3);
    assert_eq!(cleanup.escrow.accounted_lamports().unwrap(), 0);
    assert_eq!(cleanup.index_page.closed_mask, 1);
}

#[test]
fn refused_verdict_is_the_only_invalidity_slash_path() {
    let (window, budget) = frozen(4);
    let begun = begin_one(window, 20, F);
    let sealed = seal_one(
        begun.window,
        begun.index_page,
        begun.candidate,
        begun.escrow,
        F + 1,
    );
    let escrow = finish_progress(sealed.window, sealed.candidate, sealed.escrow);
    let malformed = complete_verification(
        sealed.window,
        sealed.candidate,
        escrow,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        AdapterVerifiedOutcomeV1 {
            verdict: id(30),
            relation_digest: id(31),
            kind: VerdictKind::Refused,
            refusal_code: 0,
            rank_key: RankKey::EMPTY,
            verdict_bump: 1,
        },
        liveness(),
        S,
    );
    assert_eq!(malformed, Err(Error::InvalidState));
    assert_eq!(escrow.bond_slashed, 0);

    let refused = complete_verification(
        sealed.window,
        sealed.candidate,
        escrow,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        AdapterVerifiedOutcomeV1 {
            verdict: id(30),
            relation_digest: id(31),
            kind: VerdictKind::Refused,
            refusal_code: 7,
            rank_key: RankKey::EMPTY,
            verdict_bump: 1,
        },
        liveness(),
        S,
    )
    .unwrap();
    assert_eq!(refused.escrow.bond_slashed, 30);
    assert_eq!(refused.escrow.bond_remaining, 70);
    assert_eq!(refused.disposition.neutral_sink, 30);
    assert_eq!(refused.window.valid_verdict_count, 0);
    let mut forged_penalty = refused.escrow;
    forged_penalty.bond_slashed = 31;
    forged_penalty.bond_remaining = 69;
    assert_eq!(
        claim_bond_refund(
            refused.window,
            refused.candidate,
            Some(refused.verdict),
            forged_penalty,
            liveness()
        ),
        Err(Error::MismatchedBinding)
    );
    let refund = claim_bond_refund(
        refused.window,
        refused.candidate,
        Some(refused.verdict),
        refused.escrow,
        liveness(),
    )
    .unwrap();
    assert_eq!(refund.disposition.refund_destination_credit, 70);

    let finalized = finalize_selection(
        refused.window,
        budget,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        None,
        liveness(),
        S + 1,
    )
    .unwrap();
    let mut lapsed_but_credited = with_all_index_pages_closed(finalized.budget);
    lapsed_but_credited.solver_remaining = 0;
    lapsed_but_credited.solver_credited = lapsed_but_credited.solver_initial;
    assert_eq!(lapsed_but_credited.validate(), Ok(()));
    assert_eq!(
        claim_epoch_unused(
            finalized.window,
            lapsed_but_credited,
            liveness(),
            lapsed_but_credited.accounted_lamports().unwrap(),
        ),
        Err(Error::MismatchedBinding)
    );

    let closed_work = mark_work_closed(refused.candidate, refund.escrow, liveness(), 2).unwrap();
    let work = claim_work_refund(refused.candidate, closed_work, liveness()).unwrap();
    let mut unselected_with_credit = work.escrow;
    unselected_with_credit.solver_credited = liveness().solver_prize;
    unselected_with_credit.solver_paid = liveness().solver_prize;
    unselected_with_credit.solver_credit_claimed = 1;
    assert_eq!(unselected_with_credit.validate(), Ok(()));
    assert_eq!(
        finish_candidate_cleanup(
            finalized.window,
            begun.index_page,
            refused.candidate,
            Some(refused.verdict),
            None,
            unselected_with_credit,
            liveness(),
            unselected_with_credit.accounted_lamports().unwrap(),
        ),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn hard_finalization_ignores_unverified_candidate_without_calling_it_invalid() {
    let (window, budget) = frozen(4);
    let begun = begin_one(window, 20, F);
    let sealed = seal_one(
        begun.window,
        begun.index_page,
        begun.candidate,
        begun.escrow,
        F + 1,
    );
    assert_eq!(
        finalize_selection(
            sealed.window,
            budget,
            [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
            None,
            liveness(),
            S
        ),
        Err(Error::UnresolvedCandidates)
    );
    let finalized = finalize_selection(
        sealed.window,
        budget,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        None,
        liveness(),
        V,
    )
    .unwrap();
    assert_eq!(finalized.window.selected_candidate, Id::ZERO);
    let expired = expire_candidate(
        finalized.window,
        begun.index_page,
        sealed.candidate,
        sealed.escrow,
        liveness(),
        V,
    )
    .unwrap();
    assert_eq!(expired.candidate.status, CandidateStatus::ExpiredUnverified);
    assert_eq!(expired.disposition.neutral_sink, 0);
    assert_eq!(expired.escrow.bond_remaining, 100);
}

#[test]
fn staging_expiry_slashes_abandonment_only_and_preserves_refundable_remainder() {
    let (window, budget) = frozen(4);
    let begun = begin_one(window, 20, F);
    let finalized = finalize_selection(
        begun.window,
        budget,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        None,
        liveness(),
        S,
    )
    .unwrap();
    let expired = expire_candidate(
        finalized.window,
        begun.index_page,
        begun.candidate,
        begun.escrow,
        liveness(),
        S,
    )
    .unwrap();
    assert_eq!(expired.disposition.neutral_sink, 20);
    assert_eq!(expired.escrow.bond_remaining, 80);
    assert_eq!(expired.disposition.keeper_reward, 7);
    let bond = claim_bond_refund(
        finalized.window,
        expired.candidate,
        None,
        expired.escrow,
        liveness(),
    )
    .unwrap();
    let cleaned = finish_candidate_cleanup(
        finalized.window,
        begun.index_page,
        expired.candidate,
        None,
        None,
        bond.escrow,
        liveness(),
        bond.escrow.accounted_lamports().unwrap(),
    )
    .unwrap();
    assert_eq!(cleaned.disposition.keeper_reward, 11);
    assert_eq!(cleaned.disposition.refund_destination_credit, 0);
    assert_eq!(cleaned.disposition.rent_principal_refund, 700);
}

#[test]
fn candidate_tie_break_is_local_injective_and_smaller_identity_wins() {
    let low = id(20);
    let high = id(21);
    assert_eq!(
        rank(low, 9).compare(rank(high, 9)).unwrap(),
        core::cmp::Ordering::Greater
    );
    assert_eq!(
        rank(high, 9).compare(rank(low, 9)).unwrap(),
        core::cmp::Ordering::Less
    );
    let mut wrong = [0u8; RANK_KEY_CAPACITY];
    wrong[0] = 9;
    assert_eq!(
        RankKey::new(33, wrong).unwrap().validate_for_candidate(low),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn index_pages_close_in_reverse_and_unused_epoch_funds_refund_afterward() {
    let (window, budget) = frozen(4);
    let finalized = finalize_selection(
        window,
        budget,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        None,
        liveness(),
        S,
    )
    .unwrap();
    assert_eq!(finalized.budget.index_pages_owed, 4);
    let wrong = CandidateIndexPageV1::empty(id(1), 0, 1).unwrap();
    assert_eq!(
        close_index_page(finalized.window, finalized.budget, wrong, liveness(), 500),
        Err(Error::MismatchedBinding)
    );
    assert_eq!(
        close_index_page(
            finalized.window,
            finalized.budget,
            CandidateIndexPageV1::empty(id(1), 3, 1).unwrap(),
            liveness(),
            499
        ),
        Err(Error::Underfunded)
    );
    let mut current = finalized.budget;
    for page_index in [3u8, 2, 1] {
        let observed = if page_index == 3 { 502 } else { 500 };
        let closed = close_index_page(
            finalized.window,
            current,
            CandidateIndexPageV1::empty(id(1), page_index, 1).unwrap(),
            liveness(),
            observed,
        )
        .unwrap();
        assert_eq!(closed.disposition.keeper_reward, 19);
        assert_eq!(closed.disposition.rent_principal_refund, 500);
        assert_eq!(closed.disposition.neutral_sink, observed - 500);
        current = closed.budget;
    }
    let unclosed = CandidateIndexPageV1 {
        epoch: id(1),
        page_index: 0,
        count: 1,
        closed_mask: 0,
        candidates: {
            let mut values = [Id::ZERO; CANDIDATES_PER_INDEX_PAGE];
            values[0] = id(20);
            values
        },
        stored_bump: 1,
        flags: 0,
    };
    assert_eq!(
        close_index_page(finalized.window, current, unclosed, liveness(), 500),
        Err(Error::MismatchedBinding)
    );
    let closed = close_index_page(
        finalized.window,
        current,
        CandidateIndexPageV1::empty(id(1), 0, 1).unwrap(),
        liveness(),
        500,
    )
    .unwrap();
    assert_eq!(closed.disposition.keeper_reward, 19);
    assert_eq!(closed.disposition.rent_principal_refund, 500);
    current = closed.budget;
    let refund = claim_epoch_unused(
        finalized.window,
        current,
        liveness(),
        current.accounted_lamports().unwrap() + 4,
    )
    .unwrap();
    assert_eq!(refund.disposition.refund_destination_credit, 23);
    assert_eq!(refund.disposition.neutral_sink, 4);
    assert_eq!(refund.budget.surplus_routed, 6);
    assert_eq!(refund.budget.accounted_lamports().unwrap(), 1_000);
    assert_eq!(
        claim_epoch_unused(
            finalized.window,
            refund.budget,
            liveness(),
            refund.budget.accounted_lamports().unwrap()
        ),
        Err(Error::Replay)
    );
}

#[test]
fn account_and_intent_codecs_refuse_trailing_bytes_and_wrong_versions() {
    let (window, _) = frozen(4);
    let mut bytes = [0u8; WINDOW_BYTES];
    window.encode(&mut bytes).unwrap();
    assert_eq!(CandidateWindowV3::decode(&bytes).unwrap(), window);
    assert_eq!(
        CandidateWindowV3::decode(&bytes[..WINDOW_BYTES - 1]),
        Err(CodecError::WrongLength)
    );
    bytes[1] = WINDOW_VERSION.wrapping_add(1);
    assert_eq!(
        CandidateWindowV3::decode(&bytes),
        Err(CodecError::WrongVersion)
    );

    let intent = CandidateIntentV2::Begin {
        epoch: id(1),
        candidate: id(20),
        solver: id(60),
        solver_reward_destination: id(100),
        feed: id(140),
        payer: id(180),
        refund_destination: id(220),
        expected_feed_bytes: 256,
        verification_units: 2,
    };
    let mut wire = [0u8; 233];
    assert_eq!(wire.len(), intent.encoded_len());
    intent.encode(&mut wire).unwrap();
    assert_eq!(CandidateIntentV2::decode(&wire).unwrap(), intent);
    wire[1] = 99;
    assert_eq!(
        CandidateIntentV2::decode(&wire),
        Err(CodecError::WrongVersion)
    );
}

#[test]
fn every_account_codec_round_trips_exact_versioned_state() {
    let lifecycle_value = lifecycle(4);
    let mut lifecycle_bytes = [0u8; LIFECYCLE_POLICY_BYTES];
    lifecycle_value.encode(&mut lifecycle_bytes).unwrap();
    assert_eq!(
        CandidateLifecyclePolicyV2::decode(&lifecycle_bytes).unwrap(),
        lifecycle_value
    );

    let liveness_value = liveness();
    let mut liveness_bytes = [0u8; LIVENESS_POLICY_BYTES];
    liveness_value.encode(&mut liveness_bytes).unwrap();
    assert_eq!(
        CandidateLivenessPolicyV2::decode(&liveness_bytes).unwrap(),
        liveness_value
    );

    let (window, budget_value) = frozen(4);
    let mut budget_bytes = [0u8; EPOCH_BUDGET_BYTES];
    budget_value.encode(&mut budget_bytes).unwrap();
    assert_eq!(
        EpochCandidateBudgetV2::decode(&budget_bytes).unwrap(),
        budget_value
    );

    let begun = begin_one(window, 20, F);
    let mut page_bytes = [0u8; INDEX_PAGE_BYTES];
    begun.index_page.encode(&mut page_bytes).unwrap();
    assert_eq!(
        CandidateIndexPageV1::decode(&page_bytes).unwrap(),
        begun.index_page
    );

    let sealed = seal_one(
        begun.window,
        begun.index_page,
        begun.candidate,
        begun.escrow,
        F + 1,
    );
    let escrow = finish_progress(sealed.window, sealed.candidate, sealed.escrow);
    let completed = complete_verification(
        sealed.window,
        sealed.candidate,
        escrow,
        [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
        AdapterVerifiedOutcomeV1 {
            verdict: id(30),
            relation_digest: id(31),
            kind: VerdictKind::Valid,
            refusal_code: 0,
            rank_key: rank(id(20), 9),
            verdict_bump: 1,
        },
        liveness(),
        S,
    )
    .unwrap();

    let mut candidate_bytes = [0u8; CANDIDATE_BYTES];
    completed.candidate.encode(&mut candidate_bytes).unwrap();
    assert_eq!(
        CandidateRecordV2::decode(&candidate_bytes).unwrap(),
        completed.candidate
    );
    let mut escrow_bytes = [0u8; ESCROW_BYTES];
    completed.escrow.encode(&mut escrow_bytes).unwrap();
    assert_eq!(
        CandidateEscrowV2::decode(&escrow_bytes).unwrap(),
        completed.escrow
    );
    let mut verdict_bytes = [0u8; VERDICT_BYTES];
    completed.verdict.encode(&mut verdict_bytes).unwrap();
    assert_eq!(
        CandidateVerdictV1::decode(&verdict_bytes).unwrap(),
        completed.verdict
    );
}

#[test]
fn every_intent_variant_round_trips_and_trailing_bytes_refuse() {
    let intents = [
        CandidateIntentV2::Freeze { epoch: id(1) },
        CandidateIntentV2::Begin {
            epoch: id(1),
            candidate: id(20),
            solver: id(60),
            solver_reward_destination: id(100),
            feed: id(140),
            payer: id(180),
            refund_destination: id(220),
            expected_feed_bytes: 256,
            verification_units: 2,
        },
        CandidateIntentV2::Seal {
            epoch: id(1),
            candidate: id(20),
            feed: id(140),
            content_digest: id(250),
            verification_rent_principal: 900,
            work_reward_deposit: 9,
        },
        CandidateIntentV2::Progress {
            epoch: id(1),
            candidate: id(20),
            prior_units: 0,
            new_units: 1,
        },
        CandidateIntentV2::Complete {
            epoch: id(1),
            candidate: id(20),
            expected_verdict: id(30),
        },
        CandidateIntentV2::Finalize { epoch: id(1) },
        CandidateIntentV2::Expire {
            epoch: id(1),
            candidate: id(20),
        },
        CandidateIntentV2::MarkWorkClosed {
            epoch: id(1),
            candidate: id(20),
            observed_paid_units: 2,
        },
        CandidateIntentV2::ClaimBond {
            epoch: id(1),
            candidate: id(20),
        },
        CandidateIntentV2::ClaimWork {
            epoch: id(1),
            candidate: id(20),
        },
        CandidateIntentV2::CleanupCandidate {
            epoch: id(1),
            candidate: id(20),
        },
        CandidateIntentV2::ClaimSolver {
            epoch: id(1),
            candidate: id(20),
        },
        CandidateIntentV2::CloseIndexPage {
            epoch: id(1),
            page_index: 3,
        },
        CandidateIntentV2::ClaimEpochUnused { epoch: id(1) },
    ];
    let mut buffer = [0u8; 234];
    for intent in intents {
        let length = intent.encoded_len();
        intent.encode(&mut buffer[..length]).unwrap();
        assert_eq!(
            CandidateIntentV2::decode(&buffer[..length]).unwrap(),
            intent
        );
        assert_eq!(
            CandidateIntentV2::decode(&buffer[..length + 1]),
            Err(CodecError::WrongLength)
        );
    }
}

#[test]
fn migration_is_new_epoch_only_and_mixed_score_policy_refuses() {
    let (window, _) = frozen(4);
    let begun = begin_one(window, 20, F);
    let wrong_score = ScorePolicyBindingV1 {
        policy_id: id(99),
        rank_key_len: 33,
    };
    assert_eq!(
        seal_candidate(
            begun.window,
            begun.index_page,
            begun.candidate,
            begun.escrow,
            FeedSealV1 {
                candidate: begun.candidate.candidate,
                epoch: begun.candidate.epoch,
                feed: begun.candidate.feed,
                content_digest: id(250),
                exact_bytes: 256,
                written_bytes: 256,
                canonical_padding: 1,
            },
            SealFundingV1 {
                verification_rent_principal: 900,
                work_reward_deposit: 9,
            },
            lifecycle(4),
            wrong_score,
            liveness(),
            F + 1
        ),
        Err(Error::MismatchedBinding)
    );
    assert_eq!(WINDOW_VERSION, 3);
    assert_eq!(CANDIDATE_VERSION, 2);
    assert_eq!(ESCROW_VERSION, 3);
    assert_eq!(EPOCH_BUDGET_VERSION, 3);
    assert_eq!(INTENT_VERSION, 2);
}

#[test]
fn seal_rechecks_hostile_decoded_candidate_geometry() {
    let (window, _) = frozen(4);
    let begun = begin_one(window, 20, F);
    let mut oversized = begun.candidate;
    oversized.expected_feed_bytes = lifecycle(4).max_feed_bytes + 1;
    let mut bytes = [0u8; CANDIDATE_BYTES];
    oversized.encode(&mut bytes).unwrap();
    let oversized = CandidateRecordV2::decode(&bytes).unwrap();
    assert_eq!(
        seal_candidate(
            begun.window,
            begun.index_page,
            oversized,
            begun.escrow,
            FeedSealV1 {
                candidate: oversized.candidate,
                epoch: oversized.epoch,
                feed: oversized.feed,
                content_digest: id(250),
                exact_bytes: oversized.expected_feed_bytes,
                written_bytes: oversized.expected_feed_bytes,
                canonical_padding: 1,
            },
            SealFundingV1 {
                verification_rent_principal: 900,
                work_reward_deposit: 9,
            },
            lifecycle(4),
            score(),
            liveness(),
            F + 1
        ),
        Err(Error::InvalidCount)
    );

    let mut too_many_units = begun.candidate;
    too_many_units.verification_units = lifecycle(4).max_verification_units + 1;
    too_many_units.encode(&mut bytes).unwrap();
    let too_many_units = CandidateRecordV2::decode(&bytes).unwrap();
    assert_eq!(
        seal_candidate(
            begun.window,
            begun.index_page,
            too_many_units,
            begun.escrow,
            FeedSealV1 {
                candidate: too_many_units.candidate,
                epoch: too_many_units.epoch,
                feed: too_many_units.feed,
                content_digest: id(250),
                exact_bytes: too_many_units.expected_feed_bytes,
                written_bytes: too_many_units.expected_feed_bytes,
                canonical_padding: 1,
            },
            SealFundingV1 {
                verification_rent_principal: 900,
                work_reward_deposit: liveness()
                    .work_reserve(too_many_units.verification_units)
                    .unwrap(),
            },
            lifecycle(4),
            score(),
            liveness(),
            F + 1
        ),
        Err(Error::InvalidCount)
    );
}

#[test]
fn inactive_top_padding_binds_the_canonical_discriminant() {
    let (window, budget) = frozen(4);
    let noncanonical_empty = CandidateVerdictV1 {
        kind: VerdictKind::Refused,
        ..CandidateVerdictV1::EMPTY
    };
    assert!(!noncanonical_empty.is_empty());
    assert_eq!(
        finalize_selection(
            window,
            budget,
            [
                noncanonical_empty,
                CandidateVerdictV1::EMPTY,
                CandidateVerdictV1::EMPTY,
            ],
            None,
            liveness(),
            S
        ),
        Err(Error::InvalidState)
    );
}

#[test]
fn epoch_budget_refuses_premature_paid_credited_and_refunded_compartments() {
    let pristine = budget();

    let mut finalizer_paid = pristine;
    finalizer_paid.finalizer_remaining -= 1;
    finalizer_paid.finalizer_paid += 1;
    assert_eq!(finalizer_paid.validate(), Err(Error::InvalidState));
    assert_eq!(
        freeze_window(
            open(4),
            finalizer_paid,
            lifecycle(4),
            score(),
            liveness(),
            F
        ),
        Err(Error::InvalidState)
    );

    let mut index_paid = pristine;
    index_paid.index_cleanup_remaining -= liveness().index_page_close_reward;
    index_paid.index_cleanup_paid += liveness().index_page_close_reward;
    assert_eq!(index_paid.validate(), Err(Error::InvalidState));

    let mut solver_credited = pristine;
    solver_credited.solver_remaining = 0;
    solver_credited.solver_credited = solver_credited.solver_initial;
    assert_eq!(solver_credited.validate(), Err(Error::InvalidState));

    let (window, frozen_budget) = frozen(4);
    let mut solver_refunded = frozen_budget;
    solver_refunded.solver_remaining = 0;
    solver_refunded.solver_refunded = solver_refunded.solver_initial;
    assert_eq!(solver_refunded.validate(), Err(Error::InvalidState));
    assert_eq!(
        finalize_selection(
            window,
            solver_refunded,
            [CandidateVerdictV1::EMPTY; TOP_CANDIDATE_CAPACITY],
            None,
            liveness(),
            S
        ),
        Err(Error::InvalidState)
    );
}

#[test]
fn policy_arithmetic_and_capacity_fail_closed() {
    let mut overflow = liveness();
    overflow.progress_reward_per_unit = u64::MAX;
    assert_eq!(overflow.validate(), Err(Error::ArithmeticOverflow));
    let mut too_large = lifecycle(4);
    too_large.max_begun_candidates = u16::MAX;
    assert_eq!(too_large.validate(), Err(Error::InvalidPolicy));
    assert_eq!(ADAPTER_OBLIGATIONS.len(), 15);
    assert!(PROMOTION_BLOCKERS.contains(&PromotionBlocker::CopyFrontRunningAdmissionDesign));
    assert!(PROMOTION_BLOCKERS.contains(&PromotionBlocker::QualityCapacityDenialOfService));
}
