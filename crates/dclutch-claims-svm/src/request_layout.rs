//! Canonical patchable coordinates of the Claims plan wire.
//!
//! Data-defined EffectProgram emitters use this narrow view to patch exact
//! semantic fields while [`crate::ClaimsPlanV1`] remains the sole encoder and
//! hostile decoder for the complete request.

use crate::{
    CLAIM_QUANTITY_BYTES, CLAIMS_PLAN_HEADER_BYTES_V1, DESTINATION_OWNER_OFFSET,
    EXPECTED_DESTINATION_REVISION_OFFSET, EXPECTED_MARKET_REVISION_OFFSET,
    EXPECTED_SOURCE_REVISION_OFFSET, MARKET_OFFSET, OUTCOME_COUNT_OFFSET, RELEASE_SET_OFFSET,
    REQUEST_OFFSET, SOURCE_OWNER_OFFSET,
};

/// Canonical byte coordinates of patchable [`crate::ClaimsPlanV1`] fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsPlanLayoutV1;

impl ClaimsPlanLayoutV1 {
    /// Selected execution release set identity.
    pub const RELEASE_SET: usize = RELEASE_SET_OFFSET;
    /// Logical Core Market identity.
    pub const MARKET: usize = MARKET_OFFSET;
    /// Complete parent request digest.
    pub const REQUEST_ID: usize = REQUEST_OFFSET;
    /// Source Position owner.
    pub const SOURCE_OWNER: usize = SOURCE_OWNER_OFFSET;
    /// Destination Position owner.
    pub const DESTINATION_OWNER: usize = DESTINATION_OWNER_OFFSET;
    /// Claims aggregate optimistic pre-revision as little-endian `u64`.
    pub const EXPECTED_MARKET_REVISION: usize = EXPECTED_MARKET_REVISION_OFFSET;
    /// Source Position optimistic pre-revision as little-endian `u64`.
    pub const EXPECTED_SOURCE_REVISION: usize = EXPECTED_SOURCE_REVISION_OFFSET;
    /// Destination Position optimistic pre-revision as little-endian `u64`.
    pub const EXPECTED_DESTINATION_REVISION: usize = EXPECTED_DESTINATION_REVISION_OFFSET;
    /// Product-owned runtime outcome count as little-endian `u32`.
    pub const OUTCOME_COUNT: usize = OUTCOME_COUNT_OFFSET;
    /// Start of the exact runtime-width `u64` quantity tail.
    pub const QUANTITIES: usize = CLAIMS_PLAN_HEADER_BYTES_V1;
    /// Width of one quantity tail item.
    pub const QUANTITY_BYTES: usize = CLAIM_QUANTITY_BYTES;
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;
    use crate::{CallerRole, ClaimsAction, ClaimsPlanV1, Error, PLAN_RESERVED_OFFSET};

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn public_layout_tracks_the_semantic_encoder_and_hostile_decoder() {
        let quantities = [3_u64, 5, 8]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<vec::Vec<_>>();
        let plan = ClaimsPlanV1::new(
            ClaimsAction::TransferNative,
            CallerRole::Trading,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            6,
            7,
            8,
            3,
            &quantities,
        )
        .expect("plan");
        let mut bytes = vec![0_u8; CLAIMS_PLAN_HEADER_BYTES_V1 + quantities.len()];
        plan.encode_into(&mut bytes).expect("encode");

        for (offset, expected) in [
            (ClaimsPlanLayoutV1::RELEASE_SET, id(1)),
            (ClaimsPlanLayoutV1::MARKET, id(2)),
            (ClaimsPlanLayoutV1::REQUEST_ID, id(3)),
            (ClaimsPlanLayoutV1::SOURCE_OWNER, id(4)),
            (ClaimsPlanLayoutV1::DESTINATION_OWNER, id(5)),
        ] {
            assert_eq!(bytes.get(offset..offset + 32), Some(expected.as_slice()));
        }
        for (offset, expected) in [
            (ClaimsPlanLayoutV1::EXPECTED_MARKET_REVISION, 6_u64),
            (ClaimsPlanLayoutV1::EXPECTED_SOURCE_REVISION, 7_u64),
            (ClaimsPlanLayoutV1::EXPECTED_DESTINATION_REVISION, 8_u64),
        ] {
            assert_eq!(
                bytes.get(offset..offset + 8),
                Some(expected.to_le_bytes().as_slice())
            );
        }
        assert_eq!(
            bytes.get(ClaimsPlanLayoutV1::OUTCOME_COUNT..ClaimsPlanLayoutV1::OUTCOME_COUNT + 4),
            Some(3_u32.to_le_bytes().as_slice())
        );
        assert_eq!(
            bytes.get(ClaimsPlanLayoutV1::QUANTITIES..),
            Some(quantities.as_slice())
        );
        assert_eq!(ClaimsPlanV1::decode(&bytes), Ok(plan));

        let mut hostile = bytes;
        *hostile
            .get_mut(PLAN_RESERVED_OFFSET)
            .expect("reserved byte") = 1;
        assert_eq!(
            ClaimsPlanV1::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
    }

    #[test]
    fn encoder_width_refusal_preserves_output() {
        let quantities = 9_u64.to_le_bytes();
        let plan = ClaimsPlanV1::new(
            ClaimsAction::TransferNative,
            CallerRole::Trading,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            6,
            7,
            8,
            1,
            &quantities,
        )
        .expect("plan");
        let mut output = [0x55_u8; CLAIMS_PLAN_HEADER_BYTES_V1];
        let before = output;
        assert_eq!(plan.encode_into(&mut output), Err(Error::InvalidLength));
        assert_eq!(output, before);
    }
}
