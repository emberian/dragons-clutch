// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure, fixed-memory contracts for a future fee-bearing runtime.
//!
//! This crate selects no production rate or treasury and moves no value.  It
//! owns the joins that must become exact before an adapter may relax any
//! zero-fee refusal: rated-policy selection, owner-level fee assessment,
//! signed-envelope allocation, recipient allocation, treasury accounting,
//! settlement conservation, redemption no-rake, and refusal to capitalize
//! liveness from Hoard principal or projected future revenue.

#![no_std]
#![forbid(unsafe_code)]

pub mod allocation;
pub mod integration;
pub mod selected;
pub mod treasury;

pub use clutch_batch_policy_identity::Identity32V1 as Id;

/// Maximum distinct owners or signed envelopes in one current general book.
pub const MAX_FEE_ROWS_V1: usize = 64;

/// A fail-closed refusal from the pure fee runtime contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ZeroIdentity,
    InvalidWidth,
    InvalidPolicy,
    ZeroRate,
    TreasuryUnavailable,
    UnauthenticatedRecipient,
    MismatchedBinding,
    NonCanonicalOrder,
    DuplicateIdentity,
    NonCanonicalCarry,
    ArithmeticOverflow,
    AmountOutOfRange,
    FeeEnvelopeExceeded,
    EmptyAllocation,
    ConservationFailure,
    UnauthorizedTreasury,
    OutstandingService,
    AlreadyClosed,
    InsufficientTreasuryBalance,
    RevenueSourceForbidden,
    RedemptionRakeForbidden,
    LivenessCapitalizationForbidden,
}

pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn live(id: Id) -> Result<()> {
    if id.is_zero() {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

pub(crate) fn add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}
