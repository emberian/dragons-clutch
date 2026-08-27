#![no_std]

//! Tiny executable-source probe for the Verus/upstream-Rust/Anza-SBF seam.
//!
//! This is deliberately not protocol code. It has no Solana, allocator, FFI,
//! target-specific economic branch, or external dependency. The same source
//! is compiled by the host and SBF commands in `toolchain/scripts/run_lab.sh`.

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Overflow = 1,
    InvalidRange = 2,
    Ambiguous = 3,
}

/// Compute a fee in atom units, rounding down at the one named boundary.
pub fn fee_atoms(notional: u128, basis_points: u128) -> Result<u128, Error> {
    let numerator = notional.checked_mul(basis_points).ok_or(Error::Overflow)?;
    Ok(numerator / 10_000)
}

/// Classify an observation into one closed-open interval.
pub fn classify(value: u64, lower: u64, upper: u64) -> Result<u8, Error> {
    if lower >= upper {
        return Err(Error::InvalidRange);
    }
    if value < lower || value >= upper {
        return Err(Error::InvalidRange);
    }
    Ok(0)
}

/// Apply a bounded debit without exposing a proof-only precondition.
pub fn debit(balance: u128, amount: u128) -> Result<u128, Error> {
    balance.checked_sub(amount).ok_or(Error::Overflow)
}
