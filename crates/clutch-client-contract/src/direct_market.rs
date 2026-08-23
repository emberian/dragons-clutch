//! Fixed-capacity client construction for the allocated Direct `80/1` family.
//!
//! These values prove only canonical wire construction. They do not promote a
//! disabled capability, choose accounts, authenticate Product state, or claim
//! that a deployed release executes the action.

use clutch_solana_layout::direct_market_v1::{
    DirectAdmitOrderPayloadV1, DirectFreezeBookPayloadV1, DirectInitializeMarketPayloadV1,
    DirectSubmitCandidatePayloadV1, DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1,
    DIRECT_FREEZE_BOOK_PAYLOAD_BYTES_V1, DIRECT_INITIALIZE_MARKET_PAYLOAD_BYTES_V1,
    DIRECT_SUBMIT_CANDIDATE_PAYLOAD_BYTES_V1,
};
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_MARKET_FAMILY_TAG, DIRECT_MARKET_FAMILY_VERSION,
};

/// Largest current Direct family payload.
pub const DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1: usize =
    DIRECT_FREEZE_BOOK_PAYLOAD_BYTES_V1;

/// One exact family-local action and its canonical fixed-capacity payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectMarketClientPayloadV1 {
    action: DirectMarketAction,
    bytes: [u8; DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1],
    len: u8,
}

impl DirectMarketClientPayloadV1 {
    /// Construct action 1 from the authoritative layout payload.
    #[must_use]
    pub fn initialize_market(value: DirectInitializeMarketPayloadV1) -> Self {
        let mut bytes = [0; DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1];
        let mut encoded = [0; DIRECT_INITIALIZE_MARKET_PAYLOAD_BYTES_V1];
        value.encode_into(&mut encoded);
        bytes[..DIRECT_INITIALIZE_MARKET_PAYLOAD_BYTES_V1].copy_from_slice(&encoded);
        Self {
            action: DirectMarketAction::InitializeMarket,
            bytes,
            len: 40,
        }
    }

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

    /// Construct action 4 from all sixteen exact simplex coordinates.
    pub fn freeze_book(value: DirectFreezeBookPayloadV1) -> Result<Self, DirectClientRefusalV1> {
        let mut bytes = [0; DIRECT_MARKET_CLIENT_MAX_PAYLOAD_BYTES_V1];
        value
            .encode_into(&mut bytes)
            .map_err(|_| DirectClientRefusalV1::NonCanonicalPayload)?;
        Ok(Self {
            action: DirectMarketAction::FreezeBook,
            bytes,
            len: 128,
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
            DirectMarketAction::CancelOrder
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
            DirectMarketAction::InitializeMarket
            | DirectMarketAction::AdmitOrder
            | DirectMarketAction::FreezeBook
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

/// Client-side canonical-construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectClientRefusalV1 {
    /// A nonempty action was requested through the empty-payload constructor.
    ActionPayloadMismatch,
    /// The authoritative layout encoder refused the payload.
    NonCanonicalPayload,
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
            DirectMarketClientPayloadV1::empty(DirectMarketAction::FreezeBook),
            Err(DirectClientRefusalV1::ActionPayloadMismatch)
        );
        for action in [
            DirectMarketAction::CancelOrder,
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
}
