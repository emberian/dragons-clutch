//! Exact native-Position and collateral-liability lifecycle processors.
//!
//! Every processor authenticates the provider-neutral Market, its committed
//! Realm, the Realm-selected token release, and all native/token account
//! coordinates before the first CPI.  Hoard atoms move only with the matching
//! native-claim transition; rent and physical surplus never enter Hoard.

use alloc::vec::Vec;

use dclutch_collateral_contract::{
    AccountPrivilege, COLLATERAL_VAULT_PDA_DOMAIN, CloseEmptyPositionV1, CreatePositionAndSplitV1,
    InstructionTag, MergeCompleteSetV1, RedeemResolvedOutcomeV1, SplitCompleteSetV1,
    SweepSurplusTokenAccountFactsV1, SweepSurplusV1, authorize_sweep_surplus_destination,
    validate_account_frame,
};
use dclutch_core_contract::{MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN, RealmV1};
use dclutch_token_svm::{
    AuthorityRole, CollateralAdapterReleaseV1, ExactTransferInput, Mint, TokenAccount,
    transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    realm::{
        recognized_program_loader, require_authority_policy, require_freeze_policy,
        select_adapter_release,
    },
};

const CREATE_ACCOUNTS: usize = 10;
const TRANSFER_ACCOUNTS: usize = 8;
const SWEEP_ACCOUNTS: usize = 6;
const CLOSE_ACCOUNTS: usize = 4;

const OWNER: usize = 0;
const MARKET: usize = 1;
const REALM: usize = 2;
const POSITION: usize = 3;
const VAULT: usize = 4;
const USER_TOKEN: usize = 5;
const MINT: usize = 6;
const TOKEN_PROGRAM: usize = 7;
const CREATE_SYSTEM_PROGRAM: usize = 8;
const RENT_SYSVAR: usize = 9;

const SWEEP_MARKET: usize = 0;
const SWEEP_REALM: usize = 1;
const SWEEP_VAULT: usize = 2;
const SWEEP_DESTINATION: usize = 3;
const SWEEP_MINT: usize = 4;
const SWEEP_TOKEN_PROGRAM: usize = 5;

const CLOSE_OWNER: usize = 0;
const CLOSE_MARKET: usize = 1;
const CLOSE_POSITION: usize = 2;
const CLOSE_SYSTEM_PROGRAM: usize = 3;

#[derive(Clone, Copy)]
struct RealmFacts {
    realm: RealmV1,
    release: CollateralAdapterReleaseV1,
    mint: Mint,
}

#[derive(Clone, Copy)]
struct TransferFacts {
    source: TokenAccount,
    destination: TokenAccount,
    authority_role: AuthorityRole,
    source_lamports: u64,
    destination_lamports: u64,
    mint_lamports: u64,
}

#[derive(Clone, Copy)]
struct MarketSignerFacts {
    identity_digest: [u8; 32],
    bump: u8,
}

macro_rules! dispatch_width {
    ($accounts:expr, $market_index:expr, $processor:ident, $($argument:expr),+ $(,)?) => {{
        let market_account = account($accounts, $market_index)?;
        let market_data = market_account
            .try_borrow_data()
            .map_err(|_| AdapterError::PositionAuthentication)?;
        let outcome_count = decode_market_outcome_count(&market_data)
            .map_err(|_| AdapterError::PositionAuthentication)?;
        drop(market_data);
        match outcome_count {
            2 => $processor::<2>($($argument),+),
            3 => $processor::<3>($($argument),+),
            4 => $processor::<4>($($argument),+),
            5 => $processor::<5>($($argument),+),
            6 => $processor::<6>($($argument),+),
            7 => $processor::<7>($($argument),+),
            8 => $processor::<8>($($argument),+),
            9 => $processor::<9>($($argument),+),
            10 => $processor::<10>($($argument),+),
            11 => $processor::<11>($($argument),+),
            12 => $processor::<12>($($argument),+),
            13 => $processor::<13>($($argument),+),
            14 => $processor::<14>($($argument),+),
            15 => $processor::<15>($($argument),+),
            16 => $processor::<16>($($argument),+),
            _ => Err(AdapterError::PositionAuthentication.into()),
        }
    }};
}

/// Create one exact Position PDA and atomically split its first complete set.
pub(crate) fn process_create_position_and_split(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CreatePositionAndSplitV1,
) -> Result<(), ProgramError> {
    validate_frame(
        InstructionTag::CreatePositionAndSplit,
        accounts,
        CREATE_ACCOUNTS,
    )?;
    dispatch_width!(
        accounts,
        MARKET,
        create_position_and_split,
        program_id,
        accounts,
        instruction
    )
}

/// Deposit collateral and credit one complete set to an existing Position.
pub(crate) fn process_split_complete_set(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: SplitCompleteSetV1,
) -> Result<(), ProgramError> {
    validate_frame(
        InstructionTag::SplitCompleteSet,
        accounts,
        TRANSFER_ACCOUNTS,
    )?;
    dispatch_width!(
        accounts,
        MARKET,
        split_complete_set,
        program_id,
        accounts,
        instruction
    )
}

/// Debit one complete set and release the exact backing collateral.
pub(crate) fn process_merge_complete_set(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: MergeCompleteSetV1,
) -> Result<(), ProgramError> {
    validate_frame(
        InstructionTag::MergeCompleteSet,
        accounts,
        TRANSFER_ACCOUNTS,
    )?;
    dispatch_width!(
        accounts,
        MARKET,
        merge_complete_set,
        program_id,
        accounts,
        instruction
    )
}

/// Burn one selected resolved claim and release only its canonical payout.
pub(crate) fn process_redeem_resolved_outcome(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: RedeemResolvedOutcomeV1,
) -> Result<(), ProgramError> {
    validate_frame(
        InstructionTag::RedeemResolvedOutcome,
        accounts,
        TRANSFER_ACCOUNTS,
    )?;
    dispatch_width!(
        accounts,
        MARKET,
        redeem_resolved_outcome,
        program_id,
        accounts,
        instruction
    )
}

/// Sweep physical collateral above Hoard to the immutable refund owner's token account.
pub(crate) fn process_sweep_surplus(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: SweepSurplusV1,
) -> Result<(), ProgramError> {
    validate_frame(InstructionTag::SweepSurplus, accounts, SWEEP_ACCOUNTS)?;
    dispatch_width!(
        accounts,
        SWEEP_MARKET,
        sweep_surplus,
        program_id,
        accounts,
        instruction
    )
}

/// Close one empty Position and retire exactly its Market child count.
pub(crate) fn process_close_empty_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CloseEmptyPositionV1,
) -> Result<(), ProgramError> {
    validate_frame(InstructionTag::CloseEmptyPosition, accounts, CLOSE_ACCOUNTS)?;
    dispatch_width!(
        accounts,
        CLOSE_MARKET,
        close_empty_position,
        program_id,
        accounts,
        instruction
    )
}

fn create_position_and_split<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CreatePositionAndSplitV1,
) -> Result<(), ProgramError> {
    let owner = account(accounts, OWNER)?;
    let market_account = account(accounts, MARKET)?;
    let realm_account = account(accounts, REALM)?;
    let position_account = account(accounts, POSITION)?;
    let vault = account(accounts, VAULT)?;
    let source = account(accounts, USER_TOKEN)?;
    let mint = account(accounts, MINT)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    let system = account(accounts, CREATE_SYSTEM_PROGRAM)?;
    let rent_sysvar = account(accounts, RENT_SYSVAR)?;

    authenticate_system_and_rent(system, rent_sysvar)?;
    if owner.owner != &system_program::ID
        || !owner.data_is_empty()
        || position_account.owner != &system_program::ID
        || !position_account.data_is_empty()
        || position_account.lamports() != 0
    {
        return Err(AdapterError::PositionAuthentication.into());
    }

    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    if market.root().phase() != Phase::Open
        || market.root().outstanding_children() != instruction.child_count()
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let realm = authenticate_realm(
        program_id,
        realm_account,
        mint,
        token_program,
        market.root(),
    )?;
    let position_bump =
        authenticate_new_position(program_id, position_account, market_account, owner)?;
    let vault_before = authenticate_vault(
        program_id,
        market_account,
        vault,
        mint,
        token_program,
        realm,
        market.hoard_atoms(),
    )?;
    let transfer = authenticate_transfer(
        source,
        vault,
        mint,
        token_program,
        realm,
        owner.key,
        instruction.quantity(),
    )?;
    if transfer.destination != vault_before {
        return Err(AdapterError::PositionAuthentication.into());
    }

    let mut market_after = market;
    market_after
        .register_child(instruction.generation(), instruction.child_count())
        .and_then(|()| market_after.split_complete_set(instruction.quantity()))
        .map_err(|_| AdapterError::MarketTransition)?;
    let mut position_after = PositionV1::<N>::empty(
        market_account.key.to_bytes(),
        owner.key.to_bytes(),
        instruction.generation(),
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    position_after
        .credit_complete_set(instruction.quantity())
        .map_err(|_| AdapterError::MarketTransition)?;
    let market_after_bytes = encode_market(market_after)?;
    let position_after_bytes = encode_position(position_after)?;

    let rent =
        Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::PositionAuthentication)?;
    let position_rent = rent.minimum_balance(position_after_bytes.len());
    let owner_before = owner
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let owner_after = owner_before
        .checked_sub(position_rent)
        .ok_or(AdapterError::PositionRentUnderfunded)?;
    let position_space =
        u64::try_from(position_after_bytes.len()).map_err(|_| AdapterError::Arithmetic)?;
    let market_lamports = market_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let create = create_account(
        owner.key,
        position_account.key,
        position_rent,
        position_space,
        program_id,
    );
    let transfer_instruction = checked_transfer_instruction(
        realm.release,
        source.key,
        mint.key,
        vault.key,
        owner.key,
        instruction.quantity(),
        realm.mint.decimals,
    )?;
    preflight_mutable(&[owner, market_account, position_account, vault, source])?;

    let bump = [position_bump];
    let signer = [
        POSITION_PDA_DOMAIN,
        market_account.key.as_ref(),
        owner.key.as_ref(),
        bump.as_slice(),
    ];
    invoke_signed(
        &create,
        &[owner.clone(), position_account.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;
    if owner.lamports() != owner_after
        || position_account.lamports() != position_rent
        || position_account.owner != program_id
        || position_account.data_len() != position_after_bytes.len()
    {
        return Err(AdapterError::PositionPostcondition.into());
    }

    invoke(
        &transfer_instruction,
        &[
            source.clone(),
            mint.clone(),
            vault.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| AdapterError::CollateralTransferCpi)?;
    authenticate_transfer_post(
        source,
        vault,
        mint,
        token_program,
        realm,
        transfer,
        instruction.quantity(),
    )?;
    persist_market_and_position(
        market_account,
        &market_after_bytes,
        market_after,
        position_account,
        &position_after_bytes,
        position_after,
    )?;
    if market_account.lamports() != market_lamports
        || position_account.lamports() != position_rent
        || owner.lamports() != owner_after
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn split_complete_set<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: SplitCompleteSetV1,
) -> Result<(), ProgramError> {
    let owner = account(accounts, OWNER)?;
    let market_account = account(accounts, MARKET)?;
    let realm_account = account(accounts, REALM)?;
    let position_account = account(accounts, POSITION)?;
    let vault = account(accounts, VAULT)?;
    let source = account(accounts, USER_TOKEN)?;
    let mint = account(accounts, MINT)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;

    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        mint,
        token_program,
        market.root(),
    )?;
    let position = authenticate_position::<N>(
        program_id,
        position_account,
        market_account,
        owner,
        instruction.generation(),
    )?;
    let vault_before = authenticate_vault(
        program_id,
        market_account,
        vault,
        mint,
        token_program,
        realm,
        market.hoard_atoms(),
    )?;
    let transfer = authenticate_transfer(
        source,
        vault,
        mint,
        token_program,
        realm,
        owner.key,
        instruction.quantity(),
    )?;
    if transfer.destination != vault_before {
        return Err(AdapterError::PositionAuthentication.into());
    }

    let mut market_after = market;
    market_after
        .split_complete_set(instruction.quantity())
        .map_err(|_| AdapterError::MarketTransition)?;
    let mut position_after = position;
    position_after
        .credit_complete_set(instruction.quantity())
        .map_err(|_| AdapterError::MarketTransition)?;
    execute_existing_position_transfer(
        market_account,
        market_after,
        position_account,
        position_after,
        source,
        vault,
        mint,
        token_program,
        owner,
        realm,
        transfer,
        instruction.quantity(),
        None,
    )
}

fn merge_complete_set<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: MergeCompleteSetV1,
) -> Result<(), ProgramError> {
    let owner = account(accounts, OWNER)?;
    let market_account = account(accounts, MARKET)?;
    let realm_account = account(accounts, REALM)?;
    let position_account = account(accounts, POSITION)?;
    let vault = account(accounts, VAULT)?;
    let destination = account(accounts, USER_TOKEN)?;
    let mint = account(accounts, MINT)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;

    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        mint,
        token_program,
        market.root(),
    )?;
    let position = authenticate_position::<N>(
        program_id,
        position_account,
        market_account,
        owner,
        instruction.generation(),
    )?;
    let vault_before = authenticate_vault(
        program_id,
        market_account,
        vault,
        mint,
        token_program,
        realm,
        market.hoard_atoms(),
    )?;
    let transfer = authenticate_transfer(
        vault,
        destination,
        mint,
        token_program,
        realm,
        market_account.key,
        instruction.quantity(),
    )?;
    if transfer.source != vault_before || transfer.authority_role != AuthorityRole::Owner {
        return Err(AdapterError::PositionAuthentication.into());
    }

    let mut market_after = market;
    market_after
        .merge_complete_set(instruction.quantity())
        .map_err(|_| AdapterError::MarketTransition)?;
    let mut position_after = position;
    position_after
        .debit_complete_set(instruction.quantity())
        .map_err(|_| AdapterError::MarketTransition)?;
    execute_existing_position_transfer(
        market_account,
        market_after,
        position_account,
        position_after,
        vault,
        destination,
        mint,
        token_program,
        market_account,
        realm,
        transfer,
        instruction.quantity(),
        Some(market_signer(program_id, market_account, market.root())?),
    )
}

fn redeem_resolved_outcome<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: RedeemResolvedOutcomeV1,
) -> Result<(), ProgramError> {
    let owner = account(accounts, OWNER)?;
    let market_account = account(accounts, MARKET)?;
    let realm_account = account(accounts, REALM)?;
    let position_account = account(accounts, POSITION)?;
    let vault = account(accounts, VAULT)?;
    let destination = account(accounts, USER_TOKEN)?;
    let mint = account(accounts, MINT)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;

    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    let realm = authenticate_realm(
        program_id,
        realm_account,
        mint,
        token_program,
        market.root(),
    )?;
    let position = authenticate_position::<N>(
        program_id,
        position_account,
        market_account,
        owner,
        instruction.generation(),
    )?;
    let vault_before = authenticate_vault(
        program_id,
        market_account,
        vault,
        mint,
        token_program,
        realm,
        market.hoard_atoms(),
    )?;
    let outcome = usize::from(instruction.outcome());
    if outcome >= N {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let mut market_after = market;
    let payout = market_after
        .redeem_outcome(outcome, instruction.quantity())
        .map_err(|_| AdapterError::MarketTransition)?;
    let mut position_after = position;
    position_after
        .debit_outcome(outcome, instruction.quantity())
        .map_err(|_| AdapterError::MarketTransition)?;

    let destination_before = authenticate_token_account(destination, token_program, realm)?;

    if payout == 0 {
        let market_after_bytes = encode_market(market_after)?;
        let position_after_bytes = encode_position(position_after)?;
        let market_lamports = market_account
            .try_lamports()
            .map_err(|_| AdapterError::PositionAuthentication)?;
        let position_lamports = position_account
            .try_lamports()
            .map_err(|_| AdapterError::PositionAuthentication)?;
        preflight_mutable(&[market_account, position_account])?;
        persist_market_and_position(
            market_account,
            &market_after_bytes,
            market_after,
            position_account,
            &position_after_bytes,
            position_after,
        )?;
        if market_account.lamports() != market_lamports
            || position_account.lamports() != position_lamports
        {
            return Err(AdapterError::PositionPostcondition.into());
        }
        return Ok(());
    }
    let transfer = authenticate_transfer(
        vault,
        destination,
        mint,
        token_program,
        realm,
        market_account.key,
        payout,
    )?;
    if transfer.source != vault_before
        || transfer.destination != destination_before
        || transfer.authority_role != AuthorityRole::Owner
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    execute_existing_position_transfer(
        market_account,
        market_after,
        position_account,
        position_after,
        vault,
        destination,
        mint,
        token_program,
        market_account,
        realm,
        transfer,
        payout,
        Some(market_signer(program_id, market_account, market.root())?),
    )
}

fn sweep_surplus<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: SweepSurplusV1,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, SWEEP_MARKET)?;
    let realm_account = account(accounts, SWEEP_REALM)?;
    let vault = account(accounts, SWEEP_VAULT)?;
    let destination = account(accounts, SWEEP_DESTINATION)?;
    let mint = account(accounts, SWEEP_MINT)?;
    let token_program = account(accounts, SWEEP_TOKEN_PROGRAM)?;
    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    if !matches!(
        market.root().phase(),
        Phase::Open | Phase::Resolved | Phase::Retiring
    ) {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let realm = authenticate_realm(
        program_id,
        realm_account,
        mint,
        token_program,
        market.root(),
    )?;
    let vault_before = authenticate_vault(
        program_id,
        market_account,
        vault,
        mint,
        token_program,
        realm,
        market.hoard_atoms(),
    )?;
    let destination_before = authenticate_token_account(destination, token_program, realm)?;
    authorize_sweep_surplus_destination(
        market.root(),
        realm.realm,
        sweep_facts(vault, vault_before)?,
        sweep_facts(destination, destination_before)?,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    let surplus = vault_before
        .amount
        .checked_sub(market.hoard_atoms())
        .ok_or(AdapterError::PositionAuthentication)?;
    let market_before = snapshot_data(market_account)?;
    let market_lamports = market_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if surplus == 0 {
        return Ok(());
    }
    let transfer = authenticate_transfer(
        vault,
        destination,
        mint,
        token_program,
        realm,
        market_account.key,
        surplus,
    )?;
    if transfer.source != vault_before
        || transfer.destination != destination_before
        || transfer.authority_role != AuthorityRole::Owner
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let instruction = checked_transfer_instruction(
        realm.release,
        vault.key,
        mint.key,
        destination.key,
        market_account.key,
        surplus,
        realm.mint.decimals,
    )?;
    preflight_mutable(&[market_account, vault, destination])?;
    let signer_facts = market_signer(program_id, market_account, market.root())?;
    let bump = [signer_facts.bump];
    let signer = [
        MARKET_SEED,
        signer_facts.identity_digest.as_slice(),
        bump.as_slice(),
    ];
    invoke_signed(
        &instruction,
        &[
            vault.clone(),
            mint.clone(),
            destination.clone(),
            market_account.clone(),
            token_program.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| AdapterError::CollateralTransferCpi)?;
    authenticate_transfer_post(
        vault,
        destination,
        mint,
        token_program,
        realm,
        transfer,
        surplus,
    )?;
    if snapshot_data(market_account)? != market_before
        || market_account.lamports() != market_lamports
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn close_empty_position<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CloseEmptyPositionV1,
) -> Result<(), ProgramError> {
    let owner = account(accounts, CLOSE_OWNER)?;
    let market_account = account(accounts, CLOSE_MARKET)?;
    let position_account = account(accounts, CLOSE_POSITION)?;
    let system = account(accounts, CLOSE_SYSTEM_PROGRAM)?;
    authenticate_system(system)?;
    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    if !matches!(
        market.root().phase(),
        Phase::Open | Phase::Resolved | Phase::Retiring
    ) || market.root().outstanding_children() != instruction.child_count()
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let position = authenticate_position::<N>(
        program_id,
        position_account,
        market_account,
        owner,
        instruction.generation(),
    )?;
    position
        .require_empty()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let mut market_after = market;
    market_after
        .retire_child(instruction.generation(), instruction.child_count())
        .map_err(|_| AdapterError::MarketTransition)?;
    let market_after_bytes = encode_market(market_after)?;
    let market_lamports = market_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let position_lamports = position_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let owner_before = owner
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let owner_after = owner_before
        .checked_add(position_lamports)
        .ok_or(AdapterError::Arithmetic)?;
    preflight_mutable(&[owner, market_account, position_account])?;

    persist_market(market_account, &market_after_bytes, market_after)?;
    {
        let mut owner_lamports = owner
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionClose)?;
        let mut position_balance = position_account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionClose)?;
        **owner_lamports = owner_after;
        **position_balance = 0;
    }
    // The System Program has no close instruction for a program-owned account.
    // dClutch therefore drains its own Position, removes its data while still
    // owner, and only then restores the empty account to System ownership.
    position_account
        .resize(0)
        .map_err(|_| AdapterError::PositionClose)?;
    position_account.assign(&system_program::ID);
    if owner.lamports() != owner_after
        || market_account.lamports() != market_lamports
        || position_account.lamports() != 0
        || !position_account.data_is_empty()
        || position_account.owner != &system_program::ID
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_existing_position_transfer<'info, const N: usize>(
    market_account: &AccountInfo<'info>,
    market_after: CategoricalMarketV1<N>,
    position_account: &AccountInfo<'info>,
    position_after: PositionV1<N>,
    source: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    realm: RealmFacts,
    transfer: TransferFacts,
    quantity: u64,
    signer: Option<MarketSignerFacts>,
) -> Result<(), ProgramError> {
    let market_after_bytes = encode_market(market_after)?;
    let position_after_bytes = encode_position(position_after)?;
    let market_lamports = market_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let position_lamports = position_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let instruction = checked_transfer_instruction(
        realm.release,
        source.key,
        mint.key,
        destination.key,
        authority.key,
        quantity,
        realm.mint.decimals,
    )?;
    preflight_mutable(&[market_account, position_account, source, destination])?;
    let account_infos = [
        source.clone(),
        mint.clone(),
        destination.clone(),
        authority.clone(),
        token_program.clone(),
    ];
    match signer {
        Some(facts) => {
            let bump = [facts.bump];
            let seeds = [
                MARKET_SEED,
                facts.identity_digest.as_slice(),
                bump.as_slice(),
            ];
            invoke_signed(&instruction, &account_infos, &[&seeds])
        }
        None => invoke(&instruction, &account_infos),
    }
    .map_err(|_| AdapterError::CollateralTransferCpi)?;
    authenticate_transfer_post(
        source,
        destination,
        mint,
        token_program,
        realm,
        transfer,
        quantity,
    )?;
    persist_market_and_position(
        market_account,
        &market_after_bytes,
        market_after,
        position_account,
        &position_after_bytes,
        position_after,
    )?;
    if market_account.lamports() != market_lamports
        || position_account.lamports() != position_lamports
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn validate_frame(
    tag: InstructionTag,
    accounts: &[AccountInfo<'_>],
    expected: usize,
) -> Result<(), ProgramError> {
    if accounts.len() != expected {
        return Err(AdapterError::AccountFrameLength.into());
    }
    let mut privileges = Vec::new();
    privileges
        .try_reserve_exact(expected)
        .map_err(|_| AdapterError::Arithmetic)?;
    for account in accounts {
        privileges.push(AccountPrivilege {
            is_signer: account.is_signer,
            is_writable: account.is_writable,
            is_executable: account.executable,
        });
    }
    validate_account_frame(tag, &privileges).map_err(|_| AdapterError::AccountPrivilege)?;
    require_distinct(accounts)
}

fn authenticate_market<const N: usize>(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    generation: u64,
) -> Result<CategoricalMarketV1<N>, ProgramError> {
    if market_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = market_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let market = CategoricalMarketV1::<N>::decode(&data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if market.root().identity().generation() != generation {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let identity_digest = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if market_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let encoded = encode_market(market)?;
    if encoded.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(market)
}

fn authenticate_realm(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    mint_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    root: MarketRoot,
) -> Result<RealmFacts, ProgramError> {
    if realm_account.owner != program_id
        || mint_account.owner != token_program.key
        || !recognized_program_loader(token_program.owner)
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let data = realm_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let realm = RealmV1::decode(&data).map_err(|_| AdapterError::PositionAuthentication)?;
    if realm.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let realm_digest = hash(&data).to_bytes();
    if root.identity().realm_id().to_bytes() != realm_digest {
        return Err(AdapterError::ContentIdentity.into());
    }
    let (expected_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], program_id);
    if realm_account.key != &expected_realm
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint_account.key.as_ref()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if release.token_program() != token_program.key.to_bytes() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let mint = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint.mint_authority)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    require_freeze_policy(realm.freeze_authority_policy(), &mint.freeze_authority)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(RealmFacts {
        realm,
        release,
        mint,
    })
}

fn authenticate_new_position(
    program_id: &Pubkey,
    position: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    owner: &AccountInfo<'_>,
) -> Result<u8, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(
        &[POSITION_PDA_DOMAIN, market.key.as_ref(), owner.key.as_ref()],
        program_id,
    );
    if position.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(bump)
}

fn authenticate_position<const N: usize>(
    program_id: &Pubkey,
    position_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    owner: &AccountInfo<'_>,
    generation: u64,
) -> Result<PositionV1<N>, ProgramError> {
    if position_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let (expected, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            market_account.key.as_ref(),
            owner.key.as_ref(),
        ],
        program_id,
    );
    if position_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = position_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let position =
        PositionV1::<N>::decode(&data).map_err(|_| AdapterError::PositionAuthentication)?;
    if position.market() != market_account.key.as_ref()
        || position.owner() != owner.key.as_ref()
        || position.generation() != generation
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let encoded = encode_position(position)?;
    if encoded.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(position)
}

fn authenticate_vault(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    vault: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    hoard_atoms: u64,
) -> Result<TokenAccount, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, market.key.as_ref()],
        program_id,
    );
    if vault.key != &expected || vault.owner != token_program.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let account = realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            market.key.to_bytes(),
        )
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if account.amount < hoard_atoms {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(account)
}

fn authenticate_token_account(
    token_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
) -> Result<TokenAccount, ProgramError> {
    if token_account.owner != token_program.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = token_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let account = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if account.mint != *realm.realm.collateral_mint() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(account)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_transfer(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    authority: &Pubkey,
    quantity: u64,
) -> Result<TransferFacts, ProgramError> {
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
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
            amount: quantity,
            decimals: realm.mint.decimals,
        })
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if facts.mint() != realm.mint {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(TransferFacts {
        source: facts.source(),
        destination: facts.destination(),
        authority_role: facts.authority_role(),
        source_lamports: source
            .try_lamports()
            .map_err(|_| AdapterError::PositionAuthentication)?,
        destination_lamports: destination
            .try_lamports()
            .map_err(|_| AdapterError::PositionAuthentication)?,
        mint_lamports: mint
            .try_lamports()
            .map_err(|_| AdapterError::PositionAuthentication)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_transfer_post(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    before: TransferFacts,
    quantity: u64,
) -> Result<(), ProgramError> {
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
        || source.lamports() != before.source_lamports
        || destination.lamports() != before.destination_lamports
        || mint.lamports() != before.mint_lamports
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let mint_after = realm
        .release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let source_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &source_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let destination_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &destination_data)
        .map_err(|_| AdapterError::PositionPostcondition)?;
    let mut expected_source = before.source;
    expected_source.amount = expected_source
        .amount
        .checked_sub(quantity)
        .ok_or(AdapterError::PositionPostcondition)?;
    if before.authority_role == AuthorityRole::Delegate {
        expected_source.delegated_amount = expected_source
            .delegated_amount
            .checked_sub(quantity)
            .ok_or(AdapterError::PositionPostcondition)?;
    }
    let mut expected_destination = before.destination;
    expected_destination.amount = expected_destination
        .amount
        .checked_add(quantity)
        .ok_or(AdapterError::PositionPostcondition)?;
    if mint_after != realm.mint
        || source_after != expected_source
        || destination_after != expected_destination
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn checked_transfer_instruction(
    release: CollateralAdapterReleaseV1,
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    quantity: u64,
    decimals: u8,
) -> Result<Instruction, ProgramError> {
    let spec = transfer_checked(
        release.token_program(),
        source.to_bytes(),
        mint.to_bytes(),
        destination.to_bytes(),
        authority.to_bytes(),
        quantity,
        decimals,
    )
    .map_err(|_| AdapterError::PositionAuthentication)?;
    if spec.program_id() != &release.token_program() {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let expected = [
        (source.to_bytes(), false, true),
        (mint.to_bytes(), false, false),
        (destination.to_bytes(), false, true),
        (authority.to_bytes(), true, false),
    ];
    for (actual, (address, signer, writable)) in spec.accounts().iter().zip(expected) {
        if actual.address() != &address
            || actual.is_signer() != signer
            || actual.is_writable() != writable
        {
            return Err(AdapterError::PositionAuthentication.into());
        }
    }
    let mut accounts = Vec::new();
    accounts
        .try_reserve_exact(4)
        .map_err(|_| AdapterError::Arithmetic)?;
    accounts.push(AccountMeta::new(*source, false));
    accounts.push(AccountMeta::new_readonly(*mint, false));
    accounts.push(AccountMeta::new(*destination, false));
    accounts.push(AccountMeta::new_readonly(*authority, true));
    Ok(Instruction {
        program_id: Pubkey::new_from_array(release.token_program()),
        accounts,
        data: Vec::from(*spec.data()),
    })
}

fn authenticate_system_and_rent(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    authenticate_system(system)?;
    if rent.key != &sysvar::rent::ID || rent.owner != &sysvar::ID {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(())
}

fn authenticate_system(system: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if system.key != &system_program::ID || system.owner != &native_loader::ID {
        return Err(AdapterError::PositionAuthentication.into());
    }
    Ok(())
}

fn market_signer(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    root: MarketRoot,
) -> Result<MarketSignerFacts, ProgramError> {
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected, bump) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if market_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(MarketSignerFacts {
        identity_digest,
        bump,
    })
}

fn sweep_facts(
    account: &AccountInfo<'_>,
    token: TokenAccount,
) -> Result<SweepSurplusTokenAccountFactsV1, ProgramError> {
    SweepSurplusTokenAccountFactsV1::new(account.key.to_bytes(), token.mint, token.owner)
        .map_err(|_| AdapterError::PositionAuthentication.into())
}

fn encode_market<const N: usize>(market: CategoricalMarketV1<N>) -> Result<Vec<u8>, ProgramError> {
    let length = CategoricalMarketV1::<N>::encoded_len()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let mut bytes = exact_zeroed(length)?;
    market
        .encode(&mut bytes)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(bytes)
}

fn encode_position<const N: usize>(position: PositionV1<N>) -> Result<Vec<u8>, ProgramError> {
    let length =
        PositionV1::<N>::encoded_len().map_err(|_| AdapterError::PositionAuthentication)?;
    let mut bytes = exact_zeroed(length)?;
    position
        .encode(&mut bytes)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    Ok(bytes)
}

fn exact_zeroed(length: usize) -> Result<Vec<u8>, ProgramError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    output.resize(length, 0);
    Ok(output)
}

fn snapshot_data(account: &AccountInfo<'_>) -> Result<Vec<u8>, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let mut snapshot = Vec::new();
    snapshot
        .try_reserve_exact(data.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    snapshot.extend_from_slice(&data);
    Ok(snapshot)
}

fn persist_market_and_position<const N: usize>(
    market_account: &AccountInfo<'_>,
    market_bytes: &[u8],
    market: CategoricalMarketV1<N>,
    position_account: &AccountInfo<'_>,
    position_bytes: &[u8],
    position: PositionV1<N>,
) -> Result<(), ProgramError> {
    persist_market(market_account, market_bytes, market)?;
    {
        let mut output = position_account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::PositionPostcondition)?;
        if output.len() != position_bytes.len() {
            return Err(AdapterError::PositionPostcondition.into());
        }
        output.copy_from_slice(position_bytes);
    }
    let persisted = position_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if PositionV1::<N>::decode(&persisted) != Ok(position) || &persisted[..] != position_bytes {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn persist_market<const N: usize>(
    market_account: &AccountInfo<'_>,
    market_bytes: &[u8],
    market: CategoricalMarketV1<N>,
) -> Result<(), ProgramError> {
    {
        let mut output = market_account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::PositionPostcondition)?;
        if output.len() != market_bytes.len() {
            return Err(AdapterError::PositionPostcondition.into());
        }
        output.copy_from_slice(market_bytes);
    }
    let persisted = market_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if CategoricalMarketV1::<N>::decode(&persisted) != Ok(market) || &persisted[..] != market_bytes
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn preflight_mutable(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for account in accounts {
        drop(
            account
                .try_borrow_mut_lamports()
                .map_err(|_| AdapterError::PositionAuthentication)?,
        );
        drop(
            account
                .try_borrow_mut_data()
                .map_err(|_| AdapterError::PositionAuthentication)?,
        );
    }
    Ok(())
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::AccountFrameLength.into())
}
