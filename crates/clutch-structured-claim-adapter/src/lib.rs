#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Allocation-free runtime contracts for transferable structured claims.
//!
//! The economic machine lives in `clutch-structured-claim`. This crate owns
//! the adapter-side facts that do not belong there: the exact persisted
//! descriptor image, deployment/basis reconstruction, and an atomic transfer
//! plan over two authenticated base Position projections. It deliberately has
//! no Solana SDK, CPI, hashing implementation, PDA implementation, account
//! memory, or Token-2022 parser. A small SBF adapter must authenticate those
//! boundaries and execute the returned plans exactly.

mod descriptor;
mod position_transfer;

pub use descriptor::{
    reconstruct_descriptor_identity_v1, DescriptorBasisV1, DescriptorIdentityV1,
    DescriptorStateV1, StructuredClaimDescriptorV1, DESCRIPTOR_ACCOUNT_BYTES,
    DESCRIPTOR_ACCOUNT_TAG, DESCRIPTOR_ACCOUNT_VERSION,
};
pub use position_transfer::{
    prepare_atomic_position_asset_transfer_v1, AssetTransferPhasePolicyV1,
    AtomicPositionAssetTransferRequestV1, AtomicPositionAssetTransferResultV1, PositionProjectionV1,
};

/// Maximum native outcome width shared with the structured-claim kernel.
pub const MAX_OUTCOMES: usize = clutch_structured_claim::MAX_OUTCOMES;
/// Atomic Realm-collateral or native-Egg quantity.
pub type Amount = u64;

/// Deterministic refusal from a descriptor or position-transfer contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// A byte image has the wrong exact width.
    InvalidLength,
    /// A persisted tag or version is not this contract.
    InvalidHeader,
    /// Reserved flags or padding are nonzero.
    NonCanonicalPadding,
    /// A key or digest is absent or aliases a distinct authority role.
    InvalidIdentity,
    /// The descriptor cannot reconstruct the authenticated native claim.
    InvalidClaim,
    /// The descriptor's persisted lifecycle value is unknown.
    InvalidState,
    /// A checked integer operation overflowed.
    ArithmeticOverflow,
    /// A checked integer operation underflowed.
    ArithmeticUnderflow,
    /// A quantity vector carries no asset.
    ZeroQuantity,
    /// A transfer names different markets or unexpected generations.
    DifferentPositionDomain,
    /// A Position projection is closed or structurally invalid.
    InvalidPosition,
    /// The selected phase policy rejects the authenticated market phase.
    InvalidPhase,
    /// Free cash or native Eggs cannot cover the requested debit.
    InsufficientFreeAssets,
    /// A Replay sequence cannot advance exactly once.
    ReplayExhausted,
    /// A prospective result violates exact conservation.
    InvariantViolation,
}

/// Result alias for adapter contracts.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn put<const N: usize>(
    output: &mut [u8; N],
    cursor: &mut usize,
    bytes: &[u8],
) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    let destination = output
        .get_mut(*cursor..end)
        .ok_or(Error::InvalidLength)?;
    destination.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

pub(crate) fn take<'a>(input: &'a [u8], cursor: &mut usize, width: usize) -> Result<&'a [u8]> {
    let end = cursor.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidLength)?;
    *cursor = end;
    Ok(value)
}
