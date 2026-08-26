#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF shell around the canonical successor Dealer machine.
//!
//! This is not another release role or deployment artifact. It exists solely
//! to measure the exact allocation-free semantic machine in the SVM verifier
//! before the common hot Trading outer converges its account profile.

extern crate std;

use dclutch_dealer_codec::{Action, CandidateView, Policy, Request, State, interpret_projected};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

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

/// Execute one exact Dealer request over policy, Candidate, and state accounts.
///
/// The shell intentionally omits Registry and child CPI work; those are tested
/// by the canonical Trading modules. This entry isolates real SBF verifier,
/// stack, decode, checked arithmetic, and commit-last behavior.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request =
        Request::decode(instruction_data).map_err(|_| ProgramError::InvalidInstructionData)?;
    let replacement = matches!(
        request.action,
        Action::ScheduleReplacement | Action::ActivateReplacement
    );
    let expected_accounts = if replacement { 4 } else { 3 };
    if accounts.len() != expected_accounts {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let policy_account = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    let candidate_account = accounts.get(1).ok_or(ProgramError::NotEnoughAccountKeys)?;
    let auxiliary_account = if replacement {
        Some(accounts.get(2).ok_or(ProgramError::NotEnoughAccountKeys)?)
    } else {
        None
    };
    let state_account = accounts
        .get(expected_accounts - 1)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    if policy_account.owner != program_id
        || candidate_account.owner != program_id
        || state_account.owner != program_id
        || policy_account.is_writable
        || candidate_account.is_writable
        || !state_account.is_writable
        || auxiliary_account
            .is_some_and(|account| account.owner != program_id || account.is_writable)
        || accounts
            .iter()
            .any(|account| account.is_signer || account.executable)
    {
        return Err(ProgramError::InvalidAccountData);
    }
    let policy_data = policy_account.try_borrow_data()?;
    let candidate_data = candidate_account.try_borrow_data()?;
    let state_data = state_account.try_borrow_data()?;
    let auxiliary_data = auxiliary_account
        .map(|account| account.try_borrow_data())
        .transpose()?;
    let post = project_post(
        &policy_data,
        &candidate_data,
        auxiliary_data.as_ref().map(|data| data.as_ref()),
        &state_data,
        request,
    )?;
    drop(state_data);
    drop(auxiliary_data);
    drop(candidate_data);
    drop(policy_data);
    let mut writable = state_account.try_borrow_mut_data()?;
    post.encode_into(&mut writable)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    Ok(())
}

#[inline(never)]
fn project_post(
    policy_data: &[u8],
    candidate_data: &[u8],
    auxiliary_data: Option<&[u8]>,
    state_data: &[u8],
    request: Request,
) -> Result<State, ProgramError> {
    let policy = Policy::decode(policy_data).map_err(|_| ProgramError::InvalidAccountData)?;
    let active =
        CandidateView::decode(candidate_data).map_err(|_| ProgramError::InvalidAccountData)?;
    let state = State::decode(state_data).map_err(|_| ProgramError::InvalidAccountData)?;
    let auxiliary = auxiliary_data
        .map(CandidateView::decode)
        .transpose()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let (pending, proposed) = match request.action {
        Action::ScheduleReplacement => (None, auxiliary),
        Action::ActivateReplacement => (auxiliary, None),
        _ => (None, None),
    };
    interpret_projected(policy, active, pending, proposed, state, request)
        .map(|transition| transition.post)
        .map_err(|_| ProgramError::InvalidInstructionData)
}
