use super::*;
use crate::codec_v1::{
    decode_direct_action_replay_body_v1, decode_direct_market_root_body_v1,
    decode_direct_reservation_body_v1, decode_direct_selection_body_v1,
    encode_direct_action_replay_body_v1, encode_direct_market_root_body_v1,
    encode_direct_reservation_body_v1, encode_direct_selection_body_v1,
};
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
use crate::settlement_v1::{
    prepare_direct_economic_terminal_v1,
    prepare_direct_missed_freeze_terminal_v1,
    prepare_direct_reservation_admission_with_replay_v1,
    prepare_direct_reservation_cancel_v1, AuthenticatedDirectEconomicTerminalV1,
    AuthenticatedDirectReservationCancelV1, DirectEndpointPrestateV1,
    DirectReservationOrderInputV1,
};
use clutch_batch::direct_pair_v1::DirectEconomicCandidateV1;
use clutch_batch::relation_v2::{
    price_semantics_digest_v2, EconomicDomainV2, PricePreconditionV2,
    ECONOMIC_RELATION_VERSION_V2,
};
use clutch_batch::{PartialPolicy, Side};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_general_v2_contract::{
    found_general_position_replay_v1, project_general_position_replay_prestate_v1,
    GeneralPositionReplayPrestateV1, GeneralReplayTransitionPlanV1, Id32,
};
use clutch_retirement::{
    DeletableRentOwnerV1, Identity32V1, PositionAccountV3, PositionLifecycleV3,
    PositionPurposeV3, PositionV3Fields, PositionV3Sha256Backend, RentSplitV2,
    ReplayV3HashBackend, MAX_OUTCOMES,
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

impl ReplayV3HashBackend for Sha {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, parts)
    }
}

#[derive(Clone, Copy, Debug)]
struct AllowReservation;

impl AuthenticatedDirectReservationAdmissionV1 for AllowReservation {
    fn authenticate_admission(
        &self,
        _root: DirectMarketRootV1,
        _position: AuthenticatedPositionV3,
        _existing_peer: Option<crate::reservation_v1::DirectReservationV1>,
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

#[derive(Clone, Copy, Debug)]
struct AllowCancel;

impl AuthenticatedDirectReservationCancelV1 for AllowCancel {
    fn authenticate_cancel(
        &self,
        _state: DirectRootReplayPostV1,
        _reservation: crate::reservation_v1::DirectReservationV1,
        _position_replay: GeneralPositionReplayPrestateV1,
        _observed_reservation_lamports: u64,
        _consumed_sequence: u64,
        _observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct AllowEconomicTerminal;

impl AuthenticatedDirectEconomicTerminalV1 for AllowEconomicTerminal {
    fn authenticate_terminal(
        &self,
        _state: DirectRootReplayPostV1,
        _selection: crate::selection_v1::DirectSelectionV1,
        _ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        _reason: DirectTerminalReasonV1,
        _consumed_sequence: u64,
        _observed_slot: u64,
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
    let schedule = DirectScheduleV1 {
        admission_opens_slot: 10,
        admission_closes_slot: 20,
        submission_closes_slot: 30,
        selection_deadline_slot: 40,
        settlement_deadline_slot: 50,
    };
    let mut binding = DirectMarketBindingV1 {
        market_instance_id: id(1),
        generation: 1,
        outcome_count: 16,
        realm_id: id(2),
        collateral_profile_id: id(3),
        collateral_policy_id: id(4),
        collateral_release_id: id(5),
        resolution_account: id(6),
        direct_epoch_semantics_id: [0; 32],
        fee_policy_id: DirectZeroFeeVenueV1::canonical().unwrap().revenue_policy_id,
        direct_fee_shape_id: DirectZeroFeeVenueV1::canonical()
            .unwrap()
            .semantic_id(&Sha)
            .unwrap(),
        candidate_lifecycle_policy_id: id(36),
        candidate_liveness_policy_id: id(37),
        direct_schedule_policy_id: [0; 32],
        product_root_account: id(9),
        product_market_binding_id: id(38),
        product_family_prestate_id: id(35),
        general_product_preauthorization_id: id(39),
        family_admission_sequence: 0,
        founder_series_link_account: id(18),
        founder_series_link_binding_id: id(19),
        compiler_bundle_v5_id: id(32),
        founder_series_plan_id: id(33),
        founder_series_ordinal: 0,
        direct_root_account: id(10),
        action_replay_account: id(11),
        general_market_binding: id(34),
        general_market_runtime: id(12),
        neutral_lamport_sink: id(13),
        relation_policy_id: id(14),
        price_policy_id: id(15),
        price_scale: 1_000,
    };
    binding.direct_schedule_policy_id = direct_schedule_policy_id_v1(binding, &Sha).unwrap();
    binding.direct_epoch_semantics_id =
        direct_epoch_semantics_id_v1(binding, schedule, &Sha).unwrap();
    let root = DirectMarketRootV1 {
        binding,
        schedule,
        root_rent: rent(16, 1_000, 7),
        phase: DirectRootPhaseV1::Open,
        terminal_reason: None,
        admitted_reservations: 0,
        live_reservations: 0,
        retired_reservations: 0,
        reservation_accounts: [[0; 32]; 2],
        reservation_semantic_ids: [[0; 32]; 2],
        selection_account: [0; 32],
    };
    let replay = DirectActionReplayV1 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        direct_epoch_semantics_id: binding.direct_epoch_semantics_id,
        direct_root_account: binding.direct_root_account,
        replay_account: binding.action_replay_account,
        rent: rent(17, 900, 3),
        phase: DirectReplayPhaseV1::Active,
        next_action_sequence: 1,
        action_transcript_id: DirectHashBackendV1::sha256_parts(&Sha, &[b"transcript"]),
        foundation_receipt_id: DirectHashBackendV1::sha256_parts(&Sha, &[b"foundation"]),
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

fn founding_general_prestate_for(
    position_account: u8,
    replay_account: u8,
    owner: u8,
    rent_payer: u8,
) -> GeneralPositionReplayPrestateV1 {
    let founding = found_general_position_replay_v1(
        identity(position_account),
        identity(replay_account),
        identity(1),
        identity(2),
        identity(4),
        identity(5),
        identity(owner),
        identity(12),
        16,
        7,
        8,
        RentSplitV2 {
            payer: identity(rent_payer),
            refundable_live_principal: 100,
            permanent_tombstone_principal: 80,
            donation_floor: 3,
        },
        DeletableRentOwnerV1::from_persisted(
            identity(rent_payer.checked_add(1).unwrap()),
            90,
            2,
        )
        .unwrap(),
        &Sha,
    )
    .unwrap();
    let position = AuthenticatedPositionV3 {
        account: id(position_account),
        general_market_runtime: id(12),
        semantic: founding.position(),
        semantic_id: founding.position_semantic_id().bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    project_general_position_replay_prestate_v1(
        Id32::new(id(replay_account)).unwrap(),
        8,
        0,
        founding.replay_body(),
        position,
        &Sha,
    )
    .unwrap()
}

fn founding_general_prestate() -> GeneralPositionReplayPrestateV1 {
    founding_general_prestate_for(41, 42, 40, 43)
}

fn next_general_prestate(
    previous: GeneralPositionReplayPrestateV1,
    position_poststate: clutch_owner_settlement::PositionSettlementPoststateV3,
    replay: GeneralReplayTransitionPlanV1,
) -> GeneralPositionReplayPrestateV1 {
    let position = AuthenticatedPositionV3 {
        account: position_poststate.account,
        general_market_runtime: position_poststate.general_market_runtime,
        semantic: position_poststate.semantic,
        semantic_id: replay.position_poststate_semantic_id().bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    project_general_position_replay_prestate_v1(
        previous.replay_account(),
        previous.replay_bump(),
        replay.next_sequence(),
        replay.replay_poststate_body(),
        position,
        &Sha,
    )
    .unwrap()
}

fn advance_general_fields(
    previous: GeneralPositionReplayPrestateV1,
    fields: PositionV3Fields,
    kind: clutch_general_v2_contract::GeneralReplayTransitionKindV1,
    identity_byte: u8,
) -> GeneralPositionReplayPrestateV1 {
    let position = previous.position();
    let semantic = PositionAccountV3::new(fields).unwrap();
    let poststate = clutch_owner_settlement::PositionSettlementPoststateV3 {
        account: position.account,
        general_market_runtime: position.general_market_runtime,
        prestate_semantic_id: position.semantic_id,
        semantic,
    };
    let transition = clutch_general_v2_contract::project_general_replay_transition_v1(
        previous,
        poststate,
        kind,
        Id32::new(id(identity_byte)).unwrap(),
        Id32::new(id(identity_byte.checked_add(1).unwrap())).unwrap(),
        &Sha,
    )
    .unwrap();
    next_general_prestate(previous, poststate, transition)
}

#[test]
fn exact_pair_is_required_and_replay_is_linear() {
    let one = state()
        .admit_reservation(1, 10, id(20), id(21), id(31), &Sha)
        .unwrap();
    assert_eq!(one.replay.next_action_sequence(), 2);
    assert_eq!(
        one.admit_reservation(1, 11, id(22), id(23), id(32), &Sha),
        Err(DirectMarketErrorV1::Replay)
    );
    let two = one
        .admit_reservation(2, 11, id(22), id(23), id(32), &Sha)
        .unwrap();
    let frozen = two.freeze(3, 20, id(24), id(25), &Sha).unwrap();
    assert_eq!(frozen.root.phase(), DirectRootPhaseV1::SubmissionOpen);
    assert_eq!(frozen.root.live_reservations(), 2);
}

#[test]
fn root_and_permanent_replay_semantic_bodies_round_trip_exactly() {
    let value = state();
    let root_body = encode_direct_market_root_body_v1(value.root).unwrap();
    let decoded_root = decode_direct_market_root_body_v1(&root_body).unwrap();
    assert_eq!(decoded_root, value.root);
    let replay_body = encode_direct_action_replay_body_v1(value.replay, value.root).unwrap();
    assert_eq!(
        decode_direct_action_replay_body_v1(&replay_body, decoded_root),
        Ok(value.replay)
    );
}

#[test]
fn schedule_boundaries_are_owned_by_pure_transitions() {
    assert_eq!(
        state().admit_reservation(1, 9, id(20), id(21), id(31), &Sha),
        Err(DirectMarketErrorV1::WrongPhase)
    );
    let one = state()
        .admit_reservation(1, 10, id(20), id(21), id(31), &Sha)
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
        .admit_reservation(1, 10, id(20), id(20), id(31), &Sha)
        .unwrap()
        .admit_reservation(2, 11, id(22), id(22), id(32), &Sha)
        .unwrap()
        .freeze(3, 20, id(24), id(24), &Sha)
        .unwrap();
    assert_eq!(frozen.root.phase(), DirectRootPhaseV1::SubmissionOpen);
}

#[test]
fn direct_fee_shape_refuses_every_nonzero_rate_and_envelope() {
    let canonical = DirectZeroFeeVenueV1::canonical().unwrap();
    assert_eq!(canonical.validate(), Ok(()));
    assert_ne!(canonical.semantic_id(&Sha).unwrap(), [0; 32]);
    for hostile in [
        DirectZeroFeeVenueV1 { buyer_fee_bps: 1, ..canonical },
        DirectZeroFeeVenueV1 { seller_fee_bps: 1, ..canonical },
        DirectZeroFeeVenueV1 { max_buyer_fee_atoms: 1, ..canonical },
        DirectZeroFeeVenueV1 { max_seller_fee_atoms: 1, ..canonical },
    ] {
        assert_eq!(hostile.validate(), Err(DirectMarketErrorV1::MismatchedBinding));
    }
}

#[test]
fn direct_epoch_identity_is_pre_resolution_and_schedule_exact() {
    let value = state();
    let before = direct_epoch_semantics_id_v1(value.root.binding(), value.root.schedule(), &Sha)
        .unwrap();
    assert_eq!(before, value.root.binding().direct_epoch_semantics_id);
    let mut changed = value.root.schedule();
    changed.settlement_deadline_slot += 1;
    assert_ne!(
        direct_epoch_semantics_id_v1(value.root.binding(), changed, &Sha).unwrap(),
        before,
    );
}

#[test]
fn direct_schedule_is_clock_stamped_and_bounded_by_the_release_policy() {
    let schedule = DirectScheduleV1::canonical_from_foundation_slot(100).unwrap();
    assert_eq!(schedule.admission_opens_slot, 100);
    assert_eq!(schedule.admission_closes_slot, 164);
    assert_eq!(schedule.submission_closes_slot, 228);
    assert_eq!(schedule.selection_deadline_slot, 292);
    assert_eq!(schedule.settlement_deadline_slot, 356);
    assert_eq!(
        DirectScheduleV1::canonical_from_foundation_slot(u64::MAX),
        Err(DirectMarketErrorV1::Arithmetic),
    );
}

#[test]
fn cancellation_retires_only_the_named_archive_class() {
    let admitted = state()
        .admit_reservation(1, 10, id(20), id(21), id(31), &Sha)
        .unwrap();
    assert_eq!(
        admitted.cancel_reservation(2, 19, id(23), id(24), id(22), &Sha),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );
    let cancelled = admitted
        .cancel_reservation(2, 19, id(20), id(21), id(22), &Sha)
        .unwrap();
    assert_eq!(cancelled.root.admitted_reservations(), 1);
    assert_eq!(cancelled.root.live_reservations(), 0);
    assert_eq!(cancelled.root.retired_reservations(), 1);
    let frozen = cancelled.freeze(3, 20, id(24), id(25), &Sha).unwrap();
    assert_eq!(frozen.root.phase(), DirectRootPhaseV1::FrozenEmpty);
}

#[test]
fn root_refuses_a_nonzero_live_reservation_tail() {
    let mut root = state().root;
    root.reservation_accounts[1] = id(20);
    root.reservation_semantic_ids[1] = id(21);
    assert_eq!(root.validate(), Err(DirectMarketErrorV1::InvalidCount));
}

#[test]
fn same_phase_selection_work_changes_only_permanent_replay() {
    let two = state()
        .admit_reservation(1, 10, id(20), id(21), id(31), &Sha)
        .unwrap()
        .admit_reservation(2, 11, id(22), id(23), id(32), &Sha)
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
        .terminalize(2, 20, DirectTerminalReasonV1::EmptyLapse, id(20), id(22), &Sha)
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
fn retirement_builder_derives_sorted_sources_and_coalesced_refunds() {
    let transfer = build_direct_retirement_transfer_v1(
        [
            Some(DirectRetirementSourceV1 {
                account: id(21),
                rent: rent(30, 200, 7),
                observed_lamports: 220,
            }),
            None,
            Some(DirectRetirementSourceV1 {
                account: id(20),
                rent: rent(30, 100, 5),
                observed_lamports: 110,
            }),
            None,
            None,
        ],
        id(31),
    )
    .unwrap();
    assert_eq!(transfer, retirement());
    assert_eq!(
        build_direct_retirement_transfer_v1(
            [transfer.sources[0], transfer.sources[0], None, None, None],
            id(31),
        ),
        Err(DirectMarketErrorV1::IdentityAlias)
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
fn terminal_retirement_requires_the_exact_action_replay_archive() {
    let transfer = retirement();
    assert_eq!(
        require_terminal_retirement_source_v1(&transfer, id(20), rent(30, 100, 5)),
        Ok(())
    );
    assert_eq!(
        require_terminal_retirement_source_v1(&transfer, id(22), rent(32, 80, 2)),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );
    assert_eq!(
        require_terminal_retirement_source_v1(&transfer, id(20), rent(30, 99, 5)),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );
}

#[test]
fn retirement_builder_accepts_the_complete_five_archive_family() {
    let transfer = build_direct_retirement_transfer_v1(
        [
            Some(DirectRetirementSourceV1 {
                account: id(20), rent: rent(40, 100, 1), observed_lamports: 105,
            }),
            Some(DirectRetirementSourceV1 {
                account: id(21), rent: rent(41, 101, 2), observed_lamports: 107,
            }),
            Some(DirectRetirementSourceV1 {
                account: id(22), rent: rent(42, 102, 3), observed_lamports: 109,
            }),
            Some(DirectRetirementSourceV1 {
                account: id(23), rent: rent(43, 103, 4), observed_lamports: 111,
            }),
            Some(DirectRetirementSourceV1 {
                account: id(24), rent: rent(44, 104, 5), observed_lamports: 113,
            }),
        ],
        id(31),
    )
    .unwrap();
    assert_eq!(transfer.source_count, 5);
    assert_eq!(transfer.refund_count, 5);
    assert_eq!(transfer.surplus_lamports, 35);
}

#[test]
fn zero_price_buyer_reservation_is_exact_and_counts_the_child() {
    let plan = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        state().root,
        position(100, 0),
        None,
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
        &Sha,
    )
    .unwrap();
    assert_eq!(plan.reservation.phase(), DirectReservationPhaseV1::Active);
    assert_eq!(plan.reservation.reserved_cash_atoms(), 0);
    let fields = plan.position_poststate.semantic.fields();
    assert_eq!(fields.cash_atoms, 100);
    assert_eq!(fields.reserved_cash_atoms, 0);
    assert_eq!(fields.outstanding_reservations, 1);
    let body = encode_direct_reservation_body_v1(plan.reservation, state().root).unwrap();
    assert_eq!(
        decode_direct_reservation_body_v1(&body, state().root),
        Ok(plan.reservation)
    );
}

#[test]
fn reservation_refuses_rounding_and_debits_seller_eggs_exactly() {
    assert_eq!(
        prepare_direct_reservation_admission_v1(
            &AllowReservation,
            state().root,
            position(100, 0),
            None,
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
            &Sha,
        ),
        Err(DirectMarketErrorV1::InexactCashConversion)
    );
    let seller = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        state().root,
        position(0, 20),
        None,
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
        &Sha,
    )
    .unwrap();
    let fields = seller.position_poststate.semantic.fields();
    assert_eq!(fields.native_eggs[0], 13);
    assert_eq!(fields.outstanding_reservations, 1);
}

#[test]
fn reservation_expiry_uses_the_product_direct_occurrence_not_market_generation() {
    let mut value = state();
    value.root.binding.family_admission_sequence = 7;
    assert_eq!(value.root.binding.direct_window_index(), Ok(8));
    assert_eq!(
        prepare_direct_reservation_admission_v1(
            &AllowReservation,
            value.root,
            position(100, 0),
            None,
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
            &Sha,
        ),
        Err(DirectMarketErrorV1::MismatchedBinding),
    );
}

#[test]
fn admission_requires_current_generation_and_the_exact_compatible_peer() {
    assert_eq!(
        prepare_direct_reservation_admission_v1(
            &AllowReservation,
            state().root,
            position(100, 0),
            None,
            id(50),
            id(51),
            Side::Buy,
            0,
            10,
            0,
            PartialPolicy::Allow,
            0,
            0,
            rent(52, 70, 2),
            &Sha,
        ),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );

    let buyer = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        state().root,
        position(100, 0),
        None,
        id(50),
        id(51),
        Side::Buy,
        0,
        10,
        0,
        PartialPolicy::Allow,
        1,
        0,
        rent(52, 70, 2),
        &Sha,
    )
    .unwrap();
    let after_buyer = state()
        .admit_reservation(
            1,
            10,
            buyer.reservation.account(),
            buyer.reservation.semantic_id(&Sha).unwrap(),
            id(81),
            &Sha,
        )
        .unwrap();
    assert_eq!(
        prepare_direct_reservation_admission_v1(
            &AllowReservation,
            after_buyer.root,
            position(0, 10),
            None,
            id(60),
            id(61),
            Side::Sell,
            0,
            10,
            0,
            PartialPolicy::Allow,
            1,
            0,
            rent(62, 70, 2),
            &Sha,
        ),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );
    assert_eq!(
        prepare_direct_reservation_admission_v1(
            &AllowReservation,
            after_buyer.root,
            position(0, 10),
            Some(buyer.reservation),
            id(60),
            id(61),
            Side::Sell,
            1,
            10,
            0,
            PartialPolicy::Allow,
            1,
            0,
            rent(62, 70, 2),
            &Sha,
        ),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );
    assert_eq!(
        prepare_direct_reservation_admission_v1(
            &AllowReservation,
            after_buyer.root,
            position(100, 0),
            Some(buyer.reservation),
            id(60),
            id(61),
            Side::Buy,
            0,
            10,
            0,
            PartialPolicy::Allow,
            1,
            0,
            rent(62, 70, 2),
            &Sha,
        ),
        Err(DirectMarketErrorV1::MismatchedBinding)
    );
}

fn direct_domain_and_price(selected_price: u64) -> (EconomicDomainV2, PricePreconditionV2) {
    let binding = state().root.binding();
    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: binding.market_instance_id,
        epoch_semantics_digest: binding.direct_epoch_semantics_id,
        relation_policy_digest: binding.relation_policy_id,
        price_policy_digest: binding.price_policy_id,
        epoch_index: binding.direct_window_index().unwrap(),
        outcome_count: binding.outcome_count,
        price_scale: binding.price_scale,
    };
    let mut prices = [0u64; MAX_OUTCOMES];
    prices[0] = selected_price;
    prices[1] = 1_000u64.checked_sub(selected_price).unwrap();
    let price = PricePreconditionV2 {
        policy_digest: id(15),
        semantic_price_digest: price_semantics_digest_v2(&domain, &prices).unwrap(),
        prices,
    };
    (domain, price)
}

fn direct_domain_and_zero_price() -> (EconomicDomainV2, PricePreconditionV2) {
    direct_domain_and_price(0)
}

fn candidate(fill: u64) -> DirectEconomicCandidateV1 {
    DirectEconomicCandidateV1 {
        fills: [fill, fill],
        honored_aon_mask: 0,
    }
}

fn submission_open_pair_with_endpoints() -> (
    crate::selection_v1::DirectSelectionFreezePlanV1,
    [Option<DirectEndpointPrestateV1>; 2],
) {
    let buyer_general = founding_general_prestate_for(41, 42, 40, 43);
    let seller_general = founding_general_prestate_for(45, 46, 47, 48);
    let mut seller_cash_fields = seller_general.position().semantic.fields();
    seller_cash_fields.cash_atoms = 10;
    let seller_cash = advance_general_fields(
        seller_general,
        seller_cash_fields,
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::Endow,
        92,
    );
    let mut seller_fields = seller_cash.position().semantic.fields();
    seller_fields.cash_atoms = 0;
    seller_fields.native_eggs = [10; MAX_OUTCOMES];
    let seller_funded = advance_general_fields(
        seller_cash,
        seller_fields,
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::Split,
        94,
    );
    let buyer = prepare_direct_reservation_admission_with_replay_v1(
        &AllowReservation,
        state(),
        buyer_general,
        None,
        1,
        10,
        DirectReservationOrderInputV1 {
            reservation_account: id(50),
            order_id: id(51),
            side: Side::Buy,
            outcome: 0,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_price_units_per_egg: 0,
            rent: rent(52, 70, 2),
        },
        &Sha,
    )
    .unwrap();
    let seller = prepare_direct_reservation_admission_with_replay_v1(
        &AllowReservation,
        buyer.state,
        seller_funded,
        Some(buyer.reservation),
        2,
        11,
        DirectReservationOrderInputV1 {
            reservation_account: id(60),
            order_id: id(61),
            side: Side::Sell,
            outcome: 0,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_price_units_per_egg: 0,
            rent: rent(62, 70, 2),
        },
        &Sha,
    )
    .unwrap();
    let buyer_post = next_general_prestate(
        buyer_general,
        buyer.position_poststate,
        buyer.replay_transition,
    );
    let seller_post = next_general_prestate(
        seller_funded,
        seller.position_poststate,
        seller.replay_transition,
    );
    let (domain, price) = direct_domain_and_zero_price();
    let frozen = prepare_direct_selection_freeze_v1(
        &AllowFreeze,
        seller.state,
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
    (
        frozen,
        [
            Some(DirectEndpointPrestateV1 {
                reservation: buyer.reservation,
                position_replay: buyer_post,
            }),
            Some(DirectEndpointPrestateV1 {
                reservation: seller.reservation,
                position_replay: seller_post,
            }),
        ],
    )
}

#[test]
fn empty_action8_and_missed_verification_deadline_are_total_no_trade_paths() {
    let (frozen, endpoints) = submission_open_pair_with_endpoints();
    let at_deadline = prepare_direct_economic_terminal_v1(
        &AllowEconomicTerminal,
        frozen.state,
        frozen.selection,
        endpoints,
        DirectTerminalReasonV1::UnselectedLapse,
        4,
        40,
        &Sha,
    )
    .unwrap();
    assert_eq!(
        at_deadline.state.root.terminal_reason(),
        Some(DirectTerminalReasonV1::UnselectedLapse)
    );

    let begun = begin_direct_candidate_verification_v1(
        frozen.state,
        frozen.selection,
        4,
        30,
        &Sha,
    )
    .unwrap();
    let no_candidate = prepare_direct_economic_terminal_v1(
        &AllowEconomicTerminal,
        begun.state,
        begun.selection,
        endpoints,
        DirectTerminalReasonV1::NoCandidate,
        5,
        31,
        &Sha,
    )
    .unwrap();
    assert_eq!(
        no_candidate.state.root.terminal_reason(),
        Some(DirectTerminalReasonV1::NoCandidate)
    );
    assert_eq!(no_candidate.endpoint_count, 2);
    assert_eq!(
        no_candidate.endpoints[0].unwrap().reservation_post.phase(),
        DirectReservationPhaseV1::Lapsed
    );
    assert_eq!(
        no_candidate.endpoints[1].unwrap().reservation_post.phase(),
        DirectReservationPhaseV1::Lapsed
    );
    let buyer_post = no_candidate.endpoints[0].unwrap().position_poststate.semantic.fields();
    let seller_post = no_candidate.endpoints[1].unwrap().position_poststate.semantic.fields();
    assert_eq!(buyer_post.cash_atoms, 0);
    assert_eq!(buyer_post.native_eggs[0], 0);
    assert_eq!(buyer_post.outstanding_reservations, 0);
    assert_eq!(seller_post.cash_atoms, 0);
    assert_eq!(seller_post.native_eggs[0], 10);
    assert_eq!(seller_post.outstanding_reservations, 0);
}

#[test]
fn complete_selection_reverifies_and_ranks_exact_zero_price_pair() {
    let initial = state();
    let buyer = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        initial.root,
        position(100, 0),
        None,
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
        &Sha,
    )
    .unwrap();
    let after_buyer = initial
        .admit_reservation(
            1, 10, buyer.reservation.account(), buyer.reservation.semantic_id(&Sha).unwrap(),
            id(81), &Sha,
        )
        .unwrap();
    let seller = prepare_direct_reservation_admission_v1(
        &AllowReservation,
        after_buyer.root,
        position(0, 10),
        Some(buyer.reservation),
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
        &Sha,
    )
    .unwrap();
    let after_seller = after_buyer
        .admit_reservation(
            2, 11, seller.reservation.account(), seller.reservation.semantic_id(&Sha).unwrap(),
            id(82), &Sha,
        )
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
    let third = submit_direct_candidate_v1(second.state, second.selection, 6, 23, candidate(8), &Sha).unwrap();
    let fourth = submit_direct_candidate_v1(third.state, third.selection, 7, 24, candidate(6), &Sha).unwrap();
    assert_eq!(fourth.selection.candidate_count(), 3);
    assert_eq!(fourth.selection.candidate(0).unwrap(), candidate(10));
    assert_eq!(fourth.selection.candidate(1).unwrap(), candidate(8));
    assert_eq!(fourth.selection.candidate(2).unwrap(), candidate(6));
    let begun = begin_direct_candidate_verification_v1(fourth.state, fourth.selection, 8, 30, &Sha).unwrap();
    let verified_first = verify_next_direct_candidate_v1(begun.state, begun.selection, 9, 31, &Sha).unwrap();
    let verified_second = verify_next_direct_candidate_v1(verified_first.state, verified_first.selection, 10, 32, &Sha).unwrap();
    let verified_third = verify_next_direct_candidate_v1(verified_second.state, verified_second.selection, 11, 33, &Sha).unwrap();
    let selected = finalize_direct_selection_v1(verified_third.state, verified_third.selection, 12, 34, &Sha).unwrap();
    let pair = selected.selection.selected_pair().unwrap();
    assert_eq!(pair.quantity(), 10);
    assert_eq!(pair.consideration_cash_atoms(), 0);
    let body = encode_direct_selection_body_v1(selected.selection, selected.state.root).unwrap();
    assert_eq!(
        decode_direct_selection_body_v1(&body, selected.state.root),
        Ok(selected.selection)
    );
}

#[test]
fn selection_refuses_missing_extra_duplicate_and_partial_traversal() {
    let initial = state();
    let buyer = prepare_direct_reservation_admission_v1(
        &AllowReservation, initial.root, position(100, 0), None, id(50), id(51), Side::Buy,
        0, 10, 0, PartialPolicy::Allow, 7, 0, rent(52, 70, 2), &Sha,
    ).unwrap();
    let after_buyer = initial.admit_reservation(
        1, 10, id(50), buyer.reservation.semantic_id(&Sha).unwrap(), id(81), &Sha,
    ).unwrap();
    let seller = prepare_direct_reservation_admission_v1(
        &AllowReservation, after_buyer.root, position(0, 10), Some(buyer.reservation),
        id(60), id(61), Side::Sell, 0, 10, 0, PartialPolicy::Allow, 7, 0,
        rent(62, 70, 2), &Sha,
    ).unwrap();
    let after_seller = after_buyer.admit_reservation(
        2, 11, id(60), seller.reservation.semantic_id(&Sha).unwrap(), id(82), &Sha,
    ).unwrap();
    let (domain, price) = direct_domain_and_zero_price();
    let (_, substituted_price) = direct_domain_and_price(500);
    assert_eq!(
        prepare_direct_selection_freeze_v1(
            &AllowFreeze,
            after_seller,
            3,
            20,
            id(70),
            rent(71, 80, 3),
            [Some(seller.reservation), Some(buyer.reservation)],
            domain,
            substituted_price,
            &Sha,
        ),
        Err(DirectMarketErrorV1::MismatchedBinding),
    );
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

#[test]
fn zero_price_admission_and_cancel_advance_gen1_and_refund_principal_only() {
    let general = founding_general_prestate();
    let admitted = prepare_direct_reservation_admission_with_replay_v1(
        &AllowReservation,
        state(),
        general,
        None,
        1,
        10,
        DirectReservationOrderInputV1 {
            reservation_account: id(50),
            order_id: id(51),
            side: Side::Buy,
            outcome: 0,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_price_units_per_egg: 0,
            rent: rent(52, 70, 2),
        },
        &Sha,
    )
    .unwrap();
    assert_eq!(
        admitted.replay_transition.kind(),
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::DirectMarketAdmitBuyer
    );
    assert_eq!(
        admitted.position_poststate.semantic.fields().outstanding_reservations,
        1
    );
    let admitted_general = next_general_prestate(
        general,
        admitted.position_poststate,
        admitted.replay_transition,
    );
    let cancelled = prepare_direct_reservation_cancel_v1(
        &AllowCancel,
        admitted.state,
        admitted.reservation,
        admitted_general,
        73,
        2,
        19,
        &Sha,
    )
    .unwrap();
    assert_eq!(
        cancelled.endpoint.replay_transition.kind(),
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::DirectMarketCancelBuyer
    );
    assert_eq!(
        cancelled.endpoint.position_poststate.semantic.fields().outstanding_reservations,
        0
    );
    assert_eq!(cancelled.retirement.refunds[0].unwrap().lamports, 70);
    assert_eq!(cancelled.retirement.surplus_lamports, 3);
    assert_eq!(cancelled.state.root.live_reservations(), 0);
    assert_eq!(cancelled.state.root.retired_reservations(), 1);
}

#[test]
fn empty_lapse_terminalizes_the_complete_one_reservation_prefix() {
    let general = founding_general_prestate();
    let admitted = prepare_direct_reservation_admission_with_replay_v1(
        &AllowReservation,
        state(),
        general,
        None,
        1,
        10,
        DirectReservationOrderInputV1 {
            reservation_account: id(50),
            order_id: id(51),
            side: Side::Buy,
            outcome: 0,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_price_units_per_egg: 0,
            rent: rent(52, 70, 2),
        },
        &Sha,
    )
    .unwrap();
    let admitted_general = next_general_prestate(
        general,
        admitted.position_poststate,
        admitted.replay_transition,
    );
    let (domain, price) = direct_domain_and_zero_price();
    let frozen = prepare_direct_selection_freeze_v1(
        &AllowFreeze,
        admitted.state,
        2,
        20,
        id(70),
        rent(71, 80, 3),
        [Some(admitted.reservation), None],
        domain,
        price,
        &Sha,
    )
    .unwrap();
    let terminal = prepare_direct_economic_terminal_v1(
        &AllowEconomicTerminal,
        frozen.state,
        frozen.selection,
        [
            Some(DirectEndpointPrestateV1 {
                reservation: admitted.reservation,
                position_replay: admitted_general,
            }),
            None,
        ],
        DirectTerminalReasonV1::EmptyLapse,
        3,
        20,
        &Sha,
    )
    .unwrap();
    assert_eq!(terminal.endpoint_count, 1);
    assert_eq!(terminal.selection.phase(), DirectSelectionPhaseV1::Terminal);
    assert_eq!(
        terminal.endpoints[0].unwrap().reservation_post.phase(),
        DirectReservationPhaseV1::Lapsed
    );
    assert_eq!(
        terminal.endpoints[0]
            .unwrap()
            .position_poststate
            .semantic
            .fields()
            .outstanding_reservations,
        0
    );
    assert_eq!(
        terminal.state.replay.economic_terminal_receipt_id(),
        terminal.economic_terminal_receipt_id
    );
}

#[test]
fn missed_freeze_lapse_creates_and_terminalizes_the_complete_selection_once() {
    let general = founding_general_prestate();
    let admitted = prepare_direct_reservation_admission_with_replay_v1(
        &AllowReservation,
        state(),
        general,
        None,
        1,
        10,
        DirectReservationOrderInputV1 {
            reservation_account: id(50),
            order_id: id(51),
            side: Side::Buy,
            outcome: 0,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_price_units_per_egg: 0,
            rent: rent(52, 70, 2),
        },
        &Sha,
    )
    .unwrap();
    let admitted_general = next_general_prestate(
        general,
        admitted.position_poststate,
        admitted.replay_transition,
    );
    let (domain, price) = direct_domain_and_zero_price();
    assert_eq!(
        prepare_direct_missed_freeze_terminal_v1(
            &AllowFreeze,
            &AllowEconomicTerminal,
            admitted.state,
            id(70),
            rent(71, 80, 3),
            [Some(admitted.reservation), None],
            domain,
            price,
            [
                Some(DirectEndpointPrestateV1 {
                    reservation: admitted.reservation,
                    position_replay: admitted_general,
                }),
                None,
            ],
            2,
            29,
            &Sha,
        ),
        Err(DirectMarketErrorV1::WrongPhase)
    );
    let terminal = prepare_direct_missed_freeze_terminal_v1(
        &AllowFreeze,
        &AllowEconomicTerminal,
        admitted.state,
        id(70),
        rent(71, 80, 3),
        [Some(admitted.reservation), None],
        domain,
        price,
        [
            Some(DirectEndpointPrestateV1 {
                reservation: admitted.reservation,
                position_replay: admitted_general,
            }),
            None,
        ],
        2,
        30,
        &Sha,
    )
    .unwrap();
    assert_eq!(terminal.state.root.phase(), DirectRootPhaseV1::Terminal);
    assert_eq!(
        terminal.state.root.terminal_reason(),
        Some(DirectTerminalReasonV1::MissedFreezeLapse)
    );
    assert_eq!(terminal.state.root.selection_account(), id(70));
    assert_eq!(terminal.selection.phase(), DirectSelectionPhaseV1::Terminal);
    assert_eq!(terminal.endpoint_count, 1);
}

#[test]
fn selected_pair_moves_exact_cash_and_eggs_and_releases_full_reserves() {
    let buyer_founding = founding_general_prestate_for(41, 42, 40, 43);
    let mut buyer_fields = buyer_founding.position().semantic.fields();
    buyer_fields.cash_atoms = 100;
    let buyer_pre = advance_general_fields(
        buyer_founding,
        buyer_fields,
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::Endow,
        90,
    );

    let seller_founding = founding_general_prestate_for(45, 46, 47, 48);
    let mut seller_cash_fields = seller_founding.position().semantic.fields();
    seller_cash_fields.cash_atoms = 160;
    let seller_cash = advance_general_fields(
        seller_founding,
        seller_cash_fields,
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::Endow,
        92,
    );
    let mut seller_split_fields = seller_cash.position().semantic.fields();
    seller_split_fields.cash_atoms = 150;
    seller_split_fields.native_eggs = [10; MAX_OUTCOMES];
    let seller_pre = advance_general_fields(
        seller_cash,
        seller_split_fields,
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::Split,
        94,
    );

    let buyer = prepare_direct_reservation_admission_with_replay_v1(
        &AllowReservation,
        state(),
        buyer_pre,
        None,
        1,
        10,
        DirectReservationOrderInputV1 {
            reservation_account: id(50),
            order_id: id(51),
            side: Side::Buy,
            outcome: 0,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_price_units_per_egg: 500,
            rent: rent(52, 70, 2),
        },
        &Sha,
    )
    .unwrap();
    let seller = prepare_direct_reservation_admission_with_replay_v1(
        &AllowReservation,
        buyer.state,
        seller_pre,
        Some(buyer.reservation),
        2,
        11,
        DirectReservationOrderInputV1 {
            reservation_account: id(60),
            order_id: id(61),
            side: Side::Sell,
            outcome: 0,
            quantity: 10,
            minimum_fill: 0,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: 7,
            limit_price_units_per_egg: 500,
            rent: rent(62, 70, 2),
        },
        &Sha,
    )
    .unwrap();
    let buyer_admitted = next_general_prestate(
        buyer_pre,
        buyer.position_poststate,
        buyer.replay_transition,
    );
    let seller_admitted = next_general_prestate(
        seller_pre,
        seller.position_poststate,
        seller.replay_transition,
    );
    let (domain, price) = direct_domain_and_price(500);
    let frozen = prepare_direct_selection_freeze_v1(
        &AllowFreeze,
        seller.state,
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
    assert_eq!(
        submit_direct_candidate_v1(
            frozen.state,
            frozen.selection,
            4,
            21,
            candidate(5),
            &Sha,
        ),
        Err(DirectMarketErrorV1::DirectPair(
            clutch_batch::direct_pair_v1::DirectPairErrorV1::InexactCashConversion,
        )),
    );
    let submitted = submit_direct_candidate_v1(
        frozen.state,
        frozen.selection,
        4,
        21,
        candidate(10),
        &Sha,
    )
    .unwrap();
    let begun = begin_direct_candidate_verification_v1(
        submitted.state,
        submitted.selection,
        5,
        30,
        &Sha,
    )
    .unwrap();
    let verified = verify_next_direct_candidate_v1(
        begun.state,
        begun.selection,
        6,
        31,
        &Sha,
    )
    .unwrap();
    let selected = finalize_direct_selection_v1(
        verified.state,
        verified.selection,
        7,
        32,
        &Sha,
    )
    .unwrap();
    let terminal = prepare_direct_economic_terminal_v1(
        &AllowEconomicTerminal,
        selected.state,
        selected.selection,
        [
            Some(DirectEndpointPrestateV1 {
                reservation: seller.reservation,
                position_replay: seller_admitted,
            }),
            Some(DirectEndpointPrestateV1 {
                reservation: buyer.reservation,
                position_replay: buyer_admitted,
            }),
        ],
        DirectTerminalReasonV1::Settled,
        8,
        33,
        &Sha,
    )
    .unwrap();
    let buyer_post = terminal.endpoints[0].unwrap().position_poststate.semantic.fields();
    let seller_post = terminal.endpoints[1].unwrap().position_poststate.semantic.fields();
    assert_eq!(buyer_post.cash_atoms, 95);
    assert_eq!(buyer_post.reserved_cash_atoms, 0);
    assert_eq!(buyer_post.native_eggs[0], 10);
    assert_eq!(buyer_post.outstanding_reservations, 0);
    assert_eq!(seller_post.cash_atoms, 155);
    assert_eq!(seller_post.native_eggs[0], 0);
    assert_eq!(seller_post.outstanding_reservations, 0);
    assert_eq!(
        terminal.endpoints[0].unwrap().replay_transition.kind(),
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::DirectMarketSettleBuyer
    );
    assert_eq!(
        terminal.endpoints[1].unwrap().replay_transition.kind(),
        clutch_general_v2_contract::GeneralReplayTransitionKindV1::DirectMarketSettleSeller
    );
}
