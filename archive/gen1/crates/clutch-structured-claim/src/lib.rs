#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact, bounded semantics for transferable structured claims.
//!
//! A claim is a primitive nonnegative integer vector over one immutable native
//! Egg basis. One wrapper is backed by the vector's common floor in cash and
//! its residual native Eggs. This crate compiles exact rational inputs, emits
//! canonical identity preimages, flattens composition, and stages custody
//! transitions without allocation.
//!
//! It deliberately contains no Solana SDK, account codec, Token-2022 logic,
//! hashing implementation, CPI, oracle, signer, clock, or account memory. An
//! adapter must authenticate those facts, hash the returned preimages, reconcile
//! `actual_supply` with the mint, and apply each successful transition atomically.

mod coefficient;
mod composition;
mod identity;

pub use coefficient::{
    realize_rational_shape, BackingPlan, ClaimVector, IntegerRealization, RationalCoefficient,
    RationalShape,
};
pub use composition::{CompositionAccumulator, CompositionDisposition, FlattenedComposition};
pub use identity::{
    DeploymentBinding, NativeBasisIdentity, NativeClaim, COMPLETE_SET_COMPRESSED_BACKING_V1,
    NATIVE_CLAIM_PREIMAGE_BYTES, WRAPPER_PRODUCT_PREIMAGE_BYTES,
};

/// Active or resolved phase projected from the authoritative base Market.
///
/// This is an input to supply-neutral Position custody only. It is not a
/// wrapper-owned Market lifecycle or supply ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MarketPhase {
    /// The base Market still admits Active-only custody.
    Active = 0,
    /// The base Market has an exact terminal resolution.
    Resolved = 1,
}

/// Maximum active native Egg width.
pub const MAX_OUTCOMES: usize = 16;
/// Minimum active native Egg width.
pub const MIN_OUTCOMES: u8 = 2;
/// Largest native B-spline degree admitted by the current protocol.
pub const MAX_BASIS_DEGREE: u8 = 3;
/// Collateral, Egg, and wrapper quantity in atomic units.
pub type Amount = u64;

/// A total refusal from structured-claim admission or transition staging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// Active outcome width is outside `2..=16`.
    InvalidOutcomeCount,
    /// Native basis degree is outside `0..=3`.
    InvalidDegree,
    /// A denominator is zero or conflicts with the immutable basis.
    InvalidDenominator,
    /// An exact rational is not reduced, or zero is not encoded as `0/1`.
    NonCanonicalRational,
    /// A value outside the active prefix is nonzero/noncanonical.
    NonCanonicalPadding,
    /// A claim has no positive coefficient.
    ZeroClaim,
    /// A claim is merely one native Egg and adds no atomic-product value.
    SingleEggClaim,
    /// A claim is only a complete set and must be represented as cash.
    CompleteSetClaim,
    /// A primitive claim vector has a common divisor other than one.
    NonPrimitiveClaim,
    /// A checked integer operation exceeded its frozen width.
    ArithmeticOverflow,
    /// A checked subtraction would become negative.
    ArithmeticUnderflow,
    /// An identity key is zero, aliased, or otherwise malformed.
    InvalidIdentity,
    /// Claims or a market projection name different native bases.
    DifferentBasis,
    /// A quantity is zero.
    ZeroQuantity,
    /// A composition contains no input legs.
    EmptyComposition,
    /// Authenticated input state violates a structural invariant.
    InvariantViolation,
}

/// Result alias for the structured-claim kernel.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) const fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

pub(crate) const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
