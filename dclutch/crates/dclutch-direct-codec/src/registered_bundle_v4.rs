//! Complete schema-bound Hot artifact bundles for registered Direct admission.
//!
//! Each bundle is selected by a normal CapabilityProgramV4 descriptor. Trading
//! remains family-neutral: Profile14 authenticates the fixed account frame,
//! LifecycleV5 owns maker/record first use and current Rent quotes, Transition
//! derives the reserve, and EffectV4 settles.
//!
//! The two sides share every SCHEMA in the descriptor and not one artifact
//! digest. A Buy escrows collateral and its Effect executes the ordered Custody
//! chain over a fifty-five-account frame; a Sell escrows claims, which the
//! record itself accounts for, so its Effect invokes no child at all over a
//! thirteen-account one. Everything that is genuinely common is factored --
//! `creation_descriptor`, `validate_creation_descriptor`,
//! `validate_creation_geometry` -- and everything that is not is stated per
//! side, because `e03a51fd` ruled that a Sell must REBASE on the Buy rather
//! than copy it.

use dclutch_account_profile_contract::{
    lifecycle_v3::StateLifecyclePolicyV5,
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2},
};
use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
    SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::ProgramV3 as EffectProgramV3,
    v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4},
};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::{
    RequestProfileV1,
    v2::{REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, RequestProfileV2},
};
use dclutch_sha256_adapter::digest;
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;

use crate::{
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3, DIRECT_REGISTRATION_REQUEST_BYTES_V3,
        DIRECT_SUCCESSOR_KIND_ID_V3, DirectExecutionActionV3,
    },
    registered_account_artifacts_v4::{
        DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4, DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4,
        DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4, DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4,
        DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4, DIRECT_REGISTER_SELL_ACCOUNT_PROFILE_BYTES_V4,
        DIRECT_REGISTER_SELL_FIXED_ACCOUNTS_V4, DirectRegisterBuyAccountProfileInputV4,
        DirectRegisteredCreationAccountProfileInputV4,
        encode_direct_register_buy_account_profile_v4_atomic,
        encode_direct_register_sell_account_profile_v4_atomic,
    },
    registered_creation_artifacts_v4::{
        DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4,
        DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4,
        DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4,
        DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4,
        DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4,
        DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4,
        DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4, direct_registered_creation_strategy_v4,
        encode_direct_registered_creation_request_profile_v4_atomic,
        encode_direct_registered_creation_transition_v4_atomic,
    },
    registered_effect_artifacts_v4::{
        DIRECT_REGISTER_BUY_EFFECT_BYTES_V4, DIRECT_REGISTER_SELL_EFFECT_BYTES_V4,
        encode_direct_register_buy_effect_v4_atomic, encode_direct_register_sell_effect_v4_atomic,
    },
    registered_state_artifacts_v4::{
        DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5, DirectRegisteredCreationChildRentWidthsV4,
        encode_direct_registered_creation_unified_lifecycle_v5_atomic,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
    },
};

/// Exact interpreted ExecutionStrategy record width.
pub const DIRECT_REGISTER_BUY_STRATEGY_BYTES_V4: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityProgramV4 descriptor width.
pub const DIRECT_REGISTER_BUY_DESCRIPTOR_BYTES_V4: usize = CAPABILITY_PROGRAM_V4_BYTES;

/// Chain-selected inputs not owned by the Direct artifact family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisterBuyHotBundleInputV4<'a> {
    /// Exact logical account observations in Profile14 coordinate order.
    pub account_profile: DirectRegisterBuyAccountProfileInputV4<'a>,
    /// Exact observed widths of the Custody children the Effect opens.
    pub child_rent_widths: DirectRegisteredCreationChildRentWidthsV4,
    /// Manifest-selected physical capacity profile content identity.
    pub capacity_profile: [u8; 32],
}

/// Every finalized artifact selected by RegisterBuy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisterBuyHotBundleV4 {
    /// Fixed-topology Profile14 bytes.
    pub account_profile: [u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4],
    /// Maker/record LifecycleV5 bytes.
    pub lifecycle_policy: [u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5],
    /// One-maker signed RequestProfileV2 bytes.
    pub request_profile: [u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4],
    /// RegisterBuy TransitionVMV3 bytes.
    pub transition: [u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4],
    /// Interpreted strategy selecting the transition.
    pub strategy: [u8; DIRECT_REGISTER_BUY_STRATEGY_BYTES_V4],
    /// Ordered Custody EffectV4 bytes.
    pub effect: [u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4],
    /// CapabilityProgramV4 joining every artifact above.
    pub descriptor: [u8; DIRECT_REGISTER_BUY_DESCRIPTOR_BYTES_V4],
}

/// Exact interpreted ExecutionStrategy record width for RegisterSell.
pub const DIRECT_REGISTER_SELL_STRATEGY_BYTES_V4: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityProgramV4 descriptor width for RegisterSell.
pub const DIRECT_REGISTER_SELL_DESCRIPTOR_BYTES_V4: usize = CAPABILITY_PROGRAM_V4_BYTES;

/// Chain-selected inputs not owned by the Direct artifact family.
///
/// There is no `child_rent_widths` counterpart: a Sell opens no Custody child,
/// so its LifecycleV5 quotes only the maker replay and the registered record,
/// both of which are widths this crate knows exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisterSellHotBundleInputV4<'a> {
    /// Exact logical account observations in Profile14 coordinate order.
    pub account_profile: DirectRegisteredCreationAccountProfileInputV4<'a>,
    /// Exact observed widths of the Custody children a registered BUY opens.
    ///
    /// A Sell opens none of them, and this is still required, because the
    /// lifecycle policy is a property of the ROOT rather than of one action: one
    /// entry pins one policy, so the policy names both sides' quotes and cannot
    /// be built without both sides' widths. The Sell's own projection still
    /// never reads them -- they are tagged `RegisterBuy` and
    /// `action_current_rent_quote_count(RegisterSell)` is two.
    pub child_rent_widths: DirectRegisteredCreationChildRentWidthsV4,
    /// Manifest-selected physical capacity profile content identity.
    pub capacity_profile: [u8; 32],
}

/// Every finalized artifact selected by RegisterSell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRegisterSellHotBundleV4 {
    /// Fixed-topology Profile14 bytes.
    pub account_profile: [u8; DIRECT_REGISTER_SELL_ACCOUNT_PROFILE_BYTES_V4],
    /// Maker/record LifecycleV5 bytes.
    pub lifecycle_policy: [u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5],
    /// One-maker signed RequestProfileV2 bytes.
    pub request_profile: [u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4],
    /// RegisterSell TransitionVMV3 bytes.
    pub transition: [u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4],
    /// Interpreted strategy selecting the transition.
    pub strategy: [u8; DIRECT_REGISTER_SELL_STRATEGY_BYTES_V4],
    /// Routeless local-state EffectV4 bytes.
    pub effect: [u8; DIRECT_REGISTER_SELL_EFFECT_BYTES_V4],
    /// CapabilityProgramV4 joining every artifact above.
    pub descriptor: [u8; DIRECT_REGISTER_SELL_DESCRIPTOR_BYTES_V4],
}

/// Stable registered-creation bundle refusal, shared by both sides.
///
/// One enum, because the two sides fail in the same seven ways and a caller
/// that handles a Buy's refusal handles a Sell's. It was named for the Buy when
/// the Buy was the only side; the name moved with the second side rather than
/// the second side inheriting a name that was no longer true.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisteredCreationHotBundleErrorV4 {
    /// A selected content identity or fixed geometry was invalid.
    Content,
    /// AccountProfile construction or decoding refused.
    AccountProfile,
    /// Lifecycle construction, decoding, or AccountProfile join refused.
    Lifecycle,
    /// RequestProfile construction or decoding refused.
    RequestProfile,
    /// Transition construction or decoding refused.
    Transition,
    /// Strategy construction or transition join refused.
    Strategy,
    /// Effect construction, decoding, or route geometry refused.
    Effect,
    /// Descriptor construction or exact content join refused.
    Descriptor,
    /// Account/register/request geometries differed.
    Geometry,
}

/// Emit and independently hostile-check one complete RegisterBuy Hot bundle.
pub fn build_direct_register_buy_hot_bundle_v4(
    input: DirectRegisterBuyHotBundleInputV4<'_>,
) -> Result<DirectRegisterBuyHotBundleV4, DirectRegisteredCreationHotBundleErrorV4> {
    let action = DirectExecutionActionV3::RegisterBuy;
    let mut account_scratch = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
    let mut account_profile = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
    encode_direct_register_buy_account_profile_v4_atomic(
        input.account_profile,
        &mut account_scratch,
        &mut account_profile,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::AccountProfile)?;

    let mut lifecycle_scratch = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
    let mut lifecycle_policy = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
    encode_direct_registered_creation_unified_lifecycle_v5_atomic(
        input.child_rent_widths,
        &mut lifecycle_scratch,
        &mut lifecycle_policy,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?;

    let mut request_v1_scratch = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4];
    let mut request_v1 = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4];
    let mut request_v2_scratch = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4];
    let mut request_profile = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4];
    encode_direct_registered_creation_request_profile_v4_atomic(
        action,
        &mut request_v1_scratch,
        &mut request_v1,
        &mut request_v2_scratch,
        &mut request_profile,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::RequestProfile)?;

    let mut transition_scratch = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
    let mut transition = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
    encode_direct_registered_creation_transition_v4_atomic(
        action,
        &mut transition_scratch,
        &mut transition,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Transition)?;
    let transition_id = digest(&transition);
    let strategy = direct_registered_creation_strategy_v4(transition_id)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Strategy)?;

    let mut effect_scratch = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
    let mut effect = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
    encode_direct_register_buy_effect_v4_atomic(&mut effect_scratch, &mut effect)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Effect)?;

    let descriptor = creation_descriptor(
        digest(&account_profile),
        digest(&lifecycle_policy),
        digest(&request_profile),
        transition_id,
        digest(&strategy),
        digest(&effect),
        input.capacity_profile,
    )?;
    let bundle = DirectRegisterBuyHotBundleV4 {
        account_profile,
        lifecycle_policy,
        request_profile,
        transition,
        strategy,
        effect,
        descriptor,
    };
    validate_direct_register_buy_hot_bundle_v4(&bundle, input.capacity_profile)?;
    Ok(bundle)
}

/// Hostile-decode and join every artifact selected by RegisterBuy.
pub fn validate_direct_register_buy_hot_bundle_v4(
    bundle: &DirectRegisterBuyHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<(), DirectRegisteredCreationHotBundleErrorV4> {
    let action = DirectExecutionActionV3::RegisterBuy;
    let descriptor = validate_creation_descriptor(
        &bundle.descriptor,
        digest(&bundle.account_profile),
        digest(&bundle.lifecycle_policy),
        digest(&bundle.request_profile),
        digest(&bundle.transition),
        digest(&bundle.strategy),
        digest(&bundle.effect),
        capacity_profile,
    )?;

    let account = AccountProfileV2::decode(&bundle.account_profile)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::AccountProfile)?;
    let lifecycle_id = digest(&bundle.lifecycle_policy);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        lifecycle_id,
        lifecycle_id,
        &bundle.lifecycle_policy,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?;
    // FOR THIS ACTION. The registered AccountProfile is per-side -- a Sell and a
    // Buy present different frames, which this crate's own bundle test asserts by
    // requiring their digests to differ -- while the lifecycle policy has been
    // ONE policy carrying both sides' plans since wall B was crossed. Validating
    // every plan against one side's profile asks whether the Buy's plans fit the
    // Sell's frame, which is a question with no correct answer. Direct's
    // coordinates happen not to collide today, so this passed; the Dealer LP
    // frame, where the Open payer and the Close RentCredit share fixed slot 7,
    // is where the same shape refused a correct pairing.
    lifecycle
        .validate_account_profile_for_action(account, action as u32)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?;
    if lifecycle
        .action_plan_count(action as u32)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?
        != 2
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Lifecycle);
    }
    let request_id = digest(&bundle.request_profile);
    let request =
        RequestProfileV2::decode_selected(request_id, request_id, &bundle.request_profile)
            .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::RequestProfile)?;
    let transition = TransitionProgramV3::decode(&bundle.transition)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Strategy);
    }
    let effect = EffectProgramV4::decode(&bundle.effect)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Effect)?;
    if effect.span_count() != 0
        || effect.range_count() != 0
        || effect.semantic_prefix_bytes()
            != u32::try_from(DIRECT_REGISTRATION_REQUEST_BYTES_V3)
                .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Geometry)?
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Effect);
    }
    let base = effect.base();
    validate_creation_geometry(
        account,
        request.request_profile(),
        transition,
        base,
        DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4,
    )?;
    if base.route_count() != 3 {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Geometry);
    }
    for (route, start, count, dependencies) in [
        (
            0_u16,
            DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4,
            dclutch_custody_contract::INITIALIZE_REPLAY_ACCOUNT_COUNT_V1,
            0_u16,
        ),
        (
            1,
            DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4,
            dclutch_custody_contract::OPEN_VAULT_ACCOUNT_COUNT_V1,
            1,
        ),
        (
            2,
            DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4,
            dclutch_custody_contract::TRANSFER_ACCOUNT_COUNT_V1,
            2,
        ),
    ] {
        let route = base
            .route(route)
            .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Effect)?;
        if route.role() != FixedRole::Custody
            || route.fixed_account_start() != start
            || route.fixed_account_count() != count
            || route.receipt_dependency_count() != dependencies
        {
            return Err(DirectRegisteredCreationHotBundleErrorV4::Effect);
        }
    }
    Ok(())
}

/// Emit and independently hostile-check one complete RegisterSell Hot bundle.
pub fn build_direct_register_sell_hot_bundle_v4(
    input: DirectRegisterSellHotBundleInputV4<'_>,
) -> Result<DirectRegisterSellHotBundleV4, DirectRegisteredCreationHotBundleErrorV4> {
    let action = DirectExecutionActionV3::RegisterSell;
    let mut account_scratch = [0_u8; DIRECT_REGISTER_SELL_ACCOUNT_PROFILE_BYTES_V4];
    let mut account_profile = [0_u8; DIRECT_REGISTER_SELL_ACCOUNT_PROFILE_BYTES_V4];
    encode_direct_register_sell_account_profile_v4_atomic(
        input.account_profile,
        &mut account_scratch,
        &mut account_profile,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::AccountProfile)?;

    let mut lifecycle_scratch = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
    let mut lifecycle_policy = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
    encode_direct_registered_creation_unified_lifecycle_v5_atomic(
        input.child_rent_widths,
        &mut lifecycle_scratch,
        &mut lifecycle_policy,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?;

    let mut request_v1_scratch = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4];
    let mut request_v1 = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V1_BYTES_V4];
    let mut request_v2_scratch = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4];
    let mut request_profile = [0_u8; DIRECT_REGISTERED_CREATION_REQUEST_PROFILE_V2_BYTES_V4];
    encode_direct_registered_creation_request_profile_v4_atomic(
        action,
        &mut request_v1_scratch,
        &mut request_v1,
        &mut request_v2_scratch,
        &mut request_profile,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::RequestProfile)?;

    let mut transition_scratch = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
    let mut transition = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
    encode_direct_registered_creation_transition_v4_atomic(
        action,
        &mut transition_scratch,
        &mut transition,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Transition)?;
    let transition_id = digest(&transition);
    let strategy = direct_registered_creation_strategy_v4(transition_id)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Strategy)?;

    let mut effect_scratch = [0_u8; DIRECT_REGISTER_SELL_EFFECT_BYTES_V4];
    let mut effect = [0_u8; DIRECT_REGISTER_SELL_EFFECT_BYTES_V4];
    encode_direct_register_sell_effect_v4_atomic(&mut effect_scratch, &mut effect)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Effect)?;

    let descriptor = creation_descriptor(
        digest(&account_profile),
        digest(&lifecycle_policy),
        digest(&request_profile),
        transition_id,
        digest(&strategy),
        digest(&effect),
        input.capacity_profile,
    )?;
    let bundle = DirectRegisterSellHotBundleV4 {
        account_profile,
        lifecycle_policy,
        request_profile,
        transition,
        strategy,
        effect,
        descriptor,
    };
    validate_direct_register_sell_hot_bundle_v4(&bundle, input.capacity_profile)?;
    Ok(bundle)
}

/// Hostile-decode and join every artifact selected by RegisterSell.
pub fn validate_direct_register_sell_hot_bundle_v4(
    bundle: &DirectRegisterSellHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<(), DirectRegisteredCreationHotBundleErrorV4> {
    let action = DirectExecutionActionV3::RegisterSell;
    let descriptor = validate_creation_descriptor(
        &bundle.descriptor,
        digest(&bundle.account_profile),
        digest(&bundle.lifecycle_policy),
        digest(&bundle.request_profile),
        digest(&bundle.transition),
        digest(&bundle.strategy),
        digest(&bundle.effect),
        capacity_profile,
    )?;
    let account = AccountProfileV2::decode(&bundle.account_profile)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::AccountProfile)?;
    let lifecycle_id = digest(&bundle.lifecycle_policy);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        lifecycle_id,
        lifecycle_id,
        &bundle.lifecycle_policy,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?;
    // FOR THIS ACTION. The registered AccountProfile is per-side -- a Sell and a
    // Buy present different frames, which this crate's own bundle test asserts by
    // requiring their digests to differ -- while the lifecycle policy has been
    // ONE policy carrying both sides' plans since wall B was crossed. Validating
    // every plan against one side's profile asks whether the Buy's plans fit the
    // Sell's frame, which is a question with no correct answer. Direct's
    // coordinates happen not to collide today, so this passed; the Dealer LP
    // frame, where the Open payer and the Close RentCredit share fixed slot 7,
    // is where the same shape refused a correct pairing.
    lifecycle
        .validate_account_profile_for_action(account, action as u32)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?;
    if lifecycle
        .action_plan_count(action as u32)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Lifecycle)?
        != 2
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Lifecycle);
    }
    let request_id = digest(&bundle.request_profile);
    let request =
        RequestProfileV2::decode_selected(request_id, request_id, &bundle.request_profile)
            .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::RequestProfile)?;
    let transition = TransitionProgramV3::decode(&bundle.transition)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Strategy);
    }
    let effect = EffectProgramV4::decode(&bundle.effect)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Effect)?;
    if effect.span_count() != 0
        || effect.range_count() != 0
        || effect.semantic_prefix_bytes()
            != u32::try_from(DIRECT_REGISTRATION_REQUEST_BYTES_V3)
                .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Geometry)?
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Effect);
    }
    let base = effect.base();
    // A Sell invokes no child program, so it declares no route. There is nothing
    // to order and nothing to bind a receipt dependency to.
    if base.route_count() != 0 || base.receipt_dependency_count() != 0 {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Effect);
    }
    validate_creation_geometry(
        account,
        request.request_profile(),
        transition,
        base,
        DIRECT_REGISTER_SELL_FIXED_ACCOUNTS_V4,
    )
}

/// The CapabilityProgramV4 both registered creation sides are selected by.
///
/// Every field but the six artifact digests and the capacity profile is a
/// family constant, and the two sides agree on all of them: same kind, same
/// config/request/root schemas, same root state width. A side that needed its
/// own descriptor shape would not be the same capability.
#[allow(clippy::too_many_arguments)]
fn creation_descriptor(
    account_id: [u8; 32],
    lifecycle_id: [u8; 32],
    request_id: [u8; 32],
    transition_id: [u8; 32],
    strategy_id: [u8; 32],
    effect_id: [u8; 32],
    capacity_profile: [u8; 32],
) -> Result<[u8; CAPABILITY_PROGRAM_V4_BYTES], DirectRegisteredCreationHotBundleErrorV4> {
    Ok(CapabilityProgramV4::new(
        content(DIRECT_SUCCESSOR_KIND_ID_V3)?,
        content(DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1)?,
        content(DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3)?,
        content(DIRECT_ROOT_SCHEMA_ID_V1)?,
        content(lifecycle_id)?,
        content(capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: artifact(ACCOUNT_PROFILE_SCHEMA_ID_V2, account_id)?,
            request_profile: artifact(REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, request_id)?,
            lifecycle: artifact(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, lifecycle_id)?,
            strategy: artifact(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, strategy_id)?,
            transition: artifact(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID, transition_id)?,
            effect: artifact(EFFECT_SCHEMA_ID_V4, effect_id)?,
        },
        u32::try_from(DIRECT_ROOT_STATE_BYTES_V1)
            .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Geometry)?,
    )
    .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Descriptor)?
    .encode())
}

/// Hostile-decode the descriptor and re-derive every join it claims.
///
/// The digests are taken by the caller from the bundle's own bytes, so this
/// refuses a descriptor that names an artifact the bundle does not carry.
#[allow(clippy::too_many_arguments)]
fn validate_creation_descriptor(
    bytes: &[u8; CAPABILITY_PROGRAM_V4_BYTES],
    account_id: [u8; 32],
    lifecycle_id: [u8; 32],
    request_id: [u8; 32],
    transition_id: [u8; 32],
    strategy_id: [u8; 32],
    effect_id: [u8; 32],
    capacity_profile: [u8; 32],
) -> Result<CapabilityProgramV4, DirectRegisteredCreationHotBundleErrorV4> {
    let descriptor = CapabilityProgramV4::decode(bytes)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Descriptor)?;
    if descriptor.kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
        || descriptor.config_schema().to_bytes() != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1
        || descriptor.request_schema().to_bytes() != DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3
        || descriptor.root_schema().to_bytes() != DIRECT_ROOT_SCHEMA_ID_V1
        || descriptor.derivation_policy().to_bytes() != lifecycle_id
        || descriptor.capacity_profile().to_bytes() != capacity_profile
        || descriptor.account_profile() != artifact(ACCOUNT_PROFILE_SCHEMA_ID_V2, account_id)?
        || descriptor.request_profile()
            != artifact(REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, request_id)?
        || descriptor.lifecycle()
            != artifact(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, lifecycle_id)?
        || descriptor.strategy() != artifact(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, strategy_id)?
        || descriptor.transition()
            != artifact(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID, transition_id)?
        || descriptor.effect() != artifact(EFFECT_SCHEMA_ID_V4, effect_id)?
        || descriptor.root_state_bytes()
            != u32::try_from(DIRECT_ROOT_STATE_BYTES_V1)
                .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Geometry)?
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Descriptor);
    }
    Ok(descriptor)
}

/// The four artifacts that carry a register geometry must state the SAME one,
/// and all four must agree on the fixed account count the side declares.
///
/// The register banks are family-wide: both sides project into, read from and
/// write the same 56 scalars and 32 identities, which is why the Transition is
/// shared at all. The account count is the side's own -- 55 for a Buy carrying
/// three Custody frames, 13 for a Sell carrying none -- and it is passed in
/// rather than derived so a side cannot silently be validated against the
/// other's topology.
fn validate_creation_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    base: EffectProgramV3<'_>,
    fixed_accounts: u16,
) -> Result<(), DirectRegisteredCreationHotBundleErrorV4> {
    let common_scalars = u16::try_from(DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Geometry)?;
    let common_identities = u16::try_from(DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4)
        .map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Geometry)?;
    if account.fixed_account_count() != fixed_accounts
        || account.item_account_stride() != 0
        || account.common_scalar_count() != common_scalars
        || account.item_scalar_stride() != DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4
        || account.common_identity_count() != common_identities
        || account.item_identity_stride() != DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4
        || request.common_scalar_count() != common_scalars
        || request.item_scalar_stride() != DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4
        || request.common_identity_count() != common_identities
        || request.item_identity_stride() != DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4
        || transition.common_scalar_count() != common_scalars
        || transition.item_scalar_stride() != DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4
        || transition.common_identity_count() != common_identities
        || transition.item_identity_stride() != DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4
        || base.fixed_account_count() != fixed_accounts
        || base.item_account_stride() != 0
        || base.common_scalar_count() != common_scalars
        || base.item_scalar_stride() != DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4
        || base.common_identity_count() != common_identities
        || base.item_identity_stride() != DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4
    {
        return Err(DirectRegisteredCreationHotBundleErrorV4::Geometry);
    }
    Ok(())
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectRegisteredCreationHotBundleErrorV4> {
    ContentId::new(bytes).map_err(|_| DirectRegisteredCreationHotBundleErrorV4::Content)
}

fn artifact(
    schema: [u8; 32],
    program: [u8; 32],
) -> Result<ArtifactReferenceV4, DirectRegisteredCreationHotBundleErrorV4> {
    Ok(ArtifactReferenceV4::new(
        content(schema)?,
        content(program)?,
    ))
}

#[cfg(test)]
mod tests {
    /// The observed Token-2022 vault width these builders quote against.
    ///
    /// An observation, never a protocol constant: the width belongs to the
    /// selected token program and a Token-2022 account carrying extensions is
    /// not 165 bytes. It is here because the ROOT's shared policy names the
    /// Buy's quotes even when a Sell is the side being built.
    const OBSERVED_VAULT_BYTES: u32 = 165;

    extern crate std;

    use dclutch_account_profile_contract::{
        lifecycle_v3::{CoordinateScopeV3, LifecycleRegisterKindV3},
        v2::{ProjectionRegisterKindV2, ProjectionRegisterSpaceV2, ProjectionTargetV2},
    };
    use dclutch_custody_contract::{CustodyRequestV1, DelegatedCustodyRequestV2};
    use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3;
    use dclutch_product_runtime_v2::{PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES};
    use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
    use dclutch_realm_contract::REALM_BYTES;
    use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
    use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
    use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
    use dclutch_request_profile_contract::{
        ProjectionRegisterKindV1, ProjectionRegisterSpaceV1, ProjectionTargetV1,
    };
    use dclutch_transition_vm::v3::{RegisterKindV3, RegisterSpaceV3, RegisterWriteTargetV3};
    use std::{format, vec, vec::Vec};

    use crate::registered_account_artifacts_v4::DIRECT_REGISTER_SELL_COLLATERAL_ACCOUNT_V4;
    use crate::registered_creation_artifacts_v4::REGISTERED_IDENTITY_PARENT_REQUEST_V4;

    use super::*;

    fn logical_lengths(basis_bytes: u32) -> Vec<u32> {
        let mut output = vec![0_u32; usize::from(DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4)];
        let root = dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
            + DIRECT_ROOT_STATE_BYTES_V1;
        *output.get_mut(0).expect("root") = u32::try_from(root).expect("root width");
        *output.get_mut(1).expect("config") =
            u32::try_from(crate::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1).expect("config");
        *output.get_mut(2).expect("Product") =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product");
        *output.get_mut(3).expect("portfolio") =
            u32::try_from(PORTFOLIO_HEADER_BYTES + 3 * PORTFOLIO_COEFFICIENT_BYTES)
                .expect("portfolio");
        *output.get_mut(4).expect("basis") = basis_bytes;
        *output.get_mut(5).expect("maker") =
            u32::try_from(crate::successor::DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker");
        *output.get_mut(7).expect("lifecycle RentCredit") =
            u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit");
        *output.get_mut(8).expect("record") =
            u32::try_from(crate::successor::DIRECT_REGISTERED_RECORD_BYTES_V2).expect("record");
        *output.get_mut(10).expect("Rent program") =
            u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES).expect("Rent program");
        *output.get_mut(13).expect("Core") =
            u32::try_from(dclutch_market_core_codec::STATE_BYTES).expect("Core");
        *output.get_mut(14).expect("activation") =
            u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation");
        *output.get_mut(15).expect("Registry") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Registry");
        *output.get_mut(16).expect("Trading") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Trading");
        *output.get_mut(17).expect("ProgramData") = 1_024;
        *output.get_mut(18).expect("Realm") = u32::try_from(REALM_BYTES).expect("Realm");
        *output.get_mut(23).expect("Rent sysvar") = 17;
        *output.get_mut(34).expect("mint") = 82;
        *output.get_mut(37).expect("token program") = 36;
        *output.get_mut(51).expect("source") = 165;
        for (account, representative) in [
            (22_usize, 11_usize),
            (24, 7),
            (26, 13),
            (27, 14),
            (28, 15),
            (29, 16),
            (30, 17),
            (31, 18),
            (32, 19),
            (33, 20),
            (39, 11),
            (40, 23),
            (42, 13),
            (43, 14),
            (44, 15),
            (45, 16),
            (46, 17),
            (47, 18),
            (48, 19),
            (49, 20),
            (50, 34),
            (52, 35),
            (53, 36),
            (54, 37),
        ] {
            let value = *output.get(representative).expect("representative");
            *output.get_mut(account).expect("alias") = value;
        }
        output
    }

    fn build(basis_bytes: u32) -> DirectRegisterBuyHotBundleV4 {
        let lengths = logical_lengths(basis_bytes);
        build_direct_register_buy_hot_bundle_v4(DirectRegisterBuyHotBundleInputV4 {
            account_profile: DirectRegisterBuyAccountProfileInputV4 {
                logical_data_lengths: &lengths,
            },
            child_rent_widths: DirectRegisteredCreationChildRentWidthsV4 { custody_vault: 165 },
            capacity_profile: [0x44; 32],
        })
        .expect("registered Buy bundle")
    }

    /// The thirteen widths a RegisterSell frame observes: the shared prefix, and
    /// the maker collateral token account the record pays its fill into.
    fn sell_logical_lengths(basis_bytes: u32) -> Vec<u32> {
        let buy = logical_lengths(basis_bytes);
        let mut output = buy
            .get(..usize::from(DIRECT_REGISTER_SELL_COLLATERAL_ACCOUNT_V4))
            .expect("prefix")
            .to_vec();
        // Descriptive: the rule is opaque, because a Token-2022 account carrying
        // extensions is not 165 bytes and Custody -- not Direct -- owns that.
        output.push(165);
        assert_eq!(
            output.len(),
            usize::from(DIRECT_REGISTER_SELL_FIXED_ACCOUNTS_V4)
        );
        output
    }

    fn build_sell(basis_bytes: u32) -> DirectRegisterSellHotBundleV4 {
        let lengths = sell_logical_lengths(basis_bytes);
        build_direct_register_sell_hot_bundle_v4(DirectRegisterSellHotBundleInputV4 {
            account_profile: DirectRegisteredCreationAccountProfileInputV4 {
                logical_data_lengths: &lengths,
            },
            child_rent_widths: DirectRegisteredCreationChildRentWidthsV4 {
                custody_vault: OBSERVED_VAULT_BYTES,
            },
            capacity_profile: [0x44; 32],
        })
        .expect("registered Sell bundle")
    }

    #[test]
    fn one_descriptor_joins_profile14_lifecycle_v5_and_ordered_custody() {
        let bundle = build(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis"));
        validate_direct_register_buy_hot_bundle_v4(&bundle, [0x44; 32]).expect("validate");
        let effect = EffectProgramV4::decode(&bundle.effect).expect("effect");
        let base = effect.base();
        let initialize = base.route_template(0).expect("initialize").0;
        let open = base.route_template(1).expect("open").0;
        let deposit = base.route_template(2).expect("deposit").0;
        CustodyRequestV1::decode(initialize).expect("initialize template");
        CustodyRequestV1::decode(open).expect("open template");
        DelegatedCustodyRequestV2::decode(deposit).expect("deposit template");
    }

    /// One side's five joined artifacts, as the bytes the descriptor selects.
    struct CreationArtifactsV4<'a> {
        action: DirectExecutionActionV3,
        account_profile: &'a [u8],
        lifecycle_policy: &'a [u8],
        request_profile: &'a [u8],
        transition: &'a [u8],
        effect: &'a [u8],
    }

    impl CreationArtifactsV4<'_> {
        /// Does some artifact in this bundle DECLARE itself the writer of one
        /// common register?
        ///
        /// `upstream_only` excludes the Transition's own writes, which is what a
        /// TRANSITION read demands: a register the Transition reads out of its
        /// input bank must have been written by something that ran before it.
        /// An EFFECT read may legitimately be satisfied by the Transition, which
        /// runs first, so that join passes `false`.
        fn declares_writer(&self, scalar: bool, index: u16, upstream_only: bool) -> bool {
            let account = AccountProfileV2::decode(self.account_profile).expect("profile");
            let request_id = digest_of(self.request_profile);
            let request =
                RequestProfileV2::decode_selected(request_id, request_id, self.request_profile)
                    .expect("request profile");
            let transition = TransitionProgramV3::decode(self.transition).expect("transition");
            let lifecycle_id = digest_of(self.lifecycle_policy);
            let lifecycle = StateLifecyclePolicyV5::decode_selected(
                lifecycle_id,
                lifecycle_id,
                self.lifecycle_policy,
            )
            .expect("lifecycle");

            let (account_kind, request_kind, transition_kind, lifecycle_kind) = if scalar {
                (
                    ProjectionRegisterKindV2::Scalar,
                    ProjectionRegisterKindV1::Scalar,
                    RegisterKindV3::Scalar,
                    LifecycleRegisterKindV3::Scalar,
                )
            } else {
                (
                    ProjectionRegisterKindV2::Identity,
                    ProjectionRegisterKindV1::Identity,
                    RegisterKindV3::Identity,
                    LifecycleRegisterKindV3::Identity,
                )
            };
            let quoted = scalar
                && (0..lifecycle.current_rent_quote_count()).any(|ordinal| {
                    lifecycle
                        .current_rent_quote(ordinal)
                        .expect("quote")
                        .scalar_destination()
                        .index()
                        == index
                });
            // `RequestProfileV2::writes_register` delegates to the EMBEDDED V1
            // only, so the V2 adapter's own declaration -- the identity register
            // one authenticated native Ed25519 signature is projected into -- is
            // invisible to it. That is the register the whole family's signer
            // requirement lands in, so it is a declared writer and it is read
            // here directly rather than through the delegating accessor.
            let native = request.native_signatures();
            let native_signer = (0..native.requirement_count()).any(|ordinal| {
                native
                    .requirement(ordinal)
                    .expect("signature requirement")
                    .destination_identity_register()
                    == u32::from(index)
            });
            let action = self.action as u32;
            let protected = (0..lifecycle.action_plan_count(action).expect("plans")).any(|plan| {
                let selected = lifecycle.action_plan(action, plan).expect("plan");
                (0..selected.protected_output_count().expect("outputs")).any(|ordinal| {
                    let target = selected
                        .protected_output_target(ordinal)
                        .expect("protected target");
                    target.kind() == lifecycle_kind
                        && target.scope() == CoordinateScopeV3::Fixed
                        && target.index() == index
                })
            });

            account
                .writes_register(ProjectionTargetV2 {
                    kind: account_kind,
                    space: ProjectionRegisterSpaceV2::Common,
                    index,
                })
                .expect("profile writes")
                || request
                    .writes_register(ProjectionTargetV1 {
                        kind: request_kind,
                        space: ProjectionRegisterSpaceV1::Common,
                        index,
                    })
                    .expect("request writes")
                || (!upstream_only
                    && transition
                        .writes_register(RegisterWriteTargetV3 {
                            kind: transition_kind,
                            space: RegisterSpaceV3::Common,
                            index,
                        })
                        .expect("transition writes"))
                || quoted
                || protected
                || (!scalar && native_signer)
                // The parent request digest is seeded by common Hot before any
                // family artifact runs; it is the one register with an executor
                // author rather than an artifact one.
                || (!scalar && usize::from(index) == REGISTERED_IDENTITY_PARENT_REQUEST_V4)
        }

        /// The registers this side's EffectProgram reads, MEASURED: each common
        /// register is perturbed in isolation and every fixed effect is resolved
        /// against both banks, so a register counts as read exactly when it
        /// moves some resolved effect.
        fn effect_reads(&self) -> (Vec<u16>, Vec<u16>) {
            const TAIL: u32 = 3;
            const PROBE_SCALAR: u64 = 0x5a5a_5a5a_5a5a_5a5a;
            const PROBE_IDENTITY: [u8; 32] = [0x5a; 32];
            let effect = EffectProgramV4::decode(self.effect).expect("effect");
            let base = effect.base();
            let operations = base.fixed_operation_count();
            assert!(operations > 0);

            let baseline_scalars = vec![0_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
            let baseline_identities =
                vec![[0_u8; 32]; DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
            let differs = |scalars: &[u64], identities: &[[u8; 32]]| {
                (0..operations).any(|index| {
                    base.resolved_fixed_effect(index, TAIL, scalars, identities)
                        != base.resolved_fixed_effect(
                            index,
                            TAIL,
                            &baseline_scalars,
                            &baseline_identities,
                        )
                })
            };

            let mut scalars = Vec::new();
            for index in 0..DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4 {
                let mut probe = baseline_scalars.clone();
                *probe.get_mut(index).expect("scalar") = PROBE_SCALAR;
                if differs(&probe, &baseline_identities) {
                    scalars.push(u16::try_from(index).expect("scalar register"));
                }
            }
            let mut identities = Vec::new();
            for index in 0..DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4 {
                let mut probe = baseline_identities.clone();
                *probe.get_mut(index).expect("identity") = PROBE_IDENTITY;
                if differs(&baseline_scalars, &probe) {
                    identities.push(u16::try_from(index).expect("identity register"));
                }
            }
            (scalars, identities)
        }

        /// Join one measured read set against the static write declarations and
        /// name every register nothing claims.
        fn unwritten(
            &self,
            reads: (&[u16], &[u16]),
            upstream_only: bool,
        ) -> Vec<std::string::String> {
            let mut output = Vec::new();
            for index in reads.0 {
                if !self.declares_writer(true, *index, upstream_only) {
                    output.push(format!("scalar {index}"));
                }
            }
            for index in reads.1 {
                if !self.declares_writer(false, *index, upstream_only) {
                    output.push(format!("identity {index}"));
                }
            }
            output
        }
    }

    fn digest_of(bytes: &[u8]) -> [u8; 32] {
        digest(bytes)
    }

    fn buy_artifacts(bundle: &DirectRegisterBuyHotBundleV4) -> CreationArtifactsV4<'_> {
        CreationArtifactsV4 {
            action: DirectExecutionActionV3::RegisterBuy,
            account_profile: &bundle.account_profile,
            lifecycle_policy: &bundle.lifecycle_policy,
            request_profile: &bundle.request_profile,
            transition: &bundle.transition,
            effect: &bundle.effect,
        }
    }

    fn sell_artifacts(bundle: &DirectRegisterSellHotBundleV4) -> CreationArtifactsV4<'_> {
        CreationArtifactsV4 {
            action: DirectExecutionActionV3::RegisterSell,
            account_profile: &bundle.account_profile,
            lifecycle_policy: &bundle.lifecycle_policy,
            request_profile: &bundle.request_profile,
            transition: &bundle.transition,
            effect: &bundle.effect,
        }
    }

    /// Nothing the Effect reads may be a register no artifact writes.
    ///
    /// This is the defect class that has now cost this family three finds and
    /// no chain has ever refused one of them, because no registered creation
    /// has ever executed. `52f14fa`'s successor found the Transition comparing
    /// signed credit keys against identity registers the AccountProfile never
    /// wrote; this lane found the Effect writing registers 50 and 51 into the
    /// Custody `InitializeReplay` and `OpenVault` requests' `rent_lamports`
    /// while the LifecycleV5 quote table stopped at 53, so both were zero and
    /// `CustodyRequestV1::validate` refuses `rent_lamports == 0` for both
    /// operations. Every registered Buy would have refused at its first CPI.
    ///
    /// The reads are MEASURED, not restated; the writers are the artifacts' own
    /// static declarations. Neither side is a mirror of the other.
    #[test]
    fn every_register_the_effect_reads_has_a_declared_writer() {
        let buy = build(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis"));
        let sell = build_sell(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis"));
        // Pinned per side, not floored: a resolver that errored on every bank
        // would report no reads at all and pass a floor. The Buy reads its whole
        // Custody chain's operands; the Sell's Effect is the local-state block
        // alone, and reads correspondingly fewer.
        for (artifacts, expected) in [(buy_artifacts(&buy), 43), (sell_artifacts(&sell), 30)] {
            let action = artifacts.action;
            let (scalars, identities) = artifacts.effect_reads();
            let unwritten = artifacts.unwritten((&scalars, &identities), false);
            assert!(
                unwritten.is_empty(),
                "the {action:?} Effect reads registers no artifact writes: {unwritten:?}"
            );
            let read = scalars.len() + identities.len();
            assert_eq!(
                read, expected,
                "{action:?}: the measured Effect read set moved"
            );
        }
    }

    /// NOTHING THE TRANSITION READS MAY BE A REGISTER NO UPSTREAM ARTIFACT
    /// WRITES -- the hole the Effect witness could not see.
    ///
    /// `7357aece` found `REGISTERED_IDENTITY_COLLATERAL_SOURCE_V4` read by the
    /// SHARED creation Transition, unconditionally, with its only writer inside
    /// the Custody `Transfer` window a Sell drops entirely. The Effect witness
    /// above cannot see it, for a stated reason: that witness perturbs the
    /// EFFECT, and this register is read by the TRANSITION. So the same join
    /// runs one artifact earlier, and it excludes the Transition's own writes --
    /// a register the Transition reads out of its input bank must have a writer
    /// that ran BEFORE it, which the AccountProfile, the RequestProfile and the
    /// LifecycleV5 are and the Transition is not.
    #[test]
    fn every_register_the_transition_reads_has_a_declared_upstream_writer() {
        let buy = build(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis"));
        let sell = build_sell(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis"));
        for artifacts in [buy_artifacts(&buy), sell_artifacts(&sell)] {
            let action = artifacts.action;
            let (scalars, identities) =
                crate::registered_creation_artifacts_v4::tests::transition_read_set(action);
            let scalars: Vec<u16> = scalars
                .into_iter()
                .map(|index| u16::try_from(index).expect("scalar register"))
                .collect();
            let identities: Vec<u16> = identities
                .into_iter()
                .map(|index| u16::try_from(index).expect("identity register"))
                .collect();
            let unwritten = artifacts.unwritten((&scalars, &identities), true);
            assert!(
                unwritten.is_empty(),
                "the {action:?} Transition reads registers no upstream artifact writes: \
                 {unwritten:?}"
            );
            let read = scalars.len() + identities.len();
            assert_eq!(
                read, 34,
                "{action:?}: the measured Transition read set moved"
            );
        }
    }

    /// One descriptor selects a RegisterSell, and it selects a DIFFERENT one.
    ///
    /// The two sides share every SCHEMA in the descriptor and not one artifact
    /// digest. All six differ, and the reason is worth naming per artifact,
    /// because "the sides share the register bank" is true and "the sides share
    /// an artifact" is false: the AccountProfile carries a different frame, the
    /// LifecycleV5 a different quote table, the Transition a different expected
    /// side, the Effect a different `RESERVED_CLAIMS` operand, the strategy the
    /// Transition's digest -- and the RequestProfile pins the request's own
    /// action discriminant at byte 12, so even it is side-selected. That is
    /// stated here so a lane that "reuses the Buy bundle for the Sell" fails a
    /// test rather than a chain.
    #[test]
    fn one_sell_descriptor_joins_a_routeless_effect_and_a_thirteen_account_frame() {
        let sell = build_sell(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis"));
        validate_direct_register_sell_hot_bundle_v4(&sell, [0x44; 32]).expect("validate");
        let buy = build(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis"));

        let effect = EffectProgramV4::decode(&sell.effect).expect("effect");
        let base = effect.base();
        assert_eq!(base.route_count(), 0);
        assert_eq!(
            base.fixed_account_count(),
            DIRECT_REGISTER_SELL_FIXED_ACCOUNTS_V4
        );

        // A Sell needs no `child_rent_widths` supplier at all: its LifecycleV5
        // quotes only the two accounts this crate knows the exact width of.
        let lifecycle_id = digest(&sell.lifecycle_policy);
        let lifecycle = StateLifecyclePolicyV5::decode_selected(
            lifecycle_id,
            lifecycle_id,
            &sell.lifecycle_policy,
        )
        .expect("lifecycle");
        // FOUR declarations, of which a Sell projects TWO. The policy is the
        // root's and carries both sides' quotes; the action tag is what keeps a
        // Sell from projecting rent for a Custody child it never opens. Derived
        // from the contract rather than restated as a literal subsequence.
        assert_eq!(lifecycle.current_rent_quote_count(), 4);
        assert_eq!(
            lifecycle.action_current_rent_quote_count(DirectExecutionActionV3::RegisterSell as u32),
            Ok(2),
        );
        assert_eq!(
            lifecycle.action_current_rent_quote_count(DirectExecutionActionV3::RegisterBuy as u32),
            Ok(4),
        );

        assert_ne!(digest(&sell.account_profile), digest(&buy.account_profile));
        // WALL B, INVERTED. This assertion was `assert_ne!`, and that inequality
        // WAS the wall: one manifest entry pins one `derivation_policy`, so two
        // policies with two digests meant no Direct root could admit both
        // creation actions. They are one policy and one digest now, so a single
        // entry serves both sides. The account profiles still differ, and should
        // -- the two sides genuinely present different frames.
        assert_eq!(
            digest(&sell.lifecycle_policy),
            digest(&buy.lifecycle_policy),
            "one root, one entry, one lifecycle policy: this equality is wall B being crossed",
        );
        assert_ne!(digest(&sell.request_profile), digest(&buy.request_profile));
        assert_ne!(digest(&sell.transition), digest(&buy.transition));
        assert_ne!(digest(&sell.strategy), digest(&buy.strategy));
        assert_ne!(digest(&sell.effect), digest(&buy.effect));
        assert_ne!(digest(&sell.descriptor), digest(&buy.descriptor));

        // And the set entry a Sell selector resolves to is obtainable ONLY by
        // validating this bundle -- the same gate every other action passes.
        let entry =
            crate::program_set_v4::validate_direct_register_sell_capability_v4(&sell, [0x44; 32])
                .expect("Sell capability");
        assert_eq!(entry.action(), DirectExecutionActionV3::RegisterSell);
        assert_eq!(entry.descriptor(), digest(&sell.descriptor));
        assert_eq!(
            crate::program_set_v4::validate_direct_register_sell_capability_v4(&sell, [0x45; 32]),
            Err(crate::program_set_v4::DirectProgramSetErrorV4::Bundle)
        );
    }

    #[test]
    fn capacity_or_artifact_substitution_refuses_exactly() {
        let bundle = build(736);
        assert_eq!(
            validate_direct_register_buy_hot_bundle_v4(&bundle, [0x45; 32]),
            Err(DirectRegisteredCreationHotBundleErrorV4::Descriptor)
        );
        let mut hostile = bundle;
        *hostile.effect.get_mut(128).expect("effect byte") ^= 1;
        assert_eq!(
            validate_direct_register_buy_hot_bundle_v4(&hostile, [0x44; 32]),
            Err(DirectRegisteredCreationHotBundleErrorV4::Descriptor)
        );

        let sell = build_sell(736);
        assert_eq!(
            validate_direct_register_sell_hot_bundle_v4(&sell, [0x45; 32]),
            Err(DirectRegisteredCreationHotBundleErrorV4::Descriptor)
        );
        let mut hostile = sell;
        *hostile.account_profile.get_mut(64).expect("profile byte") ^= 1;
        assert_eq!(
            validate_direct_register_sell_hot_bundle_v4(&hostile, [0x44; 32]),
            Err(DirectRegisteredCreationHotBundleErrorV4::Descriptor)
        );
        // THE SIDE SUBSTITUTION. The two Transitions are the same width, so a
        // Sell bundle carrying the Buy's Transition is a well-formed object --
        // and it is the exact object `e03a51fd`'s "rebase, do not copy" ruling
        // is about. The descriptor names the digest, so it refuses.
        let mut swapped = build_sell(736);
        swapped.transition = build(736).transition;
        assert_eq!(
            validate_direct_register_sell_hot_bundle_v4(&swapped, [0x44; 32]),
            Err(DirectRegisteredCreationHotBundleErrorV4::Descriptor)
        );
    }
}
