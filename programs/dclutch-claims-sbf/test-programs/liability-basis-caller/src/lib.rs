#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF wrapper for late LiabilityBasisV2 rollback evidence.
//!
//! This program has no protocol authority. It forwards the caller's complete
//! account frame and opaque instruction bytes to Claims, requires Claims to
//! return successfully, and can then deliberately refuse so ProgramTest can
//! prove transaction rollback across Claims, Custody, and token state.

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryInto;

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

const PROTOCOL_POSITION_MAGIC_V2: &[u8] = b"DCLPPR02";
const PROTOCOL_POSITION_BYTES_V2: usize = 320;

/// Stable test-wrapper refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LiabilityBasisTestCallerError {
    /// Wrapper bytes were malformed.
    Instruction = 0,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 1,
    /// Production Claims/Custody composition refused or returned no receipt.
    ClaimsCpi = 2,
    /// Deliberate refusal after the complete production composition returned.
    DeliberateLateFailure = 3,
}

impl From<LiabilityBasisTestCallerError> for ProgramError {
    fn from(value: LiabilityBasisTestCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one opaque Claims request and optionally refuse after its return.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let fail_after = *instruction_data
        .first()
        .ok_or(LiabilityBasisTestCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(LiabilityBasisTestCallerError::Instruction.into());
    }
    let claims_program = accounts
        .first()
        .ok_or(LiabilityBasisTestCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(LiabilityBasisTestCallerError::AccountFrame)?;
    let request = instruction_data
        .get(1..)
        .ok_or(LiabilityBasisTestCallerError::Instruction)?;
    if !claims_program.executable || claims_program.is_signer || claims_program.is_writable {
        return Err(LiabilityBasisTestCallerError::AccountFrame.into());
    }

    let protocol_position = request.len() == PROTOCOL_POSITION_BYTES_V2
        && request.get(..PROTOCOL_POSITION_MAGIC_V2.len()) == Some(PROTOCOL_POSITION_MAGIC_V2);
    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = account.is_signer || protocol_position && index == 0;
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
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    if protocol_position {
        let release_set = array::<32>(request, 16)?;
        let market = array::<32>(request, 48)?;
        let position_owner = array::<32>(request, 80)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(release_set).map_err(|_| LiabilityBasisTestCallerError::Instruction)?,
            market,
            ExecutionRoleV1::Trading,
            position_owner,
            hash(request).to_bytes(),
        )
        .map_err(|_| LiabilityBasisTestCallerError::Instruction)?;
        let bump = [Pubkey::find_program_address(&seeds.as_slices(), program_id).1];
        let [domain, release, market, role, context, digest] = seeds.as_slices();
        invoke_signed(
            &instruction,
            &infos,
            &[&[domain, release, market, role, context, digest, &bump]],
        )
        .map_err(|_| LiabilityBasisTestCallerError::ClaimsCpi)?;
    } else {
        invoke(&instruction, &infos).map_err(|_| LiabilityBasisTestCallerError::ClaimsCpi)?;
    }
    let (producer, receipt) = get_return_data().ok_or(LiabilityBasisTestCallerError::ClaimsCpi)?;
    if producer != *claims_program.key || receipt.is_empty() {
        return Err(LiabilityBasisTestCallerError::ClaimsCpi.into());
    }
    if fail_after == 1 {
        return Err(LiabilityBasisTestCallerError::DeliberateLateFailure.into());
    }
    Ok(())
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], ProgramError> {
    let end = offset
        .checked_add(N)
        .ok_or(LiabilityBasisTestCallerError::Instruction)?;
    input
        .get(offset..end)
        .ok_or(LiabilityBasisTestCallerError::Instruction)?
        .try_into()
        .map_err(|_| LiabilityBasisTestCallerError::Instruction.into())
}
