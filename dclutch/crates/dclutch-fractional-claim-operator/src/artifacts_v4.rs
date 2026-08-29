//! Exact CapabilityV4/LifecycleV5/Profile13 artifacts for executable Fractional Claims actions.

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as LIFECYCLE_SCHEMA_ID_V5,
        HEADER_BYTES as LIFECYCLE_HEADER_BYTES, StateLifecyclePolicyV5,
        encode::encode_lifecycle_policy_v5_atomic,
    },
    v2::{
        AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
        DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES,
        RULE_BYTES as ACCOUNT_RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
        TrustedIdentityEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
            AccountRuleWithPrestateInputV2, IdentityCoordinateV2, RegisterGeometryV2,
            ScalarCoordinateV2, encode_account_profile_with_dynamic_fixed_span_v2_atomic,
        },
    },
};
use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{
        HEADER_BYTES as EFFECT_HEADER_BYTES, OPERATION_BYTES as EFFECT_OPERATION_BYTES,
        ROUTE_BYTES as EFFECT_ROUTE_BYTES, RouteKindV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3,
            RequestSpaceV3, RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
        },
    },
    v4::{
        BorrowedRangePolicyV4, HEADER_BYTES_V4 as EFFECT_V4_HEADER_BYTES,
        ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4,
        encode_program_v4_atomic,
    },
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ROOT_V3,
    FRACTIONAL_CAPABILITY_KIND_ID_V1, FRACTIONAL_CAPABILITY_ROOT_BYTES_V4,
    FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4, FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2, FRACTIONAL_EXPOSURE_REQUEST_DESTINATION_TOKEN_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_EXPECTED_REVISION_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_EXPOSURE_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_HEADER_RESERVED_OFFSET_V2, FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2,
    FRACTIONAL_EXPOSURE_REQUEST_MARKET_OFFSET_V2, FRACTIONAL_EXPOSURE_REQUEST_OWNER_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_PRODUCT_RECORD_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_QUANTITY_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_RELEASE_SET_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_REPRESENTATION_COORDINATE_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_RESULT_DOMAIN_OFFSET_V2, FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_ID_V2,
    FRACTIONAL_EXPOSURE_REQUEST_SOURCE_TOKEN_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_TAIL_RESERVED_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_TERMINAL_DIGEST_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_TERMS_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2, FRACTIONAL_ROOT_BYTES_V1,
    FRACTIONAL_ROOT_MARKET_OFFSET_V1, FRACTIONAL_ROOT_REVISION_OFFSET_V1,
    FRACTIONAL_ROOT_SCHEMA_ID_V1, FRACTIONAL_ROOT_TERMS_OFFSET_V1,
    FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3, FRACTIONAL_TERMINAL_ROOT_V3, FractionalExposureActionV2,
};
use dclutch_fractional_claim_kernel::FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2;
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

use crate::FractionalClaimsAccountRuleV1;

/// Logical accounts injected by common Hot before the Claims child frame.
pub const FRACTIONAL_HOT_INJECTED_ACCOUNT_COUNT_V4: u16 = 5;
/// Exact common scalar-bank width for the executable Fractional V4 family.
pub const FRACTIONAL_COMMON_SCALARS_V4: u16 = 6;
/// Exact common identity-bank width for the executable Fractional V4 family.
pub const FRACTIONAL_COMMON_IDENTITIES_V4: u16 = 16;

const ROOT: u16 = 0;
const SELECTED_CONFIG: u16 = 1;
const PRODUCT_RECORD: u16 = 2;
const PORTFOLIO_RECORD: u16 = 3;
const LINKED_BASIS: u16 = 4;

const S_ACTION: u16 = 0;
const S_EXPECTED_REVISION: u16 = 1;
const S_QUANTITY: u16 = 2;
const S_REPRESENTATION_COORDINATE: u16 = 3;
const S_ROOT_REVISION: u16 = 4;
const S_POST_REVISION: u16 = 5;

const I_RELEASE_SET: u16 = 0;
const I_MARKET: u16 = 1;
const I_PRODUCT_RECORD: u16 = 2;
const I_RESULT_DOMAIN: u16 = 3;
const I_TERMS: u16 = 4;
const I_TOKEN_BEHAVIOR: u16 = 5;
const I_EXPOSURE: u16 = 6;
const I_OWNER: u16 = 7;
const I_SOURCE_TOKEN: u16 = 8;
const I_DESTINATION_TOKEN: u16 = 9;
const I_TERMINAL_DIGEST: u16 = 10;
const I_ROOT_TERMS: u16 = 11;
const I_ROOT_MARKET: u16 = 12;
const I_SELECTED_CONFIG: u16 = 13;
const I_AUTHENTICATED_PRODUCT: u16 = 14;
const I_TRADING_PROGRAM: u16 = 15;

/// Exact authenticated prefix observations needed to emit Profile13.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSelectedProfileInputV4 {
    /// Minimum checked bytes of the independently authenticated terms/config Record.
    pub selected_config_bytes: u32,
    /// Exact finalized Product root Record bytes.
    pub product_record_bytes: u32,
    /// Exact finalized Product portfolio Record bytes.
    pub portfolio_record_bytes: u32,
    /// Minimum checked bytes of the independently authenticated linked basis Record.
    pub linked_basis_bytes: u32,
}

/// Complete exact executable artifact-compiler input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSelectedBundleInputV4<'a> {
    /// One of the four Claims-backed executable exposure actions.
    pub action: FractionalExposureActionV2,
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// Authenticated common Hot prefix widths.
    pub profile: FractionalSelectedProfileInputV4,
    /// Exact public Claims FrameSpec projection for this action.
    pub claims_frame: &'a [FractionalClaimsAccountRuleV1],
}

/// Exact CapabilityV4/LifecycleV5/Profile13 artifact bodies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalSelectedBundleV4 {
    /// Action specialized by every family-local artifact.
    pub action: FractionalExposureActionV2,
    /// Profile13 logical-account interpreter.
    pub account_profile: Vec<u8>,
    /// Empty lifecycle policy: generic activation already authenticates the sole root PDA.
    pub lifecycle_policy: Vec<u8>,
    /// Exact 416-byte request interpreter.
    pub request_profile: Vec<u8>,
    /// Root/release/Product-bound economic transition.
    pub transition: Vec<u8>,
    /// Interpreted Transition strategy.
    pub strategy: [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    /// EffectV4 with one exact Claims route and commit-last root revision write.
    pub effect: Vec<u8>,
    /// Exact CapabilityV4 descriptor selecting all six artifacts.
    pub descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
}

/// Stable V4 artifact compiler refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalSelectedArtifactErrorV4 {
    /// Action, identity, account width, or frame shape was not executable.
    InvalidInput,
    /// Profile13 construction or hostile decoding refused.
    AccountProfile,
    /// LifecycleV5 construction or hostile decoding refused.
    Lifecycle,
    /// RequestProfile construction or hostile decoding refused.
    RequestProfile,
    /// TransitionVM construction or hostile decoding refused.
    Transition,
    /// ExecutionStrategy construction or join refused.
    Strategy,
    /// EffectV4 construction or hostile decoding refused.
    Effect,
    /// CapabilityV4 construction or join refused.
    Descriptor,
    /// Independently decoded artifact geometry differed.
    Validation,
}

/// Emit and hostile-validate one action-specialized executable Fractional bundle.
pub fn build_fractional_selected_bundle_v4(
    input: FractionalSelectedBundleInputV4<'_>,
) -> Result<FractionalSelectedBundleV4, FractionalSelectedArtifactErrorV4> {
    let (claims_accounts, root_coordinate) = action_geometry(input.action)?;
    if input.capacity_profile == [0; 32]
        || input.claims_frame.len() != usize::from(claims_accounts)
        || [
            input.profile.selected_config_bytes,
            input.profile.product_record_bytes,
            input.profile.portfolio_record_bytes,
            input.profile.linked_basis_bytes,
        ]
        .contains(&0)
    {
        return Err(FractionalSelectedArtifactErrorV4::InvalidInput);
    }
    let root_rule = input
        .claims_frame
        .get(usize::from(root_coordinate))
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    if !root_rule.signer || !root_rule.writable || root_rule.executable {
        return Err(FractionalSelectedArtifactErrorV4::InvalidInput);
    }
    let account_profile = encode_account_profile(input, root_coordinate)?;
    let lifecycle_policy = encode_empty_lifecycle()?;
    let request_profile = encode_request_profile(input.action)?;
    let transition = encode_transition()?;
    let strategy_value = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        content(digest(&transition))?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        None,
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::Strategy)?;
    let strategy = strategy_value.to_bytes();
    let effect = encode_effect(input.action, claims_accounts)?;
    let lifecycle_id = digest(&lifecycle_policy);
    let descriptor_value = CapabilityProgramV4::new(
        content(FRACTIONAL_CAPABILITY_KIND_ID_V1)?,
        content(FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2)?,
        content(FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_ID_V2)?,
        content(FRACTIONAL_ROOT_SCHEMA_ID_V1)?,
        content(lifecycle_id)?,
        content(input.capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: artifact(
                dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID,
                digest(&account_profile),
            )?,
            request_profile: artifact(
                dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                digest(&request_profile),
            )?,
            lifecycle: artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id)?,
            strategy: artifact(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, digest(&strategy))?,
            transition: artifact(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                digest(&transition),
            )?,
            effect: artifact(EFFECT_SCHEMA_ID_V4, digest(&effect))?,
        },
        u32::try_from(FRACTIONAL_ROOT_BYTES_V1)
            .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::Descriptor)?;
    let bundle = FractionalSelectedBundleV4 {
        action: input.action,
        account_profile,
        lifecycle_policy,
        request_profile,
        transition,
        strategy,
        effect,
        descriptor: descriptor_value.encode(),
    };
    validate_fractional_selected_bundle_v4(&bundle, input.capacity_profile, claims_accounts)?;
    Ok(bundle)
}

/// Hostile-decode and rejoin every exact V4 artifact body.
pub fn validate_fractional_selected_bundle_v4(
    bundle: &FractionalSelectedBundleV4,
    capacity_profile: [u8; 32],
    claims_accounts: u16,
) -> Result<(), FractionalSelectedArtifactErrorV4> {
    let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
        .map_err(|_| FractionalSelectedArtifactErrorV4::Descriptor)?;
    let account = AccountProfileV2::decode(&bundle.account_profile)
        .map_err(|_| FractionalSelectedArtifactErrorV4::AccountProfile)?;
    let lifecycle_id = digest(&bundle.lifecycle_policy);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        descriptor.lifecycle().program().to_bytes(),
        lifecycle_id,
        &bundle.lifecycle_policy,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::Lifecycle)?;
    lifecycle
        .validate_account_profile(account)
        .map_err(|_| FractionalSelectedArtifactErrorV4::Lifecycle)?;
    let request = RequestProfileV1::decode_selected(
        descriptor.request_profile().program().to_bytes(),
        digest(&bundle.request_profile),
        &bundle.request_profile,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::RequestProfile)?;
    let transition = TransitionProgramV3::decode(&bundle.transition)
        .map_err(|_| FractionalSelectedArtifactErrorV4::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
        .map_err(|_| FractionalSelectedArtifactErrorV4::Strategy)?;
    let effect = EffectProgramV4::decode(&bundle.effect)
        .map_err(|_| FractionalSelectedArtifactErrorV4::Effect)?;
    let base = effect.base();
    let expected_accounts = FRACTIONAL_HOT_INJECTED_ACCOUNT_COUNT_V4
        .checked_add(claims_accounts)
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    if descriptor.kind().to_bytes() != FRACTIONAL_CAPABILITY_KIND_ID_V1
        || descriptor.config_schema().to_bytes() != FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2
        || descriptor.request_schema().to_bytes() != FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_ID_V2
        || descriptor.root_schema().to_bytes() != FRACTIONAL_ROOT_SCHEMA_ID_V1
        || descriptor.capacity_profile().to_bytes() != capacity_profile
        || descriptor.root_state_bytes()
            != u32::try_from(FRACTIONAL_ROOT_BYTES_V1)
                .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?
        || descriptor.account_profile()
            != artifact(
                dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID,
                digest(&bundle.account_profile),
            )?
        || descriptor.request_profile()
            != artifact(
                dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                digest(&bundle.request_profile),
            )?
        || descriptor.lifecycle() != artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id)?
        || descriptor.strategy()
            != artifact(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                digest(&bundle.strategy),
            )?
        || descriptor.transition()
            != artifact(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                digest(&bundle.transition),
            )?
        || descriptor.effect() != artifact(EFFECT_SCHEMA_ID_V4, digest(&bundle.effect))?
        || strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
        || account.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || account.dynamic_fixed_span_count() != 0
        || account.fixed_account_count() != expected_accounts
        || request.fixed_request_bytes()
            != u32::try_from(FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2)
                .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?
        || request.item_request_bytes() != 0
        || effect.span_count() != 0
        || effect.range_count() != 0
        || effect.borrowed_range_policy() != BorrowedRangePolicyV4::DisjointExactCoverage
        || usize::try_from(effect.semantic_prefix_bytes()).ok()
            != Some(FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2)
        || base.fixed_account_count() != expected_accounts
        || !geometry_matches(account, request, transition, base)
    {
        return Err(FractionalSelectedArtifactErrorV4::Validation);
    }
    let route = base
        .route(0)
        .map_err(|_| FractionalSelectedArtifactErrorV4::Effect)?;
    if base.route_count() != 1
        || base.receipt_dependency_count() != 0
        || route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_start() != FRACTIONAL_HOT_INJECTED_ACCOUNT_COUNT_V4
        || route.fixed_account_count() != claims_accounts
        || route.item_account_count() != 0
        || route.fixed_request_bytes()
            != u32::try_from(FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2)
                .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?
    {
        return Err(FractionalSelectedArtifactErrorV4::Validation);
    }
    Ok(())
}

fn encode_account_profile(
    input: FractionalSelectedBundleInputV4<'_>,
    root_coordinate: u16,
) -> Result<Vec<u8>, FractionalSelectedArtifactErrorV4> {
    let none = AccountEffectPermissionsV2::new(false, false, false);
    let mut rules = Vec::with_capacity(
        usize::from(FRACTIONAL_HOT_INJECTED_ACCOUNT_COUNT_V4)
            .checked_add(input.claims_frame.len())
            .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
    );
    rules.push(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, true),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: u32::try_from(FRACTIONAL_CAPABILITY_ROOT_BYTES_V4)
                .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?,
            data_item_stride: 0,
        },
        prestate: AccountPrestateV2::Exact,
    });
    for (account, length, prestate) in [
        (
            SELECTED_CONFIG,
            input.profile.selected_config_bytes,
            AccountPrestateV2::AdapterAuthenticatedVariableData,
        ),
        (
            PRODUCT_RECORD,
            input.profile.product_record_bytes,
            AccountPrestateV2::Exact,
        ),
        (
            PORTFOLIO_RECORD,
            input.profile.portfolio_record_bytes,
            AccountPrestateV2::Exact,
        ),
        (
            LINKED_BASIS,
            input.profile.linked_basis_bytes,
            AccountPrestateV2::AdapterAuthenticatedVariableData,
        ),
    ] {
        if usize::from(account) != rules.len() {
            return Err(FractionalSelectedArtifactErrorV4::InvalidInput);
        }
        rules.push(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, false),
                effect_permissions: none,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: length,
                data_item_stride: 0,
            },
            prestate,
        });
    }
    for (index, observed) in input.claims_frame.iter().copied().enumerate() {
        if index == usize::from(root_coordinate) {
            rules.push(AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(false, false, false),
                    effect_permissions: none,
                    alias: AccountAliasInputV2::Fixed(ROOT),
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::AuthenticatedRouteAlias,
            });
        } else if observed.opaque_data {
            if observed.data_length != 0 {
                return Err(FractionalSelectedArtifactErrorV4::InvalidInput);
            }
            rules.push(AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(
                        observed.signer,
                        observed.writable,
                        observed.executable,
                    ),
                    effect_permissions: none,
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
            });
        } else {
            rules.push(AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: AccountPrivilegesV2::new(
                        observed.signer,
                        observed.writable,
                        observed.executable,
                    ),
                    effect_permissions: none,
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: observed.data_length,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::Exact,
            });
        }
    }
    let state = u32::try_from(FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4)
        .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?;
    let operations = [
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(ROOT),
            destination: IdentityCoordinateV2::common(I_ROOT_TERMS),
            data_offset: state
                .checked_add(narrow_u32(FRACTIONAL_ROOT_TERMS_OFFSET_V1)?)
                .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
        },
        AccountOperationInputV2::ProjectDataIdentity {
            account: AccountCoordinateV2::fixed(ROOT),
            destination: IdentityCoordinateV2::common(I_ROOT_MARKET),
            data_offset: state
                .checked_add(narrow_u32(FRACTIONAL_ROOT_MARKET_OFFSET_V1)?)
                .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
        },
        AccountOperationInputV2::ProjectDataU64 {
            account: AccountCoordinateV2::fixed(ROOT),
            destination: ScalarCoordinateV2::common(S_ROOT_REVISION),
            data_offset: state
                .checked_add(narrow_u32(FRACTIONAL_ROOT_REVISION_OFFSET_V1)?)
                .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(SELECTED_CONFIG),
            destination: IdentityCoordinateV2::common(I_SELECTED_CONFIG),
        },
        AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(PRODUCT_RECORD),
            destination: IdentityCoordinateV2::common(I_AUTHENTICATED_PRODUCT),
        },
        AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(ROOT),
            expected: IdentityCoordinateV2::common(I_TRADING_PROGRAM),
        },
    ];
    let bytes = DYNAMIC_FIXED_SPAN_HEADER_BYTES
        .checked_add(
            rules
                .len()
                .checked_mul(ACCOUNT_RULE_BYTES)
                .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
        )
        .and_then(|value| {
            operations
                .len()
                .checked_mul(ACCOUNT_OPERATION_BYTES)
                .and_then(|ops| value.checked_add(ops))
        })
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: I_TRADING_PROGRAM,
        },
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &operations,
        register_geometry(),
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::AccountProfile)?;
    Ok(output)
}

fn encode_empty_lifecycle() -> Result<Vec<u8>, FractionalSelectedArtifactErrorV4> {
    let mut scratch = vec![0_u8; LIFECYCLE_HEADER_BYTES];
    let mut output = vec![0_u8; LIFECYCLE_HEADER_BYTES];
    encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
        .map_err(|_| FractionalSelectedArtifactErrorV4::Lifecycle)?;
    Ok(output)
}

fn encode_request_profile(
    action: FractionalExposureActionV2,
) -> Result<Vec<u8>, FractionalSelectedArtifactErrorV4> {
    let request = |offset| {
        u32::try_from(offset)
            .map(RequestCoordinateV1::fixed)
            .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)
    };
    let scalar = ScalarRegisterV1::common;
    let identity = IdentityRegisterV1::common;
    let instructions = [
        RequestInstructionV1::require_u64(
            request(0)?,
            u64::from_le_bytes(FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2),
        ),
        RequestInstructionV1::require_u16(request(8)?, 2),
        RequestInstructionV1::require_u8(
            request(FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2)?,
            action.byte(),
        ),
        RequestInstructionV1::project_u8(
            request(FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2)?,
            scalar(S_ACTION),
        ),
        RequestInstructionV1::require_zero(
            request(FRACTIONAL_EXPOSURE_REQUEST_HEADER_RESERVED_OFFSET_V2)?,
            5,
        ),
        RequestInstructionV1::require_zero(
            request(FRACTIONAL_EXPOSURE_REQUEST_TAIL_RESERVED_OFFSET_V2)?,
            28,
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_RELEASE_SET_OFFSET_V2)?,
            identity(I_RELEASE_SET),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_MARKET_OFFSET_V2)?,
            identity(I_MARKET),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_PRODUCT_RECORD_OFFSET_V2)?,
            identity(I_PRODUCT_RECORD),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_RESULT_DOMAIN_OFFSET_V2)?,
            identity(I_RESULT_DOMAIN),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_TERMS_OFFSET_V2)?,
            identity(I_TERMS),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2)?,
            identity(I_TOKEN_BEHAVIOR),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_EXPOSURE_OFFSET_V2)?,
            identity(I_EXPOSURE),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_OWNER_OFFSET_V2)?,
            identity(I_OWNER),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_SOURCE_TOKEN_OFFSET_V2)?,
            identity(I_SOURCE_TOKEN),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_DESTINATION_TOKEN_OFFSET_V2)?,
            identity(I_DESTINATION_TOKEN),
        ),
        RequestInstructionV1::project_identity(
            request(FRACTIONAL_EXPOSURE_REQUEST_TERMINAL_DIGEST_OFFSET_V2)?,
            identity(I_TERMINAL_DIGEST),
        ),
        RequestInstructionV1::project_u64(
            request(FRACTIONAL_EXPOSURE_REQUEST_EXPECTED_REVISION_OFFSET_V2)?,
            scalar(S_EXPECTED_REVISION),
        ),
        RequestInstructionV1::project_u64(
            request(FRACTIONAL_EXPOSURE_REQUEST_QUANTITY_OFFSET_V2)?,
            scalar(S_QUANTITY),
        ),
        RequestInstructionV1::project_u32(
            request(FRACTIONAL_EXPOSURE_REQUEST_REPRESENTATION_COORDINATE_OFFSET_V2)?,
            scalar(S_REPRESENTATION_COORDINATE),
        ),
    ];
    let bytes = REQUEST_HEADER_BYTES
        .checked_add(
            instructions
                .len()
                .checked_mul(REQUEST_OPERATION_BYTES)
                .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
        )
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            u32::try_from(FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2)
                .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?,
            0,
            FRACTIONAL_COMMON_SCALARS_V4,
            0,
            FRACTIONAL_COMMON_IDENTITIES_V4,
            0,
        ),
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::RequestProfile)?;
    Ok(output)
}

fn encode_transition() -> Result<Vec<u8>, FractionalSelectedArtifactErrorV4> {
    let scalar = ScalarRegisterV3::common;
    let identity = IdentityRegisterV3::common;
    let instructions = [
        InstructionV3::scalar_eq(scalar(S_EXPECTED_REVISION), scalar(S_ROOT_REVISION)),
        InstructionV3::increment_into(scalar(S_ROOT_REVISION), scalar(S_POST_REVISION)),
        InstructionV3::identity_eq(identity(I_TERMS), identity(I_ROOT_TERMS)),
        InstructionV3::identity_eq(identity(I_MARKET), identity(I_ROOT_MARKET)),
        InstructionV3::identity_eq(identity(I_TERMS), identity(I_SELECTED_CONFIG)),
        InstructionV3::identity_eq(
            identity(I_PRODUCT_RECORD),
            identity(I_AUTHENTICATED_PRODUCT),
        ),
        InstructionV3::nonzero(scalar(S_QUANTITY)),
    ];
    let bytes = TRANSITION_HEADER_BYTES
        .checked_add(
            instructions
                .len()
                .checked_mul(TRANSITION_INSTRUCTION_BYTES)
                .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
        )
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: FRACTIONAL_COMMON_SCALARS_V4,
            item_scalar_stride: 0,
            common_identities: FRACTIONAL_COMMON_IDENTITIES_V4,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::Transition)?;
    Ok(output)
}

fn encode_effect(
    action: FractionalExposureActionV2,
    claims_accounts: u16,
) -> Result<Vec<u8>, FractionalSelectedArtifactErrorV4> {
    let fixed_accounts = FRACTIONAL_HOT_INJECTED_ACCOUNT_COUNT_V4
        .checked_add(claims_accounts)
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    let mut template = [0_u8; FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2];
    put(&mut template, 0, &FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2)?;
    put(&mut template, 8, &2_u16.to_le_bytes())?;
    *template
        .get_mut(FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2)
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)? = action.byte();
    let routes = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: FRACTIONAL_HOT_INJECTED_ACCOUNT_COUNT_V4,
        fixed_account_count: claims_accounts,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &template,
        item_request: &[],
    }];
    let identity = IdentityCoordinateV3::common;
    let scalar = ScalarCoordinateV3::common;
    let mut operations = Vec::with_capacity(15);
    for (offset, source) in [
        (
            FRACTIONAL_EXPOSURE_REQUEST_RELEASE_SET_OFFSET_V2,
            I_RELEASE_SET,
        ),
        (FRACTIONAL_EXPOSURE_REQUEST_MARKET_OFFSET_V2, I_MARKET),
        (
            FRACTIONAL_EXPOSURE_REQUEST_PRODUCT_RECORD_OFFSET_V2,
            I_PRODUCT_RECORD,
        ),
        (
            FRACTIONAL_EXPOSURE_REQUEST_RESULT_DOMAIN_OFFSET_V2,
            I_RESULT_DOMAIN,
        ),
        (FRACTIONAL_EXPOSURE_REQUEST_TERMS_OFFSET_V2, I_TERMS),
        (
            FRACTIONAL_EXPOSURE_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2,
            I_TOKEN_BEHAVIOR,
        ),
        (FRACTIONAL_EXPOSURE_REQUEST_EXPOSURE_OFFSET_V2, I_EXPOSURE),
        (FRACTIONAL_EXPOSURE_REQUEST_OWNER_OFFSET_V2, I_OWNER),
        (
            FRACTIONAL_EXPOSURE_REQUEST_SOURCE_TOKEN_OFFSET_V2,
            I_SOURCE_TOKEN,
        ),
        (
            FRACTIONAL_EXPOSURE_REQUEST_DESTINATION_TOKEN_OFFSET_V2,
            I_DESTINATION_TOKEN,
        ),
        (
            FRACTIONAL_EXPOSURE_REQUEST_TERMINAL_DIGEST_OFFSET_V2,
            I_TERMINAL_DIGEST,
        ),
    ] {
        operations.push(EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            identity(source),
        ));
    }
    for (offset, source) in [
        (
            FRACTIONAL_EXPOSURE_REQUEST_EXPECTED_REVISION_OFFSET_V2,
            S_EXPECTED_REVISION,
        ),
        (FRACTIONAL_EXPOSURE_REQUEST_QUANTITY_OFFSET_V2, S_QUANTITY),
    ] {
        operations.push(EffectInstructionV3::write_request_u64(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            scalar(source),
        ));
    }
    operations.push(EffectInstructionV3::write_request_u32(
        0,
        RequestSpaceV3::Fixed,
        narrow_u32(FRACTIONAL_EXPOSURE_REQUEST_REPRESENTATION_COORDINATE_OFFSET_V2)?,
        scalar(S_REPRESENTATION_COORDINATE),
    ));
    operations.push(EffectInstructionV3::write_u64(
        AccountCoordinateV3::fixed(ROOT),
        u32::try_from(FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4)
            .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?
            .checked_add(narrow_u32(FRACTIONAL_ROOT_REVISION_OFFSET_V1)?)
            .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?,
        scalar(S_POST_REVISION),
    ));
    let base_bytes = EFFECT_HEADER_BYTES
        .checked_add(EFFECT_ROUTE_BYTES)
        .and_then(|value| {
            operations
                .len()
                .checked_mul(EFFECT_OPERATION_BYTES)
                .and_then(|ops| value.checked_add(ops))
        })
        .and_then(|value| value.checked_add(template.len()))
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    let mut base_scratch = vec![0_u8; base_bytes];
    let mut base = vec![0_u8; base_bytes];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts,
            item_account_stride: 0,
            common_scalars: FRACTIONAL_COMMON_SCALARS_V4,
            item_scalar_stride: 0,
            common_identities: FRACTIONAL_COMMON_IDENTITIES_V4,
            item_identity_stride: 0,
        },
        &routes,
        &operations,
        &[],
        &mut base_scratch,
        &mut base,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::Effect)?;
    let bytes = EFFECT_V4_HEADER_BYTES
        .checked_add(base.len())
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    let mut scratch = vec![0_u8; bytes];
    let mut output = vec![0_u8; bytes];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        u32::try_from(FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2)
            .map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)?,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(|_| FractionalSelectedArtifactErrorV4::Effect)?;
    Ok(output)
}

fn action_geometry(
    action: FractionalExposureActionV2,
) -> Result<(u16, u16), FractionalSelectedArtifactErrorV4> {
    match action {
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap => Ok((
            narrow_u16(FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3)?,
            narrow_u16(FRACTIONAL_ATOMIC_ROOT_V3)?,
        )),
        FractionalExposureActionV2::TerminalRedeem
        | FractionalExposureActionV2::TerminalZeroBurn => Ok((
            narrow_u16(FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3)?,
            narrow_u16(FRACTIONAL_TERMINAL_ROOT_V3)?,
        )),
        _ => Err(FractionalSelectedArtifactErrorV4::InvalidInput),
    }
}

fn geometry_matches(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: dclutch_effect_kernel::v3::ProgramV3<'_>,
) -> bool {
    [
        account.common_scalar_count(),
        request.common_scalar_count(),
        transition.common_scalar_count(),
        effect.common_scalar_count(),
    ]
    .iter()
    .all(|count| *count == FRACTIONAL_COMMON_SCALARS_V4)
        && [
            account.common_identity_count(),
            request.common_identity_count(),
            transition.common_identity_count(),
            effect.common_identity_count(),
        ]
        .iter()
        .all(|count| *count == FRACTIONAL_COMMON_IDENTITIES_V4)
        && account.item_scalar_stride() == 0
        && request.item_scalar_stride() == 0
        && transition.item_scalar_stride() == 0
        && effect.item_scalar_stride() == 0
        && account.item_identity_stride() == 0
        && request.item_identity_stride() == 0
        && transition.item_identity_stride() == 0
        && effect.item_identity_stride() == 0
}

fn register_geometry() -> RegisterGeometryV2 {
    RegisterGeometryV2 {
        common_scalars: FRACTIONAL_COMMON_SCALARS_V4,
        item_scalar_stride: 0,
        common_identities: FRACTIONAL_COMMON_IDENTITIES_V4,
        item_identity_stride: 0,
    }
}

fn artifact(
    schema: [u8; 32],
    program: [u8; 32],
) -> Result<ArtifactReferenceV4, FractionalSelectedArtifactErrorV4> {
    Ok(ArtifactReferenceV4::new(
        content(schema)?,
        content(program)?,
    ))
}

fn content(bytes: [u8; 32]) -> Result<ContentId, FractionalSelectedArtifactErrorV4> {
    ContentId::new(bytes).map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn narrow_u16(value: usize) -> Result<u16, FractionalSelectedArtifactErrorV4> {
    u16::try_from(value).map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)
}

fn narrow_u32(value: usize) -> Result<u32, FractionalSelectedArtifactErrorV4> {
    u32::try_from(value).map_err(|_| FractionalSelectedArtifactErrorV4::InvalidInput)
}

fn put(
    output: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), FractionalSelectedArtifactErrorV4> {
    let end = offset
        .checked_add(value.len())
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?;
    output
        .get_mut(offset..end)
        .ok_or(FractionalSelectedArtifactErrorV4::InvalidInput)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(action: FractionalExposureActionV2) -> Vec<FractionalClaimsAccountRuleV1> {
        let (count, root) = action_geometry(action).expect("supported action");
        let mut rules = vec![
            FractionalClaimsAccountRuleV1 {
                signer: false,
                writable: false,
                executable: false,
                data_length: 64,
                opaque_data: false,
            };
            usize::from(count)
        ];
        let root = rules.get_mut(usize::from(root)).expect("root rule");
        root.signer = true;
        root.writable = true;
        root.data_length = u32::try_from(FRACTIONAL_CAPABILITY_ROOT_BYTES_V4).expect("root width");
        rules
    }

    fn input(
        action: FractionalExposureActionV2,
        frame: &[FractionalClaimsAccountRuleV1],
    ) -> FractionalSelectedBundleInputV4<'_> {
        FractionalSelectedBundleInputV4 {
            action,
            capacity_profile: [9; 32],
            profile: FractionalSelectedProfileInputV4 {
                selected_config_bytes: 64,
                product_record_bytes: 128,
                portfolio_record_bytes: 96,
                linked_basis_bytes: 80,
            },
            claims_frame: frame,
        }
    }

    #[test]
    fn exact_four_executable_actions_emit_v4_and_stay_below_64_locks() {
        for (action, claims, logical) in [
            (FractionalExposureActionV2::Wrap, 31_u16, 36_u16),
            (FractionalExposureActionV2::WholeUnwrap, 31, 36),
            (FractionalExposureActionV2::TerminalRedeem, 44, 49),
            (FractionalExposureActionV2::TerminalZeroBurn, 44, 49),
        ] {
            let rules = frame(action);
            let bundle = build_fractional_selected_bundle_v4(input(action, &rules))
                .expect("selected bundle");
            validate_fractional_selected_bundle_v4(&bundle, [9; 32], claims)
                .expect("hostile rejoin");
            let account = AccountProfileV2::decode(&bundle.account_profile).expect("profile");
            let effect = EffectProgramV4::decode(&bundle.effect).expect("effect");
            assert_eq!(account.fixed_account_count(), logical);
            assert_eq!(effect.base().fixed_account_count(), logical);
            assert!(logical < 64);
        }
    }

    #[test]
    fn unsupported_action_frame_and_root_privilege_substitution_refuse() {
        let empty = frame(FractionalExposureActionV2::Wrap);
        let mut unsupported = input(FractionalExposureActionV2::Transfer, &empty);
        assert_eq!(
            build_fractional_selected_bundle_v4(unsupported),
            Err(FractionalSelectedArtifactErrorV4::InvalidInput)
        );

        let mut wrong_width = frame(FractionalExposureActionV2::Wrap);
        wrong_width.pop();
        unsupported = input(FractionalExposureActionV2::Wrap, &wrong_width);
        assert_eq!(
            build_fractional_selected_bundle_v4(unsupported),
            Err(FractionalSelectedArtifactErrorV4::InvalidInput)
        );

        let mut substituted = frame(FractionalExposureActionV2::Wrap);
        substituted[FRACTIONAL_ATOMIC_ROOT_V3].signer = false;
        unsupported = input(FractionalExposureActionV2::Wrap, &substituted);
        assert_eq!(
            build_fractional_selected_bundle_v4(unsupported),
            Err(FractionalSelectedArtifactErrorV4::InvalidInput)
        );
    }

    #[test]
    fn every_artifact_digest_and_action_byte_is_authenticated() {
        let rules = frame(FractionalExposureActionV2::TerminalRedeem);
        let bundle = build_fractional_selected_bundle_v4(input(
            FractionalExposureActionV2::TerminalRedeem,
            &rules,
        ))
        .expect("bundle");
        for mutate in 0..6 {
            let mut hostile = bundle.clone();
            let bytes = match mutate {
                0 => &mut hostile.account_profile,
                1 => &mut hostile.lifecycle_policy,
                2 => &mut hostile.request_profile,
                3 => &mut hostile.transition,
                4 => &mut hostile.effect,
                _ => {
                    hostile.strategy[0] ^= 1;
                    assert!(validate_fractional_selected_bundle_v4(&hostile, [9; 32], 44).is_err());
                    continue;
                }
            };
            bytes[0] ^= 1;
            assert!(validate_fractional_selected_bundle_v4(&hostile, [9; 32], 44).is_err());
        }
    }
}
