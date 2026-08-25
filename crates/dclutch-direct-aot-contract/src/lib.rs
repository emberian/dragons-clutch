#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Stateless AOT implementation of the Lean-owned Direct V2 descriptor.
//!
//! Account decoding, signature authorization, replay ownership, Registry and
//! Loader reauthentication, effect projection, CPI, and state writes remain in
//! Trading's adapter boundary. This crate only evaluates the exact register
//! relation and writes an accepted candidate bank atomically.

#[rustfmt::skip]
#[allow(missing_docs)]
mod generated;

pub use generated::*;

/// Direct V2 semantic phase tag for an open Market.
pub const OPEN_PHASE_V2: u64 = 1;
/// Direct V2 semantic side tag for a seller.
pub const SELL_SIDE_V2: u64 = 0;
/// Direct V2 semantic side tag for a buyer.
pub const BUY_SIDE_V2: u64 = 1;
/// Named basis-point denominator and sole fee floor boundary.
pub const FEE_DENOMINATOR_V2: u64 = 10_000;

/// Stable refusal from the stateless Direct AOT evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input, scratch, or output did not have the descriptor's exact width.
    RegisterWidthMismatch,
    /// One semantic admission relation evaluated to false.
    CheckFailed,
    /// A lifecycle scalar was not FOK, IOC, or GTC.
    UnknownLifecycle,
    /// Checked u64 arithmetic overflowed.
    ArithmeticOverflow,
    /// Exact quote division had a zero denominator or nonzero remainder.
    InexactDivision,
    /// Floor fee division had a zero denominator.
    ZeroDenominator,
}

/// Result alias for Direct AOT execution.
pub type Result<T> = core::result::Result<T, Error>;

/// Immutable authenticated input register bank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterInput<'a> {
    /// Exact scalar values in descriptor order.
    pub scalars: &'a [u64],
    /// Exact 32-byte identities in descriptor order.
    pub identities: &'a [[u8; 32]],
}

/// Mutable caller-owned candidate register bank.
pub struct RegisterOutput<'a> {
    /// Exact scalar storage in descriptor order.
    pub scalars: &'a mut [u64],
    /// Exact identity storage in descriptor order.
    pub identities: &'a mut [[u8; 32]],
}

/// Evaluate Direct V2 atomically into caller-owned output.
///
/// All checks and arithmetic run against `scratch`. Refusal may alter scratch,
/// but input and output remain byte-for-byte unchanged. Accepted output is the
/// complete candidate register bank, not an independently authored effect plan.
pub fn execute_atomic(
    input: RegisterInput<'_>,
    scratch: RegisterOutput<'_>,
    output: RegisterOutput<'_>,
) -> Result<()> {
    require_widths(input.scalars.len(), input.identities.len())?;
    require_widths(scratch.scalars.len(), scratch.identities.len())?;
    require_widths(output.scalars.len(), output.identities.len())?;

    scratch.scalars.copy_from_slice(input.scalars);
    scratch.identities.copy_from_slice(input.identities);
    execute_candidate(scratch.scalars, scratch.identities)?;
    output.scalars.copy_from_slice(scratch.scalars);
    output.identities.copy_from_slice(scratch.identities);
    Ok(())
}

fn execute_candidate(scalars: &mut [u64], identities: &mut [[u8; 32]]) -> Result<()> {
    write(scalars, SCALAR_ZERO, 0)?;
    write(scalars, SCALAR_ONE, 1)?;
    write(scalars, SCALAR_FEE_DENOMINATOR, FEE_DENOMINATOR_V2)?;

    require(read(scalars, SCALAR_PHASE)? == read(scalars, SCALAR_ONE)?)?;
    require(read(scalars, SCALAR_FILL)? != 0)?;
    require(read(scalars, SCALAR_SELLER_FROM)? <= read(scalars, SCALAR_SLOT)?)?;
    require(read(scalars, SCALAR_SLOT)? <= read(scalars, SCALAR_SELLER_THROUGH)?)?;
    require(read(scalars, SCALAR_BUYER_FROM)? <= read(scalars, SCALAR_SLOT)?)?;
    require(read(scalars, SCALAR_SLOT)? <= read(scalars, SCALAR_BUYER_THROUGH)?)?;
    require(read(scalars, SCALAR_SELLER_SIDE)? == read(scalars, SCALAR_ZERO)?)?;
    require(read(scalars, SCALAR_BUYER_SIDE)? == read(scalars, SCALAR_ONE)?)?;
    require(
        identity(identities, IDENTITY_SELLER_MARKET)?
            == identity(identities, IDENTITY_BUYER_MARKET)?,
    )?;
    require(read(scalars, SCALAR_SELLER_GENERATION)? == read(scalars, SCALAR_BUYER_GENERATION)?)?;
    require(read(scalars, SCALAR_SELLER_OUTCOME)? == read(scalars, SCALAR_BUYER_OUTCOME)?)?;
    require(
        identity(identities, IDENTITY_SELLER_MAKER)? != identity(identities, IDENTITY_BUYER_MAKER)?,
    )?;
    require(read(scalars, SCALAR_SELLER_OUTCOME)? < read(scalars, SCALAR_OUTCOME_COUNT)?)?;

    lifecycle_accepts(
        read(scalars, SCALAR_SELLER_LIFECYCLE)?,
        read(scalars, SCALAR_SELLER_MAXIMUM)?,
        read(scalars, SCALAR_FILL)?,
    )?;
    lifecycle_accepts(
        read(scalars, SCALAR_BUYER_LIFECYCLE)?,
        read(scalars, SCALAR_BUYER_MAXIMUM)?,
        read(scalars, SCALAR_FILL)?,
    )?;
    require(read(scalars, SCALAR_SELLER_NONCE)? == read(scalars, SCALAR_SELLER_NEXT_NONCE)?)?;
    require(read(scalars, SCALAR_BUYER_NONCE)? == read(scalars, SCALAR_BUYER_NEXT_NONCE)?)?;
    let seller_nonce = read(scalars, SCALAR_SELLER_NEXT_NONCE)?
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    write(scalars, SCALAR_SELLER_NONCE_OUTPUT, seller_nonce)?;
    let buyer_nonce = read(scalars, SCALAR_BUYER_NEXT_NONCE)?
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    write(scalars, SCALAR_BUYER_NONCE_OUTPUT, buyer_nonce)?;

    require(read(scalars, SCALAR_SELLER_LIMIT)? <= read(scalars, SCALAR_EXECUTION_PRICE)?)?;
    require(read(scalars, SCALAR_EXECUTION_PRICE)? <= read(scalars, SCALAR_BUYER_LIMIT)?)?;
    require(read(scalars, SCALAR_EXECUTION_PRICE)? <= read(scalars, SCALAR_PRICE_SCALE)?)?;
    require(read(scalars, SCALAR_SELLER_FEE_BPS)? == read(scalars, SCALAR_POLICY_FEE_BPS)?)?;
    require(read(scalars, SCALAR_BUYER_FEE_BPS)? == read(scalars, SCALAR_POLICY_FEE_BPS)?)?;
    require(read(scalars, SCALAR_POLICY_FEE_BPS)? <= read(scalars, SCALAR_FEE_DENOMINATOR)?)?;

    let gross = mul_div_exact(
        read(scalars, SCALAR_FILL)?,
        read(scalars, SCALAR_EXECUTION_PRICE)?,
        read(scalars, SCALAR_PRICE_SCALE)?,
    )?;
    write(scalars, SCALAR_GROSS_OUTPUT, gross)?;
    let fee = mul_div_floor(
        gross,
        read(scalars, SCALAR_POLICY_FEE_BPS)?,
        read(scalars, SCALAR_FEE_DENOMINATOR)?,
    )?;
    write(scalars, SCALAR_FEE_OUTPUT, fee)?;

    require(read(scalars, SCALAR_FILL)? <= read(scalars, SCALAR_SELLER_CLAIMS)?)?;
    add_le(
        read(scalars, SCALAR_GROSS_OUTPUT)?,
        read(scalars, SCALAR_FEE_OUTPUT)?,
        read(scalars, SCALAR_BUYER_COLLATERAL)?,
    )?;
    add_fits(
        read(scalars, SCALAR_BUYER_CLAIMS)?,
        read(scalars, SCALAR_FILL)?,
    )?;
    add_fits(
        read(scalars, SCALAR_SELLER_COLLATERAL)?,
        read(scalars, SCALAR_GROSS_OUTPUT)?,
    )?;
    add_fits(
        read(scalars, SCALAR_VENUE_COLLATERAL)?,
        read(scalars, SCALAR_FEE_OUTPUT)?,
    )
}

fn require_widths(scalars: usize, identities: usize) -> Result<()> {
    if scalars == usize::from(DIRECT_PROGRAM_V2_SCALARS)
        && identities == usize::from(DIRECT_PROGRAM_V2_IDENTITIES)
    {
        Ok(())
    } else {
        Err(Error::RegisterWidthMismatch)
    }
}

fn read(scalars: &[u64], index: usize) -> Result<u64> {
    scalars
        .get(index)
        .copied()
        .ok_or(Error::RegisterWidthMismatch)
}

fn write(scalars: &mut [u64], index: usize, value: u64) -> Result<()> {
    *scalars.get_mut(index).ok_or(Error::RegisterWidthMismatch)? = value;
    Ok(())
}

fn identity(identities: &[[u8; 32]], index: usize) -> Result<[u8; 32]> {
    identities
        .get(index)
        .copied()
        .ok_or(Error::RegisterWidthMismatch)
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::CheckFailed)
    }
}

fn lifecycle_accepts(lifecycle: u64, maximum: u64, fill: u64) -> Result<()> {
    match lifecycle {
        0 => require(fill == maximum),
        1 | 2 => require(fill <= maximum),
        _ => Err(Error::UnknownLifecycle),
    }
}

fn mul_div_exact(left: u64, right: u64, denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::InexactDivision);
    }
    let numerator = u128::from(left) * u128::from(right);
    let denominator = u128::from(denominator);
    if numerator % denominator != 0 {
        return Err(Error::InexactDivision);
    }
    u64::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)
}

fn mul_div_floor(left: u64, right: u64, denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    let quotient = (u128::from(left) * u128::from(right)) / u128::from(denominator);
    u64::try_from(quotient).map_err(|_| Error::ArithmeticOverflow)
}

fn add_le(left: u64, right: u64, limit: u64) -> Result<()> {
    require(u128::from(left) + u128::from(right) <= u128::from(limit))
}

fn add_fits(left: u64, right: u64) -> Result<()> {
    left.checked_add(right)
        .map(|_| ())
        .ok_or(Error::ArithmeticOverflow)
}

#[cfg(test)]
mod tests;
