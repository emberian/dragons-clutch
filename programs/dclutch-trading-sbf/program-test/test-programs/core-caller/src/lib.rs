#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-SBF Core caller used to exercise Trading's caller-authority boundary.

extern crate alloc;
extern crate std;

use alloc::vec::Vec;
use dclutch_market_core_codec::{CORE_EFFECT_ENVELOPE_BYTES_V1, CoreEffectEnvelopeV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
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
    process_instruction(program_id, accounts, instruction_data)
}

/// Forward one exact Core effect after signing the canonical caller PDA.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let envelope = CoreEffectEnvelopeV1::decode(
        instruction_data
            .get(..CORE_EFFECT_ENVELOPE_BYTES_V1)
            .ok_or(ProgramError::InvalidInstructionData)?,
    )
    .map_err(|_| ProgramError::InvalidInstructionData)?;
    let trading = accounts
        .iter()
        .find(|account| account.executable && account.key != program_id)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let metas = accounts
        .iter()
        .enumerate()
        .map(|(index, account)| match (index == 0, account.is_writable) {
            (true, false) => AccountMeta::new_readonly(*account.key, true),
            (true, true) => AccountMeta::new(*account.key, true),
            (false, true) => AccountMeta::new(*account.key, false),
            (false, false) => AccountMeta::new_readonly(*account.key, false),
        })
        .collect::<Vec<_>>();
    let instruction = Instruction {
        program_id: *trading.key,
        accounts: metas,
        data: instruction_data.to_vec(),
    };
    let seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| ProgramError::InvalidSeeds)?;
    let base = seeds.as_slices();
    let (expected, bump) = Pubkey::find_program_address(&base, program_id);
    if accounts.first().map(|account| *account.key) != Some(expected) {
        return Err(ProgramError::InvalidSeeds);
    }
    let bump_seed = [bump];
    let signer = [
        base[0], base[1], base[2], base[3], base[4], base[5], &bump_seed,
    ];
    let mut infos = accounts.to_vec();
    infos.push(trading.clone());
    invoke_signed(&instruction, &infos, &[&signer])?;
    if let Some((producer, bytes)) = get_return_data()
        && producer == *trading.key
    {
        set_return_data(&bytes);
    }
    Ok(())
}
