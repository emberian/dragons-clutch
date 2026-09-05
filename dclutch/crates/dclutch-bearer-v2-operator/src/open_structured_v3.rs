//! Data-defined Hot artifacts for full-width Structured issue and unwrap.

use dclutch_vm::account_profile::lifecycle_v3::{
    CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as LIFECYCLE_SCHEMA_ID_V5, StateLifecyclePolicyV5,
};
use dclutch_vm::account_profile::v2::{
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
use dclutch_market::capability_program::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_vm::effect::{
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
use dclutch_market::execution_strategy::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_product::payoff::runtime_v3::{
    BASIS_HEADER_BYTES_V3, BASIS_WIDTH_OFFSET_V3, ProductBasisV3,
};
use dclutch_claims::rational::{
    ASSET_BYTES_V3, AuthenticatedTokenBehaviorV2, CallerRoleV2, OPEN_REPRESENTATION_HOT_MAGIC_V3,
    OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3, OPEN_REPRESENTATION_HOT_VERSION_V3,
    PHYSICAL_ABI_VERSION_V3, REQUEST_MAGIC_V2, REQUEST_STRUCTURED_HEADER_BYTES_V3,
    RepresentationActionV2,
};
use dclutch_claims::rational_kernel::RepresentationDescriptorV2;
use dclutch_claims::rational_request::generated as wire;
use dclutch_vm::request_profile::{
    HEADER_BYTES as REQUEST_PROFILE_HEADER_BYTES, MAX_BYTES as REQUEST_PROFILE_MAX_BYTES,
    OPERATION_BYTES as REQUEST_OPERATION_BYTES, RequestProfileV1,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_custody::token_svm::{
    TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use dclutch_vm::v3::{
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
pub const RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3: usize = 1;
/// Common request scalars, Product N, and trusted current slot before descriptor-K rows.
pub const RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3: usize = 8;
/// Per-coordinate descriptor-K scalar width, flattened into common registers.
pub const RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3: usize = 4;
/// Largest descriptor K admitted by the current exact RequestProfile V1 artifact.
///
/// **Derived, not chosen.** A profile is
/// `REQUEST_PROFILE_HEADER_BYTES + operations * REQUEST_OPERATION_BYTES` bytes,
/// and the canonical fixed projection costs
/// `RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3 +
/// RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3 * K`, so K is exactly
/// what the artifact bound leaves room for. It evaluates to 6 against today's
/// `REQUEST_PROFILE_MAX_BYTES_V1 = 1312` and moves by itself if any of the four
/// inputs moves. It was a hand-written `3` with the arithmetic living only in
/// this comment until 2026-08-31; a restated bound is a bound that drifts.
///
/// This ceiling is independent of Product result width `N` — see
/// `structured_actions_keep_descriptor_k_independent_from_product_n`, which
/// builds K = 3 against a 258-outcome Product.
///
/// **It stopped being the lower of the two walls on 2026-09-02.** It was 3
/// while the RequestProfile projected eight operations per coordinate, and the
/// note here said the packet capped full-width issuance at K = 2 -- one
/// coordinate BELOW this ceiling -- so widening the bound alone would admit
/// descriptors that could be published and denominated but never issued. The
/// lift it named was commit-don't-inline, and that is what physical ABI v3
/// did: three derived keys left the wire, taking the row cost from eight
/// operations to five and the base from 29 to 22, and the same arithmetic then
/// yields 6.
///
/// So the packet is now the binding wall in every frame rather than this
/// artifact. Full-width issuance is 1,005 bytes on the Claims-direct frame at
/// K = 3 and 1,149 at K = 5, against a 1,232-byte limit that K = 6 misses by
/// one byte once the house builder's unconditional `set_compute_unit_price` is
/// counted; on the Trading common-Hot route the same action is 1,197 and caps
/// at K = 3. A descriptor this ceiling admits can be issued at last.
/// See `CapabilityProgramAbi.finalizedRecordMaxBytes`.
pub const RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3: u32 =
    rational_open_structured_maximum_coordinates_v3();

/// Solve `BASE + ROW * K` operations against the RequestProfile byte bound.
///
/// The result is a `u32` because every consumer compares it against a `u32`
/// outcome or coordinate count. Truncation is asserted impossible rather than
/// assumed: the row count is at most the profile's whole operation budget,
/// `(MAX_BYTES - HEADER_BYTES) / OPERATION_BYTES`, which is a compile-time
/// constant far inside `u32`. A violation is a build failure, never a silent
/// runtime ceiling.
#[allow(clippy::cast_possible_truncation)]
const fn rational_open_structured_maximum_coordinates_v3() -> u32 {
    let operations =
        (REQUEST_PROFILE_MAX_BYTES - REQUEST_PROFILE_HEADER_BYTES) / REQUEST_OPERATION_BYTES;
    let rows = operations.saturating_sub(RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3)
        / RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3;
    assert!(
        rows <= u32::MAX as usize,
        "the RequestProfile coordinate ceiling must survive narrowing to the wire width"
    );
    rows as u32
}

/// A RequestProfile too narrow for one coordinate would make the wire unusable.
const _: () = assert!(RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3 >= 1);

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

// Physical ABI v3 sends one key per coordinate. The shard Mint, the Structured
// custody Account and the Claims custody owner are derived by the Claims
// adapter, so they are neither read from the parent request nor written into
// the child: these registers were a pipe between two wires that both lost the
// field, and the pipe goes with them.
const ITEM_ID_ACTOR_SHARDS: usize = 0;

const SCALAR_REPRESENTATION_REVISION: usize = 0;
const SCALAR_GENERATION: usize = 1;
const SCALAR_QUANTITY: usize = 2;
const SCALAR_DENOMINATOR: usize = 3;
const SCALAR_RECEIPT_SUPPLY: usize = 4;
const SCALAR_OUTCOME_COUNT: usize = 5;
// `assetCount` is derived from the action and the outcome count in v3, so it is
// on neither wire. The transition asserted `asset_count == outcome_count`; that
// equality is now structural and the register it compared is gone.
const SCALAR_PRODUCT_OUTCOME_COUNT: usize = 6;
const SCALAR_CURRENT_SLOT: usize = 7;

const ITEM_SCALAR_COEFFICIENT: usize = 0;
const ITEM_SCALAR_SHARD_SUPPLY: usize = 1;
const ITEM_SCALAR_ACTOR_SHARDS: usize = 2;
const ITEM_SCALAR_STRUCTURED_SHARDS: usize = 3;

/// Prefix projection operations in one structured RequestProfile, before rows.
///
/// Public because this pair, with `REQUEST_PROFILE_MAX_BYTES_V1`, IS the
/// executable width ceiling: a profile is `32 + operations * 24` bytes, so
/// `K = 3` is 53 operations and 1,304 bytes and `K = 4` is 61 and 1,496, which
/// the encoder refuses. A consumer that wants to state that cliff must import
/// the arithmetic rather than restate the numbers.
pub const RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3: usize = 22;
/// Projection operations one descriptor coordinate adds to the RequestProfile.
pub const RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3: usize = 5;

use RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3 as REQUEST_BASE_INSTRUCTIONS;
use RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3 as REQUEST_ROW_INSTRUCTIONS;
const TRANSITION_BASE_INSTRUCTIONS: usize = 3;
const TRANSITION_ROW_INSTRUCTIONS: usize = 1;
const EFFECT_BASE_INSTRUCTIONS: usize = 16;
const EFFECT_ROW_INSTRUCTIONS: usize = 5;

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
/// Pre-founding-safe inputs for one selected structured artifact bundle.
///
/// This is the same bundle the V3 builder emits, described without anything a
/// Market could reach. Where V3 takes a `RepresentationDescriptorV2` -- whose
/// identity is the SHA-256 of a preimage carrying the Core Market -- and an
/// `AuthenticatedTokenBehaviorV2` -- which cannot be constructed before that
/// descriptor exists -- this takes the representation WIDTH and the bare
/// `TokenBehaviorSelectionV2` those two were consulted for.
///
/// # Why this narrowing is mechanical rather than a redesign
///
/// The V3 builder never baked the descriptor: it read `outcome_count()` from
/// it and used the rest for runtime equality joins that refuse on mismatch.
/// That is measured rather than asserted --
/// `every_structured_artifact_is_byte_identical_across_two_markets` and
/// `the_whole_five_action_program_set_has_one_identity_across_two_markets` (in
/// `dclutch-structured-v2-operator`) compile complete bundles and the whole
/// five-action program set from closures differing only in the Market and
/// require every artifact byte, the `program_set_id` and the `config_id` to be
/// identical. Since the emitted bytes provably do not move with the Market,
/// removing the Market-bearing inputs cannot alter an artifact.
///
/// This type is therefore the SOLE AUTHOR of the bundle: the V3 entry point
/// performs its descriptor joins and then delegates here, so the two cannot
/// drift into two encoders of one artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalOpenStructuredSelectedBundleInputV6<'a> {
    /// IssueStructured or UnwrapStructured.
    pub action: RepresentationActionV2,
    /// Exact fixed logical account widths.
    pub fixed_data_lengths: &'a [u32],
    /// Exact per-coordinate item widths.
    pub item_data_lengths: [u32; 4],
    /// Exact Registry-authenticated ProductBasis body.
    pub product_basis: &'a [u8],
    /// Representation coordinate count `K`, supplied rather than read off a
    /// Market-bound descriptor.
    pub representation_outcome_count: u32,
    /// Immutable Realm/release selection, chosen before Market founding.
    pub token_behavior_selection: TokenBehaviorSelectionV2,
    /// Manifest-selected capability kind.
    pub kind: [u8; 32],
    /// Manifest-selected root-tail schema.
    pub root_schema: [u8; 32],
    /// Exact finalized lifecycle policy bytes.
    pub lifecycle_policy: &'a [u8],
    /// Manifest-selected capacity profile.
    pub capacity_profile: [u8; 32],
    /// Exact mutable root-tail width.
    pub root_state_bytes: u32,
}

/// Build one structured artifact bundle with no Market in scope.
pub fn build_rational_open_structured_selected_bundle_v6(
    input: RationalOpenStructuredSelectedBundleInputV6<'_>,
) -> Result<RationalOpenStructuredHotBundleV3> {
    if !matches!(
        input.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    ) {
        return Err(Error::ArtifactGeometry);
    }
    let basis = ProductBasisV3::decode(input.product_basis).map_err(Error::ProductBasis)?;
    let representation_width = usize::try_from(input.representation_outcome_count)
        .map_err(|_| Error::AccountProfileInput)?;
    if input.representation_outcome_count > RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3 {
        return Err(Error::CoordinateCeiling {
            requested: input.representation_outcome_count,
            ceiling: RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
        });
    }
    if input.representation_outcome_count == 0
        || input.fixed_data_lengths.len() != usize::from(RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3)
        || input.fixed_data_lengths.get(4).copied() != u32::try_from(input.product_basis.len()).ok()
        || input.fixed_data_lengths.get(29).copied()
            != u32::try_from(input.product_basis.len()).ok()
        || basis.basis_width() == 0
    {
        return Err(Error::AccountProfileInput);
    }
    structured_logical_accounts(representation_width)?;
    build_structured_bundle_inner(
        input.action,
        input.fixed_data_lengths,
        input.item_data_lengths,
        input.representation_outcome_count,
        input.token_behavior_selection,
        input.kind,
        input.root_schema,
        input.lifecycle_policy,
        input.capacity_profile,
        input.root_state_bytes,
    )
}

/// Build one structured artifact bundle from a Market-bound descriptor.
///
/// Identical output to [`build_rational_open_structured_selected_bundle_v6`],
/// which it delegates to. What this entry point adds is the descriptor and
/// Token-behavior admission joins: the width is READ from the descriptor rather
/// than supplied, and the descriptor's identity, release set and Token program
/// are required to agree with the authenticated behavior. Callers who hold a
/// finalized descriptor should keep using this; callers compiling a release
/// before the Market exists cannot, and want the V6 form.
pub fn build_rational_open_structured_hot_bundle_v3(
    input: RationalOpenStructuredHotBundleInputV3<'_>,
) -> Result<RationalOpenStructuredHotBundleV3> {
    if !matches!(
        input.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    ) {
        return Err(Error::ArtifactGeometry);
    }
    // The descriptor and the authenticated behavior are consulted HERE, for the
    // width and for the joins they exist to enforce, and then go no further:
    // the artifacts themselves are authored by the V6 builder below.
    let representation_outcome_count = require_representation_width(input)?;
    let selection = input.authenticated_token_behavior.selection();
    if hash(&selection.to_bytes()).to_bytes() != input.authenticated_token_behavior.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    build_structured_bundle_inner(
        input.action,
        input.fixed_data_lengths,
        input.item_data_lengths,
        representation_outcome_count,
        selection,
        input.kind,
        input.root_schema,
        input.lifecycle_policy,
        input.capacity_profile,
        input.root_state_bytes,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_structured_bundle_inner(
    action: RepresentationActionV2,
    fixed_data_lengths: &[u32],
    item_data_lengths: [u32; 4],
    representation_outcome_count: u32,
    token_behavior_selection: TokenBehaviorSelectionV2,
    kind: [u8; 32],
    root_schema: [u8; 32],
    lifecycle_policy_bytes: &[u8],
    capacity_profile: [u8; 32],
    root_state_bytes: u32,
) -> Result<RationalOpenStructuredHotBundleV3> {
    let input_action = action;
    let account_profile = encode_account_profile(
        fixed_data_lengths,
        item_data_lengths,
        representation_outcome_count,
    )?;
    let lifecycle_policy = Vec::from(lifecycle_policy_bytes);
    let request_profile = encode_request_profile(input_action, representation_outcome_count)?;
    let transition = encode_transition(representation_outcome_count)?;
    let effect = encode_effect(input_action, representation_outcome_count)?;
    let strategy_value = ExecutionStrategyProgramV2::new(
        StrategyDispositionV2::Interpreted,
        content(dclutch_vm::v3::SCHEMA_RELEASE_ID)?,
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
    let token_behavior_selection = token_behavior_selection.to_bytes();
    let lifecycle_id = digest(&lifecycle_policy)?;
    let descriptor = CapabilityProgramV4::new(
        content(kind)?,
        content(TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2)?,
        content(OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3)?,
        content(root_schema)?,
        lifecycle_id,
        content(capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: artifact(
                dclutch_vm::account_profile::v2::SCHEMA_RELEASE_ID,
                digest(&account_profile)?.to_bytes(),
            )?,
            request_profile: artifact(
                dclutch_vm::request_profile::SCHEMA_RELEASE_ID,
                digest(&request_profile)?.to_bytes(),
            )?,
            lifecycle: artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id.to_bytes())?,
            strategy: artifact(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                digest(&strategy)?.to_bytes(),
            )?,
            transition: artifact(
                dclutch_vm::v3::SCHEMA_RELEASE_ID,
                digest(&transition)?.to_bytes(),
            )?,
            effect: artifact(EFFECT_SCHEMA_ID_V4, digest(&effect)?.to_bytes())?,
        },
        root_state_bytes,
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
    // The Realm/release join the V3 entry point used to run here is now true by
    // construction -- the bundle's config IS the selection it was built from --
    // so what remains to check is the artifact geometry itself.
    validate_rational_open_structured_hot_bundle_v3(&bundle)?;
    Ok(bundle)
}

/// Independently hostile-decode and join every structured bundle artifact.
pub fn validate_rational_open_structured_hot_bundle_v3(
    bundle: &RationalOpenStructuredHotBundleV3,
) -> Result<()> {
    let representation_outcome_count = usize::try_from(bundle.representation_outcome_count)
        .map_err(|_| Error::ArtifactGeometry)?;
    if bundle.representation_outcome_count > RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3 {
        return Err(Error::CoordinateCeiling {
            requested: bundle.representation_outcome_count,
            ceiling: RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
        });
    }
    if bundle.representation_outcome_count == 0 {
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
                dclutch_vm::account_profile::v2::SCHEMA_RELEASE_ID,
                digest(&bundle.account_profile)?.to_bytes(),
            )?
        || descriptor.request_profile()
            != artifact(
                dclutch_vm::request_profile::SCHEMA_RELEASE_ID,
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
                dclutch_vm::v3::SCHEMA_RELEASE_ID,
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
    fixed_data_lengths: &[u32],
    item_data_lengths: [u32; 4],
    representation_outcome_count: u32,
) -> Result<Vec<u8>> {
    let representation_outcome_count =
        usize::try_from(representation_outcome_count).map_err(|_| Error::AccountProfileInput)?;
    let mut rules = Vec::with_capacity(structured_logical_accounts(representation_outcome_count)?);
    for index in 0..fixed_data_lengths.len() {
        let writable = matches!(index, 0 | 16 | 25 | 26);
        let signer = index == 8;
        // A route alias carries NO privileges of its own: `authenticate` takes
        // `representative_privileges` for any coordinate whose representative is
        // another (v2.rs:2360-2369), and cc228cdd made a nonzero privilege on an
        // alias a refusal because it is dead weight that reads as authority.
        // The aliased coordinate is the executable one.
        let executable = matches!(index, 6 | 15 | 19 | 21 | 23 | 27);
        let alias = match index {
            28 => AccountAliasInputV2::Fixed(19),
            29 => AccountAliasInputV2::Fixed(4),
            31 => AccountAliasInputV2::Fixed(2),
            35 => AccountAliasInputV2::Fixed(3),
            _ => AccountAliasInputV2::SelfCoordinate,
        };
        // 15 is the System program, and it is here for a measured reason. It is
        // the only coordinate this frame declares `executable` that was not
        // also declared opaque, so a release pinned its data length at exactly
        // zero -- and a builtin program account's data is its NAME, a cluster
        // fact no release can know. On the Agave 4.x line the account holds
        // `solana_system_program`, 21 bytes, which is what the account
        // projection kernel refused as `DataLengthMismatch` and what kept the
        // physical Trading common-Hot campaign from reaching submission. Every
        // other program in the frame was already opaque; this closes the set.
        let opaque = matches!(index, 6 | 7 | 15 | 19 | 20 | 21 | 23 | 24 | 25 | 26 | 27);
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
            _ => *fixed_data_lengths
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
        for (index, length) in item_data_lengths.iter().copied().enumerate() {
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
            req_fixed(wire::REQUEST_MAGIC_OFFSET_V3)?,
            u64::from_le_bytes(OPEN_REPRESENTATION_HOT_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            req_fixed(wire::REQUEST_VERSION_OFFSET_V3)?,
            OPEN_REPRESENTATION_HOT_VERSION_V3,
        ),
        RequestInstructionV1::require_u8(req_fixed(wire::REQUEST_ACTION_OFFSET_V3)?, action as u8),
        RequestInstructionV1::require_u8(
            req_fixed(wire::REQUEST_CALLER_ROLE_OFFSET_V3)?,
            CallerRoleV2::Trading as u8,
        ),
        RequestInstructionV1::require_zero(req_fixed(wire::REQUEST_RESERVED_HEADER_OFFSET_V3)?, 4),
        RequestInstructionV1::require_zero(req_fixed(wire::REQUEST_PARENT_CONTEXT_OFFSET_V3)?, 32),
        // The eight constraints that stood here asserted realm, the collateral
        // recipient, four absent revisions and a u32::MAX selected outcome. The
        // structured header class does not carry any of them in v3, so what
        // they checked is now unrepresentable rather than merely checked.
        RequestInstructionV1::require_zero(
            req_fixed(wire::STRUCTURED_REQUEST_RESERVED_TAIL_OFFSET_V3)?,
            4,
        ),
    ]);
    for (offset, register) in [
        (wire::REQUEST_RELEASE_SET_OFFSET_V3, ID_RELEASE),
        (wire::REQUEST_MARKET_OFFSET_V3, ID_MARKET),
        (wire::REQUEST_GRAPH_ID_OFFSET_V3, ID_GRAPH),
        (wire::REQUEST_DESCRIPTOR_ID_OFFSET_V3, ID_DESCRIPTOR),
        (wire::REQUEST_ACTOR_OFFSET_V3, ID_ACTOR),
        (wire::REQUEST_RECEIPT_MINT_OFFSET_V3, ID_RECEIPT_MINT),
        (
            wire::STRUCTURED_REQUEST_RECEIPT_ACCOUNT_OFFSET_V3,
            ID_RECEIPT_ACCOUNT,
        ),
        (
            wire::REQUEST_REPRESENTATION_AUTHORITY_OFFSET_V3,
            ID_AUTHORITY,
        ),
        (wire::REQUEST_TOKEN_PROGRAM_OFFSET_V3, ID_TOKEN),
    ] {
        fixed.push(RequestInstructionV1::project_identity(
            req_fixed(offset)?,
            id_common(register)?,
        ));
    }
    for (offset, register) in [
        (
            wire::REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
            SCALAR_REPRESENTATION_REVISION,
        ),
        (wire::REQUEST_GENERATION_OFFSET_V3, SCALAR_GENERATION),
        (wire::REQUEST_QUANTITY_OFFSET_V3, SCALAR_QUANTITY),
        (wire::REQUEST_DENOMINATOR_OFFSET_V3, SCALAR_DENOMINATOR),
        (
            wire::REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
            SCALAR_RECEIPT_SUPPLY,
        ),
    ] {
        fixed.push(RequestInstructionV1::project_u64(
            req_fixed(offset)?,
            scalar_common(register)?,
        ));
    }
    fixed.push(RequestInstructionV1::project_u32(
        req_fixed(wire::REQUEST_OUTCOME_COUNT_OFFSET_V3)?,
        scalar_common(SCALAR_OUTCOME_COUNT)?,
    ));
    for row in 0..representation_outcome_count {
        let row_offset = REQUEST_STRUCTURED_HEADER_BYTES_V3
            .checked_add(
                row.checked_mul(ASSET_BYTES_V3)
                    .ok_or(Error::ArtifactGeometry)?,
            )
            .ok_or(Error::ArtifactGeometry)?;
        {
            fixed.push(RequestInstructionV1::project_identity(
                req_fixed(
                    row_offset
                        .checked_add(wire::ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3)
                        .ok_or(Error::ArtifactGeometry)?,
                )?,
                id_common(row_identity(row, ITEM_ID_ACTOR_SHARDS)?)?,
            ));
        }
        for (offset, register) in [
            (wire::ASSET_COEFFICIENT_OFFSET_V3, ITEM_SCALAR_COEFFICIENT),
            (
                wire::ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3,
                ITEM_SCALAR_SHARD_SUPPLY,
            ),
            (
                wire::ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3,
                ITEM_SCALAR_ACTOR_SHARDS,
            ),
            (
                wire::ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3,
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
        // `scalar_eq(asset_count, outcome_count)` stood here. In v3 the decoder
        // DERIVES the asset count from the outcome count for a Structured
        // action, so the equality this asserted is what the encoding means.
        InstructionV3::nonzero(transition_common(SCALAR_OUTCOME_COUNT)?),
        InstructionV3::nonzero(transition_common(SCALAR_QUANTITY)?),
        InstructionV3::nonzero(transition_common(SCALAR_DENOMINATOR)?),
    ]);
    // AT MOST THE DENOMINATOR, per coordinate -- the one property both families
    // this wire carries actually share.
    //
    // The original `scalar_eq(coefficient[row], denominator)` forced EVERY
    // coordinate's weight to `D/D`. It refused the campaign's own descriptor
    // (`COEFFICIENTS = [2, 3, 5]` over `DENOMINATOR = 7`) at prelude operation
    // 4, register 9 against register 3, on real ELFs -- and it refused a Bearer
    // descriptor too, because a Bearer vector is `D * e_k` and its other `K-1`
    // coordinates are ZERO.
    //
    // `nonzero` was the first correction and it was half right: it admits the
    // fractional descriptor, and it refuses the Bearer one for the same reason
    // the equality did, at the zeros. This crate's own canonical fixture is a
    // Bearer vector -- `representation_descriptor_v3` writes `10` into
    // coefficient 0 and leaves coefficients 1 and 2 zero against `D = 10`
    // (`test_open_fixture_v3.rs:241-246`) -- so a per-row guard that refuses a
    // zero refuses the family this crate is named after.
    //
    // `coefficient <= D` holds for both: `D <= D` and `0 <= D` for a Bearer
    // vector, `2, 3, 5 <= 7` for a fractional one. And it is not vacuous: a
    // coordinate whose coefficient exceeds the denominator claims more than a
    // whole unit of the underlying, which is unbacked issuance.
    //
    // The basis-vector property itself is NOT per-row and is not expressible
    // here: it says exactly one coordinate is `D` and the rest are `0`, which a
    // per-row instruction cannot see. Its owner is
    // `BearerDescriptorV2::authenticate`.
    for row in 0..representation_outcome_count {
        prelude.push(InstructionV3::scalar_le(
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
        wire::REQUEST_MAGIC_OFFSET_V3,
        &REQUEST_MAGIC_V2,
    )?;
    put(
        &mut fixed_template,
        wire::REQUEST_VERSION_OFFSET_V3,
        &PHYSICAL_ABI_VERSION_V3.to_le_bytes(),
    )?;
    put(
        &mut fixed_template,
        wire::REQUEST_ACTION_OFFSET_V3,
        &[action as u8],
    )?;
    put(
        &mut fixed_template,
        wire::REQUEST_CALLER_ROLE_OFFSET_V3,
        &[CallerRoleV2::Trading as u8],
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
        (wire::REQUEST_PARENT_CONTEXT_OFFSET_V3, ID_PARENT),
        (wire::REQUEST_RELEASE_SET_OFFSET_V3, ID_RELEASE),
        (wire::REQUEST_MARKET_OFFSET_V3, ID_MARKET),
        (wire::REQUEST_GRAPH_ID_OFFSET_V3, ID_GRAPH),
        (wire::REQUEST_DESCRIPTOR_ID_OFFSET_V3, ID_DESCRIPTOR),
        (wire::REQUEST_ACTOR_OFFSET_V3, ID_ACTOR),
        (wire::REQUEST_RECEIPT_MINT_OFFSET_V3, ID_RECEIPT_MINT),
        (
            wire::STRUCTURED_REQUEST_RECEIPT_ACCOUNT_OFFSET_V3,
            ID_RECEIPT_ACCOUNT,
        ),
        (
            wire::REQUEST_REPRESENTATION_AUTHORITY_OFFSET_V3,
            ID_AUTHORITY,
        ),
        (wire::REQUEST_TOKEN_PROGRAM_OFFSET_V3, ID_TOKEN),
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
            wire::REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
            SCALAR_REPRESENTATION_REVISION,
        ),
        (wire::REQUEST_GENERATION_OFFSET_V3, SCALAR_GENERATION),
        (wire::REQUEST_QUANTITY_OFFSET_V3, SCALAR_QUANTITY),
        (wire::REQUEST_DENOMINATOR_OFFSET_V3, SCALAR_DENOMINATOR),
        (
            wire::REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
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
    fixed.push(EffectInstructionV3::write_request_u32(
        0,
        RequestSpaceV3::Fixed,
        narrow_u32(wire::REQUEST_OUTCOME_COUNT_OFFSET_V3)?,
        effect_scalar_common(SCALAR_OUTCOME_COUNT)?,
    ));
    for row in 0..representation_outcome_count {
        let row_offset = REQUEST_STRUCTURED_HEADER_BYTES_V3
            .checked_add(
                row.checked_mul(ASSET_BYTES_V3)
                    .ok_or(Error::ArtifactGeometry)?,
            )
            .ok_or(Error::ArtifactGeometry)?;
        {
            fixed.push(EffectInstructionV3::write_request_identity(
                0,
                RequestSpaceV3::Fixed,
                narrow_u32(
                    row_offset
                        .checked_add(wire::ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3)
                        .ok_or(Error::ArtifactGeometry)?,
                )?,
                effect_id_common(row_identity(row, ITEM_ID_ACTOR_SHARDS)?)?,
            ));
        }
        for (offset, register) in [
            (wire::ASSET_COEFFICIENT_OFFSET_V3, ITEM_SCALAR_COEFFICIENT),
            (
                wire::ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3,
                ITEM_SCALAR_SHARD_SUPPLY,
            ),
            (
                wire::ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3,
                ITEM_SCALAR_ACTOR_SHARDS,
            ),
            (
                wire::ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3,
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
        .checked_mul(ASSET_BYTES_V3)
        .and_then(|tail| REQUEST_STRUCTURED_HEADER_BYTES_V3.checked_add(tail))
        .ok_or(Error::ArtifactGeometry)
}

fn require_representation_width(input: RationalOpenStructuredHotBundleInputV3<'_>) -> Result<u32> {
    let basis = ProductBasisV3::decode(input.product_basis).map_err(Error::ProductBasis)?;
    let representation_outcome_count = input.representation_descriptor.outcome_count();
    let representation_width =
        usize::try_from(representation_outcome_count).map_err(|_| Error::AccountProfileInput)?;
    if representation_outcome_count > RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3 {
        return Err(Error::CoordinateCeiling {
            requested: representation_outcome_count,
            ceiling: RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
        });
    }
    if representation_outcome_count == 0
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
    use dclutch_claims::composition_v3::{
        ClaimsCompositionErrorV3, ClaimsCompositionParentV3, ClaimsCompositionV3,
    };
    use dclutch_product::payoff::runtime_v3::{
        BasisInputV3, BasisKindV3, compile_basis_v3,
    };
    use dclutch_claims::rational::ABSENT_REVISION;
    use dclutch_claims::rational::{
        AssetV2, RepresentationRequestHeaderV2, RepresentationRequestV2,
    };
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;

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
                // Exempt by proof: degree 0 and 1 need no price gate,
                // and a digest offered alongside one is refused.
                price_gate_certificate_digest: [0_u8; 32],
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
            for coordinate in [6_u16, 7, 15, 19, 20, 21, 23, 24, 25, 26, 27, 38, 39, 40] {
                assert_eq!(
                    account.rule(false, coordinate).expect("opaque").prestate(),
                    AccountPrestateV2::AuthenticatedOpaqueReadonlyData
                );
            }
            let effect = EffectProgramV4::decode(&bundle.effect).expect("effect");
            let effect = effect.base();
            // Every width here is read from the constant that defines it, with
            // the fixture's K = 3 as the only literal. Restated numbers are how
            // this assertion went stale: physical ABI v3 dropped four absent
            // revisions, the selected outcome and the derived asset count from
            // the structured class, and dropped three of the four per-coordinate
            // identity keys -- the shard Mint, the Structured custody Account
            // and the Claims custody owner, all now derived by the Claims
            // adapter. The scalar bank shrank 9 -> 8 and the per-row identity
            // stride 4 -> 1, and both literals stayed behind. The accounts did
            // NOT shrink, which is the point: the three keys stopped riding the
            // wire, they did not stop existing on chain.
            assert_eq!(
                effect.account_count(258).expect("account width"),
                usize::from(
                    RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3
                        + RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3 * 3
                )
            );
            assert_eq!(
                effect.scalar_count(258).expect("scalar width"),
                RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3
                    + RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3 * 3
            );
            assert_eq!(
                effect.identity_count(258).expect("identity width"),
                RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3
                    + RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3 * 3
            );
            let (fixed, item) = effect.route_template(0).expect("templates");
            assert_eq!(
                fixed.len(),
                REQUEST_STRUCTURED_HEADER_BYTES_V3 + 3 * ASSET_BYTES_V3
            );
            assert!(item.is_empty());
            assert_eq!(
                fixed
                    .get(wire::REQUEST_ACTION_OFFSET_V3)
                    .copied()
                    .expect("action"),
                action as u8
            );
        }
    }

    /// A route alias states no privilege, and its representative still states
    /// the one the runtime enforces.
    ///
    /// `cc228cdd` made a nonzero privilege on an `AuthenticatedRouteAlias` a
    /// refusal, because `authenticate` takes `representative_privileges` for any
    /// coordinate that aliases another (`v2.rs:2360-2369`) and never the alias's
    /// own field. That sweep fixed the Direct producer and stopped there; this
    /// crate's three emitters kept marking their Claims/Token-program aliases
    /// `executable` and were refused at encode from 2026-08-26 until the
    /// STRUCT-CAMP lane found them, which is why this test is a PROPERTY of the
    /// emitted artifact rather than one more fixture.
    ///
    /// The second half is the one that matters: removing the bit must not have
    /// removed the authority. Coordinate 28 aliases 19, and 19 is still
    /// executable, so the runtime still requires an executable account there.
    #[test]
    fn a_route_alias_states_no_privilege_and_its_representative_states_it_instead() {
        let basis = basis(258);
        let lengths = lengths(&basis);
        let bundle = build_rational_open_structured_hot_bundle_v3(input(
            RepresentationActionV2::IssueStructured,
            &basis,
            &lengths,
        ))
        .expect("structured bundle");
        let profile = AccountProfileV2::decode(&bundle.account_profile).expect("profile13");
        let logical = profile
            .logical_account_count_with_dynamic_spans(258, &[])
            .expect("logical width");
        let mut aliases = 0_usize;
        for coordinate in 0..logical {
            let representative = profile
                .representative_with_dynamic_spans(258, &[], coordinate)
                .expect("representative");
            if representative == coordinate {
                continue;
            }
            aliases += 1;
            let rule = profile
                .rule(false, u16::try_from(coordinate).expect("coordinate"))
                .expect("alias rule");
            assert_eq!(
                rule.prestate(),
                AccountPrestateV2::AuthenticatedRouteAlias,
                "coordinate {coordinate} aliases {representative} without saying so"
            );
            assert_eq!(
                rule.privileges(),
                0,
                "coordinate {coordinate} restates a privilege the runtime reads from {representative}"
            );
        }
        assert_eq!(aliases, 4);
        // The authority did not move: it was removed from the coordinate that
        // was being ignored, and stays on the one that is read.
        assert!(
            profile
                .rule(false, 19)
                .expect("Claims program rule")
                .route_privileges()
                .executable()
        );
        assert!(
            !profile
                .rule(false, 28)
                .expect("Claims placeholder rule")
                .route_privileges()
                .executable()
        );
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

    /// THE CHECK THAT CROSSES THE BOUNDARY.
    ///
    /// This operator declares the Structured route's account geometry and
    /// `dclutch-claims`'s composition admits it, and until this test the
    /// two had never been compared in one process. Every check confined to one
    /// side passed while they disagreed, and the disagreement surfaced on real
    /// ELFs as one `TradingSbfError::Content` out of thousands of sites.
    ///
    /// `N` is deliberately 258 against `K = 3`. The Structured family's whole
    /// point is that the representation width is not the Product result width
    /// (`structured_market.rs:26-35`), and a fixture that sets them equal
    /// cannot tell a request-bound check from a tail-bound one.
    #[test]
    fn the_claims_composition_admits_the_full_width_route_this_operator_emits() {
        let basis = basis(258);
        let lengths = lengths(&basis);
        let bundle = build_rational_open_structured_hot_bundle_v3(input(
            RepresentationActionV2::IssueStructured,
            &basis,
            &lengths,
        ))
        .expect("structured bundle");
        let program = EffectProgramV4::decode(&bundle.effect).expect("effect");
        let effect = program.base();
        let tail_count = 258_u32;
        let request = full_width_issue_request(3);
        let mut bank = vec![0_u8; effect.request_bytes(tail_count).expect("request bank")];
        assert_eq!(bank.len(), request.len(), "the route's request is the bank");
        bank.copy_from_slice(&request);
        let scalars = vec![0_u64; effect.scalar_count(tail_count).expect("scalars")];
        let identities = vec![[0_u8; 32]; effect.identity_count(tail_count).expect("identities")];
        let composition = ClaimsCompositionV3::decode_selected(
            effect,
            tail_count,
            &scalars,
            &identities,
            &bank,
            composition_parent(),
        )
        .expect("the Claims composition must admit the route this operator emits");
        assert_eq!(composition.mutation_route(), 0);
        assert_eq!(
            composition
                .rational_representation()
                .map(|request| request.header().asset_count),
            Some(3),
        );
    }

    /// The admission above is REQUEST-BOUND, which is the property that makes
    /// the operator's release-time span honest rather than merely lucky.
    ///
    /// The route declares `CLAIMS_FIXED_ACCOUNTS + K * ITEM` accounts from a
    /// constant baked at release. That constant binds nothing by itself -- the
    /// scholar's charge, and it is correct. What binds is the consumer: the
    /// composition recomputes the span from the REQUEST's own `asset_count`
    /// (`RepresentationFrameSpecV2::account_count`), so a release whose
    /// declared width does not match the request it carries is refused. A
    /// two-coordinate request under a three-coordinate release is that case.
    #[test]
    fn a_request_narrower_than_the_declared_span_is_refused_by_the_composition() {
        let basis = basis(258);
        let lengths = lengths(&basis);
        let bundle = build_rational_open_structured_hot_bundle_v3(input(
            RepresentationActionV2::IssueStructured,
            &basis,
            &lengths,
        ))
        .expect("structured bundle");
        let program = EffectProgramV4::decode(&bundle.effect).expect("effect");
        let effect = program.base();
        let tail_count = 258_u32;
        let narrow = full_width_issue_request(2);
        let mut bank = vec![0_u8; effect.request_bytes(tail_count).expect("request bank")];
        assert!(narrow.len() < bank.len());
        bank.get_mut(..narrow.len())
            .expect("narrow prefix")
            .copy_from_slice(&narrow);
        let scalars = vec![0_u64; effect.scalar_count(tail_count).expect("scalars")];
        let identities = vec![[0_u8; 32]; effect.identity_count(tail_count).expect("identities")];
        assert_eq!(
            ClaimsCompositionV3::decode_selected(
                effect,
                tail_count,
                &scalars,
                &identities,
                &bank,
                composition_parent(),
            )
            .err(),
            Some(ClaimsCompositionErrorV3::Route),
        );
    }

    fn composition_parent() -> ClaimsCompositionParentV3 {
        ClaimsCompositionParentV3 {
            release_set: id(0x51),
            market: id(0x52),
            generation: 14,
            parent_request_digest: id(0x53),
        }
    }

    fn full_width_issue_request(assets: u32) -> Vec<u8> {
        let mut rows = vec![0_u8; usize::try_from(assets).expect("width") * ASSET_BYTES_V3];
        for row in 0..assets {
            let seed = u8::try_from(row).expect("small width");
            let index = usize::try_from(row).expect("small width");
            AssetV2 {
                shard_mint: id(0x60 + seed),
                actor_shard_account: id(0x70 + seed),
                structured_custody_account: id(0x80 + seed),
                claims_custody_owner: id(0x90 + seed),
                coefficient: u64::from(row) + 2,
                expected_shard_supply: 0,
                expected_actor_shards: 0,
                expected_structured_shards: 0,
            }
            .encode_into(
                rows.get_mut(index * ASSET_BYTES_V3..(index + 1) * ASSET_BYTES_V3)
                    .expect("asset row"),
            )
            .expect("asset");
        }
        let parent = composition_parent();
        let request = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::IssueStructured,
                caller_role: CallerRoleV2::Trading,
                release_set: parent.release_set,
                market: parent.market,
                graph_id: id(0x54),
                descriptor_id: id(0x55),
                parent_context: parent.parent_request_digest,
                actor: id(0x56),
                receipt_mint: id(0x57),
                receipt_account: id(0x58),
                representation_authority: id(0x59),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: [0; 32],
                collateral_recipient: [0; 32],
                expected_representation_revision: 0,
                expected_claims_market_revision: ABSENT_REVISION,
                expected_actor_position_revision: ABSENT_REVISION,
                expected_custody_position_revision: ABSENT_REVISION,
                expected_custody_replay_revision: ABSENT_REVISION,
                generation: parent.generation,
                quantity: 1,
                denominator: 10,
                expected_receipt_supply: 0,
                outcome_count: assets,
                selected_outcome: u32::MAX,
                asset_count: assets,
            },
            &rows,
        )
        .expect("full-width request");
        let mut bytes = vec![0_u8; REQUEST_STRUCTURED_HEADER_BYTES_V3 + rows.len()];
        request.encode_into(&mut bytes).expect("request bytes");
        bytes
    }

    /// THE PER-ROW COEFFICIENT GUARD, EXECUTED -- against both families the
    /// full-width wire carries, and against the shape it must still refuse.
    ///
    /// A guard is not checked by reading it. This runs the emitted TransitionVM
    /// program through the same public `execute_fold_atomic` the runtime uses,
    /// over three register banks that differ only in the coefficients:
    ///
    /// - the Bearer vector `[10, 0, 0]` over `D = 10`, which is this crate's own
    ///   canonical descriptor and which BOTH earlier spellings of this guard
    ///   refused -- `scalar_eq` at the `D`s that are zero, `nonzero` at the same
    ///   zeros;
    /// - the campaign's fractional `[2, 3, 5]` over `D = 7`, which the original
    ///   `scalar_eq` refused on real ELFs;
    /// - an over-claiming `[8, 3, 5]` over `D = 7`, where one coordinate claims
    ///   more than a whole unit of the underlying. That is unbacked issuance and
    ///   it is still refused, which is what keeps this a guard rather than a
    ///   deletion.
    #[test]
    fn the_row_coefficient_guard_admits_both_families_and_refuses_over_claiming() {
        use dclutch_vm::v3::{
            ProgramV3 as TransitionProgram, RegisterInput, RegisterOutput, execute_fold_atomic,
        };

        let bytes = encode_transition(3).expect("transition");
        let program = TransitionProgram::decode(&bytes).expect("transition program");
        let scalars = structured_common_scalars(3).expect("scalar width");
        let identities = structured_common_identities(3).expect("identity width");

        let run = |denominator: u64, coefficients: [u64; 3]| {
            let mut bank = vec![0_u64; scalars];
            *bank.get_mut(SCALAR_QUANTITY).expect("quantity") = 1;
            *bank.get_mut(SCALAR_DENOMINATOR).expect("denominator") = denominator;
            *bank.get_mut(SCALAR_OUTCOME_COUNT).expect("outcome count") = 3;
            for (row, coefficient) in coefficients.into_iter().enumerate() {
                let register = row_scalar(row, ITEM_SCALAR_COEFFICIENT).expect("row scalar");
                *bank.get_mut(register).expect("coefficient") = coefficient;
            }
            let ids = vec![[0_u8; 32]; identities];
            let mut scratch_scalars = bank.clone();
            let mut scratch_ids = ids.clone();
            let mut output_scalars = bank.clone();
            let mut output_ids = ids.clone();
            execute_fold_atomic(
                program,
                0,
                RegisterInput {
                    scalars: &bank,
                    identities: &ids,
                },
                RegisterOutput {
                    scalars: &mut scratch_scalars,
                    identities: &mut scratch_ids,
                },
                RegisterOutput {
                    scalars: &mut output_scalars,
                    identities: &mut output_ids,
                },
            )
        };

        assert_eq!(run(10, [10, 0, 0]), Ok(()), "the Bearer basis vector");
        assert_eq!(
            run(7, [2, 3, 5]),
            Ok(()),
            "the campaign's fractional vector"
        );
        assert_eq!(
            run(7, [8, 3, 5]),
            Err(dclutch_vm::v3::Error::CheckFailed),
            "a coordinate may not claim more than one whole unit",
        );
    }
}
