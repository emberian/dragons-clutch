//! Complete schema-bound Hot artifact bundle for inline ordinary Direct.
//!
//! This host-side emitter is the sole Direct-specific artifact builder. The
//! runtime Trading program remains family-neutral: it authenticates these
//! records, projects their shared interpreters, executes fixed Claims/Custody
//! routes, and commits once.

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
    v3::ProgramV3 as EffectProgramV3,
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
        DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3, DIRECT_SUCCESSOR_KIND_ID_V3, DirectExecutionActionV3,
    },
    ordinary_account_artifacts_v3::{
        DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3, DirectInlineOrdinaryAccountProfileInputV3,
        encode_direct_inline_ordinary_account_profile_v3_atomic,
    },
    ordinary_artifacts_v3::{
        DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3,
        DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3, direct_inline_ordinary_strategy_v3,
        encode_inline_ordinary_request_profile_v3_atomic,
    },
    ordinary_effect_artifacts_v3::{
        DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
        encode_direct_inline_ordinary_effect_v4_atomic,
    },
    ordinary_v3::{
        DIRECT_ORDINARY_COMMON_IDENTITIES_V3, DIRECT_ORDINARY_COMMON_SCALARS_V3,
        DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3, DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
        DIRECT_ORDINARY_TRANSITION_BYTES_V3, encode_direct_ordinary_transition_v3,
    },
    state_artifacts_v3::{
        DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5,
        encode_direct_inline_ordinary_lifecycle_v5_atomic,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
    },
};

/// Exact interpreted ExecutionStrategy record width.
pub const DIRECT_INLINE_ORDINARY_STRATEGY_BYTES_V3: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityProgram descriptor width.
pub const DIRECT_INLINE_ORDINARY_DESCRIPTOR_BYTES_V4: usize = CAPABILITY_PROGRAM_V4_BYTES;
/// SHA-256 identity of the exact runtime-polymorphic fixed-topology AccountProfile14.
pub const DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3: [u8; 32] = [
    0xff, 0xf7, 0xc4, 0xaa, 0xf1, 0x0a, 0xe6, 0x6b, 0x4a, 0xd0, 0x9d, 0xfb, 0x58, 0xce, 0x7b, 0xe6,
    0x09, 0xcf, 0x84, 0x78, 0xc2, 0x40, 0xb7, 0x08, 0x09, 0x59, 0xec, 0x34, 0x01, 0xea, 0x23, 0x77,
];
/// SHA-256 identity of the exact maker LifecycleV5 policy.
pub const DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5: [u8; 32] = [
    0x19, 0x3b, 0xe6, 0xe3, 0xb1, 0x1e, 0x70, 0x88, 0x31, 0xc4, 0xe0, 0xa8, 0x41, 0xdf, 0xe9, 0x8c,
    0x0b, 0xd7, 0x09, 0xa9, 0x07, 0x23, 0xd1, 0xf1, 0x93, 0x5d, 0xf2, 0xb3, 0x3d, 0xc5, 0x85, 0xbc,
];
/// SHA-256 identity of the exact ordered EffectProgramV4.
pub const DIRECT_INLINE_ORDINARY_EFFECT_ID_V4: [u8; 32] = [
    0xac, 0xd5, 0x5d, 0x27, 0x8e, 0x58, 0x4d, 0x81, 0x56, 0xdf, 0x9b, 0x6b, 0xe5, 0x1b, 0xeb, 0xba,
    0x57, 0x72, 0x74, 0x9b, 0x0b, 0xda, 0x2a, 0xbb, 0x2f, 0x9b, 0x5a, 0xe0, 0xf5, 0x9f, 0xa1, 0x4b,
];

/// Chain-selected facts that are not owned by the Direct artifact family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryHotBundleInputV4<'a> {
    /// Exact logical account observations used to validate runtime-width rules.
    pub account_profile: DirectInlineOrdinaryAccountProfileInputV3<'a>,
    /// Manifest-selected physical capacity profile content identity.
    pub capacity_profile: [u8; 32],
}

/// Every finalized record selected by one ordinary CapabilityProgram.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryHotBundleV4 {
    /// Runtime-width fixed-topology AccountProfile14 bytes.
    pub account_profile: [u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3],
    /// Maker AuthenticateOrCreate LifecycleV5 bytes.
    pub lifecycle_policy: [u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5],
    /// Signed RequestProfileV2 bytes.
    pub request_profile: [u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3],
    /// TransitionVMV3 economic program bytes.
    pub transition: [u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3],
    /// Interpreted strategy selecting the transition.
    pub strategy: [u8; DIRECT_INLINE_ORDINARY_STRATEGY_BYTES_V3],
    /// Ordered Sparse Claims plus delegated Custody EffectV4 bytes.
    pub effect: [u8; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4],
    /// Descriptor joining every artifact above.
    pub descriptor: [u8; DIRECT_INLINE_ORDINARY_DESCRIPTOR_BYTES_V4],
}

/// Stable bundle emission or hostile-validation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineOrdinaryHotBundleErrorV4 {
    /// A nonzero finalized content identity or fixed width was invalid.
    Content,
    /// AccountProfile construction or decoding refused.
    AccountProfile,
    /// Lifecycle construction, decoding, or AccountProfile join refused.
    Lifecycle,
    /// RequestProfile construction or decoding refused.
    RequestProfile,
    /// Transition construction or decoding refused.
    Transition,
    /// Strategy construction or descriptor join refused.
    Strategy,
    /// Effect construction or decoding refused.
    Effect,
    /// Descriptor construction or exact content join refused.
    Descriptor,
    /// Artifact geometries did not agree exactly.
    Geometry,
}

/// Emit and independently hostile-check one complete ordinary Hot bundle.
pub fn build_direct_inline_ordinary_hot_bundle_v4(
    input: DirectInlineOrdinaryHotBundleInputV4<'_>,
) -> Result<DirectInlineOrdinaryHotBundleV4, DirectInlineOrdinaryHotBundleErrorV4> {
    let mut account_scratch = [0_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
    let mut account_profile = [0_u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3];
    encode_direct_inline_ordinary_account_profile_v3_atomic(
        input.account_profile,
        &mut account_scratch,
        &mut account_profile,
    )
    .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::AccountProfile)?;

    let mut lifecycle_scratch = [0_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5];
    let mut lifecycle_policy = [0_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V5];
    encode_direct_inline_ordinary_lifecycle_v5_atomic(
        &mut lifecycle_scratch,
        &mut lifecycle_policy,
    )
    .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Lifecycle)?;

    let mut request_v1_scratch = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3];
    let mut request_v1 = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V1_BYTES_V3];
    let mut request_v2_scratch = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3];
    let mut request_profile = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_V2_BYTES_V3];
    encode_inline_ordinary_request_profile_v3_atomic(
        &mut request_v1_scratch,
        &mut request_v1,
        &mut request_v2_scratch,
        &mut request_profile,
    )
    .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::RequestProfile)?;

    let mut transition_scratch = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
    let mut transition = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
    encode_direct_ordinary_transition_v3(&mut transition_scratch, &mut transition)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Transition)?;
    let strategy = direct_inline_ordinary_strategy_v3()
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Strategy)?;

    let mut effect_scratch = [0_u8; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4];
    let mut effect = [0_u8; DIRECT_INLINE_ORDINARY_EFFECT_BYTES_V4];
    encode_direct_inline_ordinary_effect_v4_atomic(&mut effect_scratch, &mut effect)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Effect)?;

    let account_id = digest(&account_profile);
    let lifecycle_id = digest(&lifecycle_policy);
    let request_id = digest(&request_profile);
    let transition_id = digest(&transition);
    let strategy_id = digest(&strategy);
    let effect_id = digest(&effect);
    let descriptor_value = CapabilityProgramV4::new(
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
            .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Geometry)?,
    )
    .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Descriptor)?;
    let bundle = DirectInlineOrdinaryHotBundleV4 {
        account_profile,
        lifecycle_policy,
        request_profile,
        transition,
        strategy,
        effect,
        descriptor: descriptor_value.encode(),
    };
    validate_direct_inline_ordinary_hot_bundle_v4(&bundle, input.capacity_profile)?;
    Ok(bundle)
}

/// Hostile-decode and join every artifact selected by one ordinary descriptor.
pub fn validate_direct_inline_ordinary_hot_bundle_v4(
    bundle: &DirectInlineOrdinaryHotBundleV4,
    capacity_profile: [u8; 32],
) -> Result<(), DirectInlineOrdinaryHotBundleErrorV4> {
    let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Descriptor)?;
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
                .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Geometry)?
    {
        return Err(DirectInlineOrdinaryHotBundleErrorV4::Descriptor);
    }
    let account = AccountProfileV2::decode(&bundle.account_profile)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::AccountProfile)?;
    let lifecycle_id = digest(&bundle.lifecycle_policy);
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        descriptor.lifecycle().program().to_bytes(),
        lifecycle_id,
        &bundle.lifecycle_policy,
    )
    .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Lifecycle)?;
    lifecycle
        .validate_account_profile(account)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Lifecycle)?;
    if lifecycle
        .action_plan_count(DirectExecutionActionV3::InlineOrdinary as u32)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Lifecycle)?
        != 2
    {
        return Err(DirectInlineOrdinaryHotBundleErrorV4::Lifecycle);
    }
    let request_id = digest(&bundle.request_profile);
    let request = RequestProfileV2::decode_selected(
        descriptor.request_profile().program().to_bytes(),
        request_id,
        &bundle.request_profile,
    )
    .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::RequestProfile)?;
    let transition = TransitionProgramV3::decode(&bundle.transition)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Strategy)?;
    if strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
    {
        return Err(DirectInlineOrdinaryHotBundleErrorV4::Strategy);
    }
    let effect = EffectProgramV4::decode(&bundle.effect)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Effect)?;
    if effect.span_count() != 0
        || effect.range_count() != 0
        || effect.semantic_prefix_bytes()
            != u32::try_from(crate::execution_v3::DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3)
                .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Geometry)?
    {
        return Err(DirectInlineOrdinaryHotBundleErrorV4::Effect);
    }
    validate_geometry(account, request, transition, effect.base())
}

fn validate_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileV2<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> Result<(), DirectInlineOrdinaryHotBundleErrorV4> {
    let request = request.request_profile();
    let fixed_accounts = DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3;
    let common_scalars = u16::try_from(DIRECT_ORDINARY_COMMON_SCALARS_V3)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Geometry)?;
    let common_identities = u16::try_from(DIRECT_ORDINARY_COMMON_IDENTITIES_V3)
        .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Geometry)?;
    if account.fixed_account_count() != fixed_accounts
        || account.item_account_stride() != 0
        || account.common_scalar_count() != common_scalars
        || account.item_scalar_stride() != DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3
        || account.common_identity_count() != common_identities
        || account.item_identity_stride() != DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3
        || request.common_scalar_count() != common_scalars
        || request.item_scalar_stride() != DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3
        || request.common_identity_count() != common_identities
        || request.item_identity_stride() != DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3
        || transition.common_scalar_count() != common_scalars
        || transition.item_scalar_stride() != DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3
        || transition.common_identity_count() != common_identities
        || transition.item_identity_stride() != DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3
        || effect.fixed_account_count() != fixed_accounts
        || effect.item_account_stride() != 0
        || effect.common_scalar_count() != common_scalars
        || effect.item_scalar_stride() != DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3
        || effect.common_identity_count() != common_identities
        || effect.item_identity_stride() != DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3
    {
        return Err(DirectInlineOrdinaryHotBundleErrorV4::Geometry);
    }
    Ok(())
}

fn content(bytes: [u8; 32]) -> Result<ContentId, DirectInlineOrdinaryHotBundleErrorV4> {
    ContentId::new(bytes).map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Content)
}

fn artifact(
    schema: [u8; 32],
    program: [u8; 32],
) -> Result<ArtifactReferenceV4, DirectInlineOrdinaryHotBundleErrorV4> {
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

    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::SUCCESSOR_SCHEMA_RELEASE_ID;
    use dclutch_account_profile_contract::{
        EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS, lifecycle_v3,
        v2::AccountPrestateV2,
    };
    use dclutch_capability_program_contract::v4::CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET;
    use dclutch_claims_svm::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    };
    use dclutch_custody_contract::CustodyReplayLayoutV1;
    use dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3;
    use dclutch_product_runtime_v2::{
        DOMAIN_CUT_BYTES, DOMAIN_HEADER_BYTES, PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES,
    };
    use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
    use dclutch_realm_contract::REALM_BYTES;
    use dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
    use dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES;
    use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;

    use crate::{
        execution_v3::DirectExecutionActionV3,
        ordinary_effect_artifacts_v3::DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3,
        state_artifacts_v3::{
            DIRECT_BUYER_MAKER_ACCOUNT_V3, DIRECT_LIFECYCLE_RENT_CREDIT_ACCOUNT_V3,
            DIRECT_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V3, DIRECT_MAKER_PAYER_ACCOUNT_V3,
            DIRECT_MAKER_PAYER_ROUTE_ALIAS_ACCOUNT_V3, DIRECT_SELLER_MAKER_ACCOUNT_V3,
        },
    };
    use std::{vec, vec::Vec};

    fn logical_lengths(basis_bytes: u32) -> Vec<u32> {
        let mut output = vec![0_u32; usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)];
        let root = dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
            + DIRECT_ROOT_STATE_BYTES_V1;
        *output.get_mut(0).expect("root") = u32::try_from(root).expect("root width");
        *output.get_mut(1).expect("config") =
            u32::try_from(crate::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1).expect("config");
        *output.get_mut(2).expect("Product") = u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("p");
        *output.get_mut(3).expect("portfolio") =
            u32::try_from(PORTFOLIO_HEADER_BYTES + 3 * PORTFOLIO_COEFFICIENT_BYTES)
                .expect("portfolio");
        *output.get_mut(4).expect("basis") = basis_bytes;
        for coordinate in [5_usize, 8] {
            *output.get_mut(coordinate).expect("maker") =
                u32::try_from(crate::successor::DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker");
        }
        *output.get_mut(7).expect("lifecycle RentCredit") =
            u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit width");
        *output.get_mut(10).expect("Rent program") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Rent program width");
        *output.get_mut(13).expect("claims aggregate") =
            u32::try_from(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 3 * 8).expect("aggregate");
        *output.get_mut(14).expect("basis alias") = basis_bytes;
        *output.get_mut(16).expect("Product alias") =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("p");
        *output.get_mut(18).expect("domain") =
            u32::try_from(DOMAIN_HEADER_BYTES - 2 * DOMAIN_CUT_BYTES + 3 * DOMAIN_CUT_BYTES)
                .expect("domain");
        *output.get_mut(20).expect("portfolio alias") = *output.get(3).expect("portfolio");
        *output.get_mut(22).expect("registry") = 17;
        *output.get_mut(23).expect("Core") =
            u32::try_from(dclutch_market_core_codec::STATE_BYTES).expect("Core");
        *output.get_mut(24).expect("activation") =
            u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation");
        *output.get_mut(25).expect("Registry program") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Registry program");
        *output.get_mut(26).expect("Trading program") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Trading program");
        *output.get_mut(27).expect("program data") = 1_024;
        *output.get_mut(28).expect("Claims program") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Claims program");
        *output.get_mut(29).expect("source staging") = 1_024;
        *output.get_mut(30).expect("Core program") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Core program");
        *output.get_mut(31).expect("destination staging") = 1_024;
        let position =
            u32::try_from(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 3 * 8).expect("position");
        *output.get_mut(32).expect("source Position") = position;
        *output.get_mut(33).expect("destination Position") = position;
        *output.get_mut(35).expect("Core alias") = *output.get(23).expect("Core");
        *output.get_mut(36).expect("activation alias") = *output.get(24).expect("activation");
        *output.get_mut(37).expect("registry alias") = *output.get(25).expect("registry");
        *output.get_mut(38).expect("program alias") = *output.get(26).expect("program");
        *output.get_mut(39).expect("pdata alias") = *output.get(27).expect("pdata");
        *output.get_mut(40).expect("Realm") = u32::try_from(REALM_BYTES).expect("Realm");
        *output.get_mut(42).expect("replay") =
            u32::try_from(CustodyReplayLayoutV1::BYTES).expect("replay");
        *output.get_mut(43).expect("mint") = 82;
        *output.get_mut(44).expect("buyer token") = 165;
        *output.get_mut(45).expect("seller token") = 165;
        *output.get_mut(47).expect("token program") = 36;
        *output.get_mut(73).expect("fee token") = 165;
        // Descriptive only: the Custody program rule is opaque, so no loader's
        // record width is pinned here.
        *output
            .get_mut(usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3))
            .expect("Custody program") =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Custody program");
        for (account, representative) in [
            (49, 23),
            (50, 24),
            (51, 25),
            (52, 26),
            (53, 27),
            (54, 40),
            (55, 41),
            (56, 42),
            (57, 43),
            (58, 44),
            (59, 45),
            (60, 46),
            (61, 47),
            (63, 23),
            (64, 24),
            (65, 25),
            (66, 26),
            (67, 27),
            (68, 40),
            (69, 41),
            (70, 42),
            (71, 43),
            (72, 44),
            (74, 46),
            (75, 47),
            (77, 23),
            (78, 24),
            (79, 25),
            (80, 26),
            (81, 27),
            (82, 40),
            (83, 41),
            (84, 42),
            (85, 43),
            (86, 44),
            (87, 73),
            (88, 46),
            (89, 47),
        ] {
            let value = *output.get(representative).expect("representative");
            *output.get_mut(account).expect("alias") = value;
        }
        output
    }

    fn build(basis_bytes: u32) -> DirectInlineOrdinaryHotBundleV4 {
        let lengths = logical_lengths(basis_bytes);
        build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
            account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                logical_data_lengths: &lengths,
            },
            capacity_profile: [0x44; 32],
        })
        .expect("bundle")
    }

    #[test]
    fn one_descriptor_is_polymorphic_across_categorical_and_graded_basis() {
        let categorical = build(u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("categorical"));
        let graded = build(736);
        assert_eq!(categorical, graded);
        assert_eq!(
            [
                digest(&categorical.account_profile),
                digest(&categorical.lifecycle_policy),
                digest(&categorical.effect),
            ],
            [
                DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3,
                DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5,
                DIRECT_INLINE_ORDINARY_EFFECT_ID_V4,
            ]
        );
        validate_direct_inline_ordinary_hot_bundle_v4(&categorical, [0x44; 32]).expect("validate");
    }

    #[test]
    fn capacity_or_effect_substitution_refuses_exactly() {
        let bundle = build(256);
        assert_eq!(
            validate_direct_inline_ordinary_hot_bundle_v4(&bundle, [0x45; 32]),
            Err(DirectInlineOrdinaryHotBundleErrorV4::Descriptor)
        );
        let mut hostile = bundle;
        *hostile.effect.get_mut(128).expect("effect byte") ^= 1;
        assert_eq!(
            validate_direct_inline_ordinary_hot_bundle_v4(&hostile, [0x44; 32]),
            Err(DirectInlineOrdinaryHotBundleErrorV4::Descriptor)
        );
    }

    /// The two artifacts the adapter joins agree on which coordinate owns each
    /// lifecycle authority, and each named coordinate is the semantic owner of
    /// that authority rather than a route alias of it.
    #[test]
    fn every_lifecycle_plan_names_a_representative_carrying_its_effect_authority() {
        let bundle = build(256);
        let profile = AccountProfileV2::decode(&bundle.account_profile).expect("profile");
        let policy_id = digest(&bundle.lifecycle_policy);
        let policy =
            StateLifecyclePolicyV5::decode_selected(policy_id, policy_id, &bundle.lifecycle_policy)
                .expect("policy");
        policy.validate_account_profile(profile).expect("join");
        let action = DirectExecutionActionV3::InlineOrdinary as u32;
        assert_eq!(policy.action_plan_count(action).expect("plans"), 2);
        for (ordinal, state) in [
            (0_u16, DIRECT_SELLER_MAKER_ACCOUNT_V3),
            (1, DIRECT_BUYER_MAKER_ACCOUNT_V3),
        ] {
            let indices = policy
                .action_plan(action, ordinal)
                .expect("plan")
                .project_account_indices(profile, 3, None)
                .expect("indices");
            assert_eq!(indices.state(), usize::from(state));
            assert_eq!(
                indices.payer(),
                Some(usize::from(DIRECT_MAKER_PAYER_ACCOUNT_V3))
            );
            assert_eq!(
                indices.rent_credit(),
                Some(usize::from(DIRECT_LIFECYCLE_RENT_CREDIT_ACCOUNT_V3))
            );
        }
        // Exactly the predicates `require_permissions` applies to the named
        // coordinate's own rule, which never follows a route alias.
        let payer = profile
            .rule(false, DIRECT_MAKER_PAYER_ACCOUNT_V3)
            .expect("payer");
        assert_ne!(
            payer.effect_permissions() & EFFECT_PERMISSION_DEBIT_LAMPORTS,
            0
        );
        assert_ne!(payer.prestate(), AccountPrestateV2::AuthenticatedRouteAlias);
        let credit = profile
            .rule(false, DIRECT_LIFECYCLE_RENT_CREDIT_ACCOUNT_V3)
            .expect("credit");
        assert_ne!(
            credit.effect_permissions() & EFFECT_PERMISSION_CREDIT_LAMPORTS,
            0
        );
        assert_ne!(
            credit.prestate(),
            AccountPrestateV2::AuthenticatedRouteAlias
        );
    }

    /// Reversion evidence for the payer defect: the buyer plan as it stood
    /// before this repair named coordinate 9, the route alias of the sole
    /// payer. `require_permissions` reads the named rule and never follows the
    /// alias, so the alias's zero effect permissions were exactly the refusal
    /// the adapter raised mid-preplan. The join now refuses it up front, and
    /// refuses it on either funding field.
    #[test]
    fn a_plan_that_names_an_alias_coordinate_refuses_the_join() {
        let bundle = build(256);
        let profile = AccountProfileV2::decode(&bundle.account_profile).expect("profile");
        assert_eq!(
            profile
                .rule(false, DIRECT_MAKER_PAYER_ROUTE_ALIAS_ACCOUNT_V3)
                .expect("alias")
                .effect_permissions(),
            0
        );
        let plan_table = lifecycle_v3::HEADER_BYTES
            + 2 * lifecycle_v3::RECIPE_BYTES
            + 10 * lifecycle_v3::SEED_BYTES;
        let buyer_plan = plan_table + lifecycle_v3::ACTION_PLAN_BYTES;
        for field_offset in [8_usize, 12] {
            let mut hostile = bundle.lifecycle_policy;
            hostile
                .get_mut(buyer_plan + field_offset + 2..buyer_plan + field_offset + 4)
                .expect("coordinate index")
                .copy_from_slice(&DIRECT_MAKER_PAYER_ROUTE_ALIAS_ACCOUNT_V3.to_le_bytes());
            let hostile_id = digest(&hostile);
            let policy = StateLifecyclePolicyV5::decode_selected(hostile_id, hostile_id, &hostile)
                .expect("hostile policy remains decodable");
            assert_eq!(
                policy.validate_account_profile(profile),
                Err(lifecycle_v3::Error::ProfileMismatch),
                "funding field at plan offset {field_offset}"
            );
        }
    }

    /// The Rent program coordinate that replaced the second V1 credit carries
    /// no lifecycle authority at all, so no plan can fund or close through it.
    #[test]
    fn the_rent_program_coordinate_carries_no_lifecycle_authority() {
        let bundle = build(256);
        let profile = AccountProfileV2::decode(&bundle.account_profile).expect("profile");
        let rule = profile
            .rule(false, DIRECT_LIFECYCLE_RENT_PROGRAM_ACCOUNT_V3)
            .expect("Rent program");
        assert_eq!(rule.effect_permissions(), 0);
        assert!(rule.route_privileges().executable());
        assert!(!rule.route_privileges().writable());
        assert_eq!(
            rule.prestate(),
            AccountPrestateV2::AuthenticatedOpaqueReadonlyData
        );
    }

    #[test]
    fn lifecycle_v4_schema_substitution_has_no_successor_fallback() {
        let mut hostile = build(256);
        hostile.descriptor[CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET
            ..CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET + 32]
            .copy_from_slice(&SUCCESSOR_SCHEMA_RELEASE_ID);
        assert_eq!(
            validate_direct_inline_ordinary_hot_bundle_v4(&hostile, [0x44; 32]),
            Err(DirectInlineOrdinaryHotBundleErrorV4::Descriptor)
        );
    }
}
