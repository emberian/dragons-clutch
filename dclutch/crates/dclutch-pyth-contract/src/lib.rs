#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout, hostile-decodable contracts for a Pyth resolution child.
//!
//! This crate owns no Solana accounts, Pyth SDK types, CPI, rent lookup, or
//! authority policy.  Adapters supply those values and use these exact layouts.

/// Inline Pyth feed-semantics contract.
pub mod feed_profile;
/// Exact SDK-free resolution account-role and privilege frames.
pub mod frame;
/// Funding-account contract.
pub mod funding;
/// Resolve-instruction contract.
pub mod instruction;
/// Canonical categorical Pyth policy record.
pub mod policy;
/// Resolution-receipt contract.
pub mod receipt;

/// Refusal returned by this crate's total parsers and constructors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The input did not have its required exact length.
    InvalidLength,
    /// The input was missing a required nonempty body.
    EmptyBody,
    /// The supplied output buffer did not have the required exact length.
    OutputLength,
    /// The contract magic did not match.
    InvalidMagic,
    /// The schema is not implemented.
    UnsupportedSchema,
    /// A reserved byte was nonzero.
    NonCanonicalReservedBytes,
    /// A required opaque identifier was all zero.
    ZeroIdentifier,
    /// The base and quote asset semantic identifiers were identical.
    IdenticalAssetSemanticIdentifiers,
    /// An exact checked arithmetic operation overflowed.
    ArithmeticOverflow,
    /// Canonical capability funding validation refused the composition.
    InvalidCapabilityFunding {
        /// Exact capability-contract refusal.
        error: dclutch_capability_contract::Error,
    },
    /// The supplied founding selection was not the manifest's unique selection.
    FundingSelectionMismatch,
    /// A persisted resolution Fund did not have the required activated shape.
    InvalidResolutionFundShape,
    /// The external outcome count was outside the supported range.
    InvalidOutcomeCount,
    /// A receipt kind byte was not canonical.
    InvalidReceiptKind,
    /// A winner was outside the external outcome count.
    InvalidWinner,
    /// Price receipt times were not strictly ordered.
    InvalidPublishTimes,
    /// A price receipt's two slot observations did not agree.
    SlotMismatch,
    /// A receipt's fields were not canonical for its kind.
    NonCanonicalReceipt,
    /// An instruction tag was not a recognized categorical-resolution tag.
    InvalidInstructionTag,
    /// An instruction flags byte was nonzero.
    InvalidInstructionFlags,
    /// The declared instruction body length did not match the input.
    BodyLengthMismatch,
    /// The body cannot be represented by the wire's `u16` length field.
    BodyTooLarge,
    /// The categorical Pyth policy failed its kernel semantic validator.
    InvalidPolicy {
        /// Exact kernel policy refusal.
        error: dclutch_kernel::resolution::categorical_pyth_v1::PythV1Error,
    },
}

/// Result alias for this crate.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn nonzero(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}

pub(crate) fn zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

pub(crate) fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    let source = bytes.get(offset..end).ok_or(Error::InvalidLength)?;
    source.try_into().map_err(|_| Error::InvalidLength)
}
