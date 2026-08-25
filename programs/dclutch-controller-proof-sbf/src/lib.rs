#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Controller-PDA authority membrane for claim and real custody children.

extern crate std;

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// PDA seed defining the controller authority namespace.
pub const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
/// Existing Direct V2 maker replay-root domain retained by the successor.
pub const REPLAY_SEED: &[u8] = b"dclutch/direct-replay/v2";
/// Bytes in the exact Effect V1 claim plan.
pub const CLAIM_PLAN_BYTES: usize = 72;
/// Bytes in the exact physical custody plan.
pub const CUSTODY_PLAN_BYTES: usize = 40;
/// Bytes in two bumps, exact replay coordinates, and both child plans.
pub const CONTROLLER_INSTRUCTION_BYTES: usize = 186;
/// Exact-account claim proof-program identity used by this experiment.
pub const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
/// Real custody proof-program identity used by this experiment.
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);
/// Bytes in the canonical controller journal.
pub const JOURNAL_BYTES: usize = 16;

const JOURNAL_MAGIC: &[u8; 4] = b"DCCJ";

/// Stable controller experiment refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerError {
    /// Account count or order was not canonical.
    AccountFrame = 0,
    /// Account privilege, owner, executable state, or aliasing was invalid.
    AccountAuthority = 1,
    /// The named claim or custody program was not the pinned child.
    ChildProgram = 2,
    /// The controller PDA did not match the runtime program and supplied bump.
    ControllerPda = 3,
    /// The replay PDA did not match exact Market/generation/maker coordinates.
    ReplayPda = 4,
    /// Controller journal bytes were not canonical or could not be borrowed.
    Journal = 5,
    /// Journal counter overflowed.
    JournalOverflow = 6,
    /// Instruction bytes were not the exact coordinate and plan envelope.
    Instruction = 7,
}

impl From<ControllerError> for ProgramError {
    fn from(error: ControllerError) -> Self {
        Self::Custom(error as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Authenticate both controller PDAs, then invoke claims followed by custody.
///
/// The journal mutation precedes both CPIs. Any child refusal must therefore
/// roll back caller state, claim state, and any already completed token CPI.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != 11 || instruction_data.len() != CONTROLLER_INSTRUCTION_BYTES {
        return Err(ControllerError::AccountFrame.into());
    }
    let controller_bump = read_byte(instruction_data, 0)?;
    let replay_bump = read_byte(instruction_data, 1)?;
    let market = read_slice(instruction_data, 2, 32)?;
    let generation = read_slice(instruction_data, 34, 8)?;
    let maker = read_slice(instruction_data, 42, 32)?;
    let claim_plan = read_slice(instruction_data, 74, CLAIM_PLAN_BYTES)?;
    let custody_plan = read_slice(instruction_data, 146, CUSTODY_PLAN_BYTES)?;

    let mut iterator = accounts.iter();
    let controller = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let replay = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let journal = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let projection = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let claim_program =
        next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let custody_program =
        next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let mint = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let source = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let seller = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let venue = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let token_program =
        next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;

    if controller.is_signer
        || controller.is_writable
        || controller.executable
        || replay.is_signer
        || replay.is_writable
        || replay.executable
        || journal.is_signer
        || !journal.is_writable
        || journal.executable
        || projection.is_signer
        || !projection.is_writable
        || projection.executable
        || !readonly_executable(claim_program)
        || !readonly_executable(custody_program)
        || mint.is_signer
        || mint.is_writable
        || mint.executable
        || source.is_signer
        || !source.is_writable
        || source.executable
        || seller.is_signer
        || !seller.is_writable
        || seller.executable
        || venue.is_signer
        || !venue.is_writable
        || venue.executable
        || !readonly_executable(token_program)
        || controller.key == replay.key
        || controller.key == journal.key
        || controller.key == projection.key
        || journal.key == projection.key
    {
        return Err(ControllerError::AccountAuthority.into());
    }
    if journal.owner != program_id {
        return Err(ControllerError::AccountAuthority.into());
    }
    if claim_program.key != &CLAIM_PROGRAM_ID || custody_program.key != &CUSTODY_PROGRAM_ID {
        return Err(ControllerError::ChildProgram.into());
    }

    let controller_bump_seed = [controller_bump];
    let controller_seeds: [&[u8]; 2] = [CONTROLLER_SEED, &controller_bump_seed];
    let expected_controller = Pubkey::create_program_address(&controller_seeds, program_id)
        .map_err(|_| ControllerError::ControllerPda)?;
    if controller.key != &expected_controller {
        return Err(ControllerError::ControllerPda.into());
    }
    let replay_bump_seed = [replay_bump];
    let replay_seeds: [&[u8]; 5] = [REPLAY_SEED, market, generation, maker, &replay_bump_seed];
    let expected_replay = Pubkey::create_program_address(&replay_seeds, program_id)
        .map_err(|_| ControllerError::ReplayPda)?;
    if replay.key != &expected_replay {
        return Err(ControllerError::ReplayPda.into());
    }

    increment_journal(journal)?;

    let claim_instruction = Instruction {
        program_id: CLAIM_PROGRAM_ID,
        accounts: std::vec![
            AccountMeta::new_readonly(*controller.key, true),
            AccountMeta::new(*projection.key, false),
        ],
        data: claim_plan.to_vec(),
    };
    invoke_signed(
        &claim_instruction,
        &[
            controller.clone(),
            projection.clone(),
            claim_program.clone(),
        ],
        &[&controller_seeds],
    )?;

    let custody_instruction = Instruction {
        program_id: CUSTODY_PROGRAM_ID,
        accounts: std::vec![
            AccountMeta::new_readonly(*controller.key, true),
            AccountMeta::new_readonly(*replay.key, true),
            AccountMeta::new_readonly(*mint.key, false),
            AccountMeta::new(*source.key, false),
            AccountMeta::new(*seller.key, false),
            AccountMeta::new(*venue.key, false),
            AccountMeta::new_readonly(*token_program.key, false),
        ],
        data: custody_plan.to_vec(),
    };
    invoke_signed(
        &custody_instruction,
        &[
            controller.clone(),
            replay.clone(),
            mint.clone(),
            source.clone(),
            seller.clone(),
            venue.clone(),
            token_program.clone(),
            custody_program.clone(),
        ],
        &[&controller_seeds, &replay_seeds],
    )
}

fn readonly_executable(account: &AccountInfo<'_>) -> bool {
    !account.is_signer && !account.is_writable && account.executable
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8, ControllerError> {
    input
        .get(offset)
        .copied()
        .ok_or(ControllerError::Instruction)
}

fn read_slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], ControllerError> {
    let end = offset
        .checked_add(width)
        .ok_or(ControllerError::Instruction)?;
    input.get(offset..end).ok_or(ControllerError::Instruction)
}

fn increment_journal(journal: &AccountInfo<'_>) -> ProgramResult {
    let mut data = journal
        .try_borrow_mut_data()
        .map_err(|_| ControllerError::Journal)?;
    if data.len() != JOURNAL_BYTES
        || data.get(..4) != Some(JOURNAL_MAGIC.as_slice())
        || data.get(4..8) != Some([0_u8; 4].as_slice())
    {
        return Err(ControllerError::Journal.into());
    }
    let counter_bytes: [u8; 8] = data
        .get(8..16)
        .ok_or(ControllerError::Journal)?
        .try_into()
        .map_err(|_| ControllerError::Journal)?;
    let next = u64::from_le_bytes(counter_bytes)
        .checked_add(1)
        .ok_or(ControllerError::JournalOverflow)?;
    data.get_mut(8..16)
        .ok_or(ControllerError::Journal)?
        .copy_from_slice(&next.to_le_bytes());
    Ok(())
}
