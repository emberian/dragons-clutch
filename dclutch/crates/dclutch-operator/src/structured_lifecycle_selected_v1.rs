//! Market-free activation artifacts for a Structured representation.
//!
//! Structured's selected ProgramSet owns issue, unwrap, denomination,
//! reconstitution, redemption, and four lifecycle selectors.
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
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    liability_basis_vector_width_v2,
};
use dclutch_claims::protocol_position_v2::PROTOCOL_POSITION_ADMISSION_BYTES_V2;
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
use dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
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
    /// Single-coordinate retirement using the canonical V6 lifecycle.
    pub retire_coordinate: RationalLifecycleSelectedBundleV6,
    /// Complete sparse-support retirement using the existing Claims semantic owner.
    pub retire_receipt: RationalLifecycleSelectedBundleV6,
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
        retire_coordinate: compile(
            input,
            selection,
            &lifecycle,
            LifecycleActionV2::RetireCoordinate,
        )?,
        retire_receipt: compile(
            input,
            selection,
            &lifecycle,
            LifecycleActionV2::RetireReceipt,
        )?,
        config: selection.to_bytes().to_vec(),
    })
}

impl StructuredActivationSelectedClosureV1 {
    /// Return the selected bundle for a canonical Claims lifecycle action.
    #[must_use]
    pub fn bundle(&self, action: LifecycleActionV2) -> Option<&RationalLifecycleSelectedBundleV6> {
        match action {
            LifecycleActionV2::ActivateReceipt => Some(&self.activate_receipt),
            LifecycleActionV2::ActivateCoordinate => Some(&self.activate_coordinate),
            LifecycleActionV2::RetireCoordinate => Some(&self.retire_coordinate),
            LifecycleActionV2::RetireReceipt => Some(&self.retire_receipt),
        }
    }

    /// Enumerate the Registry records the activation route authenticates.
    ///
    /// There is deliberately no ProgramSet record here: the enclosing
    /// `StructuredSelectedReleaseV1` owns the single ten-entry set and
    /// publishes this closure's records in selector order.
    pub fn publication_records(&self) -> Result<Vec<StructuredPublicationRecordV1<'_>>> {
        let mut records = Vec::with_capacity(1 + 7 * 4);
        records.push(StructuredPublicationRecordV1 {
            label: "activation-config",
            schema: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            body: &self.config,
        });
        for bundle in [
            &self.activate_receipt,
            &self.activate_coordinate,
            &self.retire_coordinate,
            &self.retire_receipt,
        ] {
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
                self.retire_coordinate.descriptor.as_slice(),
                self.retire_receipt.descriptor.as_slice(),
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
    let aggregate_bytes = liability_basis_vector_width_v2(
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        product_basis.basis_width(),
    )
    .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    let position_bytes = liability_basis_vector_width_v2(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        product_basis.basis_width(),
    )
    .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    let logical = usize::from(
        lifecycle_logical_account_count_v3(
            if action == LifecycleActionV2::RetireReceipt {
                LifecycleActionV2::ActivateReceipt
            } else {
                action
            },
            u32::from(matches!(
                action,
                LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
            )),
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
        .get_mut(11)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(12)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        crate::general_selected_release_v1::RENT_SYSVAR_ACCOUNT_BYTES_V1;
    *lengths
        .get_mut(14)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? = u32::try_from(DESCRIPTOR_HEADER_BYTES)
        .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(19)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2)
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(21)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(aggregate_bytes).map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    *lengths
        .get_mut(22)
        .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
        u32::try_from(dclutch_market::STATE_BYTES)
            .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    // Native ProtocolPosition Admit creates Position and Admission from vacant
    // accounts. Only Close consumes their live account images.
    if action == LifecycleActionV2::RetireCoordinate {
        *lengths
            .get_mut(26)
            .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
            u32::try_from(position_bytes).map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
        *lengths
            .get_mut(27)
            .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
            u32::try_from(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
                .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    }
    if matches!(
        action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    ) {
        *lengths
            .get_mut(35)
            .ok_or(StructuredLifecycleSelectedErrorV1::Input)? =
            u32::try_from(dclutch_product::DOMAIN_HEADER_BYTES)
                .map_err(|_| StructuredLifecycleSelectedErrorV1::Input)?;
    }
    let bundle_input = RationalLifecycleSelectedBundleInputV6 {
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
    };
    if action == LifecycleActionV2::RetireReceipt {
        crate::rational_lifecycle_hot::build_dynamic_retirement_bundle_v1(
            bundle_input,
            input.representation_outcome_count,
        )
    } else {
        build_rational_lifecycle_selected_bundle_v6(bundle_input)
    }
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
        basis_with_width(258)
    }

    fn basis_with_width(width: u32) -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0; BASIS_HEADER_BYTES_V3];
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
                crate::structured_activation_bundle_v1::STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V2,
            root_state_bytes: u32::try_from(
                crate::structured_activation_bundle_v1::STRUCTURED_CAPABILITY_ROOT_BYTES_V2,
            )
            .expect("root width"),
            representation_outcome_count: 3,
            item_state_bytes: 64,
            product_basis: basis,
        }
    }

    fn assert_native_profile(
        profile: dclutch_vm::account_profile::v2::AccountProfileV2<'_>,
        action: LifecycleActionV2,
    ) {
        use dclutch_vm::account_profile::v2::AccountPrestateV2::{
            AdapterAuthenticatedVariableData, AuthenticatedOpaqueReadonlyData,
            AuthenticatedRouteAlias, Exact,
        };

        let width = 258_u32;
        let logical = match action {
            LifecycleActionV2::ActivateReceipt => 25_u16,
            LifecycleActionV2::ActivateCoordinate => 39_u16,
            _ => panic!("creation profile"),
        };
        for coordinate in 0..logical {
            let (prestate, length) = match coordinate {
                0 => (
                    Exact,
                    u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1).expect("root header")
                        + input(&basis()).root_state_bytes,
                ),
                1 => (
                    Exact,
                    u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("config width"),
                ),
                2 => (
                    Exact,
                    u32::try_from(dclutch_product::admission::PRODUCT_RECORD_BYTES_V2)
                        .expect("Product width"),
                ),
                3 => (
                    Exact,
                    u32::try_from(
                        dclutch_product::portfolio_record_bytes(width as usize)
                            .expect("Portfolio width"),
                    )
                    .expect("Portfolio width u32"),
                ),
                4 => (
                    AdapterAuthenticatedVariableData,
                    u32::try_from(BASIS_HEADER_BYTES_V3).expect("basis header"),
                ),
                6 | 7 | 8 | 9 | 10 | 13 | 17 | 18 | 20 | 23 | 24 | 28 | 29 => {
                    (AuthenticatedOpaqueReadonlyData, 0)
                }
                11 => (
                    Exact,
                    u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
                        .expect("activation cache"),
                ),
                12 => (
                    Exact,
                    crate::general_selected_release_v1::RENT_SYSVAR_ACCOUNT_BYTES_V1,
                ),
                14 => (AuthenticatedOpaqueReadonlyData, 0),
                31 | 33 | 37 => (AuthenticatedRouteAlias, 0),
                15 | 32 | 34 | 36 | 38 => (Exact, 0),
                19 => (
                    Exact,
                    u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit"),
                ),
                21 => (
                    Exact,
                    u32::try_from(
                        liability_basis_vector_width_v2(
                            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                            width,
                        )
                        .expect("Claims aggregate"),
                    )
                    .expect("Claims aggregate u32"),
                ),
                22 => (
                    Exact,
                    u32::try_from(dclutch_market::STATE_BYTES).expect("Core market"),
                ),
                // ProtocolPosition Admit owns allocation of both accounts.
                26 | 27 => (Exact, 0),
                35 => (AuthenticatedOpaqueReadonlyData, 0),
                _ => (Exact, 0),
            };
            let rule = profile.rule(false, coordinate).expect("logical rule");
            assert_eq!(rule.prestate(), prestate, "prestate {coordinate}");
            assert_eq!(rule.data_length(), length, "data length {coordinate}");
        }
    }

    #[test]
    fn both_creation_selectors_match_every_native_planned_account_state() {
        let basis = basis();
        let closure = structured_activation_selected_closure_v1(input(&basis)).expect("closure");
        for (action, bundle) in [
            (
                LifecycleActionV2::ActivateReceipt,
                &closure.activate_receipt,
            ),
            (
                LifecycleActionV2::ActivateCoordinate,
                &closure.activate_coordinate,
            ),
        ] {
            let profile =
                dclutch_vm::account_profile::v2::AccountProfileV2::decode(&bundle.account_profile)
                    .expect("Profile13");
            assert_native_profile(profile, action);
        }
    }

    #[test]
    fn coordinate_position_profiles_match_admit_vacancy_and_close_live_state() {
        use dclutch_vm::account_profile::v2::{AccountPrestateV2, AccountProfileV2};

        for width in [2, 258] {
            let basis = basis_with_width(width);
            let mut geometry = input(&basis);
            geometry.representation_outcome_count = width.min(3);
            let closure = structured_activation_selected_closure_v1(geometry).expect("closure");
            let live_position = u32::try_from(
                liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, width)
                    .expect("native Position allocation width"),
            )
            .expect("Position width u32");
            let live_admission = u32::try_from(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
                .expect("native admission allocation width");
            for (bundle, widths) in [
                (&closure.activate_coordinate, [0, 0]),
                (&closure.retire_coordinate, [live_position, live_admission]),
            ] {
                let profile = AccountProfileV2::decode(&bundle.account_profile).expect("Profile13");
                for (coordinate, expected) in [26_u16, 27].into_iter().zip(widths) {
                    let rule = profile.rule(false, coordinate).expect("Position rule");
                    assert_eq!(rule.prestate(), AccountPrestateV2::Exact);
                    assert_eq!(
                        rule.data_length(),
                        expected,
                        "{:?} coordinate {coordinate}",
                        bundle.action
                    );
                }
            }
        }
    }

    #[test]
    fn lifecycle_closure_is_market_free_and_contains_creation_and_retirement() {
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
        assert_eq!(
            first
                .bundle(LifecycleActionV2::RetireCoordinate)
                .map(|bundle| bundle.action),
            Some(LifecycleActionV2::RetireCoordinate)
        );
        assert_eq!(
            first
                .bundle(LifecycleActionV2::RetireReceipt)
                .map(|bundle| bundle.action),
            Some(LifecycleActionV2::RetireReceipt)
        );
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
        for (coordinate, expected, label) in [
            (
                11,
                u32::try_from(dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
                    .expect("activation cache width"),
                "activation cache",
            ),
            (
                12,
                crate::general_selected_release_v1::RENT_SYSVAR_ACCOUNT_BYTES_V1,
                "Rent sysvar",
            ),
            (
                19,
                u32::try_from(dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2)
                    .expect("RentCredit width"),
                "RentCredit",
            ),
            (
                21,
                u32::try_from(
                    dclutch_claims::liability_basis_state_v2::liability_basis_vector_width_v2(
                        dclutch_claims::liability_basis_state_v2::LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                        258,
                    )
                    .expect("Claims aggregate width"),
                )
                .expect("Claims aggregate width u32"),
                "Claims aggregate",
            ),
            (
                22,
                u32::try_from(dclutch_market::STATE_BYTES).expect("Core market width"),
                "Core market",
            ),
            (15, 0, "vacant descriptor staging"),
        ] {
            assert_eq!(
                profile.rule(false, coordinate).expect(label).data_length(),
                expected,
                "{label}"
            );
        }
        let coordinate_profile = dclutch_vm::account_profile::v2::AccountProfileV2::decode(
            &first.activate_coordinate.account_profile,
        )
        .expect("ActivateCoordinate profile");
        for (coordinate, expected, label) in [
            (26, 0, "vacant Claims Position"),
            (27, 0, "vacant Position admission"),
        ] {
            assert_eq!(
                coordinate_profile
                    .rule(false, coordinate)
                    .expect(label)
                    .data_length(),
                expected,
                "{label}"
            );
        }
        for coordinate in [32_u16, 34, 36, 38] {
            assert_eq!(
                coordinate_profile
                    .rule(false, coordinate)
                    .expect("staging cursor")
                    .data_length(),
                0,
                "finalized record staging {coordinate} is vacant"
            );
        }
        let result = coordinate_profile.rule(false, 35).expect("result record");
        assert_eq!(result.data_length(), 0);
        assert_eq!(
            result.prestate(),
            dclutch_vm::account_profile::v2::AccountPrestateV2::AuthenticatedOpaqueReadonlyData
        );
    }

    #[test]
    fn every_activation_record_is_selected_by_its_descriptor() {
        let basis = basis();
        let closure = structured_activation_selected_closure_v1(input(&basis)).expect("closure");
        let records = closure.publication_records().expect("records");
        assert_eq!(records.len(), 1 + 7 * 4);
        let descriptors = records
            .iter()
            .filter(|record| record.label == "activation-descriptor")
            .map(StructuredPublicationRecordV1::content_id)
            .collect::<Vec<_>>();
        assert_eq!(
            descriptors,
            vec![
                hash(&closure.activate_receipt.descriptor).to_bytes(),
                hash(&closure.activate_coordinate.descriptor).to_bytes(),
                hash(&closure.retire_coordinate.descriptor).to_bytes(),
                hash(&closure.retire_receipt.descriptor).to_bytes()
            ]
        );
    }
}
