//! Data-defined Hot artifacts for full-width Structured issue and unwrap.

use dclutch_account_profile_contract::lifecycle_v3::{
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as LIFECYCLE_SCHEMA_ID_V5, StateLifecyclePolicyV5,
};
use dclutch_account_profile_contract::v2::{
    AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
    DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES,
    RULE_BYTES as ACCOUNT_RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_with_dynamic_fixed_span_v2_atomic,
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
            EffectGeometryV3, EffectInstructionV3, IdentityCoordinateV3, RequestSpaceV3,
            RouteInputV3, ScalarCoordinateV3, encode_effect_program_v3_atomic,
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
use dclutch_product_payoff_v2_codec::runtime_v3::{
    BASIS_HEADER_BYTES_V3, BASIS_WIDTH_OFFSET_V3, ProductBasisV3,
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AuthenticatedTokenBehaviorV2, CallerRoleV2,
    OPEN_REPRESENTATION_HOT_MAGIC_V3, OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3,
    OPEN_REPRESENTATION_HOT_VERSION_V3, PHYSICAL_ABI_VERSION_V2, REQUEST_HEADER_BYTES_V2,
    REQUEST_MAGIC_V2, RepresentationActionV2,
};
use dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2;
use dclutch_rational_representation_v2_request_contract::generated as wire;
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_PROFILE_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_token_svm::{
    TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    InstructionV3, ProgramGeometryV3, ProgramV3 as TransitionProgramV3, ScalarRegisterV3,
    encode_program_atomic,
};
use solana_program::hash::hash;

use crate::{Error, Result};

const INJECTED_ACCOUNTS: u16 = 5;
const CLAIMS_FIXED_ACCOUNTS: u16 = 32;
/// Fixed logical accounts: Hot evidence plus the Claims fixed prefix.
pub const RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3: u16 =
    INJECTED_ACCOUNTS + CLAIMS_FIXED_ACCOUNTS;
/// Exact Claims account stride for each Product outcome.
pub const RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3: u16 = 4;
/// Common request registers plus trusted current Trading before descriptor-K rows.
pub const RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3: usize = 11;
/// Per-coordinate descriptor-K identity width, flattened into common registers.
pub const RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3: usize = 4;
/// Common request scalars, Product N, and trusted current slot before descriptor-K rows.
pub const RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3: usize = 9;
/// Per-coordinate descriptor-K scalar width, flattened into common registers.
pub const RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3: usize = 4;
/// Largest descriptor K admitted by the current exact RequestProfile V1 artifact.
///
/// The Profile13 account ceiling would permit more rows, but the canonical
/// fixed projection has 29 prefix operations plus eight operations per row.
/// V1's 1,312-byte bound therefore makes `K = 3` the largest executable
/// geometry. This bound is independent of Product result width `N`.
pub const RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3: u32 = 3;

const ID_PARENT: usize = 0;
const ID_RELEASE: usize = 1;
const ID_MARKET: usize = 2;
const ID_GRAPH: usize = 3;
const ID_DESCRIPTOR: usize = 4;
const ID_ACTOR: usize = 5;
const ID_RECEIPT_MINT: usize = 6;
const ID_RECEIPT_ACCOUNT: usize = 7;
const ID_AUTHORITY: usize = 8;
const ID_TOKEN: usize = 9;
const ID_CURRENT_TRADING: usize = 10;

const ITEM_ID_SHARD_MINT: usize = 0;
const ITEM_ID_ACTOR_SHARDS: usize = 1;
const ITEM_ID_STRUCTURED_CUSTODY: usize = 2;
const ITEM_ID_CLAIMS_CUSTODY_OWNER: usize = 3;

const SCALAR_REPRESENTATION_REVISION: usize = 0;
const SCALAR_GENERATION: usize = 1;
const SCALAR_QUANTITY: usize = 2;
const SCALAR_DENOMINATOR: usize = 3;
const SCALAR_RECEIPT_SUPPLY: usize = 4;
const SCALAR_OUTCOME_COUNT: usize = 5;
const SCALAR_ASSET_COUNT: usize = 6;
const SCALAR_PRODUCT_OUTCOME_COUNT: usize = 7;
const SCALAR_CURRENT_SLOT: usize = 8;

const ITEM_SCALAR_COEFFICIENT: usize = 0;
const ITEM_SCALAR_SHARD_SUPPLY: usize = 1;
const ITEM_SCALAR_ACTOR_SHARDS: usize = 2;
const ITEM_SCALAR_STRUCTURED_SHARDS: usize = 3;

const REQUEST_BASE_INSTRUCTIONS: usize = 29;
const REQUEST_ROW_INSTRUCTIONS: usize = 8;
const TRANSITION_BASE_INSTRUCTIONS: usize = 4;
const TRANSITION_ROW_INSTRUCTIONS: usize = 1;
const EFFECT_BASE_INSTRUCTIONS: usize = 17;
const EFFECT_ROW_INSTRUCTIONS: usize = 8;

/// Release-owned coordinates and exact fixed/item account data widths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalOpenStructuredHotBundleInputV3<'a> {
    /// IssueStructured or UnwrapStructured.
    pub action: RepresentationActionV2,
    /// Exact logical-37 fixed account data lengths.
    pub fixed_data_lengths: &'a [u32],
    /// Exact four account data lengths repeated for every Product outcome.
    pub item_data_lengths: [u32; 4],
    /// Exact Registry-authenticated ProductBasis body; its N is independent of K.
    pub product_basis: &'a [u8],
    /// Exact finalized Rational descriptor; its K alone sizes representation rows.
    pub representation_descriptor: RepresentationDescriptorV2<'a>,
    /// Manifest-selected capability kind.
    pub kind: [u8; 32],
    /// Finalized descriptor/Market/config Token behavior admission.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
    /// Manifest-selected root-tail schema.
    pub root_schema: [u8; 32],
    /// Exact finalized successor lifecycle policy bytes.
    pub lifecycle_policy: &'a [u8],
    /// Manifest-selected capacity profile.
    pub capacity_profile: [u8; 32],
    /// Exact root-tail data width.
    pub root_state_bytes: u32,
}

/// Exact finalized artifact bodies for one structured action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalOpenStructuredHotBundleV3 {
    /// Exact descriptor-owned representation coordinate width K.
    pub representation_outcome_count: u32,
    /// Exact config record body selected alongside the descriptor.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// Descriptor-K-specialized Profile13 interpreter with opaque Loader/Token data.
    pub account_profile: Vec<u8>,
    /// Variable-width open-family RequestProfile.
    pub request_profile: Vec<u8>,
    /// Exact successor lifecycle policy.
    pub lifecycle_policy: Vec<u8>,
    /// Full-width coefficient transition.
    pub transition: Vec<u8>,
    /// Interpreted ExecutionStrategy.
    pub strategy: [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    /// One descriptor-K-specialized Once Claims effect.
    pub effect: Vec<u8>,
    /// Capability descriptor selecting every artifact.
    pub descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
}

/// Build one complete immutable structured-action artifact bundle.
pub fn build_rational_open_structured_hot_bundle_v3(
    input: RationalOpenStructuredHotBundleInputV3<'_>,
) -> Result<RationalOpenStructuredHotBundleV3> {
    if !matches!(
        input.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    ) {
        return Err(Error::ArtifactGeometry);
    }
    let representation_outcome_count = require_representation_width(input)?;
    let account_profile = encode_account_profile(input, representation_outcome_count)?;
    let lifecycle_policy = Vec::from(input.lifecycle_policy);
    let request_profile = encode_request_profile(input.action, representation_outcome_count)?;
    let transition = encode_transition(representation_outcome_count)?;
    let effect = encode_effect(input.action, representation_outcome_count)?;
    let strategy_value = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        content(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID)?,
        digest(&transition)?,
        content(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2)?,
        None,
        content(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2)?,
        None,
        content(ACCELERATOR_REQUEST_SCHEMA_ID_V2)?,
        content(ACCELERATOR_ACK_SCHEMA_ID_V2)?,
    )
    .map_err(Error::ExecutionStrategy)?;
    let strategy = strategy_value.to_bytes();
    let token_behavior_selection = input.authenticated_token_behavior.selection().to_bytes();
    if hash(&token_behavior_selection).to_bytes()
        != input.authenticated_token_behavior.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    let lifecycle_id = digest(&lifecycle_policy)?;
    let descriptor = CapabilityProgramV4::new(
        content(input.kind)?,
        content(TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2)?,
        content(OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3)?,
        content(input.root_schema)?,
        lifecycle_id,
        content(input.capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: artifact(
                dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID,
                digest(&account_profile)?.to_bytes(),
            )?,
            request_profile: artifact(
                dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                digest(&request_profile)?.to_bytes(),
            )?,
            lifecycle: artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id.to_bytes())?,
            strategy: artifact(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                digest(&strategy)?.to_bytes(),
            )?,
            transition: artifact(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                digest(&transition)?.to_bytes(),
            )?,
            effect: artifact(EFFECT_SCHEMA_ID_V4, digest(&effect)?.to_bytes())?,
        },
        input.root_state_bytes,
    )
    .map_err(Error::CapabilityDescriptor)?
    .encode();
    let bundle = RationalOpenStructuredHotBundleV3 {
        representation_outcome_count,
        token_behavior_selection,
        account_profile,
        request_profile,
        lifecycle_policy,
        transition,
        strategy,
        effect,
        descriptor,
    };
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
        &bundle,
        input.authenticated_token_behavior,
    )?;
    Ok(bundle)
}

/// Independently hostile-decode and join every structured bundle artifact.
pub fn validate_rational_open_structured_hot_bundle_v3(
    bundle: &RationalOpenStructuredHotBundleV3,
) -> Result<()> {
    let representation_outcome_count = usize::try_from(bundle.representation_outcome_count)
        .map_err(|_| Error::ArtifactGeometry)?;
    if bundle.representation_outcome_count == 0
        || bundle.representation_outcome_count > RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3
    {
        return Err(Error::ArtifactGeometry);
    }
    let logical_accounts = structured_logical_accounts(representation_outcome_count)?;
    let common_scalars = structured_common_scalars(representation_outcome_count)?;
    let common_identities = structured_common_identities(representation_outcome_count)?;
    let request_bytes = structured_request_bytes(representation_outcome_count)?;
    let descriptor =
        CapabilityProgramV4::decode(&bundle.descriptor).map_err(Error::CapabilityDescriptor)?;
    TokenBehaviorSelectionV2::decode(&bundle.token_behavior_selection)
        .map_err(Error::TokenBehavior)?;
    let account =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfileArtifact)?;
    let lifecycle_id = digest(&bundle.lifecycle_policy)?;
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        descriptor.lifecycle().program().to_bytes(),
        lifecycle_id.to_bytes(),
        &bundle.lifecycle_policy,
    )
    .map_err(Error::LifecycleArtifact)?;
    lifecycle
        .validate_account_profile(account)
        .map_err(Error::LifecycleArtifact)?;
    let request = RequestProfileV1::decode_selected(
        descriptor.request_profile().program().to_bytes(),
        hash(&bundle.request_profile).to_bytes(),
        &bundle.request_profile,
    )
    .map_err(Error::RequestProfileArtifact)?;
    let transition =
        TransitionProgramV3::decode(&bundle.transition).map_err(Error::TransitionArtifact)?;
    let strategy =
        ExecutionStrategyProgramV2::decode(&bundle.strategy).map_err(Error::ExecutionStrategy)?;
    let effect = EffectProgramV4::decode(&bundle.effect).map_err(Error::EffectArtifactV4)?;
    let effect_base = effect.base();
    let route = effect_base.route(0).map_err(Error::EffectArtifact)?;
    let (fixed_template, item_template) = effect_base
        .route_template(0)
        .map_err(Error::EffectArtifact)?;
    if descriptor.request_schema().to_bytes() != OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3
        || descriptor.config_schema().to_bytes() != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
        || descriptor.derivation_policy() != lifecycle_id
        || descriptor.account_profile()
            != artifact(
                dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID,
                digest(&bundle.account_profile)?.to_bytes(),
            )?
        || descriptor.request_profile()
            != artifact(
                dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                digest(&bundle.request_profile)?.to_bytes(),
            )?
        || descriptor.lifecycle() != artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id.to_bytes())?
        || descriptor.strategy()
            != artifact(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                digest(&bundle.strategy)?.to_bytes(),
            )?
        || descriptor.transition()
            != artifact(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                digest(&bundle.transition)?.to_bytes(),
            )?
        || descriptor.effect() != artifact(EFFECT_SCHEMA_ID_V4, digest(&bundle.effect)?.to_bytes())?
        || strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
        || account.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || account.dynamic_fixed_span_count() != 0
        || usize::from(account.fixed_account_count()) != logical_accounts
        || account.item_account_stride() != 0
        || usize::from(account.common_scalar_count()) != common_scalars
        || account.item_scalar_stride() != 0
        || usize::from(account.common_identity_count()) != common_identities
        || account.item_identity_stride() != 0
        || account.trusted_current_slot_scalar() != Some(narrow_u16(SCALAR_CURRENT_SLOT)?)
        || account.trusted_current_executing_program_identity()
            != Some(narrow_u16(ID_CURRENT_TRADING)?)
        || usize::try_from(request.fixed_request_bytes()).ok() != Some(request_bytes)
        || request.item_request_bytes() != 0
        || usize::from(request.common_scalar_count()) != common_scalars
        || request.item_scalar_stride() != 0
        || usize::from(request.common_identity_count()) != common_identities
        || request.item_identity_stride() != 0
        || usize::from(transition.common_scalar_count()) != common_scalars
        || transition.item_scalar_stride() != 0
        || usize::from(transition.common_identity_count()) != common_identities
        || transition.item_identity_stride() != 0
        || effect.span_count() != 0
        || effect.range_count() != 0
        || usize::try_from(effect.semantic_prefix_bytes()).ok() != Some(request_bytes)
        || usize::from(effect_base.fixed_account_count()) != logical_accounts
        || effect_base.item_account_stride() != 0
        || usize::from(effect_base.common_scalar_count()) != common_scalars
        || effect_base.item_scalar_stride() != 0
        || usize::from(effect_base.common_identity_count()) != common_identities
        || effect_base.item_identity_stride() != 0
        || effect_base.route_count() != 1
        || route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_start() != INJECTED_ACCOUNTS
        || usize::from(route.fixed_account_count())
            != CLAIMS_FIXED_ACCOUNTS as usize
                + representation_outcome_count * RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3 as usize
        || route.item_account_start() != 0
        || route.item_account_count() != 0
        || fixed_template.len() != request_bytes
        || !item_template.is_empty()
    {
        return Err(Error::ArtifactGeometry);
    }
    Ok(())
}

/// Validate the complete bundle and bind its selected Token behavior to
/// independently authenticated Realm and release-set identities.
pub fn validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
    bundle: &RationalOpenStructuredHotBundleV3,
    authenticated: AuthenticatedTokenBehaviorV2,
) -> Result<()> {
    validate_rational_open_structured_hot_bundle_v3(bundle)?;
    if bundle.token_behavior_selection != authenticated.selection().to_bytes()
        || hash(&bundle.token_behavior_selection).to_bytes() != authenticated.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    Ok(())
}

fn encode_account_profile(
    input: RationalOpenStructuredHotBundleInputV3<'_>,
    representation_outcome_count: u32,
) -> Result<Vec<u8>> {
    let representation_outcome_count =
        usize::try_from(representation_outcome_count).map_err(|_| Error::AccountProfileInput)?;
    let mut rules = Vec::with_capacity(structured_logical_accounts(representation_outcome_count)?);
    for index in 0..input.fixed_data_lengths.len() {
        let writable = matches!(index, 0 | 16 | 25 | 26);
        let signer = index == 8;
        let executable = matches!(index, 6 | 15 | 19 | 21 | 23 | 27 | 28);
        let alias = match index {
            28 => AccountAliasInputV2::Fixed(19),
            29 => AccountAliasInputV2::Fixed(4),
            31 => AccountAliasInputV2::Fixed(2),
            35 => AccountAliasInputV2::Fixed(3),
            _ => AccountAliasInputV2::SelfCoordinate,
        };
        let opaque = matches!(index, 6 | 7 | 19 | 20 | 21 | 23 | 24 | 25 | 26 | 27);
        let prestate = if index == 4 {
            AccountPrestateV2::AdapterAuthenticatedVariableData
        } else if alias != AccountAliasInputV2::SelfCoordinate {
            AccountPrestateV2::AuthenticatedRouteAlias
        } else if opaque {
            AccountPrestateV2::AuthenticatedOpaqueReadonlyData
        } else {
            AccountPrestateV2::Exact
        };
        let data_length = match index {
            1 => narrow_u32(TOKEN_BEHAVIOR_SELECTION_BYTES_V2)?,
            4 => narrow_u32(BASIS_HEADER_BYTES_V3)?,
            28 | 29 | 31 | 35 => 0,
            _ if opaque => 0,
            _ => *input
                .fixed_data_lengths
                .get(index)
                .ok_or(Error::AccountProfileInput)?,
        };
        rules.push(rule(
            signer,
            writable,
            executable,
            alias,
            prestate,
            data_length,
        ));
    }
    for _ in 0..representation_outcome_count {
        for (index, length) in input.item_data_lengths.iter().copied().enumerate() {
            let opaque = matches!(index, 1..=3);
            rules.push(rule(
                false,
                matches!(index, 2 | 3),
                false,
                AccountAliasInputV2::SelfCoordinate,
                if opaque {
                    AccountPrestateV2::AuthenticatedOpaqueReadonlyData
                } else {
                    AccountPrestateV2::Exact
                },
                if opaque { 0 } else { length },
            ));
        }
    }
    let operations = [AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(4),
        destination: ScalarCoordinateV2::common(narrow_u16(SCALAR_PRODUCT_OUTCOME_COUNT)?),
        data_offset: narrow_u32(BASIS_WIDTH_OFFSET_V3)?,
    }];
    let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
        + rules.len() * ACCOUNT_RULE_BYTES
        + ACCOUNT_OPERATION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: narrow_u16(SCALAR_CURRENT_SLOT)?,
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: narrow_u16(ID_CURRENT_TRADING)?,
        },
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &operations,
        register_geometry(representation_outcome_count)?,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::AccountProfileArtifact)?;
    Ok(output)
}

fn rule(
    signer: bool,
    writable: bool,
    executable: bool,
    alias: AccountAliasInputV2,
    prestate: AccountPrestateV2,
    data_length: u32,
) -> AccountRuleWithPrestateInputV2 {
    AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(signer, writable, executable),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias,
            data_length,
            data_item_stride: 0,
        },
        prestate,
    }
}

fn encode_request_profile(
    action: RepresentationActionV2,
    representation_outcome_count: u32,
) -> Result<Vec<u8>> {
    let representation_outcome_count =
        usize::try_from(representation_outcome_count).map_err(|_| Error::ArtifactGeometry)?;
    let mut fixed = Vec::with_capacity(
        REQUEST_BASE_INSTRUCTIONS
            .checked_add(
                representation_outcome_count
                    .checked_mul(REQUEST_ROW_INSTRUCTIONS)
                    .ok_or(Error::ArtifactGeometry)?,
            )
            .ok_or(Error::ArtifactGeometry)?,
    );
    fixed.extend([
        RequestInstructionV1::require_u64(
            req_fixed(wire::REQUEST_MAGIC_OFFSET)?,
            u64::from_le_bytes(OPEN_REPRESENTATION_HOT_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            req_fixed(wire::REQUEST_VERSION_OFFSET)?,
            OPEN_REPRESENTATION_HOT_VERSION_V3,
        ),
        RequestInstructionV1::require_u8(req_fixed(wire::REQUEST_ACTION_OFFSET)?, action as u8),
        RequestInstructionV1::require_u8(
            req_fixed(wire::REQUEST_CALLER_ROLE_OFFSET)?,
            CallerRoleV2::Trading as u8,
        ),
        RequestInstructionV1::require_zero(req_fixed(wire::REQUEST_RESERVED_HEADER_OFFSET)?, 4),
        RequestInstructionV1::require_zero(req_fixed(wire::REQUEST_PARENT_CONTEXT_OFFSET)?, 32),
        RequestInstructionV1::require_zero(req_fixed(wire::REQUEST_REALM_OFFSET)?, 64),
        RequestInstructionV1::require_u64(
            req_fixed(wire::REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET)?,
            ABSENT_REVISION,
        ),
        RequestInstructionV1::require_u64(
            req_fixed(wire::REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET)?,
            ABSENT_REVISION,
        ),
        RequestInstructionV1::require_u64(
            req_fixed(wire::REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET)?,
            ABSENT_REVISION,
        ),
        RequestInstructionV1::require_u64(
            req_fixed(wire::REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET)?,
            ABSENT_REVISION,
        ),
        RequestInstructionV1::require_u32(
            req_fixed(wire::REQUEST_SELECTED_OUTCOME_OFFSET)?,
            u32::MAX,
        ),
        RequestInstructionV1::require_zero(req_fixed(wire::REQUEST_RESERVED_TAIL_OFFSET)?, 4),
    ]);
    for (offset, register) in [
        (wire::REQUEST_RELEASE_SET_OFFSET, ID_RELEASE),
        (wire::REQUEST_MARKET_OFFSET, ID_MARKET),
        (wire::REQUEST_GRAPH_ID_OFFSET, ID_GRAPH),
        (wire::REQUEST_DESCRIPTOR_ID_OFFSET, ID_DESCRIPTOR),
        (wire::REQUEST_ACTOR_OFFSET, ID_ACTOR),
        (wire::REQUEST_RECEIPT_MINT_OFFSET, ID_RECEIPT_MINT),
        (wire::REQUEST_RECEIPT_ACCOUNT_OFFSET, ID_RECEIPT_ACCOUNT),
        (wire::REQUEST_REPRESENTATION_AUTHORITY_OFFSET, ID_AUTHORITY),
        (wire::REQUEST_TOKEN_PROGRAM_OFFSET, ID_TOKEN),
    ] {
        fixed.push(RequestInstructionV1::project_identity(
            req_fixed(offset)?,
            id_common(register)?,
        ));
    }
    for (offset, register) in [
        (
            wire::REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET,
            SCALAR_REPRESENTATION_REVISION,
        ),
        (wire::REQUEST_GENERATION_OFFSET, SCALAR_GENERATION),
        (wire::REQUEST_QUANTITY_OFFSET, SCALAR_QUANTITY),
        (wire::REQUEST_DENOMINATOR_OFFSET, SCALAR_DENOMINATOR),
        (
            wire::REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET,
            SCALAR_RECEIPT_SUPPLY,
        ),
    ] {
        fixed.push(RequestInstructionV1::project_u64(
            req_fixed(offset)?,
            scalar_common(register)?,
        ));
    }
    for (offset, register) in [
        (wire::REQUEST_OUTCOME_COUNT_OFFSET, SCALAR_OUTCOME_COUNT),
        (wire::REQUEST_ASSET_COUNT_OFFSET, SCALAR_ASSET_COUNT),
    ] {
        fixed.push(RequestInstructionV1::project_u32(
            req_fixed(offset)?,
            scalar_common(register)?,
        ));
    }
    for row in 0..representation_outcome_count {
        let row_offset = REQUEST_HEADER_BYTES_V2
            .checked_add(
                row.checked_mul(ASSET_BYTES_V2)
                    .ok_or(Error::ArtifactGeometry)?,
            )
            .ok_or(Error::ArtifactGeometry)?;
        for (offset, register) in [
            (wire::ASSET_SHARD_MINT_OFFSET, ITEM_ID_SHARD_MINT),
            (wire::ASSET_ACTOR_SHARD_ACCOUNT_OFFSET, ITEM_ID_ACTOR_SHARDS),
            (
                wire::ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET,
                ITEM_ID_STRUCTURED_CUSTODY,
            ),
            (
                wire::ASSET_CLAIMS_CUSTODY_OWNER_OFFSET,
                ITEM_ID_CLAIMS_CUSTODY_OWNER,
            ),
        ] {
            fixed.push(RequestInstructionV1::project_identity(
                req_fixed(
                    row_offset
                        .checked_add(offset)
                        .ok_or(Error::ArtifactGeometry)?,
                )?,
                id_common(row_identity(row, register)?)?,
            ));
        }
        for (offset, register) in [
            (wire::ASSET_COEFFICIENT_OFFSET, ITEM_SCALAR_COEFFICIENT),
            (
                wire::ASSET_EXPECTED_SHARD_SUPPLY_OFFSET,
                ITEM_SCALAR_SHARD_SUPPLY,
            ),
            (
                wire::ASSET_EXPECTED_ACTOR_SHARDS_OFFSET,
                ITEM_SCALAR_ACTOR_SHARDS,
            ),
            (
                wire::ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET,
                ITEM_SCALAR_STRUCTURED_SHARDS,
            ),
        ] {
            fixed.push(RequestInstructionV1::project_u64(
                req_fixed(
                    row_offset
                        .checked_add(offset)
                        .ok_or(Error::ArtifactGeometry)?,
                )?,
                scalar_common(row_scalar(row, register)?)?,
            ));
        }
    }
    if fixed.len()
        != REQUEST_BASE_INSTRUCTIONS + representation_outcome_count * REQUEST_ROW_INSTRUCTIONS
    {
        return Err(Error::ArtifactGeometry);
    }
    let width = REQUEST_PROFILE_HEADER_BYTES + fixed.len() * REQUEST_OPERATION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            narrow_u32(structured_request_bytes(representation_outcome_count)?)?,
            0,
            narrow_u16(structured_common_scalars(representation_outcome_count)?)?,
            0,
            narrow_u16(structured_common_identities(representation_outcome_count)?)?,
            0,
        ),
        &fixed,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::RequestProfileArtifact)?;
    Ok(output)
}

fn encode_transition(representation_outcome_count: u32) -> Result<Vec<u8>> {
    let representation_outcome_count =
        usize::try_from(representation_outcome_count).map_err(|_| Error::ArtifactGeometry)?;
    let mut prelude = Vec::with_capacity(
        TRANSITION_BASE_INSTRUCTIONS
            .checked_add(
                representation_outcome_count
                    .checked_mul(TRANSITION_ROW_INSTRUCTIONS)
                    .ok_or(Error::ArtifactGeometry)?,
            )
            .ok_or(Error::ArtifactGeometry)?,
    );
    prelude.extend([
        InstructionV3::scalar_eq(
            transition_common(SCALAR_ASSET_COUNT)?,
            transition_common(SCALAR_OUTCOME_COUNT)?,
        ),
        InstructionV3::nonzero(transition_common(SCALAR_OUTCOME_COUNT)?),
        InstructionV3::nonzero(transition_common(SCALAR_QUANTITY)?),
        InstructionV3::nonzero(transition_common(SCALAR_DENOMINATOR)?),
    ]);
    for row in 0..representation_outcome_count {
        prelude.push(InstructionV3::scalar_eq(
            transition_common(row_scalar(row, ITEM_SCALAR_COEFFICIENT)?)?,
            transition_common(SCALAR_DENOMINATOR)?,
        ));
    }
    if prelude.len()
        != TRANSITION_BASE_INSTRUCTIONS + representation_outcome_count * TRANSITION_ROW_INSTRUCTIONS
    {
        return Err(Error::ArtifactGeometry);
    }
    let width = TRANSITION_HEADER_BYTES + prelude.len() * TRANSITION_INSTRUCTION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: narrow_u16(structured_common_scalars(representation_outcome_count)?)?,
            item_scalar_stride: 0,
            common_identities: narrow_u16(structured_common_identities(
                representation_outcome_count,
            )?)?,
            item_identity_stride: 0,
        },
        &prelude,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::TransitionArtifact)?;
    Ok(output)
}

fn encode_effect(
    action: RepresentationActionV2,
    representation_outcome_count: u32,
) -> Result<Vec<u8>> {
    let representation_outcome_count =
        usize::try_from(representation_outcome_count).map_err(|_| Error::ArtifactGeometry)?;
    let request_bytes = structured_request_bytes(representation_outcome_count)?;
    let mut fixed_template = vec![0_u8; request_bytes];
    put(
        &mut fixed_template,
        wire::REQUEST_MAGIC_OFFSET,
        &REQUEST_MAGIC_V2,
    )?;
    put(
        &mut fixed_template,
        wire::REQUEST_VERSION_OFFSET,
        &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
    )?;
    put(
        &mut fixed_template,
        wire::REQUEST_ACTION_OFFSET,
        &[action as u8],
    )?;
    put(
        &mut fixed_template,
        wire::REQUEST_CALLER_ROLE_OFFSET,
        &[CallerRoleV2::Trading as u8],
    )?;
    for offset in [
        wire::REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET,
        wire::REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET,
        wire::REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET,
        wire::REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET,
    ] {
        put(&mut fixed_template, offset, &ABSENT_REVISION.to_le_bytes())?;
    }
    put(
        &mut fixed_template,
        wire::REQUEST_SELECTED_OUTCOME_OFFSET,
        &u32::MAX.to_le_bytes(),
    )?;
    let route = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: INJECTED_ACCOUNTS,
        fixed_account_count: narrow_u16(
            CLAIMS_FIXED_ACCOUNTS as usize
                + representation_outcome_count * RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3 as usize,
        )?,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &fixed_template,
        item_request: &[],
    }];
    let mut fixed = Vec::with_capacity(
        EFFECT_BASE_INSTRUCTIONS + representation_outcome_count * EFFECT_ROW_INSTRUCTIONS,
    );
    for (offset, register) in [
        (wire::REQUEST_PARENT_CONTEXT_OFFSET, ID_PARENT),
        (wire::REQUEST_RELEASE_SET_OFFSET, ID_RELEASE),
        (wire::REQUEST_MARKET_OFFSET, ID_MARKET),
        (wire::REQUEST_GRAPH_ID_OFFSET, ID_GRAPH),
        (wire::REQUEST_DESCRIPTOR_ID_OFFSET, ID_DESCRIPTOR),
        (wire::REQUEST_ACTOR_OFFSET, ID_ACTOR),
        (wire::REQUEST_RECEIPT_MINT_OFFSET, ID_RECEIPT_MINT),
        (wire::REQUEST_RECEIPT_ACCOUNT_OFFSET, ID_RECEIPT_ACCOUNT),
        (wire::REQUEST_REPRESENTATION_AUTHORITY_OFFSET, ID_AUTHORITY),
        (wire::REQUEST_TOKEN_PROGRAM_OFFSET, ID_TOKEN),
    ] {
        fixed.push(EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            effect_id_common(register)?,
        ));
    }
    for (offset, register) in [
        (
            wire::REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET,
            SCALAR_REPRESENTATION_REVISION,
        ),
        (wire::REQUEST_GENERATION_OFFSET, SCALAR_GENERATION),
        (wire::REQUEST_QUANTITY_OFFSET, SCALAR_QUANTITY),
        (wire::REQUEST_DENOMINATOR_OFFSET, SCALAR_DENOMINATOR),
        (
            wire::REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET,
            SCALAR_RECEIPT_SUPPLY,
        ),
    ] {
        fixed.push(EffectInstructionV3::write_request_u64(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            effect_scalar_common(register)?,
        ));
    }
    for (offset, register) in [
        (wire::REQUEST_OUTCOME_COUNT_OFFSET, SCALAR_OUTCOME_COUNT),
        (wire::REQUEST_ASSET_COUNT_OFFSET, SCALAR_ASSET_COUNT),
    ] {
        fixed.push(EffectInstructionV3::write_request_u32(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            effect_scalar_common(register)?,
        ));
    }
    for row in 0..representation_outcome_count {
        let row_offset = REQUEST_HEADER_BYTES_V2
            .checked_add(
                row.checked_mul(ASSET_BYTES_V2)
                    .ok_or(Error::ArtifactGeometry)?,
            )
            .ok_or(Error::ArtifactGeometry)?;
        for (offset, register) in [
            (wire::ASSET_SHARD_MINT_OFFSET, ITEM_ID_SHARD_MINT),
            (wire::ASSET_ACTOR_SHARD_ACCOUNT_OFFSET, ITEM_ID_ACTOR_SHARDS),
            (
                wire::ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET,
                ITEM_ID_STRUCTURED_CUSTODY,
            ),
            (
                wire::ASSET_CLAIMS_CUSTODY_OWNER_OFFSET,
                ITEM_ID_CLAIMS_CUSTODY_OWNER,
            ),
        ] {
            fixed.push(EffectInstructionV3::write_request_identity(
                0,
                RequestSpaceV3::Fixed,
                narrow_u32(
                    row_offset
                        .checked_add(offset)
                        .ok_or(Error::ArtifactGeometry)?,
                )?,
                effect_id_common(row_identity(row, register)?)?,
            ));
        }
        for (offset, register) in [
            (wire::ASSET_COEFFICIENT_OFFSET, ITEM_SCALAR_COEFFICIENT),
            (
                wire::ASSET_EXPECTED_SHARD_SUPPLY_OFFSET,
                ITEM_SCALAR_SHARD_SUPPLY,
            ),
            (
                wire::ASSET_EXPECTED_ACTOR_SHARDS_OFFSET,
                ITEM_SCALAR_ACTOR_SHARDS,
            ),
            (
                wire::ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET,
                ITEM_SCALAR_STRUCTURED_SHARDS,
            ),
        ] {
            fixed.push(EffectInstructionV3::write_request_u64(
                0,
                RequestSpaceV3::Fixed,
                narrow_u32(
                    row_offset
                        .checked_add(offset)
                        .ok_or(Error::ArtifactGeometry)?,
                )?,
                effect_scalar_common(row_scalar(row, register)?)?,
            ));
        }
    }
    if fixed.len()
        != EFFECT_BASE_INSTRUCTIONS + representation_outcome_count * EFFECT_ROW_INSTRUCTIONS
    {
        return Err(Error::ArtifactGeometry);
    }
    let width = EFFECT_HEADER_BYTES
        + EFFECT_ROUTE_BYTES
        + fixed.len() * EFFECT_OPERATION_BYTES
        + fixed_template.len();
    let mut scratch = vec![0_u8; width];
    let mut base = vec![0_u8; width];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: narrow_u16(structured_logical_accounts(representation_outcome_count)?)?,
            item_account_stride: 0,
            common_scalars: narrow_u16(structured_common_scalars(representation_outcome_count)?)?,
            item_scalar_stride: 0,
            common_identities: narrow_u16(structured_common_identities(
                representation_outcome_count,
            )?)?,
            item_identity_stride: 0,
        },
        &route,
        &fixed,
        &[],
        &mut scratch,
        &mut base,
    )
    .map_err(Error::EffectArtifact)?;
    let mut scratch = vec![0_u8; EFFECT_V4_HEADER_BYTES + base.len()];
    let mut output = vec![0_u8; EFFECT_V4_HEADER_BYTES + base.len()];
    encode_program_v4_atomic(
        &base,
        BorrowedRangePolicyV4::DisjointExactCoverage,
        narrow_u32(request_bytes)?,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::EffectArtifactV4)?;
    Ok(output)
}

fn register_geometry(representation_outcome_count: usize) -> Result<RegisterGeometryV2> {
    Ok(RegisterGeometryV2 {
        common_scalars: narrow_u16(structured_common_scalars(representation_outcome_count)?)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(structured_common_identities(representation_outcome_count)?)?,
        item_identity_stride: 0,
    })
}

fn req_fixed(offset: usize) -> Result<RequestCoordinateV1> {
    Ok(RequestCoordinateV1::fixed(narrow_u32(offset)?))
}

fn id_common(index: usize) -> Result<IdentityRegisterV1> {
    Ok(IdentityRegisterV1::common(narrow_u16(index)?))
}

fn scalar_common(index: usize) -> Result<ScalarRegisterV1> {
    Ok(ScalarRegisterV1::common(narrow_u16(index)?))
}

fn transition_common(index: usize) -> Result<ScalarRegisterV3> {
    Ok(ScalarRegisterV3::common(narrow_u16(index)?))
}

fn effect_id_common(index: usize) -> Result<IdentityCoordinateV3> {
    Ok(IdentityCoordinateV3::common(narrow_u16(index)?))
}

fn effect_scalar_common(index: usize) -> Result<ScalarCoordinateV3> {
    Ok(ScalarCoordinateV3::common(narrow_u16(index)?))
}

fn row_identity(row: usize, local: usize) -> Result<usize> {
    row.checked_mul(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)
        .and_then(|offset| RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3.checked_add(offset))
        .and_then(|base| base.checked_add(local))
        .ok_or(Error::ArtifactGeometry)
}

fn row_scalar(row: usize, local: usize) -> Result<usize> {
    row.checked_mul(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)
        .and_then(|offset| RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3.checked_add(offset))
        .and_then(|base| base.checked_add(local))
        .ok_or(Error::ArtifactGeometry)
}

fn structured_common_identities(representation_outcome_count: usize) -> Result<usize> {
    row_identity(representation_outcome_count, 0)
}

fn structured_common_scalars(representation_outcome_count: usize) -> Result<usize> {
    row_scalar(representation_outcome_count, 0)
}

fn structured_logical_accounts(representation_outcome_count: usize) -> Result<usize> {
    representation_outcome_count
        .checked_mul(usize::from(RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3))
        .and_then(|tail| usize::from(RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3).checked_add(tail))
        .filter(|count| *count <= 256)
        .ok_or(Error::ArtifactGeometry)
}

fn structured_request_bytes(representation_outcome_count: usize) -> Result<usize> {
    representation_outcome_count
        .checked_mul(ASSET_BYTES_V2)
        .and_then(|tail| REQUEST_HEADER_BYTES_V2.checked_add(tail))
        .ok_or(Error::ArtifactGeometry)
}

fn require_representation_width(input: RationalOpenStructuredHotBundleInputV3<'_>) -> Result<u32> {
    let basis =
        ProductBasisV3::decode(input.product_basis).map_err(|_| Error::AccountProfileInput)?;
    let representation_outcome_count = input.representation_descriptor.outcome_count();
    let representation_width =
        usize::try_from(representation_outcome_count).map_err(|_| Error::AccountProfileInput)?;
    if representation_outcome_count == 0
        || representation_outcome_count > RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3
        || input.fixed_data_lengths.len() != usize::from(RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3)
        || input.representation_descriptor.descriptor_id()
            != input.authenticated_token_behavior.descriptor_id()
        || input.representation_descriptor.release_set_id()
            != input.authenticated_token_behavior.selection().release_set()
        || input.representation_descriptor.token_program()
            != input
                .authenticated_token_behavior
                .selection()
                .token_program()
        || input.fixed_data_lengths.get(4).copied() != u32::try_from(input.product_basis.len()).ok()
        || input.fixed_data_lengths.get(29).copied()
            != u32::try_from(input.product_basis.len()).ok()
        || basis.basis_width() == 0
    {
        return Err(Error::AccountProfileInput);
    }
    structured_logical_accounts(representation_width)?;
    Ok(representation_outcome_count)
}

fn narrow_u16(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::ArtifactGeometry)
}

fn narrow_u32(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::ArtifactGeometry)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::ArtifactGeometry)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::ArtifactGeometry)?
        .copy_from_slice(value);
    Ok(())
}

fn digest(bytes: &[u8]) -> Result<ContentId> {
    content(hash(bytes).to_bytes())
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| Error::ContentIdentity)
}

fn artifact(schema: [u8; 32], program: [u8; 32]) -> Result<ArtifactReferenceV4> {
    Ok(ArtifactReferenceV4::new(
        content(schema)?,
        content(program)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BasisInputV3, BasisKindV3, compile_basis_v3,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn basis(width: u32) -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: id(1),
                result_domain_id: id(2),
                coordinate_domain_id: id(3),
                result_unit_id: id(4),
                evaluator_release_id: id(5),
                basis_width: width,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
            },
            &mut output,
        )
        .expect("basis");
        output
    }

    fn input<'a>(
        action: RepresentationActionV2,
        basis: &'a [u8],
        lengths: &'a [u32],
    ) -> RationalOpenStructuredHotBundleInputV3<'a> {
        RationalOpenStructuredHotBundleInputV3 {
            action,
            fixed_data_lengths: lengths,
            item_data_lengths: [64, 82, 165, 165],
            product_basis: basis,
            representation_descriptor: crate::test_open_fixture_v3::representation_descriptor_v3(
                id(4),
                id(16),
                3,
            ),
            kind: id(10),
            authenticated_token_behavior:
                crate::test_open_fixture_v3::authenticated_token_behavior_v3(
                    id(4),
                    id(15),
                    id(16),
                    3,
                ),
            root_schema: id(12),
            lifecycle_policy: crate::test_open_fixture_v3::lifecycle_policy(),
            capacity_profile: id(14),
            root_state_bytes: 8,
        }
    }

    fn lengths(basis: &[u8]) -> [u32; RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3 as usize] {
        let mut output = [0_u32; RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3 as usize];
        let width = u32::try_from(basis.len()).expect("basis width");
        *output.get_mut(4).expect("basis") = width;
        *output.get_mut(29).expect("basis alias") = width;
        output
    }

    #[test]
    fn structured_actions_keep_descriptor_k_independent_from_product_n() {
        let basis = basis(258);
        let lengths = lengths(&basis);
        for action in [
            RepresentationActionV2::IssueStructured,
            RepresentationActionV2::UnwrapStructured,
        ] {
            let bundle =
                build_rational_open_structured_hot_bundle_v3(input(action, &basis, &lengths))
                    .expect("structured bundle");
            validate_rational_open_structured_hot_bundle_v3(&bundle).expect("join");
            validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
                &bundle,
                input(action, &basis, &lengths).authenticated_token_behavior,
            )
            .expect("Realm/release join");
            assert_eq!(bundle.representation_outcome_count, 3);
            let account = AccountProfileV2::decode(&bundle.account_profile).expect("profile13");
            assert_eq!(account.dynamic_fixed_span_count(), 0);
            assert_eq!(
                account.logical_account_count_with_dynamic_spans(258, &[]),
                Ok(37 + 4 * 3)
            );
            assert_eq!(
                account.physical_account_count_with_dynamic_spans(258, &[]),
                Ok(33 + 4 * 3)
            );
            assert_eq!(
                account.rule(false, 29).expect("basis alias").prestate(),
                AccountPrestateV2::AuthenticatedRouteAlias
            );
            for coordinate in [6_u16, 7, 19, 20, 21, 23, 24, 25, 26, 27, 38, 39, 40] {
                assert_eq!(
                    account.rule(false, coordinate).expect("opaque").prestate(),
                    AccountPrestateV2::AuthenticatedOpaqueReadonlyData
                );
            }
            let effect = EffectProgramV4::decode(&bundle.effect).expect("effect");
            let effect = effect.base();
            assert_eq!(
                effect.account_count(258).expect("account width"),
                37 + 4 * 3
            );
            assert_eq!(effect.scalar_count(258).expect("scalar width"), 9 + 4 * 3);
            assert_eq!(
                effect.identity_count(258).expect("identity width"),
                11 + 4 * 3
            );
            let (fixed, item) = effect.route_template(0).expect("templates");
            assert_eq!(fixed.len(), REQUEST_HEADER_BYTES_V2 + 3 * ASSET_BYTES_V2);
            assert!(item.is_empty());
            assert_eq!(
                fixed
                    .get(wire::REQUEST_ACTION_OFFSET)
                    .copied()
                    .expect("action"),
                action as u8
            );
        }
    }

    #[test]
    fn structured_bundle_refuses_action_width_and_artifact_substitution() {
        let canonical_basis = basis(258);
        let canonical_lengths = lengths(&canonical_basis);
        assert_eq!(
            build_rational_open_structured_hot_bundle_v3(input(
                RepresentationActionV2::Denominate,
                &canonical_basis,
                &canonical_lengths,
            )),
            Err(Error::ArtifactGeometry)
        );
        let narrow = basis(1);
        let narrow_lengths = lengths(&narrow);
        let independent = build_rational_open_structured_hot_bundle_v3(input(
            RepresentationActionV2::IssueStructured,
            &narrow,
            &narrow_lengths,
        ))
        .expect("K=3 remains independent from N=1");
        assert_eq!(independent.representation_outcome_count, 3);
        let mut bundle = build_rational_open_structured_hot_bundle_v3(input(
            RepresentationActionV2::IssueStructured,
            &canonical_basis,
            &canonical_lengths,
        ))
        .expect("bundle");
        *bundle.request_profile.get_mut(0).expect("profile byte") ^= 1;
        assert!(validate_rational_open_structured_hot_bundle_v3(&bundle).is_err());
    }
}
