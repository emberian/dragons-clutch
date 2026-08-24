#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Provider-neutral active categorical Market contract for dClutch.
//!
//! The Market owns only its compact root, claimant-backing Hoard atoms, one
//! aggregate supply per exhaustive ordered state cell, and the categorical
//! terminal truth needed for redemption and replay. Resolution providers,
//! source accounts, feeds, oracle policies, funding, execution venues, token
//! programs, and Solana account mechanics are deliberately outside this crate.

/// Exact fixed-layout categorical Market state and transitions.
pub mod market;

/// Refusal returned by the provider-neutral Market contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An input did not have its one exact canonical width.
    InvalidLength,
    /// A caller-owned output buffer did not have its one exact width.
    OutputLength,
    /// The Market magic did not match this account contract.
    InvalidMagic,
    /// The Market schema version is not implemented.
    UnsupportedSchema,
    /// The categorical profile discriminator is not implemented.
    UnsupportedProfile,
    /// The exact categorical width is outside the selected profile.
    InvalidOutcomeCount,
    /// Reserved or inactive summary bytes were not all zero.
    NonCanonicalReservedBytes,
    /// A settlement status byte was not defined.
    UnknownSettlementStatus,
    /// A resolved settlement named an outcome outside the exact Market width.
    InvalidWinner,
    /// A resolved settlement used the reserved zero terminal sequence.
    ZeroTerminalSequence,
    /// Product-owned settlement semantics refused a field.
    InvalidProductContract {
        /// Exact Product-contract refusal.
        error: dclutch_product_contract::Error,
    },
    /// The compact Market root failed its semantic owner's validation.
    InvalidMarketRoot {
        /// Exact root-contract refusal.
        error: dclutch_core_contract::Error,
    },
    /// The aggregate categorical liabilities failed kernel validation.
    InvalidLedger {
        /// Exact categorical-kernel refusal.
        error: dclutch_kernel::Error,
    },
    /// Root phase and optional settlement truth were not a canonical pair.
    PhaseSettlementMismatch,
    /// A canceled or terminal lifecycle state retained forbidden economics.
    NonemptyEconomicState,
    /// Checked exact integer layout arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for this contract crate.
pub type Result<T> = core::result::Result<T, Error>;
