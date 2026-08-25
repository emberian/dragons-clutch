#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only SBF caller for end-to-end Custody transaction evidence.
//!
//! The program owns no protocol semantics. It validates and forwards one exact
//! canonical Custody request, signs only the release-set-owned caller-authority
//! PDA, and can deliberately fail after the child returns to prove transaction
//! rollback across the CPI boundary.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CUSTODY_REQUEST_BYTES_V1, CustodyRequestV1};
use dclutch_release_set_contract::CallerAuthoritySeedsV1;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Exact test-wrapper instruction width: one fail-after-CPI byte plus request.
pub const TEST_CALLER_INSTRUCTION_BYTES_V1: usize = CUSTODY_REQUEST_BYTES_V1 + 1;

/// Stable test-caller refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestCallerError {
    /// Wrapper or canonical Custody bytes were malformed.
    Instruction = 0,
    /// Custody program or forwarded caller-authority frame was not exact.
    AccountFrame = 1,
    /// Custody CPI failed or returned no producer-authenticated receipt.
    CustodyCpi = 2,
    /// Deliberate failure after a successful child effect.
    DeliberateLateFailure = 3,
}

impl From<TestCallerError> for ProgramError {
    fn from(value: TestCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one exact Custody request and optionally refuse after successful CPI.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != TEST_CALLER_INSTRUCTION_BYTES_V1 {
        return Err(TestCallerError::Instruction.into());
    }
    let fail_after = *instruction_data
        .first()
        .ok_or(TestCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(TestCallerError::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(1..)
        .ok_or(TestCallerError::Instruction)?;
    let request =
        CustodyRequestV1::decode(request_bytes).map_err(|_| TestCallerError::Instruction)?;
    if request.caller_program != program_id.to_bytes() || accounts.len() < 2 {
        return Err(TestCallerError::AccountFrame.into());
    }
    let custody_program = accounts.first().ok_or(TestCallerError::AccountFrame)?;
    let forwarded = accounts.get(1..).ok_or(TestCallerError::AccountFrame)?;
    if !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
        || forwarded
            .first()
            .ok_or(TestCallerError::AccountFrame)?
            .is_signer
    {
        return Err(TestCallerError::AccountFrame.into());
    }

    let request_digest = hash(request_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| TestCallerError::Instruction)?,
        request.market,
        request.caller_role,
        request.context,
        request_digest,
    )
    .map_err(|_| TestCallerError::Instruction)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&caller_seeds.as_slices(), program_id);
    if forwarded.first().ok_or(TestCallerError::AccountFrame)?.key != &expected_authority {
        return Err(TestCallerError::AccountFrame.into());
    }

    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, index == 0 || account.is_signer)
        } else {
            AccountMeta::new_readonly(*account.key, index == 0 || account.is_signer)
        });
    }
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = caller_seeds.as_slices();
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(custody_program.clone());
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TestCallerError::CustodyCpi)?;
    let (producer, receipt) = get_return_data().ok_or(TestCallerError::CustodyCpi)?;
    if producer != *custody_program.key {
        return Err(TestCallerError::CustodyCpi.into());
    }
    if fail_after == 1 {
        return Err(TestCallerError::DeliberateLateFailure.into());
    }
    set_return_data(&receipt);
    Ok(())
}
