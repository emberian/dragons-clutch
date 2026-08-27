//! Parsing and rendering of the 32-byte identifiers this service handles.
//!
//! Addresses, genesis hashes, family identities and content identities are all
//! 32 bytes.  Config accepts either base58 (the form an operator copies out of
//! an explorer) or 64 hex characters (the form a Lean-emitted constant is
//! printed in), and every artifact renders both so a reader never has to guess
//! which encoding a field is in.

use crate::error::{RelayerError, Result};

/// Exact width of every identifier in this family.
pub const ID_BYTES: usize = 32;

/// Parse a 32-byte identifier from base58 or 64 hex characters.
///
/// The two encodings are distinguished structurally rather than by a prefix: a
/// 64-character all-hex string is hex, anything else is tried as base58.  A
/// 64-character base58 string decoding to 32 bytes is not representable in
/// base58 (32 bytes encode to 43 or 44 characters), so the discrimination is
/// unambiguous.
pub fn parse_id32(field: &str, value: &str) -> Result<[u8; ID_BYTES]> {
    let trimmed = value.trim();
    let refuse = || RelayerError::Identifier {
        field: field.to_owned(),
        value: value.to_owned(),
    };

    if trimmed.len() == ID_BYTES * 2 && trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut out = [0u8; ID_BYTES];
        hex::decode_to_slice(trimmed, &mut out).map_err(|_| refuse())?;
        return Ok(out);
    }

    let mut out = [0u8; ID_BYTES];
    let written = bs58::decode(trimmed).onto(&mut out).map_err(|_| refuse())?;
    if written != ID_BYTES {
        return Err(refuse());
    }
    Ok(out)
}

/// Render an identifier as base58.
pub fn base58(bytes: &[u8; ID_BYTES]) -> String {
    bs58::encode(bytes).into_string()
}

/// Render arbitrary bytes as lowercase hex.
pub fn to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Whether an identifier is entirely zero.
///
/// The wire codec refuses zero identities; catching them at config load turns a
/// mid-cycle refusal into a startup refusal.
pub fn is_zero(bytes: &[u8; ID_BYTES]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_base58_spellings_of_one_identity_agree() {
        let bytes = [0x11u8; ID_BYTES];
        let as_hex = to_hex(&bytes);
        let as_base58 = base58(&bytes);
        assert_eq!(parse_id32("f", &as_hex).unwrap(), bytes);
        assert_eq!(parse_id32("f", &as_base58).unwrap(), bytes);
    }

    #[test]
    fn the_pinned_mainnet_genesis_hash_round_trips_through_its_base58_spelling() {
        // The base58 spelling is the one an operator copies from an explorer;
        // the byte array is the one the Lean-emitted ABI pins.  If these ever
        // disagree the daemon would refuse to run against real mainnet, so the
        // agreement is asserted rather than assumed.
        let pinned = dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1;
        assert_eq!(
            base58(&pinned),
            "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
        );
        assert_eq!(
            parse_id32("genesis", "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d").unwrap(),
            pinned
        );
    }

    #[test]
    fn the_pinned_devnet_genesis_hash_round_trips_too() {
        assert_eq!(
            base58(&dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1),
            "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
        );
    }

    #[test]
    fn a_short_identifier_refuses_instead_of_padding() {
        assert!(parse_id32("f", "11").is_err());
        assert!(parse_id32("f", "").is_err());
        assert!(parse_id32("f", &"ab".repeat(31)).is_err());
    }

    #[test]
    fn a_non_identifier_refuses() {
        assert!(parse_id32("f", "not an identifier").is_err());
        assert!(parse_id32("f", &"z".repeat(64)).is_err());
    }
}
