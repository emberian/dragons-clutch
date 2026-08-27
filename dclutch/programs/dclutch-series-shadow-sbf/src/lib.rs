#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Stateless Shadow-AOT evaluation for one exact recurring-Series artifact bundle.
//!
//! This crate exposes the checked comparison core first. A physical SBF entry
//! is enabled only by a later generated-bundle module: there is deliberately no
//! generic instruction that accepts caller-supplied artifact bytes.

extern crate alloc;
// The pinned Solana entrypoint macro expands through its core-compatible
// `std` facade even though protocol evaluation remains `no_std`.
extern crate std;

/// Physical readonly Shadow callback and typed acknowledgement boundary.
pub mod entrypoint;
/// Exact generic-interpreter and Series-semantic comparison boundary.
pub mod evaluator;
/// Compile-time selected, generator-produced release bundle boundary.
pub mod release;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &solana_program::pubkey::Pubkey,
    accounts: &[solana_program::account_info::AccountInfo<'_>],
    instruction_data: &[u8],
) -> solana_program::entrypoint::ProgramResult {
    entrypoint::process_instruction(program_id, accounts, instruction_data)
}
