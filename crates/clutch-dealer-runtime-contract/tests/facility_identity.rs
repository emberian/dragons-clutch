mod common;

use clutch_dealer_runtime_contract::*;

fn genesis(policy: &DealerPolicyV1) -> DealerFacilityGenesisV1 {
    DealerFacilityGenesisV1 {
        policy_id: policy.policy_id().unwrap(),
        sponsor: common::id(22),
        sponsor_refund_recipient: common::id(23),
        facility_nonce: 7,
    }
}

fn position(
    genesis: &DealerFacilityGenesisV1,
    policy: &DealerPolicyV1,
) -> DealerFacilityPositionV1 {
    DealerFacilityPositionV1 {
        policy_id: genesis.policy_id,
        facility_id: genesis.facility_id().unwrap().untyped(),
        market_instance_v2_id: policy.market_instance_v2_id,
        collateral_mint: policy.collateral_mint,
        token_program: policy.token_program,
        authority_state_account_id: common::id(60),
        replay_account_id: common::id(27),
        phase: DealerFacilityPositionPhaseV1::Idle,
        generation: 0,
        cash_atoms: 500,
        eggs: [0; MAX_OUTCOMES],
    }
}

fn binding(
    genesis: &DealerFacilityGenesisV1,
    policy: &DealerPolicyV1,
    position: &DealerFacilityPositionV1,
) -> FacilityPositionBindingV1 {
    FacilityPositionBindingV1 {
        facility_id: genesis.facility_id().unwrap().untyped(),
        policy_id: genesis.policy_id,
        market_instance_v2_id: policy.market_instance_v2_id,
        facility_position_semantic_id: position.position_id().unwrap(),
        facility_position_account_id: common::id(26),
        facility_replay_account_id: common::id(27),
        dealer_state_account_id: common::id(60),
        initial_position_generation: 0,
    }
}

fn initial_state(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
) -> DealerStateV1 {
    let mut state = common::funding_state();
    state.facility_id = binding.facility_id;
    state.facility_position_id = binding.facility_position_semantic_id;
    state.facility_position_account_id = binding.facility_position_account_id;
    state.facility_replay_account_id = binding.facility_replay_account_id;
    state.sponsor = genesis.sponsor;
    state.sponsor_refund_recipient = genesis.sponsor_refund_recipient;
    state.lp_page_head_id = Id::ZERO;
    state.lp_page_set_root = Id::ZERO;
    state.child_sequence = 0;
    state.total_shares = 0;
    state.children.lp_pages = 0;
    state.children.live_lp_positions = 0;
    state.children.fee_budgets = 0;
    state.children.liveness_budgets = 0;
    state
}

#[test]
fn initialization_join_is_exact_and_substitution_safe() {
    let policy = common::policy();
    let genesis = genesis(&policy);
    let position = position(&genesis, &policy);
    let binding = binding(&genesis, &policy, &position);
    let state = initial_state(&genesis, &binding);
    assert_eq!(
        validate_facility_initialization_v1(
            &genesis,
            &binding,
            &policy,
            binding.dealer_state_account_id,
            &position,
            &state,
        ),
        binding.binding_id()
    );
    let mut position_bytes = [0; DEALER_FACILITY_POSITION_BYTES_V1];
    position.encode_into(&mut position_bytes).unwrap();
    assert_eq!(
        DealerFacilityPositionV1::decode(&position_bytes),
        Ok(position)
    );

    let mut wrong_authority = position;
    wrong_authority.authority_state_account_id = common::id(61);
    assert_eq!(
        wrong_authority.validate_against(&binding, &policy),
        Err(Error::MismatchedBinding)
    );

    let mut noncanonical_eggs = position;
    noncanonical_eggs.eggs[2] = 1;
    assert_eq!(
        noncanonical_eggs.validate_against(&binding, &policy),
        Err(Error::MismatchedBinding)
    );

    let mut wrong_position = state;
    wrong_position.facility_position_id = common::id(99);
    assert_eq!(
        validate_facility_initialization_v1(
            &genesis,
            &binding,
            &policy,
            binding.dealer_state_account_id,
            &position,
            &wrong_position,
        ),
        Err(Error::MismatchedBinding)
    );

    let mut reserved_page = state;
    reserved_page.children.lp_pages = 1;
    reserved_page.lp_page_head_id = common::id(90);
    reserved_page.lp_page_set_root = common::id(91);
    assert_eq!(
        validate_facility_initialization_v1(
            &genesis,
            &binding,
            &policy,
            binding.dealer_state_account_id,
            &position,
            &reserved_page,
        ),
        Err(Error::MismatchedBinding)
    );

    assert_eq!(
        validate_facility_initialization_v1(
            &genesis,
            &binding,
            &policy,
            common::id(61),
            &position,
            &state,
        ),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn root_tombstone_requires_the_exhaustive_terminal_state_and_rent_split() {
    let policy = common::policy();
    let genesis = genesis(&policy);
    let position = position(&genesis, &policy);
    let binding = binding(&genesis, &policy, &position);
    let mut terminal = initial_state(&genesis, &binding);
    terminal.phase = DealerPhaseV1::Closed;
    terminal.sponsor_capital_disposition = SponsorCapitalDispositionV1::Refunded;
    terminal.children = DealerChildCountsV1::default();
    terminal.generation = 9;
    terminal.child_sequence = 17;
    let tombstone = DealerRootTombstoneV1 {
        policy_id: terminal.policy_id,
        facility_id: terminal.facility_id,
        facility_position_binding_id: binding.binding_id_for(&genesis, &policy).unwrap().untyped(),
        terminal_state_id: terminal.state_content_id().unwrap(),
        dealer_state_account_id: binding.dealer_state_account_id,
        rent_payer: terminal.rent.payer,
        neutral_sink: terminal.rent.neutral_sink,
        terminal_generation: terminal.generation,
        terminal_child_sequence: terminal.child_sequence,
        refunded_live_principal: terminal.rent.refundable_live_principal,
        permanent_tombstone_principal: terminal.rent.permanent_tombstone_principal,
        creation_donation_floor: terminal.rent.donation_floor,
    };
    assert_eq!(
        tombstone.validate_retirement(&genesis, &binding, &policy, &terminal),
        Ok(())
    );
    let mut bytes = [0; DEALER_ROOT_TOMBSTONE_BYTES_V1];
    tombstone.encode_into(&mut bytes).unwrap();
    assert_eq!(DealerRootTombstoneV1::decode(&bytes), Ok(tombstone));

    let mut open_child = terminal;
    open_child.children.fee_budgets = 1;
    assert!(tombstone
        .validate_retirement(&genesis, &binding, &policy, &open_child)
        .is_err());
    let mut swapped_payer = tombstone;
    swapped_payer.rent_payer = common::id(99);
    assert_eq!(
        swapped_payer.validate_retirement(&genesis, &binding, &policy, &terminal),
        Err(Error::MismatchedBinding)
    );
}
