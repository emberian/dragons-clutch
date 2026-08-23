mod common;

use core::fmt::Debug;

use clutch_dealer_runtime_contract::*;

fn exact_round_trip<T>(value: T)
where
    T: FixedCodec + Copy + Debug + Eq,
{
    let mut bytes = vec![0u8; T::ENCODED_LEN];
    value.encode_into(&mut bytes).unwrap();
    assert_eq!(T::decode(&bytes).unwrap(), value);
    assert_eq!(T::decode(&bytes[..bytes.len() - 1]), Err(Error::Truncated));
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(T::decode(&trailing), Err(Error::TrailingBytes));

    let mut bad_magic = bytes.clone();
    bad_magic[0] ^= 1;
    assert_eq!(T::decode(&bad_magic), Err(Error::BadMagic));

    let mut bad_version = bytes.clone();
    bad_version[8] ^= 1;
    assert_eq!(T::decode(&bad_version), Err(Error::BadVersion));

    let mut bad_reserved = bytes;
    bad_reserved[10] = 1;
    assert_eq!(T::decode(&bad_reserved), Err(Error::NonCanonicalPadding));
}

#[test]
fn every_body_has_an_exact_hostile_codec() {
    exact_round_trip(common::policy());
    exact_round_trip(common::funding_state());
    exact_round_trip(common::lp_page());
    exact_round_trip(common::lease());
    exact_round_trip(common::collect_pot());
    exact_round_trip(common::deliver_pot());
    exact_round_trip(common::finalizing_pot());
    exact_round_trip(common::fee_budget());
    exact_round_trip(common::liveness_budget());
}

#[test]
fn policy_refuses_bad_simplex_padding_box_lot_and_schedule() {
    let mut value = common::policy();
    value.initial_price_weights[0] = 0;
    assert_eq!(value.validate(), Err(Error::InvalidParameter));

    let mut value = common::policy();
    value.initial_price_weights[2] = 1;
    assert_eq!(value.validate(), Err(Error::NonCanonicalPadding));

    let mut value = common::policy();
    value.max_net_buy[0] = 101;
    assert_eq!(value.validate(), Err(Error::InvalidParameter));

    let mut value = common::policy();
    value.trading_close_slot = value.trading_open_slot;
    assert_eq!(value.validate(), Err(Error::InvalidSchedule));

    let mut value = common::policy();
    value.max_net_buy[0] = 1_000;
    assert_eq!(value.validate(), Err(Error::InvalidParameter));
}

#[test]
fn policy_recomputes_capital_and_signed_inventory_rules() {
    let policy = common::policy();
    assert_eq!(policy.minimum_sponsor_subsidy().unwrap(), 250);
    assert_eq!(policy.minimum_sponsor_capital().unwrap(), 250);

    let mut inventory = [0i64; MAX_OUTCOMES];
    inventory[0] = 10;
    inventory[1] = -10;
    assert_eq!(policy.validate_net_sold(&inventory), Ok(()));

    inventory[0] = 11;
    assert_eq!(
        policy.validate_net_sold(&inventory),
        Err(Error::InvalidParameter)
    );
    inventory[0] = 110;
    assert_eq!(
        policy.validate_net_sold(&inventory),
        Err(Error::InvalidParameter)
    );
    inventory[0] = 0;
    inventory[2] = 10;
    assert_eq!(
        policy.validate_net_sold(&inventory),
        Err(Error::NonCanonicalPadding)
    );
}

#[test]
fn dealer_state_contains_only_control_and_inventory_facts() {
    let state = common::trading_state();
    assert_eq!(state.validate_against_policy(&common::policy()), Ok(()));
    assert_eq!(DEALER_STATE_BYTES_V1, 680);

    let mut underfunded = state;
    underfunded.sponsor_capital_atoms = 249;
    assert_eq!(
        underfunded.validate_against_policy(&common::policy()),
        Err(Error::MismatchedBinding)
    );

    let mut bad_lot = state;
    bad_lot.net_sold[0] = 11;
    assert_eq!(
        bad_lot.validate_against_policy(&common::policy()),
        Err(Error::InvalidParameter)
    );
}

#[test]
fn funding_generation_advances_and_trading_queue_quorum_is_canonical() {
    let mut funding = common::funding_state();
    funding.generation = 9;
    assert_eq!(funding.validate_against_policy(&common::policy()), Ok(()));

    let mut trading = common::trading_state();
    trading.queued_shares = 5;
    assert_eq!(trading.validate(), Ok(()));
    assert_eq!(
        trading.validate_against_policy(&common::policy()),
        Err(Error::InvalidPhase)
    );
}

#[test]
fn state_refuses_phase_and_child_graph_aliases() {
    let mut state = common::funding_state();
    state.children.leases = 1;
    assert_eq!(state.validate(), Err(Error::InvalidChildGraph));

    let mut state = common::trading_state();
    state.children.leases = 2;
    assert_eq!(state.validate(), Err(Error::InvalidChildGraph));

    let mut state = common::funding_state();
    state.children.lp_pages = 0;
    assert_eq!(state.validate(), Err(Error::InvalidChildGraph));

    let mut state = common::funding_state();
    state.lp_page_head_id = Id::ZERO;
    assert_eq!(state.validate(), Err(Error::ZeroIdentity));

    let mut state = common::funding_state();
    state.sponsor = state.facility_id;
    assert_eq!(state.validate(), Err(Error::InvalidParameter));

    let mut state = common::funding_state();
    state.facility_replay_account_id = state.facility_position_account_id;
    assert_eq!(state.validate(), Err(Error::InvalidParameter));

    let mut state = common::funding_state();
    state.sponsor = common::policy().neutral_sink;
    assert_eq!(state.validate(), Ok(()));
    assert_eq!(
        state.validate_against_policy(&common::policy()),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn paged_lp_set_lifts_eight_owner_cap_but_stays_canonical() {
    assert_eq!(LP_ENTRIES_PER_PAGE, 16);
    assert!(usize::try_from(MAX_LP_PAGES).unwrap() * LP_ENTRIES_PER_PAGE > 8);

    let mut page = common::lp_page();
    page.entries[1] = LpEntryV1 {
        owner: common::id(41),
        shares: 1,
        queued_shares: 1,
        terminal_claim_atoms: 0,
        claimed: false,
    };
    page.entry_count = 2;
    assert_eq!(page.validate(), Ok(()));

    page.entries.swap(0, 1);
    assert_eq!(page.validate(), Err(Error::InvalidLpPage));

    let mut page = common::lp_page();
    page.entries[1] = LpEntryV1 {
        owner: common::id(41),
        shares: 1,
        queued_shares: 0,
        terminal_claim_atoms: 0,
        claimed: false,
    };
    assert_eq!(page.validate(), Err(Error::NonCanonicalPadding));

    let mut page = common::lp_page();
    page.next_page_ordinal = 1;
    assert_eq!(page.validate(), Err(Error::InvalidLpPage));

    let mut tail = common::lp_page();
    tail.page_ordinal = MAX_LP_PAGES - 1;
    tail.next_page_ordinal = MAX_LP_PAGES;
    tail.entry_count = LP_ENTRIES_PER_PAGE as u8;
    for (index, entry) in tail.entries.iter_mut().enumerate() {
        *entry = LpEntryV1 {
            owner: common::id(80 + u8::try_from(index).unwrap()),
            shares: 1,
            queued_shares: 0,
            terminal_claim_atoms: 0,
            claimed: false,
        };
    }
    assert_eq!(tail.validate(), Err(Error::InvalidLpPage));

    let mut policy_tail = common::lp_page();
    policy_tail.page_ordinal = common::policy().maximum_lp_pages - 1;
    policy_tail.next_page_ordinal = common::policy().maximum_lp_pages;
    policy_tail.entry_count = LP_ENTRIES_PER_PAGE as u8;
    for (index, entry) in policy_tail.entries.iter_mut().enumerate() {
        *entry = LpEntryV1 {
            owner: common::id(100 + u8::try_from(index).unwrap()),
            shares: 1,
            queued_shares: 0,
            terminal_claim_atoms: 0,
            claimed: false,
        };
    }
    assert_eq!(policy_tail.validate(), Ok(()));
    assert_eq!(
        policy_tail.validate_against_policy(&common::policy()),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn terminal_zero_claim_has_an_explicit_allocation_bit() {
    let mut page = common::lp_page();
    page.sealed = true;
    page.terminal_allocated = true;
    page.entries[0].claimed = true;
    assert_eq!(page.entries[0].terminal_claim_atoms, 0);
    assert_eq!(page.validate(), Ok(()));

    page.terminal_allocated = false;
    assert_eq!(page.validate(), Err(Error::InvalidLpPage));
}

#[test]
fn lease_is_exactly_one_generation_and_binds_final_candidate() {
    let lease = common::lease();
    assert_eq!(
        lease.validate_bindings(&common::policy(), &common::trading_state()),
        Ok(())
    );
    assert!(!lease.settlement_candidate_id.is_zero());
    assert!(!lease.curve_price_certificate_id.is_zero());

    let mut child_only_change = common::trading_state();
    child_only_change.child_sequence += 1;
    assert_eq!(
        lease.validate_bindings(&common::policy(), &child_only_change),
        Ok(())
    );

    let mut bad = lease;
    bad.post_generation = bad.pre_generation + 2;
    assert_eq!(bad.validate(), Err(Error::MismatchedBinding));

    let mut bad = lease;
    bad.collect_deadline_slot = bad.created_slot;
    assert_eq!(bad.validate(), Err(Error::InvalidSchedule));

    let mut bad = lease;
    bad.facility_position_pre_id = common::id(200);
    assert_eq!(
        bad.validate_bindings(&common::policy(), &common::trading_state()),
        Err(Error::MismatchedBinding)
    );
}

#[test]
fn pot_enforces_collect_then_deliver_then_finalize() {
    assert_eq!(common::collect_pot().validate(), Ok(()));
    assert_eq!(common::deliver_pot().validate(), Ok(()));
    assert_eq!(common::finalizing_pot().validate(), Ok(()));
    assert_eq!(
        common::finalizing_pot().validate_against_lease(&common::lease()),
        Ok(())
    );
    let mut expected_post = [0i64; MAX_OUTCOMES];
    expected_post[0] = 0;
    expected_post[1] = 0;
    assert_eq!(
        common::finalizing_pot().validate_transition(
            &common::policy(),
            &common::trading_state(),
            &common::lease(),
        ),
        Ok(expected_post)
    );

    let mut pot = common::collect_pot();
    pot.delivered_user_cash_atoms = 1;
    assert_eq!(pot.validate(), Err(Error::ConservationFailure));

    let mut pot = common::deliver_pot();
    pot.collected_user_cash_atoms -= 1;
    assert_eq!(pot.validate(), Err(Error::InvalidPhase));

    let mut pot = common::finalizing_pot();
    pot.delivered_user_eggs[1] -= 1;
    assert_eq!(pot.validate(), Err(Error::InvalidPhase));
}

#[test]
fn pot_refuses_a_conserved_but_wrong_curve_receipt() {
    let mut pot = common::collect_pot();
    pot.dealer_net_cash_out_atoms += 1;
    pot.user_cash_out_atoms += 1;
    assert_eq!(pot.validate(), Ok(()));
    assert_eq!(
        pot.validate_transition(
            &common::policy(),
            &common::trading_state(),
            &common::lease(),
        ),
        Err(Error::ConservationFailure)
    );
}

#[test]
fn pot_admits_a_valid_full_box_crossing_larger_than_one_inventory_cap() {
    let mut state = common::trading_state();
    state.net_sold = [0; MAX_OUTCOMES];
    state.net_sold[0] = 100;
    assert_eq!(state.validate_against_policy(&common::policy()), Ok(()));

    let mut pot = common::finalizing_pot();
    pot.user_cash_in_atoms = 0;
    pot.user_cash_out_atoms = 100;
    pot.dealer_net_cash_in_atoms = 0;
    pot.dealer_net_cash_out_atoms = 100;
    pot.facility_buy_eggs = [0; MAX_OUTCOMES];
    pot.facility_buy_eggs[0] = 200;
    pot.facility_sell_eggs = [0; MAX_OUTCOMES];
    pot.collected_user_cash_atoms = 0;
    pot.collected_user_eggs = pot.facility_buy_eggs;
    pot.delivered_user_cash_atoms = pot.user_cash_out_atoms;
    pot.delivered_user_eggs = [0; MAX_OUTCOMES];
    assert_eq!(pot.validate(), Ok(()));

    let mut expected_post = [0i64; MAX_OUTCOMES];
    expected_post[0] = -100;
    assert_eq!(
        pot.validate_transition(&common::policy(), &state, &common::lease()),
        Ok(expected_post)
    );
}

#[test]
fn pot_refuses_cursor_gross_mismatch_and_unnetted_egg_flow() {
    let mut pot = common::collect_pot();
    pot.collect_cursor = pot.row_count;
    assert_eq!(pot.validate(), Err(Error::InvalidPhase));

    let mut pot = common::collect_pot();
    pot.user_cash_out_atoms -= 1;
    assert_eq!(pot.validate(), Err(Error::ConservationFailure));

    let mut pot = common::collect_pot();
    pot.facility_sell_eggs[0] = 10;
    assert_eq!(pot.validate(), Err(Error::ConservationFailure));

    let mut pot = common::collect_pot();
    pot.facility_buy_eggs[2] = 10;
    assert_eq!(pot.validate(), Err(Error::NonCanonicalPadding));
}

#[test]
fn cursor_requests_are_contiguous_and_idempotent_without_bitmaps() {
    assert_eq!(
        classify_cursor_request(3, 8, 3, 6),
        Ok(CursorRequestV1::Advance { start: 3, end: 6 })
    );
    assert_eq!(
        classify_cursor_request(6, 8, 3, 6),
        Ok(CursorRequestV1::IdempotentRetry)
    );
    assert_eq!(
        classify_cursor_request(6, 8, 0, 2),
        Ok(CursorRequestV1::IdempotentRetry)
    );
    assert_eq!(
        classify_cursor_request(3, 8, 4, 6),
        Err(Error::MismatchedBinding)
    );
    assert_eq!(
        classify_cursor_request(3, 8, 2, 6),
        Err(Error::MismatchedBinding)
    );
    assert_eq!(
        classify_cursor_request(3, 8, 6, 2),
        Err(Error::InvalidParameter)
    );
    assert_eq!(
        classify_cursor_request(3, 8, 3, 9),
        Err(Error::InvalidParameter)
    );
}

#[test]
fn pot_custody_is_derived_at_each_exact_stage() {
    let collecting = common::collect_pot().derived_custody().unwrap();
    assert_eq!(collecting.cash_atoms, 1);
    assert_eq!(collecting.eggs[0], 0);
    assert_eq!(collecting.eggs[1], 10);

    let delivering = common::deliver_pot().derived_custody().unwrap();
    assert_eq!(delivering.cash_atoms, 21);
    assert_eq!(delivering.eggs[0], 10);
    assert_eq!(delivering.eggs[1], 10);

    let finalizing = common::finalizing_pot().derived_custody().unwrap();
    assert_eq!(finalizing.cash_atoms, 0);
    assert_eq!(finalizing.eggs[0], 10);
    assert_eq!(finalizing.eggs[1], 0);

    // Finalize consumes this exact residue and closes the pot atomically; no
    // serializable post-sweep pot phase exists.
    assert_eq!(finalizing.cash_atoms, 0);
    assert_eq!(finalizing.eggs[0], 10);
}

#[test]
fn pot_refuses_bidirectional_dealer_cash_and_impossible_custody() {
    let mut pot = common::collect_pot();
    pot.dealer_net_cash_in_atoms = 1;
    pot.user_cash_out_atoms += 1;
    assert_eq!(pot.validate(), Err(Error::ConservationFailure));

    let mut pot = common::deliver_pot();
    pot.delivered_user_cash_atoms = 31;
    assert_eq!(pot.derived_custody(), Err(Error::ConservationFailure));
    assert_eq!(pot.validate(), Err(Error::ConservationFailure));
}

#[test]
fn budgets_are_segregated_prepaid_and_exactly_conserved() {
    assert_eq!(
        common::fee_budget().validate_against_policy(&common::policy()),
        Ok(())
    );
    assert_eq!(
        common::liveness_budget().validate_against_policy(&common::policy()),
        Ok(())
    );

    let mut fee = common::fee_budget();
    fee.available_atoms -= 1;
    assert_eq!(fee.validate(), Err(Error::ConservationFailure));

    let mut live = common::liveness_budget();
    live.liability_count = 0;
    assert_eq!(live.validate(), Err(Error::ConservationFailure));

    let mut live = common::liveness_budget();
    live.phase = BudgetPhaseV1::Closed;
    assert_eq!(live.validate(), Err(Error::InvalidPhase));
}

#[test]
fn every_rent_owner_joins_the_one_policy_sink() {
    assert_eq!(
        common::lp_page().validate_against_policy(&common::policy()),
        Ok(())
    );

    let mut state = common::funding_state();
    state.rent.neutral_sink = common::id(200);
    assert_eq!(state.validate(), Ok(()));
    assert_eq!(
        state.validate_against_policy(&common::policy()),
        Err(Error::MismatchedBinding)
    );

    let mut page = common::lp_page();
    page.rent.neutral_sink = common::id(200);
    assert_eq!(page.validate(), Ok(()));
    assert_eq!(
        page.validate_against_policy(&common::policy()),
        Err(Error::MismatchedBinding)
    );

    let mut aliased = common::child_rent();
    aliased.neutral_sink = aliased.payer;
    assert_eq!(aliased.validate(), Err(Error::InvalidParameter));

    let mut budget = common::fee_budget();
    budget.principal_payer = budget.neutral_sink;
    assert_eq!(budget.validate(), Err(Error::InvalidParameter));
}

#[test]
fn counted_child_fold_requires_the_exhaustive_set() {
    let state = common::trading_state();
    let mut fold = DealerChildGraphFoldV1::new(state.facility_id, state.generation).unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::FacilityPosition,
        counted_generation: 0,
    })
    .unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::FacilityReplay,
        counted_generation: 0,
    })
    .unwrap();
    fold.observe(common::lp_page().counted_child()).unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::LpPosition,
        counted_generation: 0,
    })
    .unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::EpochBinding,
        counted_generation: state.generation,
    })
    .unwrap();
    fold.observe(common::lease().counted_child()).unwrap();
    fold.observe(common::collect_pot().counted_child()).unwrap();
    fold.observe(common::fee_budget().counted_child()).unwrap();
    assert_eq!(fold.finish(&state), Err(Error::InvalidChildGraph));

    let mut fold = DealerChildGraphFoldV1::new(state.facility_id, state.generation).unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::FacilityPosition,
        counted_generation: 0,
    })
    .unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::FacilityReplay,
        counted_generation: 0,
    })
    .unwrap();
    fold.observe(common::lp_page().counted_child()).unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::LpPosition,
        counted_generation: 0,
    })
    .unwrap();
    fold.observe(CountedDealerChildV1 {
        facility_id: state.facility_id,
        kind: DealerChildKindV1::EpochBinding,
        counted_generation: state.generation,
    })
    .unwrap();
    fold.observe(common::lease().counted_child()).unwrap();
    fold.observe(common::collect_pot().counted_child()).unwrap();
    fold.observe(common::fee_budget().counted_child()).unwrap();
    fold.observe(common::liveness_budget().counted_child())
        .unwrap();
    assert_eq!(fold.finish(&state), Ok(()));

    let mut stale = common::lease().counted_child();
    stale.counted_generation -= 1;
    let mut fold = DealerChildGraphFoldV1::new(state.facility_id, state.generation).unwrap();
    assert_eq!(fold.observe(stale), Err(Error::MismatchedBinding));

    let mut wrong = common::lp_page().counted_child();
    wrong.facility_id = common::id(199);
    let mut fold = DealerChildGraphFoldV1::new(state.facility_id, state.generation).unwrap();
    assert_eq!(fold.observe(wrong), Err(Error::MismatchedBinding));
}

#[test]
fn rejected_child_observation_is_atomic() {
    let facility = common::id(20);
    let edge = CountedDealerChildV1 {
        facility_id: facility,
        kind: DealerChildKindV1::FacilityPosition,
        counted_generation: 0,
    };
    let mut fold = DealerChildGraphFoldV1::new(facility, 0).unwrap();
    fold.observe(edge).unwrap();
    let before = fold.observed();
    assert_eq!(fold.observe(edge), Err(Error::InvalidChildGraph));
    assert_eq!(fold.observed(), before);
}

#[test]
fn pda_preimages_are_exact_disjoint_and_solana_compatible() {
    let facility = common::id(20);
    let policy = DealerPdaPreimageV1::policy(common::policy().policy_id().unwrap()).unwrap();
    assert_eq!(policy.seed(0).unwrap(), DEALER_POLICY_PDA_DOMAIN_V1);
    assert_eq!(policy.seed_count(), 2);

    let page = DealerPdaPreimageV1::lp_page(facility, 7).unwrap();
    assert_eq!(page.seed(0).unwrap(), LP_PAGE_PDA_DOMAIN_V1);
    assert_eq!(page.seed(1).unwrap(), &facility.bytes());
    assert_eq!(page.seed(2).unwrap(), &7u32.to_le_bytes());

    let lease = DealerPdaPreimageV1::lease(facility, 9).unwrap();
    let pot = DealerPdaPreimageV1::settlement_pot(facility, 9).unwrap();
    assert_eq!(lease.family(), DealerPdaFamilyV1::Lease);
    assert_eq!(pot.family(), DealerPdaFamilyV1::SettlementPot);
    assert_ne!(lease.seed(0).unwrap(), pot.seed(0).unwrap());
    assert_eq!(lease.seed(2).unwrap(), pot.seed(2).unwrap());

    assert_eq!(
        DealerPdaPreimageV1::state(Id::ZERO),
        Err(Error::ZeroIdentity)
    );
    assert_eq!(
        DealerPdaPreimageV1::lp_page(facility, MAX_LP_PAGES),
        Err(Error::InvalidParameter)
    );
}

#[test]
fn every_current_runtime_action_is_planned_and_disabled() {
    let actions = [
        DealerRuntimeActionV1::CreatePolicy,
        DealerRuntimeActionV1::Initialize,
        DealerRuntimeActionV1::CreateLpPage,
        DealerRuntimeActionV1::Contribute,
        DealerRuntimeActionV1::WithdrawFunding,
        DealerRuntimeActionV1::Activate,
        DealerRuntimeActionV1::CancelFunding,
        DealerRuntimeActionV1::RefundCancelledSponsor,
        DealerRuntimeActionV1::BindEpoch,
        DealerRuntimeActionV1::LapseEpoch,
        DealerRuntimeActionV1::SelectLeaseAndBegin,
        DealerRuntimeActionV1::Collect,
        DealerRuntimeActionV1::Deliver,
        DealerRuntimeActionV1::FinalizeSettlement,
        DealerRuntimeActionV1::AbortBeforeCollection,
        DealerRuntimeActionV1::QueueExit,
        DealerRuntimeActionV1::SponsorHalt,
        DealerRuntimeActionV1::EnterUnwind,
        DealerRuntimeActionV1::TimedClose,
        DealerRuntimeActionV1::Resolve,
        DealerRuntimeActionV1::Claim,
        DealerRuntimeActionV1::Retire,
    ];
    for action in actions {
        assert_eq!(require_action_enabled(action), Err(Error::ActionDisabled));
    }
}

#[test]
fn content_domains_are_fresh_and_role_separated() {
    let policy_id = common::policy().policy_id().unwrap();
    let state_id = common::funding_state().state_content_id().unwrap();
    let page_id = common::lp_page().page_content_id().unwrap();
    let lease_id = common::lease().lease_id().unwrap();
    let pot_id = common::finalizing_pot().pot_content_id().unwrap();
    let fee_id = common::fee_budget().budget_content_id().unwrap();
    let live_id = common::liveness_budget().budget_content_id().unwrap();
    let ids = [
        policy_id, state_id, page_id, lease_id, pot_id, fee_id, live_id,
    ];
    for left in 0..ids.len() {
        assert!(!ids[left].is_zero());
        for right in 0..left {
            assert_ne!(ids[left], ids[right]);
        }
    }
    assert!(DEALER_POLICY_CONTENT_DOMAIN_V1.ends_with(&[0]));
    assert!(DEALER_STATE_CONTENT_DOMAIN_V1.ends_with(&[0]));
    assert!(LP_PAGE_CONTENT_DOMAIN_V1.ends_with(&[0]));
    assert!(DEALER_LEASE_CONTENT_DOMAIN_V1.ends_with(&[0]));
    assert!(SETTLEMENT_POT_CONTENT_DOMAIN_V1.ends_with(&[0]));
    assert!(FEE_BUDGET_CONTENT_DOMAIN_V1.ends_with(&[0]));
    assert!(LIVENESS_BUDGET_CONTENT_DOMAIN_V1.ends_with(&[0]));
}
