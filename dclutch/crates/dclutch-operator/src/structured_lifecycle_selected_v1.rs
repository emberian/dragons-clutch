//! Market-free activation artifacts for a Structured representation.
//!
//! Structured's selected ProgramSet owns issue, unwrap, denomination,
//! reconstitution, redemption, and two normalized V6 creation selectors.
//! Receipt-Mint creation and shard/structured-custody creation retain their
//! distinct Claims lifecycle wire, Profile13, request, transition, lifecycle,
//! strategy, effect, and descriptor records. This module is the single
//! market-free producer of those two activation bundles.

use crate::rational_lifecycle_hot::{
    RationalLifecycleSelectedAccountProfileInputV5, RationalLifecycleSelectedBundleInputV6,
    RationalLifecycleSelectedBundleV6, build_rational_lifecycle_selected_bundle_v6,
    encode_rational_lifecycle_policy_v5, lifecycle_logical_account_count_v3,
};
use crate::structured_selected_release_v1::{
    StructuredPublicationRecordV1, StructuredSelectedReleaseInputV1,
};
use dclutch_claims::rational_kernel::DESCRIPTOR_HEADER_BYTES;
use dclutch_claims::rational_lifecycle::LifecycleActionV2;
use dclutch_claims::structured_kernel::{
    STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPACITY_PROFILE_ID_V2,
};
use dclutch_custody::token_svm::{
    TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, v4::CapabilityProgramV4,
};
use solana_program::hash::hash;

/// Canonical action order for the only two creation transitions.
pub const STRUCTURED_ACTIVATION_ACTIONS_V1: [LifecycleActionV2; 2] = [
    LifecycleActionV2::ActivateReceipt,
    LifecycleActionV2::ActivateCoordinate,
];

/// Stable error from Structured activation artifact compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredLifecycleSelectedErrorV1 {
    /// A source release input could not describe a valid activation closure.
    Input,
    /// A lifecycle semantic-owner encoder or decoder refused.
    Lifecycle(crate::rational_lifecycle_hot::Error),
    /// The immutable Realm/release Token behavior record was invalid.
    Token(dclutch_custody::token_svm::Error),
    /// A compiled descriptor could not be decoded to enumerate its records.
    Descriptor(dclutch_market::capability_program::Error),
}

/// Result alias for Structured activation artifact compilation.
pub type Result<T> = core::result::Result<T, StructuredLifecycleSelectedErrorV1>;

/// Complete market-free activation closure for one Structured release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredActivationSelectedClosureV1 {
    /// Receipt-Mint creation, always first.
    pub activate_receipt: RationalLifecycleSelectedBundleV6,
    /// One selected-coordinate shard/custody creation, always second.
    pub activate_coordinate: RationalLifecycleSelectedBundleV6,
    /// Exact immutable Realm/release Token behavior record shared by both.
    pub config: Vec<u8>,
}

/// Compile the two activation bundles a Structured representation needs before
/// it can issue a receipt.
pub fn structured_activation_selected_closure_v1(
    input: StructuredSelectedReleaseInputV1<'_>,
) -> Result<StructuredActivationSelectedClosureV1> {
    if input.realm == [0; 32]
        || input.release_set == [0; 32]
        || input.root_schema == [0; 32]
        || input.realm == input.release_set
        || input.root_state_bytes == 0
        || input.representation_outcome_count == 0
        || input.product_basis.is_empty()
    {
        return Err(StructuredLifecycleSelectedErrorV1::Input);
    }
    let selection = TokenBehaviorSelectionV2::new(input.realm, input.release_set)
        .map_err(StructuredLifecycleSelectedErrorV1::Token)?;
    let lifecycle = encode_rational_lifecycle_policy_v5()
        .map_err(StructuredLifecycleSelectedErrorV1::Lifecycle)?;
    let activate_receipt = compile(
        input,
        selection,
        &lifecycle,
        LifecycleActionV2::ActivateReceipt,
    )?;
    let activate_coordinate = compile(
        input,
        selection,
        &lifecycle,
        LifecycleActionV2::ActivateCoordinate,
    )?;
    Ok(StructuredActivationSelectedClosureV1 {
        activate_receipt,
        activate_coordinate,
        config: selection.to_bytes().to_vec(),
    })
}

impl StructuredActivationSelectedClosureV1 {
    /// Return the selected bundle for a creation action and refuse all other
    /// lifecycle actions.  Retirement has separate semantic ownership.
    #[must_use]
    pub fn bundle(&self, action: LifecycleActionV2) -> Option<&RationalLifecycleSelectedBundleV6> {
        match action {
            LifecycleActionV2::ActivateReceipt => Some(&self.activate_receipt),
            LifecycleActionV2::ActivateCoordinate => Some(&self.activate_coordinate),
            LifecycleActionV2::RetireCoordinate | LifecycleActionV2::RetireReceipt => None,
        }
    }

    /// Enumerate the Registry records the activation route authenticates.
    ///
    /// There is deliberately no ProgramSet record here: the enclosing
    /// `StructuredSelectedReleaseV1` owns the single eight-entry set and
    /// publishes this closure's records in selector order.
    pub fn publication_records(&self) -> Result<Vec<StructuredPublicationRecordV1<'_>>> {
        let mut records = Vec::with_capacity(1 + 7 * STRUCTURED_ACTIVATION_ACTIONS_V1.len());
        records.push(StructuredPublicationRecordV1 {
            label: "activation-config",
            schema: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            body: &self.config,
        });
        for bundle in [&self.activate_receipt, &self.activate_coordinate] {
            let descriptor = CapabilityProgramV4::decode(&bundle.descriptor)
                .map_err(StructuredLifecycleSelectedErrorV1::Descriptor)?;
            let artifacts = descriptor.artifacts();
            for (label, schema, body) in [
                (
                    "activation-descriptor",
                    dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
                    bundle.descriptor.as_slice(),
                ),
                (
                    "activation-account-profile",
                    artifacts.account_profile.schema().to_bytes(),
                    bundle.account_profile.as_slice(),
                ),
                (
                    "activation-request-profile",
                    artifacts.request_profile.schema().to_bytes(),
                    bundle.request_profile.as_slice(),
                ),
                (
                    "activation-lifecycle-policy",
                    artifacts.lifecycle.schema().to_bytes(),
                    bundle.lifecycle_policy.as_slice(),
                ),
                (
                    "activation-strategy",
                    artifacts.strategy.schema().to_bytes(),
                    bundle.strategy.as_slice(),
                ),
                (
                    "activation-transition",
                    artifacts.transition.schema().to_bytes(),
                    bundle.transition.as_slice(),
                ),
                (
                    "activation-effect",
                    artifacts.effect.schema().to_bytes(),
                    bundle.effect.as_slice(),
                ),
            ] {
                records.push(StructuredPublicationRecordV1 {
                    label,
                    schema,
                    body,
                });
            }
        }
        Ok(records)
    }

    /// Domain-separated identity of the two bundle descriptors and shared
    /// config.  It is a campaign artifact identity, never a Market manifest
    /// substitute.
    #[must_use]
    pub fn activation_id(&self) -> [u8; 32] {
        hash(
            &[
                b"dclutch:structured-activation-selected:v1".as_slice(),
                self.config.as_slice(),
                self.activate_receipt.descriptor.as_slice(),
                self.activate_coordinate.descriptor.as_slice(),
            ]
            .concat(),
        )
        .to_bytes()
    }
}

fn compile(
    input: StructuredSelectedReleaseInputV1<'_>,
    selection: TokenBehaviorSelectionV2,
    lifecycle: &[u8],
    action: LifecycleActionV2,
) -> Result<RationalLifecycleSelectedBundleV6> {
    let product_basis =
        dclutch_product::payoff::runtime_v3::ProductBasisV3::decode(input.product_basis)
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    let portfolio_bytes = dclutch_product::portfolio_record_bytes(
        usize::try_from(product_basis.basis_width())
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?,
    )
    .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    let logical = usize::from(
        lifecycle_logical_account_count_v3(
            action,
            u32::from(action == LifecycleActionV2::ActivateCoordinate),
        )
        .map_err(StructuredLifecycleSelectedErrorV1::Lifecycle)?,
    );
    let mut lengths = vec![0_u32; logical];
    *lengths
        .get_mut(0)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok()
            .and_then(|header| header.checked_add(input.root_state_bytes))
            .ok_or(StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(1)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2)
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(2)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(dclutch_product::admission::PRODUCT_RECORD_BYTES_V2)
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(3)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(portfolio_bytes).map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(4)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(input.product_basis.len())
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(14)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? = u32::try_from(DESCRIPTOR_HEADER_BYTES)
        .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    build_rational_lifecycle_selected_bundle_v6(RationalLifecycleSelectedBundleInputV6 {
        action,
        account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
            logical_data_lengths: &lengths,
            product_basis: input.product_basis,
        },
        token_behavior_selection: selection,
        kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
        root_schema: input.root_schema,
        lifecycle_policy: lifecycle,
        capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
        root_state_bytes: input.root_state_bytes,
    })
    .map_err(StructuredLifecycleSelectedErrorV1::Lifecycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_product::payoff::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn basis() -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0; BASIS_HEADER_BYTES_V3];
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
                price_gate_certificate_digest: [0; 32],
            },
            &mut output,
        )
        .expect("basis");
        output
    }

    fn input<'a>(basis: &'a [u8]) -> StructuredSelectedReleaseInputV1<'a> {
        StructuredSelectedReleaseInputV1 {
            realm: id(18),
            release_set: id(15),
            root_schema:
                crate::structured_activation_bundle_v1::STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V1,
            root_state_bytes: u32::try_from(
                crate::structured_activation_bundle_v1::STRUCTURED_CAPABILITY_ROOT_BYTES_V1,
            )
            .expect("root width"),
            representation_outcome_count: 3,
            item_state_bytes: 64,
            product_basis: basis,
        }
    }

    #[test]
    fn activation_closure_is_market_free_and_has_only_creation_actions() {
        let basis = basis();
        let first = structured_activation_selected_closure_v1(input(&basis)).expect("first");
        let second = structured_activation_selected_closure_v1(input(&basis)).expect("second");
        assert_eq!(first, second);
        assert_eq!(
            first
                .bundle(LifecycleActionV2::ActivateReceipt)
                .map(|b| b.action),
            Some(LifecycleActionV2::ActivateReceipt)
        );
        assert_eq!(
            first
                .bundle(LifecycleActionV2::ActivateCoordinate)
                .map(|b| b.action),
            Some(LifecycleActionV2::ActivateCoordinate)
        );
        assert_eq!(first.bundle(LifecycleActionV2::RetireCoordinate), None);
        assert_eq!(first.bundle(LifecycleActionV2::RetireReceipt), None);
        assert_ne!(first.activation_id(), [0; 32]);
        let profile = dclutch_vm::account_profile::v2::AccountProfileV2::decode(
            &first.activate_receipt.account_profile,
        )
        .expect("ActivateReceipt profile");
        assert_eq!(
            profile
                .rule(false, 0)
                .expect("root coordinate")
                .data_length(),
            u32::try_from(dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1)
                .expect("root header")
                + input(&basis).root_state_bytes
        );
        assert_eq!(
            profile
                .rule(false, 2)
                .expect("Product coordinate")
                .data_length(),
            u32::try_from(dclutch_product::admission::PRODUCT_RECORD_BYTES_V2)
                .expect("Product width")
        );
        assert_eq!(
            profile
                .rule(false, 3)
                .expect("Portfolio coordinate")
                .data_length(),
            u32::try_from(dclutch_product::portfolio_record_bytes(258).expect("Portfolio width"))
                .expect("Portfolio width u32")
        );
    }

    #[test]
    fn every_activation_record_is_selected_by_its_descriptor() {
        let basis = basis();
        let closure = structured_activation_selected_closure_v1(input(&basis)).expect("closure");
        let records = closure.publication_records().expect("records");
        assert_eq!(
            records.len(),
            1 + 7 * STRUCTURED_ACTIVATION_ACTIONS_V1.len()
        );
        let descriptors = records
            .iter()
            .filter(|record| record.label == "activation-descriptor")
            .map(StructuredPublicationRecordV1::content_id)
            .collect::<Vec<_>>();
        assert_eq!(
            descriptors,
            vec![
                hash(&closure.activate_receipt.descriptor).to_bytes(),
                hash(&closure.activate_coordinate.descriptor).to_bytes()
            ]
        );
    }
}
