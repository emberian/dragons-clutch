#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Atomic dClutch categorical-Pyth resolution adapter.
//!
//! The adapter authenticates one immutable provider release, posts and checks
//! a fully verified Pyth update, folds it through the total kernel, persists a
//! terminal Market receipt, reclaims the temporary update, and closes the
//! prepaid resolution Fund in one transaction.  The body-free failure route is
//! permissionless strictly after the immutable price window.

extern crate alloc;

#[cfg(test)]
extern crate std;

use dclutch_pyth_contract::instruction::ResolveCategoricalInstructionV1;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

mod authenticate;
mod close_fund;
mod error;
mod provider;
mod resolution;
#[cfg(feature = "non-production-real-pyth-lab")]
mod synthetic_release;

pub use error::AdapterError;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Decode and execute one atomic categorical Pyth resolution request.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = ResolveCategoricalInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    resolution::dispatch(program_id, accounts, instruction)
}
