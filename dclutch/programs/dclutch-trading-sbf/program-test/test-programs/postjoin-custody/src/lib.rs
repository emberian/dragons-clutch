#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-SBF Custody adversary that commits the genuine delegated transfer,
//! then corrupts one replay lineage field before returning its genuine ACK.

extern crate std;

use dclutch_custody_contract::{CustodyReplayLayoutV1, CustodyReplayV1};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    dclutch_custody_sbf::process_instruction(program_id, accounts, instruction_data)?;
    let replay = accounts.get(8).ok_or(ProgramError::NotEnoughAccountKeys)?;
    let mut data = replay
        .try_borrow_mut_data()
        .map_err(|_| ProgramError::AccountBorrowFailed)?;
    CustodyReplayV1::decode(&data).map_err(|_| ProgramError::InvalidAccountData)?;
    let byte = data
        .get_mut(CustodyReplayLayoutV1::LAST_POSTSTATE_COMMITMENT_OFFSET)
        .ok_or(ProgramError::InvalidAccountData)?;
    *byte ^= 1;
    CustodyReplayV1::decode(&data).map_err(|_| ProgramError::InvalidAccountData)?;
    Ok(())
}
