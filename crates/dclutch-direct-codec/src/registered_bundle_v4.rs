//! Complete schema-bound Hot artifact bundle for registered Direct Buy admission.
//!
//! The bundle is selected by a normal CapabilityProgramV4 descriptor. Trading
//! remains family-neutral: Profile14 authenticates the fixed account frame,
//! LifecycleV5 owns maker/record first use and current Rent quotes, Transition
//! derives the reserve, and EffectV4 executes the ordered Custody chain.

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
    v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4},
};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_request_profile_contract::v2::{
    REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, RequestProfileV2,
};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use sha2::{Digest, Sha256};

use crate::{
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3, DIRECT_REGISTRATION_REQUEST_BYTES_V3,
        DIRECT_SUCCESSOR_KIND_ID_V3, DirectExecutionActionV3,
    },
    registered_account_artifacts_v4::{
        DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4, DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4,
        DirectRegisterBuyAccountProfileInputV4,
        encode_direct_register_buy_account_profile_v4_atomic,
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
        DIRECT_REGISTER_BUY_EFFECT_BYTES_V4, encode_direct_register_buy_effect_v4_atomic,
    },
    registered_state_artifacts_v4::{
        DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5,
        encode_direct_registered_creation_lifecycle_v5_atomic,
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

/// Stable registered bundle refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRegisterBuyHotBundleErrorV4 {
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
) -> Result<DirectRegisterBuyHotBundleV4, DirectRegisterBuyHotBundleErrorV4> {
    let action = DirectExecutionActionV3::RegisterBuy;
    let mut account_scratch = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
    let mut account_profile = [0_u8; DIRECT_REGISTER_BUY_ACCOUNT_PROFILE_BYTES_V4];
    encode_direct_register_buy_account_profile_v4_atomic(
        input.account_profile,
        &mut account_scratch,
        &mut account_profile,
    )
    .map_err(|_| DirectRegisterBuyHotBundleErrorV4::AccountProfile)?;

    let mut lifecycle_scratch = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
    let mut lifecycle_policy = [0_u8; DIRECT_REGISTERED_CREATION_LIFECYCLE_BYTES_V5];
    encode_direct_registered_creation_lifecycle_v5_atomic(
        action,
        &mut lifecycle_scratch,
        &mut lifecycle_policy,
    )
    .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Lifecycle)?;

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
    .map_err(|_| DirectRegisterBuyHotBundleErrorV4::RequestProfile)?;

    let mut transition_scratch = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
    let mut transition = [0_u8; DIRECT_REGISTERED_CREATION_TRANSITION_BYTES_V4];
    encode_direct_registered_creation_transition_v4_atomic(
        action,
        &mut transition_scratch,
        &mut transition,
    )
    .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Transition)?;
    let transition_id = digest(&transition);
    let strategy = direct_registered_creation_strategy_v4(transition_id)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Strategy)?;

    let mut effect_scratch = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
    let mut effect = [0_u8; DIRECT_REGISTER_BUY_EFFECT_BYTES_V4];
    encode_direct_register_buy_effect_v4_atomic(&mut effect_scratch, &mut effect)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Effect)?;

    let account_id = digest(&account_profile);
    let lifecycle_id = digest(&lifecycle_policy);
    let request_id = digest(&request_profile);
    let strategy_id = digest(&strategy);
    let effect_id = digest(&effect);
    let descriptor = CapabilityProgramV4::new(
        content(DIRECT_SUCCESSOR_KIND_ID_V3)?,
        content(DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1)?,
        content(DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3)?,
        content(DIRECT_ROOT_SCHEMA_ID_V1)?,
        content(lifecycle_id)?,
        content(input.capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: artifact(ACCOUNT_PROFILE_SCHEMA_ID_V2, account_id)?,
            request_profile: artifact(REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, request_id)?,
            lifecycle: artifact(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, lifecycle_id)?,
            strategy: artifact(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, strategy_id)?,
            transition: artifact(dclutch_transition_vm::v3::SCHEMA_RELEASE_ID, transition_id)?,
            effect: artifact(EFFECT_SCHEMA_ID_V4, effect_id)?,
        },
        u32::try_from(DIRECT_ROOT_STATE_BYTES_V1)
            .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Geometry)?,
    )
    .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Descriptor)?;
    let bundle = DirectRegisterBuyHotBundleV4 {
        account_profile,
        lifecycle_policy,
        request_profile,
        transition,
        strategy,
        effect,
        descriptor: descriptor.encode(),
    };
    validate_direct_register_buy_hot_bundle_v4(&bundle, input.capacity_profile)?;
    Ok(bundle)
}

/// Hostile-decode and join every artifact selected by RegisterBuy.
pub fn validate_direct_register_buy_hot_bundle_v4(
    bundle: &DirectRegisterBuyHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<(), DirectRegisterBuyHotBundleErrorV4> {
    let action = DirectExecutionActionV3::RegisterBuy;
    let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Descriptor)?;
    if descriptor.kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
        || descriptor.config_schema().to_bytes() != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1
        || descriptor.request_schema().to_bytes() != DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3
        || descriptor.root_schema().to_bytes() != DIRECT_ROOT_SCHEMA_ID_V1
        || descriptor.derivation_policy().to_bytes() != digest(&bundle.lifecycle_policy)
        || descriptor.capacity_profile().to_bytes() != capacity_profile
        || descriptor.account_profile()
            != artifact(
                ACCOUNT_PROFILE_SCHEMA_ID_V2,
                digest(&bundle.account_profile),
            )?
        || descriptor.request_profile()
            != artifact(
                REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID,
                digest(&bundle.request_profile),
            )?
        || descriptor.lifecycle()
            != artifact(
                SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
                digest(&bundle.lifecycle_policy),
            )?
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
        || descriptor.root_state_bytes()
            != u32::try_from(DIRECT_ROOT_STATE_BYTES_V1)
                .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Geometry)?
    {
        return Err(DirectRegisterBuyHotBundleErrorV4::Descriptor);
    }

    let account = AccountProfileV2::decode(&bundle.account_profile)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::AccountProfile)?;
    let lifecycle_id = digest(&bundle.lifecycle_policy);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        lifecycle_id,
        lifecycle_id,
        &bundle.lifecycle_policy,
    )
    .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Lifecycle)?;
    lifecycle
        .validate_account_profile(account)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Lifecycle)?;
    if lifecycle
        .action_plan_count(action as u32)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Lifecycle)?
        != 2
    {
        return Err(DirectRegisterBuyHotBundleErrorV4::Lifecycle);
    }
    let request_id = digest(&bundle.request_profile);
    let request =
        RequestProfileV2::decode_selected(request_id, request_id, &bundle.request_profile)
            .map_err(|_| DirectRegisterBuyHotBundleErrorV4::RequestProfile)?;
    let transition = TransitionProgramV3::decode(&bundle.transition)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
    {
        return Err(DirectRegisterBuyHotBundleErrorV4::Strategy);
    }
    let effect = EffectProgramV4::decode(&bundle.effect)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Effect)?;
    if effect.span_count() != 0
        || effect.range_count() != 0
        || effect.semantic_prefix_bytes()
            != u32::try_from(DIRECT_REGISTRATION_REQUEST_BYTES_V3)
                .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Geometry)?
    {
        return Err(DirectRegisterBuyHotBundleErrorV4::Effect);
    }
    let base = effect.base();
    let request = request.request_profile();
    let common_scalars = u16::try_from(DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Geometry)?;
    let common_identities = u16::try_from(DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4)
        .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Geometry)?;
    if account.fixed_account_count() != DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4
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
        || base.fixed_account_count() != DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4
        || base.item_account_stride() != 0
        || base.common_scalar_count() != common_scalars
        || base.item_scalar_stride() != DIRECT_REGISTERED_CREATION_ITEM_SCALAR_STRIDE_V4
        || base.common_identity_count() != common_identities
        || base.item_identity_stride() != DIRECT_REGISTERED_CREATION_ITEM_IDENTITY_STRIDE_V4
        || base.route_count() != 3
    {
        return Err(DirectRegisterBuyHotBundleErrorV4::Geometry);
    }
    for (route, start, count, dependencies) in [
        (0_u16, 12_u16, 12_u16, 0_u16),
        (1, 24, 16, 1),
        (2, 40, 14, 2),
    ] {
        let route = base
            .route(route)
            .map_err(|_| DirectRegisterBuyHotBundleErrorV4::Effect)?;
        if route.role() != FixedRole::Custody
            || route.fixed_account_start() != start
            || route.fixed_account_count() != count
            || route.receipt_dependency_count() != dependencies
        {
            return Err(DirectRegisterBuyHotBundleErrorV4::Effect);
        }
    }
    Ok(())
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectRegisterBuyHotBundleErrorV4> {
    ContentId::new(bytes).map_err(|_| DirectRegisterBuyHotBundleErrorV4::Content)
}

fn artifact(
    schema: [u8; 32],
    program: [u8; 32],
) -> Result<ArtifactReferenceV4, DirectRegisterBuyHotBundleErrorV4> {
    Ok(ArtifactReferenceV4::new(
        content(schema)?,
        content(program)?,
    ))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use dclutch_custody_contract::{CustodyRequestV1, DelegatedCustodyRequestV2};
    use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3;
    use dclutch_product_runtime_v2::{PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES};
    use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
    use dclutch_realm_contract::REALM_BYTES;
    use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
    use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
    use dclutch_rent_contract::RENT_CREDIT_BYTES_V1;
    use std::{vec, vec::Vec};

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
        *output.get_mut(7).expect("maker RentCredit") =
            u32::try_from(RENT_CREDIT_BYTES_V1).expect("RentCredit");
        *output.get_mut(8).expect("record") =
            u32::try_from(crate::successor::DIRECT_REGISTERED_RECORD_BYTES_V2).expect("record");
        *output.get_mut(10).expect("record RentCredit") =
            u32::try_from(RENT_CREDIT_BYTES_V1).expect("RentCredit");
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
        *output.get_mut(23).expect("Custody program") = 17;
        *output.get_mut(33).expect("mint") = 82;
        *output.get_mut(36).expect("token program") = 36;
        *output.get_mut(50).expect("source") = 165;
        for (account, representative) in [
            (22_usize, 11_usize),
            (25, 13),
            (26, 14),
            (27, 15),
            (28, 16),
            (29, 17),
            (30, 18),
            (31, 19),
            (32, 20),
            (38, 11),
            (39, 23),
            (41, 13),
            (42, 14),
            (43, 15),
            (44, 16),
            (45, 17),
            (46, 18),
            (47, 19),
            (48, 20),
            (49, 33),
            (51, 34),
            (52, 35),
            (53, 36),
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
            capacity_profile: [0x44; 32],
        })
        .expect("registered Buy bundle")
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

    #[test]
    fn capacity_or_artifact_substitution_refuses_exactly() {
        let bundle = build(736);
        assert_eq!(
            validate_direct_register_buy_hot_bundle_v4(&bundle, [0x45; 32]),
            Err(DirectRegisterBuyHotBundleErrorV4::Descriptor)
        );
        let mut hostile = bundle;
        *hostile.effect.get_mut(128).expect("effect byte") ^= 1;
        assert_eq!(
            validate_direct_register_buy_hot_bundle_v4(&hostile, [0x44; 32]),
            Err(DirectRegisterBuyHotBundleErrorV4::Descriptor)
        );
    }
}
