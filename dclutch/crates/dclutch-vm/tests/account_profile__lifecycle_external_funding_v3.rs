//! Authority-separation agreement between Lifecycle V5 and funding-bound AccountProfile V3.

use dclutch_vm::account_profile::{
    lifecycle_v3::{
        ACTION_PLAN_BYTES, Error, HEADER_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
        StateLifecyclePolicyV5,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleGuardInputV3, LifecycleOperationInputV3,
            LifecyclePlanInputV3, LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3,
            LifecycleRefundSourceInputV3, LifecycleRegisterCoordinateV3, LifecycleSeedInputV3,
            encode_lifecycle_policy_v5_atomic,
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

fn funding_profile(actions: FundingActionMaskV3, secondary_prestate: AccountPrestateV2) -> Vec<u8> {
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
            prestate: secondary_prestate,
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
    let mut operations = vec![
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
    if secondary_prestate == AccountPrestateV2::Exact {
        // The FUND control makes coordinate 1 Exact. Its effect permissions
        // therefore need the same explicit owner anchor as the exact payer.
        operations.insert(
            0,
            AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(1),
                expected: IdentityCoordinateV2::common(1),
            },
        );
    }
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
    let funding = [FundingBoundV3::new(0, actions, 64)];
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
            refund_source: LifecycleRefundSourceInputV3::Credit,
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
    let profile_bytes = funding_profile(
        FundingActionMaskV3::CREATE_AND_CLOSE,
        AccountPrestateV2::LifecycleBound,
    );
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
    let profile_bytes = funding_profile(
        FundingActionMaskV3::CREATE_AND_CLOSE,
        AccountPrestateV2::LifecycleBound,
    );
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
        assert_eq!(
            policy
                .validate_account_profile_with_external_funding_join_for_action(profile, 1)
                .err(),
            Some(Error::ProfileMismatch),
            "an action filter must not surrender funding authority separation",
        );
    }
}

#[test]
fn a_funding_profile_cannot_join_an_action_the_policy_does_not_carry() {
    let profile_bytes = funding_profile(
        FundingActionMaskV3::CREATE_AND_CLOSE,
        AccountPrestateV2::LifecycleBound,
    );
    let profile = AccountProfileV3::decode(&profile_bytes).unwrap();
    let bytes = policy(&[1], 2, 3);
    let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &bytes).unwrap();
    policy
        .validate_account_profile_with_external_funding_join_for_action(profile, 1)
        .unwrap();
    assert_eq!(
        policy
            .validate_account_profile_with_external_funding_join_for_action(profile, 2)
            .err(),
        Some(Error::ProfileMismatch)
    );
}

#[test]
fn fund_only_refinement_keeps_lifecycle_recipe_and_create_coverage() {
    let profile_bytes = funding_profile(FundingActionMaskV3::FUND, AccountPrestateV2::Exact);
    let profile = AccountProfileV3::decode(&profile_bytes).expect("V3 profile");
    let bytes = policy(&[0], 2, 3);
    let policy =
        StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &bytes).expect("V5 policy");
    policy
        .validate_account_profile_with_external_funding_join_for_action(profile, 1)
        .expect("FUND must leave Lifecycle its create authority");
}

#[test]
fn fund_only_refinement_cannot_be_a_lifecycle_payer_or_rent_credit() {
    let profile_bytes = funding_profile(FundingActionMaskV3::FUND, AccountPrestateV2::Exact);
    let profile = AccountProfileV3::decode(&profile_bytes).expect("V3 profile");
    for hostile in [policy(&[0], 0, 3), policy(&[0], 2, 0)] {
        let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &hostile)
            .expect("hostile remains a V5 artifact");
        assert_eq!(
            policy
                .validate_account_profile_with_external_funding_join_for_action(profile, 1)
                .err(),
            Some(Error::ProfileMismatch),
            "a FUND-only coordinate is never Lifecycle funding authority",
        );
    }
}

/// Model the seal writer after its real authority-separation check. Artifact
/// digests are supplied by the adapter; this kernel test exercises exact byte
/// range binding, not the adapter's hashing or PDA authentication.
fn seal_for_join(policy: &[u8], profile: &[u8]) -> Vec<u8> {
    use dclutch_vm::capability_seal::{
        CAPABILITY_SEAL_BYTES_V1, CapabilitySealKeyV1, SealedDescriptorClosureV1,
        SealedRecordRowV1, SealedRoleV1,
    };
    let key = CapabilitySealKeyV1::new([1; 32], [2; 32], 1, [3; 32], [4; 32]).expect("seal key");
    let rows = SealedRoleV1::canonical_order().map(|role| {
        let ordinal = u8::try_from(role.ordinal()).expect("bounded seal role");
        let bytes = match role {
            SealedRoleV1::LifecyclePolicy => policy.len(),
            SealedRoleV1::AccountProfile => profile.len(),
            _ => 64,
        };
        let (schema, digest) = if role == SealedRoleV1::Descriptor {
            ([1; 32], [2; 32])
        } else {
            ([0x20 + ordinal; 32], [0x40 + ordinal; 32])
        };
        SealedRecordRowV1::new(
            role,
            u32::try_from(bytes).expect("fixture width"),
            schema,
            digest,
            [0x60 + ordinal; 32],
            [0x70 + ordinal; 32],
        )
        .expect("seal row")
    });
    let mut bytes = vec![0; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(key, rows, 254, &mut bytes).expect("seal body");
    bytes
}

#[test]
fn sealed_funding_join_binds_the_whole_wrapper_and_projects_its_base() {
    use dclutch_vm::{
        account_profile::lifecycle_v3::LifecycleRentQuoteBuffersV5,
        capability_seal::{SealedDescriptorClosureV1, SealedRoleV1},
    };
    let profile_bytes = funding_profile(
        FundingActionMaskV3::CREATE_AND_CLOSE,
        AccountPrestateV2::LifecycleBound,
    );
    let profile = AccountProfileV3::decode(&profile_bytes).expect("funding profile");
    let policy_bytes = policy(&[1], 2, 3);
    let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
        .expect("policy");
    policy
        .validate_account_profile_with_external_funding_join_for_action(profile, 1)
        .expect("seal writer proves funding authority separation for this action");
    let seal_bytes = seal_for_join(&policy_bytes, &profile_bytes);
    let seal = SealedDescriptorClosureV1::decode(&seal_bytes).expect("seal");
    let token = |role, bytes| {
        let row = seal.row(role).expect("row");
        seal.authenticate_artifact(role, row.schema(), row.content_digest(), bytes)
            .expect("adapter-authenticated exact artifact")
    };
    let sealed = seal
        .authenticate_profile_join(
            token(SealedRoleV1::LifecyclePolicy, &policy_bytes),
            token(SealedRoleV1::AccountProfile, &profile_bytes),
        )
        .expect("same seal");
    assert_eq!(
        policy
            .sealed_account_profile_join(profile.base(), sealed)
            .err(),
        Some(Error::InvalidCoordinate),
        "the pre-existing V2 recovery cannot consume a whole-wrapper token",
    );
    let join = policy
        .sealed_account_profile_with_external_funding_join(profile, sealed)
        .expect("recover the proved funded join");
    let mut scratch = [0; 5];
    let mut output = [99; 5];
    policy
        .project_authenticated_current_rent_quotes_atomic(
            profile.base(),
            Some(join),
            0,
            1,
            &[0; 5],
            &[],
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut scratch,
                output_scalars: &mut output,
            },
        )
        .expect("recovered evidence covers the embedded planner profile");
    assert_eq!(output, [0; 5]);

    let other_wrapper_bytes = funding_profile(
        FundingActionMaskV3::CREATE,
        AccountPrestateV2::LifecycleBound,
    );
    let other_wrapper = AccountProfileV3::decode(&other_wrapper_bytes).expect("other wrapper");
    assert_eq!(profile.base().bytes(), other_wrapper.base().bytes());
    assert_eq!(
        policy
            .sealed_account_profile_with_external_funding_join(other_wrapper, sealed)
            .err(),
        Some(Error::InvalidCoordinate),
        "identical embedded profiles do not authorize different funding tables",
    );
    let policy_copy = policy_bytes.clone();
    let other_policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &policy_copy)
        .expect("copied policy");
    assert_eq!(
        other_policy
            .sealed_account_profile_with_external_funding_join(profile, sealed)
            .err(),
        Some(Error::InvalidCoordinate),
        "even equal bytes at another address need their own authenticated token",
    );
}
