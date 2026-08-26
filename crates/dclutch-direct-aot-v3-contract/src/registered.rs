//! Exact current registered ordinary fill translation.

use dclutch_direct_codec::registered_fill_artifacts_v4::*;

use crate::{
    Error, RegisterInput, RegisterOutput, Result, checked_add, checked_sub, identity,
    mul_div_exact, mul_div_floor, read, require, write,
};

/// Execute the exact current registered-ordinary-fill TransitionVMV3 translation.
///
/// `tail_count` remains authenticated Product context, but this transition has
/// zero per-item register strides. Refusal may alter scratch but leaves output
/// unchanged.
pub fn execute_registered_ordinary_fill_atomic(
    _tail_count: u32,
    input: RegisterInput<'_>,
    scratch: RegisterOutput<'_>,
    output: RegisterOutput<'_>,
) -> Result<()> {
    if input.scalars.len() != DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4
        || scratch.scalars.len() != DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4
        || output.scalars.len() != DIRECT_REGISTERED_FILL_COMMON_SCALARS_V4
        || input.identities.len() != DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4
        || scratch.identities.len() != DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4
        || output.identities.len() != DIRECT_REGISTERED_FILL_COMMON_IDENTITIES_V4
    {
        return Err(Error::RegisterWidthMismatch);
    }
    scratch.scalars.copy_from_slice(input.scalars);
    scratch.identities.copy_from_slice(input.identities);
    execute_candidate(scratch.scalars, scratch.identities)?;
    output.scalars.copy_from_slice(scratch.scalars);
    output.identities.copy_from_slice(scratch.identities);
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn execute_candidate(scalars: &mut [u64], identities: &mut [[u8; 32]]) -> Result<()> {
    write(scalars, FILL_SCALAR_ZERO_V4, 0)?;
    write(scalars, FILL_SCALAR_ONE_V4, 1)?;
    write(scalars, FILL_SCALAR_GTC_V4, 2)?;
    write(
        scalars,
        FILL_SCALAR_FEE_DENOMINATOR_V4,
        u64::from(dclutch_direct_codec::successor::DIRECT_FEE_DENOMINATOR_V1),
    )?;
    write(scalars, FILL_SCALAR_TERMINAL_V4, 1)?;

    require(read(scalars, FILL_SCALAR_ROOT_PHASE_V4)? == 0)?;
    require(read(scalars, FILL_SCALAR_ROOT_OPEN_COUNT_V4)? != 0)?;
    require(read(scalars, FILL_SCALAR_QUANTITY_V4)? != 0)?;
    require(
        read(scalars, FILL_SCALAR_EXECUTION_PRICE_V4)?
            <= read(scalars, FILL_SCALAR_PRICE_SCALE_V4)?,
    )?;
    require(
        identity(identities, FILL_IDENTITY_MARKET_V4)?
            == identity(identities, FILL_IDENTITY_SELLER_INTENT_MARKET_V4)?,
    )?;
    require(
        identity(identities, FILL_IDENTITY_MARKET_V4)?
            == identity(identities, FILL_IDENTITY_BUYER_INTENT_MARKET_V4)?,
    )?;
    require(
        identity(identities, FILL_IDENTITY_MARKET_V4)?
            == identity(identities, FILL_IDENTITY_SELLER_MAKER_MARKET_V4)?,
    )?;
    require(
        identity(identities, FILL_IDENTITY_MARKET_V4)?
            == identity(identities, FILL_IDENTITY_BUYER_MAKER_MARKET_V4)?,
    )?;
    require(
        identity(identities, FILL_IDENTITY_SELLER_MAKER_V4)?
            != identity(identities, FILL_IDENTITY_BUYER_MAKER_V4)?,
    )?;
    require(
        identity(identities, FILL_IDENTITY_SELLER_MAKER_V4)?
            == identity(identities, FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4)?,
    )?;
    require(
        identity(identities, FILL_IDENTITY_BUYER_MAKER_V4)?
            == identity(identities, FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_MARKET_GENERATION_V4)?
            == read(scalars, FILL_SCALAR_SELLER_GENERATION_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_MARKET_GENERATION_V4)?
            == read(scalars, FILL_SCALAR_BUYER_GENERATION_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_MARKET_GENERATION_V4)?
            == read(scalars, FILL_SCALAR_SELLER_MAKER_GENERATION_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_MARKET_GENERATION_V4)?
            == read(scalars, FILL_SCALAR_BUYER_MAKER_GENERATION_V4)?,
    )?;
    require(read(scalars, FILL_SCALAR_SELLER_SIDE_V4)? == 0)?;
    require(read(scalars, FILL_SCALAR_BUYER_SIDE_V4)? == 1)?;
    require(read(scalars, FILL_SCALAR_SELLER_LIFECYCLE_V4)? == 2)?;
    require(read(scalars, FILL_SCALAR_BUYER_LIFECYCLE_V4)? == 2)?;
    require(
        read(scalars, FILL_SCALAR_SELLER_OUTCOME_V4)?
            == read(scalars, FILL_SCALAR_BUYER_OUTCOME_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_OUTCOME_V4)?
            < read(scalars, FILL_SCALAR_OUTCOME_COUNT_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_FEE_BPS_V4)?
            == read(scalars, FILL_SCALAR_POLICY_FEE_BPS_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_FEE_BPS_V4)?
            == read(scalars, FILL_SCALAR_POLICY_FEE_BPS_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_VALID_FROM_V4)? <= read(scalars, FILL_SCALAR_SLOT_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SLOT_V4)? <= read(scalars, FILL_SCALAR_SELLER_VALID_THROUGH_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_VALID_FROM_V4)? <= read(scalars, FILL_SCALAR_SLOT_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SLOT_V4)? <= read(scalars, FILL_SCALAR_BUYER_VALID_THROUGH_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_LIMIT_V4)?
            <= read(scalars, FILL_SCALAR_EXECUTION_PRICE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_EXECUTION_PRICE_V4)?
            <= read(scalars, FILL_SCALAR_BUYER_LIMIT_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_LIMIT_V4)? <= read(scalars, FILL_SCALAR_PRICE_SCALE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_NONCE_V4)?
            < read(scalars, FILL_SCALAR_SELLER_NEXT_NONCE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_NONCE_V4)?
            < read(scalars, FILL_SCALAR_BUYER_NEXT_NONCE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_MINIMUM_NONCE_V4)?
            <= read(scalars, FILL_SCALAR_SELLER_NONCE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_MINIMUM_NONCE_V4)?
            <= read(scalars, FILL_SCALAR_BUYER_NONCE_V4)?,
    )?;
    require(read(scalars, FILL_SCALAR_SELLER_LIVE_COUNT_V4)? != 0)?;
    require(read(scalars, FILL_SCALAR_BUYER_LIVE_COUNT_V4)? != 0)?;
    require(
        read(scalars, FILL_SCALAR_SELLER_LIVE_COUNT_V4)?
            <= read(scalars, FILL_SCALAR_SELLER_NEXT_NONCE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_MINIMUM_NONCE_V4)?
            <= read(scalars, FILL_SCALAR_SELLER_NEXT_NONCE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_LIVE_COUNT_V4)?
            <= read(scalars, FILL_SCALAR_BUYER_NEXT_NONCE_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_MINIMUM_NONCE_V4)?
            <= read(scalars, FILL_SCALAR_BUYER_NEXT_NONCE_V4)?,
    )?;
    require(read(scalars, FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4)? != 0)?;
    require(read(scalars, FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4)? != 0)?;
    require(read(scalars, FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4)? != 0)?;
    require(read(scalars, FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4)? != 0)?;
    require(
        read(scalars, FILL_SCALAR_SELLER_FILLED_V4)?
            < read(scalars, FILL_SCALAR_SELLER_MAXIMUM_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_FILLED_V4)? < read(scalars, FILL_SCALAR_BUYER_MAXIMUM_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4)?
            <= read(scalars, FILL_SCALAR_SELLER_FILLED_V4)?,
    )?;
    require(
        read(scalars, FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?
            <= read(scalars, FILL_SCALAR_BUYER_FILLED_V4)?,
    )?;

    let denominator = read(scalars, FILL_SCALAR_FEE_DENOMINATOR_V4)?;
    let fee_bps = read(scalars, FILL_SCALAR_POLICY_FEE_BPS_V4)?;
    let seller_current_fee = mul_div_floor(
        read(scalars, FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4)?,
        fee_bps,
        denominator,
    )?;
    write(
        scalars,
        FILL_SCALAR_SELLER_CURRENT_FEE_CHECK_V4,
        seller_current_fee,
    )?;
    require(seller_current_fee == read(scalars, FILL_SCALAR_SELLER_CUMULATIVE_FEE_V4)?)?;
    let buyer_current_fee = mul_div_floor(
        read(scalars, FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?,
        fee_bps,
        denominator,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_CURRENT_FEE_CHECK_V4,
        buyer_current_fee,
    )?;
    require(buyer_current_fee == read(scalars, FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4)?)?;

    let seller_current_remaining = checked_sub(
        read(scalars, FILL_SCALAR_SELLER_MAXIMUM_V4)?,
        read(scalars, FILL_SCALAR_SELLER_FILLED_V4)?,
    )?;
    write(
        scalars,
        FILL_SCALAR_SELLER_CURRENT_REMAINING_V4,
        seller_current_remaining,
    )?;
    require(seller_current_remaining == read(scalars, FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4)?)?;
    require(read(scalars, FILL_SCALAR_SELLER_RESERVED_COLLATERAL_V4)? == 0)?;

    let buyer_initial_gross = mul_div_floor(
        read(scalars, FILL_SCALAR_BUYER_MAXIMUM_V4)?,
        read(scalars, FILL_SCALAR_BUYER_LIMIT_V4)?,
        read(scalars, FILL_SCALAR_PRICE_SCALE_V4)?,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_INITIAL_GROSS_V4,
        buyer_initial_gross,
    )?;
    let buyer_initial_fee = mul_div_floor(buyer_initial_gross, fee_bps, denominator)?;
    write(scalars, FILL_SCALAR_BUYER_INITIAL_FEE_V4, buyer_initial_fee)?;
    let buyer_initial_reserve = checked_add(buyer_initial_gross, buyer_initial_fee)?;
    write(
        scalars,
        FILL_SCALAR_BUYER_INITIAL_RESERVE_V4,
        buyer_initial_reserve,
    )?;
    let buyer_spent = checked_add(
        read(scalars, FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?,
        read(scalars, FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4)?,
    )?;
    write(scalars, FILL_SCALAR_BUYER_SPENT_V4, buyer_spent)?;
    let buyer_current_reserve = checked_sub(buyer_initial_reserve, buyer_spent)?;
    write(
        scalars,
        FILL_SCALAR_BUYER_CURRENT_RESERVE_CHECK_V4,
        buyer_current_reserve,
    )?;
    require(buyer_current_reserve == read(scalars, FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4)?)?;
    require(read(scalars, FILL_SCALAR_BUYER_RESERVED_CLAIMS_V4)? == 0)?;

    let seller_filled_after = checked_add(
        read(scalars, FILL_SCALAR_SELLER_FILLED_V4)?,
        read(scalars, FILL_SCALAR_QUANTITY_V4)?,
    )?;
    write(
        scalars,
        FILL_SCALAR_SELLER_FILLED_AFTER_V4,
        seller_filled_after,
    )?;
    require(seller_filled_after <= read(scalars, FILL_SCALAR_SELLER_MAXIMUM_V4)?)?;
    let buyer_filled_after = checked_add(
        read(scalars, FILL_SCALAR_BUYER_FILLED_V4)?,
        read(scalars, FILL_SCALAR_QUANTITY_V4)?,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_FILLED_AFTER_V4,
        buyer_filled_after,
    )?;
    require(buyer_filled_after <= read(scalars, FILL_SCALAR_BUYER_MAXIMUM_V4)?)?;
    let seller_remaining = checked_sub(
        read(scalars, FILL_SCALAR_SELLER_MAXIMUM_V4)?,
        seller_filled_after,
    )?;
    write(
        scalars,
        FILL_SCALAR_SELLER_REMAINING_AFTER_V4,
        seller_remaining,
    )?;
    let buyer_remaining = checked_sub(
        read(scalars, FILL_SCALAR_BUYER_MAXIMUM_V4)?,
        buyer_filled_after,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_REMAINING_AFTER_V4,
        buyer_remaining,
    )?;

    let gross = mul_div_exact(
        read(scalars, FILL_SCALAR_QUANTITY_V4)?,
        read(scalars, FILL_SCALAR_EXECUTION_PRICE_V4)?,
        read(scalars, FILL_SCALAR_PRICE_SCALE_V4)?,
    )?;
    write(scalars, FILL_SCALAR_GROSS_V4, gross)?;
    let seller_cumulative_gross_after = checked_add(
        read(scalars, FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4)?,
        gross,
    )?;
    let buyer_cumulative_gross_after =
        checked_add(read(scalars, FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4)?, gross)?;
    write(
        scalars,
        FILL_SCALAR_SELLER_CUMULATIVE_GROSS_AFTER_V4,
        seller_cumulative_gross_after,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_CUMULATIVE_GROSS_AFTER_V4,
        buyer_cumulative_gross_after,
    )?;
    require(seller_cumulative_gross_after <= seller_filled_after)?;
    require(buyer_cumulative_gross_after <= buyer_filled_after)?;
    let seller_cumulative_fee_after =
        mul_div_floor(seller_cumulative_gross_after, fee_bps, denominator)?;
    let buyer_cumulative_fee_after =
        mul_div_floor(buyer_cumulative_gross_after, fee_bps, denominator)?;
    write(
        scalars,
        FILL_SCALAR_SELLER_CUMULATIVE_FEE_AFTER_V4,
        seller_cumulative_fee_after,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_CUMULATIVE_FEE_AFTER_V4,
        buyer_cumulative_fee_after,
    )?;
    let seller_fee_delta = checked_sub(
        seller_cumulative_fee_after,
        read(scalars, FILL_SCALAR_SELLER_CUMULATIVE_FEE_V4)?,
    )?;
    let buyer_fee_delta = checked_sub(
        buyer_cumulative_fee_after,
        read(scalars, FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4)?,
    )?;
    write(scalars, FILL_SCALAR_SELLER_FEE_DELTA_V4, seller_fee_delta)?;
    write(scalars, FILL_SCALAR_BUYER_FEE_DELTA_V4, buyer_fee_delta)?;
    let seller_net = checked_sub(gross, seller_fee_delta)?;
    let buyer_debit = checked_add(gross, buyer_fee_delta)?;
    let total_fee = checked_add(seller_fee_delta, buyer_fee_delta)?;
    let conservation = checked_add(seller_net, total_fee)?;
    write(scalars, FILL_SCALAR_SELLER_NET_V4, seller_net)?;
    write(scalars, FILL_SCALAR_BUYER_DEBIT_V4, buyer_debit)?;
    write(scalars, FILL_SCALAR_TOTAL_FEE_V4, total_fee)?;
    write(scalars, FILL_SCALAR_CONSERVATION_V4, conservation)?;
    require(conservation == buyer_debit)?;
    let seller_reserve_after = checked_sub(
        read(scalars, FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4)?,
        read(scalars, FILL_SCALAR_QUANTITY_V4)?,
    )?;
    let buyer_reserve_after = checked_sub(
        read(scalars, FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4)?,
        buyer_debit,
    )?;
    write(
        scalars,
        FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4,
        seller_reserve_after,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_RESERVED_COLLATERAL_AFTER_V4,
        buyer_reserve_after,
    )?;
    let seller_terminal = u64::from(seller_remaining == 0);
    let buyer_terminal = u64::from(buyer_remaining == 0);
    write(scalars, FILL_SCALAR_SELLER_TERMINAL_V4, seller_terminal)?;
    write(scalars, FILL_SCALAR_BUYER_TERMINAL_V4, buyer_terminal)?;
    write(
        scalars,
        FILL_SCALAR_SELLER_LIVE_COUNT_AFTER_V4,
        checked_sub(
            read(scalars, FILL_SCALAR_SELLER_LIVE_COUNT_V4)?,
            seller_terminal,
        )?,
    )?;
    write(
        scalars,
        FILL_SCALAR_BUYER_LIVE_COUNT_AFTER_V4,
        checked_sub(
            read(scalars, FILL_SCALAR_BUYER_LIVE_COUNT_V4)?,
            buyer_terminal,
        )?,
    )?;
    write(
        scalars,
        FILL_SCALAR_CLAIM_SOURCE_REVISION_AFTER_V4,
        checked_add(read(scalars, FILL_SCALAR_CLAIM_SOURCE_REVISION_V4)?, 1)?,
    )?;
    write(
        scalars,
        FILL_SCALAR_CLAIM_DESTINATION_REVISION_AFTER_V4,
        checked_add(read(scalars, FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4)?, 1)?,
    )?;
    let custody_after_seller = checked_add(read(scalars, FILL_SCALAR_CUSTODY_REVISION_V4)?, 1)?;
    write(
        scalars,
        FILL_SCALAR_CUSTODY_REVISION_AFTER_SELLER_V4,
        custody_after_seller,
    )?;
    write(
        scalars,
        FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4,
        checked_add(custody_after_seller, 1)?,
    )?;
    Ok(())
}
