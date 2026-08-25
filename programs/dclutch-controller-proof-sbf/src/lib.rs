#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Controller-PDA authority membrane for the exact-account Effect experiment.

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
/// Bytes in the exact Effect V1 claim plan forwarded to the child executor.
pub const EFFECT_PLAN_BYTES: usize = 72;
/// Bytes in the canonical controller journal.
pub const JOURNAL_BYTES: usize = 16;
/// Exact-account claim proof-program identity used by this experiment.
pub const EFFECT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);

const JOURNAL_MAGIC: &[u8; 4] = b"DCCJ";

/// Stable controller experiment refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerError {
    /// Account count or order was not canonical.
    AccountFrame = 0,
    /// Account privilege, owner, executable state, or aliasing was invalid.
    AccountAuthority = 1,
    /// The named Effect program was not the pinned child.
    EffectProgram = 2,
    /// The controller PDA did not match the runtime program and supplied bump.
    ControllerPda = 3,
    /// Controller journal bytes were not canonical or could not be borrowed.
    Journal = 4,
    /// Journal counter overflowed.
    JournalOverflow = 5,
    /// Instruction bytes were not exactly bump plus Effect V1 plan.
    Instruction = 6,
}

impl From<ControllerError> for ProgramError {
    fn from(error: ControllerError) -> Self {
        Self::Custom(error as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Authenticate the controller PDA, increment the journal, and invoke Effect.
///
/// This intentionally performs the caller mutation before CPI so the hostile
/// campaign observes whether a late child refusal restores both programs'
/// accounts atomically.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != 4 || instruction_data.len() != EFFECT_PLAN_BYTES + 1 {
        return Err(ControllerError::AccountFrame.into());
    }
    let bump = *instruction_data
        .first()
        .ok_or(ControllerError::Instruction)?;
    let effect_plan = instruction_data
        .get(1..)
        .ok_or(ControllerError::Instruction)?;

    let mut iterator = accounts.iter();
    let controller = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let journal = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let projection = next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;
    let effect_program =
        next_account_info(&mut iterator).map_err(|_| ControllerError::AccountFrame)?;

    if controller.is_signer
        || controller.is_writable
        || controller.executable
        || journal.is_signer
        || !journal.is_writable
        || journal.executable
        || projection.is_signer
        || !projection.is_writable
        || projection.executable
        || effect_program.is_signer
        || effect_program.is_writable
        || !effect_program.executable
        || controller.key == journal.key
        || controller.key == projection.key
        || journal.key == projection.key
    {
        return Err(ControllerError::AccountAuthority.into());
    }
    if journal.owner != program_id {
        return Err(ControllerError::AccountAuthority.into());
    }
    if effect_program.key != &EFFECT_PROGRAM_ID {
        return Err(ControllerError::EffectProgram.into());
    }
    let bump_seed = [bump];
    let controller_seeds: [&[u8]; 2] = [CONTROLLER_SEED, &bump_seed];
    let expected = Pubkey::create_program_address(&controller_seeds, program_id)
        .map_err(|_| ControllerError::ControllerPda)?;
    if controller.key != &expected {
        return Err(ControllerError::ControllerPda.into());
    }

    {
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
    }

    let instruction = Instruction {
        program_id: EFFECT_PROGRAM_ID,
        accounts: std::vec![
            AccountMeta::new_readonly(*controller.key, true),
            AccountMeta::new(*projection.key, false),
        ],
        data: effect_plan.to_vec(),
    };
    invoke_signed(
        &instruction,
        &[
            controller.clone(),
            projection.clone(),
            effect_program.clone(),
        ],
        &[&controller_seeds],
    )
}
