// SPDX-License-Identifier: AGPL-3.0-or-later
//! Physical frame for the permanent shared-Market Failure replay.
//!
//! This adapter owns only the four-byte Solana frame. The complete semantic
//! body remains byte-for-byte owned by `clutch-failure-policy-runtime`.

use crate::{registry, CodecError, Result};

/// Exact semantic body inside the 256-byte `0xa3/v2` account.
pub const FAILURE_MARKET_REPLAY_BODY_BYTES_V2: usize =
    registry::FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2 - 4;

/// Physical `0xa3/v2` frame around the permanent replay semantic body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketReplayAccountV2 {
    bump: u8,
    semantic_body: [u8; FAILURE_MARKET_REPLAY_BODY_BYTES_V2],
}

impl FailureMarketReplayAccountV2 {
    /// Frame one semantic body after the semantic owner hostile-decodes it.
    pub fn new(bump: u8, semantic_body: [u8; FAILURE_MARKET_REPLAY_BODY_BYTES_V2]) -> Result<Self> {
        if semantic_body.iter().all(|byte| *byte == 0) {
            return Err(CodecError::ZeroValue);
        }
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
    pub const fn semantic_body(&self) -> &[u8; FAILURE_MARKET_REPLAY_BODY_BYTES_V2] {
        &self.semantic_body
    }

    /// Encode the exact frame without reinterpreting semantic bytes.
    pub fn encode_into(
        &self,
        output: &mut [u8; registry::FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2],
    ) -> Result<()> {
        output[0] = registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG;
        output[1] = registry::FAILURE_MARKET_REPLAY_ACCOUNT_VERSION_V2;
        output[2] = self.bump;
        output[3] = 0;
        output[4..].copy_from_slice(&self.semantic_body);
        Ok(())
    }

    /// Hostile-decode the physical frame. The caller must next invoke the
    /// Failure semantic owner's exact body decoder.
    pub fn decode(input: &[u8; registry::FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2]) -> Result<Self> {
        if input[0] != registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::FAILURE_MARKET_REPLAY_ACCOUNT_VERSION_V2 {
            return Err(CodecError::WrongVersion);
        }
        if input[3] != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        let mut semantic_body = [0; FAILURE_MARKET_REPLAY_BODY_BYTES_V2];
        semantic_body.copy_from_slice(&input[4..]);
        Self::new(input[2], semantic_body)
    }
}

const _: () = assert!(FAILURE_MARKET_REPLAY_BODY_BYTES_V2 == 252);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_frame_refuses_v1_and_noncanonical_reserved_byte() {
        let mut body = [0; FAILURE_MARKET_REPLAY_BODY_BYTES_V2];
        body[0] = 9;
        let value = FailureMarketReplayAccountV2::new(7, body).unwrap();
        let mut encoded = [0; registry::FAILURE_MARKET_REPLAY_ACCOUNT_BYTES_V2];
        value.encode_into(&mut encoded).unwrap();
        assert_eq!(FailureMarketReplayAccountV2::decode(&encoded), Ok(value));

        encoded[1] = registry::FAILURE_REPLAY_TOMBSTONE_ACCOUNT_VERSION;
        assert_eq!(
            FailureMarketReplayAccountV2::decode(&encoded),
            Err(CodecError::WrongVersion)
        );
        encoded[1] = registry::FAILURE_MARKET_REPLAY_ACCOUNT_VERSION_V2;
        encoded[3] = 1;
        assert_eq!(
            FailureMarketReplayAccountV2::decode(&encoded),
            Err(CodecError::NonCanonicalPadding)
        );
    }
}
