// SPDX-License-Identifier: AGPL-3.0-or-later
//! Physical frames for reusable Failure Market interval accounts.
//!
//! This adapter crate owns only the four-byte Solana account frame. The
//! complete `0xab/v2` cell body and `0xac/v2` history body remain byte-for-byte
//! owned by `clutch-failure-policy-runtime`; this module deliberately does not
//! decode or duplicate their semantic fields.

use crate::{registry, CodecError, Result};

/// Exact semantic-owner body inside the 1,088-byte reusable cell account.
pub const FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2: usize =
    registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES - 4;
/// Exact semantic-owner body inside the 512-byte append-only history account.
pub const FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2: usize =
    registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES - 4;

/// Physical `0xab/v2` frame around the complete reusable-cell semantic body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellAccountV2 {
    bump: u8,
    semantic_body: [u8; FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2],
}

impl FailureMarketIntervalCellAccountV2 {
    /// Frame one semantic body after the semantic owner hostile-decodes it.
    pub fn new(
        bump: u8,
        semantic_body: [u8; FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2],
    ) -> Result<Self> {
        require_initialized(&semantic_body)?;
        Ok(Self {
            bump,
            semantic_body,
        })
    }

    /// Canonical PDA bump.
    pub const fn bump(&self) -> u8 {
        self.bump
    }

    /// Exact opaque semantic-owner body.
    pub const fn semantic_body(&self) -> &[u8; FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2] {
        &self.semantic_body
    }

    /// Encode the exact frame without reinterpreting semantic bytes.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES],
    ) -> Result<()> {
        require_initialized(&self.semantic_body)?;
        output[0] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG;
        output[1] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION;
        output[2] = self.bump;
        output[3] = 0;
        output[4..].copy_from_slice(&self.semantic_body);
        Ok(())
    }

    /// Hostile-decode the physical frame. The caller must next invoke the
    /// Failure semantic owner's exact body decoder.
    pub fn decode(
        input: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES],
    ) -> Result<Self> {
        authenticate_frame(
            input[0],
            input[1],
            input[3],
            registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_TAG,
            registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_VERSION,
        )?;
        let mut semantic_body = [0; FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2];
        semantic_body.copy_from_slice(&input[4..]);
        Self::new(input[2], semantic_body)
    }
}

/// Physical `0xac/v2` frame around the complete append-only history body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalHistoryAccountV2 {
    bump: u8,
    semantic_body: [u8; FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2],
}

impl FailureMarketIntervalHistoryAccountV2 {
    /// Frame one semantic body after the semantic owner hostile-decodes it.
    pub fn new(
        bump: u8,
        semantic_body: [u8; FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2],
    ) -> Result<Self> {
        require_initialized(&semantic_body)?;
        Ok(Self {
            bump,
            semantic_body,
        })
    }

    /// Canonical PDA bump.
    pub const fn bump(&self) -> u8 {
        self.bump
    }

    /// Exact opaque semantic-owner body.
    pub const fn semantic_body(&self) -> &[u8; FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2] {
        &self.semantic_body
    }

    /// Encode the exact frame without reinterpreting semantic bytes.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES],
    ) -> Result<()> {
        require_initialized(&self.semantic_body)?;
        output[0] = registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG;
        output[1] = registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION;
        output[2] = self.bump;
        output[3] = 0;
        output[4..].copy_from_slice(&self.semantic_body);
        Ok(())
    }

    /// Hostile-decode the physical frame. The caller must next invoke the
    /// Failure semantic owner's exact body decoder.
    pub fn decode(
        input: &[u8; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES],
    ) -> Result<Self> {
        authenticate_frame(
            input[0],
            input[1],
            input[3],
            registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_TAG,
            registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_VERSION,
        )?;
        let mut semantic_body = [0; FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2];
        semantic_body.copy_from_slice(&input[4..]);
        Self::new(input[2], semantic_body)
    }
}

fn authenticate_frame(
    actual_tag: u8,
    actual_version: u8,
    reserved: u8,
    expected_tag: u8,
    expected_version: u8,
) -> Result<()> {
    if actual_tag != expected_tag {
        return Err(CodecError::WrongTag);
    }
    if actual_version != expected_version {
        return Err(CodecError::WrongVersion);
    }
    if reserved != 0 {
        return Err(CodecError::NonCanonicalPadding);
    }
    Ok(())
}

fn require_initialized<const N: usize>(semantic_body: &[u8; N]) -> Result<()> {
    if semantic_body.iter().all(|byte| *byte == 0) {
        Err(CodecError::ZeroValue)
    } else {
        Ok(())
    }
}

const _: () = assert!(FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2 == 1_084);
const _: () = assert!(FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2 == 508);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_frame_refuses_v1_and_zero_semantic_bodies() {
        let mut semantic_body = [0; FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2];
        semantic_body[0] = 7;
        let value = FailureMarketIntervalCellAccountV2::new(9, semantic_body).unwrap();
        let mut encoded = [0; registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES];
        value.encode_into(&mut encoded).unwrap();
        assert_eq!(
            FailureMarketIntervalCellAccountV2::decode(&encoded),
            Ok(value)
        );

        encoded[1] = registry::FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_V1_VERSION;
        assert_eq!(
            FailureMarketIntervalCellAccountV2::decode(&encoded),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(
            FailureMarketIntervalCellAccountV2::new(
                9,
                [0; FAILURE_MARKET_INTERVAL_CELL_BODY_BYTES_V2]
            ),
            Err(CodecError::ZeroValue)
        );
    }

    #[test]
    fn history_frame_preserves_semantic_body_and_refuses_hidden_frame_data() {
        let mut semantic_body = [0; FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2];
        semantic_body[0] = 8;
        semantic_body[FAILURE_MARKET_INTERVAL_HISTORY_BODY_BYTES_V2 - 1] = 9;
        let value = FailureMarketIntervalHistoryAccountV2::new(10, semantic_body).unwrap();
        let mut encoded = [0; registry::FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES];
        value.encode_into(&mut encoded).unwrap();
        assert_eq!(
            FailureMarketIntervalHistoryAccountV2::decode(&encoded),
            Ok(value)
        );

        encoded[3] = 1;
        assert_eq!(
            FailureMarketIntervalHistoryAccountV2::decode(&encoded),
            Err(CodecError::NonCanonicalPadding)
        );
    }
}
