#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! dClutch's narrow SBF authentication boundary for categorical Pyth V1.
//!
//! This milestone validates complete account frames and all immutable facts
//! required before resolution. It deliberately performs neither a provider
//! CPI nor any Market/Fund mutation, so an authenticated request ends in the
//! explicit [`AdapterError::MutationNotImplemented`] refusal.

#[cfg(test)]
extern crate std;

use dclutch_pyth_contract::instruction::ResolveCategoricalInstructionV1;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

mod authenticate;
mod error;
#[cfg(feature = "non-production-real-pyth-lab")]
mod synthetic_release;

pub use error::AdapterError;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Decode and authenticate one categorical Pyth resolution request.
///
/// There is intentionally no provider CPI or state mutation in this release.
/// A request which authenticates successfully returns a distinct refusal,
/// preserving atomicity until the state-transition milestone is implemented.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = ResolveCategoricalInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    authenticate::dispatch(program_id, accounts, instruction)
        .and_then(|()| Err(AdapterError::MutationNotImplemented.into()))
}
