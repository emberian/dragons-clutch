//! Canonical Hot artifacts for Dealer LP-position Open and Close.
//!
//! LP positions are real Trading PDAs. Open uses the generic lifecycle
//! adapter's dust-tolerant AuthenticateOrCreate primitive, while the semantic
//! program requires the protected `created` result. Close is permissionless,
//! returns every lamport to the immutable RentCredit, and admits only a live
//! zero-share state. No Dealer-specific account writer exists.

#[cfg(not(target_os = "solana"))]
extern crate alloc;

#[cfg(not(target_os = "solana"))]
use alloc::{vec, vec::Vec};

#[cfg(not(target_os = "solana"))]
use dclutch_account_profile_contract::{
    lifecycle_v3::{
        ACTION_PLAN_BYTES as LIFECYCLE_PLAN_BYTES, HEADER_BYTES as LIFECYCLE_HEADER_BYTES,
        IMMUTABLE_IDENTITY_BINDING_BYTES as LIFECYCLE_IDENTITY_BINDING_BYTES,
        PROTECTED_OUTPUT_BYTES as LIFECYCLE_PROTECTED_BYTES,
        RECIPE_BYTES as LIFECYCLE_RECIPE_BYTES, SEED_BYTES as LIFECYCLE_SEED_BYTES,
        StateLifecyclePolicyV4,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleGuardInputV3,
            LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3,
            LifecyclePlanInputV3, LifecycleProtectedOutputsInputV3, LifecycleRecipeInputV3,
            LifecycleRegisterCoordinateV3, LifecycleSeedInputV3, encode_lifecycle_policy_v4_atomic,
        },
    },
    v2::{
        AccountPrestateV2, AccountProfileV2, TrustedEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
            ScalarCoordinateV2, encode_account_profile_with_lifecycle_v2_atomic,
        },
    },
};

use super::{
    v3_multi_lp::{
        DEALER_LP_POSITION_MAGIC_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3,
        DEALER_LP_POSITION_VERSION_V3,
    },
    v3_operator::MultiLpRequestActionV3,
};

#[cfg(not(target_os = "solana"))]
use dclutch_effect_kernel::v3::{
    HEADER_BYTES as EFFECT_HEADER_BYTES, OPERATION_BYTES as EFFECT_OPERATION_BYTES,
    ProgramV3 as EffectProgramV3,
    encode::{
        AccountCoordinateV3 as EffectAccountCoordinateV3, EffectGeometryV3, EffectInstructionV3,
        IdentityCoordinateV3 as EffectIdentityCoordinateV3,
        ScalarCoordinateV3 as EffectScalarCoordinateV3, encode_effect_program_v3_atomic,
    },
};
#[cfg(not(target_os = "solana"))]
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
#[cfg(not(target_os = "solana"))]
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3,
    ScalarRegisterV3, encode_program_atomic,
};

use super::v3_operator::{
    DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3, DEALER_MULTI_LP_REQUEST_MAGIC_V3,
    DEALER_MULTI_LP_REQUEST_VERSION_V3,
};

/// Common Hot-injected account count: root/config/Product/portfolio/basis.
pub const DEALER_LP_INJECTED_ACCOUNTS_V3: u16 = 5;
/// Canonical obligation account coordinate.
pub const DEALER_LP_OBLIGATION_ACCOUNT_V3: u16 = 5;
/// Lifecycle-owned LP state/vacancy coordinate.
pub const DEALER_LP_STATE_ACCOUNT_V3: u16 = 6;
/// Open-only payer coordinate.
pub const DEALER_LP_OPEN_PAYER_ACCOUNT_V3: u16 = 7;
/// Open RentCredit coordinate.
pub const DEALER_LP_OPEN_RENT_CREDIT_ACCOUNT_V3: u16 = 8;
/// Open System Program coordinate.
pub const DEALER_LP_OPEN_SYSTEM_ACCOUNT_V3: u16 = 9;
/// Close RentCredit coordinate.
pub const DEALER_LP_CLOSE_RENT_CREDIT_ACCOUNT_V3: u16 = 7;
/// Close System Program coordinate.
pub const DEALER_LP_CLOSE_SYSTEM_ACCOUNT_V3: u16 = 8;
/// Exact Open logical account count.
pub const DEALER_LP_OPEN_ACCOUNT_COUNT_V3: u16 = 10;
/// Exact Close logical account count.
pub const DEALER_LP_CLOSE_ACCOUNT_COUNT_V3: u16 = 9;
const DEALER_LP_POSITION_BYTES_U32_V3: u32 = 256;
const DEALER_MULTI_LP_REQUEST_BYTES_U32_V3: u32 = 312;

/// Trusted current-slot scalar.
pub const LP_CURRENT_SLOT_SCALAR_V3: u16 = 0;
/// Request expiry scalar.
pub const LP_EXPIRY_SCALAR_V3: u16 = 1;
/// Request historical rent principal scalar.
pub const LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3: u16 = 2;
/// Request Market generation scalar.
pub const LP_GENERATION_SCALAR_V3: u16 = 3;
/// Request optimistic obligation revision scalar.
pub const LP_EXPECTED_OBLIGATION_REVISION_SCALAR_V3: u16 = 4;
/// Request optimistic LP revision scalar.
pub const LP_EXPECTED_REVISION_SCALAR_V3: u16 = 5;
/// AccountProfile-observed obligation revision scalar.
pub const LP_OBSERVED_OBLIGATION_REVISION_SCALAR_V3: u16 = 6;
/// AccountProfile-observed LP revision; zero for a vacant state.
pub const LP_OBSERVED_REVISION_SCALAR_V3: u16 = 7;
/// AccountProfile-observed LP share balance; zero for vacancy.
pub const LP_OBSERVED_SHARES_SCALAR_V3: u16 = 8;
/// AccountProfile-observed immutable historical rent principal.
pub const LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3: u16 = 9;
/// AccountProfile-observed LP lamports, including dust.
pub const LP_OBSERVED_LAMPORTS_SCALAR_V3: u16 = 10;
/// Canonical LP magic loaded by Transition.
pub const LP_MAGIC_SCALAR_V3: u16 = 11;
/// Canonical LP wire version loaded by Transition.
pub const LP_VERSION_SCALAR_V3: u16 = 12;
/// Initial optimistic revision loaded by Transition.
pub const LP_INITIAL_REVISION_SCALAR_V3: u16 = 13;
/// Canonical zero loaded by Transition.
pub const LP_ZERO_SCALAR_V3: u16 = 14;
/// Persisted bump observation projected from a live LP state.
pub const LP_BUMP_OBSERVATION_SCALAR_V3: u16 = 15;
/// Lifecycle-owned AOC branch: one only when Open creates.
pub const LP_CREATED_SCALAR_V3: u16 = 16;
/// Lifecycle-owned canonical PDA bump.
pub const LP_CANONICAL_BUMP_SCALAR_V3: u16 = 17;
/// Lifecycle-owned current historical rent principal.
pub const LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3: u16 = 18;
/// Exact common scalar bank width.
pub const DEALER_LP_SCALAR_COUNT_V3: u16 = 19;

/// Common Hot parent request digest.
pub const LP_PARENT_REQUEST_DIGEST_IDENTITY_V3: u16 = 0;
/// Request release-set identity.
pub const LP_RELEASE_IDENTITY_V3: u16 = 1;
/// Request Market identity.
pub const LP_MARKET_IDENTITY_V3: u16 = 2;
/// Request child-root identity.
pub const LP_CHILD_ROOT_IDENTITY_V3: u16 = 3;
/// Request LP-position address.
pub const LP_POSITION_IDENTITY_V3: u16 = 4;
/// Request LP owner identity.
pub const LP_OWNER_IDENTITY_V3: u16 = 5;
/// Request obligation address.
pub const LP_OBLIGATION_IDENTITY_V3: u16 = 6;
/// Request obligation digest.
pub const LP_OBLIGATION_DIGEST_IDENTITY_V3: u16 = 7;
/// Request LP prestate digest; zero only for Open.
pub const LP_PRESTATE_DIGEST_IDENTITY_V3: u16 = 8;
/// AccountProfile-observed obligation address.
pub const LP_OBSERVED_OBLIGATION_IDENTITY_V3: u16 = 9;
/// AccountProfile-observed LP address.
pub const LP_OBSERVED_POSITION_IDENTITY_V3: u16 = 10;
/// AccountProfile-observed live release set.
pub const LP_OBSERVED_RELEASE_IDENTITY_V3: u16 = 11;
/// AccountProfile-observed live Market.
pub const LP_OBSERVED_MARKET_IDENTITY_V3: u16 = 12;
/// AccountProfile-observed live child root.
pub const LP_OBSERVED_CHILD_ROOT_IDENTITY_V3: u16 = 13;
/// AccountProfile-observed live LP owner.
pub const LP_OBSERVED_OWNER_IDENTITY_V3: u16 = 14;
/// AccountProfile-observed immutable RentCredit beneficiary.
pub const LP_OBSERVED_REFUND_IDENTITY_V3: u16 = 15;
/// AccountProfile-observed live obligation address.
pub const LP_OBSERVED_STATE_OBLIGATION_IDENTITY_V3: u16 = 16;
/// Unwritten zero identity used to authenticate a System-owned payer.
pub const LP_SYSTEM_OWNER_IDENTITY_V3: u16 = 17;
/// Lifecycle-owned immutable RentCredit beneficiary.
pub const LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3: u16 = 18;
/// Lifecycle-owned exact LP state key.
pub const LP_LIFECYCLE_STATE_IDENTITY_V3: u16 = 19;
/// Lifecycle-owned current Trading owner.
pub const LP_LIFECYCLE_OWNER_IDENTITY_V3: u16 = 20;
/// Exact common identity bank width.
pub const DEALER_LP_IDENTITY_COUNT_V3: u16 = 21;

/// Stable LP artifact construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerLpArtifactErrorV3 {
    /// Action or physical account width differed.
    Geometry,
    /// AccountProfile6 refused the typed shape.
    AccountProfile,
    /// StateLifecyclePolicy refused the canonical recipe.
    Lifecycle,
    /// RequestProfile refused the exact signed wire.
    RequestProfile,
    /// Transition refused the exact semantic checks.
    Transition,
    /// EffectProgram refused local state authority.
    Effect,
}

/// Exact finalized account lengths for one LP physical frame.
#[cfg(not(target_os = "solana"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpAccountProfileInputV3<'a> {
    /// Open or Close selector shape.
    pub action: MultiLpRequestActionV3,
    /// Exact data length at every logical coordinate.
    pub logical_data_lengths: &'a [u32],
}

/// Exact logical account count for one LP lifecycle action.
pub const fn dealer_lp_account_count_v3(action: MultiLpRequestActionV3) -> u16 {
    match action {
        MultiLpRequestActionV3::Open => DEALER_LP_OPEN_ACCOUNT_COUNT_V3,
        MultiLpRequestActionV3::Close => DEALER_LP_CLOSE_ACCOUNT_COUNT_V3,
    }
}

/// Encode one exact Profile6 LP lifecycle account projection.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_lp_account_profile_v3(
    input: DealerLpAccountProfileInputV3<'_>,
) -> Result<Vec<u8>, DealerLpArtifactErrorV3> {
    let count = dealer_lp_account_count_v3(input.action);
    if input.logical_data_lengths.len() != usize::from(count)
        || input
            .logical_data_lengths
            .get(usize::from(DEALER_LP_STATE_ACCOUNT_V3))
            != Some(&DEALER_LP_POSITION_BYTES_U32_V3)
    {
        return Err(DealerLpArtifactErrorV3::Geometry);
    }
    let (payer, rent_credit, system) = match input.action {
        MultiLpRequestActionV3::Open => (
            Some(DEALER_LP_OPEN_PAYER_ACCOUNT_V3),
            DEALER_LP_OPEN_RENT_CREDIT_ACCOUNT_V3,
            DEALER_LP_OPEN_SYSTEM_ACCOUNT_V3,
        ),
        MultiLpRequestActionV3::Close => (
            None,
            DEALER_LP_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            DEALER_LP_CLOSE_SYSTEM_ACCOUNT_V3,
        ),
    };
    if input.logical_data_lengths.get(usize::from(rent_credit)) != Some(&48)
        || input.logical_data_lengths.get(usize::from(system)) != Some(&0)
        || payer.is_some_and(|coordinate| {
            input.logical_data_lengths.get(usize::from(coordinate)) != Some(&0)
        })
    {
        return Err(DealerLpArtifactErrorV3::Geometry);
    }
    let rules = (0..count)
        .map(|coordinate| {
            let state = coordinate == DEALER_LP_STATE_ACCOUNT_V3;
            let payer_coordinate = payer == Some(coordinate);
            let credit = coordinate == rent_credit;
            let executable = coordinate == system;
            Ok(AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(
                        payer_coordinate,
                        coordinate == 0 || state || payer_coordinate || credit,
                        executable,
                    ),
                    effect_permissions: AccountEffectPermissionsV2::new(
                        state || payer_coordinate,
                        state || credit,
                        state,
                    ),
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: input
                        .logical_data_lengths
                        .get(usize::from(coordinate))
                        .copied()
                        .ok_or(DealerLpArtifactErrorV3::Geometry)?,
                    data_item_stride: 0,
                },
                prestate: if state {
                    AccountPrestateV2::LifecycleBound
                } else {
                    AccountPrestateV2::Exact
                },
            })
        })
        .collect::<Result<Vec<_>, DealerLpArtifactErrorV3>>()?;
    let mut operations = vec![
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(DEALER_LP_OBLIGATION_ACCOUNT_V3),
            destination: IdentityCoordinateV2::common(LP_OBSERVED_OBLIGATION_IDENTITY_V3),
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(DEALER_LP_OBLIGATION_ACCOUNT_V3),
            destination: ScalarCoordinateV2::common(LP_OBSERVED_OBLIGATION_REVISION_SCALAR_V3),
            data_offset: 16,
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(DEALER_LP_STATE_ACCOUNT_V3),
            destination: IdentityCoordinateV2::common(LP_OBSERVED_POSITION_IDENTITY_V3),
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(DEALER_LP_STATE_ACCOUNT_V3),
            destination: ScalarCoordinateV2::common(LP_OBSERVED_REVISION_SCALAR_V3),
            data_offset: 16,
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(DEALER_LP_STATE_ACCOUNT_V3),
            destination: ScalarCoordinateV2::common(LP_OBSERVED_SHARES_SCALAR_V3),
            data_offset: 216,
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(DEALER_LP_STATE_ACCOUNT_V3),
            destination: ScalarCoordinateV2::common(LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3),
            data_offset: 232,
        },
        AccountOperationInputV2::ProjectDataU16 {
            account: AccountCoordinateV2::fixed(DEALER_LP_STATE_ACCOUNT_V3),
            destination: ScalarCoordinateV2::common(LP_BUMP_OBSERVATION_SCALAR_V3),
            data_offset: 240,
        },
    ];
    for (offset, destination) in [
        (24, LP_OBSERVED_RELEASE_IDENTITY_V3),
        (56, LP_OBSERVED_MARKET_IDENTITY_V3),
        (88, LP_OBSERVED_CHILD_ROOT_IDENTITY_V3),
        (120, LP_OBSERVED_OWNER_IDENTITY_V3),
        (152, LP_OBSERVED_REFUND_IDENTITY_V3),
        (184, LP_OBSERVED_STATE_OBLIGATION_IDENTITY_V3),
    ] {
        operations.push(AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(DEALER_LP_STATE_ACCOUNT_V3),
            destination: IdentityCoordinateV2::common(destination),
            data_offset: offset,
        });
    }
    if let Some(payer) = payer {
        operations.push(AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(payer),
            expected: IdentityCoordinateV2::common(LP_SYSTEM_OWNER_IDENTITY_V3),
        });
    }
    let bytes = dclutch_account_profile_contract::v2::HEADER_BYTES
        + usize::from(count) * dclutch_account_profile_contract::v2::RULE_BYTES
        + operations.len() * dclutch_account_profile_contract::v2::OPERATION_BYTES;
    let mut scratch = vec![0; bytes];
    let mut output = vec![0; bytes];
    encode_account_profile_with_lifecycle_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: LP_CURRENT_SLOT_SCALAR_V3,
        },
        &rules,
        &[],
        &operations,
        &[],
        RegisterGeometryV2 {
            common_scalars: DEALER_LP_SCALAR_COUNT_V3,
            item_scalar_stride: 0,
            common_identities: DEALER_LP_IDENTITY_COUNT_V3,
            item_identity_stride: 0,
        },
        &mut scratch,
        &mut output,
    )
    .map_err(|_| DealerLpArtifactErrorV3::AccountProfile)?;
    AccountProfileV2::decode(&output).map_err(|_| DealerLpArtifactErrorV3::AccountProfile)?;
    Ok(output)
}

/// Exact successor lifecycle artifact width, including every immutable LP identity.
#[cfg(not(target_os = "solana"))]
pub const DEALER_LP_LIFECYCLE_BYTES_V3: usize = LIFECYCLE_HEADER_BYTES
    + LIFECYCLE_RECIPE_BYTES
    + 4 * LIFECYCLE_SEED_BYTES
    + 2 * LIFECYCLE_PLAN_BYTES
    + 2 * LIFECYCLE_PROTECTED_BYTES
    + 6 * LIFECYCLE_IDENTITY_BINDING_BYTES;

/// Encode one policy containing the canonical Open-AOC and Close plans.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_lp_lifecycle_v3(
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerLpArtifactErrorV3> {
    if scratch.len() != DEALER_LP_LIFECYCLE_BYTES_V3 || output.len() != DEALER_LP_LIFECYCLE_BYTES_V3
    {
        return Err(DealerLpArtifactErrorV3::Geometry);
    }
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(DEALER_LP_STATE_ACCOUNT_V3),
        seed_start: 0,
        seed_count: 4,
        bump_offset: 3,
        data_base: DEALER_LP_POSITION_BYTES_U32_V3,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(DEALER_LP_POSITION_PDA_DOMAIN_V3),
        LifecycleSeedInputV3::CommonIdentity(LP_CHILD_ROOT_IDENTITY_V3),
        LifecycleSeedInputV3::CommonIdentity(LP_OWNER_IDENTITY_V3),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = [
        LifecyclePlanInputV3 {
            action: u32::from(MultiLpRequestActionV3::Open.selector()),
            operation: LifecycleOperationInputV3::AuthenticateOrCreate,
            recipe: 0,
            payer: Some(LifecycleAccountCoordinateV3::fixed(
                DEALER_LP_OPEN_PAYER_ACCOUNT_V3,
            )),
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                DEALER_LP_OPEN_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(
                LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3,
            )),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(
                LP_OBSERVED_REFUND_IDENTITY_V3,
            )),
            guard: LifecycleGuardInputV3::Always,
        },
        LifecyclePlanInputV3 {
            action: u32::from(MultiLpRequestActionV3::Close.selector()),
            operation: LifecycleOperationInputV3::Close,
            recipe: 0,
            payer: None,
            rent_credit: Some(LifecycleAccountCoordinateV3::fixed(
                DEALER_LP_CLOSE_RENT_CREDIT_ACCOUNT_V3,
            )),
            principal: Some(LifecycleRegisterCoordinateV3::common(
                LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3,
            )),
            beneficiary: Some(LifecycleRegisterCoordinateV3::common(
                LP_OBSERVED_REFUND_IDENTITY_V3,
            )),
            guard: LifecycleGuardInputV3::Always,
        },
    ];
    let protected = [
        Some(LifecycleProtectedOutputsInputV3 {
            created: LP_CREATED_SCALAR_V3,
            bump_observation: LP_BUMP_OBSERVATION_SCALAR_V3,
            bump: LP_CANONICAL_BUMP_SCALAR_V3,
            historical_rent_principal: LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3,
            beneficiary: LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3,
            state: LP_LIFECYCLE_STATE_IDENTITY_V3,
            owner: LP_LIFECYCLE_OWNER_IDENTITY_V3,
        }),
        None,
    ];
    let immutable_bindings = [
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: 24,
            canonical: LifecycleRegisterCoordinateV3::common(LP_RELEASE_IDENTITY_V3),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: 56,
            canonical: LifecycleRegisterCoordinateV3::common(LP_MARKET_IDENTITY_V3),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: 88,
            canonical: LifecycleRegisterCoordinateV3::common(LP_CHILD_ROOT_IDENTITY_V3),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: 120,
            canonical: LifecycleRegisterCoordinateV3::common(LP_OWNER_IDENTITY_V3),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: 152,
            canonical: LifecycleRegisterCoordinateV3::common(LP_OWNER_IDENTITY_V3),
        },
        LifecycleImmutableIdentityBindingInputV4 {
            plan: 0,
            data_offset: 184,
            canonical: LifecycleRegisterCoordinateV3::common(LP_OBLIGATION_IDENTITY_V3),
        },
    ];
    encode_lifecycle_policy_v4_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &immutable_bindings,
        scratch,
        output,
    )
    .map_err(|_| DealerLpArtifactErrorV3::Lifecycle)?;
    StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], output)
        .map_err(|_| DealerLpArtifactErrorV3::Lifecycle)?;
    Ok(())
}

#[cfg(not(target_os = "solana"))]
const DEALER_LP_REQUEST_PROFILE_OPERATIONS_V3: usize = 17;
/// Exact LP RequestProfile bytes.
#[cfg(not(target_os = "solana"))]
pub const DEALER_LP_REQUEST_PROFILE_BYTES_V3: usize =
    REQUEST_HEADER_BYTES + DEALER_LP_REQUEST_PROFILE_OPERATIONS_V3 * REQUEST_OPERATION_BYTES;

/// Encode the exact fixed-width LP request projector.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_lp_request_profile_v3(
    action: MultiLpRequestActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerLpArtifactErrorV3> {
    if scratch.len() != DEALER_LP_REQUEST_PROFILE_BYTES_V3
        || output.len() != DEALER_LP_REQUEST_PROFILE_BYTES_V3
    {
        return Err(DealerLpArtifactErrorV3::Geometry);
    }
    let instructions = [
        RequestInstructionV1::require_u64(
            RequestCoordinateV1::fixed(0),
            u64::from_le_bytes(DEALER_MULTI_LP_REQUEST_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            RequestCoordinateV1::fixed(8),
            DEALER_MULTI_LP_REQUEST_VERSION_V3,
        ),
        RequestInstructionV1::require_u16(
            RequestCoordinateV1::fixed(DEALER_MULTI_LP_ACTION_SELECTOR_OFFSET_V3),
            action.selector(),
        ),
        RequestInstructionV1::require_zero(RequestCoordinateV1::fixed(12), 4),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(16),
            IdentityRegisterV1::common(LP_RELEASE_IDENTITY_V3),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(48),
            IdentityRegisterV1::common(LP_MARKET_IDENTITY_V3),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(80),
            IdentityRegisterV1::common(LP_CHILD_ROOT_IDENTITY_V3),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(112),
            IdentityRegisterV1::common(LP_POSITION_IDENTITY_V3),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(144),
            IdentityRegisterV1::common(LP_OWNER_IDENTITY_V3),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(176),
            IdentityRegisterV1::common(LP_OBLIGATION_IDENTITY_V3),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(208),
            IdentityRegisterV1::common(LP_OBLIGATION_DIGEST_IDENTITY_V3),
        ),
        RequestInstructionV1::project_identity(
            RequestCoordinateV1::fixed(240),
            IdentityRegisterV1::common(LP_PRESTATE_DIGEST_IDENTITY_V3),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(272),
            ScalarRegisterV1::common(LP_EXPECTED_OBLIGATION_REVISION_SCALAR_V3),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(280),
            ScalarRegisterV1::common(LP_EXPECTED_REVISION_SCALAR_V3),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(288),
            ScalarRegisterV1::common(LP_GENERATION_SCALAR_V3),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(296),
            ScalarRegisterV1::common(LP_EXPIRY_SCALAR_V3),
        ),
        RequestInstructionV1::project_u64(
            RequestCoordinateV1::fixed(304),
            ScalarRegisterV1::common(LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3),
        ),
    ];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            DEALER_MULTI_LP_REQUEST_BYTES_U32_V3,
            0,
            DEALER_LP_SCALAR_COUNT_V3,
            0,
            DEALER_LP_IDENTITY_COUNT_V3,
            0,
        ),
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DealerLpArtifactErrorV3::RequestProfile)?;
    RequestProfileV1::decode(output).map_err(|_| DealerLpArtifactErrorV3::RequestProfile)?;
    Ok(())
}

/// Exact transition instruction count for Open or Close.
#[cfg(not(target_os = "solana"))]
pub const fn dealer_lp_transition_operation_count_v3(action: MultiLpRequestActionV3) -> usize {
    match action {
        MultiLpRequestActionV3::Open => 15,
        MultiLpRequestActionV3::Close => 18,
    }
}

/// Exact LP TransitionVM bytes.
#[cfg(not(target_os = "solana"))]
pub const fn dealer_lp_transition_bytes_v3(action: MultiLpRequestActionV3) -> usize {
    TRANSITION_HEADER_BYTES
        + dealer_lp_transition_operation_count_v3(action) * TRANSITION_INSTRUCTION_BYTES
}

/// Encode the action-specific LP transition and lifecycle joins.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_lp_transition_v3(
    action: MultiLpRequestActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerLpArtifactErrorV3> {
    if scratch.len() != dealer_lp_transition_bytes_v3(action)
        || output.len() != dealer_lp_transition_bytes_v3(action)
    {
        return Err(DealerLpArtifactErrorV3::Geometry);
    }
    let scalar = |index| ScalarRegisterV3::common(index);
    let identity = |index| IdentityRegisterV3::common(index);
    let mut operations = vec![
        InstructionV3::load_const(
            scalar(LP_MAGIC_SCALAR_V3),
            u64::from_le_bytes(DEALER_LP_POSITION_MAGIC_V3),
        ),
        InstructionV3::load_const(
            scalar(LP_VERSION_SCALAR_V3),
            u64::from(DEALER_LP_POSITION_VERSION_V3),
        ),
        InstructionV3::load_const(scalar(LP_INITIAL_REVISION_SCALAR_V3), 1),
        InstructionV3::load_const(scalar(LP_ZERO_SCALAR_V3), 0),
        InstructionV3::scalar_le(
            scalar(LP_CURRENT_SLOT_SCALAR_V3),
            scalar(LP_EXPIRY_SCALAR_V3),
        ),
        InstructionV3::nonzero(scalar(LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3)),
        InstructionV3::scalar_eq(
            scalar(LP_OBSERVED_OBLIGATION_REVISION_SCALAR_V3),
            scalar(LP_EXPECTED_OBLIGATION_REVISION_SCALAR_V3),
        ),
        InstructionV3::identity_eq(
            identity(LP_OBSERVED_OBLIGATION_IDENTITY_V3),
            identity(LP_OBLIGATION_IDENTITY_V3),
        ),
        InstructionV3::identity_eq(
            identity(LP_OBSERVED_POSITION_IDENTITY_V3),
            identity(LP_POSITION_IDENTITY_V3),
        ),
    ];
    match action {
        MultiLpRequestActionV3::Open => operations.extend_from_slice(&[
            InstructionV3::scalar_eq(
                scalar(LP_OBSERVED_REVISION_SCALAR_V3),
                scalar(LP_ZERO_SCALAR_V3),
            ),
            InstructionV3::scalar_eq(
                scalar(LP_CREATED_SCALAR_V3),
                scalar(LP_INITIAL_REVISION_SCALAR_V3),
            ),
            InstructionV3::scalar_eq(
                scalar(LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3),
                scalar(LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3),
            ),
            InstructionV3::identity_eq(
                identity(LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3),
                identity(LP_OWNER_IDENTITY_V3),
            ),
            InstructionV3::identity_eq(
                identity(LP_LIFECYCLE_STATE_IDENTITY_V3),
                identity(LP_POSITION_IDENTITY_V3),
            ),
            InstructionV3::identity_ne(
                identity(LP_LIFECYCLE_OWNER_IDENTITY_V3),
                identity(LP_SYSTEM_OWNER_IDENTITY_V3),
            ),
        ]),
        MultiLpRequestActionV3::Close => operations.extend_from_slice(&[
            InstructionV3::scalar_eq(
                scalar(LP_OBSERVED_REVISION_SCALAR_V3),
                scalar(LP_EXPECTED_REVISION_SCALAR_V3),
            ),
            InstructionV3::scalar_eq(
                scalar(LP_OBSERVED_SHARES_SCALAR_V3),
                scalar(LP_ZERO_SCALAR_V3),
            ),
            InstructionV3::scalar_eq(
                scalar(LP_OBSERVED_RENT_PRINCIPAL_SCALAR_V3),
                scalar(LP_REQUEST_RENT_PRINCIPAL_SCALAR_V3),
            ),
            InstructionV3::identity_eq(
                identity(LP_OBSERVED_RELEASE_IDENTITY_V3),
                identity(LP_RELEASE_IDENTITY_V3),
            ),
            InstructionV3::identity_eq(
                identity(LP_OBSERVED_MARKET_IDENTITY_V3),
                identity(LP_MARKET_IDENTITY_V3),
            ),
            InstructionV3::identity_eq(
                identity(LP_OBSERVED_CHILD_ROOT_IDENTITY_V3),
                identity(LP_CHILD_ROOT_IDENTITY_V3),
            ),
            InstructionV3::identity_eq(
                identity(LP_OBSERVED_OWNER_IDENTITY_V3),
                identity(LP_OWNER_IDENTITY_V3),
            ),
            InstructionV3::identity_eq(
                identity(LP_OBSERVED_STATE_OBLIGATION_IDENTITY_V3),
                identity(LP_OBLIGATION_IDENTITY_V3),
            ),
            InstructionV3::identity_ne(
                identity(LP_PRESTATE_DIGEST_IDENTITY_V3),
                identity(LP_SYSTEM_OWNER_IDENTITY_V3),
            ),
        ]),
    }
    if operations.len() != dealer_lp_transition_operation_count_v3(action) {
        return Err(DealerLpArtifactErrorV3::Geometry);
    }
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: DEALER_LP_SCALAR_COUNT_V3,
            item_scalar_stride: 0,
            common_identities: DEALER_LP_IDENTITY_COUNT_V3,
            item_identity_stride: 0,
        },
        &operations,
        &[],
        &[],
        scratch,
        output,
    )
    .map_err(|_| DealerLpArtifactErrorV3::Transition)?;
    TransitionProgramV3::decode(output).map_err(|_| DealerLpArtifactErrorV3::Transition)?;
    Ok(())
}

/// Exact local Effect instruction count.
#[cfg(not(target_os = "solana"))]
pub const fn dealer_lp_effect_operation_count_v3(action: MultiLpRequestActionV3) -> usize {
    match action {
        MultiLpRequestActionV3::Open => 13,
        MultiLpRequestActionV3::Close => 0,
    }
}

/// Exact LP EffectProgram bytes.
#[cfg(not(target_os = "solana"))]
pub const fn dealer_lp_effect_bytes_v3(action: MultiLpRequestActionV3) -> usize {
    EFFECT_HEADER_BYTES + dealer_lp_effect_operation_count_v3(action) * EFFECT_OPERATION_BYTES
}

/// Encode generic local writes which initialize a newly created LP PDA.
#[cfg(not(target_os = "solana"))]
pub fn encode_dealer_lp_effect_v3(
    action: MultiLpRequestActionV3,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), DealerLpArtifactErrorV3> {
    if scratch.len() != dealer_lp_effect_bytes_v3(action)
        || output.len() != dealer_lp_effect_bytes_v3(action)
    {
        return Err(DealerLpArtifactErrorV3::Geometry);
    }
    let account = EffectAccountCoordinateV3::fixed(DEALER_LP_STATE_ACCOUNT_V3);
    let scalar = |index| EffectScalarCoordinateV3::common(index);
    let identity = |index| EffectIdentityCoordinateV3::common(index);
    let instructions = if action == MultiLpRequestActionV3::Open {
        vec![
            EffectInstructionV3::write_u64(account, 0, scalar(LP_MAGIC_SCALAR_V3)),
            EffectInstructionV3::write_u16(account, 8, scalar(LP_VERSION_SCALAR_V3)),
            EffectInstructionV3::write_u64(account, 16, scalar(LP_INITIAL_REVISION_SCALAR_V3)),
            EffectInstructionV3::write_identity(account, 24, identity(LP_RELEASE_IDENTITY_V3)),
            EffectInstructionV3::write_identity(account, 56, identity(LP_MARKET_IDENTITY_V3)),
            EffectInstructionV3::write_identity(account, 88, identity(LP_CHILD_ROOT_IDENTITY_V3)),
            EffectInstructionV3::write_identity(account, 120, identity(LP_OWNER_IDENTITY_V3)),
            EffectInstructionV3::write_identity(
                account,
                152,
                identity(LP_LIFECYCLE_BENEFICIARY_IDENTITY_V3),
            ),
            EffectInstructionV3::write_identity(account, 184, identity(LP_OBLIGATION_IDENTITY_V3)),
            EffectInstructionV3::write_u64(account, 216, scalar(LP_ZERO_SCALAR_V3)),
            EffectInstructionV3::write_u64(account, 224, scalar(LP_GENERATION_SCALAR_V3)),
            EffectInstructionV3::write_u64(
                account,
                232,
                scalar(LP_LIFECYCLE_RENT_PRINCIPAL_SCALAR_V3),
            ),
            EffectInstructionV3::write_u16(account, 240, scalar(LP_CANONICAL_BUMP_SCALAR_V3)),
        ]
    } else {
        vec![]
    };
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: dealer_lp_account_count_v3(action),
            item_account_stride: 0,
            common_scalars: DEALER_LP_SCALAR_COUNT_V3,
            item_scalar_stride: 0,
            common_identities: DEALER_LP_IDENTITY_COUNT_V3,
            item_identity_stride: 0,
        },
        &[],
        &instructions,
        &[],
        scratch,
        output,
    )
    .map_err(|_| DealerLpArtifactErrorV3::Effect)?;
    EffectProgramV3::decode(output).map_err(|_| DealerLpArtifactErrorV3::Effect)?;
    Ok(())
}

#[cfg(all(test, not(target_os = "solana")))]
mod tests {
    use super::*;

    fn lengths(action: MultiLpRequestActionV3) -> Vec<u32> {
        let mut output = vec![0; usize::from(dealer_lp_account_count_v3(action))];
        output[usize::from(DEALER_LP_OBLIGATION_ACCOUNT_V3)] = 208;
        output[usize::from(DEALER_LP_STATE_ACCOUNT_V3)] = DEALER_LP_POSITION_BYTES_U32_V3;
        let credit = match action {
            MultiLpRequestActionV3::Open => DEALER_LP_OPEN_RENT_CREDIT_ACCOUNT_V3,
            MultiLpRequestActionV3::Close => DEALER_LP_CLOSE_RENT_CREDIT_ACCOUNT_V3,
        };
        output[usize::from(credit)] = 48;
        output
    }

    #[test]
    fn open_and_close_profiles_join_one_canonical_lifecycle_policy() {
        let mut lifecycle_scratch = vec![0; DEALER_LP_LIFECYCLE_BYTES_V3];
        let mut lifecycle = vec![0; DEALER_LP_LIFECYCLE_BYTES_V3];
        encode_dealer_lp_lifecycle_v3(&mut lifecycle_scratch, &mut lifecycle).expect("lifecycle");
        let policy = StateLifecyclePolicyV4::decode_selected([1; 32], [1; 32], &lifecycle)
            .expect("decode lifecycle");
        for action in [MultiLpRequestActionV3::Open, MultiLpRequestActionV3::Close] {
            let lengths = lengths(action);
            let profile = encode_dealer_lp_account_profile_v3(DealerLpAccountProfileInputV3 {
                action,
                logical_data_lengths: &lengths,
            })
            .expect("profile");
            let profile = AccountProfileV2::decode(&profile).expect("decode profile");
            policy
                .validate_account_profile(profile)
                .expect("profile-policy join");
            assert_eq!(
                profile.trusted_current_slot_scalar(),
                Some(LP_CURRENT_SLOT_SCALAR_V3)
            );
        }
        let open = policy
            .action_plan(u32::from(MultiLpRequestActionV3::Open.selector()), 0)
            .expect("open plan");
        assert_eq!(open.protected_output_count(), Ok(6));
        assert_eq!(open.protected_observation_count(), Ok(3));
        let close = policy
            .action_plan(u32::from(MultiLpRequestActionV3::Close.selector()), 0)
            .expect("close plan");
        assert_eq!(close.protected_output_count(), Ok(0));
    }

    #[test]
    fn profile_width_and_lifecycle_bytes_refuse_atomically() {
        let short = vec![0; usize::from(DEALER_LP_OPEN_ACCOUNT_COUNT_V3 - 1)];
        assert_eq!(
            encode_dealer_lp_account_profile_v3(DealerLpAccountProfileInputV3 {
                action: MultiLpRequestActionV3::Open,
                logical_data_lengths: &short,
            }),
            Err(DealerLpArtifactErrorV3::Geometry)
        );
        let mut scratch = vec![0; DEALER_LP_LIFECYCLE_BYTES_V3];
        let mut output = vec![0xa5; DEALER_LP_LIFECYCLE_BYTES_V3 - 1];
        let before = output.clone();
        assert_eq!(
            encode_dealer_lp_lifecycle_v3(&mut scratch, &mut output),
            Err(DealerLpArtifactErrorV3::Geometry)
        );
        assert_eq!(output, before);
    }

    #[test]
    fn request_transition_and_effect_artifacts_cover_both_lifecycle_actions() {
        for action in [MultiLpRequestActionV3::Open, MultiLpRequestActionV3::Close] {
            let mut request_scratch = vec![0; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
            let mut request = vec![0; DEALER_LP_REQUEST_PROFILE_BYTES_V3];
            encode_dealer_lp_request_profile_v3(action, &mut request_scratch, &mut request)
                .expect("request profile");
            let request = RequestProfileV1::decode(&request).expect("decode request profile");
            assert_eq!(
                request.fixed_request_bytes(),
                DEALER_MULTI_LP_REQUEST_BYTES_U32_V3
            );
            assert_eq!(request.common_scalar_count(), DEALER_LP_SCALAR_COUNT_V3);
            assert_eq!(request.common_identity_count(), DEALER_LP_IDENTITY_COUNT_V3);

            let transition_bytes = dealer_lp_transition_bytes_v3(action);
            let mut transition_scratch = vec![0; transition_bytes];
            let mut transition = vec![0; transition_bytes];
            encode_dealer_lp_transition_v3(action, &mut transition_scratch, &mut transition)
                .expect("transition");
            let transition = TransitionProgramV3::decode(&transition).expect("decode transition");
            assert_eq!(transition.common_scalar_count(), DEALER_LP_SCALAR_COUNT_V3);
            assert_eq!(
                transition.common_identity_count(),
                DEALER_LP_IDENTITY_COUNT_V3
            );

            let effect_bytes = dealer_lp_effect_bytes_v3(action);
            let mut effect_scratch = vec![0; effect_bytes];
            let mut effect = vec![0; effect_bytes];
            encode_dealer_lp_effect_v3(action, &mut effect_scratch, &mut effect).expect("effect");
            let effect = EffectProgramV3::decode(&effect).expect("decode effect");
            assert_eq!(
                effect.fixed_account_count(),
                dealer_lp_account_count_v3(action)
            );
            assert_eq!(
                usize::from(effect.fixed_operation_count()),
                dealer_lp_effect_operation_count_v3(action)
            );
        }
    }
}
