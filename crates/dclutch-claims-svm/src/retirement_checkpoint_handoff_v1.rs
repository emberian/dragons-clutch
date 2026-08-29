//! Zero-liability Claims aggregate handoff to Core retirement.
//!
//! This is deliberately distinct from `market_closure_v1`: the aggregate's
//! lamports are retained in the same PDA and keep their Claims-refund
//! classification while ownership moves to Core for durable suffix recovery.

use crate::market_closure_v1::{
    CLAIMS_MARKET_CLOSURE_ACTION_V1, CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1,
    CLAIMS_MARKET_CLOSURE_RECEIPT_MAGIC_V1, CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1,
    CLAIMS_MARKET_CLOSURE_REQUEST_MAGIC_V1, ClaimsMarketClosureErrorV1,
    ClaimsMarketClosureReceiptInputV1, ClaimsMarketClosureReceiptV1,
    ClaimsMarketClosureRequestInputV1, ClaimsMarketClosureRequestV1,
};

/// Exact handoff request width.
pub const CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_BYTES_V1: usize =
    CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1;
/// Exact handoff receipt width.
pub const CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_RECEIPT_BYTES_V1: usize =
    CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1;
/// Distinct handoff request magic.
pub const CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTCRQ1";
/// Distinct handoff receipt magic.
pub const CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTCRC1";
/// Domain-separated post-handoff resource digest.
pub const CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_POST_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch/claims-retirement-checkpoint-handoff-post/v1";

const HANDOFF_ACTION_V1: u8 = 2;
const HANDOFF_RECEIPT_KIND_V1: u8 = 2;

/// One exact retirement-checkpoint handoff request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsRetirementCheckpointHandoffRequestV1(ClaimsMarketClosureRequestV1);

impl ClaimsRetirementCheckpointHandoffRequestV1 {
    /// Construct and validate the handoff request.
    pub fn new(
        input: ClaimsMarketClosureRequestInputV1,
    ) -> Result<Self, ClaimsMarketClosureErrorV1> {
        ClaimsMarketClosureRequestV1::new(input).map(Self)
    }

    /// Hostile-decode exact handoff bytes.
    pub fn decode(input: &[u8]) -> Result<Self, ClaimsMarketClosureErrorV1> {
        if input.len() != CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_BYTES_V1
            || input.get(..8)
                != Some(CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_MAGIC_V1.as_slice())
            || input.get(10).copied() != Some(HANDOFF_ACTION_V1)
        {
            return Err(ClaimsMarketClosureErrorV1::InvalidHeader);
        }
        let mut normalized = [0_u8; CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_BYTES_V1];
        normalized.copy_from_slice(input);
        normalized[..8].copy_from_slice(&CLAIMS_MARKET_CLOSURE_REQUEST_MAGIC_V1);
        normalized[10] = CLAIMS_MARKET_CLOSURE_ACTION_V1;
        ClaimsMarketClosureRequestV1::decode(&normalized).map(Self)
    }

    /// Encode exact canonical request bytes.
    pub fn to_bytes(self) -> [u8; CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_BYTES_V1] {
        let mut output = self.0.to_bytes();
        output[..8].copy_from_slice(&CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_REQUEST_MAGIC_V1);
        output[10] = HANDOFF_ACTION_V1;
        output
    }

    /// Borrow all validated request coordinates.
    pub const fn input(self) -> ClaimsMarketClosureRequestInputV1 {
        self.0.input()
    }
}

/// Immediate proof that Claims handed the exact empty aggregate to Core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsRetirementCheckpointHandoffReceiptV1(ClaimsMarketClosureReceiptV1);

impl ClaimsRetirementCheckpointHandoffReceiptV1 {
    /// Construct and validate handoff evidence.
    pub fn new(
        input: ClaimsMarketClosureReceiptInputV1,
    ) -> Result<Self, ClaimsMarketClosureErrorV1> {
        ClaimsMarketClosureReceiptV1::new(input).map(Self)
    }

    /// Hostile-decode exact handoff evidence.
    pub fn decode(input: &[u8]) -> Result<Self, ClaimsMarketClosureErrorV1> {
        if input.len() != CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_RECEIPT_BYTES_V1
            || input.get(..8)
                != Some(CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_RECEIPT_MAGIC_V1.as_slice())
            || input.get(10).copied() != Some(HANDOFF_RECEIPT_KIND_V1)
        {
            return Err(ClaimsMarketClosureErrorV1::InvalidHeader);
        }
        let mut normalized = [0_u8; CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_RECEIPT_BYTES_V1];
        normalized.copy_from_slice(input);
        normalized[..8].copy_from_slice(&CLAIMS_MARKET_CLOSURE_RECEIPT_MAGIC_V1);
        normalized[10] = 1;
        ClaimsMarketClosureReceiptV1::decode(&normalized).map(Self)
    }

    /// Encode exact canonical handoff evidence.
    pub fn to_bytes(self) -> [u8; CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_RECEIPT_BYTES_V1] {
        let mut output = self.0.to_bytes();
        output[..8].copy_from_slice(&CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_RECEIPT_MAGIC_V1);
        output[10] = HANDOFF_RECEIPT_KIND_V1;
        output
    }

    /// Borrow all validated receipt coordinates.
    pub const fn input(self) -> ClaimsMarketClosureReceiptInputV1 {
        self.0.input()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ClaimsRetirementCheckpointHandoffRequestV1 {
        ClaimsRetirementCheckpointHandoffRequestV1::new(ClaimsMarketClosureRequestInputV1 {
            release_set: [1; 32],
            market: [2; 32],
            aggregate: [3; 32],
            rent_credit: [4; 32],
            parent_request_digest: [5; 32],
            core_program: [6; 32],
            generation: 7,
            expected_revision: 9,
            resulting_revision: 10,
            claim_count: 5,
        })
        .expect("handoff request")
    }

    #[test]
    fn handoff_request_is_distinct_and_round_trips() {
        let request = request();
        let bytes = request.to_bytes();
        assert_eq!(
            ClaimsRetirementCheckpointHandoffRequestV1::decode(&bytes),
            Ok(request)
        );
        assert!(ClaimsMarketClosureRequestV1::decode(&bytes).is_err());
    }

    #[test]
    fn handoff_receipt_is_distinct_and_round_trips() {
        let receipt =
            ClaimsRetirementCheckpointHandoffReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
                producer: [7; 32],
                release_set: [1; 32],
                market: [2; 32],
                aggregate: [3; 32],
                rent_credit: [4; 32],
                request_digest: [8; 32],
                pre_resource_digest: [9; 32],
                post_resource_digest: [10; 32],
                generation: 7,
                pre_revision: 9,
                post_revision: 10,
                liability_units: 0,
                refund_lamports: 91,
                claim_count: 5,
            })
            .expect("handoff receipt");
        let bytes = receipt.to_bytes();
        assert_eq!(
            ClaimsRetirementCheckpointHandoffReceiptV1::decode(&bytes),
            Ok(receipt)
        );
        assert!(ClaimsMarketClosureReceiptV1::decode(&bytes).is_err());
    }
}
