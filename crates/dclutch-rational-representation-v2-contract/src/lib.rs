#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Physical composition contract for exact rational claim representations.
//!
//! This crate hostile-decodes one Lean-owned request, joins it to the pure
//! rational representation kernel, emits one canonical Claims plan and an
//! ordered Token effect stream, and accepts a receipt only after exact Claims,
//! Token, and (for positive terminal payout) Custody evidence joins. It owns
//! replay revision only; Claims and Token remain the economic/supply owners.

#[allow(missing_docs)]
mod generated;
mod plan;
mod receipt;
mod request;

pub use generated::{
    ASSET_BYTES_V2, PHYSICAL_ABI_VERSION_V2, RECEIPT_BYTES_V2, RECEIPT_MAGIC_V2,
    REQUEST_HEADER_BYTES_V2, REQUEST_MAGIC_V2,
};
pub use plan::{
    PreparedRepresentationV2, TokenEffectIterV2, TokenEffectStyleV2, TokenEffectV2, prepare,
};
pub use receipt::{CompletionEvidenceV2, RepresentationReceiptV2, finalize};
pub use request::{
    AssetV2, CallerRoleV2, RepresentationActionV2, RepresentationRequestHeaderV2,
    RepresentationRequestV2,
};

/// Exact absent revision sentinel shared with the canonical Claims ABI.
pub const ABSENT_REVISION: u64 = dclutch_claims_svm::NO_POSITION_REVISION;
/// Claims PDA seed for one descriptor's representation authority.
pub const RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2: &[u8] =
    b"dclutch:rational-authority:v2";
/// Claims PDA seed for one outcome's canonical shard Mint.
pub const RATIONAL_SHARD_MINT_SEED_V2: &[u8] = b"dclutch:rational-shard-mint:v2";
/// Claims PDA seed for one outcome's canonical Claims custody owner.
pub const RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2: &[u8] = b"dclutch:rational-claims:v2";
/// Claims PDA seed for one holder's rational representation replay cursor.
pub const RATIONAL_REPLAY_SEED_V2: &[u8] = b"dclutch:rational-replay:v2";

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
    /// The canonical Claims plan or returned receipt differed.
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
