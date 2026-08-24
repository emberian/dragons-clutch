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
pub mod codec;
pub mod integration;
pub mod intent;
pub mod projection;
pub mod retirement;
pub mod selected;
pub mod terminal;
pub mod treasury;
pub mod weight_v2;

pub use clutch_batch_policy_identity::Identity32V1 as Id;

/// Maximum distinct owners or signed envelopes in one current general book.
pub const MAX_FEE_ROWS_V1: usize = 64;

/// A fail-closed refusal from the pure fee runtime contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ZeroIdentity,
    IdentityAlias,
    InvalidWidth,
    InvalidPolicy,
    ZeroRate,
    TreasuryUnavailable,
    UnauthenticatedRecipient,
    MismatchedBinding,
    NonCanonicalOrder,
    DuplicateIdentity,
    NonCanonicalCarry,
    WrongAccountKind,
    WrongVersion,
    InvalidAccountData,
    NonCanonicalPadding,
    ArithmeticOverflow,
    AmountOutOfRange,
    FeeEnvelopeExceeded,
    SellerFeeForbidden,
    TerminalStateRequired,
    InsufficientBuyReservation,
    MissingParticipant,
    SelectedFeeTotalMismatch,
    IncompleteAccountGraph,
    EmptyAllocation,
    ConservationFailure,
    UnauthorizedTreasury,
    OutstandingService,
    AlreadyClosed,
    InsufficientTreasuryBalance,
    RevenueSourceForbidden,
    RedemptionRakeForbidden,
    LivenessCapitalizationForbidden,
    InvalidTerminalDisposition,
    MissingClosure,
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

pub(crate) fn independent(identities: &[Id]) -> Result<()> {
    let mut left = 0usize;
    while left < identities.len() {
        live(identities[left])?;
        let mut right = left + 1;
        while right < identities.len() {
            if identities[left] == identities[right] {
                return Err(Error::IdentityAlias);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}
