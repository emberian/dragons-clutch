#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout, hostile-decodable contracts for a Pyth resolution child.
//!
//! This crate owns no Solana accounts, Pyth SDK types, CPI, rent lookup, or
//! authority policy.  Adapters supply those values and use these exact layouts.

/// Inline Pyth feed-semantics contract.
pub mod feed_profile;
/// Funding-account contract.
pub mod funding;
/// Resolve-instruction contract.
pub mod instruction;
/// Composed categorical Market account contract.
pub mod market;
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
    /// A required bounty was zero.
    ZeroBounty,
    /// An exact checked arithmetic operation overflowed.
    ArithmeticOverflow,
    /// The actual funding balance cannot meet its immutable minimum.
    Underfunded,
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
    /// An instruction tag was not the resolve tag.
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
    /// The embedded Market root failed its owning contract's validator.
    InvalidMarketRoot {
        /// Exact Market-root contract refusal.
        error: dclutch_core_contract::Error,
    },
    /// The persisted categorical liabilities failed kernel validation.
    InvalidLedger {
        /// Exact kernel ledger refusal.
        error: dclutch_kernel::Error,
    },
    /// The policy's price cells plus failure outcome did not equal the Market width.
    PolicyOutcomeCountMismatch,
    /// A terminal receipt winner did not match the policy's outcome partition.
    ReceiptPolicyWinnerMismatch,
    /// The root lifecycle and receipt kind were not a canonical combination.
    PhaseReceiptMismatch,
    /// A lifecycle phase that requires economic emptiness retained hoard or supply.
    NonemptyEconomicState,
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
