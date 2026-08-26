//! Exact Hot V3 family projection for terminal Bearer redemption.

use dclutch_rational_representation_v2_contract::{
    RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3, RationalTerminalHotRequestV3, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_operator::{
    ConstructedInstructionV2, RationalObservationV2, TerminalObservationV2,
};
use solana_program::hash::hash;

use crate::{Error, Result, construct_chain_redeem_terminal};

/// One exact chain-derived Hot family request and its canonical Claims child.
///
/// The transaction builder uses `family_request` as the family payload inside
/// `HotExecutionEnvelopeV3`. The authenticated adapter recomputes
/// `family_digest`, specializes the exact Claims child, and invokes the
/// `claims_child` program/accounts through its release-selected caller PDA.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructedHotTerminalV3 {
    /// Exact wallet-facing Rational terminal family request.
    pub family_request: [u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3],
    /// SHA-256 of the exact family request; the canonical child parent context.
    pub family_digest: [u8; 32],
    /// Exact 49-account, 648-byte Claims child derived after the family digest.
    pub claims_child: ConstructedInstructionV2,
}

/// Construct one exact Hot V3 terminal intent from authenticated chain state.
///
/// `observation.parent_context` must be absent (`[0; 32]`) because the Hot
/// family digest is the only valid parent context. The generic Rational
/// operator is run twice: first with an internal disposable marker to project
/// the family request, then with the actual family digest. Finally the safe
/// contract independently specializes the family bytes and requires byte-for-
/// byte identity with the chain-derived child.
pub fn construct_chain_hot_redeem_terminal_v3(
    observation: RationalObservationV2<'_>,
    terminal: TerminalObservationV2<'_>,
) -> Result<ConstructedHotTerminalV3> {
    if observation.parent_context != [0; 32] {
        return Err(Error::NonCanonicalParent);
    }

    let template_observation = RationalObservationV2 {
        parent_context: [1; 32],
        ..observation
    };
    let template = construct_chain_redeem_terminal(template_observation, terminal)?;
    let template_request =
        RepresentationRequestV2::decode(&template.instruction.data).map_err(Error::HotContract)?;
    let mut family_request = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    let family =
        RationalTerminalHotRequestV3::from_child_into(template_request, &mut family_request)
            .map_err(Error::HotContract)?;
    let family_digest = hash(family.as_bytes()).to_bytes();

    let exact_observation = RationalObservationV2 {
        parent_context: family_digest,
        ..observation
    };
    let claims_child = construct_chain_redeem_terminal(exact_observation, terminal)?;
    let mut specialized = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
    family
        .specialize_child_into(family_digest, &mut specialized)
        .map_err(Error::HotContract)?;
    if claims_child.instruction.data.as_slice() != specialized {
        return Err(Error::HotChildMismatch);
    }
    Ok(ConstructedHotTerminalV3 {
        family_request,
        family_digest,
        claims_child,
    })
}
