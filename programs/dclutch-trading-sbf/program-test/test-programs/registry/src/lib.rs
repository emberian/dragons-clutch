#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-SBF Registry reauthentication producer for the Trading campaign.

extern crate std;

use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Reauthenticate one role from the exact persisted activation cache.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let RegistryInstructionV1::Reauthenticate(role) =
        RegistryInstructionV1::decode(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData)?
    else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let cache = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    let role_program = accounts.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?;
    let data = cache.try_borrow_data()?;
    let view = ActivatedExecutionReleaseSetViewV1::decode(&data)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let set_id = view
        .execution_release_set_id()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let activated = view
        .role(role)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    if activated.release().program().to_bytes() != role_program.key.to_bytes() {
        return Err(ProgramError::IncorrectProgramId);
    }
    let expected =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, set_id.as_bytes()], program_id).0;
    if cache.key != &expected || cache.owner != program_id {
        return Err(ProgramError::InvalidAccountData);
    }
    let receipt = AuthenticatedRoleReceiptV1::new(
        role,
        set_id,
        activated.release().program(),
        activated.artifact_release_id(),
        activated.release().semantic_release_id(),
    );
    set_return_data(&receipt.to_bytes());
    Ok(())
}
