//! The ticket on the wire: the type, the only writer, and the only reader.

use dclutch_direct_codec::intent_v2::CompactIntentV2;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solana_program::pubkey::Pubkey;
use solana_signature::Signature;

use crate::{Error, Result, refusal, strict_json::parse_json_without_duplicate_keys_v1};

/// The `kind` every portable ticket declares, and the only one either reader
/// accepts.
pub const PORTABLE_DIRECT_TICKET_KIND_V1: &str = "dclutch/direct-intent-ticket/v1";

/// The same explicit bound `decodeDirectIntentTicketV1` and the producer hold.
pub const MAXIMUM_TICKET_BYTES_V1: usize = 4_096;

/// One exact detached maker signature and its canonical signed intent.
///
/// THIS IS THE ONE DEFINITION. `dclutch_operator::direct_inline_v3` re-exports
/// it rather than declaring a second one, so the struct the ticket author signs
/// into and the struct the inline-execution builder consumes cannot drift: they
/// are the same struct. It lives here, at the bottom, because this crate is the
/// one both the signer and the instruction builder can afford to depend on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDirectIntentV3 {
    /// Native Ed25519 maker public key.
    pub maker: Pubkey,
    /// Detached Ed25519 signature over `intent.signed_preimage()`.
    pub signature: [u8; 64],
    /// Exact runtime-width Direct V2 semantic intent.
    pub intent: CompactIntentV2,
}

/// The intent half of the portable ticket.
///
/// Field ORDER is the wire: `serde_json::to_string_pretty` emits declaration
/// order and `JSON.stringify(value, null, 2)` emits insertion order, so this
/// declaration and the object literal in `packages/dclutch-sdk/lib/directTicket.ts`
/// have to agree line for line. The vector test proves they do.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PortableDirectTicketIntentV1 {
    /// 0 sell, 1 buy.
    pub side: u8,
    /// 0 fill-or-kill, 1 immediate-or-cancel.
    pub lifecycle: u8,
    /// Runtime outcome coordinate.
    pub outcome: u32,
    /// Base58 Market address.
    pub market: String,
    /// Canonical decimal Market generation.
    pub generation: String,
    /// Canonical decimal maker nonce.
    pub nonce: String,
    /// Canonical decimal first slot the intent is valid in.
    pub valid_from: String,
    /// Canonical decimal last slot the intent is valid in.
    pub valid_through: String,
    /// Canonical decimal maximum fill, in collateral atoms.
    pub maximum_fill: String,
    /// Canonical decimal scaled limit price.
    pub limit_price: String,
    /// Fee, in basis points, bounded by 10000.
    pub fee_basis_points: u16,
    /// Base58 address of the maker's collateral account.
    pub collateral_account: String,
}

/// The portable ticket: the maker, their detached signature, and every field
/// that signature covers. Nothing else -- a ticket carries no claim the
/// signature does not already bind.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PortableDirectTicketV1 {
    /// Always [`PORTABLE_DIRECT_TICKET_KIND_V1`].
    pub kind: String,
    /// Base58 maker address.
    pub maker: String,
    /// Lowercase hex of the 64-byte detached signature.
    pub signature: String,
    /// Every field the signature covers.
    pub intent: PortableDirectTicketIntentV1,
}

/// Emit the exact bytes `encodeDirectIntentTicketV1` emits for the same signed
/// intent -- two-space pretty JSON, no trailing newline, declaration order.
///
/// The trailing newline is deliberately absent. `JSON.stringify(value, null, 2)`
/// does not emit one, and a ticket that differs from the browser's by a single
/// byte is not the thing this crate claims to produce.
pub fn encode_portable_direct_ticket_v1(signed: &SignedDirectIntentV3) -> Result<String> {
    if signed.signature.iter().all(|byte| *byte == 0) {
        return Err(refusal(
            "a ticket requires one nonzero 64-byte detached signature",
        ));
    }
    let ticket = PortableDirectTicketV1 {
        kind: PORTABLE_DIRECT_TICKET_KIND_V1.into(),
        maker: signed.maker.to_string(),
        signature: hex_lower_v1(&signed.signature),
        intent: PortableDirectTicketIntentV1 {
            side: signed.intent.side,
            lifecycle: signed.intent.lifecycle,
            outcome: signed.intent.outcome,
            market: Pubkey::new_from_array(signed.intent.market).to_string(),
            generation: signed.intent.generation.to_string(),
            nonce: signed.intent.nonce.to_string(),
            valid_from: signed.intent.valid_from.to_string(),
            valid_through: signed.intent.valid_through.to_string(),
            maximum_fill: signed.intent.maximum_fill.to_string(),
            limit_price: signed.intent.limit_price.to_string(),
            fee_basis_points: signed.intent.fee_basis_points,
            collateral_account: Pubkey::new_from_array(signed.intent.collateral_account)
                .to_string(),
        },
    };
    let text = serde_json::to_string_pretty(&ticket)?;
    if text.len() > MAXIMUM_TICKET_BYTES_V1 {
        return Err(refusal(
            "authored ticket exceeded the explicit 4096-byte portable bound",
        ));
    }
    Ok(text)
}

/// Hostile-read one portable ticket back into the signed intent it carries.
///
/// The signature is verified here against the preimage this reader rebuilds, so
/// a tampered field is a refusal at parse time rather than a refusal at the
/// Ed25519 program much later.
pub fn parse_portable_direct_ticket_v1(bytes: &[u8], label: &str) -> Result<SignedDirectIntentV3> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_TICKET_BYTES_V1 {
        return Err(refusal(format!(
            "{label} Direct ticket is outside the 1..4096 byte bound"
        )));
    }
    let value = parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{label} Direct ticket {error}")))?;
    let ticket: PortableDirectTicketV1 = serde_json::from_value(value.clone())
        .map_err(|error| Error::new(format!("{label} Direct ticket shape: {error}")))?;
    if serde_json::to_value(&ticket)? != value || ticket.kind != PORTABLE_DIRECT_TICKET_KIND_V1 {
        return Err(refusal(format!(
            "{label} Direct ticket kind, field set, or canonical JSON values changed"
        )));
    }
    let maker = canonical_ticket_pubkey_v1(&ticket.maker, &format!("{label} maker"))?;
    let market = canonical_ticket_pubkey_v1(&ticket.intent.market, &format!("{label} Market"))?;
    let collateral = canonical_ticket_pubkey_v1(
        &ticket.intent.collateral_account,
        &format!("{label} collateral account"),
    )?;
    if maker == Pubkey::default()
        || market == Pubkey::default()
        || collateral == Pubkey::default()
        || ticket.intent.side > 1
        || ticket.intent.lifecycle > 1
        || ticket.intent.fee_basis_points > 10_000
    {
        return Err(refusal(format!(
            "{label} Direct ticket has an invalid identity, enum, or fee width"
        )));
    }
    let signature_bytes = decode_hex_v1(&ticket.signature, &format!("{label} signature"))?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| refusal(format!("{label} signature is not exactly 64 bytes")))?;
    if signature.iter().all(|byte| *byte == 0) {
        return Err(refusal(format!("{label} signature is all zero")));
    }
    let intent = CompactIntentV2 {
        side: ticket.intent.side,
        lifecycle: ticket.intent.lifecycle,
        outcome: ticket.intent.outcome,
        market: market.to_bytes(),
        generation: canonical_ticket_u64_v1(&ticket.intent.generation, "generation")?,
        nonce: canonical_ticket_u64_v1(&ticket.intent.nonce, "nonce")?,
        valid_from: canonical_ticket_u64_v1(&ticket.intent.valid_from, "validFrom")?,
        valid_through: canonical_ticket_u64_v1(&ticket.intent.valid_through, "validThrough")?,
        maximum_fill: canonical_ticket_u64_v1(&ticket.intent.maximum_fill, "maximumFill")?,
        limit_price: canonical_ticket_u64_v1(&ticket.intent.limit_price, "limitPrice")?,
        fee_basis_points: ticket.intent.fee_basis_points,
        collateral_account: collateral.to_bytes(),
    };
    let encoded = intent
        .encode()
        .map_err(|error| Error::new(format!("{label} Direct intent encode: {error:?}")))?;
    if CompactIntentV2::decode(&encoded)
        .map_err(|error| Error::new(format!("{label} Direct intent decode: {error:?}")))?
        != intent
    {
        return Err(refusal(format!(
            "{label} Direct ticket intent failed canonical codec roundtrip"
        )));
    }
    let preimage = intent
        .signed_preimage()
        .map_err(|error| Error::new(format!("{label} Direct signed preimage: {error:?}")))?;
    if !Signature::from(signature).verify(maker.as_ref(), &preimage) {
        return Err(refusal(format!(
            "{label} Direct ticket detached signature did not verify"
        )));
    }
    Ok(SignedDirectIntentV3 {
        maker,
        signature,
        intent,
    })
}

/// Parse one base58 address and refuse anything that is not its canonical text.
pub fn canonical_ticket_pubkey_v1(value: &str, label: &str) -> Result<Pubkey> {
    let key: Pubkey = value
        .parse()
        .map_err(|error| Error::new(format!("invalid pubkey {value}: {error}")))?;
    if key.to_string() != value {
        return Err(refusal(format!("{label} is not canonical base58 text")));
    }
    Ok(key)
}

/// Parse one `u64` and refuse anything that is not its canonical decimal text.
///
/// `007` and `+7` parse as seven in most readers, and a ticket whose bytes
/// depend on which reader looked at it is not a ticket.
pub fn canonical_ticket_u64_v1(value: &str, label: &str) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| refusal(format!("Direct ticket {label} is not a u64")))?;
    if parsed.to_string() != value {
        return Err(refusal(format!(
            "Direct ticket {label} is not canonical decimal text"
        )));
    }
    Ok(parsed)
}

/// Decode canonical lowercase even-width hex.
pub fn decode_hex_v1(value: &str, label: &str) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(refusal(format!(
            "{label} is not canonical lowercase even-width hex"
        )));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let (Some(high), Some(low)) = (pair.first(), pair.get(1)) else {
            return Err(refusal(format!("{label} hex")));
        };
        let (Some(high), Some(low)) = (hex_nibble_v1(*high), hex_nibble_v1(*low)) else {
            return Err(refusal(format!(
                "{label} is not canonical lowercase even-width hex"
            )));
        };
        bytes.push(high << 4 | low);
    }
    Ok(bytes)
}

fn hex_nibble_v1(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

/// Render bytes as canonical lowercase hex.
#[must_use]
pub fn hex_lower_v1(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// The SHA-256 of `bytes`, as canonical lowercase hex.
#[must_use]
pub fn sha256_hex_v1(bytes: &[u8]) -> String {
    hex_lower_v1(&Sha256::digest(bytes))
}
