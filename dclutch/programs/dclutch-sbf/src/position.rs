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
    SweepSurplusTokenAccountFactsV1, SweepSurplusV1, TransferClaimsV1,
    authorize_sweep_surplus_destination, validate_account_frame,
};
use dclutch_core_contract::{MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN, RealmV1};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
    SourceCloseCreditPlanV1,
};
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

const CREATE_ACCOUNTS: usize = 12;
const TRANSFER_ACCOUNTS: usize = 8;
const CLAIM_TRANSFER_ACCOUNTS: usize = 4;
const SWEEP_ACCOUNTS: usize = 6;
const CLOSE_ACCOUNTS: usize = 3;

const OWNER: usize = 0;
const MARKET: usize = 1;
const REALM: usize = 2;
const POSITION: usize = 3;
const VAULT: usize = 4;
const USER_TOKEN: usize = 5;
const MINT: usize = 6;
const TOKEN_PROGRAM: usize = 7;
const CREATE_PAYER: usize = 0;
const CREATE_OWNER: usize = 1;
const CREATE_MARKET: usize = 2;
const CREATE_REALM: usize = 3;
const CREATE_POSITION: usize = 4;
const CREATE_RENT_CREDIT: usize = 5;
const CREATE_VAULT: usize = 6;
const CREATE_SOURCE: usize = 7;
const CREATE_MINT: usize = 8;
const CREATE_TOKEN_PROGRAM: usize = 9;
const CREATE_SYSTEM_PROGRAM: usize = 10;
const RENT_SYSVAR: usize = 11;

const SWEEP_MARKET: usize = 0;
const SWEEP_REALM: usize = 1;
const SWEEP_VAULT: usize = 2;
const SWEEP_DESTINATION: usize = 3;
const SWEEP_MINT: usize = 4;
const SWEEP_TOKEN_PROGRAM: usize = 5;

const CLOSE_MARKET: usize = 0;
const CLOSE_POSITION: usize = 1;
const CLOSE_RENT_CREDIT: usize = 2;

const CLAIM_OWNER: usize = 0;
const CLAIM_MARKET: usize = 1;
const CLAIM_SOURCE: usize = 2;
const CLAIM_DESTINATION: usize = 3;

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
        CREATE_MARKET,
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

/// Move a selected nonzero vector of claims between two existing Positions.
///
/// This does not alter collateral, Hoard, Market bytes, or child accounting.
pub(crate) fn process_transfer_claims(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: TransferClaimsV1,
) -> Result<(), ProgramError> {
    validate_frame(
        InstructionTag::TransferClaims,
        accounts,
        CLAIM_TRANSFER_ACCOUNTS,
    )?;
    dispatch_width!(
        accounts,
        CLAIM_MARKET,
        transfer_claims,
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
    let payer = account(accounts, CREATE_PAYER)?;
    let owner = account(accounts, CREATE_OWNER)?;
    let market_account = account(accounts, CREATE_MARKET)?;
    let realm_account = account(accounts, CREATE_REALM)?;
    let position_account = account(accounts, CREATE_POSITION)?;
    let rent_credit_account = account(accounts, CREATE_RENT_CREDIT)?;
    let vault = account(accounts, CREATE_VAULT)?;
    let source = account(accounts, CREATE_SOURCE)?;
    let mint = account(accounts, CREATE_MINT)?;
    let token_program = account(accounts, CREATE_TOKEN_PROGRAM)?;
    let system = account(accounts, CREATE_SYSTEM_PROGRAM)?;
    let rent_sysvar = account(accounts, RENT_SYSVAR)?;

    authenticate_system_and_rent(system, rent_sysvar)?;
    if payer.owner != &system_program::ID
        || !payer.data_is_empty()
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
    let rent_credit = authenticate_rent_credit(program_id, rent_credit_account, owner.key)?;
    let rent_credit_lamports = rent_credit_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
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
    let payer_before = payer
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let payer_after = payer_before
        .checked_sub(position_rent)
        .ok_or(AdapterError::PositionRentUnderfunded)?;
    let position_space =
        u64::try_from(position_after_bytes.len()).map_err(|_| AdapterError::Arithmetic)?;
    let market_lamports = market_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let create = create_account(
        payer.key,
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
    preflight_mutable(&[payer, market_account, position_account, vault, source])?;

    let bump = [position_bump];
    let signer = [
        POSITION_PDA_DOMAIN,
        market_account.key.as_ref(),
        owner.key.as_ref(),
        bump.as_slice(),
    ];
    invoke_signed(
        &create,
        &[payer.clone(), position_account.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| AdapterError::PositionCreateCpi)?;
    if payer.lamports() != payer_after
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
        || payer.lamports() != payer_after
        || rent_credit_account.lamports() != rent_credit_lamports
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit_account, rent_credit)?;
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

fn transfer_claims<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: TransferClaimsV1,
) -> Result<(), ProgramError> {
    let owner = account(accounts, CLAIM_OWNER)?;
    let market_account = account(accounts, CLAIM_MARKET)?;
    let source_account = account(accounts, CLAIM_SOURCE)?;
    let destination_account = account(accounts, CLAIM_DESTINATION)?;
    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    if !matches!(
        market.root().phase(),
        Phase::Open | Phase::Resolved | Phase::Retiring
    ) || usize::from(instruction.outcome_count()) != N
    {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let source = authenticate_position::<N>(
        program_id,
        source_account,
        market_account,
        owner,
        instruction.generation(),
    )?;
    let destination = authenticate_position_from_stored_owner::<N>(
        program_id,
        destination_account,
        market_account,
        instruction.generation(),
    )?;
    if source_account.key == destination_account.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let (source_after, destination_after) =
        transfer_position_claims(source, destination, instruction.quantities())?;
    let source_bytes = encode_position(source_after)?;
    let destination_bytes = encode_position(destination_after)?;
    let market_data = snapshot_data(market_account)?;
    let market_lamports = market_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let source_lamports = source_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let destination_lamports = destination_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    // Plan every checked debit/credit and obtain all mutable borrows before
    // either Position is written, so a refusal has no partial local effect.
    preflight_mutable(&[source_account, destination_account])?;
    persist_position(source_account, &source_bytes, source_after)?;
    persist_position(destination_account, &destination_bytes, destination_after)?;
    if snapshot_data(market_account)? != market_data
        || market_account.lamports() != market_lamports
        || source_account.lamports() != source_lamports
        || destination_account.lamports() != destination_lamports
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
}

fn transfer_position_claims<const N: usize>(
    source: PositionV1<N>,
    destination: PositionV1<N>,
    quantities: [u64; dclutch_realm_contract::MAX_OUTCOMES],
) -> Result<(PositionV1<N>, PositionV1<N>), ProgramError> {
    let mut source_after = source;
    let mut destination_after = destination;
    for outcome in 0..N {
        let quantity = *quantities
            .get(outcome)
            .ok_or(AdapterError::PositionAuthentication)?;
        if quantity == 0 {
            continue;
        }
        source_after
            .debit_outcome(outcome, quantity)
            .map_err(|_| AdapterError::MarketTransition)?;
        destination_after
            .credit_outcome(outcome, quantity)
            .map_err(|_| AdapterError::MarketTransition)?;
    }
    Ok((source_after, destination_after))
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
    let market_account = account(accounts, CLOSE_MARKET)?;
    let position_account = account(accounts, CLOSE_POSITION)?;
    let rent_credit_account = account(accounts, CLOSE_RENT_CREDIT)?;
    let market = authenticate_market::<N>(program_id, market_account, instruction.generation())?;
    if !matches!(
        market.root().phase(),
        Phase::Open | Phase::Resolved | Phase::Retiring
    ) || market.root().outstanding_children() != instruction.child_count()
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let position = authenticate_position_from_stored_owner::<N>(
        program_id,
        position_account,
        market_account,
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
    let owner = Pubkey::new_from_array(*position.owner());
    let rent_credit = authenticate_rent_credit(program_id, rent_credit_account, &owner)?;
    let market_lamports = market_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let position_lamports = position_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let rent_credit_lamports = rent_credit_account
        .try_lamports()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let credit_plan =
        SourceCloseCreditPlanV1::new(position_lamports, rent_credit_lamports, position_lamports)
            .map_err(|_| AdapterError::Arithmetic)?;
    // All identity, state-transition, arithmetic, and mutable-borrow checks
    // precede the first write. A failed transaction remains rollback-safe.
    preflight_mutable(&[market_account, position_account, rent_credit_account])?;

    persist_market(market_account, &market_after_bytes, market_after)?;
    {
        let mut rent_credit_lamports = rent_credit_account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionClose)?;
        let mut position_balance = position_account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::PositionClose)?;
        **rent_credit_lamports = credit_plan.credit_after();
        **position_balance = 0;
    }
    // The System Program has no close instruction for a program-owned account.
    // dClutch drains its own Position, removes its data while still owner, and
    // then restores the vacant account to System ownership without a System
    // Program account in this permissionless frame.
    position_account
        .resize(0)
        .map_err(|_| AdapterError::PositionClose)?;
    position_account.assign(&system_program::ID);
    credit_plan
        .validate_post(position_account.lamports(), rent_credit_account.lamports())
        .map_err(|_| AdapterError::PositionPostcondition)?;
    require_unchanged_rent_credit(program_id, rent_credit_account, rent_credit)?;
    if market_account.lamports() != market_lamports
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
    require_alias_policy(tag, accounts)
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

/// Authenticate a Position for permissionless retirement.  The immutable
/// owner stored in the canonical Position bytes, rather than a caller account,
/// selects both the Position and permanent RentCredit PDAs.
fn authenticate_position_from_stored_owner<const N: usize>(
    program_id: &Pubkey,
    position_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    generation: u64,
) -> Result<PositionV1<N>, ProgramError> {
    if position_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = position_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let position =
        PositionV1::<N>::decode(&data).map_err(|_| AdapterError::PositionAuthentication)?;
    if position.market() != market_account.key.as_ref() || position.generation() != generation {
        return Err(AdapterError::PositionAuthentication.into());
    }
    let (expected, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            market_account.key.as_ref(),
            position.owner().as_slice(),
        ],
        program_id,
    );
    if position_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let encoded = encode_position(position)?;
    if encoded.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(position)
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    rent_credit_account: &AccountInfo<'_>,
    owner: &Pubkey,
) -> Result<RentCreditV1, ProgramError> {
    let authority =
        RefundAuthority::new(owner.to_bytes()).map_err(|_| AdapterError::PositionAuthentication)?;
    let authority_bytes = authority.to_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        program_id,
    );
    if rent_credit_account.key != &expected
        || rent_credit_account.owner != program_id
        || rent_credit_account.executable
        || rent_credit_account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = rent_credit_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionAuthentication)?;
    let rent_credit =
        RentCreditV1::decode(&data).map_err(|_| AdapterError::PositionAuthentication)?;
    rent_credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::PositionAuthentication)?;
    if rent_credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(rent_credit)
}

fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    rent_credit_account: &AccountInfo<'_>,
    expected: RentCreditV1,
) -> Result<(), ProgramError> {
    if rent_credit_account.owner != program_id
        || rent_credit_account.executable
        || rent_credit_account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::PositionPostcondition.into());
    }
    let data = rent_credit_account
        .try_borrow_data()
        .map_err(|_| AdapterError::PositionPostcondition)?;
    if RentCreditV1::decode(&data) != Ok(expected) || expected.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::PositionPostcondition.into());
    }
    Ok(())
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
    persist_position(position_account, position_bytes, position)
}

fn persist_position<const N: usize>(
    position_account: &AccountInfo<'_>,
    position_bytes: &[u8],
    position: PositionV1<N>,
) -> Result<(), ProgramError> {
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

fn require_alias_policy(
    tag: InstructionTag,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if tag == InstructionTag::CreatePositionAndSplit {
        let payer = account(accounts, CREATE_PAYER)?;
        let owner = account(accounts, CREATE_OWNER)?;
        let rent_credit = account(accounts, CREATE_RENT_CREDIT)?;
        // The owner is logically readonly when distinct.  Only the permitted
        // payer=owner alias can union the payer's writable privilege into it.
        if rent_credit.is_signer
            || rent_credit.is_writable
            || !payer.is_signer
            || !payer.is_writable
            || !owner.is_signer
            || (payer.key != owner.key && owner.is_writable)
            || (payer.key == owner.key && (!owner.is_writable || !payer.is_writable))
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    for (index, account) in accounts.iter().enumerate() {
        for (other_index, other) in accounts.iter().enumerate().skip(index.saturating_add(1)) {
            if account.key != other.key {
                continue;
            }
            if tag == InstructionTag::CreatePositionAndSplit
                && index == CREATE_PAYER
                && other_index == CREATE_OWNER
            {
                // Solana presents aliased metas with the union of their
                // privileges: this key must therefore be signer+writable,
                // satisfying both the payer and owner roles.
                if account.is_signer && account.is_writable && other.is_signer && other.is_writable
                {
                    continue;
                }
            }
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

#[cfg(test)]
mod tests {
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

    fn test_account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn creation_accounts(payer: Pubkey, owner: Pubkey) -> [AccountInfo<'static>; CREATE_ACCOUNTS] {
        let merged = payer == owner;
        [
            test_account(payer, true, true, 1, vec![], system_program::ID, false),
            test_account(owner, true, merged, 1, vec![], Pubkey::new_unique(), false),
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                false,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                false,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                true,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                false,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                false,
                0,
                vec![],
                Pubkey::new_unique(),
                true,
            ),
            test_account(
                system_program::ID,
                false,
                false,
                0,
                vec![],
                native_loader::ID,
                true,
            ),
            test_account(sysvar::rent::ID, false, false, 0, vec![], sysvar::ID, false),
        ]
    }

    fn rent_credit_account(
        program_id: Pubkey,
        authority: Pubkey,
        lamports: u64,
    ) -> AccountInfo<'static> {
        let (key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, authority.as_ref()],
            &program_id,
        );
        let authority = RefundAuthority::new(authority.to_bytes()).expect("nonzero authority");
        test_account(
            key,
            false,
            true,
            lamports,
            RentCreditV1::new(authority, bump).to_bytes().to_vec(),
            program_id,
            false,
        )
    }

    fn position<const N: usize>(
        market: Pubkey,
        owner: Pubkey,
        generation: u64,
        balances: [u64; N],
    ) -> PositionV1<N> {
        PositionV1::new(market.to_bytes(), owner.to_bytes(), generation, balances)
            .expect("nonzero test Position")
    }

    fn transfer_accounts(owner_signer: bool, alias_positions: bool) -> [AccountInfo<'static>; 4] {
        let source = Pubkey::new_unique();
        let destination = if alias_positions {
            source
        } else {
            Pubkey::new_unique()
        };
        [
            test_account(
                Pubkey::new_unique(),
                owner_signer,
                false,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(
                Pubkey::new_unique(),
                false,
                false,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
            test_account(source, false, true, 0, vec![], Pubkey::new_unique(), false),
            test_account(
                destination,
                false,
                true,
                0,
                vec![],
                Pubkey::new_unique(),
                false,
            ),
        ]
    }

    fn position_account<const N: usize>(
        program_id: Pubkey,
        market: Pubkey,
        owner: Pubkey,
        generation: u64,
        balances: [u64; N],
    ) -> AccountInfo<'static> {
        let position = position(market, owner, generation, balances);
        let data = encode_position(position).expect("canonical test Position encoding");
        let (key, _) = Pubkey::find_program_address(
            &[POSITION_PDA_DOMAIN, market.as_ref(), owner.as_ref()],
            &program_id,
        );
        test_account(key, false, true, 1, data, program_id, false)
    }

    #[test]
    fn third_party_payer_and_pda_owner_are_distinct_valid_roles() {
        let payer = Pubkey::new_unique();
        let (owner, _) = Pubkey::find_program_address(&[b"test-owner"], &Pubkey::new_unique());
        let accounts = creation_accounts(payer, owner);
        assert_eq!(
            validate_frame(
                InstructionTag::CreatePositionAndSplit,
                &accounts,
                CREATE_ACCOUNTS
            ),
            Ok(())
        );
    }

    #[test]
    fn payer_owner_alias_requires_runtime_privilege_union() {
        let shared = Pubkey::new_unique();
        let accounts = creation_accounts(shared, shared);
        assert_eq!(
            validate_frame(
                InstructionTag::CreatePositionAndSplit,
                &accounts,
                CREATE_ACCOUNTS
            ),
            Ok(())
        );

        let payer = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mut invalid = creation_accounts(payer, owner);
        invalid[1] = test_account(owner, true, true, 1, vec![], Pubkey::new_unique(), false);
        assert_eq!(
            validate_frame(
                InstructionTag::CreatePositionAndSplit,
                &invalid,
                CREATE_ACCOUNTS
            ),
            Err(AdapterError::AccountIdentity.into())
        );
    }

    #[test]
    fn wrong_credit_or_beneficiary_redirect_refuses_before_any_close_write() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let other_owner = Pubkey::new_unique();
        let wrong_credit = rent_credit_account(program_id, other_owner, 19);
        assert!(authenticate_rent_credit(&program_id, &wrong_credit, &owner).is_err());

        // The exact plan refuses a partial redirect before an adapter can
        // mutate the Position, RentCredit, or Market accounts.
        assert!(SourceCloseCreditPlanV1::new(20, 4, 19).is_err());
        let plan = SourceCloseCreditPlanV1::new(20, 4, 20).expect("full credit only");
        assert!(plan.validate_post(1, 24).is_err());
    }

    #[test]
    fn transfer_claims_requires_signing_source_owner_and_distinct_positions() {
        let unsigned = transfer_accounts(false, false);
        assert_eq!(
            validate_frame(
                InstructionTag::TransferClaims,
                &unsigned,
                CLAIM_TRANSFER_ACCOUNTS
            ),
            Err(AdapterError::AccountPrivilege.into())
        );
        let aliased = transfer_accounts(true, true);
        assert_eq!(
            validate_frame(
                InstructionTag::TransferClaims,
                &aliased,
                CLAIM_TRANSFER_ACCOUNTS
            ),
            Err(AdapterError::AccountIdentity.into())
        );
    }

    #[test]
    fn transfer_claims_checked_vector_refuses_underflow_overflow_and_keeps_inputs() {
        let market = Pubkey::new_unique();
        let source = position(market, Pubkey::new_unique(), 4, [7, 9]);
        let destination = position(market, Pubkey::new_unique(), 4, [11, 13]);
        let mut quantities = [0; dclutch_realm_contract::MAX_OUTCOMES];
        quantities[0] = 3;
        quantities[1] = 4;
        let (source_after, destination_after) =
            transfer_position_claims(source, destination, quantities).expect("checked transfer");
        assert_eq!(source_after.balances(), &[4, 5]);
        assert_eq!(destination_after.balances(), &[14, 17]);

        let original_source = source;
        let original_destination = destination;
        quantities[0] = 8;
        assert!(transfer_position_claims(source, destination, quantities).is_err());
        assert_eq!(source, original_source);
        assert_eq!(destination, original_destination);

        let destination_at_max = position(market, Pubkey::new_unique(), 4, [u64::MAX, 13]);
        quantities[0] = 1;
        quantities[1] = 0;
        assert!(transfer_position_claims(source, destination_at_max, quantities).is_err());
    }

    #[test]
    fn transfer_position_authentication_refuses_wrong_market_generation_and_nonowner() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let position = position_account(program_id, market, owner, 4, [7, 9]);
        let market_account =
            test_account(market, false, false, 0, vec![], Pubkey::new_unique(), false);
        let owner_account =
            test_account(owner, true, false, 0, vec![], Pubkey::new_unique(), false);
        assert!(
            authenticate_position::<2>(&program_id, &position, &market_account, &owner_account, 4)
                .is_ok()
        );

        let wrong_market = test_account(
            Pubkey::new_unique(),
            false,
            false,
            0,
            vec![],
            Pubkey::new_unique(),
            false,
        );
        assert!(
            authenticate_position::<2>(&program_id, &position, &wrong_market, &owner_account, 4)
                .is_err()
        );
        assert!(
            authenticate_position::<2>(&program_id, &position, &market_account, &owner_account, 5)
                .is_err()
        );
        let nonowner = test_account(
            Pubkey::new_unique(),
            true,
            false,
            0,
            vec![],
            Pubkey::new_unique(),
            false,
        );
        assert!(
            authenticate_position::<2>(&program_id, &position, &market_account, &nonowner, 4)
                .is_err()
        );
    }
}
