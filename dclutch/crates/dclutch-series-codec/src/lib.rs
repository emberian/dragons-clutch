#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout recurring-Series interpreter.
//!
//! Lean owns the widths, offsets, tags, schedule arithmetic, and canonical
//! fixtures in `DClutchSemantics.SeriesAbi`. This crate hostile-decodes those
//! wires and returns an owned atomic candidate. It never mutates accounts,
//! allocates, performs CPI, or treats a caller assertion as release authority.
//!
//! The adapter must authenticate the normalized Registry receipt against the
//! selected finalized release set, bind every decoded identity to the actual
//! account graph, perform the returned transfers and Market creation, and
//! commit the candidate only if all physical operations succeed.

#[rustfmt::skip]
mod generated_series;
mod interpreter;
mod wire;

pub use interpreter::*;
pub use wire::*;

/// Bytes in one immutable Series template.
pub const TEMPLATE_BYTES: usize = generated_series::TEMPLATE_BYTES;
/// Bytes in one replay-owned Series cursor.
pub const SERIES_STATE_BYTES: usize = generated_series::SERIES_BYTES;
/// Bytes in one exactly prepaid occurrence ticket.
pub const TICKET_BYTES: usize = generated_series::TICKET_BYTES;
/// Bytes in one normalized current Registry/Core receipt.
pub const RELEASE_RECEIPT_BYTES: usize = generated_series::RECEIPT_BYTES;
/// Bytes in one transition request.
pub const REQUEST_BYTES: usize = generated_series::REQUEST_BYTES;

/// Stable refusal from hostile decoding or transition interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have the one exact Lean-owned width.
    InvalidLength,
    /// Input magic did not identify the requested Series wire.
    InvalidMagic,
    /// Schema version did not match the generated version.
    UnsupportedVersion,
    /// A reserved byte was nonzero.
    NonCanonicalReserved,
    /// A phase, ticket phase, action, role, or receipt flag was unknown.
    UnknownTag,
    /// A required account or content identity was zero.
    ZeroIdentity,
    /// A required count, period, seed, or physical limit was zero.
    ZeroQuantity,
    /// Checked fixed-width arithmetic overflowed.
    ArithmeticOverflow,
    /// Immutable Template and projected state identities did not join exactly.
    IdentityMismatch,
    /// Registry release-set selection or current receipt did not match.
    ReleaseAdmission,
    /// Series or Ticket phase/cursor/funding invariants did not hold.
    InvalidState,
    /// Optimistic replay revision was stale or substituted.
    RevisionMismatch,
    /// The requested occurrence was early, late, or not yet expirable.
    ScheduleRefusal,
    /// A command-specific recipient was missing or noncanonical.
    RecipientRefusal,
    /// A fixed physical profile bound was exceeded.
    ProfileBound,
    /// More transfers were emitted than the mathematical compartment bound.
    TransferBound,
}

/// Result alias for the fixed Series boundary.
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests;
