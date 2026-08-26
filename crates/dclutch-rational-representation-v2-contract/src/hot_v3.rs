//! Ephemeral Hot V3 terminal intent and exact Claims-child specialization.

use crate::{
    Error, RepresentationActionV2, RepresentationReceiptV2, RepresentationRequestV2, Result,
    array_at,
    generated::{
        ACTION_REDEEM_TERMINAL, CALLER_ROLE_TRADING, PHYSICAL_ABI_VERSION_V2,
        RECEIPT_CLAIMS_PROGRAM_OFFSET, RECEIPT_REPRESENTATION_PROGRAM_OFFSET, REQUEST_MAGIC_V2,
    },
    generated_hot_v3::*,
    is_zero, put, put_byte, require_zero, u16_at,
};

/// Borrowed wallet-facing intent for exactly one terminal rational redemption.
///
/// The parent-context coordinate is zero in this family message. The Hot
/// adapter hashes the exact family bytes and writes that digest into the
/// canonical Rational V2 child. This avoids a self-digest fixed point while
/// preserving every economic field and the one exact asset row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalTerminalHotRequestV3<'a> {
    bytes: &'a [u8],
}

impl<'a> RationalTerminalHotRequestV3<'a> {
    /// Hostile-decode one exact terminal family request.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() != RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3)?
            != RATIONAL_TERMINAL_HOT_MAGIC_V3
        {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3)?
            != RATIONAL_TERMINAL_HOT_VERSION_V3
        {
            return Err(Error::UnsupportedVersion);
        }
        if input[RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3] != ACTION_REDEEM_TERMINAL
            || input[RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3] != CALLER_ROLE_TRADING
        {
            return Err(Error::InvalidActionShape);
        }
        require_zero(input, RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3, 32)?;

        // Reuse the sole semantic owner for terminal Rational V2 request
        // validation. A nonzero marker is supplied only to make the otherwise
        // identical child shape decodable; it is never returned or persisted.
        let mut child = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
        child.copy_from_slice(input);
        put(
            &mut child,
            RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3,
            &REQUEST_MAGIC_V2,
        )?;
        put(
            &mut child,
            RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3,
            &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
        )?;
        put(
            &mut child,
            RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3,
            &[1_u8; 32],
        )?;
        let request = RepresentationRequestV2::decode(&child)?;
        if request.header().action != RepresentationActionV2::RedeemTerminal
            || request.header().asset_count != RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3
        {
            return Err(Error::InvalidActionShape);
        }
        Ok(Self { bytes: input })
    }

    /// Project a canonical terminal child template into the wallet-facing
    /// family form. The child's old parent digest is intentionally discarded.
    pub fn from_child_into<'b>(
        child: RepresentationRequestV2<'_>,
        output: &'b mut [u8],
    ) -> Result<RationalTerminalHotRequestV3<'b>> {
        if child.header().action != RepresentationActionV2::RedeemTerminal
            || child.header().asset_count != RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3
            || output.len() != RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3
        {
            return Err(Error::InvalidActionShape);
        }
        child.encode_into(output)?;
        put(
            output,
            RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3,
            &RATIONAL_TERMINAL_HOT_MAGIC_V3,
        )?;
        put(
            output,
            RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3,
            &RATIONAL_TERMINAL_HOT_VERSION_V3.to_le_bytes(),
        )?;
        output[RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3
            ..RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3 + 32]
            .fill(0);
        RationalTerminalHotRequestV3::<'b>::decode(output)
    }

    /// Specialize this family request into the exact Rational V2 Claims child.
    ///
    /// `family_digest` must be the SHA-256 digest of [`Self::as_bytes`],
    /// computed by the authenticated Hot adapter. The returned request borrows
    /// the caller-owned output buffer.
    pub fn specialize_child_into<'b>(
        self,
        family_digest: [u8; 32],
        output: &'b mut [u8],
    ) -> Result<RepresentationRequestV2<'b>> {
        if is_zero(family_digest) {
            return Err(Error::ZeroIdentity);
        }
        if output.len() != RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        output.copy_from_slice(self.bytes);
        put(
            output,
            RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3,
            &REQUEST_MAGIC_V2,
        )?;
        put(
            output,
            RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3,
            &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
        )?;
        put(
            output,
            RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3,
            &family_digest,
        )?;
        // Keep the exact fixed action/role explicit even though decode already
        // admitted them in the family request.
        put_byte(
            output,
            RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3,
            ACTION_REDEEM_TERMINAL,
        )?;
        put_byte(
            output,
            RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3,
            CALLER_ROLE_TRADING,
        )?;
        RepresentationRequestV2::decode(output)
    }

    /// Exact bytes whose digest becomes the child parent context.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Authenticate one Rational terminal receipt against the exact child request
/// and current Claims producer selected by the execution release.
///
/// This does not accept a caller-authored receipt DTO: it hostile-decodes the
/// exact 592-byte Claims return value and independently checks both producer
/// coordinates before joining it to the exact child digest.
pub fn verify_rational_terminal_receipt_v3(
    child: RepresentationRequestV2<'_>,
    child_digest: [u8; 32],
    receipt_bytes: &[u8],
    expected_claims_program: [u8; 32],
) -> Result<RepresentationReceiptV2> {
    if is_zero(child_digest) || is_zero(expected_claims_program) {
        return Err(Error::ZeroIdentity);
    }
    if child.header().action != RepresentationActionV2::RedeemTerminal {
        return Err(Error::InvalidActionShape);
    }
    if array_at::<32>(receipt_bytes, RECEIPT_REPRESENTATION_PROGRAM_OFFSET)?
        != expected_claims_program
        || array_at::<32>(receipt_bytes, RECEIPT_CLAIMS_PROGRAM_OFFSET)? != expected_claims_program
    {
        return Err(Error::ReceiptMismatch);
    }
    let receipt = RepresentationReceiptV2::decode(receipt_bytes)?;
    receipt.verify_for(child, child_digest)?;
    Ok(receipt)
}
