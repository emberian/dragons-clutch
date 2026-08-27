#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Readonly ProgramTest caller for the General admitted accelerator.
//!
//! The caller exists only to provide the real instructions-sysvar relationship
//! of a top-level Trading-shaped request invoking the accelerator by CPI. It
//! reads an exact accelerator request from account zero, forwards the remaining
//! frame read-only, signs only the canonical caller-authority PDA, and relays
//! the accelerator's typed return data. It owns no protocol semantics or state.

extern crate alloc;

use alloc::vec::Vec;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// PDA seed used only by the readonly real-SBF test caller.
pub const GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1: &[u8] =
    b"general-accelerator-test-caller";

/// Stable refusal from the test-only caller.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAcceleratorTestCallerErrorV1 {
    /// The request/program/frame accounts were missing or malformed.
    Frame = 0x10_9000,
    /// The canonical caller-authority PDA differed.
    Authority = 0x10_9001,
    /// The accelerator returned no typed bytes or another producer returned.
    ReturnData = 0x10_9002,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    GeneralAcceleratorTestCallerErrorV1::Frame as u32
        == dclutch_refusal_registry::TEST_GENERAL_ACCELERATOR_CALLER_BASE,
    "GeneralAcceleratorTestCallerErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (GeneralAcceleratorTestCallerErrorV1::ReturnData as u32)
        < dclutch_refusal_registry::TEST_GENERAL_ACCELERATOR_CALLER_BASE
            + dclutch_refusal_registry::BAND_SPAN,
    "GeneralAcceleratorTestCallerErrorV1 must not run past its registered refusal band"
);

impl From<GeneralAcceleratorTestCallerErrorV1> for ProgramError {
    fn from(value: GeneralAcceleratorTestCallerErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Invoke one admitted accelerator request without granting state or CPI authority.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    _instruction_data: &[u8],
) -> ProgramResult {
    let request_account = accounts
        .first()
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let accelerator = accounts
        .get(1)
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let frame = accounts
        .get(2..)
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let authority = frame
        .first()
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let (expected_authority, bump) = Pubkey::find_program_address(
        &[GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1],
        program_id,
    );
    if authority.key != &expected_authority
        || authority.is_signer
        || authority.is_writable
        || authority.executable
        || !accelerator.executable
        || request_account.is_signer
        || request_account.is_writable
        || request_account.executable
    {
        return Err(GeneralAcceleratorTestCallerErrorV1::Authority.into());
    }
    let request = request_account
        .try_borrow_data()
        .map_err(|_| GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let metas = frame
        .iter()
        .enumerate()
        .map(|(index, account)| AccountMeta {
            pubkey: *account.key,
            is_signer: index == 0,
            is_writable: false,
        })
        .collect::<Vec<_>>();
    let instruction = Instruction {
        program_id: *accelerator.key,
        accounts: metas,
        data: request.to_vec(),
    };
    let mut infos = Vec::with_capacity(
        frame
            .len()
            .checked_add(1)
            .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?,
    );
    infos.extend(frame.iter().cloned());
    infos.push(accelerator.clone());
    invoke_signed(
        &instruction,
        &infos,
        &[&[GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1, &[bump]]],
    )?;
    let (producer, bytes) =
        get_return_data().ok_or(GeneralAcceleratorTestCallerErrorV1::ReturnData)?;
    if producer != *accelerator.key || bytes.is_empty() {
        return Err(GeneralAcceleratorTestCallerErrorV1::ReturnData.into());
    }
    set_return_data(&bytes);
    Ok(())
}
