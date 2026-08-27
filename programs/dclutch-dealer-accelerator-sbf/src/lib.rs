#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Readonly stateless admitted-AOT accelerator for Dealer scenario execution.
//!
//! Common Trading authenticates the release, action descriptor, Product,
//! execution artifacts, exact request, Profile13 account expansion, and input
//! register bank. This program independently rejoins every Dealer semantic
//! account through that public view, evaluates the sole scenario-solvency
//! transition, and returns one canonical candidate-bank chunk. It never writes
//! an account, invokes a child, or owns protocol state.

extern crate alloc;
extern crate std;

use alloc::vec;

use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_HEADER_BYTES_V2, ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2, AcceleratorAckV2,
    AcceleratorRequestV2,
};
use dclutch_trading_sbf::{
    dealer::v3_accelerator_accounts::evaluate_authenticated_dealer_scenario_v4,
    hot_v3::authenticate_accelerator_invocation_v4,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

/// Stable physical refusal from the Dealer accelerator boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerAcceleratorSbfErrorV4 {
    /// AcceleratorRequestV2 transport or candidate-bank width differed.
    InvalidRequest = 0xD000,
    /// Common Trading could not authenticate the release/artifact/runtime view.
    InvalidInvocation = 0xD001,
    /// A canonical acknowledgement could not be constructed.
    InvalidAcknowledgement = 0xD002,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    DealerAcceleratorSbfErrorV4::InvalidRequest as u32
        == dclutch_refusal_registry::DEALER_ACCELERATOR_REFUSAL_BASE,
    "DealerAcceleratorSbfErrorV4 must start at its registered refusal band base"
);
const _: () = assert!(
    (DealerAcceleratorSbfErrorV4::InvalidAcknowledgement as u32)
        < dclutch_refusal_registry::DEALER_ACCELERATOR_REFUSAL_BASE
            + dclutch_refusal_registry::BAND_SPAN,
    "DealerAcceleratorSbfErrorV4 must not run past its registered refusal band"
);

impl From<DealerAcceleratorSbfErrorV4> for ProgramError {
    fn from(value: DealerAcceleratorSbfErrorV4) -> Self {
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

/// Evaluate one authenticated Dealer candidate chunk.
///
/// Physical authentication failures return a program error with no return
/// data. A fully authenticated invocation whose Dealer semantics refuse emits
/// the canonical refused acknowledgement; common Trading therefore retains
/// sole authority over effects and write-last commitment.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = AcceleratorRequestV2::decode(instruction_data)
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidRequest)?;
    let bank_bytes = usize::try_from(request.total_bank_bytes())
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidRequest)?;
    let invocation = authenticate_accelerator_invocation_v4(program_id, accounts, instruction_data)
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidInvocation)?;
    let mut candidate = vec![0_u8; bank_bytes];
    let evaluation = evaluate_authenticated_dealer_scenario_v4(&invocation, &mut candidate);
    let request_digest = content(instruction_data)?;
    let acknowledgement = match evaluation {
        Ok(_) => {
            let bank_digest = content(&candidate)?;
            let start = usize::try_from(request.chunk_offset())
                .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
            let remaining = candidate
                .len()
                .checked_sub(start)
                .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
            let payload_bytes = remaining.min(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2);
            let end = start
                .checked_add(payload_bytes)
                .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
            let payload = candidate
                .get(start..end)
                .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
            AcceleratorAckV2::accepted(request, request_digest, bank_digest, payload)
                .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?
        }
        Err(_) => AcceleratorAckV2::refused(request, request_digest),
    };
    let output_bytes = ACCELERATOR_ACK_HEADER_BYTES_V2
        .checked_add(acknowledgement.payload().len())
        .ok_or(DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
    let mut output = vec![0_u8; output_bytes];
    acknowledgement
        .encode_into(&mut output)
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)?;
    set_return_data(&output);
    Ok(())
}

fn content(bytes: &[u8]) -> Result<ContentId, DealerAcceleratorSbfErrorV4> {
    ContentId::new(hash(bytes).to_bytes())
        .map_err(|_| DealerAcceleratorSbfErrorV4::InvalidAcknowledgement)
}
