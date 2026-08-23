// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_candidate_lifecycle::*;

const F: u64 = 100;
const R: u64 = 105;
const S: u64 = 110;
const V: u64 = 120;

fn id(value: u64) -> Id {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&value.to_le_bytes());
    Id::from_bytes(bytes)
}

fn admission() -> CandidateAdmissionPolicyV3 {
    CandidateAdmissionPolicyV3 {
        policy_id: id(4),
        neutral_sink: id(6),
        commit_span_slots: R - F,
        reveal_span_slots: S - R,
        verification_span_slots: V - S,
        bond_lamports: 100,
        abandonment_penalty: 30,
        node_cleanup_reward: 11,
        flags: 0,
    }
}

fn score() -> ScorePolicyBindingV1 {
    ScorePolicyBindingV1 {
        policy_id: id(5),
        rank_key_len: 33,
    }
}

fn frozen() -> CandidateWindowV4 {
    let open =
        CandidateWindowV4::open(id(1), id(2), id(3), admission(), score(), F, 1).expect("open");
    freeze_candidate_window_v4(open, admission(), score(), F).expect("freeze")
}

fn commit_input(value: u64) -> CommitCandidateInputV3 {
    CommitCandidateInputV3 {
        node: id(1_000 + value),
        commitment: id(2_000 + value),
        submitter_authority: id(3_000 + value),
        solver_reward_destination: id(4_000 + value),
        payer: id(5_000 + value),
        refund_destination: id(6_000 + value),
        node_rent_principal: 500,
        stored_bump: 1,
    }
}

fn opening(
    node: CandidateAdmissionNodeV3,
    candidate_digest: Id,
) -> AdapterVerifiedCommitmentOpeningV1 {
    AdapterVerifiedCommitmentOpeningV1 {
        epoch: node.epoch,
        node: node.node,
        admission_policy_id: node.admission_policy_id,
        commitment: node.commitment,
        submitter_authority: node.submitter_authority,
        solver_reward_destination: node.solver_reward_destination,
        candidate_digest,
    }
}

fn rank(node: Id, score_byte: u8) -> RankKey {
    let mut bytes = [0u8; RANK_KEY_CAPACITY];
    bytes[0] = score_byte;
    let identity = node.bytes();
    let mut index = 0usize;
    while index < 32 {
        bytes[index + 1] = !identity[index];
        index += 1;
    }
    RankKey::new(33, bytes).expect("rank")
}

fn valid_verdict(
    window: CandidateWindowV4,
    node: CandidateAdmissionNodeV3,
    rank_key: RankKey,
) -> AdapterVerifiedVerdictV3 {
    AdapterVerifiedVerdictV3 {
        epoch: window.epoch,
        node: node.node,
        candidate_digest: node.candidate_digest,
        relation_policy_id: window.relation_policy_id,
        score_policy_id: window.score_policy_id,
        kind: AdapterVerifiedVerdictKindV3::Valid { rank_key },
    }
}

fn cleanup(node: CandidateAdmissionNodeV3, selected: bool) -> AdapterVerifiedAdmissionCleanupV3 {
    AdapterVerifiedAdmissionCleanupV3 {
        epoch: node.epoch,
        node: node.node,
        candidate_digest: node.candidate_digest,
        candidate_bundle_closed: 1,
        selected_settlement_terminal_slot: if selected { V } else { 0 },
    }
}

#[test]
fn more_than_legacy_capacity_uses_no_shared_sponsor_page_and_cleans_boundedly() {
    let mut window = frozen();
    let mut nodes = Vec::new();

    for value in 1u64..=80 {
        let transition = commit_candidate_v3(window, commit_input(value), admission(), score(), F)
            .expect("individually funded commit");
        assert_eq!(transition.required_lamports, 611);
        window = transition.window;
        nodes.push(transition.node);
    }

    assert_eq!(window.admitted_count, 80);
    assert_eq!(window.live_node_count, 80);
    assert_eq!(window.admission_head, nodes[79].node);

    for node in &mut nodes {
        let expired = expire_commitment_v3(window, *node, admission(), score(), S)
            .expect("expire abandonment");
        window = expired.0;
        *node = expired.1;
    }
    window = finalize_selection_v3(window, admission(), score(), S).expect("resolved finalize");

    while let Some(node) = nodes.pop() {
        let transition = close_admission_head_v3(
            window,
            node,
            cleanup(node, false),
            admission(),
            score(),
            611,
        )
        .expect("one bounded LIFO close");
        assert_eq!(transition.disposition.keeper_reward, 11);
        assert_eq!(transition.disposition.neutral_sink_credit, 30);
        assert_eq!(transition.disposition.bond_refund, 70);
        assert_eq!(transition.disposition.rent_principal_refund, 500);
        window = transition.window;
    }

    assert!(window
        .admission_ledger_retired()
        .expect("valid retired ledger"));
    assert_eq!(window.closed_node_count, 80);
}

#[test]
fn commit_reveal_binding_blocks_simple_reveal_copy_and_reward_substitution() {
    let start = frozen();
    let committed =
        commit_candidate_v3(start, commit_input(1), admission(), score(), F).expect("commit");
    let window = committed.window;
    let node = committed.node;

    let mut forged_funding = node;
    forged_funding.bond_lamports = 99;
    assert_eq!(
        reveal_candidate_v3(
            window,
            forged_funding,
            opening(forged_funding, id(9_000)),
            admission(),
            score(),
            R,
        ),
        Err(Error::MismatchedBinding)
    );

    assert_eq!(
        commit_candidate_v3(window, commit_input(2), admission(), score(), R),
        Err(Error::NotActive)
    );

    let mut wrong_commitment = opening(node, id(9_000));
    wrong_commitment.commitment = id(9_001);
    assert_eq!(
        reveal_candidate_v3(window, node, wrong_commitment, admission(), score(), R),
        Err(Error::MismatchedBinding)
    );

    let mut stolen_reward = opening(node, id(9_000));
    stolen_reward.solver_reward_destination = id(9_002);
    assert_eq!(
        reveal_candidate_v3(window, node, stolen_reward, admission(), score(), R),
        Err(Error::MismatchedBinding)
    );

    let revealed = reveal_candidate_v3(
        window,
        node,
        opening(node, id(9_000)),
        admission(),
        score(),
        R,
    )
    .expect("bound opening");
    assert_eq!(
        reveal_candidate_v3(
            revealed.0,
            revealed.1,
            opening(revealed.1, id(9_000)),
            admission(),
            score(),
            R,
        ),
        Err(Error::Replay)
    );
}

#[test]
fn exact_deadlines_are_half_open_and_replays_are_atomic_refusals() {
    let window = frozen();
    assert_eq!(
        commit_candidate_v3(window, commit_input(1), admission(), score(), F - 1),
        Err(Error::NotActive)
    );
    let committed = commit_candidate_v3(window, commit_input(1), admission(), score(), R - 1)
        .expect("last commit slot");
    assert_eq!(
        commit_candidate_v3(
            committed.window,
            commit_input(1),
            admission(),
            score(),
            R - 1,
        ),
        Err(Error::DuplicateIdentity)
    );
    assert_eq!(
        reveal_candidate_v3(
            committed.window,
            committed.node,
            opening(committed.node, id(8_000)),
            admission(),
            score(),
            R - 1,
        ),
        Err(Error::NotActive)
    );
    let revealed = reveal_candidate_v3(
        committed.window,
        committed.node,
        opening(committed.node, id(8_000)),
        admission(),
        score(),
        S - 1,
    )
    .expect("last reveal slot");
    assert_eq!(
        reveal_candidate_v3(
            committed.window,
            committed.node,
            opening(committed.node, id(8_000)),
            admission(),
            score(),
            S,
        ),
        Err(Error::NotActive)
    );
    let verdict = valid_verdict(revealed.0, revealed.1, rank(revealed.1.node, 7));
    assert_eq!(
        record_verdict_v3(revealed.0, revealed.1, verdict, admission(), score(), V,),
        Err(Error::NotActive)
    );
    let verified = record_verdict_v3(revealed.0, revealed.1, verdict, admission(), score(), S)
        .expect("first verification slot");
    assert_eq!(
        record_verdict_v3(verified.0, verified.1, verdict, admission(), score(), S,),
        Err(Error::Replay)
    );
}

#[test]
fn equal_economic_scores_use_canonical_node_tie_break_not_append_order() {
    let first =
        commit_candidate_v3(frozen(), commit_input(200), admission(), score(), F).expect("first");
    let second = commit_candidate_v3(first.window, commit_input(100), admission(), score(), F)
        .expect("second");
    let shared_candidate = id(42_000);
    let first_reveal = reveal_candidate_v3(
        second.window,
        first.node,
        opening(first.node, shared_candidate),
        admission(),
        score(),
        R,
    )
    .expect("first reveal");
    let second_reveal = reveal_candidate_v3(
        first_reveal.0,
        second.node,
        opening(second.node, shared_candidate),
        admission(),
        score(),
        R,
    )
    .expect("second reveal");
    let first_verified = record_verdict_v3(
        second_reveal.0,
        first_reveal.1,
        valid_verdict(second_reveal.0, first_reveal.1, rank(first.node.node, 9)),
        admission(),
        score(),
        S,
    )
    .expect("first verdict");
    let second_verified = record_verdict_v3(
        first_verified.0,
        second_reveal.1,
        valid_verdict(first_verified.0, second_reveal.1, rank(second.node.node, 9)),
        admission(),
        score(),
        S,
    )
    .expect("second verdict");

    assert_eq!(second_verified.0.best_candidate_node, second.node.node);
    let finalized =
        finalize_selection_v3(second_verified.0, admission(), score(), S).expect("early final");
    assert_eq!(finalized.selected_candidate_node, second.node.node);

    let copied_rank = rank(first.node.node, 9);
    assert_eq!(
        record_verdict_v3(
            first_verified.0,
            second_reveal.1,
            valid_verdict(first_verified.0, second_reveal.1, copied_rank),
            admission(),
            score(),
            S,
        ),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn unresolved_work_hard_closes_at_v_and_cleanup_is_reverse_counted() {
    let first =
        commit_candidate_v3(frozen(), commit_input(1), admission(), score(), F).expect("first");
    let second = commit_candidate_v3(first.window, commit_input(2), admission(), score(), F)
        .expect("second");
    let first_reveal = reveal_candidate_v3(
        second.window,
        first.node,
        opening(first.node, id(7_001)),
        admission(),
        score(),
        R,
    )
    .expect("first reveal");
    let second_reveal = reveal_candidate_v3(
        first_reveal.0,
        second.node,
        opening(second.node, id(7_002)),
        admission(),
        score(),
        R,
    )
    .expect("second reveal");
    let first_verified = record_verdict_v3(
        second_reveal.0,
        first_reveal.1,
        valid_verdict(second_reveal.0, first_reveal.1, rank(first.node.node, 3)),
        admission(),
        score(),
        S,
    )
    .expect("one verdict");
    assert_eq!(
        finalize_selection_v3(first_verified.0, admission(), score(), V - 1),
        Err(Error::UnresolvedCandidates)
    );
    let finalized =
        finalize_selection_v3(first_verified.0, admission(), score(), V).expect("hard close");

    assert_eq!(
        close_admission_head_v3(
            finalized,
            first_verified.1,
            cleanup(first_verified.1, true),
            admission(),
            score(),
            611,
        ),
        Err(Error::MismatchedBinding)
    );
    let expired = expire_unverified_v3(finalized, second_reveal.1, admission(), score(), V)
        .expect("expire tail");
    assert_eq!(
        close_admission_head_v3(
            expired.0,
            expired.1,
            cleanup(expired.1, false),
            admission(),
            score(),
            610,
        ),
        Err(Error::Underfunded)
    );
    let tail_closed = close_admission_head_v3(
        expired.0,
        expired.1,
        cleanup(expired.1, false),
        admission(),
        score(),
        618,
    )
    .expect("tail close");
    assert_eq!(tail_closed.disposition.neutral_sink_credit, 7);
    assert_eq!(
        close_admission_head_v3(
            tail_closed.window,
            first_verified.1,
            cleanup(first_verified.1, false),
            admission(),
            score(),
            611,
        ),
        Err(Error::MismatchedBinding)
    );
    let selected_closed = close_admission_head_v3(
        tail_closed.window,
        first_verified.1,
        cleanup(first_verified.1, true),
        admission(),
        score(),
        611,
    )
    .expect("selected settlement terminal");
    assert!(selected_closed
        .window
        .admission_ledger_retired()
        .expect("retired"));
}

#[test]
fn abandoned_commitment_penalty_and_cleanup_replays_are_exact() {
    let committed =
        commit_candidate_v3(frozen(), commit_input(1), admission(), score(), F).expect("commit");
    let expired = expire_commitment_v3(committed.window, committed.node, admission(), score(), S)
        .expect("expire");
    assert_eq!(
        expire_commitment_v3(expired.0, expired.1, admission(), score(), S),
        Err(Error::Replay)
    );
    let finalized = finalize_selection_v3(expired.0, admission(), score(), S).expect("finalize");
    let closed = close_admission_head_v3(
        finalized,
        expired.1,
        cleanup(expired.1, false),
        admission(),
        score(),
        616,
    )
    .expect("close");
    assert_eq!(closed.disposition.keeper_reward, 11);
    assert_eq!(closed.disposition.rent_principal_refund, 500);
    assert_eq!(closed.disposition.bond_refund, 70);
    assert_eq!(closed.disposition.neutral_sink_credit, 35);
    assert_eq!(
        close_admission_head_v3(
            closed.window,
            expired.1,
            cleanup(expired.1, false),
            admission(),
            score(),
            616,
        ),
        Err(Error::MismatchedBinding)
    );
}
