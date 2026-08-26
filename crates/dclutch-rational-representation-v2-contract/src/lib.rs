#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Physical composition contract for exact rational claim representations.
//!
//! This crate hostile-decodes one Lean-owned request, joins it to the pure
//! rational representation kernel, emits one canonical affine Claims packet
//! for open transfers and an ordered Token effect stream, and accepts a receipt
//! only after exact Claims and Token evidence joins. Terminal completion is
//! held until typed LiabilityBasisV2 payout evidence is supplied. It owns replay
//! revision only; Claims and Token remain the economic/supply owners.

#[allow(missing_docs)]
mod generated;
#[allow(missing_docs)]
mod generated_hot_v3;
mod hot_v3;
mod plan;
mod receipt;
mod replay;
mod request;
mod seeds;

pub use generated::{
    ASSET_BYTES_V2, PHYSICAL_ABI_VERSION_V2, RECEIPT_BYTES_V2, RECEIPT_MAGIC_V2,
    REQUEST_HEADER_BYTES_V2, REQUEST_MAGIC_V2,
};
pub use generated_hot_v3::{
    RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3, RATIONAL_TERMINAL_HOT_ACTOR_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_CLAIMS_CUSTODY_OWNER_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_COEFFICIENT_OFFSET_V3, RATIONAL_TERMINAL_HOT_ASSET_COUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_OFFSET_V3, RATIONAL_TERMINAL_HOT_ASSET_SHARD_MINT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_STRUCTURED_CUSTODY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_COLLATERAL_RECIPIENT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_DENOMINATOR_OFFSET_V3, RATIONAL_TERMINAL_HOT_DESCRIPTOR_ID_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_ACTOR_POSITION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3, RATIONAL_TERMINAL_HOT_GENERATION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_GRAPH_ID_OFFSET_V3, RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_MAGIC_V3, RATIONAL_TERMINAL_HOT_MARKET_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_OUTCOME_COUNT_OFFSET_V3, RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_QUANTITY_OFFSET_V3, RATIONAL_TERMINAL_HOT_REALM_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_RECEIPT_ACCOUNT_OFFSET_V3, RATIONAL_TERMINAL_HOT_RECEIPT_MINT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_RELEASE_SET_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REPRESENTATION_AUTHORITY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3, RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3,
    RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_PREIMAGE_V3,
    RATIONAL_TERMINAL_HOT_RESERVED_HEADER_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_RESERVED_TAIL_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_SELECTED_OUTCOME_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_TOKEN_PROGRAM_OFFSET_V3, RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_VERSION_V3,
};
pub use hot_v3::{
    RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3, RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3,
    RATIONAL_TERMINAL_IDENTITY_ACTOR_SHARD_ACCOUNT_V3, RATIONAL_TERMINAL_IDENTITY_ACTOR_V3,
    RATIONAL_TERMINAL_IDENTITY_CLAIMS_CUSTODY_OWNER_V3,
    RATIONAL_TERMINAL_IDENTITY_COLLATERAL_RECIPIENT_V3, RATIONAL_TERMINAL_IDENTITY_DESCRIPTOR_V3,
    RATIONAL_TERMINAL_IDENTITY_GRAPH_V3, RATIONAL_TERMINAL_IDENTITY_MARKET_V3,
    RATIONAL_TERMINAL_IDENTITY_PARENT_DIGEST_V3, RATIONAL_TERMINAL_IDENTITY_REALM_V3,
    RATIONAL_TERMINAL_IDENTITY_RECEIPT_MINT_V3, RATIONAL_TERMINAL_IDENTITY_RELEASE_SET_V3,
    RATIONAL_TERMINAL_IDENTITY_REPRESENTATION_AUTHORITY_V3,
    RATIONAL_TERMINAL_IDENTITY_SHARD_MINT_V3, RATIONAL_TERMINAL_IDENTITY_STRUCTURED_CUSTODY_V3,
    RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3,
    RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3, RATIONAL_TERMINAL_SCALAR_ACTOR_SHARDS_V3,
    RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3, RATIONAL_TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3, RATIONAL_TERMINAL_SCALAR_CUSTODY_POSITION_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_CUSTODY_REPLAY_REVISION_V3, RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3,
    RATIONAL_TERMINAL_SCALAR_GENERATION_V3, RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3,
    RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3, RATIONAL_TERMINAL_SCALAR_QUANTITY_V3,
    RATIONAL_TERMINAL_SCALAR_RECEIPT_SUPPLY_V3,
    RATIONAL_TERMINAL_SCALAR_REPRESENTATION_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3, RATIONAL_TERMINAL_SCALAR_SHARD_SUPPLY_V3,
    RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3, RationalTerminalHotRegistersV3,
    RationalTerminalHotRequestV3, verify_rational_terminal_receipt_v3,
};
pub use plan::{
    AffineBatchContextV2, PreparedRepresentationV2, TokenEffectIterV2, TokenEffectStyleV2,
    TokenEffectV2, prepare,
};
pub use receipt::{CompletionEvidenceV2, RepresentationReceiptV2, finalize};
pub use replay::{
    RATIONAL_REPLAY_BYTES_V2, RATIONAL_REPLAY_MAGIC_V2, RATIONAL_REPLAY_VERSION_V2,
    RationalReplayV2,
};
pub use request::{
    AssetV2, CallerRoleV2, RepresentationActionV2, RepresentationRequestHeaderV2,
    RepresentationRequestV2,
};
pub use seeds::{RATIONAL_RECEIPT_MINT_SEED_V2, RationalReceiptMintSeedsV2};

/// Exact absent revision sentinel shared with the canonical Claims ABI.
pub const ABSENT_REVISION: u64 = dclutch_claims_svm::NO_POSITION_REVISION;
/// Claims PDA seed for one descriptor's representation authority.
pub const RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2: &[u8] = b"dclutch:rational-authority:v2";
/// Claims PDA seed for one outcome's canonical shard Mint.
pub const RATIONAL_SHARD_MINT_SEED_V2: &[u8] = b"dclutch:rational-shard-mint:v2";
/// Claims PDA seed for one outcome's canonical Claims custody owner.
pub const RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2: &[u8] = b"dclutch:rational-claims:v2";
/// Claims PDA seed for one outcome's canonical closeable Structured custody account.
pub const RATIONAL_STRUCTURED_CUSTODY_SEED_V2: &[u8] = b"dclutch:rational-structured:v2";
/// Claims PDA seed for one holder's rational representation replay cursor.
pub const RATIONAL_REPLAY_SEED_V2: &[u8] = b"dclutch:rational-replay:v2";
/// Fixed account prefix before one four-account row per active request asset.
///
/// The suffix added in the successor frame is the independently authenticated
/// linked basis plus Product Runtime V2 graph. These are immutable authority
/// inputs, never duplicated Claims balances.
pub const RATIONAL_BASE_ACCOUNT_COUNT_V2: usize = 32;
/// Accounts in one active request asset row.
pub const RATIONAL_ASSET_ACCOUNT_COUNT_V2: usize = 4;
/// Positive terminal Claims-coordinate plus Custody account suffix width.
pub const RATIONAL_TERMINAL_ACCOUNT_COUNT_V2: usize = 13;

/// Stable hostile-decode, composition, or postcondition refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed or runtime-derived wire width differed.
    InvalidLength,
    /// Magic bytes selected another wire family.
    InvalidMagic,
    /// The physical ABI version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes or a tag were noncanonical.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// The action's receipt, terminal, outcome, asset, or revision shape differed.
    InvalidActionShape,
    /// Runtime outcome or asset width was zero or unrepresentable.
    InvalidWidth,
    /// Two semantic roles or two asset rows aliased.
    AccountAlias,
    /// A checked scalar, revision, or offset overflowed.
    ArithmeticOverflow,
    /// An observed balance could not fund an exact action.
    InsufficientBalance,
    /// Request, graph, descriptor, Market, or Token observations did not join.
    ProjectionMismatch,
    /// The canonical affine Claims packet or returned receipt differed.
    ClaimsMismatch,
    /// An ordered Token effect or post-state observation differed.
    TokenMismatch,
    /// A required Custody request/receipt differed or appeared when inactive.
    CustodyMismatch,
    /// A replay, program, request digest, or normalized receipt was substituted.
    ReceiptMismatch,
}

/// Result alias for the physical composition contract.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn require_nonzero(value: [u8; 32]) -> Result<[u8; 32]> {
    if is_zero(value) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(value)
    }
}

pub(crate) fn is_zero(value: [u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

pub(crate) fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if subslice(input, offset, width)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonical)
    }
}

pub(crate) fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

pub(crate) fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

pub(crate) fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

pub(crate) fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

pub(crate) fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    subslice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

pub(crate) fn subslice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)
}

pub(crate) fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

pub(crate) fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}
