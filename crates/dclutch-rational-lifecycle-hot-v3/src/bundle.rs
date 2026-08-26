//! Content-addressed CapabilityProgram V3 bundle for one lifecycle action.

use dclutch_account_profile_contract::v2::{AccountProfileV2, TYPED_SCALAR_ARTIFACT_PROFILE};
use dclutch_capability_program_contract::v3::{CAPABILITY_PROGRAM_V3_BYTES, CapabilityProgramV3};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, RouteKindV3},
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
    ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_rational_representation_v2_kernel::REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2,
    hot_v3::{
        RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3, RationalLifecycleHotLayoutV3,
        RationalLifecycleHotRegisterLayoutV3,
    },
};
use dclutch_request_profile_contract::RequestProfileV1;
use dclutch_transition_vm::v3::ProgramV3 as TransitionProgramV3;
use solana_program::hash::hash;

use crate::{
    Error, Result,
    account_profile::{
        RationalLifecycleAccountProfileInputV3, encode_rational_lifecycle_account_profile_v3,
    },
    artifacts::{
        encode_rational_lifecycle_request_profile_v3, encode_rational_lifecycle_transition_v3,
    },
    effect::{
        encode_rational_lifecycle_effect_v3, lifecycle_claims_account_count_v3,
        lifecycle_logical_account_count_v3,
    },
    validate_action_geometry,
};

/// Exact interpreted ExecutionStrategy record width.
pub const RATIONAL_LIFECYCLE_STRATEGY_BYTES_V3: usize = EXECUTION_STRATEGY_PROGRAM_BYTES_V2;
/// Exact CapabilityProgramV3 descriptor width.
pub const RATIONAL_LIFECYCLE_DESCRIPTOR_BYTES_V3: usize = CAPABILITY_PROGRAM_V3_BYTES;

/// Release-owned coordinates plus exact descriptor/action account observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalLifecycleHotBundleInputV3<'a> {
    /// Exact lifecycle action selected by this immutable capability descriptor.
    pub action: LifecycleActionV2,
    /// Exact descriptor nonzero-support row count selected by this artifact.
    pub coordinate_count: u32,
    /// Exact logical account widths and authenticated Product basis body.
    pub account_profile: RationalLifecycleAccountProfileInputV3<'a>,
    /// Manifest-selected Rational lifecycle capability kind.
    pub kind: [u8; 32],
    /// Manifest-selected mutable root-tail schema.
    pub root_schema: [u8; 32],
    /// Finalized StateLifecyclePolicy V4 content identity.
    pub derivation_policy: [u8; 32],
    /// Manifest-selected physical capacity profile.
    pub capacity_profile: [u8; 32],
    /// Exact mutable root-tail byte width.
    pub root_state_bytes: u32,
}

/// Exact bytes which must each become one finalized Registry record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleHotBundleV3 {
    /// Descriptor-specialized lifecycle action.
    pub action: LifecycleActionV2,
    /// Descriptor-specialized nonzero support width.
    pub coordinate_count: u32,
    /// Exact logical account interpreter bytes.
    pub account_profile: Vec<u8>,
    /// Exact family request interpreter bytes.
    pub request_profile: Vec<u8>,
    /// Exact economic transition interpreter bytes.
    pub transition: Vec<u8>,
    /// Interpreted strategy selecting the exact TransitionVM bytes.
    pub strategy: [u8; RATIONAL_LIFECYCLE_STRATEGY_BYTES_V3],
    /// One-route Claims effect interpreter bytes.
    pub effect: Vec<u8>,
    /// Capability descriptor selecting every exact artifact content identity.
    pub descriptor: [u8; RATIONAL_LIFECYCLE_DESCRIPTOR_BYTES_V3],
}

/// Emit one complete action-specific lifecycle Hot bundle.
///
/// This V3 bundle owns fixed-cardinality activation and selected-coordinate
/// actions. Complete-support receipt retirement is exclusively compact V4.
pub fn build_rational_lifecycle_hot_bundle_v3(
    input: RationalLifecycleHotBundleInputV3<'_>,
) -> Result<RationalLifecycleHotBundleV3> {
    validate_action_geometry(input.action, input.coordinate_count)?;
    let account_profile = encode_rational_lifecycle_account_profile_v3(
        input.action,
        input.coordinate_count,
        input.account_profile,
    )?;
    let request_profile =
        encode_rational_lifecycle_request_profile_v3(input.action, input.coordinate_count)?;
    let transition = encode_rational_lifecycle_transition_v3(input.action, input.coordinate_count)?;
    let effect = encode_rational_lifecycle_effect_v3(input.action, input.coordinate_count)?;
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
    .map_err(Error::Strategy)?;
    let strategy = strategy_value.to_bytes();
    let descriptor_value = CapabilityProgramV3::new(
        content(input.kind)?,
        content(REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3)?,
        content(RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3)?,
        content(input.root_schema)?,
        digest(&account_profile)?,
        content(input.derivation_policy)?,
        content(input.capacity_profile)?,
        digest(&effect)?,
        content(dclutch_request_profile_contract::SCHEMA_RELEASE_ID)?,
        digest(&request_profile)?,
        content(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)?,
        digest(&strategy)?,
        input.root_state_bytes,
    )
    .map_err(Error::Descriptor)?;
    let bundle = RationalLifecycleHotBundleV3 {
        action: input.action,
        coordinate_count: input.coordinate_count,
        account_profile,
        request_profile,
        transition,
        strategy,
        effect,
        descriptor: descriptor_value.encode(),
    };
    validate_rational_lifecycle_hot_bundle_v3(&bundle)?;
    Ok(bundle)
}

/// Hostile-decode and join every artifact to the exact action geometry.
pub fn validate_rational_lifecycle_hot_bundle_v3(
    bundle: &RationalLifecycleHotBundleV3,
) -> Result<()> {
    let coordinates = validate_action_geometry(bundle.action, bundle.coordinate_count)?;
    let registers = RationalLifecycleHotRegisterLayoutV3::new(coordinates);
    let descriptor = CapabilityProgramV3::decode(&bundle.descriptor).map_err(Error::Descriptor)?;
    let account =
        AccountProfileV2::decode(&bundle.account_profile).map_err(Error::AccountProfile)?;
    let request = RequestProfileV1::decode_selected(
        descriptor.request_profile_program().to_bytes(),
        hash(&bundle.request_profile).to_bytes(),
        &bundle.request_profile,
    )
    .map_err(Error::RequestProfile)?;
    let projected_request_bytes = request.request_bytes(0).map_err(Error::RequestProfile)?;
    let transition = TransitionProgramV3::decode(&bundle.transition).map_err(Error::Transition)?;
    let strategy = ExecutionStrategyProgramV2::decode(&bundle.strategy).map_err(Error::Strategy)?;
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect_program().to_bytes(),
        hash(&bundle.effect).to_bytes(),
        &bundle.effect,
    )
    .map_err(Error::Effect)?;
    strategy
        .validate_descriptor_selection(digest(&bundle.strategy)?, descriptor)
        .map_err(Error::Strategy)?;

    let logical_accounts =
        lifecycle_logical_account_count_v3(bundle.action, bundle.coordinate_count)?;
    let request_bytes =
        RationalLifecycleHotLayoutV3::request_bytes(coordinates).ok_or(Error::InvalidLength)?;
    let common_scalars = narrow_u16(registers.scalar_count().ok_or(Error::InvalidLength)?)?;
    let common_identities = narrow_u16(registers.identity_count().ok_or(Error::InvalidLength)?)?;
    if descriptor.config_schema().to_bytes() != REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3
        || descriptor.request_schema().to_bytes() != RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3
        || descriptor.account_profile() != digest(&bundle.account_profile)?
        || descriptor.request_profile_schema().to_bytes()
            != dclutch_request_profile_contract::SCHEMA_RELEASE_ID
        || descriptor.request_profile_program() != digest(&bundle.request_profile)?
        || descriptor.transition_schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        || descriptor.transition_program() != digest(&bundle.strategy)?
        || descriptor.effect_program() != digest(&bundle.effect)?
        || strategy.disposition() != StrategyDispositionV2::Interpreted
        || strategy.transition_schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID
        || strategy.transition_program() != digest(&bundle.transition)?
        || account.fixed_account_count() != logical_accounts
        || account.item_account_stride() != 0
        || !account_profile_matches(account)?
        || projected_request_bytes != request_bytes
        || request.item_request_bytes() != 0
        || effect.fixed_account_count() != logical_accounts
        || effect.item_account_stride() != 0
        || effect.route_count() != 1
        || effect.receipt_dependency_count() != 0
        || !geometry_matches(
            account,
            request,
            transition,
            effect,
            common_scalars,
            common_identities,
        )
    {
        return Err(Error::ArtifactGeometry);
    }
    let route = effect.route(0).map_err(Error::Effect)?;
    if route.role() != FixedRole::Claims
        || route.kind() != RouteKindV3::Once
        || route.fixed_account_start() != 5
        || route.fixed_account_count()
            != lifecycle_claims_account_count_v3(bundle.action, bundle.coordinate_count)?
        || route.item_account_count() != 0
        || route.receipt_dependency_count() != 0
    {
        return Err(Error::ArtifactGeometry);
    }
    Ok(())
}

fn account_profile_matches(account: AccountProfileV2<'_>) -> Result<bool> {
    let Some(product_width) = account
        .tail_count_projection()
        .map_err(Error::AccountProfile)?
    else {
        return Ok(false);
    };
    if product_width.account() != 4
        || usize::from(product_width.register())
            != dclutch_rational_representation_v2_lifecycle_contract::hot_v3::RATIONAL_LIFECYCLE_SCALAR_PRODUCT_OUTCOME_COUNT_V3
        || usize::try_from(product_width.data_offset()).ok()
            != Some(dclutch_product_payoff_v2_codec::runtime_v3::BASIS_WIDTH_OFFSET_V3)
    {
        return Ok(false);
    }
    Ok(account.artifact_profile() == TYPED_SCALAR_ARTIFACT_PROFILE
        && account
            .nonzero_u64_tail_count_projection()
            .map_err(Error::AccountProfile)?
            .is_none())
}

fn geometry_matches(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
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

fn narrow_u16(value: usize) -> Result<u16> {
    u16::try_from(value).map_err(|_| Error::InvalidLength)
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

    fn input<'a>(
        action: LifecycleActionV2,
        coordinate_count: u32,
        basis: &'a [u8],
        lengths: &'a [u32],
    ) -> RationalLifecycleHotBundleInputV3<'a> {
        RationalLifecycleHotBundleInputV3 {
            action,
            coordinate_count,
            account_profile: RationalLifecycleAccountProfileInputV3 {
                logical_data_lengths: lengths,
                product_basis: basis,
            },
            kind: id(10),
            root_schema: id(11),
            derivation_policy: id(12),
            capacity_profile: id(13),
            root_state_bytes: 8,
        }
    }

    #[test]
    fn selected_action_bundles_join_exactly() {
        let basis = basis();
        for (action, coordinate_count) in [
            (LifecycleActionV2::ActivateReceipt, 0),
            (LifecycleActionV2::ActivateCoordinate, 1),
            (LifecycleActionV2::RetireCoordinate, 1),
        ] {
            let count = usize::from(
                lifecycle_logical_account_count_v3(action, coordinate_count)
                    .expect("logical count"),
            );
            let mut lengths = vec![0_u32; count];
            *lengths.get_mut(4).expect("Product basis") =
                u32::try_from(basis.len()).expect("basis length");
            if matches!(
                action,
                LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
            ) {
                *lengths.get_mut(31).expect("basis alias") =
                    u32::try_from(basis.len()).expect("basis length");
            }
            let bundle = build_rational_lifecycle_hot_bundle_v3(input(
                action,
                coordinate_count,
                &basis,
                &lengths,
            ))
            .expect("bundle");
            validate_rational_lifecycle_hot_bundle_v3(&bundle).expect("joined bundle");
            let descriptor = CapabilityProgramV3::decode(&bundle.descriptor).expect("descriptor");
            assert_eq!(
                descriptor.config_schema().to_bytes(),
                REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3
            );
            assert_eq!(
                descriptor.request_schema().to_bytes(),
                RATIONAL_LIFECYCLE_HOT_SCHEMA_RELEASE_ID_V3
            );
        }
    }

    #[test]
    fn caller_carried_retirement_bundle_is_unreachable_for_every_k() {
        let basis = basis();
        for count in [1, 2, 3] {
            assert_eq!(
                build_rational_lifecycle_hot_bundle_v3(input(
                    LifecycleActionV2::RetireReceipt,
                    count,
                    &basis,
                    &[],
                )),
                Err(Error::ActionGeometry)
            );
        }
    }

    #[test]
    fn same_width_product_or_artifact_substitution_cannot_reuse_bundle() {
        let basis = basis();
        let count = usize::from(
            lifecycle_logical_account_count_v3(LifecycleActionV2::ActivateReceipt, 0)
                .expect("logical count"),
        );
        let mut lengths = vec![0_u32; count];
        *lengths.get_mut(4).expect("Product basis") =
            u32::try_from(basis.len()).expect("basis length");
        let canonical = build_rational_lifecycle_hot_bundle_v3(input(
            LifecycleActionV2::ActivateReceipt,
            0,
            &basis,
            &lengths,
        ))
        .expect("canonical");
        let mut substituted = canonical.clone();
        *substituted
            .request_profile
            .get_mut(0)
            .expect("profile magic") ^= 1;
        assert!(validate_rational_lifecycle_hot_bundle_v3(&substituted).is_err());

        let mut other_basis = basis;
        *other_basis.get_mut(48).expect("Product identity byte") ^= 1;
        let other = build_rational_lifecycle_hot_bundle_v3(input(
            LifecycleActionV2::ActivateReceipt,
            0,
            &other_basis,
            &lengths,
        ))
        .expect("same-width Product");
        assert_eq!(canonical.account_profile, other.account_profile);
    }
}
