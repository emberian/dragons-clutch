#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for affine Claims rollback evidence.
//!
//! The caller owns no protocol state. It decodes the canonical affine plan,
//! derives and signs the exact release-scoped caller-authority PDA, forwards
//! the complete account tail to the current Claims program, validates the
//! immediate Claims receipt, and can deliberately refuse afterward.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::{
    CallerRole,
    affine_batch_v2::{AffineBatchPlanV2, AffineBatchReceiptV2},
};
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Stable test-only caller refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AffineBatchTestCallerError {
    /// Wrapper or affine-plan bytes were malformed.
    Instruction = 0,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 1,
    /// Claims refused or did not return its exact plan-bound receipt.
    ClaimsCpi = 2,
    /// Deliberate refusal after the complete Claims composition returned.
    DeliberateLateFailure = 3,
}

impl From<AffineBatchTestCallerError> for ProgramError {
    fn from(value: AffineBatchTestCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one canonical affine request and optionally refuse after receipt
/// validation. Wrapper byte zero succeeds; byte one triggers the late refusal.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let fail_after = *instruction_data
        .first()
        .ok_or(AffineBatchTestCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(AffineBatchTestCallerError::Instruction.into());
    }
    let request = instruction_data
        .get(1..)
        .ok_or(AffineBatchTestCallerError::Instruction)?;
    let plan =
        AffineBatchPlanV2::decode(request).map_err(|_| AffineBatchTestCallerError::Instruction)?;
    let claims_program = accounts
        .first()
        .ok_or(AffineBatchTestCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(AffineBatchTestCallerError::AccountFrame)?;
    if !claims_program.executable || claims_program.is_signer || claims_program.is_writable {
        return Err(AffineBatchTestCallerError::AccountFrame.into());
    }

    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = index == 0;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: request.to_vec(),
    };
    let role = match plan.caller_role() {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Trading => ExecutionRoleV1::Trading,
    };
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(plan.release_set()).map_err(|_| AffineBatchTestCallerError::Instruction)?,
        plan.market(),
        role,
        plan.request_id(),
        hash(request).to_bytes(),
    )
    .map_err(|_| AffineBatchTestCallerError::Instruction)?;
    let bump = [Pubkey::find_program_address(&seeds.as_slices(), program_id).1];
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump]],
    )
    .map_err(|_| AffineBatchTestCallerError::ClaimsCpi)?;

    let (producer, receipt_bytes) =
        get_return_data().ok_or(AffineBatchTestCallerError::ClaimsCpi)?;
    let receipt = AffineBatchReceiptV2::decode(&receipt_bytes)
        .map_err(|_| AffineBatchTestCallerError::ClaimsCpi)?;
    if producer != *claims_program.key
        || receipt.claims_program() != claims_program.key.to_bytes()
        || receipt.packet_digest() != hash(request).to_bytes()
        || receipt.validate_plan(plan).is_err()
    {
        return Err(AffineBatchTestCallerError::ClaimsCpi.into());
    }
    if fail_after == 1 {
        return Err(AffineBatchTestCallerError::DeliberateLateFailure.into());
    }
    Ok(())
}
