//! Recurring-Series transient founding entrypoint.
//!
//! The wire has its own magic and is never accepted by ordinary FoundingV5.
//! Its implementation delegates to the canonical founding adapter so the
//! reconstructed request executes the unchanged permit, custody, product,
//! rent, allocation, commit, and receipt checks.

use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

/// Execute one exact recurring-Series transient founding request.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    crate::founding_v5::process_series_transport(program_id, accounts, instruction_data)
}
