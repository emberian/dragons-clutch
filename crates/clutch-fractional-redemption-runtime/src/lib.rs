// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact fractional redemption without rounded-away claimant value.
//!
//! This crate promotes the checked research algebra into a fixed-layout
//! runtime contract. It owns only the new policy, aggregate-credit ledger,
//! claimant credit, and permanent credit tombstone. Native claim supply,
//! Position balances, Replay ordinals, the immutable Resolution vector, and
//! Realm collateral remain owned by their canonical components.
//!
//! The concrete Solana account adapter exists, but every action remains
//! capability-disabled. Its handlers establish program ownership, canonical
//! PDAs, signers, the immutable Resolution and full-width ClaimLedger V3/Hoard
//! V2 owners, exact Token-2022 burn deltas, exact Realm-selected collateral
//! deltas, rent exemption, and rollback before committing one of this crate's
//! complete plans.

mod account;
mod adapter;
mod codec;
mod math;
mod transition;

pub use account::*;
pub use adapter::*;
pub use math::*;
pub use transition::*;

/// Largest canonical native outcome width.
pub const MAX_OUTCOMES: usize = 16;
/// Width of every content or account identity.
pub const ID_BYTES: usize = 32;

/// Deterministic refusal from the fractional-redemption runtime contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// Input or output was shorter than its exact fixed layout.
    Truncated,
    /// Input or output had trailing bytes.
    TrailingBytes,
    /// Account magic or discriminator was wrong.
    WrongTag,
    /// Account or instruction version was wrong.
    WrongVersion,
    /// Reserved bytes or inactive fixed-width fields were nonzero.
    NonCanonicalPadding,
    /// An identity required by this contract was zero.
    ZeroIdentity,
    /// Outcome count, denominator, weight, or lot was invalid.
    InvalidPayout,
    /// A source, claimant, Market, Resolution, Realm, or policy did not match.
    MismatchedBinding,
    /// An account lifecycle or terminal policy was invalid for the action.
    WrongPhase,
    /// An expected Replay or ledger sequence did not match.
    ReplayMismatch,
    /// A quantity required to be positive was zero.
    ZeroQuantity,
    /// A fast-path claim did not produce an exact whole-atom payout.
    NonIntegralLot,
    /// The presented canonical supply was smaller than the burn.
    InsufficientClaims,
    /// Canonical Hoard locked claim principal was smaller than the payout.
    InsufficientBacking,
    /// A source credit did not contain the requested numerator.
    InsufficientCredit,
    /// Checked integer arithmetic overflowed or underflowed.
    Arithmetic,
    /// `D*C >= weighted remaining liability + aggregate credit` did not hold.
    Insolvent,
    /// The aggregate credit owner differed from claimant-credit poststates.
    AggregateMismatch,
    /// A credit account was not empty at a zero-only close boundary.
    CreditOutstanding,
    /// Claims, credits, or collateral backing prevent terminal retirement.
    LiabilityOutstanding,
    /// A canonical account identity or generation already has a live owner.
    AlreadyInitialized,
    /// A permanent tombstone was required but absent or malformed.
    TombstoneRequired,
    /// The disabled SBF capability boundary refused before account inspection.
    CapabilityDisabled,
    /// The canonical Position V3 contract refused the proposed transition.
    PositionRefused,
    /// The canonical Replay V3 contract refused the proposed transition.
    ReplayRefused,
    /// The Realm-selected collateral contract refused the proposed join.
    CollateralRefused,
    /// The independently selected claim-issuance plane refused the join.
    ClaimPlaneRefused,
    /// Stored rent ownership or close disposition was invalid.
    RentRefused,
}

/// Result alias for total checked transitions.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn map_retirement(error: clutch_retirement::RetirementErrorV2) -> Error {
    use clutch_retirement::RetirementErrorV2;
    match error {
        RetirementErrorV2::WrongGeneration => Error::ReplayMismatch,
        RetirementErrorV2::ArithmeticOverflow | RetirementErrorV2::CounterOverflow => {
            Error::Arithmetic
        }
        RetirementErrorV2::Truncated => Error::Truncated,
        RetirementErrorV2::TrailingBytes => Error::TrailingBytes,
        RetirementErrorV2::WrongTag => Error::WrongTag,
        RetirementErrorV2::WrongVersion => Error::WrongVersion,
        _ => Error::RentRefused,
    }
}
