//! Market-neutral CapabilityV4 bundle for Rational lifecycle Hot V6.

use dclutch_account_profile_contract::{
    lifecycle_v3::{
        StateLifecyclePolicyV5, CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5 as LIFECYCLE_SCHEMA_ID_V5,
    },
    v2::{AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE},
};
use dclutch_capability_program_contract::v4::{
    ArtifactReferenceV4, CapabilityArtifactsV4, CapabilityProgramV4, CAPABILITY_PROGRAM_V4_BYTES,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::RouteKindV3,
    v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4},
};
use dclutch_execution_strategy_contract::v2::{
    ExecutionStrategyProgramV2, StrategyDispositionV2, ACCELERATOR_ACK_SCHEMA_ID_V2,
    ACCELERATOR_REQUEST_SCHEMA_ID_V2, EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2, EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
};
use dclutch_rational_representation_v2_contract::AuthenticatedTokenBehaviorV2;
use dclutch_rational_representation_v2_lifecycle_contract::{
    hot_v3::RationalLifecycleHotLayoutV3,
    hot_v6::{RationalLifecycleHotRegisterLayoutV6, RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V6},
    LifecycleActionV2,
};
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_token_svm::{
    TokenBehaviorSelectionV2, TOKEN_BEHAVIOR_SELECTION_BYTES_V2,
    TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
};
use solana_program::hash::hash;

use crate::{
    artifacts::{
        encode_rational_lifecycle_selected_request_profile_v6,
        encode_rational_lifecycle_transition_v6,
    },
    effect::encode_rational_lifecycle_selected_effect_v6,
    lifecycle_claims_account_count_v3, lifecycle_logical_account_count_v3,
    selected_profile_v5::encode_rational_lifecycle_selected_account_profile_v6,
    Error, RationalLifecycleSelectedAccountProfileInputV5, Result,
};

/// Exact interpreted strategy width reused by V6.
pub const RATIONAL_LIFECYCLE_SELECTED_STRATEGY_BYTES_V6: usize =
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityV4 descriptor width reused by V6.
pub const RATIONAL_LIFECYCLE_SELECTED_DESCRIPTOR_BYTES_V6: usize = CAPABILITY_PROGRAM_V4_BYTES;

/// Pre-founding-safe inputs for one fixed-cardinality V6 artifact bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleSelectedBundleInputV6<'a> {
    /// Exact fixed-cardinality lifecycle action.
    pub action: LifecycleActionV2,
    /// Exact Profile13 observations, including ProductBasis N.
    pub account_profile: RationalLifecycleSelectedAccountProfileInputV5<'a>,
    /// Realm/release-selected Token behavior; no Market identity is embedded.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
    /// Manifest-selected capability kind.
    pub kind: [u8; 32],
    /// Manifest-selected root schema.
    pub root_schema: [u8; 32],
    /// Exact LifecycleV5 bytes.
    pub lifecycle_policy: &'a [u8],
    /// Manifest-selected capacity profile.
    pub capacity_profile: [u8; 32],
    /// Exact mutable root width.
    pub root_state_bytes: u32,
}

/// Finalized V6 bundle whose content identity is independent of Market.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleSelectedBundleV6 {
    /// Selected lifecycle action.
    pub action: LifecycleActionV2,
    /// Release selected by Token behavior, not by a Market-bound descriptor.
    pub release_set: [u8; 32],
    /// Token program selected before Market founding.
    pub token_program: [u8; 32],
    /// Exact finalized Token behavior config.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// Profile13 account interpreter.
    pub account_profile: Vec<u8>,
    /// V6 market-neutral RequestProfile.
    pub request_profile: Vec<u8>,
    /// V6 descriptor-equality TransitionVM.
    pub transition: Vec<u8>,
    /// Exact LifecycleV5 policy.
    pub lifecycle_policy: Vec<u8>,
    /// Exact interpreted strategy.
    pub strategy: [u8; RATIONAL_LIFECYCLE_SELECTED_STRATEGY_BYTES_V6],
    /// EffectV4 Claims route using authenticated descriptor identity.
    pub effect: Vec<u8>,
    /// CapabilityV4 descriptor selecting the V6 request schema.
    pub descriptor: [u8; RATIONAL_LIFECYCLE_SELECTED_DESCRIPTOR_BYTES_V6],
}

/// Build one pre-founding-safe V6 artifact bundle.
pub fn build_rational_lifecycle_selected_bundle_v6(
    input: RationalLifecycleSelectedBundleInputV6<'_>,
) -> Result<RationalLifecycleSelectedBundleV6> {
    let coordinate_count = coordinate_count(input.action)?;
    let selection = input.authenticated_token_behavior.selection();
    let release_set = selection.release_set();
    let token_program = selection.token_program();
    let token_behavior_selection = selection.to_bytes();
    if release_set == [0; 32]
        || token_program == [0; 32]
        || hash(&token_behavior_selection).to_bytes()
            != input.authenticated_token_behavior.content_digest()
    {
        return Err(Error::ContentIdentity);
    }
    let account_profile =
        encode_rational_lifecycle_selected_account_profile_v6(input.action, input.account_profile)?;
    let request_profile = encode_rational_lifecycle_selected_request_profile_v6(input.action)?;
    let transition = encode_rational_lifecycle_transition_v6(input.action, coordinate_count)?;
    let effect = encode_rational_lifecycle_selected_effect_v6(input.action)?;
    let lifecycle_policy = Vec::from(input.lifecycle_policy);
    let lifecycle_id = digest(&lifecycle_policy)?;
    let strategy = ExecutionStrategyProgramV2::new(
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
    .map_err(Error::Strategy)?
    .to_bytes();
    let descriptor = CapabilityProgramV4::new(
        content(input.kind)?,
        content(TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2)?,
        content(RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V6)?,
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
            effect: artifact(SCHEMA_RELEASE_ID_V4, hash(&effect).to_bytes())?,
        },
        input.root_state_bytes,
    )
    .map_err(Error::Descriptor)?
    .encode();
    let bundle = RationalLifecycleSelectedBundleV6 {
        action: input.action,
        release_set,
        token_program,
        token_behavior_selection,
        account_profile,
        request_profile,
        transition,
        lifecycle_policy,
        strategy,
        effect,
        descriptor,
    };
    validate_rational_lifecycle_selected_bundle_v6(&bundle)?;
    Ok(bundle)
}

/// Hostile-decode and rederive every V6 artifact identity.
pub fn validate_rational_lifecycle_selected_bundle_v6(
    bundle: &RationalLifecycleSelectedBundleV6,
) -> Result<()> {
    let coordinate_count = coordinate_count(bundle.action)?;
    let coordinates = usize::try_from(coordinate_count).map_err(|_| Error::InvalidLength)?;
    let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).map_err(Error::Descriptor)?;
    let selection = TokenBehaviorSelectionV2::decode(&bundle.token_behavior_selection)
        .map_err(Error::TokenBehavior)?;
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
    let effect = EffectProgramV4::decode(&bundle.effect).map_err(Error::EffectV4)?;
    let base = effect.base();
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
    let registers = RationalLifecycleHotRegisterLayoutV6::new(coordinates);
    let scalars = u16::try_from(registers.scalar_count().ok_or(Error::InvalidLength)?)
        .map_err(|_| Error::InvalidLength)?;
    let identities = u16::try_from(registers.identity_count().ok_or(Error::InvalidLength)?)
        .map_err(|_| Error::InvalidLength)?;
    let logical = lifecycle_logical_account_count_v3(bundle.action, coordinate_count)?;
    let family_bytes =
        RationalLifecycleHotLayoutV3::request_bytes(coordinates).ok_or(Error::InvalidLength)?;
    if bundle.release_set != selection.release_set()
        || bundle.token_program != selection.token_program()
        || account.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || account.dynamic_fixed_span_count() != 0
        || account.fixed_account_count() != logical
        || bundle.request_profile
            != encode_rational_lifecycle_selected_request_profile_v6(bundle.action)?
        || bundle.transition
            != encode_rational_lifecycle_transition_v6(bundle.action, coordinate_count)?
        || bundle.effect != encode_rational_lifecycle_selected_effect_v6(bundle.action)?
        || descriptor.request_schema().to_bytes() != RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V6
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
        || descriptor.effect() != artifact(SCHEMA_RELEASE_ID_V4, hash(&bundle.effect).to_bytes())?
        || strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema() != descriptor.transition().schema()
        || strategy.transition_program() != descriptor.transition().program()
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

fn coordinate_count(action: LifecycleActionV2) -> Result<u32> {
    match action {
        LifecycleActionV2::ActivateReceipt => Ok(0),
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => Ok(1),
        LifecycleActionV2::RetireReceipt => Err(Error::ActionGeometry),
    }
}

fn content(value: [u8; 32]) -> Result<ContentId> {
    ContentId::new(value).map_err(|_| Error::ContentIdentity)
}

fn digest(bytes: &[u8]) -> Result<ContentId> {
    content(hash(bytes).to_bytes())
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
        encode::encode_lifecycle_policy_v5_atomic, HEADER_BYTES as LIFECYCLE_HEADER_BYTES,
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        compile_basis_v3, BasisInputV3, BasisKindV3, BASIS_HEADER_BYTES_V3,
    };
    use dclutch_rational_representation_v2_contract::{
        authenticate_token_behavior_v2, TokenBehaviorRecordAdmissionV2,
    };
    use dclutch_rational_representation_v2_kernel::{
        descriptor_v3::{
            encode_representation_descriptor_v3_atomic, representation_descriptor_bytes_v3,
            RepresentationDescriptorInputV3,
        },
        DescriptorAdmissionV2, RepresentationDescriptorV2,
    };
    use dclutch_rational_representation_v2_lifecycle_contract::{
        hot_v3::RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3, hot_v6::RationalLifecycleHotRequestV6,
        LifecycleHeaderV2, LifecycleRequestV2, LIFECYCLE_HEADER_BYTES_V2,
    };
    use dclutch_request_profile_contract::{project_atomic, ProjectionRegistersV1};
    use dclutch_token_svm::{TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2};
    use dclutch_transition_vm::v3::{
        execute_fold_atomic, Error as TransitionError, RegisterInput, RegisterOutput,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
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
        .expect("ProductBasisV3");
        output
    }

    fn descriptor_bytes(market: [u8; 32]) -> Vec<u8> {
        let bytes = representation_descriptor_bytes_v3(3).expect("K3 width");
        let mut scratch = vec![0_u8; bytes];
        let mut output = vec![0_u8; bytes];
        encode_representation_descriptor_v3_atomic(
            RepresentationDescriptorInputV3 {
                exposure_id: id(11),
                exposure_digest: id(12),
                root_id: id(13),
                market,
                release_set: id(15),
                receipt_mint: id(16),
                token_program: TOKEN_2022_PROGRAM_ID,
                denominator: 10,
                coefficients: &[2, 0, 5],
            },
            &mut scratch,
            &mut output,
        )
        .expect("descriptor");
        output
    }

    fn descriptor(bytes: &[u8]) -> RepresentationDescriptorV2<'_> {
        let digest = hash(bytes).to_bytes();
        RepresentationDescriptorV2::decode(
            bytes,
            DescriptorAdmissionV2 {
                selected_descriptor_id: digest,
                finalized_descriptor_id: digest,
                recomputed_descriptor_digest: digest,
                finalized_descriptor_digest: digest,
                record_authenticated: true,
                derived_representation_authority: id(17),
                authority_derivation_authenticated: true,
            },
        )
        .expect("authenticated descriptor")
    }

    fn token_behavior(descriptor: RepresentationDescriptorV2<'_>) -> AuthenticatedTokenBehaviorV2 {
        let realm = id(18);
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
            .expect("LifecycleV5");
        output
    }

    fn bundle(
        behavior: AuthenticatedTokenBehaviorV2,
        basis: &[u8],
        descriptor_bytes: usize,
        lifecycle: &[u8],
    ) -> RationalLifecycleSelectedBundleV6 {
        let count = usize::from(
            lifecycle_logical_account_count_v3(LifecycleActionV2::ActivateReceipt, 0)
                .expect("logical count"),
        );
        let mut lengths = vec![0_u32; count];
        *lengths.get_mut(1).expect("selection coordinate") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
        *lengths.get_mut(4).expect("basis coordinate") =
            u32::try_from(basis.len()).expect("basis width");
        *lengths.get_mut(14).expect("descriptor coordinate") =
            u32::try_from(descriptor_bytes).expect("descriptor width");
        build_rational_lifecycle_selected_bundle_v6(RationalLifecycleSelectedBundleInputV6 {
            action: LifecycleActionV2::ActivateReceipt,
            account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
                logical_data_lengths: &lengths,
                product_basis: basis,
            },
            authenticated_token_behavior: behavior,
            kind: id(41),
            root_schema: id(42),
            lifecycle_policy: lifecycle,
            capacity_profile: id(43),
            root_state_bytes: 64,
        })
        .expect("V6 bundle")
    }

    fn project_request(
        bundle: &RationalLifecycleSelectedBundleV6,
        family: &[u8],
        authenticated_descriptor: [u8; 32],
    ) -> (Vec<u64>, Vec<[u8; 32]>) {
        let profile = RequestProfileV1::decode(&bundle.request_profile).expect("request profile");
        let scalars = profile.scalar_count(0).expect("scalars");
        let identities = profile.identity_count(0).expect("identities");
        let input_scalars = vec![0_u64; scalars];
        let mut input_identities = vec![[0_u8; 32]; identities];
        *input_identities
            .get_mut(RATIONAL_LIFECYCLE_IDENTITY_DESCRIPTOR_V3)
            .expect("descriptor identity coordinate") = authenticated_descriptor;
        let mut scratch_scalars = vec![0_u64; scalars];
        let mut scratch_identities = vec![[0_u8; 32]; identities];
        let mut output_scalars = vec![0_u64; scalars];
        let mut output_identities = vec![[0_u8; 32]; identities];
        project_atomic(
            profile,
            0,
            family,
            ProjectionRegistersV1 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("request projection");
        (output_scalars, output_identities)
    }

    #[test]
    fn v6_artifacts_are_market_neutral_and_descriptor_equality_is_runtime_checked() {
        let basis = basis();
        let first_bytes = descriptor_bytes(id(21));
        let second_bytes = descriptor_bytes(id(22));
        let first = descriptor(&first_bytes);
        let second = descriptor(&second_bytes);
        let lifecycle = lifecycle_policy();
        let first_bundle = bundle(token_behavior(first), &basis, first_bytes.len(), &lifecycle);
        let second_bundle = bundle(
            token_behavior(second),
            &basis,
            second_bytes.len(),
            &lifecycle,
        );
        assert_eq!(first.market_id(), id(21));
        assert_eq!(second.market_id(), id(22));
        assert_ne!(first.descriptor_id(), second.descriptor_id());
        assert_eq!(first_bundle, second_bundle);
        validate_rational_lifecycle_selected_bundle_v6(&first_bundle).expect("V6 geometry");

        let child_bytes = {
            let request = LifecycleRequestV2::new(
                LifecycleHeaderV2 {
                    action: LifecycleActionV2::ActivateReceipt,
                    release_set: first.release_set_id(),
                    market: first.market_id(),
                    graph_id: first.graph_id(),
                    descriptor_id: first.descriptor_id(),
                    parent_context: id(31),
                    representation_authority: first.representation_authority(),
                    receipt_mint: first.receipt_mint(),
                    token_program: first.token_program(),
                    rent_credit: id(32),
                    rent_program: id(33),
                    generation: 1,
                    expected_claims_market_revision: 0,
                    observed_receipt_lamports: 1,
                    receipt_rent_principal: 1,
                    expected_receipt_supply: 0,
                    outcome_count: first.outcome_count(),
                    coordinate_count: 0,
                    rent_credit_before: 1,
                    rent_credit_after: 1,
                },
                &[],
            )
            .expect("Claims request");
            let mut child = vec![0_u8; LIFECYCLE_HEADER_BYTES_V2];
            request.encode_into(&mut child).expect("Claims bytes");
            child
        };
        let child = LifecycleRequestV2::decode(&child_bytes).expect("Claims child");
        let mut family_bytes = vec![0_u8; child_bytes.len()];
        RationalLifecycleHotRequestV6::from_child_into(child, &mut family_bytes)
            .expect("V6 family");
        let transition = dclutch_transition_vm::v3::ProgramV3::decode(&first_bundle.transition)
            .expect("transition");

        let (scalars, identities) =
            project_request(&first_bundle, &family_bytes, first.descriptor_id());
        let mut scratch_scalars = scalars.clone();
        let mut scratch_identities = identities.clone();
        let mut output_scalars = vec![u64::MAX; scalars.len()];
        let mut output_identities = vec![[0xa5; 32]; identities.len()];
        execute_fold_atomic(
            transition,
            0,
            RegisterInput {
                scalars: &scalars,
                identities: &identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        )
        .expect("matching descriptor accepted");

        let (hostile_scalars, hostile_identities) =
            project_request(&first_bundle, &family_bytes, second.descriptor_id());
        let mut hostile_scratch_scalars = hostile_scalars.clone();
        let mut hostile_scratch_identities = hostile_identities.clone();
        let mut hostile_output_scalars = vec![0x5a5a_u64; hostile_scalars.len()];
        let mut hostile_output_identities = vec![[0x5a; 32]; hostile_identities.len()];
        let before_scalars = hostile_output_scalars.clone();
        let before_identities = hostile_output_identities.clone();
        assert_eq!(
            execute_fold_atomic(
                transition,
                0,
                RegisterInput {
                    scalars: &hostile_scalars,
                    identities: &hostile_identities,
                },
                RegisterOutput {
                    scalars: &mut hostile_scratch_scalars,
                    identities: &mut hostile_scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut hostile_output_scalars,
                    identities: &mut hostile_output_identities,
                },
            ),
            Err(TransitionError::CheckFailed)
        );
        assert_eq!(hostile_output_scalars, before_scalars);
        assert_eq!(hostile_output_identities, before_identities);
    }
}
