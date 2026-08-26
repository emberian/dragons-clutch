#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Stateless AOT translations of the current Direct TransitionVMV3 programs.
//!
//! This crate is not an independent execution authority. A production
//! CapabilityProgram selects it only through an exact translation certificate,
//! immutable Registry admission, checked executable release, and the same
//! authenticated input bank as the canonical TransitionVM program.

use dclutch_direct_codec::ordinary_v3::*;

/// Stable refusal from a current Direct AOT translation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input, scratch, or output banks had another exact Product-derived width.
    RegisterWidthMismatch,
    /// One canonical admission relation evaluated to false.
    CheckFailed,
    /// A signed lifecycle was outside FOK/IOC/GTC.
    UnknownLifecycle,
    /// Checked scalar arithmetic overflowed or underflowed.
    Arithmetic,
    /// Exact quote division had a zero denominator or nonzero remainder.
    InexactDivision,
    /// Floor division had a zero denominator.
    ZeroDenominator,
}

/// Result alias for the current Direct AOT translation.
pub type Result<T> = core::result::Result<T, Error>;

/// Immutable authenticated register bank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterInput<'a> {
    /// Common scalars followed by Product-item scalar strides.
    pub scalars: &'a [u64],
    /// Common identities followed by Product-item identity strides.
    pub identities: &'a [[u8; 32]],
}

/// Mutable caller-owned scratch or candidate bank.
pub struct RegisterOutput<'a> {
    /// Common scalars followed by Product-item scalar strides.
    pub scalars: &'a mut [u64],
    /// Common identities followed by Product-item identity strides.
    pub identities: &'a mut [[u8; 32]],
}

/// Execute the exact current InlineOrdinary TransitionVMV3 translation.
///
/// Refusal may alter scratch but leaves the caller's output unchanged.
pub fn execute_inline_ordinary_atomic(
    tail_count: u32,
    input: RegisterInput<'_>,
    scratch: RegisterOutput<'_>,
    output: RegisterOutput<'_>,
) -> Result<()> {
    let scalar_count = inline_scalar_count(tail_count)?;
    if input.scalars.len() != scalar_count
        || scratch.scalars.len() != scalar_count
        || output.scalars.len() != scalar_count
        || input.identities.len() != DIRECT_ORDINARY_COMMON_IDENTITIES_V3
        || scratch.identities.len() != DIRECT_ORDINARY_COMMON_IDENTITIES_V3
        || output.identities.len() != DIRECT_ORDINARY_COMMON_IDENTITIES_V3
    {
        return Err(Error::RegisterWidthMismatch);
    }
    scratch.scalars.copy_from_slice(input.scalars);
    scratch.identities.copy_from_slice(input.identities);
    execute_inline_candidate(tail_count, scratch.scalars, scratch.identities)?;
    output.scalars.copy_from_slice(scratch.scalars);
    output.identities.copy_from_slice(scratch.identities);
    Ok(())
}

fn execute_inline_candidate(
    tail_count: u32,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<()> {
    write(scalars, SCALAR_ZERO_V3, 0)?;
    write(scalars, SCALAR_ONE_V3, 1)?;
    write(
        scalars,
        SCALAR_FEE_DENOMINATOR_V3,
        u64::from(dclutch_direct_codec::successor::DIRECT_FEE_DENOMINATOR_V1),
    )?;
    require(read(scalars, SCALAR_ROOT_PHASE_V3)? == 0)?;
    require(read(scalars, SCALAR_FILL_V3)? != 0)?;
    require(read(scalars, SCALAR_SELLER_VALID_FROM_V3)? <= read(scalars, SCALAR_SLOT_V3)?)?;
    require(read(scalars, SCALAR_SLOT_V3)? <= read(scalars, SCALAR_SELLER_VALID_THROUGH_V3)?)?;
    require(read(scalars, SCALAR_BUYER_VALID_FROM_V3)? <= read(scalars, SCALAR_SLOT_V3)?)?;
    require(read(scalars, SCALAR_SLOT_V3)? <= read(scalars, SCALAR_BUYER_VALID_THROUGH_V3)?)?;
    require(read(scalars, SCALAR_SELLER_SIDE_V3)? == 0)?;
    require(read(scalars, SCALAR_BUYER_SIDE_V3)? == 1)?;
    require(
        identity(identities, IDENTITY_SELLER_INTENT_MARKET_V3)?
            == identity(identities, IDENTITY_BUYER_INTENT_MARKET_V3)?,
    )?;
    require(
        identity(identities, IDENTITY_SELLER_INTENT_MARKET_V3)?
            == identity(identities, IDENTITY_MARKET_V3)?,
    )?;
    require(
        read(scalars, SCALAR_SELLER_GENERATION_V3)? == read(scalars, SCALAR_BUYER_GENERATION_V3)?,
    )?;
    require(
        read(scalars, SCALAR_SELLER_GENERATION_V3)? == read(scalars, SCALAR_MARKET_GENERATION_V3)?,
    )?;
    require(read(scalars, SCALAR_SELLER_OUTCOME_V3)? == read(scalars, SCALAR_BUYER_OUTCOME_V3)?)?;
    require(
        identity(identities, IDENTITY_SELLER_NATIVE_SIGNER_V3)?
            == identity(identities, IDENTITY_SELLER_REQUEST_MAKER_V3)?,
    )?;
    require(
        identity(identities, IDENTITY_BUYER_NATIVE_SIGNER_V3)?
            == identity(identities, IDENTITY_BUYER_REQUEST_MAKER_V3)?,
    )?;
    require(
        identity(identities, IDENTITY_SELLER_REQUEST_MAKER_V3)?
            != identity(identities, IDENTITY_BUYER_REQUEST_MAKER_V3)?,
    )?;
    require(
        identity(identities, IDENTITY_SELLER_COLLATERAL_REQUEST_V3)?
            == identity(identities, IDENTITY_SELLER_TOKEN_ACCOUNT_V3)?,
    )?;
    require(
        identity(identities, IDENTITY_BUYER_COLLATERAL_REQUEST_V3)?
            == identity(identities, IDENTITY_BUYER_TOKEN_ACCOUNT_V3)?,
    )?;
    require(read(scalars, SCALAR_SELLER_OUTCOME_V3)? < read(scalars, SCALAR_OUTCOME_COUNT_V3)?)?;
    require(read(scalars, SCALAR_PRICE_SCALE_V3)? != 0)?;
    lifecycle_accepts(
        read(scalars, SCALAR_SELLER_LIFECYCLE_V3)?,
        read(scalars, SCALAR_SELLER_MAXIMUM_V3)?,
        read(scalars, SCALAR_FILL_V3)?,
    )?;
    lifecycle_accepts(
        read(scalars, SCALAR_BUYER_LIFECYCLE_V3)?,
        read(scalars, SCALAR_BUYER_MAXIMUM_V3)?,
        read(scalars, SCALAR_FILL_V3)?,
    )?;
    require(read(scalars, SCALAR_SELLER_NONCE_V3)? == read(scalars, SCALAR_SELLER_NEXT_NONCE_V3)?)?;
    require(read(scalars, SCALAR_BUYER_NONCE_V3)? == read(scalars, SCALAR_BUYER_NEXT_NONCE_V3)?)?;
    let seller_nonce = checked_add(read(scalars, SCALAR_SELLER_NEXT_NONCE_V3)?, 1)?;
    let buyer_nonce = checked_add(read(scalars, SCALAR_BUYER_NEXT_NONCE_V3)?, 1)?;
    write(scalars, SCALAR_SELLER_NONCE_AFTER_V3, seller_nonce)?;
    write(scalars, SCALAR_BUYER_NONCE_AFTER_V3, buyer_nonce)?;
    require(read(scalars, SCALAR_SELLER_LIMIT_V3)? <= read(scalars, SCALAR_EXECUTION_PRICE_V3)?)?;
    require(read(scalars, SCALAR_EXECUTION_PRICE_V3)? <= read(scalars, SCALAR_BUYER_LIMIT_V3)?)?;
    require(read(scalars, SCALAR_EXECUTION_PRICE_V3)? <= read(scalars, SCALAR_PRICE_SCALE_V3)?)?;
    require(read(scalars, SCALAR_SELLER_FEE_BPS_V3)? == read(scalars, SCALAR_POLICY_FEE_BPS_V3)?)?;
    require(read(scalars, SCALAR_BUYER_FEE_BPS_V3)? == read(scalars, SCALAR_POLICY_FEE_BPS_V3)?)?;
    let gross = mul_div_exact(
        read(scalars, SCALAR_FILL_V3)?,
        read(scalars, SCALAR_EXECUTION_PRICE_V3)?,
        read(scalars, SCALAR_PRICE_SCALE_V3)?,
    )?;
    let fee = mul_div_floor(
        gross,
        read(scalars, SCALAR_POLICY_FEE_BPS_V3)?,
        read(scalars, SCALAR_FEE_DENOMINATOR_V3)?,
    )?;
    let seller_net = checked_sub(gross, fee)?;
    let buyer_debit = checked_add(gross, fee)?;
    let combined_fee = checked_add(fee, fee)?;
    require(checked_add(seller_net, combined_fee)? == buyer_debit)?;
    write(scalars, SCALAR_GROSS_V3, gross)?;
    write(scalars, SCALAR_FEE_V3, fee)?;
    write(scalars, SCALAR_SELLER_NET_V3, seller_net)?;
    write(scalars, SCALAR_BUYER_DEBIT_V3, buyer_debit)?;
    write(scalars, SCALAR_COMBINED_FEE_V3, combined_fee)?;
    require(read(scalars, SCALAR_SELLER_CREATED_V3)? <= 1)?;
    require(read(scalars, SCALAR_BUYER_CREATED_V3)? <= 1)?;
    let root_after = checked_add(
        checked_add(
            read(scalars, SCALAR_ROOT_OPEN_COUNT_V3)?,
            read(scalars, SCALAR_SELLER_CREATED_V3)?,
        )?,
        read(scalars, SCALAR_BUYER_CREATED_V3)?,
    )?;
    write(scalars, SCALAR_ROOT_OPEN_COUNT_AFTER_V3, root_after)?;
    require(
        identity(identities, IDENTITY_SELLER_STATE_OWNER_V3)?
            == identity(identities, IDENTITY_TRADING_PROGRAM_V3)?,
    )?;
    require(
        identity(identities, IDENTITY_BUYER_STATE_OWNER_V3)?
            == identity(identities, IDENTITY_TRADING_PROGRAM_V3)?,
    )?;

    let seller_intermediate = u64::from(seller_net != 0 && combined_fee != 0);
    let fee_nonzero = u64::from(combined_fee != 0);
    let seller_terminal = u64::from(combined_fee == 0 && seller_net != 0);
    let fee_sole = u64::from(seller_net == 0 && combined_fee != 0);
    write(
        scalars,
        SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3,
        seller_intermediate,
    )?;
    write(scalars, SCALAR_FEE_NONZERO_V3, fee_nonzero)?;
    write(
        scalars,
        SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3,
        seller_terminal,
    )?;
    write(scalars, SCALAR_FEE_SOLE_ROUTE_ENABLED_V3, fee_sole)?;
    let custody_after_seller = checked_add(
        read(scalars, SCALAR_CUSTODY_REVISION_V3)?,
        checked_add(seller_terminal, seller_intermediate)?,
    )?;
    let custody_after_fee = checked_add(
        custody_after_seller,
        checked_add(seller_intermediate, fee_sole)?,
    )?;
    write(
        scalars,
        SCALAR_CUSTODY_AFTER_SELLER_V3,
        custody_after_seller,
    )?;
    write(scalars, SCALAR_CUSTODY_AFTER_FEE_V3, custody_after_fee)?;
    let claim_transfer = read(scalars, SCALAR_FILL_V3)?;
    write(scalars, SCALAR_CLAIM_TRANSFER_V3, claim_transfer)?;
    write(
        scalars,
        SCALAR_MAKER_VERSION_V3,
        u64::from(dclutch_direct_codec::successor::DirectMakerReplayLayoutV1::ABI_VERSION),
    )?;
    write(
        scalars,
        SCALAR_MAKER_MAGIC_V3,
        dclutch_direct_codec::successor::DirectMakerReplayLayoutV1::MAGIC_WORD,
    )?;

    let outcome =
        usize::try_from(read(scalars, SCALAR_SELLER_OUTCOME_V3)?).map_err(|_| Error::Arithmetic)?;
    let count = usize::try_from(tail_count).map_err(|_| Error::Arithmetic)?;
    if outcome >= count {
        return Err(Error::CheckFailed);
    }
    let mut item = 0_usize;
    while item < count {
        let base = item_scalar_base(item)?;
        let item_index_register = base
            .checked_add(usize::from(ITEM_SCALAR_INDEX_V3))
            .ok_or(Error::Arithmetic)?;
        let item_quantity_register = base
            .checked_add(usize::from(ITEM_SCALAR_CLAIM_QUANTITY_V3))
            .ok_or(Error::Arithmetic)?;
        let item_index = read(scalars, item_index_register)?;
        write(scalars, item_quantity_register, 0)?;
        if item_index == u64::try_from(outcome).map_err(|_| Error::Arithmetic)? {
            write(scalars, item_quantity_register, claim_transfer)?;
        }
        item = item.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    Ok(())
}

fn inline_scalar_count(tail_count: u32) -> Result<usize> {
    usize::try_from(tail_count)
        .map_err(|_| Error::RegisterWidthMismatch)?
        .checked_mul(usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3))
        .and_then(|tail| DIRECT_ORDINARY_COMMON_SCALARS_V3.checked_add(tail))
        .ok_or(Error::RegisterWidthMismatch)
}

fn item_scalar_base(item: usize) -> Result<usize> {
    item.checked_mul(usize::from(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3))
        .and_then(|tail| DIRECT_ORDINARY_COMMON_SCALARS_V3.checked_add(tail))
        .ok_or(Error::Arithmetic)
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

fn checked_add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right).ok_or(Error::Arithmetic)
}

fn checked_sub(left: u64, right: u64) -> Result<u64> {
    left.checked_sub(right).ok_or(Error::Arithmetic)
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
    u64::try_from(numerator / denominator).map_err(|_| Error::Arithmetic)
}

fn mul_div_floor(left: u64, right: u64, denominator: u64) -> Result<u64> {
    if denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    u64::try_from((u128::from(left) * u128::from(right)) / u128::from(denominator))
        .map_err(|_| Error::Arithmetic)
}

#[cfg(test)]
mod tests;
