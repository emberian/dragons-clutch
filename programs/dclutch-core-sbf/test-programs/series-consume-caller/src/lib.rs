#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-SBF test caller for the release-selected Trading-to-Core boundary.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_RECEIPT_BYTES_V5, CLAIMS_FOUNDING_RECEIPT_MAGIC_V5,
};
use dclutch_market_core_codec::{
    SERIES_CORE_REQUEST_BYTES_V1, SeriesCoreActionV1, SeriesCoreRequestV1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_series_v3_kernel::ticket_content_id;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    pubkey::Pubkey,
};

const CALLER_AUTHORITY: usize = 0;
const FOUND_CORE_PROGRAM: usize = 19;
const FOUND_TICKET_RAW: usize = 39;
const OPEN_CORE_PROGRAM: usize = 13;
const OPEN_TICKET_RAW: usize = 21;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Sign the canonical universal Trading caller PDA and forward the exact
/// Series request/proof bytes to the selected Core program.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request_bytes = instruction_data
        .get(..SERIES_CORE_REQUEST_BYTES_V1)
        .ok_or(solana_program::program_error::ProgramError::InvalidInstructionData)?;
    let request = SeriesCoreRequestV1::decode(request_bytes)
        .map_err(|_| solana_program::program_error::ProgramError::InvalidInstructionData)?;
    if request.action() != SeriesCoreActionV1::Consume {
        return Err(solana_program::program_error::ProgramError::InvalidInstructionData);
    }
    let open = instruction_data
        .len()
        .checked_sub(CLAIMS_FOUNDING_RECEIPT_BYTES_V5)
        .and_then(|start| instruction_data.get(start..start + CLAIMS_FOUNDING_RECEIPT_MAGIC_V5.len()))
        == Some(CLAIMS_FOUNDING_RECEIPT_MAGIC_V5.as_slice());
    let (core_index, ticket_index) = if open {
        (OPEN_CORE_PROGRAM, OPEN_TICKET_RAW)
    } else {
        (FOUND_CORE_PROGRAM, FOUND_TICKET_RAW)
    };
    let caller = accounts
        .get(CALLER_AUTHORITY)
        .ok_or(solana_program::program_error::ProgramError::NotEnoughAccountKeys)?;
    let core = accounts
        .get(core_index)
        .ok_or(solana_program::program_error::ProgramError::NotEnoughAccountKeys)?;
    let ticket_raw = accounts
        .get(ticket_index)
        .ok_or(solana_program::program_error::ProgramError::NotEnoughAccountKeys)?;
    let ticket_data = ticket_raw.try_borrow_data()?;
    let ticket = ticket_content_id(&ticket_data)
        .map_err(|_| solana_program::program_error::ProgramError::InvalidAccountData)?;
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set().to_bytes(),
        request
            .market()
            .ok_or(solana_program::program_error::ProgramError::InvalidInstructionData)?
            .to_bytes(),
        ExecutionRoleV1::Trading,
        ticket.to_bytes(),
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| solana_program::program_error::ProgramError::InvalidSeeds)?;
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if caller.key != &expected {
        return Err(solana_program::program_error::ProgramError::InvalidSeeds);
    }
    drop(ticket_data);

    let mut metas = Vec::with_capacity(accounts.len());
    for (index, account) in accounts.iter().enumerate() {
        let signer = index == CALLER_AUTHORITY;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *core.key,
        accounts: metas,
        data: instruction_data.to_vec(),
    };
    let [domain, release_set, market, role, context, request_digest] = seeds.as_slices();
    let bump_seed = [bump];
    let signer_seeds = [
        domain,
        release_set,
        market,
        role,
        context,
        request_digest,
        bump_seed.as_slice(),
    ];
    invoke_signed(&instruction, accounts, &[&signer_seeds])
}
