#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-SBF caller that presents ONE projected Custody leg, on its own.
//!
//! # What this program is for, and what it deliberately is not
//!
//! `FEE_SECOND_TRANSACTION_V1` §1 argues from source that Custody's admission
//! for a Direct fee transfer names no transaction: the caller authority's
//! `context` seed is the buyer's maker replay root, the replay revision and the
//! delegated allowance are the only sequencing, and the instructions sysvar is
//! never read on that path. Executing that argument needs a transaction that
//! carries the fee leg and nothing else -- and the shipped Direct route cannot
//! produce one, because its transition co-enables `SellerIntermediate` and
//! `FeeContinuation` from the same fee register and projects both inside one
//! Hot execution.
//!
//! Custody will accept a delegated transfer only from the program the activated
//! release set binds to the role the request names, so no third program can
//! stand in beside Trading: it has to stand IN Trading's place. This program
//! does exactly that and nothing else. It is deployed as the Trading role of a
//! probe-only release set, it signs the caller authority the projected request
//! derives, and it forwards those exact bytes to Custody.
//!
//! It therefore proves things about CUSTODY, not about Trading. What it can
//! establish is whether Custody admits a projected fee request presented in a
//! later transaction, and what that costs. What it cannot establish is that
//! Trading can build one -- that route does not exist and this program is not
//! a sketch of it.

extern crate alloc;
extern crate std;

use alloc::vec::Vec;
use dclutch_custody::{DELEGATED_CUSTODY_REQUEST_BYTES_V2, DelegatedCustodyRequestV2};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Accounts of the canonical Custody `Transfer` frame, before the callee.
///
/// Restated from `CustodyFrameSpecV1::new(OperationV1::Transfer)`'s own width
/// rather than imported as a name only so this file states the number a reader
/// can check against the frame the caller passes; the CONTENT of every
/// coordinate comes from the caller, and Custody re-checks all fourteen
/// privileges itself.
const TRANSFER_FRAME_ACCOUNTS: usize = 14;

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

/// Sign the projected request's caller authority and forward it to Custody.
///
/// `instruction_data` is the exact `DelegatedCustodyRequestV2` the Direct Effect
/// projects for one route -- no prefix, no suffix, no bump relay -- so the
/// digest that seeds the caller authority is the digest of the wire, exactly as
/// `custody_composition_v3::prepare` computes it. The accounts are the fourteen
/// Custody `Transfer` coordinates in order, then the Custody program itself,
/// which the frame never names.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != DELEGATED_CUSTODY_REQUEST_BYTES_V2
        || accounts.len() != TRANSFER_FRAME_ACCOUNTS + 1
    {
        return Err(ProgramError::InvalidInstructionData);
    }
    let request = DelegatedCustodyRequestV2::decode(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.custody.release_set,
        request.custody.market,
        ExecutionRoleV1::Trading,
        request.custody.context,
        hash(instruction_data).to_bytes(),
    )
    .map_err(|_| ProgramError::InvalidSeeds)?;
    let base = seeds.as_slices();
    let (expected, bump) = Pubkey::find_program_address(&base, program_id);
    let authority = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    if authority.key != &expected {
        return Err(ProgramError::InvalidSeeds);
    }
    let callee = accounts
        .get(TRANSFER_FRAME_ACCOUNTS)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let frame = accounts
        .get(..TRANSFER_FRAME_ACCOUNTS)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    // The frame's privileges come from the transaction; only coordinate zero's
    // signer bit is this program's to add, and it is the one Custody's frame
    // spec requires and no keypair can supply.
    let metas = frame
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let signer = index == 0;
            if account.is_writable {
                AccountMeta::new(*account.key, signer)
            } else {
                AccountMeta::new_readonly(*account.key, signer)
            }
        })
        .collect::<Vec<_>>();
    let instruction = Instruction {
        program_id: *callee.key,
        accounts: metas,
        data: instruction_data.to_vec(),
    };
    let bump_seed = [bump];
    invoke_signed(
        &instruction,
        accounts,
        &[&[
            base[0], base[1], base[2], base[3], base[4], base[5], &bump_seed,
        ]],
    )
}
