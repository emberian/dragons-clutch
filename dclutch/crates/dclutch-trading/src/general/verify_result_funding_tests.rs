//! Exercise the published refund projection and pure Result/Create together.

use super::*;
use crate::general::hot_candidate_v3::general_hot_scalar_count_v3;
use crate::general::state_artifacts_v3::{
    GeneralChildRentWidthsV5, encode_general_state_lifecycle_v5_atomic,
    general_state_lifecycle_bytes_v5,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleAccountIdV2, LifecycleRentCreditV2},
};
use dclutch_vm::account_profile::{
    AccountObservationV1,
    lifecycle_v3::{
        AuthenticatedRentCreditV3, AuthenticatedRentMinimumV3, AuthenticatedRentQuoteV5,
        Error as StateLifecycleErrorV3, LifecycleContextV3, LifecycleOperationV3,
        LifecycleProtectedRegisterBuffersV3, LifecycleRegistersV3, LifecycleRentQuoteBuffersV5,
        PlannedObservationsV3, StateLifecyclePlanV3, StateLifecyclePolicyV5,
        plan_lifecycle_with_protected_outputs_atomic,
    },
    v2::{
        AccountProfileV2, ProjectionRegistersV2,
        encode::encode_account_profile_with_dynamic_fixed_span_v2_atomic,
        project_dynamic_fixed_spans_atomic,
    },
};
use std::{vec, vec::Vec};

#[test]
fn verify_result_create_uses_current_rent_and_credit_refund_before_and_after_evaluation() {
    let action = Action::VerifyCandidateRow;
    let count = 1;
    let widths = GeneralExternalAccountWidthsV3 {
        linked_basis_prefix: 64,
        result_domain: 192,
        rent_sysvar: 17,
        core_market: 320,
        activation_cache: 160,
        upgradeable_program: 36,
        trading_programdata_prefix: 45,
        claims_programdata_prefix: 45,
        core_programdata_prefix: 45,
        realm_record: 112,
        rent_credit: u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("credit width"),
    };
    let profile_len = general_account_profile_bytes_v3(action).expect("profile width");
    let mut profile_bytes = vec![0; profile_len];
    encode_general_account_profile_v3_atomic(
        action,
        widths,
        &mut vec![0; profile_len],
        &mut profile_bytes,
    )
    .expect("published account profile");
    let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
    let scalar_width =
        usize::try_from(general_hot_scalar_count_v3(action, count).expect("scalars"))
            .expect("width");
    let identity_width = usize::from(profile.common_identity_count());
    let slots = profile.logical_account_count(count).expect("account count");
    let credit_index = usize::from(GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3);
    let payer_index = usize::from(GENERAL_VERIFY_PAYER_ACCOUNT_V3);
    let result_index = usize::from(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3);
    let keys = (0..slots)
        .map(|index| [u8::try_from(index + 1).expect("coordinate"); 32])
        .collect::<Vec<_>>();
    let trading = [0xa1; 32];
    let refund = [0xb1; 32];
    assert_ne!(refund, keys[payer_index]);
    let credit = LifecycleRentCreditV2::new(
        RefundAuthority::new(refund).expect("refund"),
        LifecycleAccountIdV2::new([0xb2; 32]).expect("market"),
        LifecycleAccountIdV2::new([0xb3; 32]).expect("release"),
        9,
        1,
    )
    .expect("credit");
    let credit_bytes = credit.to_bytes();
    let mut owners = vec![trading; slots];
    owners[result_index] = [0; 32];
    owners[payer_index] = [0; 32];
    fn make_observations<'a>(
        keys: &'a [[u8; 32]],
        owners: &'a [[u8; 32]],
        credit_bytes: &'a [u8],
        payer_index: usize,
        credit_index: usize,
    ) -> Vec<AccountObservationV1<'a>> {
        keys.iter()
            .zip(owners)
            .enumerate()
            .map(|(index, (key, owner))| {
                AccountObservationV1::new(
                    key,
                    owner,
                    if index == payer_index { 10_000_000 } else { 0 },
                    if index == credit_index {
                        credit_bytes
                    } else {
                        &[]
                    },
                    index == payer_index,
                    true,
                    false,
                )
            })
            .collect::<Vec<_>>()
    }
    let observations = make_observations(&keys, &owners, &credit_bytes, payer_index, credit_index);

    // Execute the published producer in a reduced projection profile. Other
    // unrelated account roles are handled by the full entrypoint fixture.
    let operations = (0..general_account_profile_operation_count_v3(action))
        .map(|index| general_account_profile_operation_v3(action, index).expect("operation"))
        .filter(|op| matches!(op, AccountOperationInputV2::ProjectDataIdentity { destination, .. }
            if *destination == IdentityCoordinateV2::common(u16::try_from(identity::RESULT_BENEFICIARY_OBSERVATION).expect("coordinate"))))
        .collect::<Vec<_>>();
    let rule = exact_rule(false, true, false, widths.rent_credit, 0, no_effects());
    let projection_len =
        DYNAMIC_FIXED_SPAN_HEADER_BYTES + slots * RULE_BYTES + operations.len() * OPERATION_BYTES;
    let mut projection_bytes = vec![0; projection_len];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::None,
        &[],
        &vec![rule; slots],
        &[],
        &operations,
        RegisterGeometryV2 {
            common_scalars: u16::try_from(scalar_width).expect("scalars"),
            item_scalar_stride: 0,
            common_identities: u16::try_from(identity_width).expect("identities"),
            item_identity_stride: 0,
        },
        &mut vec![0; projection_len],
        &mut projection_bytes,
    )
    .expect("refund projection profile");
    // Only the refund producer is selected, but execute it on canonical Credit
    // bytes through the real VM, rather than inserting the expected identity.
    let projection_observations = keys
        .iter()
        .zip(&owners)
        .map(|(key, owner)| {
            AccountObservationV1::new(key, owner, 0, &credit_bytes, false, true, false)
        })
        .collect::<Vec<_>>();
    let mut identities = vec![[0; 32]; identity_width];
    project_dynamic_fixed_spans_atomic(
        AccountProfileV2::decode(&projection_bytes).expect("projection profile"),
        count,
        &[],
        &projection_observations,
        ProjectionRegistersV2 {
            input_scalars: &vec![0; scalar_width],
            input_identities: &vec![[0; 32]; identity_width],
            scratch_scalars: &mut vec![0; scalar_width],
            scratch_identities: &mut vec![[0; 32]; identity_width],
            output_scalars: &mut vec![0; scalar_width],
            output_identities: &mut identities,
        },
        None,
    )
    .expect("published refund projection");

    let child_widths = GeneralChildRentWidthsV5::new(count, 165).expect("child widths");
    let policy_len = general_state_lifecycle_bytes_v5(action).expect("policy width");
    let mut policy_bytes = vec![0; policy_len];
    encode_general_state_lifecycle_v5_atomic(
        action,
        Some(child_widths),
        &mut vec![0; policy_len],
        &mut policy_bytes,
    )
    .expect("canonical policy");
    let policy =
        StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &policy_bytes).expect("policy");
    let quote = policy.current_rent_quote(0).expect("rent declaration");
    let current = AuthenticatedRentMinimumV3 {
        data_bytes: child_widths.verified_candidate,
        lamports: 2_227_200,
    };
    let mut scalars = vec![0; scalar_width];
    policy
        .project_authenticated_current_rent_quotes_atomic(
            profile,
            None,
            count,
            action as u32,
            &vec![0; scalar_width],
            &[AuthenticatedRentQuoteV5 {
                exact_data_len: current.data_bytes,
                scalar_destination: quote.scalar_destination().index(),
                current_minimum: current.lamports,
            }],
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut vec![0; scalar_width],
                output_scalars: &mut scalars,
            },
        )
        .expect("authenticated quote projection");
    scalars[usize::try_from(scalar::VERIFY_TERMINAL).expect("guard")] = 1;
    let selected = policy.action_plan(action as u32, 1).expect("Result/Create");
    assert_eq!(selected.operation(), LifecycleOperationV3::Create);
    let plan =
        |scalars: &[u64], identities: &[[u8; 32]], observations: &[AccountObservationV1<'_>]| {
            plan_lifecycle_with_protected_outputs_atomic(
                selected,
                LifecycleContextV3 {
                    account_profile: profile,
                    tail_count: count,
                    item_index: None,
                    accounts: PlannedObservationsV3::observed(observations),
                    registers: LifecycleRegistersV3 {
                        scalars,
                        identities,
                    },
                    trading_program: trading,
                    system_program: [0; 32],
                    adapter_derived_pda: keys[result_index],
                    rent_credit: Some(AuthenticatedRentCreditV3 {
                        key: keys[credit_index],
                        beneficiary: refund,
                        lamports: 0,
                    }),
                    current_rent_minimum: Some(current),
                },
                7,
                LifecycleProtectedRegisterBuffersV3 {
                    scalar_scratch: &mut vec![0; scalar_width],
                    identity_scratch: &mut vec![[0; 32]; identity_width],
                    output_scalars: &mut vec![0; scalar_width],
                    output_identities: &mut vec![[0; 32]; identity_width],
                },
            )
        };
    let accepted = plan(&scalars, &identities, &observations)
        .expect("published projection must admit one-time Result/Create");
    let StateLifecyclePlanV3::Create(created) = accepted else {
        panic!("must create");
    };
    assert_eq!(created.beneficiary, refund);
    assert_eq!(created.historical_rent_principal, current.lamports);
    assert_eq!(created.payer, keys[payer_index]);
    let mut wrong_rent = scalars.clone();
    wrong_rent[usize::try_from(scalar::RESULT_PRINCIPAL_OBSERVATION).expect("principal")] += 1;
    assert_eq!(
        plan(&wrong_rent, &identities, &observations),
        Err(StateLifecycleErrorV3::InvalidRent)
    );
    for wrong_refund in [[0; 32], keys[payer_index]] {
        let mut hostile = identities.clone();
        hostile[usize::try_from(identity::RESULT_BENEFICIARY_OBSERVATION).expect("refund")] =
            wrong_refund;
        assert_eq!(
            plan(&scalars, &hostile, &observations),
            Err(StateLifecycleErrorV3::InvalidRent)
        );
    }
    owners[result_index] = trading;
    let live = make_observations(&keys, &owners, &credit_bytes, payer_index, credit_index);
    assert_eq!(
        plan(&scalars, &identities, &live),
        Err(StateLifecycleErrorV3::InvalidState)
    );
}
