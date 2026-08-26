//! Ephemeral Hot V3 terminal intent and exact Claims-child specialization.

use crate::{
    Error, RepresentationActionV2, RepresentationReceiptV2, RepresentationRequestV2, Result,
    array_at, byte_at,
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

/// Exact RequestProfile/Transition register contract for terminal redemption.
///
/// Identity zero is reserved for the authenticated Hot family digest. Every
/// remaining coordinate is projected from the exact family request; there is
/// no caller-authored register DTO. Scalars preserve full `u64` values and
/// widen the runtime `u32` Product coordinates without truncation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalTerminalHotRegistersV3 {
    identities: [[u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3],
    scalars: [u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3],
}

/// Exact common identity-bank width for terminal Rational Hot V3.
pub const RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3: usize = 15;
/// Exact common scalar-bank width for terminal Rational Hot V3.
pub const RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3: usize = 16;

/// Identity register containing SHA-256 of the exact family request.
pub const RATIONAL_TERMINAL_IDENTITY_PARENT_DIGEST_V3: usize = 0;
/// Identity register containing the execution release set.
pub const RATIONAL_TERMINAL_IDENTITY_RELEASE_SET_V3: usize = 1;
/// Identity register containing the logical Core Market.
pub const RATIONAL_TERMINAL_IDENTITY_MARKET_V3: usize = 2;
/// Identity register containing the representation graph.
pub const RATIONAL_TERMINAL_IDENTITY_GRAPH_V3: usize = 3;
/// Identity register containing the rational descriptor.
pub const RATIONAL_TERMINAL_IDENTITY_DESCRIPTOR_V3: usize = 4;
/// Identity register containing the redeeming holder.
pub const RATIONAL_TERMINAL_IDENTITY_ACTOR_V3: usize = 5;
/// Identity register containing the Structured receipt Mint.
pub const RATIONAL_TERMINAL_IDENTITY_RECEIPT_MINT_V3: usize = 6;
/// Identity register containing the Claims representation authority.
pub const RATIONAL_TERMINAL_IDENTITY_REPRESENTATION_AUTHORITY_V3: usize = 7;
/// Identity register containing the selected Token program.
pub const RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3: usize = 8;
/// Identity register containing the immutable Realm.
pub const RATIONAL_TERMINAL_IDENTITY_REALM_V3: usize = 9;
/// Identity register containing the holder's collateral recipient.
pub const RATIONAL_TERMINAL_IDENTITY_COLLATERAL_RECIPIENT_V3: usize = 10;
/// Identity register containing the selected outcome shard Mint.
pub const RATIONAL_TERMINAL_IDENTITY_SHARD_MINT_V3: usize = 11;
/// Identity register containing the holder shard Token Account.
pub const RATIONAL_TERMINAL_IDENTITY_ACTOR_SHARD_ACCOUNT_V3: usize = 12;
/// Identity register containing the inactive Structured custody Token Account.
pub const RATIONAL_TERMINAL_IDENTITY_STRUCTURED_CUSTODY_V3: usize = 13;
/// Identity register containing the canonical Claims custody owner.
pub const RATIONAL_TERMINAL_IDENTITY_CLAIMS_CUSTODY_OWNER_V3: usize = 14;

/// Scalar register containing the representation replay revision.
pub const RATIONAL_TERMINAL_SCALAR_REPRESENTATION_REVISION_V3: usize = 0;
/// Scalar register containing the Claims Market revision.
pub const RATIONAL_TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V3: usize = 1;
/// Scalar register containing the absent actor-Position sentinel.
pub const RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3: usize = 2;
/// Scalar register containing the Claims custody-Position revision.
pub const RATIONAL_TERMINAL_SCALAR_CUSTODY_POSITION_REVISION_V3: usize = 3;
/// Scalar register containing the Custody replay revision.
pub const RATIONAL_TERMINAL_SCALAR_CUSTODY_REPLAY_REVISION_V3: usize = 4;
/// Scalar register containing the immutable Market generation.
pub const RATIONAL_TERMINAL_SCALAR_GENERATION_V3: usize = 5;
/// Scalar register containing exact terminal native-claim quantity.
pub const RATIONAL_TERMINAL_SCALAR_QUANTITY_V3: usize = 6;
/// Scalar register containing the exact shard denominator.
pub const RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3: usize = 7;
/// Scalar register containing pre-execution receipt-Mint supply.
pub const RATIONAL_TERMINAL_SCALAR_RECEIPT_SUPPLY_V3: usize = 8;
/// Scalar register containing Product-owned runtime outcome count.
pub const RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3: usize = 9;
/// Scalar register containing selected runtime outcome.
pub const RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3: usize = 10;
/// Scalar register containing the fixed one-row asset count.
pub const RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3: usize = 11;
/// Scalar register containing the selected portfolio coefficient.
pub const RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3: usize = 12;
/// Scalar register containing the shard-Mint pre-supply.
pub const RATIONAL_TERMINAL_SCALAR_SHARD_SUPPLY_V3: usize = 13;
/// Scalar register containing the holder's pre-burn shard balance.
pub const RATIONAL_TERMINAL_SCALAR_ACTOR_SHARDS_V3: usize = 14;
/// Scalar register containing the inactive Structured custody balance.
pub const RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3: usize = 15;

impl RationalTerminalHotRegistersV3 {
    /// Read one exact common identity register.
    pub fn identity(self, index: usize) -> Result<[u8; 32]> {
        self.identities
            .get(index)
            .copied()
            .ok_or(Error::InvalidWidth)
    }

    /// Read one exact common scalar register.
    pub fn scalar(self, index: usize) -> Result<u64> {
        self.scalars.get(index).copied().ok_or(Error::InvalidWidth)
    }

    /// Borrow the exact identity bank.
    pub const fn identities(&self) -> &[[u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3] {
        &self.identities
    }

    /// Borrow the exact scalar bank.
    pub const fn scalars(&self) -> &[u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3] {
        &self.scalars
    }
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
        if byte_at(input, RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3)? != ACTION_REDEEM_TERMINAL
            || byte_at(input, RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3)? != CALLER_ROLE_TRADING
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
        output
            .get_mut(
                RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3
                    ..RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3 + 32,
            )
            .ok_or(Error::InvalidLength)?
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

    /// Project the exact family request into its frozen register contract.
    ///
    /// This is the semantic reference for generated RequestProfile artifacts.
    /// It first performs the same child specialization which production Hot
    /// executes, so every register is read from the canonical Claims request
    /// rather than from a parallel client object.
    pub fn project_registers(
        self,
        family_digest: [u8; 32],
    ) -> Result<RationalTerminalHotRegistersV3> {
        let mut child_bytes = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
        let child = self.specialize_child_into(family_digest, &mut child_bytes)?;
        let header = child.header();
        let asset = child.asset(0)?;
        let mut identities = [[0_u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3];
        identities[RATIONAL_TERMINAL_IDENTITY_PARENT_DIGEST_V3] = family_digest;
        identities[RATIONAL_TERMINAL_IDENTITY_RELEASE_SET_V3] = header.release_set;
        identities[RATIONAL_TERMINAL_IDENTITY_MARKET_V3] = header.market;
        identities[RATIONAL_TERMINAL_IDENTITY_GRAPH_V3] = header.graph_id;
        identities[RATIONAL_TERMINAL_IDENTITY_DESCRIPTOR_V3] = header.descriptor_id;
        identities[RATIONAL_TERMINAL_IDENTITY_ACTOR_V3] = header.actor;
        identities[RATIONAL_TERMINAL_IDENTITY_RECEIPT_MINT_V3] = header.receipt_mint;
        identities[RATIONAL_TERMINAL_IDENTITY_REPRESENTATION_AUTHORITY_V3] =
            header.representation_authority;
        identities[RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3] = header.token_program;
        identities[RATIONAL_TERMINAL_IDENTITY_REALM_V3] = header.realm;
        identities[RATIONAL_TERMINAL_IDENTITY_COLLATERAL_RECIPIENT_V3] =
            header.collateral_recipient;
        identities[RATIONAL_TERMINAL_IDENTITY_SHARD_MINT_V3] = asset.shard_mint;
        identities[RATIONAL_TERMINAL_IDENTITY_ACTOR_SHARD_ACCOUNT_V3] = asset.actor_shard_account;
        identities[RATIONAL_TERMINAL_IDENTITY_STRUCTURED_CUSTODY_V3] =
            asset.structured_custody_account;
        identities[RATIONAL_TERMINAL_IDENTITY_CLAIMS_CUSTODY_OWNER_V3] = asset.claims_custody_owner;

        let mut scalars = [0_u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3];
        scalars[RATIONAL_TERMINAL_SCALAR_REPRESENTATION_REVISION_V3] =
            header.expected_representation_revision;
        scalars[RATIONAL_TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V3] =
            header.expected_claims_market_revision;
        scalars[RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3] =
            header.expected_actor_position_revision;
        scalars[RATIONAL_TERMINAL_SCALAR_CUSTODY_POSITION_REVISION_V3] =
            header.expected_custody_position_revision;
        scalars[RATIONAL_TERMINAL_SCALAR_CUSTODY_REPLAY_REVISION_V3] =
            header.expected_custody_replay_revision;
        scalars[RATIONAL_TERMINAL_SCALAR_GENERATION_V3] = header.generation;
        scalars[RATIONAL_TERMINAL_SCALAR_QUANTITY_V3] = header.quantity;
        scalars[RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3] = header.denominator;
        scalars[RATIONAL_TERMINAL_SCALAR_RECEIPT_SUPPLY_V3] = header.expected_receipt_supply;
        scalars[RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3] = u64::from(header.outcome_count);
        scalars[RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3] = u64::from(header.selected_outcome);
        scalars[RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3] = u64::from(header.asset_count);
        scalars[RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3] = asset.coefficient;
        scalars[RATIONAL_TERMINAL_SCALAR_SHARD_SUPPLY_V3] = asset.expected_shard_supply;
        scalars[RATIONAL_TERMINAL_SCALAR_ACTOR_SHARDS_V3] = asset.expected_actor_shards;
        scalars[RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3] = asset.expected_structured_shards;
        Ok(RationalTerminalHotRegistersV3 {
            identities,
            scalars,
        })
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
