//! Content-addressed CapabilityProgram bundle for terminal Bearer redemption.

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as LIFECYCLE_SCHEMA_ID_V5, StateLifecyclePolicyV5,
    },
    v2::{AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE},
};
use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CAPABILITY_PROGRAM_V4_BYTES, CapabilityArtifactsV4, CapabilityProgramV4,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
    v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4},
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3,
    RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3, RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3,
    RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3,
};
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_token_svm::{
    TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use solana_program::hash::hash;

use crate::{
    Error, RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3, RATIONAL_TERMINAL_EFFECT_BYTES_V3,
    RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3, RATIONAL_TERMINAL_REQUEST_PROFILE_BYTES_V3,
    RATIONAL_TERMINAL_TRANSITION_BYTES_V3, RationalTerminalAccountProfileInputV3, Result,
    encode_rational_terminal_account_profile_v3, encode_rational_terminal_effect_v3,
    encode_rational_terminal_request_profile_v3, encode_rational_terminal_transition_v3,
};

/// Exact interpreted ExecutionStrategy record width.
pub const RATIONAL_TERMINAL_STRATEGY_BYTES_V3: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityProgramV4 descriptor width.
pub const RATIONAL_TERMINAL_DESCRIPTOR_BYTES_V3: usize = CAPABILITY_PROGRAM_V4_BYTES;

/// Release-owned semantic coordinates plus chain-derived AccountProfile facts.
///
/// The request, interpreter schemas, and every emitted artifact content ID are
/// fixed by this operator. The remaining coordinates are selected by the
/// immutable manifest entry and therefore cannot be invented by a generic
/// client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalTerminalHotBundleInputV3<'a> {
    /// Exact logical account widths and authenticated Product basis body.
    pub account_profile: RationalTerminalAccountProfileInputV3<'a>,
    /// Manifest-selected Bearer capability kind.
    pub kind: [u8; 32],
    /// Finalized descriptor/Market/config Token behavior admission.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
    /// Manifest-selected mutable root-tail schema.
    pub root_schema: [u8; 32],
    /// Exact finalized successor lifecycle policy bytes.
    pub lifecycle_policy: &'a [u8],
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// Exact mutable root-tail byte width.
    pub root_state_bytes: u32,
}

/// Exact bytes which must each become one finalized Registry record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalTerminalHotBundleV3 {
    /// Exact Realm/release-selected Token behavior config bytes.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// Runtime-width logical account interpreter.
    pub account_profile: [u8; RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3],
    /// Family request interpreter.
    pub request_profile: [u8; RATIONAL_TERMINAL_REQUEST_PROFILE_BYTES_V3],
    /// Exact successor lifecycle policy.
    pub lifecycle_policy: Vec<u8>,
    /// Economic transition interpreter.
    pub transition: [u8; RATIONAL_TERMINAL_TRANSITION_BYTES_V3],
    /// Interpreted strategy selecting the exact TransitionVM bytes.
    pub strategy: [u8; RATIONAL_TERMINAL_STRATEGY_BYTES_V3],
    /// One-route Claims effect interpreter.
    pub effect: [u8; RATIONAL_TERMINAL_EFFECT_BYTES_V3],
    /// Capability descriptor selecting every exact artifact content identity.
    pub descriptor: [u8; RATIONAL_TERMINAL_DESCRIPTOR_BYTES_V3],
}

/// Emit one complete terminal Bearer Hot bundle from checked semantic inputs.
pub fn build_rational_terminal_hot_bundle_v3(
    input: RationalTerminalHotBundleInputV3<'_>,
) -> Result<RationalTerminalHotBundleV3> {
    let account_profile = encode_rational_terminal_account_profile_v3(input.account_profile)?;
    let request_profile = encode_rational_terminal_request_profile_v3()?;
    let transition = encode_rational_terminal_transition_v3()?;
    let effect = encode_rational_terminal_effect_v3()?;
    let lifecycle_policy = Vec::from(input.lifecycle_policy);
    let lifecycle_id = digest(&lifecycle_policy)?;
    let transition_id = digest(&transition)?;
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
    .map_err(Error::ExecutionStrategy)?;
    let strategy = strategy_value.to_bytes();
    let token_behavior_selection = input.authenticated_token_behavior.selection().to_bytes();
    if hash(&token_behavior_selection).to_bytes()
        != input.authenticated_token_behavior.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    let descriptor_value = CapabilityProgramV4::new(
        content(input.kind)?,
        content(TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2)?,
        content(RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3)?,
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
    .map_err(Error::CapabilityDescriptor)?;
    let bundle = RationalTerminalHotBundleV3 {
        token_behavior_selection,
        account_profile,
        request_profile,
        lifecycle_policy,
        transition,
        strategy,
        effect,
        descriptor: descriptor_value.encode(),
    };
    validate_rational_terminal_hot_bundle_for_authenticated_selection_v3(
        &bundle,
        input.authenticated_token_behavior,
    )?;
    Ok(bundle)
}

/// Independently hostile-decode and join every emitted artifact.
pub fn validate_rational_terminal_hot_bundle_v3(
    bundle: &RationalTerminalHotBundleV3,
) -> Result<()> {
    let descriptor =
        CapabilityProgramV4::decode(&bundle.descriptor).map_err(Error::CapabilityDescriptor)?;
    TokenBehaviorSelectionV2::decode(&bundle.token_behavior_selection)
        .map_err(Error::TokenBehavior)?;
    let account =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfileArtifact)?;
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
    let effect = EffectProgramV4::decode(&bundle.effect).map_err(Error::EffectArtifactV4)?;
    let effect_base = effect.base();
    if descriptor.request_schema().to_bytes() != RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3
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
        || account.fixed_account_count() != RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3
        || account.item_account_stride() != 0
        || request
            .request_bytes(0)
            .map_err(Error::RequestProfileArtifact)?
            != RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3
        || request.item_request_bytes() != 0
        || effect.span_count() != 0
        || effect.range_count() != 0
        || usize::try_from(effect.semantic_prefix_bytes()).ok()
            != Some(RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3)
        || effect_base.fixed_account_count() != RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3
        || effect_base.item_account_stride() != 0
        || effect_base.route_count() != 1
        || effect_base.receipt_dependency_count() != 0
        || !geometry_matches(account, request, transition, effect_base)
    {
        return Err(Error::ArtifactGeometry);
    }
    let route = effect_base.route(0).map_err(Error::EffectArtifact)?;
    if route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.receipt_dependency_count() != 0
    {
        return Err(Error::ArtifactGeometry);
    }
    Ok(())
}

/// Validate the complete terminal bundle and bind its selected Token behavior
/// to independently authenticated Realm and release-set identities.
pub fn validate_rational_terminal_hot_bundle_for_authenticated_selection_v3(
    bundle: &RationalTerminalHotBundleV3,
    authenticated: AuthenticatedTokenBehaviorV2,
) -> Result<()> {
    validate_rational_terminal_hot_bundle_v3(bundle)?;
    if bundle.token_behavior_selection != authenticated.selection().to_bytes()
        || hash(&bundle.token_behavior_selection).to_bytes() != authenticated.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    Ok(())
}

fn geometry_matches(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
) -> bool {
    let scalars = u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3).ok();
    let identities = u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3).ok();
    scalars == Some(account.common_scalar_count())
        && scalars == Some(request.common_scalar_count())
        && scalars == Some(transition.common_scalar_count())
        && scalars == Some(effect.common_scalar_count())
        && identities == Some(account.common_identity_count())
        && identities == Some(request.common_identity_count())
        && identities == Some(transition.common_identity_count())
        && identities == Some(effect.common_identity_count())
        && account.item_scalar_stride() == 0
        && request.item_scalar_stride() == 0
        && transition.item_scalar_stride() == 0
        && effect.item_scalar_stride() == 0
        && account.item_identity_stride() == 0
        && request.item_identity_stride() == 0
        && transition.item_identity_stride() == 0
        && effect.item_identity_stride() == 0
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
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };

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
        .expect("basis");
        output
    }

    fn input<'a>(basis: &'a [u8], lengths: &'a [u32]) -> RationalTerminalHotBundleInputV3<'a> {
        RationalTerminalHotBundleInputV3 {
            account_profile: RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: lengths,
                product_basis: basis,
            },
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

    fn lengths(basis: &[u8]) -> [u32; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize] {
        let mut output = [0_u32; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
        let width = u32::try_from(basis.len()).expect("basis length");
        *output.get_mut(4).expect("basis coordinate") = width;
        *output.get_mut(29).expect("basis alias coordinate") = width;
        output
    }

    #[test]
    fn complete_bundle_joins_exact_interpreted_artifacts() {
        let basis = basis(258, 1);
        let lengths = lengths(&basis);
        let bundle = build_rational_terminal_hot_bundle_v3(input(&basis, &lengths))
            .expect("complete bundle");
        validate_rational_terminal_hot_bundle_v3(&bundle).expect("joined bundle");
        let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).expect("descriptor");
        assert_eq!(
            descriptor.request_schema().to_bytes(),
            RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3
        );
        assert_eq!(
            descriptor.account_profile().program().to_bytes(),
            digest(&bundle.account_profile).expect("id").to_bytes()
        );
        assert_eq!(
            descriptor.effect().program().to_bytes(),
            digest(&bundle.effect).expect("id").to_bytes()
        );
    }

    #[test]
    fn substituted_artifact_cannot_reuse_descriptor() {
        let canonical_basis = basis(258, 1);
        let canonical_lengths = lengths(&canonical_basis);
        let canonical =
            build_rational_terminal_hot_bundle_v3(input(&canonical_basis, &canonical_lengths))
                .expect("complete bundle");
        let mut substituted = canonical.clone();
        *substituted
            .account_profile
            .get_mut(0)
            .expect("profile magic") ^= 1;
        assert!(validate_rational_terminal_hot_bundle_v3(&substituted).is_err());

        // Same-width Product substitution is not copied into this artifact:
        // common Hot independently authenticates logical coordinate four.
        let other_basis = basis(258, 9);
        let other_lengths = lengths(&other_basis);
        let other = build_rational_terminal_hot_bundle_v3(input(&other_basis, &other_lengths))
            .expect("generic profile");
        assert_eq!(canonical.account_profile, other.account_profile);
    }

    #[test]
    fn zero_semantic_coordinate_refuses_before_output() {
        let basis = basis(258, 1);
        let lengths = lengths(&basis);
        let mut invalid = input(&basis, &lengths);
        invalid.capacity_profile = [0; 32];
        assert_eq!(
            build_rational_terminal_hot_bundle_v3(invalid),
            Err(Error::ContentIdentity)
        );
    }
}
