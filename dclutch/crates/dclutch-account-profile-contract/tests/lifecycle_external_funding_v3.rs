//! Authority-separation agreement between Lifecycle V5 and funding-bound AccountProfile V3.

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        ACTION_PLAN_BYTES, Error, HEADER_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
        StateLifecyclePolicyV5,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleGuardInputV3, LifecycleOperationInputV3,
            LifecyclePlanInputV3, LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3,
            LifecycleRegisterCoordinateV3, LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
        },
    },
    v2::{
        AccountPrestateV2, HEADER_BYTES as PROFILE_HEADER_BYTES,
        OPERATION_BYTES as PROFILE_OPERATION_BYTES, RULE_BYTES as PROFILE_RULE_BYTES,
        TrustedEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
            ScalarCoordinateV2, encode_account_profile_with_lifecycle_v2_atomic,
        },
    },
    v3::{
        AccountProfileV3, FUNDING_BOUND_BYTES_V3, FundingActionMaskV3, FundingBoundV3,
        HEADER_BYTES_V3, encode_account_profile_v3_atomic,
    },
};

const POLICY_ID: [u8; 32] = [0x71; 32];

fn funding_profile() -> Vec<u8> {
    let lifecycle_rule = AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(false, true, false),
        effect_permissions: AccountEffectPermissionsV2::new(true, true, true),
        alias: AccountAliasInputV2::SelfCoordinate,
        data_length: 64,
        data_item_stride: 0,
    };
    // `73ffb010` made an AuthenticateOrCreate plan require its payer to carry
    // DEBIT_LAMPORTS, because a plan that may create the account must be able
    // to fund it. This fixture predates that and gave the payer no permission
    // at all, so the profile it builds is one no policy can create against.
    let exact_rule = |signer, debit_lamports| AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(signer, true, false),
        effect_permissions: AccountEffectPermissionsV2::new(debit_lamports, false, false),
        alias: AccountAliasInputV2::SelfCoordinate,
        data_length: 0,
        data_item_stride: 0,
    };
    let rules = [
        AccountRuleWithPrestateInputV2 {
            rule: lifecycle_rule,
            prestate: AccountPrestateV2::LifecycleBound,
        },
        AccountRuleWithPrestateInputV2 {
            rule: lifecycle_rule,
            prestate: AccountPrestateV2::LifecycleBound,
        },
        // Coordinate 2 is every plan's payer and is debited to fund the create.
        AccountRuleWithPrestateInputV2 {
            rule: exact_rule(true, true),
            prestate: AccountPrestateV2::Exact,
        },
        // Coordinate 3 is the RentCredit. These plans never close, so it needs
        // no lamport permission of its own here.
        AccountRuleWithPrestateInputV2 {
            rule: exact_rule(false, false),
            prestate: AccountPrestateV2::Exact,
        },
    ];
    let operations = [
        // A debitable account that is not LifecycleBound must be anchored by a
        // RequireOwner naming it, or the V2 encoder refuses the profile with
        // EffectOwnerUnanchored. The payer acquired DEBIT_LAMPORTS above, so it
        // acquires its owner relation here.
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(2),
            expected: IdentityCoordinateV2::common(1),
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(1),
            destination: ScalarCoordinateV2::common(2),
            data_offset: 8,
        },
        AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(3),
            destination: ScalarCoordinateV2::common(1),
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(3),
            destination: IdentityCoordinateV2::common(0),
        },
    ];
    let base_width = PROFILE_HEADER_BYTES
        + rules.len() * PROFILE_RULE_BYTES
        + operations.len() * PROFILE_OPERATION_BYTES;
    let mut base_scratch = vec![0_u8; base_width];
    let mut base = vec![0_u8; base_width];
    encode_account_profile_with_lifecycle_v2_atomic(
        TrustedEnvironmentV2::None,
        &rules,
        &[],
        &operations,
        &[],
        RegisterGeometryV2 {
            common_scalars: 5,
            item_scalar_stride: 0,
            common_identities: 4,
            item_identity_stride: 0,
        },
        &mut base_scratch,
        &mut base,
    )
    .expect("base lifecycle profile");
    let funding = [FundingBoundV3::new(
        0,
        FundingActionMaskV3::CREATE_AND_CLOSE,
        64,
    )];
    let width = HEADER_BYTES_V3 + FUNDING_BOUND_BYTES_V3 + base.len();
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v3_atomic(&base, &funding, &mut scratch, &mut output)
        .expect("funding-bound profile");
    output
}

fn policy(recipe_states: &[u16], payer: u16, rent_credit: u16) -> Vec<u8> {
    let recipes: Vec<_> = recipe_states
        .iter()
        .enumerate()
        .map(|(index, state)| LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(*state),
            seed_start: u16::try_from(index).expect("bounded fixture"),
            seed_count: 1,
            bump_offset: 0,
            data_base: 64,
            data_stride: 0,
        })
        .collect();
    let seeds = vec![LifecycleSeedInputV3::CanonicalBump; recipes.len()];
    let plans: Vec<_> = recipes
        .iter()
        .enumerate()
        .map(|(index, _)| LifecyclePlanInputV3 {
            action: u32::try_from(index + 1).expect("bounded fixture"),
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: u16::try_from(index).expect("bounded fixture"),
            payer: Some(LifecycleAccountCoordinateV3::fixed(payer)),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(rent_credit)),
            principal: Some(LifecycleRegisterCoordinateV3::common(1)),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(0)),
            guard: LifecycleGuardInputV3::Always,
        })
        .collect();
    let protected = vec![
        Some(LifecycleProtectedOutputsInputV3 {
            created: 0,
            bump_observation: 2,
            bump: 3,
            historical_rent_principal: 4,
            beneficiary: 1,
            state: 2,
            owner: 3,
        });
        plans.len()
    ];
    let width = HEADER_BYTES
        + recipes.len() * RECIPE_BYTES
        + seeds.len() * SEED_BYTES
        + plans.len() * ACTION_PLAN_BYTES
        + protected.len() * PROTECTED_OUTPUT_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .expect("V5 policy");
    output
}

#[test]
fn funding_table_is_sole_create_close_authority() {
    let profile_bytes = funding_profile();
    let profile = AccountProfileV3::decode(&profile_bytes).expect("V3 profile");
    let policy_bytes = policy(&[1], 2, 3);
    let owning_bytes = policy(&[0, 1], 2, 3);
    let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
        .expect("V5 policy");
    let owning = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &owning_bytes)
        .expect("V5 policy owning coordinate zero");

    assert_eq!(
        policy.validate_account_profile(profile.base()),
        Err(Error::ProfileMismatch),
        "a bare V2 join must not reinterpret external funding"
    );
    // The refusal above has to be coordinate zero's unowned LifecycleBound
    // prestate and nothing else, or it is a code reached before its subject. A
    // policy that does own coordinate zero passes the very same bare join.
    assert_eq!(owning.validate_account_profile(profile.base()), Ok(()));
    let _join = policy
        .validate_account_profile_with_external_funding_join(profile)
        .expect("mandatory V3 join owns coordinate zero");
}

#[test]
fn lifecycle_state_payer_and_rent_credit_dual_coverage_are_refused() {
    let profile_bytes = funding_profile();
    let profile = AccountProfileV3::decode(&profile_bytes).expect("V3 profile");
    for hostile in [
        policy(&[0, 1], 2, 3),
        policy(&[1], 0, 3),
        policy(&[1], 2, 0),
    ] {
        let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &hostile)
            .expect("hostile remains a valid V5 artifact");
        assert_eq!(
            policy.validate_account_profile_with_external_funding(profile),
            Err(Error::ProfileMismatch)
        );
    }
}
