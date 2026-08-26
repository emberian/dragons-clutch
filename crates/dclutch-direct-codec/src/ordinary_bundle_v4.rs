//! Complete schema-bound Hot artifact bundle for inline ordinary Direct.
//!
//! This host-side emitter is the sole Direct-specific artifact builder. The
//! runtime Trading program remains family-neutral: it authenticates these
//! records, projects their shared interpreters, executes fixed Claims/Custody
//! routes, and commits once.

use dclutch_account_profile_contract::{
    lifecycle_v3::{SUCCESSOR_SCHEMA_RELEASE_ID as LIFECYCLE_SCHEMA_ID_V4, StateLifecyclePolicyV4},
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2},
};
use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v3::{
    ProgramV3 as EffectProgramV3, SCHEMA_RELEASE_ID as EFFECT_SCHEMA_ID_V4,
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
        DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V4,
        encode_direct_inline_ordinary_lifecycle_v4_atomic,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
    },
};

/// Exact interpreted ExecutionStrategy record width.
pub const DIRECT_INLINE_ORDINARY_STRATEGY_BYTES_V3: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityProgram descriptor width.
pub const DIRECT_INLINE_ORDINARY_DESCRIPTOR_BYTES_V4: usize = CAPABILITY_PROGRAM_V4_BYTES;
/// SHA-256 identity of the exact runtime-polymorphic AccountProfile11.
pub const DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3: [u8; 32] = [
    0x7f, 0x00, 0x01, 0xa9, 0xb6, 0xeb, 0xf2, 0xa3, 0x95, 0xc6, 0xae, 0xf9, 0x2a, 0xed, 0x04, 0x66,
    0x20, 0xb2, 0x2e, 0xa5, 0x14, 0x48, 0x9c, 0x65, 0x3d, 0x01, 0x99, 0xde, 0x4b, 0x8e, 0x10, 0x24,
];
/// SHA-256 identity of the exact maker LifecycleV4 policy.
pub const DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V4: [u8; 32] = [
    0x7c, 0x16, 0x3e, 0xcb, 0xe0, 0x99, 0xf2, 0x8f, 0xb8, 0x4f, 0x1a, 0x85, 0xb8, 0xee, 0xe4, 0xf1,
    0x6e, 0x9d, 0x5f, 0x25, 0x09, 0xa8, 0xc9, 0x18, 0x54, 0x3f, 0xa5, 0x7b, 0x04, 0x88, 0x31, 0xd2,
];
/// SHA-256 identity of the exact ordered EffectProgramV4.
pub const DIRECT_INLINE_ORDINARY_EFFECT_ID_V4: [u8; 32] = [
    0xee, 0xc9, 0xbb, 0xfd, 0x76, 0x7d, 0x60, 0x01, 0x10, 0x98, 0x1d, 0xc8, 0x79, 0x46, 0x47, 0x41,
    0x82, 0xb0, 0xd6, 0x40, 0xf8, 0x40, 0xea, 0xda, 0x65, 0x6a, 0x68, 0x58, 0xd3, 0x42, 0x59, 0xa6,
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
    /// Runtime-width AccountProfile11 bytes.
    pub account_profile: [u8; DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_BYTES_V3],
    /// Maker AuthenticateOrCreate LifecycleV4 bytes.
    pub lifecycle_policy: [u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V4],
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

    let mut lifecycle_scratch = [0_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V4];
    let mut lifecycle_policy = [0_u8; DIRECT_INLINE_ORDINARY_LIFECYCLE_BYTES_V4];
    encode_direct_inline_ordinary_lifecycle_v4_atomic(
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
            lifecycle: artifact(LIFECYCLE_SCHEMA_ID_V4, lifecycle_id)?,
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
            != artifact(LIFECYCLE_SCHEMA_ID_V4, digest(&bundle.lifecycle_policy))?
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
    let lifecycle = StateLifecyclePolicyV4::decode_selected(
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
    let effect_id = digest(&bundle.effect);
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect().program().to_bytes(),
        effect_id,
        &bundle.effect,
    )
    .map_err(|_| DirectInlineOrdinaryHotBundleErrorV4::Effect)?;
    validate_geometry(account, request, transition, effect)
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
        *output.get_mut(7).expect("seller rent") = 64;
        *output.get_mut(10).expect("buyer rent") = 64;
        *output.get_mut(13).expect("claims aggregate") =
            u32::try_from(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 3 * 8).expect("aggregate");
        *output.get_mut(14).expect("basis alias") = basis_bytes;
        *output.get_mut(16).expect("Product alias") =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("p");
        *output.get_mut(18).expect("domain") =
            u32::try_from(DOMAIN_HEADER_BYTES - DOMAIN_CUT_BYTES + 3 * DOMAIN_CUT_BYTES)
                .expect("domain");
        *output.get_mut(20).expect("portfolio alias") = *output.get(3).expect("portfolio");
        *output.get_mut(22).expect("registry") = 17;
        *output.get_mut(23).expect("Core") = 352;
        *output.get_mut(24).expect("activation") = 128;
        *output.get_mut(25).expect("registry cache") = 36;
        *output.get_mut(26).expect("program") = 36;
        *output.get_mut(27).expect("program data") = 1_024;
        *output.get_mut(28).expect("source admission") = 36;
        *output.get_mut(29).expect("source staging") = 1_024;
        *output.get_mut(30).expect("destination admission") = 36;
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
                DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V4,
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
}
