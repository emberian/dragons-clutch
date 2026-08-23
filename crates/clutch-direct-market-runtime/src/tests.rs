use super::*;
use crate::reservation_v1::{
    prepare_direct_reservation_admission_v1, AuthenticatedDirectReservationAdmissionV1,
    DirectReservationPhaseV1,
};
use crate::selection_v1::{
    begin_direct_candidate_verification_v1, finalize_direct_selection_v1,
    prepare_direct_selection_freeze_v1, submit_direct_candidate_v1,
    verify_next_direct_candidate_v1, AuthenticatedDirectSelectionFreezeV1,
    DirectSelectionPhaseV1,
};
use clutch_batch::direct_pair_v1::DirectEconomicCandidateV1;
use clutch_batch::relation_v2::{
    price_semantics_digest_v2, EconomicDomainV2, PricePreconditionV2,
    ECONOMIC_RELATION_VERSION_V2,
};
use clutch_batch::{PartialPolicy, Side};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_retirement::{
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, PositionV3Fields,
    PositionV3Sha256Backend, RentSplitV2, MAX_OUTCOMES,
};
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

impl PositionV3Sha256Backend for Sha {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        self.sha256_parts(&[domain, body])
    }
}

#[derive(Clone, Copy, Debug)]
struct AllowReservation;

impl AuthenticatedDirectReservationAdmissionV1 for AllowReservation {
    fn authenticate_admission(
        &self,
        _root: DirectMarketRootV1,
        _position: AuthenticatedPositionV3,
        _reservation_account: [u8; 32],
        _order_id: [u8; 32],
        _side: Side,
        _outcome: u8,
        _quantity: u64,
        _minimum_fill: u64,
        _partial_policy: PartialPolicy,
        _expiry_epoch: u64,
        _limit_price_units_per_egg: u128,
        _rent: DirectRentOwnerV1,
    ) -> Result<(), DirectMarketErrorV1> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct AllowFreeze;

impl AuthenticatedDirectSelectionFreezeV1 for AllowFreeze {
    fn authenticate_freeze(
        &self,
        _root: DirectMarketRootV1,
        _selection_account: [u8; 32],
        _rent: DirectRentOwnerV1,
        _reservations: &[Option<crate::reservation_v1::DirectReservationV1>; 2],
        _reservation_semantic_ids: &[[u8; 32]; 2],
        _domain: &EconomicDomainV2,
        _price: &PricePreconditionV2,
    ) -> Result<(), DirectMarketErrorV1> {
        Ok(())
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

fn identity(value: u8) -> Identity32V1 {
    Identity32V1::new(id(value)).unwrap()
}

fn position(cash: u64, eggs_at_zero: u64) -> AuthenticatedPositionV3 {
    let mut native_eggs = [0u64; MAX_OUTCOMES];
    native_eggs[0] = eggs_at_zero;
    let semantic = PositionAccountV3::new(PositionV3Fields {
        purpose: PositionPurposeV3::General,
        lifecycle: PositionLifecycleV3::Open,
        outcome_count: 16,
        stored_bump: 7,
        generation: 1,
        market_instance_id: identity(1),
        realm_id: identity(2),
        collateral_policy_id: identity(4),
        collateral_release_id: identity(5),
        owner: identity(40),
        controller: identity(40),
        replay_account: identity(42),
        purpose_binding_id: identity(12),
        cash_atoms: cash,
        reserved_cash_atoms: 0,
        native_eggs,
        outstanding_reservations: 0,
        rent: RentSplitV2 {
            payer: identity(43),
            refundable_live_principal: 100,
            permanent_tombstone_principal: 80,
            donation_floor: 3,
        },
    })
    .unwrap();
    let semantic_id = semantic.semantic_id(&Sha).unwrap().bytes();
    AuthenticatedPositionV3 {
        account: id(41),
        general_market_runtime: id(12),
        semantic,
        semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    }
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
fn cross_namespace_equal_bytes_are_not_account_aliases() {
    let mut binding = state().root.binding();
    binding.relation_policy_id = binding.resolution_account;
    binding.compiler_bundle_v5_id = binding.product_root_account;
    assert_eq!(binding.validate(), Ok(()));

    let frozen = state()
        .admit_reservation(1, 10, id(20), id(20), &Sha)
        .unwrap()
        .admit_reservation(2, 11, id(22), id(22), &Sha)
        .unwrap()
        .freeze(3, 20, id(24), id(24), &Sha)
        .unwrap();
    assert_eq!(frozen.root.phase(), DirectRootPhaseV1::SubmissionOpen);
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

#[test]
fn zero_price_buyer_reservation_is_exact_and_counts_the_child() {
    let plan = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        state().root,
        position(100, 0),
        id(50),
        id(51),
        Side::Buy,
        0,
        10,
        0,
        PartialPolicy::Allow,
        7,
        0,
        rent(52, 70, 2),
    )
    .unwrap();
    assert_eq!(plan.reservation.phase(), DirectReservationPhaseV1::Active);
    assert_eq!(plan.reservation.reserved_cash_atoms(), 0);
    let fields = plan.position_poststate.semantic.fields();
    assert_eq!(fields.cash_atoms, 100);
    assert_eq!(fields.reserved_cash_atoms, 0);
    assert_eq!(fields.outstanding_reservations, 1);
}

#[test]
fn reservation_refuses_rounding_and_debits_seller_eggs_exactly() {
    assert_eq!(
        prepare_direct_reservation_admission_v1(
            &AllowReservation,
            state().root,
            position(100, 0),
            id(50),
            id(51),
            Side::Buy,
            0,
            1,
            0,
            PartialPolicy::Allow,
            7,
            50,
            rent(52, 70, 2),
        ),
        Err(DirectMarketErrorV1::InexactCashConversion)
    );
    let seller = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        state().root,
        position(0, 20),
        id(50),
        id(51),
        Side::Sell,
        0,
        7,
        7,
        PartialPolicy::AllOrNone,
        7,
        0,
        rent(52, 70, 2),
    )
    .unwrap();
    let fields = seller.position_poststate.semantic.fields();
    assert_eq!(fields.native_eggs[0], 13);
    assert_eq!(fields.outstanding_reservations, 1);
}

fn direct_domain_and_zero_price() -> (EconomicDomainV2, PricePreconditionV2) {
    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: id(1),
        epoch_semantics_digest: id(7),
        relation_policy_digest: id(14),
        price_policy_digest: id(15),
        epoch_index: 1,
        outcome_count: 16,
        price_scale: 1_000,
    };
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[1] = 1_000;
    let price = PricePreconditionV2 {
        policy_digest: id(15),
        semantic_price_digest: price_semantics_digest_v2(&domain, &prices).unwrap(),
        prices,
    };
    (domain, price)
}

fn candidate(fill: u64) -> DirectEconomicCandidateV1 {
    DirectEconomicCandidateV1 {
        fills: [fill, fill],
        honored_aon_mask: 0,
    }
}

#[test]
fn complete_selection_reverifies_and_ranks_exact_zero_price_pair() {
    let initial = state();
    let buyer = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        initial.root,
        position(100, 0),
        id(50),
        id(51),
        Side::Buy,
        0,
        10,
        0,
        PartialPolicy::Allow,
        7,
        0,
        rent(52, 70, 2),
    )
    .unwrap();
    let after_buyer = initial
        .admit_reservation(1, 10, buyer.reservation.account(), buyer.reservation.semantic_id(&Sha).unwrap(), &Sha)
        .unwrap();
    let seller = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        after_buyer.root,
        position(0, 10),
        id(60),
        id(61),
        Side::Sell,
        0,
        10,
        0,
        PartialPolicy::Allow,
        7,
        0,
        rent(62, 70, 2),
    )
    .unwrap();
    let after_seller = after_buyer
        .admit_reservation(2, 11, seller.reservation.account(), seller.reservation.semantic_id(&Sha).unwrap(), &Sha)
        .unwrap();
    let (domain, price) = direct_domain_and_zero_price();
    let frozen = prepare_direct_selection_freeze_v1(
        &AllowFreeze,
        after_seller,
        3,
        20,
        id(70),
        rent(71, 80, 3),
        [Some(seller.reservation), Some(buyer.reservation)],
        domain,
        price,
        &Sha,
    )
    .unwrap();
    assert_eq!(frozen.selection.phase(), DirectSelectionPhaseV1::SubmissionOpen);
    assert_eq!(frozen.selection.reservation_account(0).unwrap(), id(50));

    let first = submit_direct_candidate_v1(frozen.state, frozen.selection, 4, 21, candidate(5), &Sha).unwrap();
    let second = submit_direct_candidate_v1(first.state, first.selection, 5, 22, candidate(10), &Sha).unwrap();
    let begun = begin_direct_candidate_verification_v1(second.state, second.selection, 6, 30, &Sha).unwrap();
    let verified_first = verify_next_direct_candidate_v1(begun.state, begun.selection, 7, 31, &Sha).unwrap();
    let verified_second = verify_next_direct_candidate_v1(verified_first.state, verified_first.selection, 8, 32, &Sha).unwrap();
    let selected = finalize_direct_selection_v1(verified_second.state, verified_second.selection, 9, 33, &Sha).unwrap();
    let pair = selected.selection.selected_pair().unwrap();
    assert_eq!(pair.quantity(), 10);
    assert_eq!(pair.consideration_cash_atoms(), 0);
}

#[test]
fn selection_refuses_missing_extra_duplicate_and_partial_traversal() {
    let initial = state();
    let buyer = prepare_direct_reservation_admission_v1(
        &AllowReservation, initial.root, position(100, 0), id(50), id(51), Side::Buy,
        0, 10, 0, PartialPolicy::Allow, 7, 0, rent(52, 70, 2),
    ).unwrap();
    let after_buyer = initial.admit_reservation(
        1, 10, id(50), buyer.reservation.semantic_id(&Sha).unwrap(), &Sha,
    ).unwrap();
    let seller = prepare_direct_reservation_admission_v1(
        &AllowReservation, after_buyer.root, position(0, 10), id(60), id(61), Side::Sell,
        0, 10, 0, PartialPolicy::Allow, 7, 0, rent(62, 70, 2),
    ).unwrap();
    let after_seller = after_buyer.admit_reservation(
        2, 11, id(60), seller.reservation.semantic_id(&Sha).unwrap(), &Sha,
    ).unwrap();
    let (domain, price) = direct_domain_and_zero_price();
    assert_eq!(
        prepare_direct_selection_freeze_v1(
            &AllowFreeze, after_seller, 3, 20, id(70), rent(71, 80, 3),
            [Some(buyer.reservation), None], domain, price, &Sha,
        ),
        Err(DirectMarketErrorV1::InvalidCount)
    );
    assert_eq!(
        prepare_direct_selection_freeze_v1(
            &AllowFreeze, after_seller, 3, 20, id(70), rent(71, 80, 3),
            [Some(buyer.reservation), Some(buyer.reservation)], domain, price, &Sha,
        ),
        Err(DirectMarketErrorV1::IdentityAlias)
    );

    let frozen = prepare_direct_selection_freeze_v1(
        &AllowFreeze, after_seller, 3, 20, id(70), rent(71, 80, 3),
        [Some(buyer.reservation), Some(seller.reservation)], domain, price, &Sha,
    ).unwrap();
    let submitted = submit_direct_candidate_v1(
        frozen.state, frozen.selection, 4, 21, candidate(10), &Sha,
    ).unwrap();
    assert_eq!(
        submit_direct_candidate_v1(
            submitted.state, submitted.selection, 5, 22, candidate(10), &Sha,
        ),
        Err(DirectMarketErrorV1::IdentityAlias)
    );
    let begun = begin_direct_candidate_verification_v1(
        submitted.state, submitted.selection, 5, 30, &Sha,
    ).unwrap();
    assert_eq!(
        finalize_direct_selection_v1(begun.state, begun.selection, 6, 31, &Sha),
        Err(DirectMarketErrorV1::WrongPhase)
    );
}
