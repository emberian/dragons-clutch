#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Read-only ProgramTest caller for the Dealer admitted accelerator.
//!
//! The caller supplies the real top-level Hot instruction required by the
//! Instructions sysvar, reads one exact accelerator request from account zero,
//! signs only its canonical caller-authority PDA, and relays typed return data.
//! It owns no protocol semantics, account mutation, or child authority.

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

/// PDA seed used only by the read-only real-SBF test caller.
pub const DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1: &[u8] =
    b"dealer-accelerator-test-caller";

/// Stable refusal from the test-only caller.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAcceleratorTestCallerErrorV1 {
    /// The request, accelerator, or forwarded frame was malformed.
    Frame = 0,
    /// The canonical caller-authority PDA or privilege set differed.
    Authority = 1,
    /// The accelerator returned no typed bytes or another producer returned.
    ReturnData = 2,
}

impl From<DealerAcceleratorTestCallerErrorV1> for ProgramError {
    fn from(value: DealerAcceleratorTestCallerErrorV1) -> Self {
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

/// Invoke one admitted accelerator request without granting mutation authority.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    _instruction_data: &[u8],
) -> ProgramResult {
    let request_account = accounts
        .first()
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let accelerator = accounts
        .get(1)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let frame = accounts
        .get(2..)
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let authority = frame
        .first()
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let (expected_authority, bump) = Pubkey::find_program_address(
        &[DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1],
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
        return Err(DealerAcceleratorTestCallerErrorV1::Authority.into());
    }
    let request = request_account
        .try_borrow_data()
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
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
            .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
    );
    infos.extend(frame.iter().cloned());
    infos.push(accelerator.clone());
    invoke_signed(
        &instruction,
        &infos,
        &[&[DEALER_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1, &[bump]]],
    )?;
    let (producer, bytes) =
        get_return_data().ok_or(DealerAcceleratorTestCallerErrorV1::ReturnData)?;
    if producer != *accelerator.key || bytes.is_empty() {
        return Err(DealerAcceleratorTestCallerErrorV1::ReturnData.into());
    }
    set_return_data(&bytes);
    Ok(())
}
