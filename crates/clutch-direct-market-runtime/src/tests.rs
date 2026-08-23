use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug)]
struct Sha;

impl DirectHashBackendV1 for Sha {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts {
            hash.update(part);
        }
        hash.finalize().into()
    }
}

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn rent(payer: u8, principal: u64, donation: u64) -> DirectRentOwnerV1 {
    DirectRentOwnerV1 {
        payer: id(payer),
        principal_lamports: principal,
        donation_floor_lamports: donation,
    }
}

fn state() -> DirectRootReplayPostV1 {
    let binding = DirectMarketBindingV1 {
        market_instance_id: id(1),
        generation: 1,
        outcome_count: 16,
        realm_id: id(2),
        collateral_profile_id: id(3),
        collateral_policy_id: id(4),
        collateral_release_id: id(5),
        resolution_account: id(6),
        resolution_semantic_id: id(7),
        resolution_data_id: id(8),
        product_root_account: id(9),
        founder_series_link_account: id(18),
        founder_series_link_binding_id: id(19),
        compiler_bundle_v5_id: id(32),
        founder_series_plan_id: id(33),
        founder_series_ordinal: 0,
        direct_root_account: id(10),
        action_replay_account: id(11),
        general_market_runtime: id(12),
        neutral_lamport_sink: id(13),
        relation_policy_id: id(14),
        price_policy_id: id(15),
        price_scale: 1_000,
    };
    let root = DirectMarketRootV1 {
        binding,
        schedule: DirectScheduleV1 {
            admission_opens_slot: 10,
            admission_closes_slot: 20,
            submission_closes_slot: 30,
            selection_deadline_slot: 40,
            settlement_deadline_slot: 50,
        },
        root_rent: rent(16, 1_000, 7),
        phase: DirectRootPhaseV1::Open,
        terminal_reason: None,
        admitted_reservations: 0,
        live_reservations: 0,
        retired_reservations: 0,
        selection_account: [0; 32],
    };
    let replay = DirectActionReplayV1 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        direct_root_account: binding.direct_root_account,
        replay_account: binding.action_replay_account,
        rent: rent(17, 900, 3),
        phase: DirectReplayPhaseV1::Active,
        next_action_sequence: 1,
        action_transcript_id: Sha.sha256_parts(&[b"transcript"]),
        foundation_receipt_id: Sha.sha256_parts(&[b"foundation"]),
        economic_terminal_receipt_id: [0; 32],
        family_terminal_receipt_id: [0; 32],
    };
    replay.validate_against(root).unwrap();
    DirectRootReplayPostV1 { root, replay }
}

#[test]
fn exact_pair_is_required_and_replay_is_linear() {
    let one = state()
        .admit_reservation(1, 10, id(20), id(21), &Sha)
        .unwrap();
    assert_eq!(one.replay.next_action_sequence(), 2);
    assert_eq!(
        one.admit_reservation(1, 11, id(22), id(23), &Sha),
        Err(DirectMarketErrorV1::Replay)
    );
    let two = one
        .admit_reservation(2, 11, id(22), id(23), &Sha)
        .unwrap();
    let frozen = two.freeze(3, 20, id(24), id(25), &Sha).unwrap();
    assert_eq!(frozen.root.phase(), DirectRootPhaseV1::SubmissionOpen);
    assert_eq!(frozen.root.live_reservations(), 2);
}

#[test]
fn schedule_boundaries_are_owned_by_pure_transitions() {
    assert_eq!(
        state().admit_reservation(1, 9, id(20), id(21), &Sha),
        Err(DirectMarketErrorV1::WrongPhase)
    );
    let one = state()
        .admit_reservation(1, 10, id(20), id(21), &Sha)
        .unwrap();
    assert_eq!(
        one.freeze(2, 19, id(22), id(23), &Sha),
        Err(DirectMarketErrorV1::WrongPhase)
    );
    assert_eq!(
        one.freeze(2, 30, id(22), id(23), &Sha),
        Err(DirectMarketErrorV1::WrongPhase)
    );
}

#[test]
fn cancellation_retires_only_the_named_archive_class() {
    let admitted = state()
        .admit_reservation(1, 10, id(20), id(21), &Sha)
        .unwrap();
    let cancelled = admitted
        .cancel_reservation(2, 19, id(22), &Sha)
        .unwrap();
    assert_eq!(cancelled.root.admitted_reservations(), 1);
    assert_eq!(cancelled.root.live_reservations(), 0);
    assert_eq!(cancelled.root.retired_reservations(), 1);
    let frozen = cancelled.freeze(3, 20, id(24), id(25), &Sha).unwrap();
    assert_eq!(frozen.root.phase(), DirectRootPhaseV1::FrozenEmpty);
}

#[test]
fn same_phase_selection_work_changes_only_permanent_replay() {
    let two = state()
        .admit_reservation(1, 10, id(20), id(21), &Sha)
        .unwrap()
        .admit_reservation(2, 11, id(22), id(23), &Sha)
        .unwrap()
        .freeze(3, 20, id(24), id(25), &Sha)
        .unwrap();
    let root_id = two.root.semantic_id(&Sha).unwrap();
    let transcript = two.replay.action_transcript_id();
    let submitted = two.record_submission(4, 21, id(26), &Sha).unwrap();
    assert_eq!(submitted.root.semantic_id(&Sha).unwrap(), root_id);
    assert_ne!(submitted.replay.action_transcript_id(), transcript);
    assert_eq!(submitted.replay.next_action_sequence(), 5);
}

#[test]
fn empty_lapse_sets_economic_receipt_only_in_permanent_owner() {
    let frozen = state().freeze(1, 20, id(20), id(21), &Sha).unwrap();
    let terminal = frozen
        .terminalize(2, 20, DirectTerminalReasonV1::EmptyLapse, id(22), &Sha)
        .unwrap();
    assert_eq!(terminal.root.phase(), DirectRootPhaseV1::Terminal);
    assert_eq!(
        terminal.root.terminal_reason(),
        Some(DirectTerminalReasonV1::EmptyLapse)
    );
    assert_eq!(terminal.replay.economic_terminal_receipt_id(), id(22));
    assert_eq!(terminal.replay.family_terminal_receipt_id(), [0; 32]);
}

fn retirement() -> DirectRetirementTransferV1 {
    DirectRetirementTransferV1 {
        sources: [
            Some(DirectRetirementSourceV1 {
                account: id(20),
                rent: rent(30, 100, 5),
                observed_lamports: 110,
            }),
            Some(DirectRetirementSourceV1 {
                account: id(21),
                rent: rent(30, 200, 7),
                observed_lamports: 220,
            }),
            None,
            None,
        ],
        source_count: 2,
        refunds: [
            Some(DirectPrincipalRefundV1 {
                recipient: id(30),
                lamports: 300,
            }),
            None,
            None,
            None,
        ],
        refund_count: 1,
        neutral_lamport_sink: id(31),
        surplus_lamports: 30,
    }
}

#[test]
fn retirement_refunds_principal_only_and_coalesces_sorted_payers() {
    assert_eq!(retirement().validate(), Ok(()));
    let mut donation_as_refund = retirement();
    donation_as_refund.refunds[0] = Some(DirectPrincipalRefundV1 {
        recipient: id(30),
        lamports: 330,
    });
    assert_eq!(
        donation_as_refund.validate(),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );
}

#[test]
fn retirement_refuses_duplicate_sources_and_nonzero_tail() {
    let mut duplicate = retirement();
    duplicate.sources[1] = duplicate.sources[0];
    assert_eq!(duplicate.validate(), Err(DirectMarketErrorV1::IdentityAlias));

    let mut tail = retirement();
    tail.sources[2] = tail.sources[1];
    assert_eq!(tail.validate(), Err(DirectMarketErrorV1::InvalidCount));
}
