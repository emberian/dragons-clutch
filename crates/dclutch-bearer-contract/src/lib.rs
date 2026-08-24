#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Optional fixed-layout bearer outcome-claim capability for dClutch.
//!
//! The Market remains the sole owner of aggregate liabilities and Hoard
//! collateral. A [`state::BearerCapabilityV1`] owns only the amount of each
//! outcome currently represented by its canonical Token-2022 Mint. A native
//! [`dclutch_realm_contract::PositionV1`] remains the sole owner of balances in
//! that representation. Total transition plans move amounts between those two
//! representations or change them together with the Market ledger.
//!
//! Token-2022 parsing, PDA derivation, CPI, account memory, Rent, and atomic
//! persistence are deliberately an adapter trust boundary. See `DESIGN.md` for
//! the exact conservation theorem and boundary assumptions.

/// Canonical SDK-free account frames and privilege policy.
pub mod frame;
/// Canonical exact-width instruction codecs.
pub mod instruction;
/// Fixed-layout capability state and hostile Token-2022 projections.
pub mod state;
/// Total cross-representation transition planning.
pub mod transition;

/// Explicit refusal returned by the bearer capability contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A byte slice did not have its one exact canonical width.
    InvalidLength,
    /// A caller-owned output buffer did not have its one exact width.
    OutputLength,
    /// Magic bytes did not identify the requested record or instruction.
    InvalidMagic,
    /// A schema or profile discriminator is not implemented.
    UnsupportedSchema,
    /// Reserved bytes were not zero.
    NonCanonicalReservedBytes,
    /// A categorical width was outside provisional profile V1.
    InvalidOutcomeCount,
    /// An outcome index was outside the active width.
    InvalidOutcome,
    /// A required key or authority was all zero.
    ZeroIdentifier,
    /// A quantity which must move value was zero.
    ZeroQuantity,
    /// Exact integer arithmetic overflowed or underflowed.
    ArithmeticOverflow,
    /// The immutable Market generation did not match.
    GenerationMismatch,
    /// A Position, capability, or account named a different Market.
    MarketMismatch,
    /// The authenticated Realm identity did not match the Market identity.
    RealmMismatch,
    /// The transition is not admitted in the Market's current phase.
    InvalidMarketPhase,
    /// The Market's manifest content identity did not match.
    ManifestMismatch,
    /// The selected manifest entry was not the bearer kind.
    CapabilityKindMismatch,
    /// The selected manifest entry did not name this semantic release.
    CapabilityReleaseMismatch,
    /// The authenticated bearer config did not match the selected entry.
    CapabilityConfigMismatch,
    /// The selected manifest entry named another child layout.
    ChildSchemaMismatch,
    /// The selected manifest entry named another child derivation policy.
    ChildDerivationMismatch,
    /// The capability funding quote did not exactly fund physical activation.
    ActivationFundingMismatch,
    /// The SPL Token-2022 program was not selected.
    WrongTokenProgram,
    /// A supplied claim Mint was not the canonical outcome Mint.
    WrongMint,
    /// A Mint or token Account named the wrong authority.
    WrongAuthority,
    /// A claim Mint did not use zero display decimals.
    WrongDecimals,
    /// A Mint or token Account was not initialized.
    UninitializedTokenState,
    /// The exact required Token-2022 Mint extensions were not present.
    WrongMintExtensions,
    /// A claim token Account was not initialized and transferable.
    TokenAccountNotTransferable,
    /// A holder claim Account retained unsupported TLV extensions.
    WrongTokenAccountExtensions,
    /// A claim token Account retained a wrapped-native reserve.
    NativeTokenAccount,
    /// A claim token Account held fewer atoms than requested.
    InsufficientTokenBalance,
    /// Observed Token-2022 Mint supply differed from accounted bearer supply.
    UnaccountedMintSupply,
    /// Account roles aliased outside the one explicit program-account exception.
    AccountAlias,
    /// An account frame had the wrong role or exact privilege set.
    InvalidAccountFrame,
    /// Bearer supply remained when retirement was requested.
    OutstandingBearerSupply,
    /// Capability funding semantics refused the transition.
    CapabilityContract {
        /// Exact capability-contract refusal.
        error: dclutch_capability_contract::Error,
    },
    /// Provider-neutral Market semantics refused the transition.
    MarketContract {
        /// Exact Market-contract refusal.
        error: dclutch_market_contract::Error,
    },
    /// Realm or native Position semantics refused the transition.
    RealmContract {
        /// Exact Realm-contract refusal.
        error: dclutch_realm_contract::Error,
    },
}

/// Result alias for this contract.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}

pub(crate) fn require_quantity(quantity: u64) -> Result<()> {
    if quantity == 0 {
        Err(Error::ZeroQuantity)
    } else {
        Ok(())
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
