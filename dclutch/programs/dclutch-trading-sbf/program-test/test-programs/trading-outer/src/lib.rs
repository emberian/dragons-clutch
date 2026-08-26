#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-SBF entrypoint exposing only the canonical common Trading outer.

extern crate std;

use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    dclutch_trading_sbf::process_instruction(program_id, accounts, instruction_data)
}
