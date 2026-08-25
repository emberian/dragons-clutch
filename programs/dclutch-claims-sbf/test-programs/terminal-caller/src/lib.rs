#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF wrapper for late Claims composition rollback evidence.
//!
//! The program owns no protocol state or semantic authority. It forwards one
//! exact terminal representation request, then can deliberately refuse only
//! after the production Claims program (and its Custody child) returned.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_representation_codec::{
    ACTION_WIRE_BYTES_V1, ActionV1, ClaimsReleaseAdmission, DescriptorV1, EconomicPhase,
    Error as RepresentationError, StateV1, prepare,
};
use dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Exact wrapper wire: fail-after-return flag plus production terminal wire.
pub const TEST_TERMINAL_WIRE_BYTES_V1: usize = 1 + ACTION_WIRE_BYTES_V1 + CUSTODY_REQUEST_BYTES_V1;

/// Stable test-wrapper refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TestTerminalCallerError {
    /// Wrapper bytes were malformed.
    Instruction = 0,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 1,
    /// Production Claims/Custody composition refused.
    ClaimsCpi = 2,
    /// Deliberate refusal after the complete production composition returned.
    DeliberateLateFailure = 3,
    /// The exact pure representation transition diverged under SBF execution.
    RepresentationPreflight = 4,
}

impl From<TestTerminalCallerError> for ProgramError {
    fn from(value: TestTerminalCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

fn preflight_error(error: RepresentationError) -> ProgramError {
    ProgramError::Custom(match error {
        RepresentationError::InvalidLength => 18,
        RepresentationError::InvalidMagic => 19,
        RepresentationError::UnsupportedVersion => 20,
        RepresentationError::NonCanonicalReserved => 21,
        RepresentationError::ZeroIdentity => 22,
        RepresentationError::InvalidClaimVector => 23,
        RepresentationError::ZeroReceiptUnits => 24,
        RepresentationError::UnknownAction => 25,
        RepresentationError::ReleaseMismatch => 26,
        RepresentationError::UnauthenticatedRelease => 27,
        RepresentationError::IdentityMismatch => 28,
        RepresentationError::ReplayMismatch => 29,
        RepresentationError::AlreadyRetired => 30,
        RepresentationError::InvalidPhase => 31,
        RepresentationError::InvalidLots => 32,
        RepresentationError::InsufficientLots => 33,
        RepresentationError::NonceOverflow => 34,
        RepresentationError::ArithmeticOverflow => 35,
    })
}

fn check_generated_rule_matrix(
    descriptor: DescriptorV1<'_>,
    state: StateV1,
    action: ActionV1,
    admission: ClaimsReleaseAdmission,
) -> ProgramResult {
    for (tag, expected) in [
        (1_u8, [true, false, false, false]),
        (2_u8, [true, true, true, false]),
        (3_u8, [false, true, true, false]),
        (4_u8, [false, true, true, true]),
    ] {
        let issued_lots = if tag == 4 { 0 } else { state.issued_lots };
        let lots = if tag == 4 { 0 } else { action.lots };
        let case_state = StateV1 {
            issued_lots,
            ..state
        };
        let case_action = ActionV1 {
            tag,
            expected_issued_lots: issued_lots,
            lots,
            ..action
        };
        for (phase_index, phase) in [
            (0_u32, EconomicPhase::Open),
            (1_u32, EconomicPhase::Terminal),
            (2_u32, EconomicPhase::Retiring),
            (3_u32, EconomicPhase::Retired),
        ] {
            let cell = u32::from(tag) * 4 + phase_index;
            let accepted = expected
                .get(usize::try_from(phase_index).map_err(|_| ProgramError::Custom(99))?)
                .copied()
                .ok_or(ProgramError::Custom(99))?;
            match prepare(descriptor, case_state, case_action, phase, admission) {
                Ok(_) if accepted => {}
                Ok(_) => return Err(ProgramError::Custom(100 + cell)),
                Err(RepresentationError::InvalidPhase) if !accepted => {}
                Err(_) if accepted => return Err(ProgramError::Custom(120 + cell)),
                Err(_) => return Err(ProgramError::Custom(160 + cell)),
            }
        }
    }
    for (case, tag) in [(0_u32, 0_u8), (1_u32, 5_u8), (2_u32, u8::MAX)] {
        let hostile = ActionV1 { tag, ..action };
        match prepare(
            descriptor,
            state,
            hostile,
            EconomicPhase::Terminal,
            admission,
        ) {
            Err(RepresentationError::UnknownAction) => {}
            _ => return Err(ProgramError::Custom(200 + case)),
        }
    }
    Ok(())
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one terminal Claims request and optionally refuse after return.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != TEST_TERMINAL_WIRE_BYTES_V1 {
        return Err(TestTerminalCallerError::Instruction.into());
    }
    let fail_after = *instruction_data
        .first()
        .ok_or(TestTerminalCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(TestTerminalCallerError::Instruction.into());
    }
    let claims_program = accounts
        .first()
        .ok_or(TestTerminalCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(TestTerminalCallerError::AccountFrame)?;
    if !claims_program.executable || claims_program.is_signer || claims_program.is_writable {
        return Err(TestTerminalCallerError::AccountFrame.into());
    }

    let mut metas = Vec::with_capacity(forwarded.len());
    for account in forwarded {
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, account.is_signer)
        } else {
            AccountMeta::new_readonly(*account.key, account.is_signer)
        });
    }
    let request = instruction_data
        .get(1..)
        .ok_or(TestTerminalCallerError::Instruction)?;
    {
        let action = ActionV1::decode(
            request
                .get(..ACTION_WIRE_BYTES_V1)
                .ok_or(TestTerminalCallerError::Instruction)?,
        )
        .map_err(preflight_error)?;
        if action.tag != 3 {
            return Err(ProgramError::Custom(50 + u32::from(action.tag)));
        }
        let descriptor_data = forwarded
            .get(1)
            .ok_or(TestTerminalCallerError::AccountFrame)?
            .try_borrow_data()
            .map_err(|_| TestTerminalCallerError::AccountFrame)?;
        let descriptor = DescriptorV1::decode(&descriptor_data).map_err(preflight_error)?;
        let state_data = forwarded
            .get(2)
            .ok_or(TestTerminalCallerError::AccountFrame)?
            .try_borrow_data()
            .map_err(|_| TestTerminalCallerError::AccountFrame)?;
        let state = StateV1::decode(&state_data).map_err(preflight_error)?;
        let admission = ClaimsReleaseAdmission {
            selected_release_set_id: descriptor.release_set_id(),
            receipt_release_set_id: descriptor.release_set_id(),
            registry_authenticated: true,
            claims_role_authenticated: true,
            activation_cache_authenticated: true,
            current_deployment_reauthenticated: true,
        };
        check_generated_rule_matrix(descriptor, state, action, admission)?;
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: request.to_vec(),
    };
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    invoke(&instruction, &infos).map_err(|_| TestTerminalCallerError::ClaimsCpi)?;
    let (producer, receipt) = get_return_data().ok_or(TestTerminalCallerError::ClaimsCpi)?;
    if producer != *claims_program.key {
        return Err(TestTerminalCallerError::ClaimsCpi.into());
    }
    if fail_after == 1 {
        return Err(TestTerminalCallerError::DeliberateLateFailure.into());
    }
    set_return_data(&receipt);
    Ok(())
}
