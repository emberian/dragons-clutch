//! Data-defined Hot artifacts for selected open Bearer split and merge.

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
    AuthenticatedTokenBehaviorV2, CallerRoleV2, OPEN_REPRESENTATION_HOT_MAGIC_V3,
    OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3, OPEN_REPRESENTATION_HOT_VERSION_V3,
    PHYSICAL_ABI_VERSION_V2, REQUEST_HEADER_BYTES_V2, REQUEST_MAGIC_V2, RepresentationActionV2,
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

/// Common Hot-injected logical observations.
const INJECTED_ACCOUNTS: u16 = 5;
/// Exact selected-action Claims child frame: fixed 32 plus one four-account row.
pub const RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3: u16 = 36;
/// Exact logical AccountProfile/Effect width.
pub const RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3: u16 =
    INJECTED_ACCOUNTS + RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3;
/// Parent/request registers plus trusted current Trading program.
pub const RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3: usize = 16;
/// Request scalars, Product width, and trusted current slot.
pub const RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3: usize = 18;

const ID_PARENT: usize = 0;
const ID_RELEASE: usize = 1;
const ID_MARKET: usize = 2;
const ID_GRAPH: usize = 3;
const ID_DESCRIPTOR: usize = 4;
const ID_ACTOR: usize = 5;
const ID_RECEIPT_MINT: usize = 6;
const ID_AUTHORITY: usize = 7;
const ID_TOKEN: usize = 8;
const ID_SHARD_MINT: usize = 11;
const ID_ACTOR_SHARDS: usize = 12;
const ID_STRUCTURED_CUSTODY: usize = 13;
const ID_CLAIMS_CUSTODY_OWNER: usize = 14;
const ID_CURRENT_TRADING: usize = 15;

const SCALAR_REPRESENTATION_REVISION: usize = 0;
const SCALAR_CLAIMS_REVISION: usize = 1;
const SCALAR_ACTOR_POSITION_REVISION: usize = 2;
const SCALAR_CUSTODY_POSITION_REVISION: usize = 3;
const SCALAR_CUSTODY_REPLAY_REVISION: usize = 4;
const SCALAR_GENERATION: usize = 5;
const SCALAR_QUANTITY: usize = 6;
const SCALAR_DENOMINATOR: usize = 7;
const SCALAR_RECEIPT_SUPPLY: usize = 8;
const SCALAR_OUTCOME_COUNT: usize = 9;
const SCALAR_SELECTED_OUTCOME: usize = 10;
const SCALAR_ASSET_COUNT: usize = 11;
const SCALAR_COEFFICIENT: usize = 12;
const SCALAR_SHARD_SUPPLY: usize = 13;
const SCALAR_ACTOR_SHARDS: usize = 14;
const SCALAR_STRUCTURED_SHARDS: usize = 15;
const SCALAR_PRODUCT_OUTCOME_COUNT: usize = 16;
const SCALAR_CURRENT_SLOT: usize = 17;

const REQUEST_INSTRUCTIONS: usize = 39;
const TRANSITION_INSTRUCTIONS: usize = 5;
const EFFECT_INSTRUCTIONS: usize = 29;
const REQUEST_BYTES: usize = REQUEST_HEADER_BYTES_V2 + wire::ASSET_BYTES_V2;

/// Release-owned coordinates plus one finalized deployment's account widths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalOpenSelectedHotBundleInputV3<'a> {
    /// Denominate or Reconstitute.
    pub action: RepresentationActionV2,
    /// Exact logical-41 account data lengths.
    pub logical_data_lengths: &'a [u32],
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

/// Exact finalized artifact bodies for one selected open action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalOpenSelectedHotBundleV3 {
    /// Exact config record body selected alongside the descriptor.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// Profile11 logical account interpreter with compact physical aliases.
    pub account_profile: Vec<u8>,
    /// Exact open-family RequestProfile.
    pub request_profile: Vec<u8>,
    /// Exact successor lifecycle policy.
    pub lifecycle_policy: Vec<u8>,
    /// Bearer selected-coordinate transition.
    pub transition: Vec<u8>,
    /// Interpreted ExecutionStrategy.
    pub strategy: [u8; EXECUTION_STRATEGY_PROGRAM_BYTES_V2],
    /// One-route Claims effect.
    pub effect: Vec<u8>,
    /// Capability descriptor selecting every artifact.
    pub descriptor: [u8; CAPABILITY_PROGRAM_V4_BYTES],
}

/// Build one complete immutable selected-action artifact bundle.
pub fn build_rational_open_selected_hot_bundle_v3(
    input: RationalOpenSelectedHotBundleInputV3<'_>,
) -> Result<RationalOpenSelectedHotBundleV3> {
    if !matches!(
        input.action,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
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
    let bundle = RationalOpenSelectedHotBundleV3 {
        token_behavior_selection,
        account_profile,
        request_profile,
        lifecycle_policy,
        transition,
        strategy,
        effect,
        descriptor,
    };
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
        &bundle,
        input.authenticated_token_behavior,
    )?;
    Ok(bundle)
}

/// Independently hostile-decode and join every bundle artifact.
pub fn validate_rational_open_selected_hot_bundle_v3(
    bundle: &RationalOpenSelectedHotBundleV3,
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
        || account.fixed_account_count() != RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3
        || account.item_account_stride() != 0
        || account.common_scalar_count() != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?
        || account.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?
        || account.trusted_current_slot_scalar() != Some(narrow_u16(SCALAR_CURRENT_SLOT)?)
        || account.trusted_current_executing_program_identity()
            != Some(narrow_u16(ID_CURRENT_TRADING)?)
        || request
            .request_bytes(0)
            .map_err(Error::RequestProfileArtifact)?
            != REQUEST_BYTES
        || request.common_scalar_count() != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?
        || request.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?
        || transition.item_scalar_stride() != 0
        || transition.common_scalar_count() != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?
        || transition.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?
        || effect.fixed_account_count() != RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3
        || effect.item_account_stride() != 0
        || effect.common_scalar_count() != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?
        || effect.common_identity_count()
            != narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?
        || effect.route_count() != 1
        || route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_count() != RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3
        || route.fixed_account_start() != INJECTED_ACCOUNTS
    {
        return Err(Error::ArtifactGeometry);
    }
    Ok(())
}

/// Validate the complete bundle and bind its selected Token behavior to
/// independently authenticated Realm and release-set identities.
pub fn validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
    bundle: &RationalOpenSelectedHotBundleV3,
    authenticated: AuthenticatedTokenBehaviorV2,
) -> Result<()> {
    validate_rational_open_selected_hot_bundle_v3(bundle)?;
    if bundle.token_behavior_selection != authenticated.selection().to_bytes()
        || hash(&bundle.token_behavior_selection).to_bytes() != authenticated.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    Ok(())
}

fn encode_account_profile(input: RationalOpenSelectedHotBundleInputV3<'_>) -> Result<Vec<u8>> {
    if input.logical_data_lengths.len() != usize::from(RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3) {
        return Err(Error::AccountProfileInput);
    }
    let basis =
        ProductBasisV3::decode(input.product_basis).map_err(|_| Error::AccountProfileInput)?;
    if basis.kind() != BasisKindV3::CategoricalQ1 || basis.basis_width() < 2 {
        return Err(Error::AccountProfileInput);
    }
    let mut rules = Vec::with_capacity(input.logical_data_lengths.len());
    for index in 0..input.logical_data_lengths.len() {
        let writable = matches!(index, 0 | 16 | 17 | 28 | 37 | 38 | 39);
        let signer = index == 8;
        let executable = matches!(index, 6 | 19 | 23 | 26 | 27);
        let alias = match index {
            26 => AccountAliasInputV2::Fixed(19),
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
            4 => u32::try_from(BASIS_HEADER_BYTES_V3).map_err(|_| Error::AccountProfileInput)?,
            26 | 29 | 31 | 35 => 0,
            _ => *input
                .logical_data_lengths
                .get(index)
                .ok_or(Error::AccountProfileInput)?,
        };
        rules.push(AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(signer, writable, executable),
                effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                alias,
                data_length,
                data_item_stride: 0,
            },
            prestate,
        });
    }
    let operations = [AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(4),
        destination: ScalarCoordinateV2::common(narrow_u16(SCALAR_PRODUCT_OUTCOME_COUNT)?),
        data_offset: narrow_u32(BASIS_WIDTH_OFFSET_V3)?,
    }];
    let width = AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES
        + rules.len() * ACCOUNT_RULE_BYTES
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
        &rules,
        &[],
        &operations,
        &[],
        register_geometry()?,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::AccountProfileArtifact)?;
    Ok(output)
}

fn encode_request_profile(action: RepresentationActionV2) -> Result<Vec<u8>> {
    let mut fixed = Vec::with_capacity(REQUEST_INSTRUCTIONS);
    fixed.extend([
        RequestInstructionV1::require_u64(
            req(wire::REQUEST_MAGIC_OFFSET)?,
            u64::from_le_bytes(OPEN_REPRESENTATION_HOT_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            req(wire::REQUEST_VERSION_OFFSET)?,
            OPEN_REPRESENTATION_HOT_VERSION_V3,
        ),
        RequestInstructionV1::require_u8(req(wire::REQUEST_ACTION_OFFSET)?, action as u8),
        RequestInstructionV1::require_u8(
            req(wire::REQUEST_CALLER_ROLE_OFFSET)?,
            CallerRoleV2::Trading as u8,
        ),
        RequestInstructionV1::require_zero(req(wire::REQUEST_RESERVED_HEADER_OFFSET)?, 4),
        RequestInstructionV1::require_zero(req(wire::REQUEST_PARENT_CONTEXT_OFFSET)?, 32),
        RequestInstructionV1::require_zero(req(wire::REQUEST_RECEIPT_ACCOUNT_OFFSET)?, 32),
        RequestInstructionV1::require_zero(req(wire::REQUEST_REALM_OFFSET)?, 32),
        RequestInstructionV1::require_zero(req(wire::REQUEST_COLLATERAL_RECIPIENT_OFFSET)?, 32),
        RequestInstructionV1::require_u32(req(wire::REQUEST_ASSET_COUNT_OFFSET)?, 1),
        RequestInstructionV1::require_zero(req(wire::REQUEST_RESERVED_TAIL_OFFSET)?, 4),
    ]);
    for (offset, register) in [
        (wire::REQUEST_RELEASE_SET_OFFSET, ID_RELEASE),
        (wire::REQUEST_MARKET_OFFSET, ID_MARKET),
        (wire::REQUEST_GRAPH_ID_OFFSET, ID_GRAPH),
        (wire::REQUEST_DESCRIPTOR_ID_OFFSET, ID_DESCRIPTOR),
        (wire::REQUEST_ACTOR_OFFSET, ID_ACTOR),
        (wire::REQUEST_RECEIPT_MINT_OFFSET, ID_RECEIPT_MINT),
        (wire::REQUEST_REPRESENTATION_AUTHORITY_OFFSET, ID_AUTHORITY),
        (wire::REQUEST_TOKEN_PROGRAM_OFFSET, ID_TOKEN),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_SHARD_MINT_OFFSET,
            ID_SHARD_MINT,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_ACTOR_SHARD_ACCOUNT_OFFSET,
            ID_ACTOR_SHARDS,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET,
            ID_STRUCTURED_CUSTODY,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_CLAIMS_CUSTODY_OWNER_OFFSET,
            ID_CLAIMS_CUSTODY_OWNER,
        ),
    ] {
        fixed.push(RequestInstructionV1::project_identity(
            req(offset)?,
            idreg(register)?,
        ));
    }
    for (offset, register) in [
        (
            wire::REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET,
            SCALAR_REPRESENTATION_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET,
            SCALAR_CLAIMS_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET,
            SCALAR_ACTOR_POSITION_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET,
            SCALAR_CUSTODY_POSITION_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET,
            SCALAR_CUSTODY_REPLAY_REVISION,
        ),
        (wire::REQUEST_GENERATION_OFFSET, SCALAR_GENERATION),
        (wire::REQUEST_QUANTITY_OFFSET, SCALAR_QUANTITY),
        (wire::REQUEST_DENOMINATOR_OFFSET, SCALAR_DENOMINATOR),
        (
            wire::REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET,
            SCALAR_RECEIPT_SUPPLY,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_COEFFICIENT_OFFSET,
            SCALAR_COEFFICIENT,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_EXPECTED_SHARD_SUPPLY_OFFSET,
            SCALAR_SHARD_SUPPLY,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_EXPECTED_ACTOR_SHARDS_OFFSET,
            SCALAR_ACTOR_SHARDS,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET,
            SCALAR_STRUCTURED_SHARDS,
        ),
    ] {
        fixed.push(RequestInstructionV1::project_u64(
            req(offset)?,
            sreg(register)?,
        ));
    }
    for (offset, register) in [
        (wire::REQUEST_OUTCOME_COUNT_OFFSET, SCALAR_OUTCOME_COUNT),
        (
            wire::REQUEST_SELECTED_OUTCOME_OFFSET,
            SCALAR_SELECTED_OUTCOME,
        ),
        (wire::REQUEST_ASSET_COUNT_OFFSET, SCALAR_ASSET_COUNT),
    ] {
        fixed.push(RequestInstructionV1::project_u32(
            req(offset)?,
            sreg(register)?,
        ));
    }
    if fixed.len() != REQUEST_INSTRUCTIONS {
        return Err(Error::ArtifactGeometry);
    }
    let width = REQUEST_PROFILE_HEADER_BYTES + fixed.len() * REQUEST_OPERATION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_request_profile_v1_atomic(
        RequestGeometryV1::new(
            narrow_u32(REQUEST_BYTES)?,
            0,
            narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?,
            0,
            narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?,
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

fn encode_transition() -> Result<Vec<u8>> {
    let s = |index| ScalarRegisterV3::common(narrow_u16(index).unwrap_or_default());
    let instructions = [
        InstructionV3::scalar_eq(s(SCALAR_OUTCOME_COUNT), s(SCALAR_PRODUCT_OUTCOME_COUNT)),
        InstructionV3::scalar_lt(s(SCALAR_SELECTED_OUTCOME), s(SCALAR_PRODUCT_OUTCOME_COUNT)),
        InstructionV3::nonzero(s(SCALAR_QUANTITY)),
        InstructionV3::nonzero(s(SCALAR_DENOMINATOR)),
        InstructionV3::scalar_eq(s(SCALAR_COEFFICIENT), s(SCALAR_DENOMINATOR)),
    ];
    let width = TRANSITION_HEADER_BYTES + TRANSITION_INSTRUCTIONS * TRANSITION_INSTRUCTION_BYTES;
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_program_atomic(
        ProgramGeometryV3 {
            common_scalars: narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?,
            item_scalar_stride: 0,
            common_identities: narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?,
            item_identity_stride: 0,
        },
        &instructions,
        &[],
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::TransitionArtifact)?;
    Ok(output)
}

fn encode_effect(action: RepresentationActionV2) -> Result<Vec<u8>> {
    let mut template = vec![0_u8; REQUEST_BYTES];
    put(&mut template, wire::REQUEST_MAGIC_OFFSET, &REQUEST_MAGIC_V2)?;
    put(
        &mut template,
        wire::REQUEST_VERSION_OFFSET,
        &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
    )?;
    put(&mut template, wire::REQUEST_ACTION_OFFSET, &[action as u8])?;
    put(
        &mut template,
        wire::REQUEST_CALLER_ROLE_OFFSET,
        &[CallerRoleV2::Trading as u8],
    )?;
    let route = [RouteInputV3 {
        role: FixedRole::Claims,
        kind: RouteKindV3::Once,
        enable_common_scalar: None,
        witness_range_common_scalar: None,
        receipt_dependency: None,
        fixed_account_start: INJECTED_ACCOUNTS,
        fixed_account_count: RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3,
        item_account_start: 0,
        item_account_count: 0,
        fixed_request: &template,
        item_request: &[],
    }];
    let mut instructions = Vec::with_capacity(EFFECT_INSTRUCTIONS);
    for (offset, register) in [
        (wire::REQUEST_RELEASE_SET_OFFSET, ID_RELEASE),
        (wire::REQUEST_MARKET_OFFSET, ID_MARKET),
        (wire::REQUEST_GRAPH_ID_OFFSET, ID_GRAPH),
        (wire::REQUEST_DESCRIPTOR_ID_OFFSET, ID_DESCRIPTOR),
        (wire::REQUEST_PARENT_CONTEXT_OFFSET, ID_PARENT),
        (wire::REQUEST_ACTOR_OFFSET, ID_ACTOR),
        (wire::REQUEST_RECEIPT_MINT_OFFSET, ID_RECEIPT_MINT),
        (wire::REQUEST_REPRESENTATION_AUTHORITY_OFFSET, ID_AUTHORITY),
        (wire::REQUEST_TOKEN_PROGRAM_OFFSET, ID_TOKEN),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_SHARD_MINT_OFFSET,
            ID_SHARD_MINT,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_ACTOR_SHARD_ACCOUNT_OFFSET,
            ID_ACTOR_SHARDS,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET,
            ID_STRUCTURED_CUSTODY,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_CLAIMS_CUSTODY_OWNER_OFFSET,
            ID_CLAIMS_CUSTODY_OWNER,
        ),
    ] {
        instructions.push(EffectInstructionV3::write_request_identity(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            IdentityCoordinateV3::common(narrow_u16(register)?),
        ));
    }
    for (offset, register) in [
        (
            wire::REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET,
            SCALAR_REPRESENTATION_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET,
            SCALAR_CLAIMS_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET,
            SCALAR_ACTOR_POSITION_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET,
            SCALAR_CUSTODY_POSITION_REVISION,
        ),
        (
            wire::REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET,
            SCALAR_CUSTODY_REPLAY_REVISION,
        ),
        (wire::REQUEST_GENERATION_OFFSET, SCALAR_GENERATION),
        (wire::REQUEST_QUANTITY_OFFSET, SCALAR_QUANTITY),
        (wire::REQUEST_DENOMINATOR_OFFSET, SCALAR_DENOMINATOR),
        (
            wire::REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET,
            SCALAR_RECEIPT_SUPPLY,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_COEFFICIENT_OFFSET,
            SCALAR_COEFFICIENT,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_EXPECTED_SHARD_SUPPLY_OFFSET,
            SCALAR_SHARD_SUPPLY,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_EXPECTED_ACTOR_SHARDS_OFFSET,
            SCALAR_ACTOR_SHARDS,
        ),
        (
            wire::REQUEST_HEADER_BYTES_V2 + wire::ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET,
            SCALAR_STRUCTURED_SHARDS,
        ),
    ] {
        instructions.push(EffectInstructionV3::write_request_u64(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            ScalarCoordinateV3::common(narrow_u16(register)?),
        ));
    }
    for (offset, register) in [
        (wire::REQUEST_OUTCOME_COUNT_OFFSET, SCALAR_OUTCOME_COUNT),
        (
            wire::REQUEST_SELECTED_OUTCOME_OFFSET,
            SCALAR_SELECTED_OUTCOME,
        ),
        (wire::REQUEST_ASSET_COUNT_OFFSET, SCALAR_ASSET_COUNT),
    ] {
        instructions.push(EffectInstructionV3::write_request_u32(
            0,
            RequestSpaceV3::Fixed,
            narrow_u32(offset)?,
            ScalarCoordinateV3::common(narrow_u16(register)?),
        ));
    }
    if instructions.len() != EFFECT_INSTRUCTIONS {
        return Err(Error::ArtifactGeometry);
    }
    let width = EFFECT_HEADER_BYTES
        + EFFECT_ROUTE_BYTES
        + instructions.len() * EFFECT_OPERATION_BYTES
        + template.len();
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode_effect_program_v3_atomic(
        EffectGeometryV3 {
            fixed_accounts: RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3,
            item_account_stride: 0,
            common_scalars: narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?,
            item_scalar_stride: 0,
            common_identities: narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?,
            item_identity_stride: 0,
        },
        &route,
        &instructions,
        &[],
        &mut scratch,
        &mut output,
    )
    .map_err(Error::EffectArtifact)?;
    Ok(output)
}

fn register_geometry() -> Result<RegisterGeometryV2> {
    Ok(RegisterGeometryV2 {
        common_scalars: narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3)?,
        item_scalar_stride: 0,
        common_identities: narrow_u16(RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3)?,
        item_identity_stride: 0,
    })
}

fn req(offset: usize) -> Result<RequestCoordinateV1> {
    Ok(RequestCoordinateV1::fixed(narrow_u32(offset)?))
}

fn idreg(index: usize) -> Result<IdentityRegisterV1> {
    Ok(IdentityRegisterV1::common(narrow_u16(index)?))
}

fn sreg(index: usize) -> Result<ScalarRegisterV1> {
    Ok(ScalarRegisterV1::common(narrow_u16(index)?))
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

    fn basis(width: u32, product: u8) -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: id(product),
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
        .expect("categorical basis");
        output
    }

    fn lengths(basis: &[u8]) -> [u32; RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3 as usize] {
        let mut output = [0_u32; RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3 as usize];
        let width = u32::try_from(basis.len()).expect("basis width");
        *output.get_mut(4).expect("basis coordinate") = width;
        *output.get_mut(29).expect("basis alias") = width;
        output
    }

    fn input<'a>(
        action: RepresentationActionV2,
        basis: &'a [u8],
        lengths: &'a [u32],
    ) -> RationalOpenSelectedHotBundleInputV3<'a> {
        RationalOpenSelectedHotBundleInputV3 {
            action,
            logical_data_lengths: lengths,
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

    #[test]
    fn selected_actions_build_one_profile11_once_route() {
        let basis = basis(258, 1);
        let lengths = lengths(&basis);
        for action in [
            RepresentationActionV2::Denominate,
            RepresentationActionV2::Reconstitute,
        ] {
            let bundle =
                build_rational_open_selected_hot_bundle_v3(input(action, &basis, &lengths))
                    .expect("selected artifact bundle");
            validate_rational_open_selected_hot_bundle_v3(&bundle).expect("complete join");
            validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
                &bundle,
                input(action, &basis, &lengths).authenticated_token_behavior,
            )
            .expect("Realm/release join");

            let account = AccountProfileV2::decode(&bundle.account_profile).expect("profile");
            assert_eq!(
                account.rule(false, 4).expect("basis rule").prestate(),
                AccountPrestateV2::AdapterAuthenticatedVariableData
            );
            let alias = account.rule(false, 29).expect("basis child alias");
            assert_eq!(alias.prestate(), AccountPrestateV2::AuthenticatedRouteAlias);
            assert_eq!(alias.alias_index(), 4);
            assert_eq!(alias.effect_permissions(), 0);
            assert_eq!(account.logical_account_count(0), Ok(41));
            assert_eq!(account.physical_account_count(0), Ok(37));

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
            let (template, item) = effect.route_template(0).expect("route template");
            assert_eq!(item, []);
            assert_eq!(
                template
                    .get(wire::REQUEST_ACTION_OFFSET)
                    .copied()
                    .expect("action"),
                action as u8
            );
            assert_eq!(
                template
                    .get(
                        wire::REQUEST_PARENT_CONTEXT_OFFSET
                            ..wire::REQUEST_PARENT_CONTEXT_OFFSET + 32
                    )
                    .expect("parent"),
                [0_u8; 32]
            );
        }
    }

    #[test]
    fn artifact_or_semantic_substitution_refuses_without_partial_acceptance() {
        let canonical_basis = basis(258, 1);
        let canonical_lengths = lengths(&canonical_basis);
        let canonical = build_rational_open_selected_hot_bundle_v3(input(
            RepresentationActionV2::Denominate,
            &canonical_basis,
            &canonical_lengths,
        ))
        .expect("canonical");

        let mut substituted = canonical.clone();
        *substituted
            .account_profile
            .get_mut(0)
            .expect("account profile byte") ^= 1;
        assert!(validate_rational_open_selected_hot_bundle_v3(&substituted).is_err());

        let mut substituted = canonical.clone();
        *substituted
            .lifecycle_policy
            .get_mut(0)
            .expect("lifecycle byte") ^= 1;
        assert!(matches!(
            validate_rational_open_selected_hot_bundle_v3(&substituted),
            Err(Error::LifecycleArtifact(_))
        ));
        assert!(
            dclutch_capability_program_contract::v3::CapabilityProgramV3::decode(
                &canonical.descriptor
            )
            .is_err()
        );

        let mut substituted = canonical.clone();
        *substituted
            .token_behavior_selection
            .get_mut(80)
            .expect("behavior profile") ^= 1;
        assert_eq!(
            validate_rational_open_selected_hot_bundle_v3(&substituted),
            Err(Error::TokenBehavior(
                dclutch_token_svm::Error::InvalidAdapterRelease
            ))
        );

        assert_eq!(
            validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
                &canonical,
                crate::test_open_fixture_v3::authenticated_token_behavior_v3(
                    id(4),
                    id(17),
                    id(16),
                    258,
                ),
            ),
            Err(Error::ContentIdentity)
        );

        let other_basis = basis(258, 9);
        let other_lengths = lengths(&other_basis);
        let other = build_rational_open_selected_hot_bundle_v3(input(
            RepresentationActionV2::Denominate,
            &other_basis,
            &other_lengths,
        ))
        .expect("other Product");
        assert_eq!(canonical.account_profile, other.account_profile);

        assert_eq!(
            build_rational_open_selected_hot_bundle_v3(input(
                RepresentationActionV2::IssueStructured,
                &canonical_basis,
                &canonical_lengths,
            )),
            Err(Error::ArtifactGeometry)
        );
        let narrow = basis(1, 1);
        let narrow_lengths = lengths(&narrow);
        assert_eq!(
            build_rational_open_selected_hot_bundle_v3(input(
                RepresentationActionV2::Denominate,
                &narrow,
                &narrow_lengths,
            )),
            Err(Error::AccountProfileInput)
        );
    }
}
