#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived specialization of generic Rational Representation V2 actions
//! for transferable Bearer basis-vector claims.
//!
//! This crate owns no asset derivation, balance observation, request encoding,
//! or physical authority. The generic Rational operator derives those facts
//! from authenticated chain state; this layer only proves that the finalized
//! descriptor and graph select one runtime outcome with coefficient equal to
//! the exact denominator.

mod hot_account_profile_v3;
mod hot_artifacts_v3;
mod hot_bundle_v3;
mod hot_effect_v3;
mod hot_terminal_v3;
mod hot_transaction_v3;
mod open_selected_v3;

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
    build_rational_terminal_hot_bundle_v3, validate_rational_terminal_hot_bundle_v3,
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
pub use open_selected_v3::{
    RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3, RATIONAL_OPEN_SELECTED_COMMON_IDENTITIES_V3,
    RATIONAL_OPEN_SELECTED_COMMON_SCALARS_V3, RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3,
    RationalOpenSelectedHotBundleInputV3, RationalOpenSelectedHotBundleV3,
    build_rational_open_selected_hot_bundle_v3, validate_rational_open_selected_hot_bundle_v3,
};

use dclutch_bearer_v2_contract::BearerDescriptorV2;
use dclutch_rational_representation_v2_kernel::{
    ContentAdmissionV2, DescriptorAdmissionV2, RepresentationDescriptorV2, RepresentationGraphV2,
};
use dclutch_rational_representation_v2_operator::{
    ConstructedInstructionV2, RationalObservationV2, SelectedActionInputV2, TerminalObservationV2,
};
use solana_program::{hash::hash, pubkey::Pubkey};

/// Stable operator construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The chain-derived generic Rational operator refused the observation.
    ChainOperator(dclutch_rational_representation_v2_operator::Error),
    /// The safe Rational terminal Hot contract refused the projected message.
    HotContract(dclutch_rational_representation_v2_contract::Error),
    /// The caller supplied a parent even though Hot owns that digest.
    NonCanonicalParent,
    /// Independent family specialization differed from the chain-derived child.
    HotChildMismatch,
    /// Typed terminal RequestProfile artifact encoding refused.
    RequestProfileArtifact(dclutch_request_profile_contract::Error),
    /// Typed terminal TransitionVM artifact encoding refused.
    TransitionArtifact(dclutch_transition_vm::v3::Error),
    /// Typed terminal EffectProgram artifact encoding refused.
    EffectArtifact(dclutch_effect_kernel::v3::Error),
    /// Typed terminal AccountProfile artifact encoding refused.
    AccountProfileArtifact(dclutch_account_profile_contract::v2::Error),
    /// ProductBasisV3 bytes or logical account observations differed.
    AccountProfileInput,
    /// A semantic coordinate or computed artifact digest was zero.
    ContentIdentity,
    /// Exact interpreted ExecutionStrategy construction or join refused.
    ExecutionStrategy(dclutch_execution_strategy_contract::v2::Error),
    /// CapabilityProgramV3 construction or hostile decoding refused.
    CapabilityDescriptor(dclutch_capability_program_contract::Error),
    /// Independently decoded artifact banks did not have one exact geometry.
    ArtifactGeometry,
    /// Checked Hot envelope or exact physical account construction refused.
    HotInstruction,
    /// Finalized descriptor/graph bytes were not the selected Bearer basis vector.
    NotBearer,
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
    let built =
        dclutch_rational_representation_v2_operator::construct_denominate(observation, input)
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
    let built =
        dclutch_rational_representation_v2_operator::construct_reconstitute(observation, input)
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
    let built = dclutch_rational_representation_v2_operator::construct_redeem_terminal(
        observation,
        terminal,
    )
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
    .map_err(|_| Error::NotBearer)?;
    let graph = RepresentationGraphV2::decode(
        graph_bytes,
        ContentAdmissionV2 {
            selected_graph_id: descriptor.graph_id(),
            finalized_graph_id: descriptor.graph_id(),
            recomputed_graph_digest: graph_digest,
            finalized_graph_digest: graph_digest,
            record_authenticated: true,
        },
    )
    .map_err(|_| Error::NotBearer)?;
    BearerDescriptorV2::authenticate(
        descriptor,
        graph,
        dclutch_bearer_v2_contract::BearerBindingV2 {
            descriptor_id: descriptor.descriptor_id(),
            graph_id: descriptor.graph_id(),
            graph_digest: descriptor.graph_digest(),
            root_id: descriptor.root_id(),
            market: descriptor.market_id(),
            release_set: descriptor.release_set_id(),
            receipt_mint: descriptor.receipt_mint(),
            token_program: descriptor.token_program(),
            representation_authority: descriptor.representation_authority(),
            outcome_count: descriptor.outcome_count(),
            denominator: descriptor.denominator(),
            selected_outcome,
        },
    )
    .map_err(|_| Error::NotBearer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_rational_representation_v2_contract::RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2;
    use dclutch_rational_representation_v2_kernel::{
        DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
        GRAPH_HEADER_BYTES, GRAPH_MAGIC_V2, GRAPH_NODE_BYTES, SCHEMA_VERSION_V2,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

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

    fn graph_fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES + WIDTH as usize * 8];
        put(&mut bytes, 0, &GRAPH_MAGIC_V2);
        put(&mut bytes, 8, &SCHEMA_VERSION_V2.to_le_bytes());
        put(&mut bytes, 16, &id(20));
        put(&mut bytes, 48, &id(14));
        put_u32(&mut bytes, 80, WIDTH);
        put_u32(&mut bytes, 84, 1);
        put_u32(&mut bytes, 88, 0);
        put_u64(&mut bytes, 96, 100);
        put(&mut bytes, GRAPH_HEADER_BYTES, &id(14));
        *bytes.get_mut(GRAPH_HEADER_BYTES + 44).expect("kind") = 0;
        put_u64(&mut bytes, GRAPH_HEADER_BYTES + 48, u64::from(SELECTED));
        let exposure = GRAPH_HEADER_BYTES + GRAPH_NODE_BYTES;
        put_u64(&mut bytes, exposure, 0);
        put_u64(&mut bytes, exposure + 8, 100);
        put_u64(&mut bytes, exposure + 16, 0);
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

    #[test]
    fn chain_gate_refuses_non_basis_and_selected_outcome_substitution() {
        let graph_bytes = graph_fixture();
        let mut descriptor_bytes = descriptor_fixture();
        put(&mut descriptor_bytes, 48, &hash(&graph_bytes).to_bytes());
        let claims_program = id(60);
        let authority = Pubkey::find_program_address(
            &[
                RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
                &hash(&descriptor_bytes).to_bytes(),
            ],
            &Pubkey::new_from_array(claims_program),
        )
        .0;
        assert_eq!(
            authenticate_basis_bytes(&descriptor_bytes, &graph_bytes, authority, SELECTED),
            Ok(())
        );
        assert_eq!(
            authenticate_basis_bytes(&descriptor_bytes, &graph_bytes, authority, 0),
            Err(Error::NotBearer)
        );

        let mut non_basis = descriptor_bytes;
        put_u64(&mut non_basis, DESCRIPTOR_HEADER_BYTES, DENOMINATOR);
        let non_basis_authority = Pubkey::find_program_address(
            &[
                RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
                &hash(&non_basis).to_bytes(),
            ],
            &Pubkey::new_from_array(claims_program),
        )
        .0;
        assert_eq!(
            authenticate_basis_bytes(&non_basis, &graph_bytes, non_basis_authority, SELECTED,),
            Err(Error::NotBearer)
        );
    }
}
