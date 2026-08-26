//! Physical pre-founding Hoard custody rooted in Core ProjectFound return data.

use alloc::{vec, vec::Vec};
use core::convert::TryFrom;

use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CUSTODY_REPLAY_PDA_DOMAIN_V1,
    CompartmentV1, CustodyReplayV1, CustodyVaultSeedsV1,
    PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1, PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1,
    PROJECTED_CUSTODY_STATE_BYTES_V1, ProjectedCustodyCallerSeedsV1, ProjectedCustodyLockReceiptV1,
    ProjectedCustodyOperationV1, ProjectedCustodyReceiptV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateSeedsV1, ProjectedCustodyStateV1, normal_replay_from_realization_v1,
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity, ProjectFoundReceiptV1, ProjectFoundRequestV1, Request, STATE_BYTES,
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_token_svm::{
    ACCOUNT_BYTES, ExactTransferInput, ExactTransferProfileV1, PRODUCTION_ADAPTER_RELEASES,
    close_account, initialize_account3, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{CustodySbfError, token_instruction};

const CALLER: usize = 0;
const STATE: usize = 1;
const CACHE: usize = 2;
const REGISTRY: usize = 3;
const CALLER_PROGRAM: usize = 4;
const CALLER_PROGRAMDATA: usize = 5;
const RENT_CREDIT: usize = 6;

const INITIALIZE_ACCOUNTS: usize = 42;
const INITIALIZE_CORE_PROGRAM: usize = 7;
const INITIALIZE_PAYER: usize = 8;
const INITIALIZE_RENT: usize = 9;
const INITIALIZE_SYSTEM: usize = 10;
const INITIALIZE_FOUND_START: usize = 11;

const OPEN_ACCOUNTS: usize = 15;
const OPEN_VAULT: usize = 7;
const OPEN_AUTHORITY: usize = 8;
const OPEN_MINT: usize = 9;
const OPEN_TOKEN_PROGRAM: usize = 10;
const OPEN_PAYER: usize = 11;
const OPEN_RENT: usize = 12;
const OPEN_SYSTEM: usize = 13;
const OPEN_MARKET: usize = 14;

const LOCK_ACCOUNTS: usize = 14;
const LOCK_VAULT: usize = 7;
const LOCK_SOURCE: usize = 8;
const LOCK_REFUND_OWNER: usize = 9;
const LOCK_AUTHORITY: usize = 10;
const LOCK_MINT: usize = 11;
const LOCK_TOKEN_PROGRAM: usize = 12;
const LOCK_MARKET: usize = 13;

const REFUND_ACCOUNTS: usize = 14;
const REFUND_VAULT: usize = 7;
const REFUND_DESTINATION: usize = 8;
const REFUND_OWNER: usize = 9;
const REFUND_AUTHORITY: usize = 10;
const REFUND_MINT: usize = 11;
const REFUND_TOKEN_PROGRAM: usize = 12;
const REFUND_MARKET: usize = 13;

const REALIZE_ACCOUNTS: usize = PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1;
const REALIZE_VAULT: usize = 7;
const REALIZE_MARKET: usize = 8;
const REALIZE_AUTHORITY: usize = 9;
const REALIZE_MINT: usize = 10;
const REALIZE_TOKEN_PROGRAM: usize = 11;

const ABORT_ACCOUNTS: usize = 11;
const ABORT_VAULT: usize = 7;
const ABORT_AUTHORITY: usize = 8;
const ABORT_TOKEN_PROGRAM: usize = 9;
const ABORT_MARKET: usize = 10;

const LOCK_CLOSE_ACCOUNTS: usize = PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1;
const LOCK_CLOSE_HOARD: usize = 7;
const LOCK_CLOSE_SOURCE: usize = 8;
const LOCK_CLOSE_AUTHORITY: usize = 9;
const LOCK_CLOSE_MINT: usize = 10;
const LOCK_CLOSE_TOKEN_PROGRAM: usize = 11;
const LOCK_CLOSE_SOURCE_REPLAY: usize = 12;
const LOCK_CLOSE_MARKET: usize = 13;

/// Execute one exact projected custody action.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    require_count(accounts, request.operation)?;
    let request_digest = hash(request_bytes).to_bytes();
    authenticate_common(program_id, accounts, request, request_digest)?;
    match request.operation {
        ProjectedCustodyOperationV1::Initialize => {
            initialize(program_id, accounts, request, request_digest)
        }
        ProjectedCustodyOperationV1::OpenHoard => {
            open_hoard(program_id, accounts, request, request_digest)
        }
        ProjectedCustodyOperationV1::LockHoard => {
            lock_hoard(program_id, accounts, request, request_digest)
        }
        ProjectedCustodyOperationV1::RefundAndClose => {
            refund_and_close(program_id, accounts, request, request_digest)
        }
        ProjectedCustodyOperationV1::RealizeAndClose => {
            realize_and_close(program_id, accounts, request, request_digest)
        }
        ProjectedCustodyOperationV1::AbortOpenAndClose => {
            abort_open_and_close(program_id, accounts, request, request_digest)
        }
        ProjectedCustodyOperationV1::LockHoardAndCloseSource => {
            lock_hoard_and_close_source(program_id, accounts, request, request_digest)
        }
    }
}

#[inline(never)]
fn authenticate_common(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let caller = account(accounts, CALLER)?;
    let state = account(accounts, STATE)?;
    let cache = account(accounts, CACHE)?;
    let registry = account(accounts, REGISTRY)?;
    let caller_program = account(accounts, CALLER_PROGRAM)?;
    let caller_programdata = account(accounts, CALLER_PROGRAMDATA)?;
    let rent_credit = account(accounts, RENT_CREDIT)?;
    if !caller.is_signer
        || caller.is_writable
        || caller.executable
        || state.is_signer
        || !state.is_writable
        || state.executable
        || cache.is_signer
        || cache.is_writable
        || cache.executable
        || registry.is_signer
        || registry.is_writable
        || !registry.executable
        || caller_program.is_signer
        || caller_program.is_writable
        || !caller_program.executable
        || caller_programdata.is_signer
        || caller_programdata.is_writable
        || caller_programdata.executable
        || rent_credit.is_signer
        || rent_credit.executable
        || caller_program.key.to_bytes() != request.caller_program
        || registry.key == caller_program.key
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let terminal = matches!(
        request.operation,
        ProjectedCustodyOperationV1::RefundAndClose
            | ProjectedCustodyOperationV1::AbortOpenAndClose
            | ProjectedCustodyOperationV1::LockHoardAndCloseSource
    );
    if rent_credit.is_writable != terminal {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let state_seeds = ProjectedCustodyStateSeedsV1::from_request(request);
    if Pubkey::find_program_address(&state_seeds.as_slices(), program_id).0 != *state.key {
        return Err(CustodySbfError::Replay.into());
    }
    if request.operation == ProjectedCustodyOperationV1::Initialize {
        if state.owner != &system_program::ID || state.data_len() != 0 {
            return Err(CustodySbfError::Replay.into());
        }
    } else if state.owner != program_id || state.data_len() != PROJECTED_CUSTODY_STATE_BYTES_V1 {
        return Err(CustodySbfError::Replay.into());
    }
    let caller_seeds = ProjectedCustodyCallerSeedsV1::new(request, request_digest);
    if Pubkey::find_program_address(&caller_seeds.as_slices(), caller_program.key).0 != *caller.key
    {
        return Err(CustodySbfError::CallerAuthority.into());
    }
    authenticate_release(
        program_id,
        cache,
        registry,
        caller_program,
        caller_programdata,
        request,
    )?;
    authenticate_rent_credit(rent_credit, request)?;
    Ok(())
}

#[inline(never)]
fn authenticate_release<'info>(
    program_id: &Pubkey,
    cache: &AccountInfo<'info>,
    registry: &AccountInfo<'info>,
    caller_program: &AccountInfo<'info>,
    caller_programdata: &AccountInfo<'info>,
    request: ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &request.release_set],
        registry.key,
    )
    .0;
    if cache.key != &expected_cache || cache.owner != registry.key {
        return Err(CustodySbfError::Release.into());
    }
    let cache_data = cache
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| CustodySbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| CustodySbfError::Release)?
        .as_bytes()
        != &request.release_set
        || activated
            .role(ExecutionRoleV1::Custody)
            .map_err(|_| CustodySbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &program_id.to_bytes()
        || activated
            .role(ExecutionRoleV1::Core)
            .map_err(|_| CustodySbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &request.core_program
    {
        return Err(CustodySbfError::Release.into());
    }
    drop(cache_data);
    let instruction = Instruction {
        program_id: *registry.key,
        accounts: vec![
            AccountMeta::new_readonly(*cache.key, false),
            AccountMeta::new_readonly(*caller_program.key, false),
            AccountMeta::new_readonly(*caller_programdata.key, false),
        ],
        data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Trading)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            cache.clone(),
            caller_program.clone(),
            caller_programdata.clone(),
            registry.clone(),
        ],
    )
    .map_err(|_| CustodySbfError::Release)?;
    let (producer, bytes) = get_return_data().ok_or(CustodySbfError::Release)?;
    let receipt =
        AuthenticatedRoleReceiptV1::decode(&bytes).map_err(|_| CustodySbfError::Release)?;
    if producer != *registry.key
        || receipt.role() != ExecutionRoleV1::Trading
        || receipt.execution_release_set_id().as_bytes() != &request.release_set
        || receipt.program().as_bytes() != &request.caller_program
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(())
}

fn authenticate_rent_credit(
    account: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    if account.key.to_bytes() != request.rent_credit
        || account.owner.to_bytes() != request.rent_program
    {
        return Err(CustodySbfError::Create.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Create)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| CustodySbfError::Create)?;
    validate_lifecycle_credit_binding(
        credit,
        request.market,
        request.release_set,
        request.generation,
    )?;
    let seeds = credit.pda_seeds();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[seeds.domain(), &market, &generation, &bump],
        account.owner,
    )
    .map_err(|_| CustodySbfError::Create)?;
    if expected != *account.key {
        return Err(CustodySbfError::Create.into());
    }
    Ok(())
}

fn validate_lifecycle_credit_binding(
    credit: LifecycleRentCreditV2,
    market: [u8; 32],
    release_set: [u8; 32],
    generation: u64,
) -> Result<(), ProgramError> {
    if credit.market().to_bytes() != market
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != generation
    {
        Err(CustodySbfError::Create.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod lifecycle_rent_tests {
    use dclutch_rent_contract::{RefundAuthority, lifecycle_v2::LifecycleAccountIdV2};

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn projected_custody_refuses_cross_release_credit_reuse() {
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(id(1)).expect("refund wallet"),
            LifecycleAccountIdV2::new(id(2)).expect("Market"),
            LifecycleAccountIdV2::new(id(3)).expect("release set"),
            4,
            5,
        )
        .expect("lifecycle credit");
        assert_eq!(
            validate_lifecycle_credit_binding(credit, id(2), id(3), 4),
            Ok(())
        );
        assert_eq!(
            validate_lifecycle_credit_binding(credit, id(2), id(6), 4),
            Err(CustodySbfError::Create.into())
        );
        assert_eq!(
            validate_lifecycle_credit_binding(credit, id(2), id(3), 5),
            Err(CustodySbfError::Create.into())
        );
    }
}

#[inline(never)]
fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let core_program = account(accounts, INITIALIZE_CORE_PROGRAM)?;
    let payer = account(accounts, INITIALIZE_PAYER)?;
    let rent_account = account(accounts, INITIALIZE_RENT)?;
    let system = account(accounts, INITIALIZE_SYSTEM)?;
    if core_program.key.to_bytes() != request.core_program
        || !core_program.executable
        || !payer.is_signer
        || !payer.is_writable
        || payer.executable
        || payer.key.to_bytes() != request.payer
        || rent_account.key != &sysvar::rent::ID
        || rent_account.is_writable
        || rent_account.is_signer
        || rent_account.executable
        || system.key != &system_program::ID
        || !system.executable
        || system.is_writable
        || system.is_signer
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let found_accounts = accounts
        .get(INITIALIZE_FOUND_START..INITIALIZE_FOUND_START + 31)
        .ok_or(CustodySbfError::AccountFrame)?;
    if found_accounts
        .get(1)
        .ok_or(CustodySbfError::AccountFrame)?
        .key
        .to_bytes()
        != request.market
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let found = Request::administrative(
        Action::Found,
        request.generation,
        Identity::new(request.market).map_err(|_| CustodySbfError::Release)?,
    );
    let projection_request = ProjectFoundRequestV1::new(found)
        .map_err(|_| CustodySbfError::Release)?
        .encode()
        .map_err(|_| CustodySbfError::Release)?;
    let metas = found_accounts
        .iter()
        .map(|info| AccountMeta::new_readonly(*info.key, false))
        .collect::<Vec<_>>();
    let instruction = Instruction {
        program_id: *core_program.key,
        accounts: metas,
        data: projection_request.to_vec(),
    };
    let mut infos = found_accounts.to_vec();
    infos.push(core_program.clone());
    invoke(&instruction, &infos).map_err(|_| CustodySbfError::Release)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(CustodySbfError::Release)?;
    if producer != *core_program.key
        || hash(&receipt_bytes).to_bytes() != request.projection_receipt_digest
    {
        return Err(CustodySbfError::Release.into());
    }
    let projection =
        ProjectFoundReceiptV1::decode(&receipt_bytes).map_err(|_| CustodySbfError::Release)?;
    projection
        .verify_found_request(
            hash(&found.encode().map_err(|_| CustodySbfError::Release)?).to_bytes(),
        )
        .map_err(|_| CustodySbfError::Release)?;
    let rent = Rent::from_account_info(rent_account).map_err(|_| CustodySbfError::Create)?;
    let lamports = rent.minimum_balance(PROJECTED_CUSTODY_STATE_BYTES_V1);
    if lamports != request.state_rent_lamports {
        return Err(CustodySbfError::Create.into());
    }
    let state_account = account(accounts, STATE)?;
    let seeds = ProjectedCustodyStateSeedsV1::from_request(request);
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release, context] = seeds.as_slices();
    top_up_allocate_assign(
        payer,
        state_account,
        system,
        lamports,
        PROJECTED_CUSTODY_STATE_BYTES_V1,
        program_id,
        &[domain, market, release, context, &bump_seed],
    )?;
    let current_slot = Clock::get().map_err(|_| CustodySbfError::Create)?.slot;
    let state = ProjectedCustodyStateV1::initialize(
        request,
        projection,
        producer.to_bytes(),
        hash(&receipt_bytes).to_bytes(),
        request_digest,
        current_slot,
        true,
        bump,
    )
    .map_err(|_| CustodySbfError::Replay)?;
    commit_state(state_account, state)
}

#[inline(never)]
fn open_hoard(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let vault = account(accounts, OPEN_VAULT)?;
    let authority = account(accounts, OPEN_AUTHORITY)?;
    let mint = account(accounts, OPEN_MINT)?;
    let token_program = account(accounts, OPEN_TOKEN_PROGRAM)?;
    let payer = account(accounts, OPEN_PAYER)?;
    let rent_account = account(accounts, OPEN_RENT)?;
    let system = account(accounts, OPEN_SYSTEM)?;
    require_vacant_market(account(accounts, OPEN_MARKET)?, request)?;
    authenticate_token_frame(
        program_id,
        vault,
        authority,
        mint,
        token_program,
        request,
        true,
    )?;
    if vault.owner != &system_program::ID
        || vault.data_len() != 0
        || !vault.is_writable
        || !payer.is_signer
        || !payer.is_writable
        || payer.key.to_bytes() != request.payer
        || rent_account.key != &sysvar::rent::ID
        || system.key != &system_program::ID
        || !system.executable
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| CustodySbfError::Create)?;
    let lamports = rent.minimum_balance(ACCOUNT_BYTES);
    if lamports != request.vault_rent_lamports {
        return Err(CustodySbfError::Create.into());
    }
    let seeds = CustodyVaultSeedsV1::new(
        request.market,
        request.release_set,
        request.context_digest,
        CompartmentV1::HoardPrincipal,
    );
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release, context, compartment] = seeds.as_slices();
    top_up_allocate_assign(
        payer,
        vault,
        system,
        lamports,
        ACCOUNT_BYTES,
        token_program.key,
        &[domain, market, release, context, compartment, &bump_seed],
    )?;
    let spec = initialize_account3(
        request.token_program,
        request.hoard_vault,
        request.mint,
        authority.key.to_bytes(),
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    invoke(
        &token_instruction(&spec),
        &[vault.clone(), mint.clone(), token_program.clone()],
    )
    .map_err(|_| CustodySbfError::TokenCpi)?;
    let amount = read_vault_amount(vault, authority, request)?;
    let current = read_state(account(accounts, STATE)?)?;
    let next = current
        .open_hoard(request, request_digest, amount, true)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_state(account(accounts, STATE)?, next)
}

#[inline(never)]
fn lock_hoard(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let vault = account(accounts, LOCK_VAULT)?;
    let source = account(accounts, LOCK_SOURCE)?;
    let source_authority = account(accounts, LOCK_REFUND_OWNER)?;
    let authority = account(accounts, LOCK_AUTHORITY)?;
    let mint = account(accounts, LOCK_MINT)?;
    let token_program = account(accounts, LOCK_TOKEN_PROGRAM)?;
    require_vacant_market(account(accounts, LOCK_MARKET)?, request)?;
    authenticate_token_frame(
        program_id,
        vault,
        authority,
        mint,
        token_program,
        request,
        false,
    )?;
    if !source.is_writable
        || !vault.is_writable
        || source.owner != token_program.key
        || source.key.to_bytes() != request.funding_source_vault
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let source_seeds = CustodyVaultSeedsV1::new(
        request.market,
        request.release_set,
        request.funding_source_context,
        request.funding_source_compartment,
    );
    let expected_source = Pubkey::find_program_address(&source_seeds.as_slices(), program_id).0;
    if source.key != &expected_source
        || source_authority.key != authority.key
        || source_authority.is_signer
        || source_authority.is_writable
        || source_authority.executable
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let profile = collateral_profile(request)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let vault_data = vault
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let mint_facts = profile
        .check_mint(request.token_program, &mint_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    let facts = profile
        .check_transfer(ExactTransferInput {
            program_id: request.token_program,
            mint_address: request.mint,
            mint_data: &mint_data,
            source_data: &source_data,
            destination_data: &vault_data,
            authority: authority.key.to_bytes(),
            amount: request.amount,
            decimals: mint_facts.decimals,
        })
        .map_err(|_| CustodySbfError::TokenState)?;
    if facts.destination().owner != authority.key.to_bytes() {
        return Err(CustodySbfError::TokenState.into());
    }
    let before = facts.destination().amount;
    let source_before = facts.source().amount;
    drop(vault_data);
    drop(source_data);
    drop(mint_data);
    let spec = transfer_checked(
        request.token_program,
        source.key.to_bytes(),
        request.mint,
        request.hoard_vault,
        authority.key.to_bytes(),
        request.amount,
        mint_facts.decimals,
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let authority_bump = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        program_id,
    )
    .1;
    let authority_bump_seed = [authority_bump];
    invoke_signed(
        &token_instruction(&spec),
        &[
            source.clone(),
            mint.clone(),
            vault.clone(),
            source_authority.clone(),
            token_program.clone(),
        ],
        &[&[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
            &authority_bump_seed,
        ]],
    )
    .map_err(|_| CustodySbfError::TokenCpi)?;
    let after = read_vault_amount(vault, authority, request)?;
    let source_after = {
        let bytes = source
            .try_borrow_data()
            .map_err(|_| CustodySbfError::TokenState)?;
        profile
            .check_transfer_account(request.token_program, &bytes)
            .map_err(|_| CustodySbfError::TokenState)?
            .amount
    };
    if source_before.checked_sub(request.amount) != Some(source_after) {
        return Err(CustodySbfError::Postcondition.into());
    }
    let state = read_state(account(accounts, STATE)?)?;
    let next = state
        .lock_hoard(request, request_digest, before, after, true)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_state(account(accounts, STATE)?, next)
}

#[inline(never)]
fn refund_and_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let vault = account(accounts, REFUND_VAULT)?;
    let destination = account(accounts, REFUND_DESTINATION)?;
    let owner = account(accounts, REFUND_OWNER)?;
    let authority = account(accounts, REFUND_AUTHORITY)?;
    let mint = account(accounts, REFUND_MINT)?;
    let token_program = account(accounts, REFUND_TOKEN_PROGRAM)?;
    require_vacant_market(account(accounts, REFUND_MARKET)?, request)?;
    authenticate_token_frame(
        program_id,
        vault,
        authority,
        mint,
        token_program,
        request,
        false,
    )?;
    if !destination.is_writable
        || !vault.is_writable
        || destination.owner != token_program.key
        || !owner.is_signer
        || owner.key.to_bytes() != request.refund_owner
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let before = read_vault_amount(vault, authority, request)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let profile = collateral_profile(request)?;
    let decimals = profile
        .check_mint(request.token_program, &mint_data)
        .map_err(|_| CustodySbfError::TokenState)?
        .decimals;
    let destination_before = {
        let bytes = destination
            .try_borrow_data()
            .map_err(|_| CustodySbfError::TokenState)?;
        let token = profile
            .check_transfer_account(request.token_program, &bytes)
            .map_err(|_| CustodySbfError::TokenState)?;
        if token.mint != request.mint || token.owner != request.refund_owner {
            return Err(CustodySbfError::TokenState.into());
        }
        token.amount
    };
    drop(mint_data);
    let spec = transfer_checked(
        request.token_program,
        request.hoard_vault,
        request.mint,
        destination.key.to_bytes(),
        authority.key.to_bytes(),
        request.amount,
        decimals,
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let authority_bump = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        program_id,
    )
    .1;
    let authority_bump_seed = [authority_bump];
    invoke_signed(
        &token_instruction(&spec),
        &[
            vault.clone(),
            mint.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
            &authority_bump_seed,
        ]],
    )
    .map_err(|_| CustodySbfError::TokenCpi)?;
    let after = read_vault_amount(vault, authority, request)?;
    let destination_after = {
        let bytes = destination
            .try_borrow_data()
            .map_err(|_| CustodySbfError::TokenState)?;
        profile
            .check_transfer_account(request.token_program, &bytes)
            .map_err(|_| CustodySbfError::TokenState)?
            .amount
    };
    if destination_before.checked_add(request.amount) != Some(destination_after) {
        return Err(CustodySbfError::Postcondition.into());
    }
    let state = read_state(account(accounts, STATE)?)?;
    let receipt = state
        .refund_and_close(
            request,
            request_digest,
            Clock::get().map_err(|_| CustodySbfError::Replay)?.slot,
            before,
            after,
            account(accounts, RENT_CREDIT)?.key.to_bytes(),
            true,
        )
        .map_err(|_| CustodySbfError::Replay)?;
    close_vault_to_rent_credit(
        vault,
        authority,
        token_program,
        account(accounts, RENT_CREDIT)?,
        request,
        program_id,
    )?;
    close_state_to_rent_credit(
        account(accounts, STATE)?,
        account(accounts, RENT_CREDIT)?,
        program_id,
    )?;
    return_receipt(receipt)
}

#[inline(never)]
fn realize_and_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let vault = account(accounts, REALIZE_VAULT)?;
    let market = account(accounts, REALIZE_MARKET)?;
    let authority = account(accounts, REALIZE_AUTHORITY)?;
    let mint = account(accounts, REALIZE_MINT)?;
    let token_program = account(accounts, REALIZE_TOKEN_PROGRAM)?;
    authenticate_token_frame(
        program_id,
        vault,
        authority,
        mint,
        token_program,
        request,
        false,
    )?;
    if market.key.to_bytes() != request.market
        || market.owner.to_bytes() != request.core_program
        || market.data_len() != STATE_BYTES
        || market.is_writable
        || vault.is_writable
    {
        return Err(CustodySbfError::Release.into());
    }
    let market_data = market
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let market_state = CoreState::decode(&market_data).map_err(|_| CustodySbfError::Release)?;
    let market_digest = hash(&market_data).to_bytes();
    drop(market_data);
    let amount = read_vault_amount(vault, authority, request)?;
    let state = read_state(account(accounts, STATE)?)?;
    let receipt = state
        .realize_and_close(
            request,
            request_digest,
            market_state,
            market_digest,
            amount,
            account(accounts, RENT_CREDIT)?.key.to_bytes(),
        )
        .map_err(|_| CustodySbfError::Replay)?;
    // The canonical Hoard vault and its authority are already the normal
    // Custody PDA tuple. Realization deliberately performs no token CPI and
    // rewrites the projection replay in-place as the normal live replay.
    let receipt_bytes = receipt
        .encode()
        .map_err(|_| CustodySbfError::Postcondition)?;
    let normal = normal_replay_from_realization_v1(state, receipt, hash(&receipt_bytes).to_bytes())
        .map_err(|_| CustodySbfError::Replay)?;
    let replay = account(accounts, STATE)?;
    replay
        .resize(CUSTODY_REPLAY_BYTES_V1)
        .map_err(|_| CustodySbfError::Commit)?;
    let normal_bytes = normal.to_bytes().map_err(|_| CustodySbfError::Commit)?;
    {
        let mut data = replay
            .try_borrow_mut_data()
            .map_err(|_| CustodySbfError::Commit)?;
        if data.len() != normal_bytes.len() {
            return Err(CustodySbfError::Commit.into());
        }
        data.copy_from_slice(&normal_bytes);
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

#[inline(never)]
fn abort_open_and_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let vault = account(accounts, ABORT_VAULT)?;
    let authority = account(accounts, ABORT_AUTHORITY)?;
    let token_program = account(accounts, ABORT_TOKEN_PROGRAM)?;
    let rent_credit = account(accounts, RENT_CREDIT)?;
    require_vacant_market(account(accounts, ABORT_MARKET)?, request)?;
    authenticate_vault_without_mint(program_id, vault, authority, token_program, request)?;
    if !vault.is_writable || vault.lamports() != request.vault_rent_lamports {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let balance = read_vault_amount(vault, authority, request)?;
    let state = read_state(account(accounts, STATE)?)?;
    let receipt = state
        .abort_open_and_close(
            request,
            request_digest,
            Clock::get().map_err(|_| CustodySbfError::Replay)?.slot,
            balance,
            rent_credit.key.to_bytes(),
            true,
        )
        .map_err(|_| CustodySbfError::Replay)?;
    let rent_before = rent_credit.lamports();
    close_vault_to_rent_credit(
        vault,
        authority,
        token_program,
        rent_credit,
        request,
        program_id,
    )?;
    close_state_to_rent_credit(account(accounts, STATE)?, rent_credit, program_id)?;
    let expected = rent_before
        .checked_add(request.vault_rent_lamports)
        .and_then(|value| value.checked_add(request.state_rent_lamports))
        .ok_or(CustodySbfError::Postcondition)?;
    if rent_credit.lamports() != expected {
        return Err(CustodySbfError::Postcondition.into());
    }
    return_receipt(receipt)
}

#[inline(never)]
fn lock_hoard_and_close_source(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let hoard = account(accounts, LOCK_CLOSE_HOARD)?;
    let source = account(accounts, LOCK_CLOSE_SOURCE)?;
    let authority = account(accounts, LOCK_CLOSE_AUTHORITY)?;
    let mint = account(accounts, LOCK_CLOSE_MINT)?;
    let token_program = account(accounts, LOCK_CLOSE_TOKEN_PROGRAM)?;
    let source_replay = account(accounts, LOCK_CLOSE_SOURCE_REPLAY)?;
    let rent_credit = account(accounts, RENT_CREDIT)?;
    require_vacant_market(account(accounts, LOCK_CLOSE_MARKET)?, request)?;
    authenticate_token_frame(
        program_id,
        hoard,
        authority,
        mint,
        token_program,
        request,
        false,
    )?;
    authenticate_source_frame(program_id, source, source_replay, token_program, request)?;
    if !hoard.is_writable
        || source.lamports() != request.funding_source_vault_rent_lamports
        || source_replay.lamports() != request.funding_source_state_rent_lamports
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let (source_after, hoard_after) = transfer_source_into_hoard(
        program_id,
        source,
        hoard,
        authority,
        mint,
        token_program,
        request,
    )?;
    let source_replay_state = read_source_replay(source_replay)?;
    let state = read_state(account(accounts, STATE)?)?;
    let (next, receipt) = state
        .lock_hoard_and_close_source(
            request,
            request_digest,
            source_replay.key.to_bytes(),
            source_replay_state,
            request.amount,
            source_after,
            0,
            hoard_after,
            source.lamports(),
            source_replay.lamports(),
            rent_credit.key.to_bytes(),
            true,
        )
        .map_err(|_| CustodySbfError::Replay)?;
    let rent_before = rent_credit.lamports();
    close_specific_vault_to_rent_credit(
        source,
        authority,
        token_program,
        rent_credit,
        request,
        program_id,
        request.funding_source_vault,
    )?;
    close_state_to_rent_credit(source_replay, rent_credit, program_id)?;
    let expected = rent_before
        .checked_add(request.funding_source_vault_rent_lamports)
        .and_then(|value| value.checked_add(request.funding_source_state_rent_lamports))
        .ok_or(CustodySbfError::Postcondition)?;
    if rent_credit.lamports() != expected {
        return Err(CustodySbfError::Postcondition.into());
    }
    commit_state(account(accounts, STATE)?, next)?;
    return_lock_receipt(receipt)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn transfer_source_into_hoard<'info>(
    program_id: &Pubkey,
    source: &AccountInfo<'info>,
    hoard: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    request: ProjectedCustodyRequestV1,
) -> Result<(u64, u64), ProgramError> {
    let profile = collateral_profile(request)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let hoard_data = hoard
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let mint_facts = profile
        .check_mint(request.token_program, &mint_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    let facts = profile
        .check_transfer(ExactTransferInput {
            program_id: request.token_program,
            mint_address: request.mint,
            mint_data: &mint_data,
            source_data: &source_data,
            destination_data: &hoard_data,
            authority: authority.key.to_bytes(),
            amount: request.amount,
            decimals: mint_facts.decimals,
        })
        .map_err(|_| CustodySbfError::TokenState)?;
    if facts.source().owner != authority.key.to_bytes()
        || facts.destination().owner != authority.key.to_bytes()
        || facts.source().amount != request.amount
        || facts.destination().amount != 0
    {
        return Err(CustodySbfError::TokenState.into());
    }
    drop(hoard_data);
    drop(source_data);
    drop(mint_data);
    let spec = transfer_checked(
        request.token_program,
        request.funding_source_vault,
        request.mint,
        request.hoard_vault,
        authority.key.to_bytes(),
        request.amount,
        mint_facts.decimals,
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let authority_bump = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        program_id,
    )
    .1;
    let authority_bump_seed = [authority_bump];
    invoke_signed(
        &token_instruction(&spec),
        &[
            source.clone(),
            mint.clone(),
            hoard.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
            &authority_bump_seed,
        ]],
    )
    .map_err(|_| CustodySbfError::TokenCpi)?;
    Ok((
        read_vault_amount(source, authority, request)?,
        read_vault_amount(hoard, authority, request)?,
    ))
}

fn authenticate_token_frame(
    program_id: &Pubkey,
    vault: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
    vacant: bool,
) -> Result<(), ProgramError> {
    let vault_seeds = CustodyVaultSeedsV1::new(
        request.market,
        request.release_set,
        request.context_digest,
        CompartmentV1::HoardPrincipal,
    );
    let authority_expected = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        program_id,
    )
    .0;
    if Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0 != *vault.key
        || vault.key.to_bytes() != request.hoard_vault
        || authority.key != &authority_expected
        || authority.is_signer
        || authority.is_writable
        || authority.executable
        || mint.key.to_bytes() != request.mint
        || mint.owner != token_program.key
        || token_program.key.to_bytes() != request.token_program
        || !token_program.executable
        || collateral_profile(request)?.program_id() != request.token_program
    {
        return Err(CustodySbfError::TokenState.into());
    }
    if vacant {
        if vault.owner != &system_program::ID || vault.data_len() != 0 {
            return Err(CustodySbfError::TokenState.into());
        }
    } else if vault.owner != token_program.key || vault.data_len() != ACCOUNT_BYTES {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(())
}

fn authenticate_vault_without_mint(
    program_id: &Pubkey,
    vault: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    let vault_seeds = CustodyVaultSeedsV1::new(
        request.market,
        request.release_set,
        request.context_digest,
        CompartmentV1::HoardPrincipal,
    );
    let authority_expected = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        program_id,
    )
    .0;
    if Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0 != *vault.key
        || vault.key.to_bytes() != request.hoard_vault
        || vault.owner != token_program.key
        || vault.data_len() != ACCOUNT_BYTES
        || authority.key != &authority_expected
        || authority.is_signer
        || authority.is_writable
        || authority.executable
        || token_program.key.to_bytes() != request.token_program
        || !token_program.executable
        || collateral_profile(request)?.program_id() != request.token_program
    {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(())
}

fn authenticate_source_frame(
    program_id: &Pubkey,
    source: &AccountInfo<'_>,
    source_replay: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    let vault_seeds = CustodyVaultSeedsV1::new(
        request.market,
        request.release_set,
        request.funding_source_context,
        request.funding_source_compartment,
    );
    let replay_expected = Pubkey::find_program_address(
        &[
            CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
            &request.funding_source_context,
        ],
        program_id,
    )
    .0;
    if Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0 != *source.key
        || source.key.to_bytes() != request.funding_source_vault
        || !source.is_writable
        || source.owner != token_program.key
        || source.data_len() != ACCOUNT_BYTES
        || source_replay.key != &replay_expected
        || *source_replay.key == account_key_for_projection(program_id, request)
        || !source_replay.is_writable
        || source_replay.owner != program_id
        || source_replay.data_len() != CUSTODY_REPLAY_BYTES_V1
    {
        return Err(CustodySbfError::Replay.into());
    }
    Ok(())
}

fn account_key_for_projection(program_id: &Pubkey, request: ProjectedCustodyRequestV1) -> Pubkey {
    let seeds = ProjectedCustodyStateSeedsV1::from_request(request);
    Pubkey::find_program_address(&seeds.as_slices(), program_id).0
}

fn require_vacant_market(
    market: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    if market.key.to_bytes() != request.market
        || market.owner != &system_program::ID
        || market.data_len() != 0
        || market.is_signer
        || market.is_writable
        || market.executable
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

fn collateral_profile(
    request: ProjectedCustodyRequestV1,
) -> Result<ExactTransferProfileV1, ProgramError> {
    for release in PRODUCTION_ADAPTER_RELEASES {
        if hash(&release.to_bytes()).to_bytes() == request.collateral_release
            && release.profile().program_id() == request.token_program
        {
            return Ok(release.profile());
        }
    }
    Err(CustodySbfError::Realm.into())
}

fn read_vault_amount(
    vault: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
) -> Result<u64, ProgramError> {
    let data = vault
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    collateral_profile(request)?
        .check_custody_account(
            request.token_program,
            &data,
            request.mint,
            authority.key.to_bytes(),
        )
        .map(|account| account.amount)
        .map_err(|_| CustodySbfError::TokenState.into())
}

fn read_state(account: &AccountInfo<'_>) -> Result<ProjectedCustodyStateV1, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    ProjectedCustodyStateV1::decode(&data).map_err(|_| CustodySbfError::Replay.into())
}

fn read_source_replay(account: &AccountInfo<'_>) -> Result<CustodyReplayV1, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    CustodyReplayV1::decode(&data).map_err(|_| CustodySbfError::Replay.into())
}

fn commit_state(
    account: &AccountInfo<'_>,
    state: ProjectedCustodyStateV1,
) -> Result<(), ProgramError> {
    let bytes = state.encode().map_err(|_| CustodySbfError::Commit)?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| CustodySbfError::Commit)?;
    if data.len() != bytes.len() {
        return Err(CustodySbfError::Commit.into());
    }
    data.copy_from_slice(&bytes);
    Ok(())
}

fn top_up_allocate_assign<'info>(
    payer: &AccountInfo<'info>,
    target: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    required_lamports: u64,
    bytes: usize,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    if target.owner != &system_program::ID
        || target.data_len() != 0
        || target.lamports() > required_lamports
    {
        return Err(CustodySbfError::Create.into());
    }
    let top_up = required_lamports
        .checked_sub(target.lamports())
        .ok_or(CustodySbfError::Create)?;
    if top_up > 0 {
        invoke(
            &transfer(payer.key, target.key, top_up),
            &[payer.clone(), target.clone(), system.clone()],
        )
        .map_err(|_| CustodySbfError::Create)?;
    }
    invoke_signed(
        &allocate(
            target.key,
            u64::try_from(bytes).map_err(|_| CustodySbfError::Create)?,
        ),
        &[target.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| CustodySbfError::Create)?;
    invoke_signed(
        &assign(target.key, owner),
        &[target.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| CustodySbfError::Create)?;
    if target.owner != owner || target.data_len() != bytes || target.lamports() != required_lamports
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    Ok(())
}

fn close_vault_to_rent_credit<'info>(
    vault: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    rent_credit: &AccountInfo<'info>,
    request: ProjectedCustodyRequestV1,
    program_id: &Pubkey,
) -> Result<(), ProgramError> {
    close_specific_vault_to_rent_credit(
        vault,
        authority,
        token_program,
        rent_credit,
        request,
        program_id,
        request.hoard_vault,
    )
}

#[allow(clippy::too_many_arguments)]
fn close_specific_vault_to_rent_credit<'info>(
    vault: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    rent_credit: &AccountInfo<'info>,
    request: ProjectedCustodyRequestV1,
    program_id: &Pubkey,
    vault_address: [u8; 32],
) -> Result<(), ProgramError> {
    let spec = close_account(
        request.token_program,
        vault_address,
        request.rent_credit,
        authority.key.to_bytes(),
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let bump = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        program_id,
    )
    .1;
    let bump_seed = [bump];
    invoke_signed(
        &token_instruction(&spec),
        &[
            vault.clone(),
            rent_credit.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
            &bump_seed,
        ]],
    )
    .map_err(|_| CustodySbfError::TokenCpi.into())
}

fn close_state_to_rent_credit(
    state: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    program_id: &Pubkey,
) -> Result<(), ProgramError> {
    if state.owner != program_id || state.key == rent_credit.key {
        return Err(CustodySbfError::Commit.into());
    }
    let amount = state.lamports();
    let destination = rent_credit
        .lamports()
        .checked_add(amount)
        .ok_or(CustodySbfError::Postcondition)?;
    state
        .try_borrow_mut_data()
        .map_err(|_| CustodySbfError::Commit)?
        .fill(0);
    **state
        .try_borrow_mut_lamports()
        .map_err(|_| CustodySbfError::Commit)? = 0;
    **rent_credit
        .try_borrow_mut_lamports()
        .map_err(|_| CustodySbfError::Commit)? = destination;
    state.resize(0).map_err(|_| CustodySbfError::Commit)?;
    state.assign(&system_program::ID);
    Ok(())
}

fn return_receipt(receipt: ProjectedCustodyReceiptV1) -> Result<(), ProgramError> {
    let bytes = receipt
        .encode()
        .map_err(|_| CustodySbfError::Postcondition)?;
    set_return_data(&bytes);
    Ok(())
}

fn return_lock_receipt(receipt: ProjectedCustodyLockReceiptV1) -> Result<(), ProgramError> {
    let bytes = receipt
        .encode()
        .map_err(|_| CustodySbfError::Postcondition)?;
    set_return_data(&bytes);
    Ok(())
}

fn require_count(
    accounts: &[AccountInfo<'_>],
    operation: ProjectedCustodyOperationV1,
) -> Result<(), ProgramError> {
    let expected = match operation {
        ProjectedCustodyOperationV1::Initialize => INITIALIZE_ACCOUNTS,
        ProjectedCustodyOperationV1::OpenHoard => OPEN_ACCOUNTS,
        ProjectedCustodyOperationV1::LockHoard => LOCK_ACCOUNTS,
        ProjectedCustodyOperationV1::RefundAndClose => REFUND_ACCOUNTS,
        ProjectedCustodyOperationV1::RealizeAndClose => REALIZE_ACCOUNTS,
        ProjectedCustodyOperationV1::AbortOpenAndClose => ABORT_ACCOUNTS,
        ProjectedCustodyOperationV1::LockHoardAndCloseSource => LOCK_CLOSE_ACCOUNTS,
    };
    if accounts.len() != expected {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| CustodySbfError::AccountFrame.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_projected_operation_has_one_exact_frame_width() {
        assert_eq!(INITIALIZE_ACCOUNTS, INITIALIZE_FOUND_START + 31);
        assert_eq!(OPEN_ACCOUNTS, 15);
        assert_eq!(LOCK_ACCOUNTS, 14);
        assert_eq!(REFUND_ACCOUNTS, 14);
        assert_eq!(REALIZE_ACCOUNTS, 12);
        assert_eq!(ABORT_ACCOUNTS, 11);
        assert_eq!(LOCK_CLOSE_ACCOUNTS, 14);
    }
}
