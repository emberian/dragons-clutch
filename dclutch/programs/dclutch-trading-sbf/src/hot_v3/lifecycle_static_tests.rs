use super::*;
use alloc::vec;
use dclutch_trading::general::{
    account_rules_v3::{
        GeneralExternalAccountWidthsV3, encode_general_account_profile_v3_atomic,
        general_account_profile_bytes_v3,
    },
    state_artifacts_v3::{
        GeneralChildRentWidthsV5, encode_general_family_state_lifecycle_v5_atomic,
        general_family_state_lifecycle_bytes_v5,
    },
};
use dclutch_trading::general_codec::Action;

fn artifact_bytes() -> (Vec<u8>, Vec<u8>) {
    artifact_bytes_for(Action::PlaceOrder)
}

fn artifact_bytes_for(action: Action) -> (Vec<u8>, Vec<u8>) {
    let width = general_family_state_lifecycle_bytes_v5();
    let mut policy = vec![0; width];
    encode_general_family_state_lifecycle_v5_atomic(
        GeneralChildRentWidthsV5::new(4, 165).unwrap(),
        &mut vec![0; width],
        &mut policy,
    )
    .unwrap();
    let width = general_account_profile_bytes_v3(action).unwrap();
    let mut profile = vec![0; width];
    encode_general_account_profile_v3_atomic(
        action,
        GeneralExternalAccountWidthsV3 {
            linked_basis_prefix: 256,
            result_domain: 192,
            rent_sysvar: 17,
            core_market: 368,
            activation_cache: 1288,
            upgradeable_program: 36,
            trading_programdata_prefix: 45,
            claims_programdata_prefix: 45,
            core_programdata_prefix: 45,
            realm_record: 112,
            rent_credit: 128,
        },
        &mut vec![0; width],
        &mut profile,
    )
    .unwrap();
    (policy, profile)
}

#[test]
fn lifecycle_static_facts_bind_every_immutable_input() {
    let (policy_bytes, profile_bytes) = artifact_bytes();
    let other_policy_bytes = policy_bytes.clone();
    let other_profile_bytes = profile_bytes.clone();
    let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &policy_bytes).unwrap();
    let other_policy =
        StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &other_policy_bytes).unwrap();
    let profile = AccountProfileV2::decode(&profile_bytes).unwrap();
    let other_profile = AccountProfileV2::decode(&other_profile_bytes).unwrap();
    let action = Action::PlaceOrder as u32;
    let join = policy
        .validate_account_profile_join_for_action(profile, action)
        .unwrap();
    let rent = Rent::default();
    let other_rent = Rent::free();
    let facts = LifecycleStaticFactsV4::new(policy, profile, action, 2, &rent, join).unwrap();
    let actual = [
        facts.require_binding(policy, profile, action, 2, &rent),
        facts.require_binding(other_policy, profile, action, 2, &rent),
        facts.require_binding(policy, other_profile, action, 2, &rent),
        facts.require_binding(policy, profile, Action::OpenBatch as u32, 2, &rent),
        facts.require_binding(policy, profile, action, 3, &rent),
        facts.require_binding(policy, profile, action, 2, &other_rent),
    ];
    let refused = Err(TradingSbfError::Transition.into());
    assert_eq!(
        actual,
        [
            Ok(()),
            refused.clone(),
            refused.clone(),
            refused.clone(),
            refused.clone(),
            refused
        ]
    );
}

#[test]
fn lifecycle_static_rent_quotes_follow_exact_target_width_and_snapshot() {
    let (policy_bytes, profile_bytes) = artifact_bytes();
    let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &policy_bytes).unwrap();
    let profile = AccountProfileV2::decode(&profile_bytes).unwrap();
    let action = Action::PlaceOrder as u32;
    let join = policy
        .validate_account_profile_join_for_action(profile, action)
        .unwrap();
    let rent = Rent::default();
    let changed_rent = Rent::free();
    let mut quotes = Vec::new();
    for (tail, snapshot) in [(2, &rent), (3, &rent), (2, &changed_rent)] {
        let facts =
            LifecycleStaticFactsV4::new(policy, profile, action, tail, snapshot, join).unwrap();
        assert_eq!(
            facts.selections.len(),
            usize::from(policy.action_plan_count(action).unwrap())
        );
        assert_eq!(facts.planned, 2);
        let selection = facts.selections.first().unwrap();
        let quote = selection.current_rent_minimum.unwrap();
        assert_eq!(
            quote.data_bytes,
            selection.selected.target_data_bytes(tail).unwrap()
        );
        assert_eq!(
            quote.lamports,
            snapshot.minimum_balance(usize::try_from(quote.data_bytes).unwrap())
        );
        quotes.push(quote);
    }
    assert_ne!(quotes[0].data_bytes, quotes[1].data_bytes);
    assert_ne!(quotes[0].lamports, quotes[2].lamports);
}

#[test]
fn lifecycle_atomic_planner_refusal_does_not_publish_dirty_scratch() {
    let (policy_bytes, profile_bytes) = artifact_bytes();
    let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &policy_bytes).unwrap();
    let profile = AccountProfileV2::decode(&profile_bytes).unwrap();
    let action = Action::PlaceOrder as u32;
    let join = policy
        .validate_account_profile_join_for_action(profile, action)
        .unwrap();
    let selected = policy
        .action_plan(action, 0)
        .unwrap()
        .with_validated_join(join);
    let scalars = vec![7; usize::from(profile.common_scalar_count())];
    let identities = vec![[8; 32]; usize::from(profile.common_identity_count())];
    let mut scalar_scratch = vec![91; scalars.len()];
    let mut identity_scratch = vec![[92; 32]; identities.len()];
    let mut output_scalars = vec![93; scalars.len()];
    let mut output_identities = vec![[94; 32]; identities.len()];
    let result = plan_lifecycle_with_protected_outputs_atomic(
        selected,
        LifecycleContextV3 {
            account_profile: profile,
            tail_count: 0,
            item_index: None,
            accounts: PlannedObservationsV3::observed(&[]),
            registers: LifecycleRegistersV3 {
                scalars: &scalars,
                identities: &identities,
            },
            trading_program: [0; 32],
            system_program: [0; 32],
            adapter_derived_pda: [2; 32],
            rent_credit: None,
            current_rent_minimum: None,
        },
        255,
        LifecycleProtectedRegisterBuffersV3 {
            scalar_scratch: &mut scalar_scratch,
            identity_scratch: &mut identity_scratch,
            output_scalars: &mut output_scalars,
            output_identities: &mut output_identities,
        },
    );
    assert_eq!(
        result,
        Err(dclutch_vm::account_profile::lifecycle_v3::Error::IdentityMismatch)
    );
    assert_eq!(scalar_scratch, scalars);
    assert_eq!(identity_scratch, identities);
    assert_eq!(output_scalars, vec![93; scalars.len()]);
    assert_eq!(output_identities, vec![[94; 32]; identities.len()]);
}

/// These are the production Verify declarations, including the conditional
/// Result/Create between two unconditional invocations. False guards must be
/// accounted for even though they add no row to the physical plan table.
#[test]
fn lifecycle_guard_decisions_accept_terminal_and_nonterminal_verify_sets() {
    let (policy_bytes, profile_bytes) = artifact_bytes_for(Action::VerifyCandidateRow);
    let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &policy_bytes).unwrap();
    let profile = AccountProfileV2::decode(&profile_bytes).unwrap();
    let action = Action::VerifyCandidateRow as u32;
    let join = policy
        .validate_account_profile_join_for_action(profile, action)
        .unwrap();
    let rent = Rent::default();
    let facts = LifecycleStaticFactsV4::new(policy, profile, action, 2, &rent, join).unwrap();
    assert_eq!(facts.planned, 3);
    let identities = vec![
        [0; 32];
        usize::from(profile.common_identity_count())
            + 2 * usize::from(profile.item_identity_stride())
    ];
    for terminal in [0_u64, 1] {
        let mut scalars = vec![
            0;
            usize::from(profile.common_scalar_count())
                + 2 * usize::from(profile.item_scalar_stride())
        ];
        scalars[usize::try_from(
            dclutch_trading::general::hot_candidate_v3::scalar::VERIFY_TERMINAL,
        )
        .unwrap()] = terminal;
        let mut decisions = LifecycleGuardDecisionsV4::new(facts.planned).unwrap();
        for verifying in [false, true] {
            let mut active = 0;
            let mut observed = Vec::new();
            let mut hint_states = Vec::new();
            for (ordinal, selection) in facts.selections.iter().enumerate() {
                assert_eq!(selection.invocation_count, 1);
                let item = selection.selected.invocation_item(2, 0).unwrap();
                let enabled = selection
                    .selected
                    .is_enabled(
                        profile,
                        2,
                        item,
                        LifecycleRegistersV3 {
                            scalars: &scalars,
                            identities: &identities,
                        },
                    )
                    .unwrap();
                decisions.admit(ordinal, enabled, verifying).unwrap();
                observed.push(enabled);
                if enabled {
                    active += 1;
                    for seed in 0..selection.seed_count {
                        if matches!(
                            selection
                                .selected
                                .materialize_seed_input(
                                    profile,
                                    2,
                                    item,
                                    LifecycleRegistersV3 {
                                        scalars: &scalars,
                                        identities: &identities
                                    },
                                    seed,
                                )
                                .unwrap(),
                            LifecycleSeedInputValueV3::CanonicalBump
                        ) {
                            hint_states.push(
                                selection
                                    .selected
                                    .project_account_indices(profile, 2, item)
                                    .unwrap()
                                    .state(),
                            );
                        }
                    }
                }
            }
            use dclutch_trading::general::state_artifacts_v3::{
                GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3,
                GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3,
            };
            let mut expected_hints = vec![usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3)];
            if terminal == 1 {
                expected_hints.push(usize::from(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3));
            }
            expected_hints.push(usize::from(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3));
            assert_eq!(hint_states, expected_hints);
            assert_eq!(observed, [true, terminal == 1, true]);
            assert_eq!(decisions.finish(facts.planned, active), Ok(()));
            assert_eq!(active, 2 + usize::try_from(terminal).unwrap());
        }
    }
}

#[test]
fn lifecycle_guard_decisions_refuse_flips_even_with_equal_active_counts() {
    for (before, after) in [
        ([true, false, true], [true, true, true]),
        ([true, true, true], [true, false, true]),
        ([true, false, true], [false, true, true]),
    ] {
        let mut decisions = LifecycleGuardDecisionsV4::new(before.len()).unwrap();
        for (ordinal, enabled) in before.into_iter().enumerate() {
            decisions.admit(ordinal, enabled, false).unwrap();
        }
        assert_eq!(
            decisions.finish(before.len(), before.into_iter().filter(|v| *v).count()),
            Ok(())
        );
        let result = after
            .into_iter()
            .enumerate()
            .try_for_each(|(ordinal, enabled)| decisions.admit(ordinal, enabled, true));
        assert_eq!(result, Err(TradingSbfError::Transition.into()));
        // A refusing replan cannot rewrite the preplan decision authority.
        assert_eq!(decisions.enabled, before);
        assert_eq!(decisions.active, before.into_iter().filter(|v| *v).count());
    }
}

#[test]
fn lifecycle_guard_decisions_refuse_incomplete_walk_and_active_count() {
    let mut decisions = LifecycleGuardDecisionsV4::new(3).unwrap();
    for (ordinal, enabled) in [true, false, true].into_iter().enumerate() {
        decisions.admit(ordinal, enabled, false).unwrap();
    }
    assert_eq!(decisions.finish(3, 2), Ok(()));
    assert_eq!(
        decisions.finish(2, 2),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(
        decisions.finish(3, 3),
        Err(TradingSbfError::Transition.into())
    );
    assert_eq!(
        decisions.admit(3, false, true),
        Err(TradingSbfError::Transition.into())
    );
}
