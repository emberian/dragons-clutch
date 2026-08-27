//! Hostile corpus for Lifecycle V5 authenticated current-Rent quote projection.

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        ACTION_PLAN_BYTES, AuthenticatedRentQuoteV5, CURRENT_RENT_QUOTE_BYTES_V5, Error,
        HEADER_BYTES, LifecycleRentQuoteBuffersV5, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES,
        SEED_BYTES, StateLifecyclePolicyV4, StateLifecyclePolicyV5,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5, LifecycleGuardInputV3,
            LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3,
            LifecyclePlanInputV3, LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3,
            LifecycleRegisterCoordinateV3, LifecycleSeedInputV3, encode_lifecycle_policy_v4_atomic,
            encode_lifecycle_policy_v5_atomic,
        },
    },
    v2::{
        self, AccountPrestateV2, AccountProfileV2, TrustedEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2,
            AccountRuleInputV2, AccountRuleWithPrestateInputV2, IdentityCoordinateV2,
            RegisterGeometryV2, ScalarCoordinateV2, encode_account_profile_v2_atomic,
            encode_account_profile_with_lifecycle_v2_atomic,
        },
    },
};

const POLICY_ID: [u8; 32] = [0x75; 32];
const QUOTES: [LifecycleCurrentRentQuoteInputV5; 4] = [
    LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 1_152,
        scalar_destination: 38,
    },
    LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 512,
        scalar_destination: 39,
    },
    LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 256,
        scalar_destination: 47,
    },
    LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 182,
        scalar_destination: 48,
    },
];

fn quote_inputs() -> [AuthenticatedRentQuoteV5; 4] {
    [
        AuthenticatedRentQuoteV5 {
            exact_data_len: 1_152,
            scalar_destination: 38,
            current_minimum: 10_001,
        },
        AuthenticatedRentQuoteV5 {
            exact_data_len: 512,
            scalar_destination: 39,
            current_minimum: 10_002,
        },
        AuthenticatedRentQuoteV5 {
            exact_data_len: 256,
            scalar_destination: 47,
            current_minimum: 10_003,
        },
        AuthenticatedRentQuoteV5 {
            exact_data_len: 182,
            scalar_destination: 48,
            current_minimum: 10_004,
        },
    ]
}

fn exact_profile(projected_scalar: Option<u16>) -> Vec<u8> {
    let rules = [AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(false, true, false),
        effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
        alias: AccountAliasInputV2::SelfCoordinate,
        data_length: 8,
        data_item_stride: 0,
    }];
    let mut operations = vec![AccountOperationInputV2::ProjectKey {
        account: AccountCoordinateV2::fixed(0),
        destination: IdentityCoordinateV2::common(0),
    }];
    if let Some(destination) = projected_scalar {
        operations.push(AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(destination),
        });
    }
    encode_exact_profile(&rules, &operations, 50, 1)
}

fn lifecycle_bound_profile() -> Vec<u8> {
    let rules = [AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 8,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::LifecycleBound,
    }];
    let operations = [AccountOperationInputV2::ProjectKey {
        account: AccountCoordinateV2::fixed(0),
        destination: IdentityCoordinateV2::common(0),
    }];
    encode_profile(&rules, &operations, 50, 1)
}

fn protected_lifecycle_profile() -> Vec<u8> {
    let rules = [
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, true, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 152,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        },
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(true, true, false),
                effect_permissions: AccountEffectPermissionsV2::new(true, false, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        },
        AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, true, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, true, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        },
    ];
    let operations = [
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
            data_offset: 0,
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(1),
            data_offset: 8,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(0),
            destination: IdentityCoordinateV2::common(0),
            data_offset: 16,
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(1),
            expected: IdentityCoordinateV2::common(4),
        },
    ];
    encode_profile(&rules, &operations, 50, 5)
}

fn encode_profile(
    rules: &[AccountRuleWithPrestateInputV2],
    operations: &[AccountOperationInputV2],
    common_scalars: u16,
    common_identities: u16,
) -> Vec<u8> {
    let width =
        v2::HEADER_BYTES + rules.len() * v2::RULE_BYTES + operations.len() * v2::OPERATION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_with_lifecycle_v2_atomic(
        TrustedEnvironmentV2::None,
        rules,
        &[],
        operations,
        &[],
        RegisterGeometryV2 {
            common_scalars,
            item_scalar_stride: 0,
            common_identities,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut output,
    )
    .expect("account profile");
    output
}

fn encode_exact_profile(
    rules: &[AccountRuleInputV2],
    operations: &[AccountOperationInputV2],
    common_scalars: u16,
    common_identities: u16,
) -> Vec<u8> {
    let width =
        v2::HEADER_BYTES + rules.len() * v2::RULE_BYTES + operations.len() * v2::OPERATION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_v2_atomic(
        AccountProfileArtifactV2::RuntimeTail,
        rules,
        &[],
        operations,
        &[],
        RegisterGeometryV2 {
            common_scalars,
            item_scalar_stride: 0,
            common_identities,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut output,
    )
    .expect("exact account profile");
    output
}

fn policy(quotes: &[LifecycleCurrentRentQuoteInputV5]) -> Vec<u8> {
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(0),
        seed_start: 0,
        seed_count: 2,
        bump_offset: 1,
        data_base: 8,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"rent-quote-v5"),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: 1,
        operation: LifecycleOperationInputV3::Authenticate,
        recipe: 0,
        payer: None,
        rent_credit: None,
        principal: None,
        beneficiary: None,
        guard: LifecycleGuardInputV3::Always,
    }];
    let protected = [None];
    let width = HEADER_BYTES
        + RECIPE_BYTES
        + 2 * SEED_BYTES
        + ACTION_PLAN_BYTES
        + PROTECTED_OUTPUT_BYTES
        + quotes.len() * CURRENT_RENT_QUOTE_BYTES_V5;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &[],
        quotes,
        &mut scratch,
        &mut output,
    )
    .expect("V5 policy");
    output
}

#[test]
fn four_authenticated_quotes_project_atomically_to_protected_common_scalars() {
    let profile_bytes = exact_profile(None);
    let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
    let policy_bytes = policy(&QUOTES);
    let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
        .expect("V5 policy");
    assert_eq!(policy.current_rent_quote_count(), 4);
    assert_eq!(
        policy
            .current_rent_quote(0)
            .map(|value| (value.exact_data_len(), value.scalar_destination().index())),
        Ok((1_152, 38))
    );
    policy
        .validate_account_profile(profile)
        .expect("protected profile join");

    let input = [0_u64; 50];
    let mut scratch = [0_u64; 50];
    let mut output = [7_u64; 50];
    policy
        .project_authenticated_current_rent_quotes_atomic(
            profile,
            None,
            0,
            &input,
            &quote_inputs(),
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut scratch,
                output_scalars: &mut output,
            },
        )
        .expect("authenticated quote projection");
    assert_eq!(output.get(38), Some(&10_001));
    assert_eq!(output.get(39), Some(&10_002));
    assert_eq!(output.get(47), Some(&10_003));
    assert_eq!(output.get(48), Some(&10_004));
    assert_eq!(output.get(37), Some(&0));
    assert_eq!(output.get(49), Some(&0));
    assert_eq!(
        policy.validate_projected_current_rent_quotes(profile, None, 0, &output, &quote_inputs()),
        Ok(())
    );
    *output.get_mut(47).expect("protected quote") = 1;
    assert_eq!(
        policy.validate_projected_current_rent_quotes(profile, None, 0, &output, &quote_inputs()),
        Err(Error::InvalidRentQuote)
    );
}

#[test]
fn quote_order_width_minimum_and_prefilled_destination_are_refused_atomically() {
    let profile_bytes = exact_profile(None);
    let profile = AccountProfileV2::decode(&profile_bytes).expect("profile");
    let policy_bytes = policy(&QUOTES);
    let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
        .expect("V5 policy");
    let input = [0_u64; 50];

    for hostile in [
        {
            let mut value = quote_inputs();
            value[0].exact_data_len = 512;
            value
        },
        {
            let mut value = quote_inputs();
            value[0].scalar_destination = 39;
            value
        },
        {
            let mut value = quote_inputs();
            value[0].current_minimum = 0;
            value
        },
    ] {
        let mut scratch = [0_u64; 50];
        let mut output = [9_u64; 50];
        assert_eq!(
            policy.project_authenticated_current_rent_quotes_atomic(
                profile,
                None,
                0,
                &input,
                &hostile,
                LifecycleRentQuoteBuffersV5 {
                    scalar_scratch: &mut scratch,
                    output_scalars: &mut output,
                },
            ),
            Err(Error::InvalidRentQuote)
        );
        assert_eq!(output, [9_u64; 50]);
    }

    let mut prefilled = input;
    *prefilled.get_mut(38).expect("quote coordinate") = 1;
    let mut scratch = [0_u64; 50];
    let mut output = [9_u64; 50];
    assert_eq!(
        policy.project_authenticated_current_rent_quotes_atomic(
            profile,
            None,
            0,
            &prefilled,
            &quote_inputs(),
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut scratch,
                output_scalars: &mut output,
            },
        ),
        Err(Error::ProfileMismatch)
    );
    assert_eq!(output, [9_u64; 50]);

    let mut scratch = [0_u64; 50];
    let mut output = [9_u64; 50];
    let inputs = quote_inputs();
    let missing = inputs.get(..3).expect("short quote list");
    assert_eq!(
        policy.project_authenticated_current_rent_quotes_atomic(
            profile,
            None,
            0,
            &input,
            missing,
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut scratch,
                output_scalars: &mut output,
            },
        ),
        Err(Error::InvalidRentQuote)
    );
    assert_eq!(output, [9_u64; 50]);
}

#[test]
fn duplicate_unordered_zero_and_excess_declarations_are_refused_atomically() {
    for hostile in [
        [
            QUOTES[0],
            LifecycleCurrentRentQuoteInputV5 {
                scalar_destination: 38,
                ..QUOTES[1]
            },
            QUOTES[2],
            QUOTES[3],
        ],
        [QUOTES[1], QUOTES[0], QUOTES[2], QUOTES[3]],
        [
            LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: 0,
                ..QUOTES[0]
            },
            QUOTES[1],
            QUOTES[2],
            QUOTES[3],
        ],
    ] {
        assert_failed_policy_encode(&hostile, Error::InvalidRentQuote);
    }

    let excess = [LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 1,
        scalar_destination: 1,
    }; 17];
    assert_failed_policy_encode(&excess, Error::InvalidRentQuote);
}

fn assert_failed_policy_encode(quotes: &[LifecycleCurrentRentQuoteInputV5], expected: Error) {
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(0),
        seed_start: 0,
        seed_count: 2,
        bump_offset: 1,
        data_base: 8,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"rent-quote-v5"),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: 1,
        operation: LifecycleOperationInputV3::Authenticate,
        recipe: 0,
        payer: None,
        rent_credit: None,
        principal: None,
        beneficiary: None,
        guard: LifecycleGuardInputV3::Always,
    }];
    let width = HEADER_BYTES
        + RECIPE_BYTES
        + 2 * SEED_BYTES
        + ACTION_PLAN_BYTES
        + PROTECTED_OUTPUT_BYTES
        + quotes.len() * CURRENT_RENT_QUOTE_BYTES_V5;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0x5a_u8; width];
    let before = output.clone();
    assert_eq!(
        encode_lifecycle_policy_v5_atomic(
            &recipes,
            &seeds,
            &plans,
            &[None],
            &[],
            quotes,
            &mut scratch,
            &mut output,
        ),
        Err(expected)
    );
    assert_eq!(output, before);
}

#[test]
fn account_profile_writer_and_out_of_range_destination_are_refused() {
    let policy_bytes = policy(&QUOTES);
    let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &policy_bytes)
        .expect("V5 policy");
    let writer_bytes = exact_profile(Some(38));
    let writer = AccountProfileV2::decode(&writer_bytes).expect("writer profile");
    assert_eq!(
        policy.validate_account_profile(writer),
        Err(Error::ProfileMismatch)
    );

    let narrow_bytes = encode_exact_profile(
        &[AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 8,
            data_item_stride: 0,
        }],
        &[AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(0),
            destination: IdentityCoordinateV2::common(0),
        }],
        48,
        1,
    );
    let narrow = AccountProfileV2::decode(&narrow_bytes).expect("narrow profile");
    assert_eq!(
        policy.validate_account_profile(narrow),
        Err(Error::ProfileMismatch)
    );
}

#[test]
fn current_rent_destination_cannot_alias_lifecycle_protected_output() {
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(0),
        seed_start: 0,
        seed_count: 2,
        bump_offset: 1,
        data_base: 152,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"protected-v5"),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: 1,
        operation: LifecycleOperationInputV3::AuthenticateOrCreate,
        recipe: 0,
        payer: Some(LifecycleAccountCoordinateV3::fixed(1)),
        rent_credit: Some(LifecycleAccountCoordinateV3::fixed(2)),
        principal: Some(LifecycleRegisterCoordinateV3::common(1)),
        beneficiary: Some(LifecycleRegisterCoordinateV3::common(0)),
        guard: LifecycleGuardInputV3::Always,
    }];
    let protected = [Some(LifecycleProtectedOutputsInputV3 {
        created: 38,
        bump_observation: 0,
        bump: 2,
        historical_rent_principal: 3,
        beneficiary: 1,
        state: 2,
        owner: 3,
    })];
    let quote = [LifecycleCurrentRentQuoteInputV5 {
        exact_data_len: 512,
        scalar_destination: 38,
    }];
    let width = HEADER_BYTES
        + RECIPE_BYTES
        + 2 * SEED_BYTES
        + ACTION_PLAN_BYTES
        + PROTECTED_OUTPUT_BYTES
        + CURRENT_RENT_QUOTE_BYTES_V5;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_lifecycle_policy_v5_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &[],
        &quote,
        &mut scratch,
        &mut output,
    )
    .expect("collision policy bytes");
    let policy = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &output)
        .expect("collision policy decode");
    let profile_bytes = protected_lifecycle_profile();
    let profile = AccountProfileV2::decode(&profile_bytes).expect("protected profile");
    assert_eq!(
        policy.validate_account_profile(profile),
        Err(Error::ProfileMismatch)
    );
}

#[test]
fn canonical_empty_v5_requires_no_lifecycle_bound_account() {
    let mut scratch = [0_u8; HEADER_BYTES];
    let mut output = [0_u8; HEADER_BYTES];
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
        .expect("canonical empty V5");
    let empty = StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &output)
        .expect("empty V5 decode");
    assert!(empty.is_empty());
    let exact_bytes = exact_profile(None);
    let exact = AccountProfileV2::decode(&exact_bytes).expect("exact profile");
    assert_eq!(empty.validate_account_profile(exact), Ok(()));

    let lifecycle_bytes = lifecycle_bound_profile();
    let lifecycle = AccountProfileV2::decode(&lifecycle_bytes).expect("lifecycle profile");
    assert_eq!(
        empty.validate_account_profile(lifecycle),
        Err(Error::ProfileMismatch)
    );

    let mut partial = output;
    partial
        .get_mut(12..14)
        .expect("recipe count")
        .copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &partial),
        Err(Error::EmptyPolicy)
    );
}

#[test]
fn v5_is_distinct_from_v4_and_reserved_bytes_are_hostile_decoded() {
    let mut v5 = policy(&QUOTES);
    assert!(matches!(
        StateLifecyclePolicyV4::decode_selected(POLICY_ID, POLICY_ID, &v5),
        Err(Error::UnsupportedProfile)
    ));
    *v5.get_mut(24).expect("V5 reserved byte") = 1;
    assert!(matches!(
        StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &v5),
        Err(Error::NonCanonicalReserved)
    ));

    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(0),
        seed_start: 0,
        seed_count: 2,
        bump_offset: 1,
        data_base: 8,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"v4"),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [LifecyclePlanInputV3 {
        action: 1,
        operation: LifecycleOperationInputV3::Authenticate,
        recipe: 0,
        payer: None,
        rent_credit: None,
        principal: None,
        beneficiary: None,
        guard: LifecycleGuardInputV3::Always,
    }];
    let width =
        HEADER_BYTES + RECIPE_BYTES + 2 * SEED_BYTES + ACTION_PLAN_BYTES + PROTECTED_OUTPUT_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut v4 = vec![0_u8; width];
    encode_lifecycle_policy_v4_atomic(
        &recipes,
        &seeds,
        &plans,
        &[None],
        &[] as &[LifecycleImmutableIdentityBindingInputV4],
        &mut scratch,
        &mut v4,
    )
    .expect("V4 policy");
    assert!(matches!(
        StateLifecyclePolicyV5::decode_selected(POLICY_ID, POLICY_ID, &v4),
        Err(Error::UnsupportedProfile)
    ));
}
