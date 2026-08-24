// SPDX-License-Identifier: AGPL-3.0-or-later
//! Borrowed physical frame for the current Direct `0xb1/3` root.
//!
//! The semantic body is interpreted only by `clutch-direct-market-runtime`.
//! Borrowing avoids a second 2.5KiB account copy in the SBF adapter.

use crate::{registry, CodecError, Result};

/// Exact current root semantic-body width.
pub const DIRECT_MARKET_ROOT_BODY_BYTES_V3: usize =
    registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3 - 4;

/// Borrowed hostile-decoded current Direct root frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectMarketRootAccountV3<'a> {
    bump: u8,
    semantic_body: &'a [u8],
}

impl<'a> DirectMarketRootAccountV3<'a> {
    /// Hostile-decode exact tag/version/reserved byte without copying the body.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() != registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3 {
            return Err(CodecError::WrongLength);
        }
        if input[0] != registry::DIRECT_MARKET_ROOT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::DIRECT_MARKET_ROOT_ACCOUNT_VERSION_V3 {
            return Err(CodecError::WrongVersion);
        }
        if input[3] != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        let semantic_body = &input[4..];
        if semantic_body.iter().all(|byte| *byte == 0) {
            return Err(CodecError::ZeroValue);
        }
        Ok(Self { bump: input[2], semantic_body })
    }

    /// Canonical PDA bump.
    pub const fn bump(self) -> u8 { self.bump }
    /// Exact borrowed V3 semantic body.
    pub const fn semantic_body(self) -> &'a [u8] { self.semantic_body }
}

/// Encode an already-semantic-validated V3 body into the physical frame.
pub fn encode_direct_market_root_account_v3(
    bump: u8,
    semantic_body: &[u8],
    output: &mut [u8],
) -> Result<()> {
    if semantic_body.len() != DIRECT_MARKET_ROOT_BODY_BYTES_V3
        || output.len() != registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3
    {
        return Err(CodecError::WrongLength);
    }
    if semantic_body.iter().all(|byte| *byte == 0) {
        return Err(CodecError::ZeroValue);
    }
    output[0] = registry::DIRECT_MARKET_ROOT_ACCOUNT_TAG;
    output[1] = registry::DIRECT_MARKET_ROOT_ACCOUNT_VERSION_V3;
    output[2] = bump;
    output[3] = 0;
    output[4..].copy_from_slice(semantic_body);
    Ok(())
}

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V3 == 2_530);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_frame_refuses_v1_and_noncanonical_reserved_byte() {
        let body = [7u8; DIRECT_MARKET_ROOT_BODY_BYTES_V3];
        let mut account = [0u8; registry::DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3];
        encode_direct_market_root_account_v3(9, &body, &mut account).unwrap();
        let decoded = DirectMarketRootAccountV3::decode(&account).unwrap();
        assert_eq!(decoded.bump(), 9);
        assert_eq!(decoded.semantic_body(), body);
        account[1] = registry::DIRECT_MARKET_ROOT_ACCOUNT_VERSION;
        assert_eq!(DirectMarketRootAccountV3::decode(&account), Err(CodecError::WrongVersion));
        account[1] = registry::DIRECT_MARKET_ROOT_ACCOUNT_VERSION_V3;
        account[3] = 1;
        assert_eq!(
            DirectMarketRootAccountV3::decode(&account),
            Err(CodecError::NonCanonicalPadding),
        );
    }
}
