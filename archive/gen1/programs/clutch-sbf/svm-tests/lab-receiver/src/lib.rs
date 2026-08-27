//! A narrow laboratory writer for the Pyth pull transaction seam.
//!
//! This program is not a Pyth receiver model: it verifies no VAA, Merkle path,
//! feed, time, or confidence. Its sole job is to copy one already-shaped
//! 134-byte `PriceUpdateV2` body into account position 4. That makes the bank
//! campaign non-vacuous: Dragon consumes bytes written by the immediately
//! preceding instruction, and a later Dragon refusal must roll that write back
//! atomically.
//!
//! The first eight instruction bytes are intentionally treated as opaque. The
//! test program accepts a hostile discriminator while still performing the
//! write, so Dragon's own exact-post authentication is exercised. A deployed
//! Pyth receiver would independently reject an unknown discriminator.
#![no_std]

use solana_account_info::AccountInfo;
use solana_program_entrypoint::{entrypoint, ProgramResult};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;

const POST_ACCOUNT_COUNT: usize = 7;
const POST_DISCRIMINATOR_LEN: usize = 8;
const PRICE_UPDATE_V2_ACCOUNT_LEN: usize = 134;
const POST_DATA_LEN: usize = POST_DISCRIMINATOR_LEN + PRICE_UPDATE_V2_ACCOUNT_LEN;
const UPDATE_POSITION: usize = 4;
const WRITE_AUTHORITY_POSITION: usize = 6;

entrypoint!(process_instruction);

/// Copy the canonical update body carried after the eight-byte opcode into
/// the receiver-owned, writable, signing update account.
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() < POST_ACCOUNT_COUNT {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    if instruction_data.len() != POST_DATA_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let update = &accounts[UPDATE_POSITION];
    if !update.is_signer || !accounts[WRITE_AUTHORITY_POSITION].is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !update.is_writable || update.owner != program_id {
        return Err(ProgramError::InvalidAccountData);
    }
    let mut body = update.try_borrow_mut_data()?;
    if body.len() != PRICE_UPDATE_V2_ACCOUNT_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    body.copy_from_slice(&instruction_data[POST_DISCRIMINATOR_LEN..]);
    Ok(())
}
