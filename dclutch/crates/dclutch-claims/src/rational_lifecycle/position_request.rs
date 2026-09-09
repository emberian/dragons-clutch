//! Sole nested protocol-Position request projection for coordinate lifecycle.
use super::{Error, LifecycleActionV2, LifecycleRequestV2};
use crate::protocol_position_v2::{
    ProtocolPositionActionV2, ProtocolPositionErrorV2, ProtocolPositionOwnerKindV2,
    ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
};

/// Located refusal while projecting the nested coordinate request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinatePositionRequestErrorV2 {
    /// Receipt-wide lifecycle actions have no nested Position transition.
    Action,
    /// The sole coordinate row could not be read.
    Coordinate(Error),
    /// The projected request violates the protocol-Position contract.
    Position(ProtocolPositionErrorV2),
}

impl LifecycleRequestV2<'_> {
    /// Project the exact protocol-Position Admit or Close owned by this coordinate.
    ///
    /// The adapter supplies the digest of the exact canonical Claims lifecycle
    /// wire. It still owns hashing and authenticating physical PDA identities,
    /// privileges and account state. No owner, row or action override is accepted.
    pub fn protocol_position_request(
        self,
        lifecycle_digest: [u8; 32],
    ) -> Result<ProtocolPositionRequestV2, CoordinatePositionRequestErrorV2> {
        let header = self.header();
        let (action, presence) = match header.action {
            LifecycleActionV2::ActivateCoordinate => (
                ProtocolPositionActionV2::Admit,
                ProtocolPositionPresenceV2::Vacant,
            ),
            LifecycleActionV2::RetireCoordinate => (
                ProtocolPositionActionV2::Close,
                ProtocolPositionPresenceV2::Existing,
            ),
            LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt => {
                return Err(CoordinatePositionRequestErrorV2::Action);
            }
        };
        let row = self
            .coordinates()
            .next()
            .ok_or(CoordinatePositionRequestErrorV2::Coordinate(
                Error::InvalidLength,
            ))?
            .map_err(CoordinatePositionRequestErrorV2::Coordinate)?;
        ProtocolPositionRequestV2 {
            action,
            owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
            presence,
            release_set: header.release_set,
            market: header.market,
            position_owner: row.claims_custody_owner,
            parent_request_digest: lifecycle_digest,
            rent_credit: header.rent_credit,
            rent_program: header.rent_program,
            generation: header.generation,
            expected_market_revision: header.expected_claims_market_revision,
            expected_position_revision: row.expected_position_revision,
            observed_position_lamports: row.observed_position_lamports,
            observed_admission_lamports: row.observed_admission_lamports,
            position_rent_principal: row.position_rent_principal,
            admission_rent_principal: row.admission_rent_principal,
            capability_descriptor: header.descriptor_id,
            capability_outcome: row.outcome,
        }
        .new()
        .map_err(CoordinatePositionRequestErrorV2::Position)
    }
}
