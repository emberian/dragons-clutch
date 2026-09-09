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
    let width = general_family_state_lifecycle_bytes_v5();
    let mut policy = vec![0; width];
    encode_general_family_state_lifecycle_v5_atomic(
        GeneralChildRentWidthsV5::new(4, 165).unwrap(),
        &mut vec![0; width],
        &mut policy,
    )
    .unwrap();
    let width = general_account_profile_bytes_v3(Action::PlaceOrder).unwrap();
    let mut profile = vec![0; width];
    encode_general_account_profile_v3_atomic(
        Action::PlaceOrder,
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
