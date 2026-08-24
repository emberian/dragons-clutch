#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact SDK-free wire contracts for the first real collateral lifecycle.
//!
//! This crate deliberately owns neither SVM addresses nor account data. A
//! composing adapter must authenticate every key, owner, PDA, executable
//! program, sysvar, immutable content identity, token state, rent amount, CPI
//! result, and exact pre/post balance delta described by these contracts.
//! [`frame`] names those obligations without depending on a Solana SDK, while
//! [`instruction`] owns hostile-decodable fixed instruction data.

#[cfg(test)]
extern crate std;

/// Fixed collateral-custody root and rent-refund ownership.
pub mod custody;
/// Exact ordered account-role contracts.
pub mod frame;
/// Exact fixed-width collateral lifecycle instruction data.
pub mod instruction;

pub use custody::{
    COLLATERAL_CUSTODY_BYTES, COLLATERAL_CUSTODY_MAGIC, COLLATERAL_CUSTODY_PDA_DOMAIN,
    COLLATERAL_CUSTODY_SCHEMA_VERSION, COLLATERAL_VAULT_PDA_DOMAIN, CollateralCustodyV1,
};
pub use frame::{
    AccountClass, AccountPrivilege, AccountRole, InstructionFrame, Role,
    SweepSurplusTokenAccountFactsV1, authorize_sweep_surplus_destination, instruction_frame,
    validate_account_frame,
};
pub use instruction::{
    CLOSE_EMPTY_POSITION_BYTES, CREATE_POSITION_AND_SPLIT_BYTES, CREATE_REALM_BYTES,
    CloseEmptyPositionV1, CreatePositionAndSplitV1, CreateRealmV1, FOUND_MARKET_AND_FUND_BYTES,
    FoundMarketAndFundV1, HEADER_BYTES, INSTRUCTION_MAGIC, INSTRUCTION_SCHEMA_VERSION,
    InstructionTag, InstructionV1, MERGE_COMPLETE_SET_BYTES, MergeCompleteSetV1,
    OPEN_COLLATERAL_VAULT_BYTES, OpenCollateralVaultV1, REDEEM_RESOLVED_OUTCOME_BYTES,
    RETIRE_EMPTY_VAULT_BYTES, RedeemResolvedOutcomeV1, RetireEmptyVaultV1,
    SPLIT_COMPLETE_SET_BYTES, SWEEP_SURPLUS_BYTES, SplitCompleteSetV1, SweepSurplusV1,
};

/// Explicit refusal returned by instruction or account-frame decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Instruction data did not have its one exact semantic width.
    InvalidLength,
    /// An output slice did not have its one exact semantic width.
    OutputLength,
    /// Instruction magic did not identify this contract.
    InvalidMagic,
    /// The instruction schema version is not implemented.
    UnsupportedSchema,
    /// The semantic tag is not implemented.
    UnknownInstructionTag,
    /// A typed decoder observed another known semantic tag.
    InstructionTagMismatch,
    /// Instruction flags were not the canonical zero value.
    NonCanonicalFlags,
    /// Reserved bytes were not all zero.
    NonCanonicalReservedBytes,
    /// The outcome count was outside the current provisional measured profile.
    InvalidOutcomeCount,
    /// An operation that must move collateral or claims named zero quantity.
    ZeroQuantity,
    /// A required Market, authority, or refund identity was all zero.
    ZeroIdentifier,
    /// The embedded immutable Realm record was invalid.
    InvalidRealm {
        /// The exact Realm-contract refusal.
        error: dclutch_realm_contract::Error,
    },
    /// The embedded canonical Market identity was invalid.
    InvalidMarketIdentity {
        /// The exact Market-core refusal.
        error: dclutch_core_contract::Error,
    },
    /// An account frame had too few or too many roles.
    AccountCountMismatch,
    /// An account had privileges inconsistent with its exact semantic role.
    AccountPrivilegeMismatch,
    /// The permissionless sweep destination was the collateral Vault itself.
    SweepDestinationAliasesVault,
    /// The permissionless sweep destination did not use the immutable Realm Mint.
    SweepDestinationMintMismatch,
    /// The permissionless sweep destination owner did not match Market rent refund.
    SweepDestinationOwnerMismatch,
}

/// Result alias for this contract crate.
pub type Result<T> = core::result::Result<T, Error>;
