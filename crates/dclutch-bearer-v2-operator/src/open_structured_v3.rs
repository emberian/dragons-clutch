//! Data-defined Hot artifacts for full-width Structured issue and unwrap.

use dclutch_account_profile_contract::lifecycle_v3::{
    SUCCESSOR_SCHEMA_RELEASE_ID as LIFECYCLE_SCHEMA_ID_V4, StateLifecyclePolicyV4,
};
use dclutch_account_profile_contract::v2::{
    AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE, AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES,
    AccountPrestateV2, AccountProfileV2, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES,
    RULE_BYTES as ACCOUNT_RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_with_authenticated_route_alias_v2_atomic,
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
use dclutch_product_payoff_v2_codec::runtime_v3::{
    BASIS_HEADER_BYTES_V3, BASIS_WIDTH_OFFSET_V3, BasisKindV3, ProductBasisV3,
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AuthenticatedTokenBehaviorV2, CallerRoleV2,
    OPEN_REPRESENTATION_HOT_MAGIC_V3, OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3,
    OPEN_REPRESENTATION_HOT_VERSION_V3, PHYSICAL_ABI_VERSION_V2, REQUEST_HEADER_BYTES_V2,
    REQUEST_MAGIC_V2, RepresentationActionV2,
};
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
/// Common request registers plus trusted current Trading.
pub const RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3: usize = 11;
/// Per-outcome request identity stride.
pub const RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3: usize = 4;
/// Common request scalars, Product width, and trusted current slot.
pub const RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3: usize = 9;
/// Per-outcome request scalar stride.
pub const RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3: usize = 4;

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

const REQUEST_FIXED_INSTRUCTIONS: usize = 30;
const REQUEST_ITEM_INSTRUCTIONS: usize = 8;
const TRANSITION_PRELUDE_INSTRUCTIONS: usize = 4;
const TRANSITION_ITEM_INSTRUCTIONS: usize = 1;
const EFFECT_FIXED_INSTRUCTIONS: usize = 17;
const EFFECT_ITEM_INSTRUCTIONS: usize = 8;

/// Release-owned coordinates and exact fixed/item account data widths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalOpenStructuredHotBundleInputV3<'a> {
    /// IssueStructured or UnwrapStructured.
    pub action: RepresentationActionV2,
    /// Exact logical-37 fixed account data lengths.
    pub fixed_data_lengths: &'a [u32],
    /// Exact four account data lengths repeated for every Product outcome.
    pub item_data_lengths: [u32; 4],
    /// Exact Registry-authenticated CategoricalQ1 ProductBasis body.
    pub product_basis: &'a [u8],
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
    /// Exact config record body selected alongside the descriptor.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// Profile11 affine account interpreter with compact physical aliases.
    pub account_profile: Vec<u8>,
    /// Variable-width open-family RequestProfile.
    pub request_profile: Vec<u8>,
    /// Exact successor lifecycle policy.
    pub lifecycle_policy: Vec<u8>,
    /// Full-width coefficient transition.
    pub transition: Vec<u8>,
    /// Interpreted ExecutionStrategy.
    pub strategy: [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    /// One AffineOnce Claims effect.
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
    let account_profile = encode_account_profile(input)?;
    let lifecycle_policy = Vec::from(input.lifecycle_policy);
    let request_profile = encode_request_profile(input.action)?;
    let transition = encode_transition()?;
    let effect = encode_effect(input.action)?;
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
            lifecycle: artifact(LIFECYCLE_SCHEMA_ID_V4, lifecycle_id.to_bytes())?,
            strategy: artifact(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                digest(&strategy)?.to_bytes(),
            )?,
            transition: artifact(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                digest(&transition)?.to_bytes(),
            )?,
            effect: artifact(
                dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID,
                digest(&effect)?.to_bytes(),
            )?,
        },
        input.root_state_bytes,
    )
    .map_err(Error::CapabilityDescriptor)?
    .encode();
    let bundle = RationalOpenStructuredHotBundleV3 {
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
    let descriptor =
        CapabilityProgramV4::decode(&bundle.descriptor).map_err(Error::CapabilityDescriptor)?;
    TokenBehaviorSelectionV2::decode(&bundle.token_behavior_selection)
        .map_err(Error::TokenBehavior)?;
    let account =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfileArtifact)?;
    let lifecycle_id = digest(&bundle.lifecycle_policy)?;
    let lifecycle = StateLifecyclePolicyV4::decode_selected(
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
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect().program().to_bytes(),
        hash(&bundle.effect).to_bytes(),
        &bundle.effect,
    )
    .map_err(Error::EffectArtifact)?;
    let route = effect.route(0).map_err(Error::EffectArtifact)?;
    let (fixed_template, item_template) =
        effect.route_template(0).map_err(Error::EffectArtifact)?;
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
        || descriptor.lifecycle() != artifact(LIFECYCLE_SCHEMA_ID_V4, lifecycle_id.to_bytes())?
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
        || descriptor.effect()
            != artifact(
                dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID,
                digest(&bundle.effect)?.to_bytes(),
            )?
        || strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
        || account.artifact_profile() != AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
        || account.fixed_account_count() != RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3
        || account.item_account_stride() != RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3
        || account.common_scalar_count() != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?
        || account.item_scalar_stride() != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?
        || account.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?
        || account.item_identity_stride()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?
        || account.trusted_current_slot_scalar() != Some(narrow_u16(SCALAR_CURRENT_SLOT)?)
        || account.trusted_current_executing_program_identity()
            != Some(narrow_u16(ID_CURRENT_TRADING)?)
        || request.fixed_request_bytes() != narrow_u32(REQUEST_HEADER_BYTES_V2)?
        || request.item_request_bytes() != narrow_u32(ASSET_BYTES_V2)?
        || request.common_scalar_count() != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?
        || request.item_scalar_stride() != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?
        || request.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?
        || request.item_identity_stride()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?
        || transition.common_scalar_count()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?
        || transition.item_scalar_stride() != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?
        || transition.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?
        || transition.item_identity_stride()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?
        || effect.fixed_account_count() != RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3
        || effect.item_account_stride() != RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3
        || effect.common_scalar_count() != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?
        || effect.item_scalar_stride() != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?
        || effect.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?
        || effect.item_identity_stride() != narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?
        || effect.route_count() != 1
        || route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::AffineOnce
        || route.fixed_account_start() != INJECTED_ACCOUNTS
        || route.fixed_account_count() != CLAIMS_FIXED_ACCOUNTS
        || route.item_account_start() != 0
        || route.item_account_count() != RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3
        || fixed_template.len() != REQUEST_HEADER_BYTES_V2
        || item_template.len() != ASSET_BYTES_V2
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

fn encode_account_profile(input: RationalOpenStructuredHotBundleInputV3<'_>) -> Result<Vec<u8>> {
    if input.fixed_data_lengths.len() != usize::from(RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3) {
        return Err(Error::AccountProfileInput);
    }
    let basis =
        ProductBasisV3::decode(input.product_basis).map_err(|_| Error::AccountProfileInput)?;
    if basis.kind() != BasisKindV3::CategoricalQ1 || basis.basis_width() < 2 {
        return Err(Error::AccountProfileInput);
    }
    let mut fixed_rules = Vec::with_capacity(input.fixed_data_lengths.len());
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
        let prestate = if index == 4 {
            AccountPrestateV2::AdapterAuthenticatedVariableData
        } else if alias != AccountAliasInputV2::SelfCoordinate {
            AccountPrestateV2::AuthenticatedRouteAlias
        } else {
            AccountPrestateV2::Exact
        };
        let data_length = match index {
            1 => narrow_u32(TOKEN_BEHAVIOR_SELECTION_BYTES_V2)?,
            4 => narrow_u32(BASIS_HEADER_BYTES_V3)?,
            28 | 29 | 31 | 35 => 0,
            _ => *input
                .fixed_data_lengths
                .get(index)
                .ok_or(Error::AccountProfileInput)?,
        };
        fixed_rules.push(rule(
            signer,
            writable,
            executable,
            alias,
            prestate,
            data_length,
        ));
    }
    let item_rules = input
        .item_data_lengths
        .iter()
        .enumerate()
        .map(|(index, length)| {
            rule(
                false,
                matches!(index, 2 | 3),
                false,
                AccountAliasInputV2::SelfCoordinate,
                AccountPrestateV2::Exact,
                *length,
            )
        })
        .collect::<Vec<_>>();
    let operations = [AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(4),
        destination: ScalarCoordinateV2::common(narrow_u16(SCALAR_PRODUCT_OUTCOME_COUNT)?),
        data_offset: narrow_u32(BASIS_WIDTH_OFFSET_V3)?,
    }];
    let width = AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES
        + (fixed_rules.len() + item_rules.len()) * ACCOUNT_RULE_BYTES
        + ACCOUNT_OPERATION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_account_profile_with_authenticated_route_alias_v2_atomic(
        TrustedEnvironmentV2::CurrentSlot {
            destination: narrow_u16(SCALAR_CURRENT_SLOT)?,
        },
        TrustedIdentityEnvironmentV2::CurrentExecutingProgram {
            destination: narrow_u16(ID_CURRENT_TRADING)?,
        },
        TrustedBuiltinIdentityV2::None,
        &fixed_rules,
        &item_rules,
        &operations,
        &[],
        register_geometry()?,
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

fn encode_request_profile(action: RepresentationActionV2) -> Result<Vec<u8>> {
    let mut fixed = Vec::with_capacity(REQUEST_FIXED_INSTRUCTIONS);
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
        RequestInstructionV1::require_zero(req_fixed(wire::REQUEST_REALM_OFFSET)?, 32),
        RequestInstructionV1::require_zero(
            req_fixed(wire::REQUEST_COLLATERAL_RECIPIENT_OFFSET)?,
            32,
        ),
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
    let mut item = Vec::with_capacity(REQUEST_ITEM_INSTRUCTIONS);
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
        item.push(RequestInstructionV1::project_identity(
            req_item(offset)?,
            id_item(register)?,
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
        item.push(RequestInstructionV1::project_u64(
            req_item(offset)?,
            scalar_item(register)?,
        ));
    }
    if fixed.len() != REQUEST_FIXED_INSTRUCTIONS || item.len() != REQUEST_ITEM_INSTRUCTIONS {
        return Err(Error::ArtifactGeometry);
    }
    let width = REQUEST_PROFILE_HEADER_BYTES + (fixed.len() + item.len()) * REQUEST_OPERATION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            narrow_u32(REQUEST_HEADER_BYTES_V2)?,
            narrow_u32(ASSET_BYTES_V2)?,
            narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?,
            narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?,
            narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?,
            narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?,
        ),
        &fixed,
        &item,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::RequestProfileArtifact)?;
    Ok(output)
}

fn encode_transition() -> Result<Vec<u8>> {
    let prelude = [
        InstructionV3::scalar_eq(
            transition_common(SCALAR_OUTCOME_COUNT)?,
            transition_common(SCALAR_PRODUCT_OUTCOME_COUNT)?,
        ),
        InstructionV3::scalar_eq(
            transition_common(SCALAR_ASSET_COUNT)?,
            transition_common(SCALAR_PRODUCT_OUTCOME_COUNT)?,
        ),
        InstructionV3::nonzero(transition_common(SCALAR_QUANTITY)?),
        InstructionV3::nonzero(transition_common(SCALAR_DENOMINATOR)?),
    ];
    let item = [InstructionV3::scalar_eq(
        transition_item(ITEM_SCALAR_COEFFICIENT)?,
        transition_common(SCALAR_DENOMINATOR)?,
    )];
    if prelude.len() != TRANSITION_PRELUDE_INSTRUCTIONS
        || item.len() != TRANSITION_ITEM_INSTRUCTIONS
    {
        return Err(Error::ArtifactGeometry);
    }
    let width =
        TRANSITION_HEADER_BYTES + (prelude.len() + item.len()) * TRANSITION_INSTRUCTION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?,
            item_scalar_stride: narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?,
            common_identities: narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?,
            item_identity_stride: narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?,
        },
        &prelude,
        &item,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::TransitionArtifact)?;
    Ok(output)
}

fn encode_effect(action: RepresentationActionV2) -> Result<Vec<u8>> {
    let mut fixed_template = vec![0_u8; REQUEST_HEADER_BYTES_V2];
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
    let item_template = [0_u8; ASSET_BYTES_V2];
    let route = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::AffineOnce,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: INJECTED_ACCOUNTS,
        fixed_account_count: CLAIMS_FIXED_ACCOUNTS,
        item_account_start: 0,
        item_account_count: RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3,
        fixed_request: &fixed_template,
        item_request: &item_template,
    }];
    let mut fixed = Vec::with_capacity(EFFECT_FIXED_INSTRUCTIONS);
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
    let mut item = Vec::with_capacity(EFFECT_ITEM_INSTRUCTIONS);
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
        item.push(EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Item,
            narrow_u32(offset)?,
            effect_id_item(register)?,
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
        item.push(EffectInstructionV3::write_request_u64(
            0,
            RequestSpaceV3::Item,
            narrow_u32(offset)?,
            effect_scalar_item(register)?,
        ));
    }
    if fixed.len() != EFFECT_FIXED_INSTRUCTIONS || item.len() != EFFECT_ITEM_INSTRUCTIONS {
        return Err(Error::ArtifactGeometry);
    }
    let width = EFFECT_HEADER_BYTES
        + EFFECT_ROUTE_BYTES
        + (fixed.len() + item.len()) * EFFECT_OPERATION_BYTES
        + fixed_template.len()
        + item_template.len();
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3,
            item_account_stride: RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3,
            common_scalars: narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?,
            item_scalar_stride: narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?,
            common_identities: narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?,
            item_identity_stride: narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?,
        },
        &route,
        &fixed,
        &item,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::EffectArtifact)?;
    Ok(output)
}

fn register_geometry() -> Result<RegisterGeometryV2> {
    Ok(RegisterGeometryV2 {
        common_scalars: narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3)?,
        item_scalar_stride: narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3)?,
        common_identities: narrow_u16(RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3)?,
        item_identity_stride: narrow_u16(RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3)?,
    })
}

fn req_fixed(offset: usize) -> Result<RequestCoordinateV1> {
    Ok(RequestCoordinateV1::fixed(narrow_u32(offset)?))
}

fn req_item(offset: usize) -> Result<RequestCoordinateV1> {
    Ok(RequestCoordinateV1::item(narrow_u32(offset)?))
}

fn id_common(index: usize) -> Result<IdentityRegisterV1> {
    Ok(IdentityRegisterV1::common(narrow_u16(index)?))
}

fn id_item(index: usize) -> Result<IdentityRegisterV1> {
    Ok(IdentityRegisterV1::item(narrow_u16(index)?))
}

fn scalar_common(index: usize) -> Result<ScalarRegisterV1> {
    Ok(ScalarRegisterV1::common(narrow_u16(index)?))
}

fn scalar_item(index: usize) -> Result<ScalarRegisterV1> {
    Ok(ScalarRegisterV1::item(narrow_u16(index)?))
}

fn transition_common(index: usize) -> Result<ScalarRegisterV3> {
    Ok(ScalarRegisterV3::common(narrow_u16(index)?))
}

fn transition_item(index: usize) -> Result<ScalarRegisterV3> {
    Ok(ScalarRegisterV3::item(narrow_u16(index)?))
}

fn effect_id_common(index: usize) -> Result<IdentityCoordinateV3> {
    Ok(IdentityCoordinateV3::common(narrow_u16(index)?))
}

fn effect_id_item(index: usize) -> Result<IdentityCoordinateV3> {
    Ok(IdentityCoordinateV3::item(narrow_u16(index)?))
}

fn effect_scalar_common(index: usize) -> Result<ScalarCoordinateV3> {
    Ok(ScalarCoordinateV3::common(narrow_u16(index)?))
}

fn effect_scalar_item(index: usize) -> Result<ScalarCoordinateV3> {
    Ok(ScalarCoordinateV3::item(narrow_u16(index)?))
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
    use dclutch_product_payoff_v2_codec::runtime_v3::{BasisInputV3, compile_basis_v3};

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
            kind: id(10),
            authenticated_token_behavior:
                crate::test_open_fixture_v3::authenticated_token_behavior_v3(
                    id(4),
                    id(15),
                    id(16),
                    ProductBasisV3::decode(basis).expect("basis").basis_width(),
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
    fn structured_actions_build_one_full_width_affine_claims_route() {
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
            let account = AccountProfileV2::decode(&bundle.account_profile).expect("profile11");
            assert_eq!(account.logical_account_count(258), Ok(37 + 4 * 258));
            assert_eq!(account.physical_account_count(258), Ok(33 + 4 * 258));
            assert_eq!(
                account.rule(false, 29).expect("basis alias").prestate(),
                AccountPrestateV2::AuthenticatedRouteAlias
            );
            let effect = EffectProgramV3::decode_selected(
                CapabilityProgramV4::decode(&bundle.descriptor)
                    .expect("descriptor")
                    .effect()
                    .program()
                    .to_bytes(),
                hash(&bundle.effect).to_bytes(),
                &bundle.effect,
            )
            .expect("effect");
            assert_eq!(
                effect.account_count(258).expect("account width"),
                37 + 4 * 258
            );
            assert_eq!(effect.scalar_count(258).expect("scalar width"), 9 + 4 * 258);
            assert_eq!(
                effect.identity_count(258).expect("identity width"),
                11 + 4 * 258
            );
            let (fixed, item) = effect.route_template(0).expect("templates");
            assert_eq!(fixed.len(), REQUEST_HEADER_BYTES_V2);
            assert_eq!(item.len(), ASSET_BYTES_V2);
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
        assert_eq!(
            build_rational_open_structured_hot_bundle_v3(input(
                RepresentationActionV2::IssueStructured,
                &narrow,
                &narrow_lengths,
            )),
            Err(Error::AccountProfileInput)
        );
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
