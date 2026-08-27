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

use dclutch_capability_program_contract::hot_v3::{HOT_ROOT_ACCOUNT_V3, HotExecutionEnvelopeV3};
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Stable refusal from the test-only caller.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAcceleratorTestCallerErrorV1 {
    /// The request, accelerator, or forwarded frame was malformed.
    Frame = 0x10_8000,
    /// The canonical caller-authority PDA or privilege set differed.
    Authority = 0x10_8001,
    /// The accelerator returned no typed bytes or another producer returned.
    ReturnData = 0x10_8002,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    DealerAcceleratorTestCallerErrorV1::Frame as u32
        == dclutch_refusal_registry::TEST_DEALER_ACCELERATOR_CALLER_BASE,
    "DealerAcceleratorTestCallerErrorV1 must start at its registered refusal band base"
);
const _: () = assert!(
    (DealerAcceleratorTestCallerErrorV1::ReturnData as u32)
        < dclutch_refusal_registry::TEST_DEALER_ACCELERATOR_CALLER_BASE
            + dclutch_refusal_registry::BAND_SPAN,
    "DealerAcceleratorTestCallerErrorV1 must not run past its registered refusal band"
);

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
    instruction_data: &[u8],
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
    let root = frame
        .get(
            1_usize
                .checked_add(HOT_ROOT_ACCOUNT_V3)
                .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?,
        )
        .ok_or(DealerAcceleratorTestCallerErrorV1::Frame)?;
    let request = request_account
        .try_borrow_data()
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let (expected_authority, seeds, bump) = dealer_accelerator_test_caller_authority_v1(
        program_id,
        instruction_data,
        root.key,
        &request,
    )?;
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
    let bump = [bump];
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump]],
    )?;
    let (producer, bytes) =
        get_return_data().ok_or(DealerAcceleratorTestCallerErrorV1::ReturnData)?;
    if producer != *accelerator.key || bytes.is_empty() {
        return Err(DealerAcceleratorTestCallerErrorV1::ReturnData.into());
    }
    set_return_data(&bytes);
    Ok(())
}

/// Derive the canonical Trading caller-authority PDA for one test invocation.
pub fn dealer_accelerator_test_caller_authority_v1(
    program_id: &Pubkey,
    hot_instruction: &[u8],
    root: &Pubkey,
    request: &[u8],
) -> Result<(Pubkey, CallerAuthoritySeedsV1, u8), ProgramError> {
    let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(hot_instruction)
        .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(envelope.release_set())
            .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?,
        envelope.market(),
        ExecutionRoleV1::Trading,
        root.to_bytes(),
        hash(request).to_bytes(),
    )
    .map_err(|_| DealerAcceleratorTestCallerErrorV1::Frame)?;
    let (authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    Ok((authority, seeds, bump))
}
