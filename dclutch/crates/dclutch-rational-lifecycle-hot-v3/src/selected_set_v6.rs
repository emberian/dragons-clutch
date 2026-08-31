//! The market-free four-action lifecycle ProgramSet.
//!
//! # The module the market-free half was left one short of
//!
//! Rational built `selected_bundle_v6` ("Market-neutral CapabilityV4 bundle")
//! for its three fixed-cardinality actions and a market-neutral compact bundle
//! for the fourth, and then never built the set that binds them. The only
//! shippable ProgramSet builder, [`crate::build_rational_lifecycle_program_set_v5`],
//! consumes the market-BEARING V5 bundles, so the capability a founded Market
//! could actually select was still the trapped one.
//!
//! # What "market-free" has to mean here, and why it is the whole point
//!
//! A Market's capability manifest keys each entry by `kind_id` and names a
//! `release_id` that is the SHA-256 of these ProgramSet bytes. The manifest
//! digest is in turn a seed of the Market PDA. So every byte this module emits
//! must be fixed BEFORE the Market address exists -- otherwise
//! `manifest ⊃ release_id = SHA-256(set) ⊃ … ⊃ market = PDA(seeds ⊇
//! SHA-256(manifest))` is a fixed point no author can construct, which is
//! exactly the wall Fractional still sits behind.
//!
//! This builder therefore takes a [`TokenBehaviorSelectionV2`] -- the immutable
//! Realm/release record the descriptors themselves name as `config_schema` --
//! and never a `RepresentationDescriptorV2` or an
//! `AuthenticatedTokenBehaviorV2`, both of which carry or require the Core
//! Market. The market-freedom is structural: there is no parameter here through
//! which a Market could arrive.

use dclutch_capability_program_contract::{
    set_v2::{
        CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, CapabilityProgramSetV2,
        SelectorWidthV2, encode_program_set_v2, encoded_program_set_bytes_v2,
    },
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_core_contract::ContentId;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2, compact_hot_v4::RationalLifecycleCompactHotLayoutV4,
    hot_v3::RationalLifecycleHotLayoutV3,
};
use dclutch_token_svm::{TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TokenBehaviorSelectionV2};
use solana_program::hash::hash;

use crate::{
    Error, RationalLifecycleCompactBundleV4, RationalLifecycleSelectedBundleV6, Result,
    validate_rational_lifecycle_compact_bundle_v4, validate_rational_lifecycle_selected_bundle_v6,
};

/// Canonical order of the four selectable lifecycle actions.
///
/// The set's entry order, and the order every consumer walks. Written once
/// here so a caller cannot present the four bundles in an order that disagrees
/// with the table the selector routes through.
pub const RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6: [LifecycleActionV2; 4] = [
    LifecycleActionV2::ActivateReceipt,
    LifecycleActionV2::ActivateCoordinate,
    LifecycleActionV2::RetireCoordinate,
    LifecycleActionV2::RetireReceipt,
];

/// Four market-free lifecycle descriptors sharing one Token behavior selection.
#[derive(Clone, Copy, Debug)]
pub struct RationalLifecycleProgramSetInputV6<'a> {
    /// Immutable Realm/release Token behavior, derivable before founding.
    pub token_behavior_selection: TokenBehaviorSelectionV2,
    /// Fixed-cardinality receipt activation bundle.
    pub activate_receipt: &'a RationalLifecycleSelectedBundleV6,
    /// Fixed-cardinality coordinate activation bundle.
    pub activate_coordinate: &'a RationalLifecycleSelectedBundleV6,
    /// Fixed-cardinality coordinate retirement bundle.
    pub retire_coordinate: &'a RationalLifecycleSelectedBundleV6,
    /// Support-derived complete receipt-retirement bundle.
    pub retire_receipt: &'a RationalLifecycleCompactBundleV4,
}

/// Canonical config and four-entry market-free lifecycle ProgramSetV2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalLifecycleProgramSetV6 {
    /// Exact Realm/release-selected Token behavior config bytes.
    ///
    /// This is the record a Rational manifest entry's `config_id` digests, and
    /// the one the descriptors name as `config_schema`.
    pub token_behavior_selection: [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2],
    /// SHA-256 identity of the config record.
    pub token_behavior_selection_id: [u8; 32],
    /// Exact schema-bound CapabilityProgramSetV2 bytes.
    pub program_set: Vec<u8>,
    /// SHA-256 identity a Market capability manifest selects as `release_id`.
    pub program_set_id: [u8; 32],
}

/// Build the four-action lifecycle ProgramSet a founded Market can select.
pub fn build_rational_lifecycle_program_set_v6(
    input: RationalLifecycleProgramSetInputV6<'_>,
) -> Result<RationalLifecycleProgramSetV6> {
    validate_input(input)?;
    // The compact and fixed-cardinality families must agree on where the
    // action selector sits, or one of the four entries would be unreachable
    // through the very table that selects it.
    if RationalLifecycleHotLayoutV3::ACTION != RationalLifecycleCompactHotLayoutV4::ACTION {
        return Err(Error::ArtifactGeometry);
    }
    let entries = entries(input)?;
    let width = encoded_program_set_bytes_v2(entries.len()).map_err(|_| Error::ArtifactGeometry)?;
    let mut program_set = vec![0_u8; width];
    encode_program_set_v2(
        u32::try_from(RationalLifecycleHotLayoutV3::ACTION).map_err(|_| Error::ArtifactGeometry)?,
        SelectorWidthV2::U8,
        &entries,
        &mut program_set,
    )
    .map_err(|_| Error::ArtifactGeometry)?;
    let selection = input.token_behavior_selection.to_bytes();
    let output = RationalLifecycleProgramSetV6 {
        token_behavior_selection: selection,
        token_behavior_selection_id: hash(&selection).to_bytes(),
        program_set_id: hash(&program_set).to_bytes(),
        program_set,
    };
    validate_rational_lifecycle_program_set_v6(&output, input)?;
    Ok(output)
}

/// Hostile-decode one ProgramSet and rebuild every entry from its bundles.
///
/// This does not inspect the set for plausibility: it re-derives the entry
/// table and requires exact agreement, so a substituted descriptor identity,
/// a reordered action, or a foreign config refuses.
pub fn validate_rational_lifecycle_program_set_v6(
    value: &RationalLifecycleProgramSetV6,
    input: RationalLifecycleProgramSetInputV6<'_>,
) -> Result<()> {
    validate_input(input)?;
    let expected_selection = input.token_behavior_selection.to_bytes();
    if value.token_behavior_selection != expected_selection
        || value.token_behavior_selection_id != hash(&expected_selection).to_bytes()
        || value.program_set_id != hash(&value.program_set).to_bytes()
    {
        return Err(Error::ContentIdentity);
    }
    let decoded =
        CapabilityProgramSetV2::decode(&value.program_set).map_err(|_| Error::ArtifactGeometry)?;
    if decoded.selector_offset()
        != u32::try_from(RationalLifecycleHotLayoutV3::ACTION)
            .map_err(|_| Error::ArtifactGeometry)?
        || decoded.selector_width() != SelectorWidthV2::U8
        || usize::from(decoded.entry_count()) != RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6.len()
    {
        return Err(Error::ArtifactGeometry);
    }
    for (ordinal, expected) in entries(input)?.iter().enumerate() {
        if decoded
            .entry(u16::try_from(ordinal).map_err(|_| Error::ArtifactGeometry)?)
            .map_err(|_| Error::ArtifactGeometry)?
            != *expected
        {
            return Err(Error::ArtifactGeometry);
        }
    }
    Ok(())
}

/// The four entries, in canonical action order, derived from the bundles.
fn entries(
    input: RationalLifecycleProgramSetInputV6<'_>,
) -> Result<[CapabilityProgramSetEntryV2; 4]> {
    Ok([
        selected_entry(input.activate_receipt, LifecycleActionV2::ActivateReceipt)?,
        selected_entry(
            input.activate_coordinate,
            LifecycleActionV2::ActivateCoordinate,
        )?,
        selected_entry(input.retire_coordinate, LifecycleActionV2::RetireCoordinate)?,
        compact_entry(input.retire_receipt)?,
    ])
}

/// Every bundle must join the ONE config record this set publishes.
///
/// A set whose entries named different Token behavior selections would publish
/// one `config_id` while its descriptors authenticated against another.
fn validate_input(input: RationalLifecycleProgramSetInputV6<'_>) -> Result<()> {
    let selection = input.token_behavior_selection.to_bytes();
    let release_set = input.token_behavior_selection.release_set();
    let token_program = input.token_behavior_selection.token_program();
    for (bundle, action) in [
        (input.activate_receipt, LifecycleActionV2::ActivateReceipt),
        (
            input.activate_coordinate,
            LifecycleActionV2::ActivateCoordinate,
        ),
        (input.retire_coordinate, LifecycleActionV2::RetireCoordinate),
    ] {
        validate_rational_lifecycle_selected_bundle_v6(bundle)?;
        if bundle.action != action {
            return Err(Error::ActionGeometry);
        }
        if bundle.token_behavior_selection != selection
            || bundle.release_set != release_set
            || bundle.token_program != token_program
        {
            return Err(Error::ContentIdentity);
        }
    }
    validate_rational_lifecycle_compact_bundle_v4(input.retire_receipt)?;
    if input.retire_receipt.token_behavior_selection != selection {
        return Err(Error::ContentIdentity);
    }
    Ok(())
}

fn selected_entry(
    bundle: &RationalLifecycleSelectedBundleV6,
    action: LifecycleActionV2,
) -> Result<CapabilityProgramSetEntryV2> {
    if bundle.action != action {
        return Err(Error::ActionGeometry);
    }
    Ok(CapabilityProgramSetEntryV2::new(
        u32::from(action.tag()),
        descriptor_reference(&bundle.descriptor)?,
    ))
}

fn compact_entry(bundle: &RationalLifecycleCompactBundleV4) -> Result<CapabilityProgramSetEntryV2> {
    Ok(CapabilityProgramSetEntryV2::new(
        u32::from(LifecycleActionV2::RetireReceipt.tag()),
        descriptor_reference(&bundle.descriptor)?,
    ))
}

fn descriptor_reference(bytes: &[u8]) -> Result<CapabilityDescriptorReferenceV2> {
    Ok(CapabilityDescriptorReferenceV2::new(
        ContentId::new(CAPABILITY_PROGRAM_SCHEMA_ID_V4).map_err(|_| Error::ContentIdentity)?,
        ContentId::new(hash(bytes).to_bytes()).map_err(|_| Error::ContentIdentity)?,
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::{
        HEADER_BYTES as LIFECYCLE_HEADER_BYTES, encode::encode_lifecycle_policy_v5_atomic,
    };
    use dclutch_product_payoff_v2_codec::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, compile_basis_v3,
    };
    use dclutch_rational_representation_v2_lifecycle_contract::{
        LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2, RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
    };
    use dclutch_token_svm::{TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_BYTES_V2};

    use crate::{
        RationalLifecycleCompactArtifactInputV6, RationalLifecycleCompactBundleInputV6,
        RationalLifecycleSelectedAccountProfileInputV5, RationalLifecycleSelectedBundleInputV6,
        build_rational_lifecycle_compact_bundle_v6, build_rational_lifecycle_selected_bundle_v6,
        lifecycle_logical_account_count_v3,
    };

    /// Ordered representation the fixtures retire: support at outcomes 0 and 2.
    pub(crate) const COEFFICIENTS: [u64; 3] = [2, 0, 5];

    pub(crate) fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    pub(crate) fn basis() -> [u8; BASIS_HEADER_BYTES_V3] {
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
                // Exempt by proof: degree 0 and 1 need no price gate,
                // and a digest offered alongside one is refused.
                price_gate_certificate_digest: [0_u8; 32],
            },
            &mut output,
        )
        .expect("ProductBasisV3");
        output
    }

    pub(crate) fn lifecycle_policy() -> Vec<u8> {
        let mut scratch = vec![0_u8; LIFECYCLE_HEADER_BYTES];
        let mut output = vec![0_u8; LIFECYCLE_HEADER_BYTES];
        encode_lifecycle_policy_v5_atomic(&[], &[], &[], &[], &[], &[], &mut scratch, &mut output)
            .expect("LifecycleV5");
        output
    }

    /// Byte width the descriptor account will have for `COEFFICIENTS`.
    fn descriptor_width() -> usize {
        dclutch_rational_representation_v2_kernel::DESCRIPTOR_HEADER_BYTES + COEFFICIENTS.len() * 8
    }

    pub(crate) fn selection() -> TokenBehaviorSelectionV2 {
        // Realm and release set: both known before any Market exists. The
        // Realm is in fact a SEED of the Market PDA, so it strictly precedes
        // the address rather than merely being available early.
        TokenBehaviorSelectionV2::new(id(18), id(15)).expect("pre-Market selection")
    }

    pub(crate) fn selected(
        action: LifecycleActionV2,
        basis: &[u8],
        lifecycle: &[u8],
    ) -> RationalLifecycleSelectedBundleV6 {
        let coordinate_count = u32::from(action != LifecycleActionV2::ActivateReceipt);
        let count = usize::from(
            lifecycle_logical_account_count_v3(action, coordinate_count).expect("logical count"),
        );
        let mut lengths = vec![0_u32; count];
        *lengths.get_mut(1).expect("selection") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
        *lengths.get_mut(4).expect("basis") = u32::try_from(basis.len()).expect("basis width");
        *lengths.get_mut(14).expect("descriptor") =
            u32::try_from(descriptor_width()).expect("descriptor width");
        build_rational_lifecycle_selected_bundle_v6(RationalLifecycleSelectedBundleInputV6 {
            action,
            account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
                logical_data_lengths: &lengths,
                product_basis: basis,
            },
            token_behavior_selection: selection(),
            kind: RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
            root_schema: id(42),
            lifecycle_policy: lifecycle,
            capacity_profile: id(43),
            root_state_bytes: 64,
        })
        .expect("V6 bundle")
    }

    pub(crate) fn compact(basis: &[u8], lifecycle: &[u8]) -> RationalLifecycleCompactBundleV4 {
        let support = COEFFICIENTS
            .iter()
            .filter(|coefficient| **coefficient != 0)
            .count();
        let start = usize::from(crate::RATIONAL_LIFECYCLE_HOT_INJECTED_ACCOUNT_COUNT_V3)
            + dclutch_rational_representation_v2_lifecycle_contract::LIFECYCLE_COMMON_ACCOUNT_COUNT_V2;
        let mut lengths = vec![0_u32; start + support * LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2];
        *lengths.get_mut(1).expect("selection") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
        *lengths.get_mut(4).expect("basis") = u32::try_from(basis.len()).expect("basis width");
        *lengths.get_mut(14).expect("descriptor") =
            u32::try_from(descriptor_width()).expect("descriptor width");
        build_rational_lifecycle_compact_bundle_v6(RationalLifecycleCompactBundleInputV6 {
            artifacts: RationalLifecycleCompactArtifactInputV6 {
                logical_data_lengths: &lengths,
                product_basis: basis,
                coefficients: &COEFFICIENTS,
            },
            kind: RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
            token_behavior_selection: selection(),
            root_schema: id(42),
            lifecycle_policy: lifecycle,
            capacity_profile: id(43),
            root_state_bytes: 64,
        })
        .expect("compact V6 bundle")
    }

    fn build() -> RationalLifecycleProgramSetV6 {
        let basis = basis();
        let lifecycle = lifecycle_policy();
        let activate_receipt = selected(LifecycleActionV2::ActivateReceipt, &basis, &lifecycle);
        let activate_coordinate =
            selected(LifecycleActionV2::ActivateCoordinate, &basis, &lifecycle);
        let retire_coordinate = selected(LifecycleActionV2::RetireCoordinate, &basis, &lifecycle);
        let retire_receipt = compact(&basis, &lifecycle);
        let input = RationalLifecycleProgramSetInputV6 {
            token_behavior_selection: selection(),
            activate_receipt: &activate_receipt,
            activate_coordinate: &activate_coordinate,
            retire_coordinate: &retire_coordinate,
            retire_receipt: &retire_receipt,
        };
        let set = build_rational_lifecycle_program_set_v6(input).expect("V6 ProgramSet");
        validate_rational_lifecycle_program_set_v6(&set, input).expect("rejoin");
        set
    }

    /// A Rational selection precedes the Market it will bind.
    ///
    /// The counterpart of SEL-SEAM's General pin, and the property whose
    /// absence kept Rational unselectable. There is no Market anywhere in this
    /// test -- not as a parameter, not as a fixture -- and yet a complete
    /// four-action ProgramSet and its config record come out with stable
    /// identities. Those two identities are exactly the `release_id` and
    /// `config_id` a capability manifest entry carries, so the manifest, its
    /// digest, and therefore the Market PDA seeded by that digest are all
    /// well defined BEFORE the Market exists.
    ///
    /// Byte-stability is asserted across two independent builds because a
    /// manifest entry that moved between runs would be just as unusable as one
    /// that depended on the Market.
    #[test]
    fn a_rational_selection_precedes_the_market_it_will_bind() {
        let first = build();
        let second = build();
        assert_eq!(first, second);
        assert_eq!(first.program_set_id, second.program_set_id);
        assert_eq!(
            first.token_behavior_selection_id,
            second.token_behavior_selection_id
        );
        assert_ne!(first.program_set_id, [0; 32]);
        assert_ne!(first.token_behavior_selection_id, [0; 32]);
    }

    /// The set routes all four actions, and only those four.
    #[test]
    fn the_four_actions_each_select_their_own_descriptor() {
        let set = build();
        let decoded = CapabilityProgramSetV2::decode(&set.program_set).expect("decode");
        assert_eq!(decoded.entry_count(), 4);
        assert_eq!(
            decoded.selector_offset(),
            u32::try_from(RationalLifecycleHotLayoutV3::ACTION).expect("offset")
        );
        assert_eq!(decoded.selector_width(), SelectorWidthV2::U8);

        let mut seen = Vec::new();
        for (ordinal, action) in RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6.into_iter().enumerate() {
            let entry = decoded
                .entry(u16::try_from(ordinal).expect("ordinal"))
                .expect("entry");
            assert_eq!(entry.selector(), u32::from(action.tag()));
            let descriptor = entry.descriptor().program().to_bytes();
            // Four actions, four DISTINCT descriptors: a set that pointed two
            // actions at one program would route one of them wrongly while
            // still encoding and authenticating cleanly.
            assert!(!seen.contains(&descriptor));
            seen.push(descriptor);
        }
    }

    /// Substituting any published byte refuses on the rebuild.
    #[test]
    fn a_substituted_set_config_or_descriptor_refuses() {
        let basis = basis();
        let lifecycle = lifecycle_policy();
        let activate_receipt = selected(LifecycleActionV2::ActivateReceipt, &basis, &lifecycle);
        let activate_coordinate =
            selected(LifecycleActionV2::ActivateCoordinate, &basis, &lifecycle);
        let retire_coordinate = selected(LifecycleActionV2::RetireCoordinate, &basis, &lifecycle);
        let retire_receipt = compact(&basis, &lifecycle);
        let input = RationalLifecycleProgramSetInputV6 {
            token_behavior_selection: selection(),
            activate_receipt: &activate_receipt,
            activate_coordinate: &activate_coordinate,
            retire_coordinate: &retire_coordinate,
            retire_receipt: &retire_receipt,
        };
        let set = build_rational_lifecycle_program_set_v6(input).expect("set");

        // A moved descriptor identity in the encoded table.
        let mut moved = set.clone();
        *moved.program_set.last_mut().expect("last byte") ^= 0xff;
        assert_eq!(
            validate_rational_lifecycle_program_set_v6(&moved, input),
            Err(Error::ContentIdentity)
        );

        // A published set identity that does not digest its own bytes.
        let mut relabelled = set.clone();
        relabelled.program_set_id = id(99);
        assert_eq!(
            validate_rational_lifecycle_program_set_v6(&relabelled, input),
            Err(Error::ContentIdentity)
        );

        // A config record other than the one the descriptors authenticate to.
        let mut foreign = set.clone();
        foreign.token_behavior_selection =
            TokenBehaviorSelectionV2::new(id(88), id(15))
                .expect("other realm")
                .to_bytes();
        assert_eq!(
            validate_rational_lifecycle_program_set_v6(&foreign, input),
            Err(Error::ContentIdentity)
        );

        // Two actions presented in the wrong slots.
        let swapped = RationalLifecycleProgramSetInputV6 {
            activate_receipt: &activate_coordinate,
            activate_coordinate: &activate_receipt,
            ..input
        };
        assert_eq!(
            build_rational_lifecycle_program_set_v6(swapped),
            Err(Error::ActionGeometry)
        );
    }

    /// A bundle built against another Realm cannot join this set.
    ///
    /// The set publishes ONE config record. If a bundle authenticated to a
    /// different Token behavior selection could enter it, the published
    /// `config_id` would name a record the descriptors do not select.
    #[test]
    fn a_bundle_from_another_selection_cannot_join_the_set() {
        let basis = basis();
        let lifecycle = lifecycle_policy();
        let activate_coordinate =
            selected(LifecycleActionV2::ActivateCoordinate, &basis, &lifecycle);
        let retire_coordinate = selected(LifecycleActionV2::RetireCoordinate, &basis, &lifecycle);
        let retire_receipt = compact(&basis, &lifecycle);

        let other_selection =
            TokenBehaviorSelectionV2::new(id(77), id(15)).expect("other realm selection");
        let count = usize::from(
            lifecycle_logical_account_count_v3(LifecycleActionV2::ActivateReceipt, 0)
                .expect("logical count"),
        );
        let mut lengths = vec![0_u32; count];
        *lengths.get_mut(1).expect("selection") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection width");
        *lengths.get_mut(4).expect("basis") = u32::try_from(basis.len()).expect("basis width");
        *lengths.get_mut(14).expect("descriptor") =
            u32::try_from(descriptor_width()).expect("descriptor width");
        let foreign_receipt =
            build_rational_lifecycle_selected_bundle_v6(RationalLifecycleSelectedBundleInputV6 {
                action: LifecycleActionV2::ActivateReceipt,
                account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
                    logical_data_lengths: &lengths,
                    product_basis: &basis,
                },
                token_behavior_selection: other_selection,
                kind: RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1,
                root_schema: id(42),
                lifecycle_policy: &lifecycle,
                capacity_profile: id(43),
                root_state_bytes: 64,
            })
            .expect("foreign bundle");

        assert_eq!(
            build_rational_lifecycle_program_set_v6(RationalLifecycleProgramSetInputV6 {
                token_behavior_selection: selection(),
                activate_receipt: &foreign_receipt,
                activate_coordinate: &activate_coordinate,
                retire_coordinate: &retire_coordinate,
                retire_receipt: &retire_receipt,
            }),
            Err(Error::ContentIdentity)
        );
    }
}
