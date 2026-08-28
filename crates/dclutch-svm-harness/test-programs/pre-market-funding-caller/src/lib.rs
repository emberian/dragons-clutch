#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only Trading caller for the exact pre-Market Resolution initializer.
//!
//! The program owns no protocol state. It derives the release-set-bound
//! Trading caller authority from the production request, invokes Resolution
//! with the exact 44-account frame, and passes through only a well-shaped
//! Resolution return receipt.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1, PRE_MARKET_FUNDING_RECEIPT_BYTES_V2,
    PreMarketFundingAbortReceiptV1, PreMarketFundingAbortRequestV1, PreMarketFundingReceiptV2,
    PreMarketFundingRequestV2,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Exact production initializer frame forwarded by this caller.
pub const TEST_PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1: usize = 44;
/// Exact production expiry-close frame forwarded by this caller.
pub const TEST_PRE_MARKET_FUNDING_ABORT_ACCOUNT_COUNT_V1: usize = 16;

const CALLER_AUTHORITY: usize = 0;
const CALLER_PROGRAM: usize = 1;
const RESOLUTION_PROGRAM: usize = 3;
const FUNDING_SOURCE: usize = 5;
const LEDGER: usize = 6;
const FOUND_START: usize = 7;
const FOUND_RENT_CREDIT: usize = FOUND_START + 2;
const FOUND_ACTIVATION_CACHE: usize = FOUND_START + 24;
const FOUND_REGISTRY_PROGRAM: usize = FOUND_START + 27;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one exact initializer request under its Trading authority PDA.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() == TEST_PRE_MARKET_FUNDING_ABORT_ACCOUNT_COUNT_V1 {
        return process_abort(program_id, accounts, instruction_data);
    }
    if accounts.len() != TEST_PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let request = PreMarketFundingRequestV2::decode(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let caller = account(accounts, CALLER_PROGRAM)?;
    let authority = account(accounts, CALLER_AUTHORITY)?;
    let resolution = account(accounts, RESOLUTION_PROGRAM)?;
    let funding_source = account(accounts, FUNDING_SOURCE)?;
    let ledger = account(accounts, LEDGER)?;
    let cache = account(accounts, FOUND_ACTIVATION_CACHE)?;
    let registry = account(accounts, FOUND_REGISTRY_PROGRAM)?;
    if caller.key != program_id
        || !caller.executable
        || caller.is_signer
        || caller.is_writable
        || !resolution.executable
        || resolution.is_signer
        || resolution.is_writable
        || authority.is_signer
        || authority.is_writable
        || authority.executable
        || !funding_source.is_signer
        || !funding_source.is_writable
        || !ledger.is_writable
        || ledger.is_signer
        || ledger.executable
        || cache.owner != registry.key
    {
        return Err(ProgramError::InvalidAccountData);
    }
    let cache_data = cache.try_borrow_data()?;
    let activation = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let release_set = activation
        .execution_release_set_id()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    if activation
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| ProgramError::InvalidAccountData)?
        .release()
        .program()
        .as_bytes()
        != &program_id.to_bytes()
    {
        return Err(ProgramError::IncorrectProgramId);
    }
    let authority_seeds = CallerAuthoritySeedsV1::new(
        release_set,
        request.project_found.found.market.to_bytes(),
        ExecutionRoleV1::Trading,
        request.manifest,
        hash(instruction_data).to_bytes(),
    )
    .map_err(|_| ProgramError::InvalidSeeds)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if authority.key != &expected_authority {
        return Err(ProgramError::InvalidSeeds);
    }

    let mut metas = Vec::with_capacity(accounts.len());
    for (index, value) in accounts.iter().enumerate() {
        metas.push(if index == CALLER_AUTHORITY {
            AccountMeta::new_readonly(*value.key, true)
        } else if index == FUNDING_SOURCE {
            AccountMeta::new(*value.key, true)
        } else if index == LEDGER || index == FOUND_RENT_CREDIT {
            AccountMeta::new(*value.key, false)
        } else {
            AccountMeta::new_readonly(*value.key, false)
        });
    }
    let instruction = Instruction {
        program_id: *resolution.key,
        accounts: metas,
        data: instruction_data.into(),
    };
    let bump_seed = [bump];
    let [
        domain,
        release_set_seed,
        market,
        role,
        context,
        request_digest,
    ] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        accounts,
        &[&[
            domain,
            release_set_seed,
            market,
            role,
            context,
            request_digest,
            &bump_seed,
        ]],
    )?;
    let (producer, receipt_bytes) = get_return_data().ok_or(ProgramError::InvalidAccountData)?;
    if producer != *resolution.key
        || receipt_bytes.len() != PRE_MARKET_FUNDING_RECEIPT_BYTES_V2
        || PreMarketFundingReceiptV2::decode(&receipt_bytes).is_err()
    {
        return Err(ProgramError::InvalidAccountData);
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

fn process_abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = PreMarketFundingAbortRequestV1::decode(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let caller = account(accounts, 1)?;
    let authority = account(accounts, 0)?;
    let resolution = account(accounts, 3)?;
    if caller.key != program_id || !caller.executable || !resolution.executable {
        return Err(ProgramError::IncorrectProgramId);
    }
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        request.manifest,
        hash(instruction_data).to_bytes(),
    )
    .map_err(|_| ProgramError::InvalidSeeds)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if authority.key != &expected_authority {
        return Err(ProgramError::InvalidSeeds);
    }
    let metas = accounts
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if index == 0 {
                AccountMeta::new_readonly(*value.key, true)
            } else if matches!(index, 6 | 7 | 8) {
                AccountMeta::new(*value.key, false)
            } else {
                AccountMeta::new_readonly(*value.key, false)
            }
        })
        .collect();
    let instruction = Instruction {
        program_id: *resolution.key,
        accounts: metas,
        data: instruction_data.into(),
    };
    let bump_seed = [bump];
    let [domain, release_set, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        accounts,
        &[&[
            domain,
            release_set,
            market,
            role,
            context,
            digest,
            &bump_seed,
        ]],
    )?;
    let (producer, receipt) = get_return_data().ok_or(ProgramError::InvalidAccountData)?;
    if producer != *resolution.key
        || receipt.len() != PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1
        || PreMarketFundingAbortReceiptV1::decode(&receipt).is_err()
    {
        return Err(ProgramError::InvalidAccountData);
    }
    set_return_data(&receipt);
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(ProgramError::NotEnoughAccountKeys)
}
