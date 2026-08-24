//! Fixed-capacity client construction for the current Direct `80/1` family.
//!
//! These values prove only canonical wire construction. They do not promote a
//! capability, choose accounts, authenticate Product state, or claim that a
//! deployed release executes the action; release admission remains a separate
//! checked operator/onchain boundary.

use clutch_solana_layout::direct_market_v1::{
    DirectAdmitOrderPayloadV1,
    DirectSubmitCandidatePayloadV1, DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1,
    DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1,
};
use clutch_solana_layout::registry::{
    DirectMarketAction, ExtensionAction, ExtensionEnvelope, ExtensionFamily,
    EXTENSION_ENVELOPE_BYTES, DIRECT_MARKET_FAMILY_TAG, DIRECT_MARKET_FAMILY_VERSION,
};
use clutch_solana_reference::ExtensionRequest;

/// Largest current Direct family payload.
pub const DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1: usize =
    DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1;
/// Largest exact replay-bearing request for the current Direct family.
pub const DIRECT_MARKET_CLIENT_MAX_REQUEST_BYTES_V1: usize =
    clutch_solana_reference::MAX_REQUEST_LEN - clutch_solana_layout::MAX_INTENT_BYTES
        + EXTENSION_ENVELOPE_BYTES
        + DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1;

/// One exact family-local action and its canonical fixed-capacity payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectMarketClientPayloadV1 {
    action: DirectMarketAction,
    bytes: [u8; DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1],
    len: u8,
}

impl DirectMarketClientPayloadV1 {
    /// Construct action 2 from the authoritative layout payload.
    pub fn admit_order(value: DirectAdmitOrderPayloadV1) -> Result<Self, DirectClientRefusalV1> {
        let mut bytes = [0; DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1];
        let mut encoded = [0; DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1];
        value
            .encode_into(&mut encoded)
            .map_err(|_| DirectClientRefusalV1::NonCanonicalPayload)?;
        bytes[..DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1].copy_from_slice(&encoded);
        Ok(Self {
            action: DirectMarketAction::AdmitOrder,
            bytes,
            len: 80,
        })
    }

    /// Construct action 5 from the authoritative compact RelationV2 payload.
    #[must_use]
    pub fn submit_candidate(value: DirectSubmitCandidatePayloadV1) -> Self {
        let mut bytes = [0; DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1];
        let mut encoded = [0; DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1];
        value.encode_into(&mut encoded);
        bytes[..DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1].copy_from_slice(&encoded);
        Self {
            action: DirectMarketAction::SubmitCandidate,
            bytes,
            len: 24,
        }
    }

    /// Construct one of the exact empty-payload lifecycle actions.
    pub const fn empty(action: DirectMarketAction) -> Result<Self, DirectClientRefusalV1> {
        match action {
            DirectMarketAction::InitializeMarket
            | DirectMarketAction::CancelOrder
            | DirectMarketAction::FreezeBook
            | DirectMarketAction::BeginVerification
            | DirectMarketAction::VerifyCandidate
            | DirectMarketAction::FinalizeSelection
            | DirectMarketAction::SettlePair
            | DirectMarketAction::LapseEmpty
            | DirectMarketAction::LapseUnselected
            | DirectMarketAction::LapseSelected
            | DirectMarketAction::RetireTerminal => Ok(Self {
                action,
                bytes: [0; DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1],
                len: 0,
            }),
            DirectMarketAction::AdmitOrder
            | DirectMarketAction::SubmitCandidate => {
                Err(DirectClientRefusalV1::ActionPayloadMismatch)
            }
        }
    }

    /// Central successor family tag.
    #[must_use]
    pub const fn family_tag(self) -> u8 { DIRECT_MARKET_FAMILY_TAG }

    /// Central successor family version.
    #[must_use]
    pub const fn family_version(self) -> u8 { DIRECT_MARKET_FAMILY_VERSION }

    /// Exact family-local action.
    #[must_use]
    pub const fn action(self) -> DirectMarketAction { self.action }

    /// Canonical payload bytes without the unused fixed-capacity tail.
    #[must_use]
    pub fn payload(&self) -> &[u8] { &self.bytes[..usize::from(self.len)] }
}

/// Exact outer Clutch request carrying one canonical Direct `80/1` payload.
///
/// This is construction evidence only. In particular, producing these bytes
/// does not assert that the exact action is enabled by a checked release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectMarketClientRequestV1 {
    bytes: [u8; DIRECT_MARKET_CLIENT_MAX_REQUEST_BYTES_V1],
    len: u8,
}

impl DirectMarketClientRequestV1 {
    /// Encode the replay sequence and typed Direct action through the sole
    /// authoritative successor-envelope and outer-request codecs.
    pub fn encode(
        sequence: u64,
        payload: &DirectMarketClientPayloadV1,
    ) -> Result<Self, DirectClientRefusalV1> {
        if (payload.action() == DirectMarketAction::InitializeMarket) != (sequence == 0) {
            return Err(DirectClientRefusalV1::ReplayCoordinateMismatch);
        }
        let envelope = ExtensionEnvelope {
            family: ExtensionFamily::DirectMarket,
            action: ExtensionAction::DirectMarket(payload.action()),
            payload: payload.payload(),
        };
        let request = ExtensionRequest { sequence, envelope };
        let mut bytes = [0; DIRECT_MARKET_CLIENT_MAX_REQUEST_BYTES_V1];
        let exact = request
            .encode(&mut bytes)
            .map_err(|_| DirectClientRefusalV1::NonCanonicalRequest)?;
        let len = u8::try_from(exact)
            .map_err(|_| DirectClientRefusalV1::NonCanonicalRequest)?;
        Ok(Self { bytes, len })
    }

    /// Canonical request bytes without the unused fixed-capacity tail.
    #[must_use]
    pub fn bytes(&self) -> &[u8] { &self.bytes[..usize::from(self.len)] }
}

/// Client-side canonical-construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectClientRefusalV1 {
    /// A nonempty action was requested through the empty-payload constructor.
    ActionPayloadMismatch,
    /// The authoritative layout encoder refused the payload.
    NonCanonicalPayload,
    /// The authoritative registry or replay-request encoder refused the join.
    NonCanonicalRequest,
    /// Action 1 alone uses sequence zero; every successor consumes nonzero replay.
    ReplayCoordinateMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::{PartialPolicy, Side};

    #[test]
    fn zero_price_order_is_canonical_client_input() {
        let payload = DirectMarketClientPayloadV1::admit_order(DirectAdmitOrderPayloadV1 {
            order_id: [7; 32],
            side: Side::Buy,
            outcome: 0,
            partial_policy: PartialPolicy::Allow,
            quantity: 2,
            minimum_fill: 1,
            expiry_epoch: 5,
            limit_price_units_per_egg: 0,
        })
        .unwrap();
        assert_eq!(payload.family_tag(), 80);
        assert_eq!(payload.family_version(), 1);
        assert_eq!(payload.action(), DirectMarketAction::AdmitOrder);
        assert_eq!(payload.payload().len(), 80);
        assert_eq!(&payload.payload()[64..80], &[0; 16]);
    }

    #[test]
    fn payload_classes_are_disjoint() {
        assert_eq!(
            DirectMarketClientPayloadV1::empty(DirectMarketAction::AdmitOrder),
            Err(DirectClientRefusalV1::ActionPayloadMismatch)
        );
        for action in [
            DirectMarketAction::InitializeMarket,
            DirectMarketAction::CancelOrder,
            DirectMarketAction::FreezeBook,
            DirectMarketAction::BeginVerification,
            DirectMarketAction::VerifyCandidate,
            DirectMarketAction::FinalizeSelection,
            DirectMarketAction::SettlePair,
            DirectMarketAction::LapseEmpty,
            DirectMarketAction::LapseUnselected,
            DirectMarketAction::LapseSelected,
            DirectMarketAction::RetireTerminal,
        ] {
            assert_eq!(
                DirectMarketClientPayloadV1::empty(action)
                    .unwrap()
                    .payload(),
                &[]
            );
        }
    }

    #[test]
    fn request_owner_refuses_wrong_replay_coordinate_for_every_consumer() {
        let init = DirectMarketClientPayloadV1::empty(
            DirectMarketAction::InitializeMarket,
        ).unwrap();
        assert_eq!(
            DirectMarketClientRequestV1::encode(1, &init),
            Err(DirectClientRefusalV1::ReplayCoordinateMismatch),
        );
        let freeze = DirectMarketClientPayloadV1::empty(DirectMarketAction::FreezeBook).unwrap();
        assert_eq!(
            DirectMarketClientRequestV1::encode(0, &freeze),
            Err(DirectClientRefusalV1::ReplayCoordinateMismatch),
        );
    }

    #[test]
    fn exact_outer_request_round_trips_without_promoting_capability() {
        let payload = DirectMarketClientPayloadV1::empty(
            DirectMarketAction::BeginVerification,
        )
        .unwrap();
        let request = DirectMarketClientRequestV1::encode(9, &payload).unwrap();
        let decoded = clutch_solana_reference::ExtensionRequest::decode(request.bytes()).unwrap();
        assert_eq!(decoded.sequence, 9);
        assert_eq!(decoded.envelope.family, ExtensionFamily::DirectMarket);
        assert_eq!(
            decoded.envelope.action,
            ExtensionAction::DirectMarket(DirectMarketAction::BeginVerification)
        );
        assert_eq!(decoded.envelope.payload, &[]);
    }
}
