//! Optional Token-2022 bearer-claim capability runtime.
//!
//! This module is deliberately the only SBF boundary for the bearer contract.
//! It never derives an economic result itself: hostile account bytes become the
//! contract's exact observations, the contract produces an atomic plan, and
//! every CPI is checked against that plan before persistent state is written.

use alloc::vec::Vec;

use dclutch_bearer_contract::{
    frame::{AccountMetaV1, validate_account_frame},
    instruction::InstructionV1,
    state::{
        BEARER_MINT_BYTES, BEARER_TOKEN_ACCOUNT_BYTES, BearerCapabilityV1, MintObservationV1,
        TokenAccountObservationV1, TokenAccountStateV1,
    },
};
use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, FUNDING_STATE_BYTES,
    FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_collateral_contract::{
    COLLATERAL_CUSTODY_PDA_DOMAIN, COLLATERAL_VAULT_PDA_DOMAIN, CollateralCustodyV1,
};
use dclutch_core_contract::ContentId;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{PositionV1, REALM_PDA_DOMAIN, RealmV1};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
    SourceCloseCreditPlanV1,
};
use dclutch_token_svm::{
    CollateralAdapterReleaseV1, ExactTransferInput, Mint, TokenAccount, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;
use spl_token_2022_interface::extension::ExtensionType;

use crate::{
    AdapterError,
    realm::{
        recognized_program_loader, require_authority_policy, require_freeze_policy,
        select_adapter_release,
    },
};

/// Route one exact bearer instruction.  The family dispatcher checks the
/// magic before entering this routine; this routine still decodes it exactly
/// so a direct caller cannot obtain a compatibility path.
pub(crate) fn dispatch<'a>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let instruction =
        InstructionV1::decode(instruction_data).map_err(|_| AdapterError::InvalidInstruction)?;
    let frame = account_frame(accounts)?;
    match instruction.outcome_count() {
        2 => process::<2>(program_id, accounts, frame.as_slice(), instruction),
        3 => process::<3>(program_id, accounts, frame.as_slice(), instruction),
        4 => process::<4>(program_id, accounts, frame.as_slice(), instruction),
        5 => process::<5>(program_id, accounts, frame.as_slice(), instruction),
        6 => process::<6>(program_id, accounts, frame.as_slice(), instruction),
        7 => process::<7>(program_id, accounts, frame.as_slice(), instruction),
        8 => process::<8>(program_id, accounts, frame.as_slice(), instruction),
        9 => process::<9>(program_id, accounts, frame.as_slice(), instruction),
        10 => process::<10>(program_id, accounts, frame.as_slice(), instruction),
        11 => process::<11>(program_id, accounts, frame.as_slice(), instruction),
        12 => process::<12>(program_id, accounts, frame.as_slice(), instruction),
        13 => process::<13>(program_id, accounts, frame.as_slice(), instruction),
        14 => process::<14>(program_id, accounts, frame.as_slice(), instruction),
        15 => process::<15>(program_id, accounts, frame.as_slice(), instruction),
        16 => process::<16>(program_id, accounts, frame.as_slice(), instruction),
        _ => Err(AdapterError::BearerAuthentication.into()),
    }
}

fn account_frame(accounts: &[AccountInfo<'_>]) -> Result<Vec<AccountMetaV1>, ProgramError> {
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for account in accounts {
        frame.push(AccountMetaV1 {
            key: account.key.to_bytes(),
            is_signer: account.is_signer,
            is_writable: account.is_writable,
            is_executable: account.executable,
        });
    }
    Ok(frame)
}

fn process<'a, const N: usize>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    frame: &[AccountMetaV1],
    instruction: InstructionV1,
) -> Result<(), ProgramError> {
    validate_account_frame::<N>(instruction.action(), frame)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    match instruction {
        InstructionV1::Activate {
            generation,
            expected_prior_child_count,
            ..
        } => activate::<N>(program_id, accounts, generation, expected_prior_child_count),
        InstructionV1::Audit { generation, .. } => audit::<N>(program_id, accounts, generation),
        InstructionV1::Set {
            action: dclutch_bearer_contract::instruction::ActionV1::SplitNative,
            generation,
            quantity,
            ..
        } => split_native::<N>(program_id, accounts, generation, quantity),
        InstructionV1::Set {
            action: dclutch_bearer_contract::instruction::ActionV1::MergeNative,
            generation,
            quantity,
            ..
        } => merge_native::<N>(program_id, accounts, generation, quantity),
        InstructionV1::Outcome {
            action: dclutch_bearer_contract::instruction::ActionV1::Materialize,
            generation,
            quantity,
            outcome,
            ..
        } => materialize::<N>(
            program_id,
            accounts,
            generation,
            usize::from(outcome),
            quantity,
        ),
        InstructionV1::Outcome {
            action: dclutch_bearer_contract::instruction::ActionV1::Dematerialize,
            generation,
            quantity,
            outcome,
            ..
        } => dematerialize::<N>(
            program_id,
            accounts,
            generation,
            usize::from(outcome),
            quantity,
        ),
        InstructionV1::Outcome {
            action: dclutch_bearer_contract::instruction::ActionV1::Transfer,
            generation,
            quantity,
            outcome,
            ..
        } => transfer::<N>(
            program_id,
            accounts,
            generation,
            usize::from(outcome),
            quantity,
        ),
        InstructionV1::Set {
            action: dclutch_bearer_contract::instruction::ActionV1::SplitBearer,
            generation,
            quantity,
            ..
        } => split_bearer::<N>(program_id, accounts, generation, quantity),
        InstructionV1::Set {
            action: dclutch_bearer_contract::instruction::ActionV1::MergeBearer,
            generation,
            quantity,
            ..
        } => merge_bearer::<N>(program_id, accounts, generation, quantity),
        InstructionV1::Outcome {
            action: dclutch_bearer_contract::instruction::ActionV1::RedeemNative,
            generation,
            quantity,
            outcome,
            ..
        } => redeem_native::<N>(
            program_id,
            accounts,
            generation,
            usize::from(outcome),
            quantity,
        ),
        InstructionV1::Outcome {
            action: dclutch_bearer_contract::instruction::ActionV1::RedeemBearer,
            generation,
            quantity,
            outcome,
            ..
        } => redeem_bearer::<N>(
            program_id,
            accounts,
            generation,
            usize::from(outcome),
            quantity,
        ),
        InstructionV1::Retire {
            generation,
            expected_prior_child_count,
            ..
        } => retire::<N>(program_id, accounts, generation, expected_prior_child_count),
        // No fallback or compatibility decoder exists: an action reaches this
        // arm only when its exact fixed-layout route has been selected.
        _ => Err(AdapterError::BearerTransition.into()),
    }
}

fn activate<'a, const N: usize>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'a>],
    generation: u64,
    expected_prior_child_count: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let manifest_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let funding_account = account(accounts, 4)?;
    let refund_account = account(accounts, 5)?;
    let payer = account(accounts, 6)?;
    let token_program = account(accounts, 7)?;
    let system = account(accounts, 8)?;
    let rent_sysvar = account(accounts, 9)?;
    authenticate_system_and_rent(system, rent_sysvar)?;
    if !payer.is_signer || payer.owner != &system_program::ID || !payer.data_is_empty() {
        return Err(AdapterError::BearerAuthentication.into());
    }
    require_vacant(state_account)?;
    for index in 0..N {
        require_vacant(account(accounts, 10 + index)?)?;
    }
    if token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let mut market = decode_market::<N>(program_id, market_account, generation)?;
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let manifest_id = ContentId::new(hash(&manifest_data).to_bytes())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let (config_id, config) = decode_config(config_account)?;
    let refund = authenticate_rent_credit(
        program_id,
        refund_account,
        &Pubkey::new_from_array(config.rent_refund()),
    )?;
    let funding = decode_funding(
        program_id,
        funding_account,
        market_account,
        generation,
        manifest_id,
        manifest,
    )?;
    let rent =
        Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::BearerAuthentication)?;
    let funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let custody =
        FundingCustodyObservationV1::native_only(funding_account.lamports(), funding_rent)
            .map_err(|_| AdapterError::BearerAuthentication)?;
    let state_rent = rent.minimum_balance(
        BearerCapabilityV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?,
    );
    let mint_rent = rent.minimum_balance(BEARER_MINT_BYTES);
    let mint_total = mint_rent
        .checked_mul(u64::try_from(N).map_err(|_| AdapterError::Arithmetic)?)
        .ok_or(AdapterError::Arithmetic)?;
    let controller = state_account.key.to_bytes();
    let mut mint_keys = [[0u8; 32]; N];
    for index in 0..N {
        mint_keys[index] = canonical_mint(program_id, market_account.key, generation, index)?;
        if account(accounts, 10 + index)?.key.to_bytes() != mint_keys[index] {
            return Err(AdapterError::BearerAuthentication.into());
        }
    }
    let now = Clock::get()
        .map_err(|_| AdapterError::BearerAuthentication)?
        .slot;
    let mut funding_after = funding;
    let (state, _plan) = dclutch_bearer_contract::transition::activate(
        market_account.key.to_bytes(),
        &mut market,
        manifest_id,
        manifest,
        config_id,
        config,
        &mut funding_after,
        custody,
        now,
        state_rent,
        mint_total,
        expected_prior_child_count,
        controller,
        mint_keys,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    let total_debit = state_rent
        .checked_add(mint_total)
        .ok_or(AdapterError::Arithmetic)?;
    let payer_before = payer.lamports();
    payer_before
        .checked_sub(total_debit)
        .ok_or(AdapterError::BearerCreateCpi)?;
    let funding_after_lamports = funding_account
        .lamports()
        .checked_sub(total_debit)
        .ok_or(AdapterError::BearerAuthentication)?;
    if funding_after_lamports != funding_rent {
        return Err(AdapterError::BearerPostcondition.into());
    }
    let generation_bytes = generation.to_le_bytes();
    let state_seeds = [
        dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
        market_account.key.as_ref(),
        generation_bytes.as_slice(),
    ];
    let (_, state_bump) = Pubkey::find_program_address(&state_seeds, program_id);
    if state_account.key
        != &Pubkey::create_program_address(
            &[
                state_seeds[0],
                state_seeds[1],
                state_seeds[2],
                &[state_bump],
            ],
            program_id,
        )
        .map_err(|_| AdapterError::BearerAuthentication)?
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let state_len = BearerCapabilityV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    create_pda_account(
        program_id,
        payer,
        state_account,
        system,
        state_rent,
        state_len,
        program_id,
        &state_seeds,
        state_bump,
    )?;
    for index in 0..N {
        let mint = account(accounts, 10 + index)?;
        let outcome = [u8::try_from(index).map_err(|_| AdapterError::Arithmetic)?];
        let mint_seeds = [
            dclutch_bearer_contract::state::BEARER_MINT_PDA_DOMAIN,
            market_account.key.as_ref(),
            generation_bytes.as_slice(),
            outcome.as_slice(),
        ];
        let (_, bump) = Pubkey::find_program_address(&mint_seeds, program_id);
        create_pda_account(
            program_id,
            payer,
            mint,
            system,
            mint_rent,
            BEARER_MINT_BYTES,
            token_program.key,
            &mint_seeds,
            bump,
        )?;
        initialize_bearer_mint(mint, state_account, token_program)?;
        if parse_mint(mint, token_program)?
            .validate_profile(mint_keys[index], controller)
            .is_err()
        {
            return Err(AdapterError::BearerPostcondition.into());
        }
    }
    // All fallible account, plan, and CPI checks are complete before the three
    // persistent writes.  The payer is reimbursed from segregated funding only.
    move_lamports_exact(funding_account, payer, total_debit)?;
    persist_market(market_account, market)?;
    persist_funding(funding_account, funding_after)?;
    persist_state(state_account, state)?;
    require_unchanged_rent_credit(program_id, refund_account, refund)?;
    if payer.lamports() != payer_before || funding_account.lamports() != funding_after_lamports {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn retire<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    expected_prior_child_count: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let manifest_account = account(accounts, 2)?;
    let config_account = account(accounts, 3)?;
    let refund = account(accounts, 4)?;
    let token_program = account(accounts, 5)?;
    let system = account(accounts, 6)?;
    let rent = account(accounts, 7)?;
    if token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID {
        return Err(AdapterError::BearerAuthentication.into());
    }
    authenticate_system_and_rent(system, rent)?;
    let mut market = decode_market::<N>(program_id, market_account, generation)?;
    let state = decode_state::<N>(program_id, state_account, market_account, generation)?;
    if manifest_account.owner != program_id || config_account.owner != program_id {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let manifest_id = ContentId::new(hash(&manifest_data).to_bytes())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let config_data = config_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let config = dclutch_bearer_contract::state::BearerConfigV1::decode(&config_data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let config_id = ContentId::new(hash(&config_data).to_bytes())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let refund_state = authenticate_rent_credit(
        program_id,
        refund,
        &Pubkey::new_from_array(config.rent_refund()),
    )?;
    let mut expected_mints = [[0u8; 32]; N];
    let mut observations = [empty_mint(); N];
    let refund_before = refund.lamports();
    let mut mint_lamports = 0u64;
    for index in 0..N {
        let mint = account(accounts, 8 + index)?;
        expected_mints[index] = canonical_mint(program_id, market_account.key, generation, index)?;
        observations[index] = parse_mint(mint, token_program)?;
        mint_lamports = mint_lamports
            .checked_add(mint.lamports())
            .ok_or(AdapterError::Arithmetic)?;
    }
    let plan = dclutch_bearer_contract::transition::retire(
        market_account.key.to_bytes(),
        &mut market,
        state,
        manifest_id,
        manifest,
        config_id,
        config,
        expected_prior_child_count,
        state_account.key.to_bytes(),
        expected_mints,
        observations,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    if plan.rent_refund != refund.key.to_bytes() {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let generation_bytes = generation.to_le_bytes();
    let (_, bump) = Pubkey::find_program_address(
        &[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market_account.key.as_ref(),
            &generation_bytes,
        ],
        program_id,
    );
    for index in 0..N {
        let mint = account(accounts, 8 + index)?;
        let close = spl_token_2022_interface::instruction::close_account(
            token_program.key,
            mint.key,
            refund.key,
            state_account.key,
            &[],
        )?;
        invoke_signed(
            &close,
            &[
                mint.clone(),
                refund.clone(),
                state_account.clone(),
                token_program.clone(),
            ],
            &[&[
                dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
                market_account.key.as_ref(),
                &generation_bytes,
                &[bump],
            ]],
        )
        .map_err(|_| AdapterError::BearerTokenCpi)?;
        if mint.lamports() != 0 || !mint.data_is_empty() {
            return Err(AdapterError::BearerClose.into());
        }
    }
    let state_lamports = state_account.lamports();
    let refund_after = refund_before
        .checked_add(mint_lamports)
        .and_then(|value| value.checked_add(state_lamports))
        .ok_or(AdapterError::Arithmetic)?;
    let close = SourceCloseCreditPlanV1::new(state_lamports, refund.lamports(), state_lamports)
        .map_err(|_| AdapterError::Arithmetic)?;
    // Every fallible close/authentication operation precedes these final
    // writes, so a refusal leaves Market/state/rent balances unchanged.
    persist_market(market_account, market)?;
    move_lamports_exact(state_account, refund, state_lamports)?;
    state_account
        .resize(0)
        .map_err(|_| AdapterError::BearerClose)?;
    state_account.assign(&system_program::ID);
    close
        .validate_post(state_account.lamports(), refund.lamports())
        .map_err(|_| AdapterError::BearerClose)?;
    require_unchanged_rent_credit(program_id, refund, refund_state)?;
    if state_account.lamports() != 0
        || !state_account.data_is_empty()
        || state_account.owner != &system_program::ID
        || plan.market_child_count_after.checked_add(1) != Some(plan.market_child_count_before)
        || refund.lamports() != refund_after
    {
        return Err(AdapterError::BearerClose.into());
    }
    Ok(())
}

fn split_native<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    quantity: u64,
) -> Result<(), ProgramError> {
    native_value::<N>(
        program_id,
        accounts,
        generation,
        quantity,
        NativeAction::Split,
    )
}

fn merge_native<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    quantity: u64,
) -> Result<(), ProgramError> {
    native_value::<N>(
        program_id,
        accounts,
        generation,
        quantity,
        NativeAction::Merge,
    )
}

fn redeem_native<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    outcome: usize,
    quantity: u64,
) -> Result<(), ProgramError> {
    native_value::<N>(
        program_id,
        accounts,
        generation,
        quantity,
        NativeAction::Redeem { outcome },
    )
}

#[derive(Clone, Copy)]
enum NativeAction {
    Split,
    Merge,
    Redeem { outcome: usize },
}

fn native_value<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    quantity: u64,
    action: NativeAction,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let position_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let custody_account = account(accounts, 3)?;
    let vault = account(accounts, 4)?;
    let collateral_account = account(accounts, 5)?;
    let holder = account(accounts, 6)?;
    let token_program = account(accounts, 7)?;
    let mint = account(accounts, 8)?;
    if !holder.is_signer {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let mut market = decode_market::<N>(program_id, market_account, generation)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        mint,
        token_program,
        market.root(),
    )?;
    authenticate_custody(program_id, custody_account, market_account, generation)?;
    let vault_before = authenticate_vault(
        program_id,
        vault,
        market_account,
        mint,
        token_program,
        realm,
    )?;
    let mut position = decode_position::<N>(
        program_id,
        position_account,
        market_account,
        holder,
        generation,
    )?;
    let plan = match action {
        NativeAction::Split => dclutch_bearer_contract::transition::split_to_position(
            market_account.key.to_bytes(),
            &mut market,
            &mut position,
            holder.key.to_bytes(),
            realm.binding()?,
            quantity,
        ),
        NativeAction::Merge => dclutch_bearer_contract::transition::merge_from_position(
            market_account.key.to_bytes(),
            &mut market,
            &mut position,
            holder.key.to_bytes(),
            realm.binding()?,
            quantity,
        ),
        NativeAction::Redeem { outcome } => dclutch_bearer_contract::transition::redeem_native(
            market_account.key.to_bytes(),
            &mut market,
            &mut position,
            holder.key.to_bytes(),
            realm.binding()?,
            outcome,
            quantity,
        )
        .map(|plan| plan.payout),
    }
    .map_err(|_| AdapterError::BearerTransition)?;
    let (source, destination) = match plan.direction() {
        dclutch_bearer_contract::transition::CollateralDirectionV1::DepositToHoard => {
            (collateral_account, vault)
        }
        dclutch_bearer_contract::transition::CollateralDirectionV1::WithdrawFromHoard => {
            (vault, collateral_account)
        }
    };
    execute_collateral_transfer(
        program_id,
        source,
        destination,
        mint,
        token_program,
        realm,
        holder,
        plan.amount,
        matches!(
            plan.direction(),
            dclutch_bearer_contract::transition::CollateralDirectionV1::WithdrawFromHoard
        )
        .then(|| {
            Ok((
                market_account,
                market_signer(program_id, market_account, market.root())?,
            ))
        })
        .transpose()?,
    )?;
    if authenticate_vault(
        program_id,
        vault,
        market_account,
        mint,
        token_program,
        realm,
    )?
    .amount
        != expected_vault_amount(vault_before.amount, plan.amount(), plan.direction())?
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    persist_market(market_account, market)?;
    persist_position(position_account, position)
}

fn split_bearer<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    quantity: u64,
) -> Result<(), ProgramError> {
    bearer_complete_set::<N>(program_id, accounts, generation, quantity, true)
}

fn merge_bearer<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    quantity: u64,
) -> Result<(), ProgramError> {
    bearer_complete_set::<N>(program_id, accounts, generation, quantity, false)
}

fn bearer_complete_set<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    quantity: u64,
    split: bool,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let custody_account = account(accounts, 3)?;
    let vault = account(accounts, 4)?;
    let collateral_account = account(accounts, 5)?;
    let holder = account(accounts, 6)?;
    let collateral_program = account(accounts, 7)?;
    let collateral_mint = account(accounts, 8)?;
    let token_program = account(accounts, 9)?;
    if !holder.is_signer || token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let mut market = decode_market::<N>(program_id, market_account, generation)?;
    let mut state = decode_state::<N>(program_id, state_account, market_account, generation)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        collateral_mint,
        collateral_program,
        market.root(),
    )?;
    authenticate_custody(program_id, custody_account, market_account, generation)?;
    let vault_before = authenticate_vault(
        program_id,
        vault,
        market_account,
        collateral_mint,
        collateral_program,
        realm,
    )?;
    let mut mint_keys = [[0; 32]; N];
    let mut mints = [empty_mint(); N];
    let mut tokens = [empty_claim(); N];
    for index in 0..N {
        let mint = account(accounts, 10 + 2 * index)?;
        let token = account(accounts, 11 + 2 * index)?;
        mint_keys[index] = canonical_mint(program_id, market_account.key, generation, index)?;
        mints[index] = parse_mint(mint, token_program)?;
        tokens[index] = parse_claim_account(token, token_program)?;
    }
    let (collateral, plans) = if split {
        dclutch_bearer_contract::transition::split_to_bearer(
            market_account.key.to_bytes(),
            &mut market,
            &mut state,
            holder.key.to_bytes(),
            realm.binding()?,
            quantity,
            state_account.key.to_bytes(),
            mint_keys,
            mints,
            tokens,
        )
    } else {
        dclutch_bearer_contract::transition::merge_from_bearer(
            market_account.key.to_bytes(),
            &mut market,
            &mut state,
            holder.key.to_bytes(),
            realm.binding()?,
            quantity,
            state_account.key.to_bytes(),
            mint_keys,
            mints,
            tokens,
        )
    }
    .map_err(|_| AdapterError::BearerTransition)?;
    let (source, destination) = if split {
        (collateral_account, vault)
    } else {
        (vault, collateral_account)
    };
    execute_collateral_transfer(
        program_id,
        source,
        destination,
        collateral_mint,
        collateral_program,
        realm,
        holder,
        collateral.amount(),
        (!split)
            .then(|| {
                Ok((
                    market_account,
                    market_signer(program_id, market_account, market.root())?,
                ))
            })
            .transpose()?,
    )?;
    for index in 0..N {
        let mint = account(accounts, 10 + 2 * index)?;
        let token = account(accounts, 11 + 2 * index)?;
        if split {
            mint_to_plan(
                program_id,
                market_account,
                generation,
                state_account,
                mint,
                token,
                token_program,
                plans[index],
            )?;
        } else {
            burn_plan(
                program_id,
                market_account,
                generation,
                state_account,
                mint,
                token,
                holder,
                token_program,
                plans[index],
            )?;
        }
    }
    if authenticate_vault(
        program_id,
        vault,
        market_account,
        collateral_mint,
        collateral_program,
        realm,
    )?
    .amount
        != expected_vault_amount(
            vault_before.amount,
            collateral.amount(),
            collateral.direction(),
        )?
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    persist_market(market_account, market)?;
    persist_state(state_account, state)
}

fn redeem_bearer<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    outcome: usize,
    quantity: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let realm_account = account(accounts, 2)?;
    let custody = account(accounts, 3)?;
    let vault = account(accounts, 4)?;
    let destination = account(accounts, 5)?;
    let mint_account = account(accounts, 6)?;
    let claim = account(accounts, 7)?;
    let holder = account(accounts, 8)?;
    let collateral_program = account(accounts, 9)?;
    let collateral_mint = account(accounts, 10)?;
    let token_program = account(accounts, 11)?;
    if !holder.is_signer || token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let mut market = decode_market::<N>(program_id, market_account, generation)?;
    let mut state = decode_state::<N>(program_id, state_account, market_account, generation)?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        collateral_mint,
        collateral_program,
        market.root(),
    )?;
    authenticate_custody(program_id, custody, market_account, generation)?;
    let vault_before = authenticate_vault(
        program_id,
        vault,
        market_account,
        collateral_mint,
        collateral_program,
        realm,
    )?;
    let expected = canonical_mint(program_id, market_account.key, generation, outcome)?;
    let plan = dclutch_bearer_contract::transition::redeem_bearer(
        market_account.key.to_bytes(),
        &mut market,
        &mut state,
        holder.key.to_bytes(),
        realm.binding()?,
        outcome,
        quantity,
        state_account.key.to_bytes(),
        expected,
        parse_mint(mint_account, token_program)?,
        parse_claim_account(claim, token_program)?,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    let burn = plan.bearer_burn.ok_or(AdapterError::BearerPostcondition)?;
    burn_plan(
        program_id,
        market_account,
        generation,
        state_account,
        mint_account,
        claim,
        holder,
        token_program,
        burn,
    )?;
    execute_collateral_transfer(
        program_id,
        vault,
        destination,
        collateral_mint,
        collateral_program,
        realm,
        holder,
        plan.payout.amount(),
        Some((
            market_account,
            market_signer(program_id, market_account, market.root())?,
        )),
    )?;
    if authenticate_vault(
        program_id,
        vault,
        market_account,
        collateral_mint,
        collateral_program,
        realm,
    )?
    .amount
        != expected_vault_amount(
            vault_before.amount,
            plan.payout.amount(),
            plan.payout.direction(),
        )?
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    persist_market(market_account, market)?;
    persist_state(state_account, state)
}

fn transfer<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    outcome: usize,
    quantity: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let mint_account = account(accounts, 2)?;
    let source_account = account(accounts, 3)?;
    let destination_account = account(accounts, 4)?;
    let holder = account(accounts, 5)?;
    let token_program = account(accounts, 6)?;
    if !holder.is_signer || token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let market = decode_market::<N>(program_id, market_account, generation)?;
    let state = decode_state::<N>(program_id, state_account, market_account, generation)?;
    let mint = parse_mint(mint_account, token_program)?;
    let source = parse_claim_account(source_account, token_program)?;
    let destination = parse_claim_account(destination_account, token_program)?;
    let expected = canonical_mint(program_id, market_account.key, generation, outcome)?;
    let plan = dclutch_bearer_contract::transition::transfer(
        market_account.key.to_bytes(),
        &market,
        &state,
        outcome,
        quantity,
        state_account.key.to_bytes(),
        holder.key.to_bytes(),
        expected,
        mint,
        source,
        destination,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    let instruction = spl_token_2022_interface::instruction::transfer_checked(
        token_program.key,
        source_account.key,
        mint_account.key,
        destination_account.key,
        holder.key,
        &[],
        plan.amount,
        0,
    )?;
    invoke_signed(
        &instruction,
        &[
            source_account.clone(),
            mint_account.clone(),
            destination_account.clone(),
            holder.clone(),
            token_program.clone(),
        ],
        &[],
    )
    .map_err(|_| AdapterError::BearerTokenCpi)?;
    if parse_mint(mint_account, token_program)?.supply != plan.unchanged_mint_supply
        || parse_claim_account(source_account, token_program)?.amount != plan.source_balance_after
        || parse_claim_account(destination_account, token_program)?.amount
            != plan.destination_balance_after
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn dematerialize<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    outcome: usize,
    quantity: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let position_account = account(accounts, 2)?;
    let mint_account = account(accounts, 3)?;
    let source_account = account(accounts, 4)?;
    let holder = account(accounts, 5)?;
    let token_program = account(accounts, 6)?;
    if token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID || !holder.is_signer
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let market = decode_market::<N>(program_id, market_account, generation)?;
    let mut state = decode_state::<N>(program_id, state_account, market_account, generation)?;
    let mut position = decode_position::<N>(
        program_id,
        position_account,
        market_account,
        holder,
        generation,
    )?;
    let mint = parse_mint(mint_account, token_program)?;
    let source = parse_claim_account(source_account, token_program)?;
    let expected = canonical_mint(program_id, market_account.key, generation, outcome)?;
    let plan = dclutch_bearer_contract::transition::dematerialize(
        market_account.key.to_bytes(),
        &market,
        &mut state,
        &mut position,
        holder.key.to_bytes(),
        outcome,
        quantity,
        state_account.key.to_bytes(),
        expected,
        mint,
        source,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    let instruction = checked_permissioned_burn(
        token_program.key,
        source_account.key,
        mint_account.key,
        state_account.key,
        holder.key,
        plan.amount,
    )?;
    let generation_bytes = generation.to_le_bytes();
    let (_, bump) = Pubkey::find_program_address(
        &[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market_account.key.as_ref(),
            &generation_bytes,
        ],
        program_id,
    );
    invoke_signed(
        &instruction,
        &[
            source_account.clone(),
            mint_account.clone(),
            state_account.clone(),
            holder.clone(),
            token_program.clone(),
        ],
        &[&[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market_account.key.as_ref(),
            &generation_bytes,
            &[bump],
        ]],
    )
    .map_err(|_| AdapterError::BearerTokenCpi)?;
    if parse_mint(mint_account, token_program)?.supply != plan.mint_supply_after
        || parse_claim_account(source_account, token_program)?.amount != plan.account_balance_after
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    persist_state(state_account, state)?;
    persist_position(position_account, position)?;
    Ok(())
}

fn materialize<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    outcome: usize,
    quantity: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let position_account = account(accounts, 2)?;
    let mint_account = account(accounts, 3)?;
    let destination_account = account(accounts, 4)?;
    let holder = account(accounts, 5)?;
    let token_program = account(accounts, 6)?;
    if token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID || !holder.is_signer
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let market = decode_market::<N>(program_id, market_account, generation)?;
    let mut state = decode_state::<N>(program_id, state_account, market_account, generation)?;
    let mut position = decode_position::<N>(
        program_id,
        position_account,
        market_account,
        holder,
        generation,
    )?;
    let expected_mint = canonical_mint(program_id, market_account.key, generation, outcome)?;
    let mint = parse_mint(mint_account, token_program)?;
    let destination = parse_claim_account(destination_account, token_program)?;
    let plan = dclutch_bearer_contract::transition::materialize(
        market_account.key.to_bytes(),
        &market,
        &mut state,
        &mut position,
        holder.key.to_bytes(),
        outcome,
        quantity,
        state_account.key.to_bytes(),
        expected_mint,
        mint,
        destination,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    let instruction = checked_mint_to(
        token_program.key,
        mint_account.key,
        destination_account.key,
        state_account.key,
        plan.amount,
    )?;
    let generation_bytes = generation.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market_account.key.as_ref(),
            &generation_bytes,
        ],
        program_id,
    );
    if expected != *state_account.key {
        return Err(AdapterError::BearerAuthentication.into());
    }
    invoke_signed(
        &instruction,
        &[
            mint_account.clone(),
            destination_account.clone(),
            state_account.clone(),
            token_program.clone(),
        ],
        &[&[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market_account.key.as_ref(),
            &generation_bytes,
            &[bump],
        ]],
    )
    .map_err(|_| AdapterError::BearerTokenCpi)?;
    let after_mint = parse_mint(mint_account, token_program)?;
    let after_destination = parse_claim_account(destination_account, token_program)?;
    if after_mint.supply != plan.mint_supply_after
        || after_destination.amount != plan.account_balance_after
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    persist_state(state_account, state)?;
    persist_position(position_account, position)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct RealmFacts {
    realm: RealmV1,
    release: CollateralAdapterReleaseV1,
    mint: Mint,
}

impl RealmFacts {
    fn binding(self) -> Result<dclutch_bearer_contract::transition::RealmBindingV1, ProgramError> {
        Ok(dclutch_bearer_contract::transition::RealmBindingV1 {
            content_id: ContentId::new(hash(&self.realm.to_bytes()).to_bytes())
                .map_err(|_| AdapterError::BearerAuthentication)?,
            realm: self.realm,
        })
    }
}

fn authenticate_realm(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    mint_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    root: dclutch_core_contract::MarketRoot,
) -> Result<RealmFacts, ProgramError> {
    if realm_account.owner != program_id
        || mint_account.owner != token_program.key
        || !recognized_program_loader(token_program.owner)
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = realm_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let realm = RealmV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if realm.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let digest = hash(&data).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &digest], program_id);
    if root.identity().realm_id().to_bytes() != digest
        || realm_account.key != &expected
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint_account.key.as_ref()
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let mint = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint.mint_authority)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    require_freeze_policy(realm.freeze_authority_policy(), &mint.freeze_authority)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    Ok(RealmFacts {
        realm,
        release,
        mint,
    })
}

fn authenticate_custody(
    program_id: &Pubkey,
    custody: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    generation: u64,
) -> Result<(), ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_CUSTODY_PDA_DOMAIN, market.key.as_ref()],
        program_id,
    );
    if custody.key != &expected || custody.owner != program_id {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = custody
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let decoded =
        CollateralCustodyV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if decoded.to_bytes().as_slice() != &data[..]
        || decoded.market() != market.key.to_bytes()
        || decoded.generation() != generation
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(())
}

fn authenticate_vault(
    program_id: &Pubkey,
    vault: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
) -> Result<TokenAccount, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, market.key.as_ref()],
        program_id,
    );
    if vault.key != &expected || vault.owner != token_program.key {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            market.key.to_bytes(),
        )
        .map_err(|_| AdapterError::BearerAuthentication.into())
}

#[derive(Clone, Copy)]
struct MarketSigner {
    key: Pubkey,
    digest: [u8; 32],
    bump: u8,
}

fn market_signer(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    root: dclutch_core_contract::MarketRoot,
) -> Result<MarketSigner, ProgramError> {
    let digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected, bump) =
        Pubkey::find_program_address(&[crate::authenticate::MARKET_SEED, &digest], program_id);
    if market.key != &expected {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(MarketSigner {
        key: *market.key,
        digest,
        bump,
    })
}

fn execute_collateral_transfer<'a>(
    _program_id: &Pubkey,
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    realm: RealmFacts,
    holder: &AccountInfo<'a>,
    amount: u64,
    market_signer: Option<(&AccountInfo<'a>, MarketSigner)>,
) -> Result<(), ProgramError> {
    let authority = market_signer.map_or(*holder.key, |(_, signer)| signer.key);
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let facts = realm
        .release
        .profile()
        .check_transfer(ExactTransferInput {
            program_id: token_program.key.to_bytes(),
            mint_address: mint.key.to_bytes(),
            mint_data: &mint_data,
            source_data: &source_data,
            destination_data: &destination_data,
            authority: authority.to_bytes(),
            amount,
            decimals: realm.mint.decimals,
        })
        .map_err(|_| AdapterError::BearerAuthentication)?;
    drop((source_data, destination_data, mint_data));
    if facts.mint() != realm.mint {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let spec = transfer_checked(
        realm.release.token_program(),
        source.key.to_bytes(),
        mint.key.to_bytes(),
        destination.key.to_bytes(),
        authority.to_bytes(),
        amount,
        realm.mint.decimals,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    let instruction = Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: Vec::from([
            AccountMeta::new(*source.key, false),
            AccountMeta::new_readonly(*mint.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(authority, true),
        ]),
        data: Vec::from(*spec.data()),
    };
    if let Some((market, signer)) = market_signer {
        let bump = [signer.bump];
        let seeds = [
            crate::authenticate::MARKET_SEED,
            signer.digest.as_slice(),
            bump.as_slice(),
        ];
        invoke_signed(
            &instruction,
            &[
                source.clone(),
                mint.clone(),
                destination.clone(),
                market.clone(),
                token_program.clone(),
            ],
            &[&seeds],
        )
        .map_err(|_| AdapterError::CollateralTransferCpi)?;
        Ok(())
    } else {
        invoke(
            &instruction,
            &[
                source.clone(),
                mint.clone(),
                destination.clone(),
                holder.clone(),
                token_program.clone(),
            ],
        )
        .map_err(|_| AdapterError::CollateralTransferCpi)?;
        Ok(())
    }
}

fn expected_vault_amount(
    before: u64,
    amount: u64,
    direction: dclutch_bearer_contract::transition::CollateralDirectionV1,
) -> Result<u64, ProgramError> {
    match direction {
        dclutch_bearer_contract::transition::CollateralDirectionV1::DepositToHoard => before
            .checked_add(amount)
            .ok_or(AdapterError::Arithmetic.into()),
        dclutch_bearer_contract::transition::CollateralDirectionV1::WithdrawFromHoard => before
            .checked_sub(amount)
            .ok_or(AdapterError::BearerPostcondition.into()),
    }
}

fn decode_config(
    account: &AccountInfo<'_>,
) -> Result<(ContentId, dclutch_bearer_contract::state::BearerConfigV1), ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if account.executable {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let id =
        ContentId::new(hash(&data).to_bytes()).map_err(|_| AdapterError::BearerAuthentication)?;
    let config = dclutch_bearer_contract::state::BearerConfigV1::decode(&data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    Ok((id, config))
}

fn decode_funding(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    generation: u64,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
) -> Result<FundingStateV1, ProgramError> {
    if account.owner != program_id || account.data_len() != FUNDING_STATE_BYTES {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let funding = FundingStateV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if funding.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let derivation = CapabilityFundingDerivationV1::new(
        market.key.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    let (expected, _) = Pubkey::find_program_address(&derivation.seed_components(), program_id);
    if account.key != &expected {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(funding)
}

fn persist_funding(account: &AccountInfo<'_>, funding: FundingStateV1) -> Result<(), ProgramError> {
    let bytes = funding.to_bytes();
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if data.len() != bytes.len() {
        return Err(AdapterError::BearerPostcondition.into());
    }
    data.copy_from_slice(&bytes);
    if FundingStateV1::decode(&data) != Ok(funding) {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authority: &Pubkey,
) -> Result<RentCreditV1, ProgramError> {
    let refund = RefundAuthority::new(authority.to_bytes())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, refund.to_bytes().as_slice()],
        program_id,
    );
    if account.key != &expected
        || account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    credit
        .validate_binding(refund, bump)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(credit)
}

fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    credit: RentCreditV1,
) -> Result<(), ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if RentCreditV1::decode(&data) != Ok(credit) || credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn require_vacant(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || !account.data_is_empty() || account.lamports() != 0 {
        Err(AdapterError::BearerAuthentication.into())
    } else {
        Ok(())
    }
}

fn authenticate_system_and_rent(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if system.key != &system_program::ID
        || system.owner != &native_loader::ID
        || rent.key != &sysvar::rent::ID
        || rent.owner != &sysvar::ID
    {
        Err(AdapterError::BearerAuthentication.into())
    } else {
        Ok(())
    }
}

fn create_pda_account<'a>(
    _program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    new_account: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    lamports: u64,
    space: usize,
    owner: &Pubkey,
    seeds: &[&[u8]],
    bump: u8,
) -> Result<(), ProgramError> {
    let instruction = create_account(
        payer.key,
        new_account.key,
        lamports,
        u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?,
        owner,
    );
    let bump_seed = [bump];
    let mut signer = Vec::new();
    signer
        .try_reserve_exact(seeds.len() + 1)
        .map_err(|_| AdapterError::Arithmetic)?;
    for seed in seeds {
        signer.push(*seed);
    }
    signer.push(&bump_seed);
    invoke_signed(
        &instruction,
        &[payer.clone(), new_account.clone(), system.clone()],
        &[signer.as_slice()],
    )
    .map_err(|_| AdapterError::BearerCreateCpi)?;
    if new_account.lamports() != lamports
        || new_account.owner != owner
        || new_account.data_len() != space
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn initialize_bearer_mint<'a>(
    mint: &AccountInfo<'a>,
    controller: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Result<(), ProgramError> {
    let close = spl_token_2022_interface::instruction::initialize_mint_close_authority(
        token_program.key,
        mint.key,
        Some(controller.key),
    )?;
    invoke(&close, &[mint.clone(), token_program.clone()])
        .map_err(|_| AdapterError::BearerTokenCpi)?;
    let burn = spl_token_2022_interface::extension::permissioned_burn::instruction::initialize(
        token_program.key,
        mint.key,
        controller.key,
    )?;
    invoke(&burn, &[mint.clone(), token_program.clone()])
        .map_err(|_| AdapterError::BearerTokenCpi)?;
    let initialize = spl_token_2022_interface::instruction::initialize_mint2(
        token_program.key,
        mint.key,
        controller.key,
        None,
        0,
    )?;
    invoke(&initialize, &[mint.clone(), token_program.clone()])
        .map_err(|_| AdapterError::BearerTokenCpi)?;
    Ok(())
}

fn move_lamports_exact(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    amount: u64,
) -> Result<(), ProgramError> {
    let source_before = source.lamports();
    let destination_before = destination.lamports();
    let destination_after = destination_before
        .checked_add(amount)
        .ok_or(AdapterError::Arithmetic)?;
    let source_after = source_before
        .checked_sub(amount)
        .ok_or(AdapterError::Arithmetic)?;
    {
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::BearerPostcondition)?;
        let mut destination_lamports = destination
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::BearerPostcondition)?;
        **source_lamports = source_after;
        **destination_lamports = destination_after;
    }
    if source.lamports() != source_after || destination.lamports() != destination_after {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn mint_to_plan<'a>(
    program_id: &Pubkey,
    market: &AccountInfo<'a>,
    generation: u64,
    state: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    plan: dclutch_bearer_contract::transition::TokenSupplyPlanV1,
) -> Result<(), ProgramError> {
    if plan.operation != dclutch_bearer_contract::transition::TokenSupplyOperationV1::Mint
        || plan.mint != mint.key.to_bytes()
        || plan.token_account != token.key.to_bytes()
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    let instruction = checked_mint_to(
        token_program.key,
        mint.key,
        token.key,
        state.key,
        plan.amount,
    )?;
    invoke_state_signed(
        program_id,
        market,
        generation,
        state,
        token_program,
        &instruction,
        &[
            mint.clone(),
            token.clone(),
            state.clone(),
            token_program.clone(),
        ],
    )?;
    if parse_mint(mint, token_program)?.supply != plan.mint_supply_after
        || parse_claim_account(token, token_program)?.amount != plan.account_balance_after
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn burn_plan<'a>(
    program_id: &Pubkey,
    market: &AccountInfo<'a>,
    generation: u64,
    state: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token: &AccountInfo<'a>,
    holder: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    plan: dclutch_bearer_contract::transition::TokenSupplyPlanV1,
) -> Result<(), ProgramError> {
    if plan.operation != dclutch_bearer_contract::transition::TokenSupplyOperationV1::Burn
        || plan.mint != mint.key.to_bytes()
        || plan.token_account != token.key.to_bytes()
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    let instruction = checked_permissioned_burn(
        token_program.key,
        token.key,
        mint.key,
        state.key,
        holder.key,
        plan.amount,
    )?;
    invoke_state_signed(
        program_id,
        market,
        generation,
        state,
        token_program,
        &instruction,
        &[
            token.clone(),
            mint.clone(),
            state.clone(),
            holder.clone(),
            token_program.clone(),
        ],
    )?;
    if parse_mint(mint, token_program)?.supply != plan.mint_supply_after
        || parse_claim_account(token, token_program)?.amount != plan.account_balance_after
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn invoke_state_signed<'a>(
    program_id: &Pubkey,
    market: &AccountInfo<'a>,
    generation: u64,
    _state: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    instruction: &Instruction,
    infos: &[AccountInfo<'a>],
) -> Result<(), ProgramError> {
    let generation_bytes = generation.to_le_bytes();
    let (_, bump) = Pubkey::find_program_address(
        &[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market.key.as_ref(),
            &generation_bytes,
        ],
        program_id,
    );
    invoke_signed(
        instruction,
        infos,
        &[&[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market.key.as_ref(),
            &generation_bytes,
            &[bump],
        ]],
    )
    .map_err(|_| AdapterError::BearerTokenCpi)?;
    let _ = token_program;
    Ok(())
}

fn empty_claim() -> TokenAccountObservationV1 {
    TokenAccountObservationV1 {
        key: [0; 32],
        program_owner: [0; 32],
        data_len: 0,
        mint: [0; 32],
        authority: [0; 32],
        amount: 0,
        state: TokenAccountStateV1::Uninitialized,
        has_native_reserve: false,
        extension_count: 0,
    }
}

fn decode_position<const N: usize>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    holder: &AccountInfo<'_>,
    generation: u64,
) -> Result<PositionV1<N>, ProgramError> {
    if account.owner != program_id {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let (expected, _) = Pubkey::find_program_address(
        &[
            dclutch_realm_contract::POSITION_PDA_DOMAIN,
            market.key.as_ref(),
            holder.key.as_ref(),
        ],
        program_id,
    );
    if expected != *account.key {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let position = PositionV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if position.market() != market.key.as_ref()
        || position.owner() != holder.key.as_ref()
        || position.generation() != generation
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    Ok(position)
}

fn persist_state<const N: usize>(
    account: &AccountInfo<'_>,
    state: BearerCapabilityV1<N>,
) -> Result<(), ProgramError> {
    let length =
        BearerCapabilityV1::<N>::encoded_len().map_err(|_| AdapterError::BearerPostcondition)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    bytes.resize(length, 0);
    state
        .encode(&mut bytes)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::BearerPostcondition)?;
        if data.len() != bytes.len() {
            return Err(AdapterError::BearerPostcondition.into());
        }
        data.copy_from_slice(&bytes);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if BearerCapabilityV1::<N>::decode(&data) != Ok(state) || data.as_ref() != bytes.as_slice() {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}
fn persist_position<const N: usize>(
    account: &AccountInfo<'_>,
    position: PositionV1<N>,
) -> Result<(), ProgramError> {
    let length = PositionV1::<N>::encoded_len().map_err(|_| AdapterError::BearerPostcondition)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    bytes.resize(length, 0);
    position
        .encode(&mut bytes)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::BearerPostcondition)?;
        if data.len() != bytes.len() {
            return Err(AdapterError::BearerPostcondition.into());
        }
        data.copy_from_slice(&bytes);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if PositionV1::<N>::decode(&data) != Ok(position) || data.as_ref() != bytes.as_slice() {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn persist_market<const N: usize>(
    account: &AccountInfo<'_>,
    market: CategoricalMarketV1<N>,
) -> Result<(), ProgramError> {
    let length =
        CategoricalMarketV1::<N>::encoded_len().map_err(|_| AdapterError::BearerPostcondition)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    bytes.resize(length, 0);
    market
        .encode(&mut bytes)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::BearerPostcondition)?;
        if data.len() != bytes.len() {
            return Err(AdapterError::BearerPostcondition.into());
        }
        data.copy_from_slice(&bytes);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if CategoricalMarketV1::<N>::decode(&data) != Ok(market) || data.as_ref() != bytes.as_slice() {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn audit<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, 0)?;
    let state_account = account(accounts, 1)?;
    let token_program = account(accounts, 2)?;
    if state_account.owner != program_id
        || token_program.key.to_bytes() != dclutch_token_svm::TOKEN_2022_PROGRAM_ID
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let market = decode_market::<N>(program_id, market_account, generation)?;
    let state = decode_state::<N>(program_id, state_account, market_account, generation)?;
    let controller = state_account.key.to_bytes();
    let mut observations = [empty_mint(); N];
    let mut expected = [[0u8; 32]; N];
    for index in 0..N {
        let mint = account(accounts, 3 + index)?;
        expected[index] = canonical_mint(program_id, market_account.key, generation, index)?;
        observations[index] = parse_mint(mint, token_program)?;
    }
    dclutch_bearer_contract::transition::audit_mints(
        &state,
        market_account.key.to_bytes(),
        &market,
        controller,
        expected,
        observations,
    )
    .map_err(|_| AdapterError::BearerTransition.into())
}

fn decode_market<const N: usize>(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    generation: u64,
) -> Result<CategoricalMarketV1<N>, ProgramError> {
    if market_account.owner != program_id {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = market_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if decode_market_outcome_count(&data).map_err(|_| AdapterError::BearerAuthentication)?
        != N as u8
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let market =
        CategoricalMarketV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if market.root().identity().generation() != generation {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let identity_digest = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(
        &[crate::authenticate::MARKET_SEED, &identity_digest],
        program_id,
    );
    if market_account.key != &expected {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(market)
}

fn decode_state<const N: usize>(
    program_id: &Pubkey,
    state_account: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    generation: u64,
) -> Result<BearerCapabilityV1<N>, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[
            dclutch_bearer_contract::state::BEARER_CAPABILITY_PDA_DOMAIN,
            market.key.as_ref(),
            &generation.to_le_bytes(),
        ],
        program_id,
    );
    if state_account.key != &expected || state_account.owner != program_id {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = state_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let state =
        BearerCapabilityV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if state.market() != market.key.as_ref() || state.generation() != generation {
        return Err(AdapterError::ReplayMismatch.into());
    }
    Ok(state)
}

fn canonical_mint(
    program_id: &Pubkey,
    market: &Pubkey,
    generation: u64,
    outcome: usize,
) -> Result<[u8; 32], ProgramError> {
    let outcome = u8::try_from(outcome).map_err(|_| AdapterError::BearerAuthentication)?;
    let (mint, _) = Pubkey::find_program_address(
        &[
            dclutch_bearer_contract::state::BEARER_MINT_PDA_DOMAIN,
            market.as_ref(),
            &generation.to_le_bytes(),
            &[outcome],
        ],
        program_id,
    );
    Ok(mint.to_bytes())
}

fn parse_mint(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<MintObservationV1, ProgramError> {
    if account.owner != token_program.key || account.data_len() != BEARER_MINT_BYTES {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let mint_authority = coption_key(&data, 0)?;
    let freeze_authority = coption_key(&data, 46)?;
    let supply = u64_at(&data, 36)?;
    let decimals = byte(&data, 44)?;
    let initialized = byte(&data, 45)? == 1;
    // 82-byte Mint base, 83-byte pad, Mint account-type byte, then exactly
    // two fully-consumed TLVs.  Unknown, duplicate, short, or trailing TLVs
    // are all rejected before an observation reaches the pure contract.
    if data.get(82..165) != Some(&[0; 83]) || byte(&data, 165)? != 1 {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let mut close = None;
    let mut burn = None;
    let mut offset = 166usize;
    let mut count = 0u16;
    while offset < data.len() {
        let kind = u16_at(&data, offset)?;
        let length = usize::from(u16_at(&data, offset + 2)?);
        let value = data
            .get(offset + 4..offset + 4 + length)
            .ok_or(AdapterError::BearerAuthentication)?;
        if length != 32 {
            return Err(AdapterError::BearerAuthentication.into());
        }
        let key: [u8; 32] = value
            .try_into()
            .map_err(|_| AdapterError::BearerAuthentication)?;
        match kind {
            kind if kind == ExtensionType::MintCloseAuthority as u16
                && close.replace(key).is_none() => {}
            kind if kind == ExtensionType::PermissionedBurn as u16
                && burn.replace(key).is_none() => {}
            _ => return Err(AdapterError::BearerAuthentication.into()),
        }
        count = count.checked_add(1).ok_or(AdapterError::Arithmetic)?;
        offset = offset
            .checked_add(4 + length)
            .ok_or(AdapterError::Arithmetic)?;
    }
    Ok(MintObservationV1 {
        key: account.key.to_bytes(),
        program_owner: account.owner.to_bytes(),
        data_len: data.len(),
        supply,
        decimals,
        initialized,
        mint_authority,
        freeze_authority,
        close_authority: close,
        permissioned_burn_authority: burn,
        extension_count: count,
    })
}

fn empty_mint() -> MintObservationV1 {
    MintObservationV1 {
        key: [0; 32],
        program_owner: [0; 32],
        data_len: 0,
        supply: 0,
        decimals: 0,
        initialized: false,
        mint_authority: None,
        freeze_authority: None,
        close_authority: None,
        permissioned_burn_authority: None,
        extension_count: 0,
    }
}
fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::BearerAuthentication.into())
}
fn byte(data: &[u8], offset: usize) -> Result<u8, ProgramError> {
    data.get(offset)
        .copied()
        .ok_or(AdapterError::BearerAuthentication.into())
}
fn u16_at(data: &[u8], offset: usize) -> Result<u16, ProgramError> {
    Ok(u16::from_le_bytes(
        data.get(offset..offset + 2)
            .ok_or(AdapterError::BearerAuthentication)?
            .try_into()
            .map_err(|_| AdapterError::BearerAuthentication)?,
    ))
}
fn u64_at(data: &[u8], offset: usize) -> Result<u64, ProgramError> {
    Ok(u64::from_le_bytes(
        data.get(offset..offset + 8)
            .ok_or(AdapterError::BearerAuthentication)?
            .try_into()
            .map_err(|_| AdapterError::BearerAuthentication)?,
    ))
}
fn coption_key(data: &[u8], offset: usize) -> Result<Option<[u8; 32]>, ProgramError> {
    match u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or(AdapterError::BearerAuthentication)?
            .try_into()
            .map_err(|_| AdapterError::BearerAuthentication)?,
    ) {
        0 => Ok(None),
        1 => Ok(Some(key_at(data, offset + 4)?)),
        _ => Err(AdapterError::BearerAuthentication.into()),
    }
}
fn key_at(data: &[u8], offset: usize) -> Result<[u8; 32], ProgramError> {
    data.get(offset..offset + 32)
        .ok_or(AdapterError::BearerAuthentication)?
        .try_into()
        .map_err(|_| AdapterError::BearerAuthentication.into())
}

fn parse_claim_account(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<TokenAccountObservationV1, ProgramError> {
    if account.owner != token_program.key || account.data_len() != BEARER_TOKEN_ACCOUNT_BYTES {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let state = match byte(&data, 108)? {
        0 => TokenAccountStateV1::Uninitialized,
        1 => TokenAccountStateV1::Initialized,
        2 => TokenAccountStateV1::Frozen,
        _ => return Err(AdapterError::BearerAuthentication.into()),
    };
    let native = u32::from_le_bytes(
        data.get(109..113)
            .ok_or(AdapterError::BearerAuthentication)?
            .try_into()
            .map_err(|_| AdapterError::BearerAuthentication)?,
    );
    if native > 1 {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(TokenAccountObservationV1 {
        key: account.key.to_bytes(),
        program_owner: account.owner.to_bytes(),
        data_len: data.len(),
        mint: key_at(&data, 0)?,
        authority: key_at(&data, 32)?,
        amount: u64_at(&data, 64)?,
        state,
        has_native_reserve: native == 1,
        extension_count: 0,
    })
}

// Keep Token-2022 instruction construction pinned to the reviewed interface
// crate.  The adapter converts its public Instruction into the program SDK
// type only through this compile-checked boundary.
#[allow(dead_code)]
fn checked_mint_to(
    token_program: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    controller: &Pubkey,
    amount: u64,
) -> Result<Instruction, ProgramError> {
    spl_token_2022_interface::instruction::mint_to_checked(
        token_program,
        mint,
        destination,
        controller,
        &[],
        amount,
        0,
    )
}

fn checked_permissioned_burn(
    token_program: &Pubkey,
    source: &Pubkey,
    mint: &Pubkey,
    controller: &Pubkey,
    holder: &Pubkey,
    amount: u64,
) -> Result<Instruction, ProgramError> {
    spl_token_2022_interface::extension::permissioned_burn::instruction::burn_checked(
        token_program,
        source,
        mint,
        controller,
        holder,
        &[],
        amount,
        0,
    )
}
