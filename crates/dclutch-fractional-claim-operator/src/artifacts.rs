//! Canonical action-specialized generic artifacts for the Fractional family.

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        ACTION_PLAN_BYTES, HEADER_BYTES as LIFECYCLE_HEADER_BYTES,
        IMMUTABLE_IDENTITY_BINDING_BYTES, PROTECTED_OUTPUT_BYTES, RECIPE_BYTES, SEED_BYTES,
        StateLifecyclePolicyV4,
        encode::{
            LifecycleAccountCoordinateV3, LifecycleGuardInputV3,
            LifecycleImmutableIdentityBindingInputV4, LifecycleOperationInputV3,
            LifecyclePlanInputV3, LifecycleRecipeInputV3, LifecycleRegisterCoordinateV3,
            LifecycleSeedInputV3, encode_lifecycle_policy_v4_atomic,
        },
    },
    v2::{
        AccountProfileV2, HEADER_BYTES as ACCOUNT_HEADER_BYTES,
        OPERATION_BYTES as ACCOUNT_OPERATION_BYTES, RULE_BYTES as ACCOUNT_RULE_BYTES,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2,
            AccountRuleInputV2, IdentityCoordinateV2, RegisterGeometryV2, ScalarCoordinateV2,
            encode_account_profile_v2_atomic,
        },
    },
};
use dclutch_capability_program_contract::v3::{CAPABILITY_PROGRAM_V3_BYTES, CapabilityProgramV3};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES, OPERATION_BYTES as EFFECT_OPERATION_BYTES,
        ProgramV3 as EffectProgramV3, ROUTE_BYTES as EFFECT_ROUTE_BYTES, RouteKindV3,
        encode::{
            EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3, RequestSpaceV3,
            RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
        },
    },
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_CAPABILITY_KIND_ID_V1, FRACTIONAL_FAMILY_REQUEST_BYTES_V1,
    FRACTIONAL_FAMILY_REQUEST_MAGIC_V1, FRACTIONAL_FAMILY_REQUEST_SCHEMA_ID_V1,
    FRACTIONAL_REQUEST_ACTION_OFFSET_V1, FRACTIONAL_REQUEST_DESTINATION_TOKEN_OFFSET_V1,
    FRACTIONAL_REQUEST_EXPECTED_REVISION_OFFSET_V1, FRACTIONAL_REQUEST_HEADER_RESERVED_BYTES_V1,
    FRACTIONAL_REQUEST_HEADER_RESERVED_OFFSET_V1, FRACTIONAL_REQUEST_MARKET_OFFSET_V1,
    FRACTIONAL_REQUEST_OUTCOME_OFFSET_V1, FRACTIONAL_REQUEST_OWNER_OFFSET_V1,
    FRACTIONAL_REQUEST_PRODUCT_RECORD_OFFSET_V1, FRACTIONAL_REQUEST_QUANTITY_OFFSET_V1,
    FRACTIONAL_REQUEST_RELEASE_SET_OFFSET_V1, FRACTIONAL_REQUEST_RESULT_DOMAIN_OFFSET_V1,
    FRACTIONAL_REQUEST_SOURCE_TOKEN_OFFSET_V1, FRACTIONAL_REQUEST_TAIL_RESERVED_BYTES_V1,
    FRACTIONAL_REQUEST_TAIL_RESERVED_OFFSET_V1, FRACTIONAL_REQUEST_TERMINAL_DIGEST_OFFSET_V1,
    FRACTIONAL_REQUEST_TERMINAL_OUTCOME_OFFSET_V1, FRACTIONAL_REQUEST_TERMS_OFFSET_V1,
    FRACTIONAL_REQUEST_TOKEN_BEHAVIOR_OFFSET_V1, FRACTIONAL_ROOT_BYTES_V1,
    FRACTIONAL_ROOT_MARKET_OFFSET_V1, FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1,
    FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1, FRACTIONAL_ROOT_REVISION_OFFSET_V1,
    FRACTIONAL_ROOT_SCHEMA_ID_V1, FRACTIONAL_ROOT_TERMS_OFFSET_V1, FractionalActionV1,
};
use dclutch_fractional_claim_kernel::FRACTIONAL_TERMS_SCHEMA_ID_V1;
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    IdentityRegisterV3, InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3,
    ScalarRegisterV3, encode_program_atomic,
};
use sha2::{Digest, Sha256};

/// Scalar bank width shared by Account, Request, Transition, and Effect programs.
pub const FRACTIONAL_COMMON_SCALARS_V1: u16 = 7;
/// Identity bank width shared by Account, Request, Transition, and Effect programs.
pub const FRACTIONAL_COMMON_IDENTITIES_V1: u16 = 14;

const ROOT_ACCOUNT: u16 = 0;
const RENT_BENEFICIARY_ACCOUNT: u16 = 1;
const CLAIMS_FRAME_START: u16 = 2;

const S_ACTION: u16 = 0;
const S_EXPECTED_REVISION: u16 = 1;
const S_QUANTITY: u16 = 2;
const S_OUTCOME: u16 = 3;
const S_TERMINAL_OUTCOME: u16 = 4;
const S_ROOT_REVISION: u16 = 5;
const S_ROOT_PRINCIPAL: u16 = 6;

const I_RELEASE_SET: u16 = 0;
const I_MARKET: u16 = 1;
const I_PRODUCT_RECORD: u16 = 2;
const I_RESULT_DOMAIN: u16 = 3;
const I_TERMS: u16 = 4;
const I_TOKEN_BEHAVIOR: u16 = 5;
const I_OWNER: u16 = 6;
const I_SOURCE_TOKEN: u16 = 7;
const I_DESTINATION_TOKEN: u16 = 8;
const I_TERMINAL_DIGEST: u16 = 9;
const I_ROOT_TERMS: u16 = 10;
const I_ROOT_MARKET: u16 = 11;
const I_ROOT_BENEFICIARY: u16 = 12;
const I_RENT_BENEFICIARY_ACCOUNT: u16 = 13;

const REQUEST_INSTRUCTIONS: usize = 20;
const ACCOUNT_OPERATIONS: usize = 6;
const EFFECT_OPERATIONS: usize = 14;
const LIFECYCLE_PLANS: usize = 7;

/// One exact Claims child-frame account rule supplied by the finalized
/// physical FrameSpec compiler, never by a transaction caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimsAccountRuleV1 {
    /// Whether Claims requires signer privilege.
    pub signer: bool,
    /// Whether Claims requires writable privilege.
    pub writable: bool,
    /// Whether this coordinate must be executable.
    pub executable: bool,
    /// Exact observed data width selected by the physical release.
    pub data_length: u32,
}

/// Exact emitted Registry bodies for one action-specialized descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalFinalizedArtifactBundleV1 {
    /// Action selected by this request/transition/effect trio.
    pub action: FractionalActionV1,
    /// Runtime account projection.
    pub account_profile: Vec<u8>,
    /// Root derivation/authentication/close policy.
    pub lifecycle: Vec<u8>,
    /// Exact family request parser.
    pub request_profile: Vec<u8>,
    /// Exact family-local transition checks.
    pub transition: Vec<u8>,
    /// Interpreted ExecutionStrategyV2.
    pub strategy: [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    /// Sole byte-identical Fractional request route to Claims.
    pub effect: Vec<u8>,
    /// CapabilityProgramV3 selecting every emitted body.
    pub descriptor: [u8; CAPABILITY_PROGRAM_V3_BYTES],
}

/// Stable release-compiler refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalArtifactCompilerErrorV1 {
    /// A selected physical identity or Claims frame was empty/oversized.
    InvalidInput,
    /// AccountProfile emission or hostile decoding refused.
    AccountProfile,
    /// Lifecycle policy emission or hostile decoding refused.
    Lifecycle,
    /// RequestProfile emission or hostile decoding refused.
    RequestProfile,
    /// TransitionVM emission or hostile decoding refused.
    Transition,
    /// EffectProgram emission or hostile decoding refused.
    Effect,
    /// ExecutionStrategy construction refused.
    Strategy,
    /// Capability descriptor construction refused.
    Descriptor,
    /// Independently decoded artifact joins differed.
    Validation,
}

/// Emit one deterministic action-specialized generic Fractional artifact set.
///
/// The Claims frame is release-compiler input obtained from its public
/// FrameSpec. The emitted effect forwards the exact same 384-byte Fractional
/// request ABI; it does not define a second child DTO. The root is created by
/// capability activation and only authenticated here until zero-supply retire.
pub fn build_fractional_finalized_artifact_bundle_v1(
    action: FractionalActionV1,
    physical_profile: [u8; 32],
    claims_frame: &[FractionalClaimsAccountRuleV1],
) -> Result<FractionalFinalizedArtifactBundleV1, FractionalArtifactCompilerErrorV1> {
    if is_zero(&physical_profile) || claims_frame.is_empty() {
        return Err(FractionalArtifactCompilerErrorV1::InvalidInput);
    }
    let account_profile = encode_account_profile(claims_frame)?;
    let lifecycle = encode_lifecycle()?;
    let request_profile = encode_request_profile(action)?;
    let transition = encode_transition(action)?;
    let effect = encode_effect(action, claims_frame.len())?;
    let transition_id = content(digest(&transition))?;
    let strategy_value = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        transition_id,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        None,
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(|_| FractionalArtifactCompilerErrorV1::Strategy)?;
    let strategy = strategy_value.to_bytes();
    let descriptor = CapabilityProgramV3::new(
        content(FRACTIONAL_CAPABILITY_KIND_ID_V1)?,
        content(FRACTIONAL_TERMS_SCHEMA_ID_V1)?,
        content(FRACTIONAL_FAMILY_REQUEST_SCHEMA_ID_V1)?,
        content(FRACTIONAL_ROOT_SCHEMA_ID_V1)?,
        content(digest(&account_profile))?,
        content(digest(&lifecycle))?,
        content(physical_profile)?,
        content(digest(&effect))?,
        content(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
        content(digest(&request_profile))?,
        content(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?,
        content(digest(&strategy))?,
        u32::try_from(FRACTIONAL_ROOT_BYTES_V1)
            .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?,
    )
    .map_err(|_| FractionalArtifactCompilerErrorV1::Descriptor)?
    .encode();
    let bundle = FractionalFinalizedArtifactBundleV1 {
        action,
        account_profile,
        lifecycle,
        request_profile,
        transition,
        strategy,
        effect,
        descriptor,
    };
    validate_bundle(&bundle, claims_frame.len(), physical_profile)?;
    Ok(bundle)
}

fn encode_account_profile(
    claims_frame: &[FractionalClaimsAccountRuleV1],
) -> Result<Vec<u8>, FractionalArtifactCompilerErrorV1> {
    let fixed_count = CLAIMS_FRAME_START
        .checked_add(
            u16::try_from(claims_frame.len())
                .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?,
        )
        .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)?;
    let no_effect = AccountEffectPermissionsV2::new(false, false, false);
    let mut rules = Vec::with_capacity(usize::from(fixed_count));
    rules.push(AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(false, true, false),
        effect_permissions: no_effect,
        alias: AccountAliasInputV2::SelfCoordinate,
        data_length: u32::try_from(FRACTIONAL_ROOT_BYTES_V1)
            .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?,
        data_item_stride: 0,
    });
    rules.push(AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(false, true, false),
        effect_permissions: no_effect,
        alias: AccountAliasInputV2::SelfCoordinate,
        data_length: 0,
        data_item_stride: 0,
    });
    rules.extend(claims_frame.iter().map(|rule| AccountRuleInputV2 {
        privileges: AccountPrivilegesV2::new(rule.signer, rule.writable, rule.executable),
        effect_permissions: no_effect,
        alias: AccountAliasInputV2::SelfCoordinate,
        data_length: rule.data_length,
        data_item_stride: 0,
    }));
    let operations = [
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(ROOT_ACCOUNT),
            destination: IdentityCoordinateV2::common(I_ROOT_TERMS),
            data_offset: narrow_u32(FRACTIONAL_ROOT_TERMS_OFFSET_V1)?,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(ROOT_ACCOUNT),
            destination: IdentityCoordinateV2::common(I_ROOT_MARKET),
            data_offset: narrow_u32(FRACTIONAL_ROOT_MARKET_OFFSET_V1)?,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(ROOT_ACCOUNT),
            destination: IdentityCoordinateV2::common(I_ROOT_BENEFICIARY),
            data_offset: narrow_u32(FRACTIONAL_ROOT_RENT_BENEFICIARY_OFFSET_V1)?,
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(RENT_BENEFICIARY_ACCOUNT),
            destination: IdentityCoordinateV2::common(I_RENT_BENEFICIARY_ACCOUNT),
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(ROOT_ACCOUNT),
            destination: ScalarCoordinateV2::common(S_ROOT_REVISION),
            data_offset: narrow_u32(FRACTIONAL_ROOT_REVISION_OFFSET_V1)?,
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(ROOT_ACCOUNT),
            destination: ScalarCoordinateV2::common(S_ROOT_PRINCIPAL),
            data_offset: narrow_u32(FRACTIONAL_ROOT_RENT_PRINCIPAL_OFFSET_V1)?,
        },
    ];
    let bytes = ACCOUNT_HEADER_BYTES
        .checked_add(
            rules
                .len()
                .checked_mul(ACCOUNT_RULE_BYTES)
                .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)?,
        )
        .and_then(|value| value.checked_add(ACCOUNT_OPERATIONS * ACCOUNT_OPERATION_BYTES))
        .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)?;
    let mut scratch = vec![0; bytes];
    let mut output = vec![0; bytes];
    encode_account_profile_v2_atomic(
        AccountProfileArtifactV2::RuntimeTail,
        &rules,
        &[],
        &operations,
        &[],
        register_geometry(),
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalArtifactCompilerErrorV1::AccountProfile)?;
    Ok(output)
}

fn encode_lifecycle() -> Result<Vec<u8>, FractionalArtifactCompilerErrorV1> {
    let recipes = [LifecycleRecipeInputV3 {
        state: LifecycleAccountCoordinateV3::fixed(ROOT_ACCOUNT),
        seed_start: 0,
        seed_count: 4,
        bump_offset: 3,
        data_base: u32::try_from(FRACTIONAL_ROOT_BYTES_V1)
            .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?,
        data_stride: 0,
    }];
    let seeds = [
        LifecycleSeedInputV3::Literal(b"dclutch/fractional-root-v1"),
        LifecycleSeedInputV3::CommonIdentity(I_TERMS),
        LifecycleSeedInputV3::CommonIdentity(I_MARKET),
        LifecycleSeedInputV3::CanonicalBump,
    ];
    let plans = core::array::from_fn::<_, LIFECYCLE_PLANS, _>(|index| {
        let retire = index == usize::from(FractionalActionV1::ZeroSupplyRetire.byte());
        LifecyclePlanInputV3 {
            action: u32::try_from(index).unwrap_or(u32::MAX),
            operation: if retire {
                LifecycleOperationInputV3::Close
            } else {
                LifecycleOperationInputV3::Authenticate
            },
            recipe: 0,
            payer: None,
            rent_credit: retire.then_some(LifecycleAccountCoordinateV3::fixed(
                RENT_BENEFICIARY_ACCOUNT,
            )),
            principal: retire.then_some(LifecycleRegisterCoordinateV3::common(S_ROOT_PRINCIPAL)),
            beneficiary: retire
                .then_some(LifecycleRegisterCoordinateV3::common(I_ROOT_BENEFICIARY)),
            guard: LifecycleGuardInputV3::Always,
        }
    });
    let protected = [None; LIFECYCLE_PLANS];
    let bindings: [LifecycleImmutableIdentityBindingInputV4; 0] = [];
    let bytes = LIFECYCLE_HEADER_BYTES
        + RECIPE_BYTES
        + seeds.len() * SEED_BYTES
        + plans.len() * ACTION_PLAN_BYTES
        + protected.len() * PROTECTED_OUTPUT_BYTES
        + bindings.len() * IMMUTABLE_IDENTITY_BINDING_BYTES;
    let mut scratch = vec![0; bytes];
    let mut output = vec![0; bytes];
    encode_lifecycle_policy_v4_atomic(
        &recipes,
        &seeds,
        &plans,
        &protected,
        &bindings,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalArtifactCompilerErrorV1::Lifecycle)?;
    Ok(output)
}

fn encode_request_profile(
    action: FractionalActionV1,
) -> Result<Vec<u8>, FractionalArtifactCompilerErrorV1> {
    let request = |offset| {
        u32::try_from(offset)
            .map(RequestCoordinateV1::fixed)
            .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)
    };
    let scalar = |index| ScalarRegisterV1::common(index);
    let identity = |index| IdentityRegisterV1::common(index);
    let instructions = [
        RequestInstructionV1::require_u64(
            request(0)?,
            u64::from_le_bytes(FRACTIONAL_FAMILY_REQUEST_MAGIC_V1),
        ),
        RequestInstructionV1::require_u16(request(8)?, 1),
        RequestInstructionV1::require_u8(
            request(FRACTIONAL_REQUEST_ACTION_OFFSET_V1)?,
            action.byte(),
        ),
        RequestInstructionV1::project_u8(
            request(FRACTIONAL_REQUEST_ACTION_OFFSET_V1)?,
            scalar(S_ACTION),
        ),
        RequestInstructionV1::require_zero(
            request(FRACTIONAL_REQUEST_HEADER_RESERVED_OFFSET_V1)?,
            u32::try_from(FRACTIONAL_REQUEST_HEADER_RESERVED_BYTES_V1)
                .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?,
        ),
        RequestInstructionV1::require_zero(
            request(FRACTIONAL_REQUEST_TAIL_RESERVED_OFFSET_V1)?,
            u32::try_from(FRACTIONAL_REQUEST_TAIL_RESERVED_BYTES_V1)
                .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?,
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_RELEASE_SET_OFFSET_V1)?,
            identity(I_RELEASE_SET),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_MARKET_OFFSET_V1)?,
            identity(I_MARKET),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_PRODUCT_RECORD_OFFSET_V1)?,
            identity(I_PRODUCT_RECORD),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_RESULT_DOMAIN_OFFSET_V1)?,
            identity(I_RESULT_DOMAIN),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_TERMS_OFFSET_V1)?,
            identity(I_TERMS),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_TOKEN_BEHAVIOR_OFFSET_V1)?,
            identity(I_TOKEN_BEHAVIOR),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_OWNER_OFFSET_V1)?,
            identity(I_OWNER),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_SOURCE_TOKEN_OFFSET_V1)?,
            identity(I_SOURCE_TOKEN),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_DESTINATION_TOKEN_OFFSET_V1)?,
            identity(I_DESTINATION_TOKEN),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_REQUEST_TERMINAL_DIGEST_OFFSET_V1)?,
            identity(I_TERMINAL_DIGEST),
        ),
        RequestInstructionV1::project_u64(
            request(FRACTIONAL_REQUEST_EXPECTED_REVISION_OFFSET_V1)?,
            scalar(S_EXPECTED_REVISION),
        ),
        RequestInstructionV1::project_u64(
            request(FRACTIONAL_REQUEST_QUANTITY_OFFSET_V1)?,
            scalar(S_QUANTITY),
        ),
        RequestInstructionV1::project_u32(
            request(FRACTIONAL_REQUEST_OUTCOME_OFFSET_V1)?,
            scalar(S_OUTCOME),
        ),
        RequestInstructionV1::project_u32(
            request(FRACTIONAL_REQUEST_TERMINAL_OUTCOME_OFFSET_V1)?,
            scalar(S_TERMINAL_OUTCOME),
        ),
    ];
    let bytes = REQUEST_HEADER_BYTES + REQUEST_INSTRUCTIONS * REQUEST_OPERATION_BYTES;
    let mut scratch = vec![0; bytes];
    let mut output = vec![0; bytes];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(FRACTIONAL_FAMILY_REQUEST_BYTES_V1)
                .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?,
            0,
            FRACTIONAL_COMMON_SCALARS_V1,
            0,
            FRACTIONAL_COMMON_IDENTITIES_V1,
            0,
        ),
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalArtifactCompilerErrorV1::RequestProfile)?;
    Ok(output)
}

fn encode_transition(
    action: FractionalActionV1,
) -> Result<Vec<u8>, FractionalArtifactCompilerErrorV1> {
    let scalar = ScalarRegisterV3::common;
    let identity = IdentityRegisterV3::common;
    let mut instructions = vec![
        InstructionV3::scalar_eq(scalar(S_EXPECTED_REVISION), scalar(S_ROOT_REVISION)),
        InstructionV3::identity_eq(identity(I_TERMS), identity(I_ROOT_TERMS)),
        InstructionV3::identity_eq(identity(I_MARKET), identity(I_ROOT_MARKET)),
        InstructionV3::identity_eq(
            identity(I_ROOT_BENEFICIARY),
            identity(I_RENT_BENEFICIARY_ACCOUNT),
        ),
    ];
    if action.carries_quantity() {
        instructions.push(InstructionV3::nonzero(scalar(S_QUANTITY)));
    }
    let bytes = TRANSITION_HEADER_BYTES + instructions.len() * TRANSITION_INSTRUCTION_BYTES;
    let mut scratch = vec![0; bytes];
    let mut output = vec![0; bytes];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: FRACTIONAL_COMMON_SCALARS_V1,
            item_scalar_stride: 0,
            common_identities: FRACTIONAL_COMMON_IDENTITIES_V1,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalArtifactCompilerErrorV1::Transition)?;
    Ok(output)
}

fn encode_effect(
    action: FractionalActionV1,
    claims_accounts: usize,
) -> Result<Vec<u8>, FractionalArtifactCompilerErrorV1> {
    let claims_accounts = u16::try_from(claims_accounts)
        .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?;
    let fixed_accounts = CLAIMS_FRAME_START
        .checked_add(claims_accounts)
        .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)?;
    let mut template = [0; FRACTIONAL_FAMILY_REQUEST_BYTES_V1];
    put(&mut template, 0, &FRACTIONAL_FAMILY_REQUEST_MAGIC_V1)?;
    put(&mut template, 8, &1_u16.to_le_bytes())?;
    *template
        .get_mut(FRACTIONAL_REQUEST_ACTION_OFFSET_V1)
        .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)? = action.byte();
    let route = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: CLAIMS_FRAME_START,
        fixed_account_count: claims_accounts,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &template,
        item_request: &[],
    }];
    let identity = IdentityCoordinateV3::common;
    let scalar = ScalarCoordinateV3::common;
    let operations = [
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_RELEASE_SET_OFFSET_V1)?,
            identity(I_RELEASE_SET),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_MARKET_OFFSET_V1)?,
            identity(I_MARKET),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_PRODUCT_RECORD_OFFSET_V1)?,
            identity(I_PRODUCT_RECORD),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_RESULT_DOMAIN_OFFSET_V1)?,
            identity(I_RESULT_DOMAIN),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_TERMS_OFFSET_V1)?,
            identity(I_TERMS),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_TOKEN_BEHAVIOR_OFFSET_V1)?,
            identity(I_TOKEN_BEHAVIOR),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_OWNER_OFFSET_V1)?,
            identity(I_OWNER),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_SOURCE_TOKEN_OFFSET_V1)?,
            identity(I_SOURCE_TOKEN),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_DESTINATION_TOKEN_OFFSET_V1)?,
            identity(I_DESTINATION_TOKEN),
        ),
        EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_TERMINAL_DIGEST_OFFSET_V1)?,
            identity(I_TERMINAL_DIGEST),
        ),
        EffectInstructionV3::write_request_u64(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_EXPECTED_REVISION_OFFSET_V1)?,
            scalar(S_EXPECTED_REVISION),
        ),
        EffectInstructionV3::write_request_u64(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_QUANTITY_OFFSET_V1)?,
            scalar(S_QUANTITY),
        ),
        EffectInstructionV3::write_request_u32(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_OUTCOME_OFFSET_V1)?,
            scalar(S_OUTCOME),
        ),
        EffectInstructionV3::write_request_u32(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(FRACTIONAL_REQUEST_TERMINAL_OUTCOME_OFFSET_V1)?,
            scalar(S_TERMINAL_OUTCOME),
        ),
    ];
    let bytes = EFFECT_HEADER_BYTES
        + EFFECT_ROUTE_BYTES
        + EFFECT_OPERATIONS * EFFECT_OPERATION_BYTES
        + template.len();
    let mut scratch = vec![0; bytes];
    let mut output = vec![0; bytes];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts,
            item_account_stride: 0,
            common_scalars: FRACTIONAL_COMMON_SCALARS_V1,
            item_scalar_stride: 0,
            common_identities: FRACTIONAL_COMMON_IDENTITIES_V1,
            item_identity_stride: 0,
        },
        &route,
        &operations,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalArtifactCompilerErrorV1::Effect)?;
    Ok(output)
}

fn validate_bundle(
    bundle: &FractionalFinalizedArtifactBundleV1,
    claims_accounts: usize,
    physical_profile: [u8; 32],
) -> Result<(), FractionalArtifactCompilerErrorV1> {
    let descriptor = CapabilityProgramV3::decode(&bundle.descriptor)
        .map_err(|_| FractionalArtifactCompilerErrorV1::Descriptor)?;
    let account = AccountProfileV2::decode(&bundle.account_profile)
        .map_err(|_| FractionalArtifactCompilerErrorV1::AccountProfile)?;
    let lifecycle_id = digest(&bundle.lifecycle);
    let lifecycle =
        StateLifecyclePolicyV4::decode_selected(lifecycle_id, lifecycle_id, &bundle.lifecycle)
            .map_err(|_| FractionalArtifactCompilerErrorV1::Lifecycle)?;
    let request_id = digest(&bundle.request_profile);
    let request =
        RequestProfileV1::decode_selected(request_id, request_id, &bundle.request_profile)
            .map_err(|_| FractionalArtifactCompilerErrorV1::RequestProfile)?;
    let transition = TransitionProgramV3::decode(&bundle.transition)
        .map_err(|_| FractionalArtifactCompilerErrorV1::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
        .map_err(|_| FractionalArtifactCompilerErrorV1::Strategy)?;
    let effect_id = digest(&bundle.effect);
    let effect = EffectProgramV3::decode_selected(effect_id, effect_id, &bundle.effect)
        .map_err(|_| FractionalArtifactCompilerErrorV1::Effect)?;
    strategy
        .validate_descriptor_selection(content(digest(&bundle.strategy))?, descriptor)
        .map_err(|_| FractionalArtifactCompilerErrorV1::Validation)?;
    lifecycle
        .validate_account_profile(account)
        .map_err(|_| FractionalArtifactCompilerErrorV1::Validation)?;
    let fixed_accounts = usize::from(CLAIMS_FRAME_START)
        .checked_add(claims_accounts)
        .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)?;
    if descriptor.kind().to_bytes() != FRACTIONAL_CAPABILITY_KIND_ID_V1
        || descriptor.config_schema().to_bytes() != FRACTIONAL_TERMS_SCHEMA_ID_V1
        || descriptor.request_schema().to_bytes() != FRACTIONAL_FAMILY_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema().to_bytes() != FRACTIONAL_ROOT_SCHEMA_ID_V1
        || descriptor.account_profile().to_bytes() != digest(&bundle.account_profile)
        || descriptor.derivation_policy().to_bytes() != lifecycle_id
        || descriptor.capacity_profile().to_bytes() != physical_profile
        || descriptor.effect_program().to_bytes() != effect_id
        || descriptor.request_profile_program().to_bytes() != request_id
        || descriptor.transition_program().to_bytes() != digest(&bundle.strategy)
        || strategy.transition_program().to_bytes() != digest(&bundle.transition)
        || usize::from(account.fixed_account_count()) != fixed_accounts
        || usize::from(effect.fixed_account_count()) != fixed_accounts
        || request
            .request_bytes(0)
            .map_err(|_| FractionalArtifactCompilerErrorV1::Validation)?
            != FRACTIONAL_FAMILY_REQUEST_BYTES_V1
        || account.common_scalar_count() != FRACTIONAL_COMMON_SCALARS_V1
        || request.common_scalar_count() != FRACTIONAL_COMMON_SCALARS_V1
        || transition.common_scalar_count() != FRACTIONAL_COMMON_SCALARS_V1
        || effect.common_scalar_count() != FRACTIONAL_COMMON_SCALARS_V1
        || account.common_identity_count() != FRACTIONAL_COMMON_IDENTITIES_V1
        || request.common_identity_count() != FRACTIONAL_COMMON_IDENTITIES_V1
        || transition.common_identity_count() != FRACTIONAL_COMMON_IDENTITIES_V1
        || effect.common_identity_count() != FRACTIONAL_COMMON_IDENTITIES_V1
        || lifecycle
            .action_plan_count(u32::from(bundle.action.byte()))
            .map_err(|_| FractionalArtifactCompilerErrorV1::Validation)?
            != 1
    {
        return Err(FractionalArtifactCompilerErrorV1::Validation);
    }
    let route = effect
        .route(0)
        .map_err(|_| FractionalArtifactCompilerErrorV1::Effect)?;
    if route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_start() != CLAIMS_FRAME_START
        || usize::from(route.fixed_account_count()) != claims_accounts
        || route.fixed_request_bytes()
            != u32::try_from(FRACTIONAL_FAMILY_REQUEST_BYTES_V1)
                .map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)?
    {
        return Err(FractionalArtifactCompilerErrorV1::Validation);
    }
    Ok(())
}

fn register_geometry() -> RegisterGeometryV2 {
    RegisterGeometryV2 {
        common_scalars: FRACTIONAL_COMMON_SCALARS_V1,
        item_scalar_stride: 0,
        common_identities: FRACTIONAL_COMMON_IDENTITIES_V1,
        item_identity_stride: 0,
    }
}

fn narrow_u32(value: usize) -> Result<u32, FractionalArtifactCompilerErrorV1> {
    u32::try_from(value).map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn content(value: [u8; 32]) -> Result<ContentId, FractionalArtifactCompilerErrorV1> {
    ContentId::new(value).map_err(|_| FractionalArtifactCompilerErrorV1::InvalidInput)
}

fn put(
    output: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), FractionalArtifactCompilerErrorV1> {
    let end = offset
        .checked_add(value.len())
        .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)?;
    output
        .get_mut(offset..end)
        .ok_or(FractionalArtifactCompilerErrorV1::InvalidInput)?
        .copy_from_slice(value);
    Ok(())
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}
