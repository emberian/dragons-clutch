//! CapabilityV4/LifecycleV5 bundles for fixed-cardinality lifecycle actions.

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
    v3::RouteKindV3,
    v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4},
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_rational_representation_v2_contract::AuthenticatedTokenBehaviorV2;
use dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2,
    hot_v3::{RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3, RationalLifecycleHotLayoutV3},
};
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_token_svm::{
    TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use solana_program::hash::hash;

use crate::{
    Error, RationalLifecycleSelectedAccountProfileInputV5, Result,
    artifacts::{
        encode_rational_lifecycle_selected_request_profile_v5,
        encode_rational_lifecycle_transition_v3,
    },
    effect::{
        encode_rational_lifecycle_selected_effect_v4, lifecycle_claims_account_count_v3,
        lifecycle_logical_account_count_v3,
    },
    selected_profile_v5::encode_rational_lifecycle_selected_account_profile_v5,
};

/// Exact selected strategy width.
pub const RATIONAL_LIFECYCLE_SELECTED_STRATEGY_BYTES_V5: usize =
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityV4 descriptor width.
pub const RATIONAL_LIFECYCLE_SELECTED_DESCRIPTOR_BYTES_V5: usize = CAPABILITY_PROGRAM_V4_BYTES;

/// Same-finalized authority for one selected fixed-cardinality lifecycle bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleSelectedBundleInputV5<'a> {
    /// Exact fixed-cardinality lifecycle action. Complete retirement uses compact V4.
    pub action: LifecycleActionV2,
    /// Exact Profile13 logical observations.
    pub account_profile: RationalLifecycleSelectedAccountProfileInputV5<'a>,
    /// Finalized immutable representation descriptor owning width `K`.
    pub representation_descriptor: RepresentationDescriptorV2<'a>,
    /// Realm/release/descriptor-authenticated Token behavior selection.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
    /// Manifest-selected lifecycle capability kind.
    pub kind: [u8; 32],
    /// Manifest-selected mutable root schema.
    pub root_schema: [u8; 32],
    /// Exact finalized StateLifecyclePolicyV5 bytes.
    pub lifecycle_policy: &'a [u8],
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// Exact mutable root bytes.
    pub root_state_bytes: u32,
}

/// Exact bytes finalized as one selected fixed-cardinality capability bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleSelectedBundleV5 {
    /// Exact selected lifecycle action.
    pub action: LifecycleActionV2,
    /// Finalized representation descriptor identity baked into RequestProfile.
    pub representation_descriptor_id: [u8; 32],
    /// Exact release set baked into RequestProfile.
    pub release_set: [u8; 32],
    /// Exact Token program baked into RequestProfile.
    pub token_program: [u8; 32],
    /// Exact Realm/release-selected Token behavior config bytes.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// Exact Profile13 account interpreter.
    pub account_profile: Vec<u8>,
    /// Descriptor/release/Token-bound request interpreter.
    pub request_profile: Vec<u8>,
    /// Exact economic transition interpreter.
    pub transition: Vec<u8>,
    /// Exact successor lifecycle policy.
    pub lifecycle_policy: Vec<u8>,
    /// Exact interpreted strategy.
    pub strategy: [u8; RATIONAL_LIFECYCLE_SELECTED_STRATEGY_BYTES_V5],
    /// Exact EffectV4 Claims route.
    pub effect: Vec<u8>,
    /// Exact CapabilityV4 descriptor.
    pub descriptor: [u8; RATIONAL_LIFECYCLE_SELECTED_DESCRIPTOR_BYTES_V5],
}

/// Build one descriptor-specific CapabilityV4/LifecycleV5/Profile13 bundle.
pub fn build_rational_lifecycle_selected_bundle_v5(
    input: RationalLifecycleSelectedBundleInputV5<'_>,
) -> Result<RationalLifecycleSelectedBundleV5> {
    let coordinate_count = coordinate_count(input.action)?;
    let representation_descriptor_id = input.representation_descriptor.descriptor_id();
    let release_set = input.representation_descriptor.release_set_id();
    let token_program = input.representation_descriptor.token_program();
    if representation_descriptor_id != input.authenticated_token_behavior.descriptor_id()
        || release_set != input.authenticated_token_behavior.selection().release_set()
        || token_program
            != input
                .authenticated_token_behavior
                .selection()
                .token_program()
    {
        return Err(Error::ArtifactGeometry);
    }
    let account_profile =
        encode_rational_lifecycle_selected_account_profile_v5(input.action, input.account_profile)?;
    let request_profile = encode_rational_lifecycle_selected_request_profile_v5(
        input.action,
        representation_descriptor_id,
        release_set,
        token_program,
    )?;
    let transition = encode_rational_lifecycle_transition_v3(input.action, coordinate_count)?;
    let effect = encode_rational_lifecycle_selected_effect_v4(input.action)?;
    let lifecycle_policy = Vec::from(input.lifecycle_policy);
    let lifecycle_id = digest(&lifecycle_policy)?;
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
    .map_err(Error::Strategy)?;
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
        content(RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3)?,
        content(input.root_schema)?,
        lifecycle_id,
        content(input.capacity_profile)?,
        CapabilityArtifactsV4 {
            account_profile: artifact(
                dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID,
                hash(&account_profile).to_bytes(),
            )?,
            request_profile: artifact(
                dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                hash(&request_profile).to_bytes(),
            )?,
            lifecycle: artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id.to_bytes())?,
            strategy: artifact(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                hash(&strategy).to_bytes(),
            )?,
            transition: artifact(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                hash(&transition).to_bytes(),
            )?,
            effect: artifact(EFFECT_SCHEMA_ID_V4, hash(&effect).to_bytes())?,
        },
        input.root_state_bytes,
    )
    .map_err(Error::Descriptor)?;
    let bundle = RationalLifecycleSelectedBundleV5 {
        action: input.action,
        representation_descriptor_id,
        release_set,
        token_program,
        token_behavior_selection,
        account_profile,
        request_profile,
        transition,
        lifecycle_policy,
        strategy,
        effect,
        descriptor: descriptor_value.encode(),
    };
    validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5(
        &bundle,
        input.authenticated_token_behavior,
    )?;
    Ok(bundle)
}

/// Hostile-decode and join every selected artifact.
pub fn validate_rational_lifecycle_selected_bundle_v5(
    bundle: &RationalLifecycleSelectedBundleV5,
) -> Result<()> {
    let coordinate_count = coordinate_count(bundle.action)?;
    let coordinates = usize::try_from(coordinate_count).map_err(|_| Error::InvalidLength)?;
    let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).map_err(Error::Descriptor)?;
    TokenBehaviorSelectionV2::decode(&bundle.token_behavior_selection)
        .map_err(Error::TokenBehavior)?;
    let expected_request = encode_rational_lifecycle_selected_request_profile_v5(
        bundle.action,
        bundle.representation_descriptor_id,
        bundle.release_set,
        bundle.token_program,
    )?;
    if bundle.request_profile != expected_request {
        return Err(Error::ArtifactGeometry);
    }
    let account =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfile)?;
    let request = RequestProfileV1::decode_selected(
        descriptor.request_profile().program().to_bytes(),
        hash(&bundle.request_profile).to_bytes(),
        &bundle.request_profile,
    )
    .map_err(Error::RequestProfile)?;
    let transition = dclutch_transition_vm::v3::ProgramV3::decode(&bundle.transition)
        .map_err(Error::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy).map_err(Error::Strategy)?;
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
    let effect = EffectProgramV4::decode(&bundle.effect).map_err(Error::EffectV4)?;
    let base = effect.base();
    let registers = dclutch_rational_representation_v2_lifecycle_contract::hot_v3::RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let scalars = u16::try_from(registers.scalar_count().ok_or(Error::InvalidLength)?)
        .map_err(|_| Error::InvalidLength)?;
    let identities = u16::try_from(registers.identity_count().ok_or(Error::InvalidLength)?)
        .map_err(|_| Error::InvalidLength)?;
    let logical = lifecycle_logical_account_count_v3(bundle.action, coordinate_count)?;
    let family_bytes =
        RationalLifecycleHotLayoutV3::request_bytes(coordinates).ok_or(Error::InvalidLength)?;
    if descriptor.config_schema().to_bytes() != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
        || descriptor.request_schema().to_bytes() != RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3
        || descriptor.account_profile()
            != artifact(
                dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID,
                hash(&bundle.account_profile).to_bytes(),
            )?
        || descriptor.request_profile()
            != artifact(
                dclutch_request_profile_contract::SCHEMA_RELEASE_ID,
                hash(&bundle.request_profile).to_bytes(),
            )?
        || descriptor.lifecycle() != artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id.to_bytes())?
        || descriptor.strategy()
            != artifact(
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                hash(&bundle.strategy).to_bytes(),
            )?
        || descriptor.transition()
            != artifact(
                dclutch_transition_vm::v3::SCHEMA_RELEASE_ID,
                hash(&bundle.transition).to_bytes(),
            )?
        || descriptor.effect() != artifact(EFFECT_SCHEMA_ID_V4, hash(&bundle.effect).to_bytes())?
        || strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
        || account.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || account.dynamic_fixed_span_count() != 0
        || account.fixed_account_count() != logical
        || request.fixed_request_bytes()
            != u32::try_from(family_bytes).map_err(|_| Error::InvalidLength)?
        || request.item_request_bytes() != 0
        || effect.span_count() != 0
        || effect.range_count() != 0
        || usize::try_from(effect.semantic_prefix_bytes()).ok() != Some(family_bytes)
        || base.fixed_account_count() != logical
        || !geometry_matches(account, request, transition, base, scalars, identities)
    {
        return Err(Error::ArtifactGeometry);
    }
    let route = base.route(0).map_err(Error::Effect)?;
    if base.route_count() != 1
        || base.receipt_dependency_count() != 0
        || route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_start() != 5
        || route.fixed_account_count()
            != lifecycle_claims_account_count_v3(bundle.action, coordinate_count)?
        || route.item_account_count() != 0
        || route.receipt_dependency_count() != 0
    {
        return Err(Error::ArtifactGeometry);
    }
    Ok(())
}

/// Bind one bundle to independently authenticated finalized Token behavior.
pub fn validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5(
    bundle: &RationalLifecycleSelectedBundleV5,
    authenticated: AuthenticatedTokenBehaviorV2,
) -> Result<()> {
    validate_rational_lifecycle_selected_bundle_v5(bundle)?;
    if bundle.representation_descriptor_id != authenticated.descriptor_id()
        || bundle.release_set != authenticated.selection().release_set()
        || bundle.token_program != authenticated.selection().token_program()
        || bundle.token_behavior_selection != authenticated.selection().to_bytes()
        || hash(&bundle.token_behavior_selection).to_bytes() != authenticated.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    Ok(())
}

fn coordinate_count(action: LifecycleActionV2) -> Result<u32> {
    match action {
        LifecycleActionV2::ActivateReceipt => Ok(0),
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => Ok(1),
        LifecycleActionV2::RetireReceipt => Err(Error::ActionGeometry),
    }
}

fn geometry_matches(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: dclutch_transition_vm::v3::ProgramV3<'_>,
    effect: dclutch_effect_kernel::v3::ProgramV3<'_>,
    scalars: u16,
    identities: u16,
) -> bool {
    scalars == account.common_scalar_count()
        && scalars == request.common_scalar_count()
        && scalars == transition.common_scalar_count()
        && scalars == effect.common_scalar_count()
        && identities == account.common_identity_count()
        && identities == request.common_identity_count()
        && identities == transition.common_identity_count()
        && identities == effect.common_identity_count()
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
    use dclutch_account_profile_contract::lifecycle_v3::{
        HEADER_BYTES as LIFECYCLE_HEADER_BYTES, encode::encode_lifecycle_policy_v5_atomic,
    };
    use dclutch_account_profile_contract::v2::AccountPrestateV2;
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };
    use dclutch_rational_representation_v2_contract::{
        TokenBehaviorRecordAdmissionV2, authenticate_token_behavior_v2,
    };
    use dclutch_rational_representation_v2_kernel::{
        DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
        DESCRIPTOR_SCHEMA_VERSION_V3, DescriptorAdmissionV2,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture range")
            .copy_from_slice(value);
    }

    fn basis() -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: id(1),
                result_domain_id: id(2),
                coordinate_domain_id: id(3),
                result_unit_id: id(4),
                evaluator_release_id: id(5),
                basis_width: 258,
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

    fn descriptor_bytes() -> Vec<u8> {
        let mut output = vec![0_u8; DESCRIPTOR_HEADER_BYTES + 5 * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut output, 0, &DESCRIPTOR_MAGIC_V3);
        put(&mut output, 8, &DESCRIPTOR_SCHEMA_VERSION_V3.to_le_bytes());
        for (offset, value) in [
            (16, id(11)),
            (48, id(12)),
            (80, id(13)),
            (112, id(14)),
            (144, id(15)),
            (176, id(16)),
            (208, dclutch_token_svm::TOKEN_2022_PROGRAM_ID),
        ] {
            put(&mut output, offset, &value);
        }
        put(&mut output, 240, &5_u32.to_le_bytes());
        put(&mut output, 248, &10_u64.to_le_bytes());
        for (index, coefficient) in [0_u64, 7, 5, 0, 9].iter().enumerate() {
            put(
                &mut output,
                DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
                &coefficient.to_le_bytes(),
            );
        }
        output
    }

    fn descriptor(bytes: &[u8]) -> RepresentationDescriptorV2<'_> {
        RepresentationDescriptorV2::decode(
            bytes,
            DescriptorAdmissionV2 {
                selected_descriptor_id: id(21),
                finalized_descriptor_id: id(21),
                recomputed_descriptor_digest: id(21),
                finalized_descriptor_digest: id(21),
                record_authenticated: true,
                derived_representation_authority: id(22),
                authority_derivation_authenticated: true,
            },
        )
        .expect("descriptor")
    }

    fn token_behavior(
        descriptor: RepresentationDescriptorV2<'_>,
        realm: [u8; 32],
    ) -> AuthenticatedTokenBehaviorV2 {
        let selection = TokenBehaviorSelectionV2::new(realm, descriptor.release_set_id())
            .expect("selection")
            .to_bytes();
        let digest = hash(&selection).to_bytes();
        authenticate_token_behavior_v2(
            descriptor,
            realm,
            &selection,
            TokenBehaviorRecordAdmissionV2 {
                selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                selected_content_digest: digest,
                finalized_content_digest: digest,
                recomputed_content_digest: digest,
                record_authenticated: true,
                market_realm_authenticated: true,
            },
        )
        .expect("Token behavior")
    }

    fn lifecycle_policy() -> Vec<u8> {
        let mut scratch = vec![0_u8; LIFECYCLE_HEADER_BYTES];
        let mut output = vec![0_u8; LIFECYCLE_HEADER_BYTES];
        encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
            .expect("lifecycle V5");
        output
    }

    #[test]
    fn selected_actions_are_capability_v4_lifecycle_v5_profile13_only() -> Result<()> {
        let basis = basis();
        let descriptor_bytes = descriptor_bytes();
        let representation = descriptor(&descriptor_bytes);
        let authenticated = token_behavior(representation, id(51));
        let lifecycle = lifecycle_policy();
        for action in [
            LifecycleActionV2::ActivateReceipt,
            LifecycleActionV2::ActivateCoordinate,
            LifecycleActionV2::RetireCoordinate,
        ] {
            let count = usize::from(lifecycle_logical_account_count_v3(
                action,
                coordinate_count(action)?,
            )?);
            let mut lengths = vec![0_u32; count];
            *lengths.get_mut(1).expect("selection") =
                u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
            *lengths.get_mut(4).expect("basis") = u32::try_from(basis.len()).expect("basis width");
            *lengths.get_mut(14).expect("descriptor") =
                u32::try_from(descriptor_bytes.len()).expect("descriptor width");
            let bundle = build_rational_lifecycle_selected_bundle_v5(
                RationalLifecycleSelectedBundleInputV5 {
                    action,
                    account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
                        logical_data_lengths: &lengths,
                        product_basis: &basis,
                    },
                    representation_descriptor: representation,
                    authenticated_token_behavior: authenticated,
                    kind: id(41),
                    root_schema: id(42),
                    lifecycle_policy: &lifecycle,
                    capacity_profile: id(44),
                    root_state_bytes: 64,
                },
            )
            .expect("selected bundle");
            validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5(
                &bundle,
                authenticated,
            )
            .expect("selected join");
            let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).expect("CapabilityV4");
            assert_eq!(
                descriptor.lifecycle().schema().to_bytes(),
                LIFECYCLE_SCHEMA_ID_V5
            );
            assert_eq!(descriptor.effect().schema().to_bytes(), EFFECT_SCHEMA_ID_V4);
            let account = AccountProfileV2::decode(&bundle.account_profile).expect("Profile13");
            assert_eq!(
                account.artifact_profile(),
                DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
            );
            assert_eq!(account.dynamic_fixed_span_count(), 0);
            if action != LifecycleActionV2::ActivateReceipt {
                assert_eq!(
                    account.rule(false, 26).expect("Position rule").prestate(),
                    AccountPrestateV2::Exact
                );
                assert_eq!(
                    account.rule(false, 27).expect("admission rule").prestate(),
                    AccountPrestateV2::Exact
                );
                assert_eq!(
                    account.rule(false, 28).expect("shard Mint rule").prestate(),
                    AccountPrestateV2::AuthenticatedOpaqueReadonlyData
                );
                assert_eq!(
                    account
                        .rule(false, 29)
                        .expect("structured Token account rule")
                        .prestate(),
                    AccountPrestateV2::AuthenticatedOpaqueReadonlyData
                );
            }

            let mut substituted = bundle.clone();
            substituted.representation_descriptor_id = id(52);
            assert!(validate_rational_lifecycle_selected_bundle_v5(&substituted).is_err());
            assert_eq!(
                validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5(
                    &bundle,
                    token_behavior(representation, id(52)),
                ),
                Err(Error::ContentIdentity)
            );
        }
        assert_eq!(
            coordinate_count(LifecycleActionV2::RetireReceipt),
            Err(Error::ActionGeometry)
        );
        Ok(())
    }
}
