#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Stateless SBF accelerator for the Lean-owned Direct V2 descriptor.

// `entrypoint_no_alloc!` expands through a core-compatible `std` facade in the
// pinned Solana toolchain.
extern crate std;

use dclutch_core_contract::ContentId;
use dclutch_direct_aot_contract::{
    DIRECT_PROGRAM_V2_IDENTITIES, DIRECT_PROGRAM_V2_SCALARS, Error as DirectAotError,
    RegisterInput, RegisterOutput, execute_atomic,
};
use dclutch_execution_strategy_contract::{
    ACCELERATOR_ACK_HEADER_BYTES_V1, AcceleratorAckV1, AcceleratorRequestV1,
    decode_register_bank_into, encode_register_bank_into,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

/// Exact Direct scalar-bank width.
pub const DIRECT_AOT_SCALARS_V1: usize = DIRECT_PROGRAM_V2_SCALARS as usize;
/// Exact Direct identity-bank width.
pub const DIRECT_AOT_IDENTITIES_V1: usize = DIRECT_PROGRAM_V2_IDENTITIES as usize;
/// Exact Direct scalar-then-identity bank bytes.
pub const DIRECT_AOT_BANK_BYTES_V1: usize = 456;
/// Exact Direct accelerator request bytes.
pub const DIRECT_AOT_REQUEST_BYTES_V1: usize = 584;
/// Exact accepted Direct accelerator acknowledgement bytes.
pub const DIRECT_AOT_ACCEPTED_ACK_BYTES_V1: usize = 616;
/// Exact refused Direct accelerator acknowledgement bytes.
pub const DIRECT_AOT_REFUSED_ACK_BYTES_V1: usize = ACCELERATOR_ACK_HEADER_BYTES_V1;
/// Measured fully signed direct v0 transaction bytes with one fee payer.
pub const DIRECT_AOT_STANDALONE_V0_WIRE_BYTES_V1: usize = 756;
/// Pinned Solana transaction packet extent.
pub const SOLANA_PACKET_DATA_BYTES_V1: usize = 1_232;

const _: () = assert!(DIRECT_AOT_SCALARS_V1 == 41);
const _: () = assert!(DIRECT_AOT_IDENTITIES_V1 == 4);
const _: () = assert!(DIRECT_AOT_BANK_BYTES_V1 == 41 * 8 + 4 * 32);
const _: () = assert!(DIRECT_AOT_REQUEST_BYTES_V1 == 128 + DIRECT_AOT_BANK_BYTES_V1);
const _: () = assert!(DIRECT_AOT_ACCEPTED_ACK_BYTES_V1 == 160 + DIRECT_AOT_BANK_BYTES_V1);

/// Stable physical refusal from the stateless AOT adapter.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectAotSbfError {
    /// The invocation supplied any account, signer, writable state, or child.
    NonStatelessFrame = 0xA000,
    /// The request wire or its runtime bank counts were not exact Direct V2.
    InvalidRequest = 0xA001,
    /// The scalar-then-identity input bank was malformed.
    InvalidBank = 0xA002,
    /// An accepted output or acknowledgement could not be encoded canonically.
    InvalidAck = 0xA003,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    DirectAotSbfError::NonStatelessFrame as u32
        == dclutch_refusal_registry::DIRECT_AOT_REFUSAL_BASE,
    "DirectAotSbfError must start at its registered refusal band base"
);
const _: () = assert!(
    (DirectAotSbfError::InvalidAck as u32)
        < dclutch_refusal_registry::DIRECT_AOT_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "DirectAotSbfError must not run past its registered refusal band"
);

impl From<DirectAotSbfError> for ProgramError {
    fn from(value: DirectAotSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

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

/// Evaluate one exact stateless request and publish its canonical ack.
///
/// No account is accepted, so there is no signer, writable account, rent,
/// persistent replay state, or CPI authority at this boundary.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if !accounts.is_empty() {
        return Err(DirectAotSbfError::NonStatelessFrame.into());
    }
    let mut output = [0_u8; DIRECT_AOT_ACCEPTED_ACK_BYTES_V1];
    let written = evaluate_into(instruction_data, &mut output)?;
    let ack = output.get(..written).ok_or(DirectAotSbfError::InvalidAck)?;
    set_return_data(ack);
    Ok(())
}

/// Evaluate into caller-owned maximum acknowledgement storage.
///
/// The returned prefix length is 616 on acceptance and 160 on semantic
/// refusal. Physical decode errors return an error and no acknowledgement.
pub fn evaluate_into(
    instruction_data: &[u8],
    output: &mut [u8; DIRECT_AOT_ACCEPTED_ACK_BYTES_V1],
) -> core::result::Result<usize, ProgramError> {
    let request = AcceleratorRequestV1::decode(instruction_data)
        .map_err(|_| DirectAotSbfError::InvalidRequest)?;
    if request.scalar_count() != DIRECT_PROGRAM_V2_SCALARS
        || request.identity_count() != DIRECT_PROGRAM_V2_IDENTITIES
        || request.bank().len() != DIRECT_AOT_BANK_BYTES_V1
        || instruction_data.len() != DIRECT_AOT_REQUEST_BYTES_V1
    {
        return Err(DirectAotSbfError::InvalidRequest.into());
    }

    let mut input_scalars = [0_u64; DIRECT_AOT_SCALARS_V1];
    let mut input_identities = [[0_u8; 32]; DIRECT_AOT_IDENTITIES_V1];
    decode_register_bank_into(request.bank(), &mut input_scalars, &mut input_identities)
        .map_err(|_| DirectAotSbfError::InvalidBank)?;
    let request_digest = content_digest(instruction_data)?;

    let mut scratch_scalars = [0_u64; DIRECT_AOT_SCALARS_V1];
    let mut scratch_identities = [[0_u8; 32]; DIRECT_AOT_IDENTITIES_V1];
    let mut candidate_scalars = [0_u64; DIRECT_AOT_SCALARS_V1];
    let mut candidate_identities = [[0_u8; 32]; DIRECT_AOT_IDENTITIES_V1];
    let result = execute_atomic(
        RegisterInput {
            scalars: &input_scalars,
            identities: &input_identities,
        },
        RegisterOutput {
            scalars: &mut scratch_scalars,
            identities: &mut scratch_identities,
        },
        RegisterOutput {
            scalars: &mut candidate_scalars,
            identities: &mut candidate_identities,
        },
    );

    match result {
        Ok(()) => {
            let mut bank = [0_u8; DIRECT_AOT_BANK_BYTES_V1];
            encode_register_bank_into(&candidate_scalars, &candidate_identities, &mut bank)
                .map_err(|_| DirectAotSbfError::InvalidBank)?;
            let bank_digest = content_digest(&bank)?;
            let ack = AcceleratorAckV1::accepted(request, request_digest, bank_digest, &bank)
                .map_err(|_| DirectAotSbfError::InvalidAck)?;
            ack.encode_into(output)
                .map_err(|_| DirectAotSbfError::InvalidAck)?;
            Ok(DIRECT_AOT_ACCEPTED_ACK_BYTES_V1)
        }
        Err(error) if semantic_refusal(error) => {
            let ack = AcceleratorAckV1::refused(request, request_digest);
            let destination = output
                .get_mut(..DIRECT_AOT_REFUSED_ACK_BYTES_V1)
                .ok_or(DirectAotSbfError::InvalidAck)?;
            ack.encode_into(destination)
                .map_err(|_| DirectAotSbfError::InvalidAck)?;
            Ok(DIRECT_AOT_REFUSED_ACK_BYTES_V1)
        }
        Err(_) => Err(DirectAotSbfError::InvalidBank.into()),
    }
}

fn semantic_refusal(error: DirectAotError) -> bool {
    !matches!(error, DirectAotError::RegisterWidthMismatch)
}

fn content_digest(bytes: &[u8]) -> core::result::Result<ContentId, ProgramError> {
    ContentId::new(hash(bytes).to_bytes()).map_err(|_| DirectAotSbfError::InvalidAck.into())
}

#[cfg(test)]
mod tests;
