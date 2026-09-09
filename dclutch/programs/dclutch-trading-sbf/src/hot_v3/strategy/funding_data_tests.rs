use super::*;
use dclutch_vm::{
    account_profile::lifecycle_v3::{AuthenticateStatePlanV3, CreateStatePlanV3},
    effect::{
        v3::{
            HEADER_BYTES, OPERATION_BYTES,
            encode::{
                AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, ScalarCoordinateV3,
                encode_effect_program_v3_atomic,
            },
        },
        v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, encode_program_v4_atomic},
        v5::{FUNDING_ACTION_BYTES_V5, FundingActionV5, HEADER_BYTES_V5, encode_program_v5_atomic},
    },
};

fn funded_effect(operation: EffectInstructionV3) -> Vec<u8> {
    let len = HEADER_BYTES + OPERATION_BYTES;
    let mut base = vec![0; len];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: 11,
            item_account_stride: 0,
            common_scalars: 2,
            item_scalar_stride: 0,
            common_identities: 1,
            item_identity_stride: 0,
        },
        &[],
        &[operation],
        &[],
        &mut vec![0; len],
        &mut base,
    )
    .expect("base effect");
    let mut v4 = vec![0; HEADER_BYTES_V4 + len];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        1,
        &[],
        &[],
        &mut vec![0; v4.len()],
        &mut v4,
    )
    .expect("range effect");
    let mut v5 = vec![0; HEADER_BYTES_V5 + FUNDING_ACTION_BYTES_V5 + v4.len()];
    encode_program_v5_atomic(
        &v4,
        &[FundingActionV5::fund(5, 8, 10, 1, 0)],
        &[],
        &mut vec![0; v5.len()],
        &mut v5,
    )
    .expect("funded effect");
    v5
}

fn create_at(state: usize) -> PreparedLifecycleInvocationV3 {
    PreparedLifecycleInvocationV3 {
        plan: StateLifecyclePlanV3::Create(CreateStatePlanV3 {
            state: [1; 32],
            payer: [2; 32],
            rent_credit: [3; 32],
            beneficiary: [2; 32],
            refund_source: LifecycleRefundSourceV3::Credit,
            target_data_bytes: 64,
            historical_rent_principal: 10,
            state_before: 0,
            state_after: 10,
            payer_debit: 10,
            payer_after: 990,
            bump: 9,
        }),
        state,
        payer: Some(8),
        rent_credit: Some(9),
        seeds: Vec::new(),
        immutable_identity_bindings: Vec::new(),
    }
}

fn project_funded(
    operation: EffectInstructionV3,
    plans: &[PreparedLifecycleInvocationV3],
) -> Result<ProjectedEffectsV3, ProgramError> {
    let bytes = funded_effect(operation);
    let effect =
        decode_selected_effect_v4(EFFECT_SCHEMA_ID_V5, &bytes).expect("selected funding effect");
    let mut inputs = vec![
        AccountInput {
            lamports: 0,
            data_len: 64
        };
        11
    ];
    inputs[8].lamports = 1000;
    if plans
        .iter()
        .any(|p| p.state == 5 && matches!(p.plan, StateLifecyclePlanV3::Create(_)))
    {
        inputs[5].data_len = 0;
    }
    let mut permissions = [AccountPermission::read_only(); 11];
    permissions[5] = AccountPermission::new(false, true, true);
    permissions[6] = AccountPermission::new(false, true, true);
    permissions[8] = AccountPermission::new(true, true, true);
    permissions[10] = AccountPermission::new(false, false, true);
    let aliases = (0..11).collect::<Vec<_>>();
    project_hot_effects_v3(
        effect,
        0,
        &[1234, 25],
        &[[2; 32]],
        inputs,
        plans,
        false,
        &permissions,
        &aliases,
        11,
        0,
    )
}

#[test]
fn funding_fund_allows_only_the_same_lifecycle_created_state_data() {
    let write = EffectInstructionV3::write_u64(
        AccountCoordinateV3::fixed(5),
        0,
        ScalarCoordinateV3::common(0),
    );
    let expected = ProgramError::from(TradingSbfError::Transition);
    assert_eq!(
        project_funded(write, &[]).err(),
        Some(expected.clone()),
        "Fund without a lifecycle creator cannot write data"
    );
    assert_eq!(
        project_funded(write, &[create_at(6)]).err(),
        Some(expected.clone()),
        "another state creator conveys no authority"
    );
    let mut authenticated = create_at(5);
    authenticated.plan = StateLifecyclePlanV3::Authenticate(AuthenticateStatePlanV3 {
        state: [1; 32],
        data_bytes: 64,
        lamports: 0,
        bump: 9,
    });
    assert_eq!(
        project_funded(write, &[authenticated]).err(),
        Some(expected.clone()),
        "Authenticate is not creation authority"
    );
    for coordinate in [8, 10] {
        let forbidden = EffectInstructionV3::write_u64(
            AccountCoordinateV3::fixed(coordinate),
            0,
            ScalarCoordinateV3::common(0),
        );
        assert_eq!(
            project_funded(forbidden, &[create_at(5)]).err(),
            Some(expected.clone()),
            "funding payer and System data remain protected"
        );
    }
    let lamports = EffectInstructionV3::transfer_lamports(
        AccountCoordinateV3::fixed(8),
        AccountCoordinateV3::fixed(5),
        ScalarCoordinateV3::common(1),
    );
    assert_eq!(
        project_funded(lamports, &[create_at(5)]).err(),
        Some(expected),
        "local transfer cannot duplicate funding"
    );
    let accepted = project_funded(write, &[create_at(5)])
        .expect("the lifecycle-created Candidate can receive its header after work funding");
    assert_eq!(accepted.lamports[5], 25);
    assert_eq!(accepted.lamports[8], 975);
}
