//! Executable terminal Series V3 differential oracle.
//!
//! These helpers accept an already finalized-content-joined family action. They
//! reauthenticate the mutable Trading PDAs, derive the exact lifecycle plan,
//! and perform only commit-last Ticket retirement or root closure. No Core,
//! Market, Claims, or Custody authority is fabricated for controller actions.
//! They are measurement and differential-test evidence for the generic
//! Account/Request/Transition/Effect interpreter, which remains the sole
//! canonical Trading state writer and CPI authority. Common dispatch must not
//! select this module by a Series tag.

use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, rent::Rent,
};

use super::{
    accounts::{
        SeriesAccountErrorV3, authenticate_root, authenticate_ticket, commit_close_root,
        commit_retire_ticket,
    },
    instruction::SeriesActionV3,
    projector::AuthenticatedSeriesActionV3,
};

/// Exact account suffix for one terminal Ticket retirement.
pub const SERIES_RETIRE_ACCOUNT_COUNT_V3: usize = 3;
/// Exact account suffix for terminal Series-root closure.
pub const SERIES_CLOSE_ACCOUNT_COUNT_V3: usize = 2;

const ROOT: usize = 0;
const TICKET: usize = 1;
const BENEFICIARY: usize = 2;
const CLOSE_BENEFICIARY: usize = 1;

/// Execute one terminal Ticket retirement under the finalized Series owner.
///
/// Accounts are exactly `[Series root, Ticket replay, Ticket refund owner]`.
/// Root replay state is committed after the Ticket refund and deletion.
pub fn process_retire_v3(
    program_id: &Pubkey,
    action: AuthenticatedSeriesActionV3<'_>,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if action.action() != SeriesActionV3::Retire || accounts.len() != SERIES_RETIRE_ACCOUNT_COUNT_V3
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    let root = account(accounts, ROOT)?;
    let ticket = account(accounts, TICKET)?;
    let beneficiary = account(accounts, BENEFICIARY)?;
    let template = action.template();
    let root_state = authenticate_root(
        program_id,
        root,
        action.template_id(),
        template.occurrence_count(),
    )?;
    let ticket_record = action
        .ticket()
        .ok_or(SeriesAccountErrorV3::State)?
        .content_id();
    let ticket_state = authenticate_ticket(program_id, root.key, ticket, ticket_record)?;
    let plan = action
        .plan_retire(root_state.state(), ticket_state, ticket.lamports())
        .map_err(|_| SeriesAccountErrorV3::State)?;
    commit_retire_ticket(
        program_id,
        root,
        ticket,
        beneficiary,
        template.occurrence_count(),
        plan,
    )
}

/// Execute terminal Series-root closure under the finalized Series owner.
///
/// Accounts are exactly `[Series root, Template refund owner]`. Current Rent
/// classifies the root reserve; close Rent and donations remain distinct in
/// the pure plan even though all three refund to the same immutable owner.
pub fn process_close_v3(
    program_id: &Pubkey,
    action: AuthenticatedSeriesActionV3<'_>,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
) -> Result<(), ProgramError> {
    if action.action() != SeriesActionV3::Close || accounts.len() != SERIES_CLOSE_ACCOUNT_COUNT_V3 {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    let root = account(accounts, ROOT)?;
    let beneficiary = account(accounts, CLOSE_BENEFICIARY)?;
    let template = action.template();
    let root_state = authenticate_root(
        program_id,
        root,
        action.template_id(),
        template.occurrence_count(),
    )?;
    let plan = action
        .plan_close(
            root_state.state(),
            root.lamports(),
            rent.minimum_balance(root.data_len()),
        )
        .map_err(|_| SeriesAccountErrorV3::State)?;
    commit_close_root(program_id, root, beneficiary, plan)
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| SeriesAccountErrorV3::Frame.into())
}
