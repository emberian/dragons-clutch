#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived specialization of generic Rational Representation V2 actions
//! for transferable Bearer basis-vector claims.
//!
//! This crate owns no asset derivation, balance observation, request encoding,
//! or physical authority. The generic Rational operator derives those facts
//! from authenticated chain state; this layer proves that a finalized
//! descriptor which IS a Bearer basis vector selects the one runtime outcome
//! whose coefficient equals the exact denominator.
//!
//! **The specialization is chosen by the descriptor, not by the builder.** A
//! descriptor that is not a basis vector at any coordinate takes the generic
//! Rational constructors unchanged, because the operator must be able to build
//! what the chain admits and the chain denominates a fractional descriptor
//! today. Before 2026-09-01 the gate was hardcoded on the selected builders and
//! absent from the full-width ones, so this crate would issue a receipt whose
//! selected actions it then refused to build.

mod hot_account_profile_v3;
mod hot_artifacts_v3;
mod hot_bundle_v3;
mod hot_effect_v3;
mod hot_terminal_v3;
mod hot_transaction_v3;
mod open_capability_set_v3;
mod open_lifecycle_policy_v5;
mod open_release_v1;
mod open_selected_transaction_v3;
mod open_selected_v3;
mod open_structured_transaction_v3;
mod open_structured_v3;
#[cfg(test)]
mod test_open_fixture_v3;

/// The action a caller must name to select one of this crate's five builders.
///
/// Re-exported because every public bundle-input struct here carries an `action`
/// field of this type: without this line a caller cannot construct one without
/// depending on a crate this API never mentions.
pub use dclutch_claims::rational::RepresentationActionV2;
pub use hot_account_profile_v3::{
    RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3, RationalTerminalAccountProfileInputV3,
    encode_rational_terminal_account_profile_v3,
};
pub use hot_artifacts_v3::{
    RATIONAL_TERMINAL_REQUEST_PROFILE_BYTES_V3, RATIONAL_TERMINAL_TRANSITION_BYTES_V3,
    encode_rational_terminal_request_profile_v3, encode_rational_terminal_transition_v3,
};
pub use hot_bundle_v3::{
    RATIONAL_TERMINAL_DESCRIPTOR_BYTES_V3, RATIONAL_TERMINAL_STRATEGY_BYTES_V3,
    RationalTerminalHotBundleInputV3, RationalTerminalHotBundleV3,
    RationalTerminalSelectedBundleInputV6, build_rational_terminal_hot_bundle_v3,
    build_rational_terminal_selected_bundle_v6,
    validate_rational_terminal_hot_bundle_for_authenticated_selection_v3,
    validate_rational_terminal_hot_bundle_v3,
};
pub use hot_effect_v3::{
    RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3, RATIONAL_TERMINAL_EFFECT_BYTES_V3,
    RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3, RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3,
    encode_rational_terminal_effect_v3,
};
pub use hot_terminal_v3::{ConstructedHotTerminalV3, construct_chain_hot_redeem_terminal_v3};
pub use hot_transaction_v3::{
    CheckedRationalHotOuterReleaseV3, RationalTerminalHotInstructionV3, RationalTerminalHotStateV3,
    build_rational_terminal_hot_instruction_v3,
};
pub use open_capability_set_v3::{
    RationalOpenCapabilityProgramSetInputV3, RationalOpenCapabilityProgramSetInputV6,
    RationalOpenCapabilityProgramSetV3, build_rational_open_capability_program_set_v3,
    build_rational_open_capability_program_set_v6,
    validate_rational_open_capability_program_set_v3,
};
pub use open_lifecycle_policy_v5::{
    OPEN_CAPABILITY_LIFECYCLE_POLICY_BYTES_V5, encode_open_capability_lifecycle_policy_v5,
};
pub use open_release_v1::{
    OPEN_CAPABILITY_SELECTED_ACTION_COUNT_V1, OPEN_CAPABILITY_SELECTED_ACTIONS_V1,
    OpenCapabilityActionArtifactBytesV1, OpenCapabilityArtifactReleaseBytesV1,
    OpenCapabilityArtifactSelectionV1, OpenCapabilityJoinedReleaseV1,
    authenticate_open_capability_release_v1,
};
pub use open_selected_transaction_v3::{
    ConstructedHotOpenSelectedV3, RationalOpenSelectedHotInstructionV3,
    RationalOpenSelectedHotStateV3, build_rational_open_selected_hot_instruction_v3,
    construct_chain_hot_denominate_v3, construct_chain_hot_reconstitute_v3,
};
pub use open_selected_v3::{
    RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3, RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3,
    RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3, RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3,
    RationalOpenSelectedBundleInputV6, RationalOpenSelectedHotBundleInputV3,
    RationalOpenSelectedHotBundleV3, build_rational_open_selected_bundle_v6,
    build_rational_open_selected_hot_bundle_v3,
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3,
    validate_rational_open_selected_hot_bundle_v3,
};
pub use open_structured_transaction_v3::{
    ConstructedHotOpenStructuredV3, RationalOpenStructuredHotInstructionV3,
    RationalOpenStructuredHotStateV3, build_rational_open_structured_hot_instruction_v3,
    construct_chain_hot_issue_structured_v3, construct_chain_hot_unwrap_structured_v3,
};
pub use open_structured_v3::{
    RATIONAL_OPEN_STRUCTURED_COMMON_IDENTITIES_V3, RATIONAL_OPEN_STRUCTURED_COMMON_SCALARS_V3,
    RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3, RATIONAL_OPEN_STRUCTURED_ITEM_ACCOUNTS_V3,
    RATIONAL_OPEN_STRUCTURED_ITEM_IDENTITIES_V3, RATIONAL_OPEN_STRUCTURED_ITEM_SCALARS_V3,
    RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
    RATIONAL_OPEN_STRUCTURED_REQUEST_BASE_OPERATIONS_V3,
    RATIONAL_OPEN_STRUCTURED_REQUEST_ROW_OPERATIONS_V3, RationalOpenStructuredHotBundleInputV3,
    RationalOpenStructuredHotBundleV3, RationalOpenStructuredSelectedBundleInputV6,
    build_rational_open_structured_hot_bundle_v3,
    build_rational_open_structured_selected_bundle_v6,
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3,
    validate_rational_open_structured_hot_bundle_v3,
};

use crate::rational_representation::{
    ConstructedInstructionV2, RationalObservationV2, SelectedActionInputV2, TerminalObservationV2,
};
use dclutch_claims::bearer::{BearerBindingV2, BearerDescriptorV2};
use dclutch_claims::composition::{CompositionExposureBundleV3, RecordAdmissionV3};
use dclutch_claims::rational_kernel::{DescriptorAdmissionV2, RepresentationDescriptorV2};
use solana_program::{hash::hash, pubkey::Pubkey};

/// Stable operator construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The chain-derived generic Rational operator refused the observation.
    ChainOperator(crate::rational_representation::Error),
    /// The safe Rational terminal Hot contract refused the projected message.
    HotContract(dclutch_claims::rational::Error),
    /// The caller supplied a parent even though Hot owns that digest.
    NonCanonicalParent,
    /// Independent family specialization differed from the chain-derived child.
    HotChildMismatch,
    /// Typed terminal RequestProfile artifact encoding refused.
    RequestProfileArtifact(dclutch_vm::request_profile::Error),
    /// Typed terminal TransitionVM artifact encoding refused.
    TransitionArtifact(dclutch_vm::v3::Error),
    /// Typed terminal EffectProgram artifact encoding refused.
    EffectArtifact(dclutch_vm::effect::v3::Error),
    /// Typed EffectProgram successor envelope encoding or decoding refused.
    EffectArtifactV4(dclutch_vm::effect::v4::ErrorV4),
    /// Typed terminal AccountProfile artifact encoding refused.
    AccountProfileArtifact(dclutch_vm::account_profile::v2::Error),
    /// Logical account observations differed from the declared frame.
    AccountProfileInput,
    /// Hostile decoding of the injected ProductBasisV3 artifact refused.
    ///
    /// Split out of `AccountProfileInput`, whose own sentence used to name two
    /// unrelated accusations -- "ProductBasisV3 bytes OR logical account
    /// observations" -- and published one code for both. Four sites decode the
    /// basis and the codec already tells them whether it was width, magic,
    /// schema or a zero identity; they threw that away and left the reader
    /// unable to tell a malformed artifact from a miscounted frame.
    ProductBasis(dclutch_product::payoff::runtime_v3::Error),
    /// The canonical Token-2022 behavior selection was not exact.
    TokenBehavior(dclutch_custody::token_svm::Error),
    /// A semantic coordinate or computed artifact digest was zero.
    ContentIdentity,
    /// Exact interpreted ExecutionStrategy construction or join refused.
    ExecutionStrategy(dclutch_market::execution_strategy::v2::Error),
    /// CapabilityProgramV4 construction or hostile decoding refused.
    CapabilityDescriptor(dclutch_market::capability_program::Error),
    /// Successor lifecycle artifact decoding or AccountProfile join refused.
    LifecycleArtifact(dclutch_vm::account_profile::lifecycle_v3::Error),
    /// Schema-bound CapabilityProgramSetV2 construction or selection refused.
    CapabilityProgramSet(dclutch_market::capability_program::set_v2::ProgramSetErrorV2),
    /// Independently decoded artifact banks did not have one exact geometry.
    ArtifactGeometry,
    /// The descriptor named more coordinates than the derived artifact ceiling
    /// admits.
    ///
    /// Split out of `AccountProfileInput`, where it was one of six conjuncts and
    /// so published "the account profile inputs were wrong" for a refusal that
    /// is neither about accounts nor about inputs. The ceiling is
    /// `RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3`, itself derived from
    /// how many RequestProfile operations fit in the packet allowance, so this
    /// refusal is the K cliff and the two numbers say where it stands.
    CoordinateCeiling {
        /// The descriptor's coordinate count.
        requested: u32,
        /// The largest coordinate count the artifact admits.
        ceiling: u32,
    },
    /// A built instruction list did not have the length its file declares.
    ///
    /// This is split out of `ArtifactGeometry` because it is not a geometry
    /// disagreement between two parties -- it is one file disagreeing with
    /// itself, and the two numbers locate it exactly. Physical ABI v3 shortened
    /// three instruction lists and left three declared counts behind; each one
    /// surfaced as an undifferentiated geometry or length complaint naming no
    /// field, and each had to be rediscovered by probe. The numbers ride the
    /// refusal now. Where a list has a fixed length, prefer declaring it in the
    /// array type instead, which makes the same drift a compile error.
    InstructionCount {
        /// The count the file states beside the list.
        declared: usize,
        /// The count the list actually reached.
        built: usize,
    },
    /// Checked Hot envelope or exact physical account construction refused.
    HotInstruction,
    /// The common Hot ABI refused the envelope, and says which of its four
    /// causes it was: width, magic, profile, or a zero identity.
    HotEnvelope(dclutch_market::capability_program::hot_v3::HotExecutionErrorV3),
    /// The finalized descriptor bytes or their graph join refused.
    BearerDescriptor(dclutch_claims::rational_kernel::Error),
    /// The finalized composition exposure bundle refused.
    BearerExposure(dclutch_claims::composition::Error),
    /// The descriptor IS a Bearer basis vector and the request named another
    /// coordinate. Carries the specialization's own refusal.
    ///
    /// It no longer fires for a descriptor that is not a basis vector at all:
    /// see [`authenticate_basis_bytes`].
    NotBearer(dclutch_claims::bearer::Error),
}

/// Result alias for operator construction.
pub type Result<T> = core::result::Result<T, Error>;

/// Construct the exact chain-derived Denominate instruction and additionally
/// require that the finalized descriptor is a Bearer basis vector at the
/// selected runtime outcome.
pub fn construct_chain_denominate(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
) -> Result<ConstructedInstructionV2> {
    let built = crate::rational_representation::construct_denominate(observation, input)
        .map_err(Error::ChainOperator)?;
    authenticate_chain_basis(observation, built.representation_authority, input.outcome)?;
    Ok(built)
}

/// Construct the exact chain-derived Reconstitute instruction under the same
/// finalized Bearer basis-vector gate.
pub fn construct_chain_reconstitute(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
) -> Result<ConstructedInstructionV2> {
    let built = crate::rational_representation::construct_reconstitute(observation, input)
        .map_err(Error::ChainOperator)?;
    authenticate_chain_basis(observation, built.representation_authority, input.outcome)?;
    Ok(built)
}

/// Construct the exact chain-derived terminal Bearer redemption instruction.
/// Claims and Custody remain the sole payout and collateral authorities.
pub fn construct_chain_redeem_terminal(
    observation: RationalObservationV2<'_>,
    terminal: TerminalObservationV2<'_>,
) -> Result<ConstructedInstructionV2> {
    let built = crate::rational_representation::construct_redeem_terminal(observation, terminal)
        .map_err(Error::ChainOperator)?;
    authenticate_chain_basis(
        observation,
        built.representation_authority,
        terminal.outcome,
    )?;
    Ok(built)
}

fn authenticate_chain_basis(
    observation: RationalObservationV2<'_>,
    representation_authority: Pubkey,
    selected_outcome: u32,
) -> Result<()> {
    authenticate_basis_bytes(
        observation.descriptor.raw.data,
        observation.graph.raw.data,
        representation_authority,
        selected_outcome,
    )
}

fn authenticate_basis_bytes(
    descriptor_bytes: &[u8],
    graph_bytes: &[u8],
    representation_authority: Pubkey,
    selected_outcome: u32,
) -> Result<()> {
    let descriptor_digest = hash(descriptor_bytes).to_bytes();
    let graph_digest = hash(graph_bytes).to_bytes();
    let descriptor = RepresentationDescriptorV2::decode(
        descriptor_bytes,
        DescriptorAdmissionV2 {
            selected_descriptor_id: descriptor_digest,
            finalized_descriptor_id: descriptor_digest,
            recomputed_descriptor_digest: descriptor_digest,
            finalized_descriptor_digest: descriptor_digest,
            record_authenticated: true,
            derived_representation_authority: representation_authority.to_bytes(),
            authority_derivation_authenticated: true,
        },
    )
    .map_err(Error::BearerDescriptor)?;
    let exposure = CompositionExposureBundleV3::decode(
        graph_bytes,
        RecordAdmissionV3 {
            selected_id: descriptor.graph_id(),
            finalized_id: descriptor.graph_id(),
            recomputed_digest: graph_digest,
            finalized_digest: graph_digest,
            record_authenticated: true,
        },
    )
    .map_err(Error::BearerExposure)?;
    // REQUIRED OF BOTH FAMILIES, and checked before the specialization is
    // chosen: the descriptor's graph join, and the coordinate being inside the
    // representation. `BearerDescriptorV2::authenticate` checks both itself,
    // and every other equality in its binding is a self-comparison here, so
    // lifting exactly these two out is what leaves `NotBasisVector` as the only
    // reason it can still refuse below.
    descriptor
        .authenticate_exposure(exposure)
        .map_err(Error::BearerDescriptor)?;
    if selected_outcome >= descriptor.outcome_count() {
        return Err(Error::NotBearer(
            dclutch_claims::bearer::Error::BindingMismatch,
        ));
    }
    let binding = |outcome| BearerBindingV2 {
        descriptor_id: descriptor.descriptor_id(),
        exposure_id: descriptor.graph_id(),
        exposure_digest: descriptor.graph_digest(),
        root_id: descriptor.root_id(),
        market: descriptor.market_id(),
        release_set: descriptor.release_set_id(),
        receipt_mint: descriptor.receipt_mint(),
        token_program: descriptor.token_program(),
        representation_authority: descriptor.representation_authority(),
        representation_width: descriptor.outcome_count(),
        denominator: descriptor.denominator(),
        selected_outcome: outcome,
    };
    // THE SPECIALIZATION IS CHOSEN BY THE DESCRIPTOR, not by which builder the
    // caller reached for.
    //
    // A Bearer descriptor is `D * e_k`. For one the gate is exactly as strict
    // as it has always been: the request must name `k`, and naming any other
    // coordinate returns the specialization's own refusal. For a descriptor
    // that is not a basis vector at any coordinate, the gate does not apply --
    // the generic Rational constructors own it, which is the same path the
    // full-width issue actions already take bare
    // (`open_structured_transaction_v3.rs:189-203`).
    //
    // Why the asymmetry had to go rather than be made symmetric: THE OPERATOR
    // MUST BE ABLE TO BUILD WHAT THE CHAIN ADMITS. The chain denominates a
    // fractional descriptor on real ELFs today -- the campaign's `[2, 3, 5]`
    // over `7`, in `real_sbf_open_actions_are_exact_and_conserved` -- so a
    // hardcoded Bearer gate on the selected builders made this operator issue a
    // receipt whose selected actions it would then refuse to build.
    //
    // The classification ASKS THE CONTRACT rather than restating its rule: a
    // second implementation of "is this a basis vector" in the operator is
    // exactly the mirror this crate exists to avoid. The two checks above are
    // lifted so that a failure here can only be the basis-vector shape.
    match BearerDescriptorV2::authenticate(descriptor, exposure, binding(selected_outcome)) {
        Ok(_) => Ok(()),
        Err(refusal) => {
            let mut outcome = 0_u32;
            while outcome < descriptor.outcome_count() {
                if BearerDescriptorV2::authenticate(descriptor, exposure, binding(outcome)).is_ok()
                {
                    return Err(Error::NotBearer(refusal));
                }
                outcome = outcome.checked_add(1).ok_or(Error::ArtifactGeometry)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::composition::{
        CompositionExposureInputV3, CompositionExposureRowInputV3, CompositionExposureTermV3,
        composition_exposure_bytes_v3, encode_composition_exposure_v3_atomic,
    };
    use dclutch_claims::rational::RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2;
    use dclutch_claims::rational_kernel::{
        DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
    };
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;

    const WIDTH: u32 = 3;
    const SELECTED: u32 = 1;
    const DENOMINATOR: u64 = 10;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture offset")
            .copy_from_slice(value);
    }

    fn put_u32(output: &mut [u8], offset: usize, value: u32) {
        put(output, offset, &value.to_le_bytes());
    }

    fn put_u64(output: &mut [u8], offset: usize, value: u64) {
        put(output, offset, &value.to_le_bytes());
    }

    fn exposure_fixture() -> Vec<u8> {
        let terms = [
            [CompositionExposureTermV3 {
                product_coordinate: 0,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 1,
                numerator: 1,
            }],
            [CompositionExposureTermV3 {
                product_coordinate: 2,
                numerator: 1,
            }],
        ];
        let rows = [
            CompositionExposureRowInputV3 {
                node_id: id(10),
                denominator: 1,
                terms: &terms[0],
            },
            CompositionExposureRowInputV3 {
                node_id: id(11),
                denominator: 1,
                terms: &terms[1],
            },
            CompositionExposureRowInputV3 {
                node_id: id(12),
                denominator: 1,
                terms: &terms[2],
            },
        ];
        let width = composition_exposure_bytes_v3(WIDTH, WIDTH).expect("width");
        let mut scratch = vec![0_u8; width];
        let mut bytes = vec![0_u8; width];
        encode_composition_exposure_v3_atomic(
            CompositionExposureInputV3 {
                market: id(2),
                result_domain: id(30),
                release_set: id(3),
                product_basis: id(31),
                representation_basis: id(32),
                graph_id: id(33),
                product_width: WIDTH,
                rows: &rows,
            },
            &mut scratch,
            &mut bytes,
        )
        .expect("exposure");
        bytes
    }

    fn descriptor_fixture() -> Vec<u8> {
        let mut bytes =
            vec![0_u8; DESCRIPTOR_HEADER_BYTES + WIDTH as usize * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut bytes, 0, &DESCRIPTOR_MAGIC_V3);
        put(&mut bytes, 8, &3_u16.to_le_bytes());
        put(&mut bytes, 16, &id(20));
        put(&mut bytes, 48, &id(21));
        put(&mut bytes, 80, &id(14));
        put(&mut bytes, 112, &id(2));
        put(&mut bytes, 144, &id(3));
        put(&mut bytes, 176, &id(4));
        put(&mut bytes, 208, &TOKEN_2022_PROGRAM_ID);
        put_u32(&mut bytes, 240, WIDTH);
        put_u64(&mut bytes, 248, DENOMINATOR);
        put_u64(
            &mut bytes,
            DESCRIPTOR_HEADER_BYTES + DESCRIPTOR_COEFFICIENT_BYTES,
            DENOMINATOR,
        );
        bytes
    }

    /// THE SPECIALIZATION IS CHOSEN BY THE DESCRIPTOR, and the gate on a Bearer
    /// one is exactly as strict as it ever was.
    ///
    /// The last assertion is the behaviour that changed, and it changed on a
    /// principle: THE OPERATOR MUST BE ABLE TO BUILD WHAT THE CHAIN ADMITS. The
    /// chain denominates a fractional descriptor on real ELFs today, so an
    /// operator that refuses to build one is refusing a capability the protocol
    /// has -- while the same operator's full-width issue path builds against
    /// that descriptor with no gate at all, which is how it came to issue a
    /// receipt whose selected actions it would then refuse to unwind.
    #[test]
    fn chain_gate_binds_a_bearer_descriptor_and_steps_aside_for_a_fractional_one() {
        let graph_bytes = exposure_fixture();
        let mut descriptor_bytes = descriptor_fixture();
        put(&mut descriptor_bytes, 48, &hash(&graph_bytes).to_bytes());
        let claims_program = id(60);
        let authority_for = |bytes: &[u8]| {
            Pubkey::find_program_address(
                &[
                    RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
                    &hash(bytes).to_bytes(),
                ],
                &Pubkey::new_from_array(claims_program),
            )
            .0
        };
        let authority = authority_for(&descriptor_bytes);

        // `descriptor_fixture` is `[0, D, 0]`: a Bearer basis vector at
        // `SELECTED`. Its coordinate is admitted, every other coordinate is
        // refused with the specialization's OWN error rather than a coarse one,
        // and a coordinate outside the representation is refused before the
        // shape is ever consulted.
        assert_eq!(
            authenticate_basis_bytes(&descriptor_bytes, &graph_bytes, authority, SELECTED),
            Ok(())
        );
        assert_eq!(
            authenticate_basis_bytes(&descriptor_bytes, &graph_bytes, authority, 0),
            Err(Error::NotBearer(
                dclutch_claims::bearer::Error::NotBasisVector
            ))
        );
        assert_eq!(
            authenticate_basis_bytes(&descriptor_bytes, &graph_bytes, authority, WIDTH),
            Err(Error::NotBearer(
                dclutch_claims::bearer::Error::BindingMismatch
            ))
        );

        // Two coefficients at the denominator is a basis vector at NO
        // coordinate, so the Bearer gate does not apply to it and the generic
        // Rational constructors own it. This assertion used to read
        // `Err(NotBearer)`.
        let mut non_basis = descriptor_bytes.clone();
        put_u64(&mut non_basis, DESCRIPTOR_HEADER_BYTES, DENOMINATOR);
        assert_eq!(
            authenticate_basis_bytes(
                &non_basis,
                &graph_bytes,
                authority_for(&non_basis),
                SELECTED
            ),
            Ok(())
        );

        // And the campaign's own shape: a genuinely fractional vector, admitted
        // at every coordinate it carries. This is the descriptor the chain
        // denominates today and the operator could not build for.
        let mut fractional = descriptor_bytes;
        put_u64(&mut fractional, 248, 7);
        for (outcome, coefficient) in [2_u64, 3, 5].into_iter().enumerate() {
            put_u64(
                &mut fractional,
                DESCRIPTOR_HEADER_BYTES + outcome * DESCRIPTOR_COEFFICIENT_BYTES,
                coefficient,
            );
        }
        let fractional_authority = authority_for(&fractional);
        for outcome in 0..WIDTH {
            assert_eq!(
                authenticate_basis_bytes(&fractional, &graph_bytes, fractional_authority, outcome),
                Ok(()),
                "the chain denominates this descriptor; the operator must build it"
            );
        }
    }
}
