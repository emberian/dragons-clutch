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

use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
use dclutch_series_v3_kernel::AccountKeyV3;
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
pub const SERIES_RETIRE_ACCOUNT_COUNT_V3: usize = 4;
/// Exact account suffix for terminal Series-root closure.
pub const SERIES_CLOSE_ACCOUNT_COUNT_V3: usize = 3;

const ROOT: usize = 0;
const TICKET: usize = 1;
const RENT_CREDIT: usize = 2;
const RETIRE_RENT_PROGRAM: usize = 3;
const CLOSE_RENT_CREDIT: usize = 1;
const CLOSE_RENT_PROGRAM: usize = 2;

/// Execute one terminal Ticket retirement under the finalized Series owner.
///
/// Accounts are exactly `[Series root, Ticket replay, lifecycle RentCredit,
/// current Rent program]`. Root replay is committed after the Ticket credit
/// and deletion. The immutable Ticket refund owner is checked against the
/// credit's wallet but never receives this sub-resource close directly.
pub fn process_retire_v3(
    program_id: &Pubkey,
    action: AuthenticatedSeriesActionV3<'_>,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
) -> Result<(), ProgramError> {
    if action.action() != SeriesActionV3::Retire || accounts.len() != SERIES_RETIRE_ACCOUNT_COUNT_V3
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    let root = account(accounts, ROOT)?;
    let ticket = account(accounts, TICKET)?;
    let rent_credit = account(accounts, RENT_CREDIT)?;
    let rent_program = account(accounts, RETIRE_RENT_PROGRAM)?;
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
    let rent_sink = authenticate_rent_sink(
        root_state,
        action
            .ticket()
            .ok_or(SeriesAccountErrorV3::State)?
            .ticket()
            .refund_owner(),
        rent_credit,
        rent_program,
        rent,
    )?;
    let plan = action
        .plan_retire(
            root_state.state(),
            ticket_state,
            ticket.lamports(),
            rent.minimum_balance(ticket.data_len()),
            rent_sink,
        )
        .map_err(|_| SeriesAccountErrorV3::State)?;
    commit_retire_ticket(
        program_id,
        root,
        ticket,
        rent_credit,
        template.occurrence_count(),
        plan,
    )
}

/// Execute terminal Series-root closure under the finalized Series owner.
///
/// Accounts are exactly `[Series root, lifecycle RentCredit, current Rent
/// program]`. Current Rent classifies the root reserve; close Rent and
/// donations remain distinct even though all three credit the same lifecycle.
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
    let rent_credit = account(accounts, CLOSE_RENT_CREDIT)?;
    let rent_program = account(accounts, CLOSE_RENT_PROGRAM)?;
    let template = action.template();
    let root_state = authenticate_root(
        program_id,
        root,
        action.template_id(),
        template.occurrence_count(),
    )?;
    let rent_sink = authenticate_rent_sink(
        root_state,
        template.refund_owner(),
        rent_credit,
        rent_program,
        rent,
    )?;
    let plan = action
        .plan_close(
            root_state.state(),
            root.lamports(),
            rent.minimum_balance(root.data_len()),
            rent_sink,
        )
        .map_err(|_| SeriesAccountErrorV3::State)?;
    commit_close_root(program_id, root, rent_credit, plan)
}

fn authenticate_rent_sink(
    root: super::accounts::AuthenticatedSeriesRootV3,
    expected_wallet: AccountKeyV3,
    rent_credit: &AccountInfo<'_>,
    rent_program: &AccountInfo<'_>,
    rent: &Rent,
) -> Result<super::lifecycle::SeriesLifecycleRentSinkV3, ProgramError> {
    if rent_program.is_signer
        || rent_program.is_writable
        || !rent_program.executable
        || rent_credit.owner != rent_program.key
        || rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || rent_credit.is_signer
        || !rent_credit.is_writable
        || rent_credit.executable
        || !rent.is_exempt(rent_credit.lamports(), LIFECYCLE_RENT_CREDIT_BYTES_V2)
    {
        return Err(SeriesAccountErrorV3::Frame.into());
    }
    let header = root.header();
    let credit_key =
        AccountKeyV3::new(rent_credit.key.to_bytes()).map_err(|_| SeriesAccountErrorV3::State)?;
    let market = AccountKeyV3::new(header.market()).map_err(|_| SeriesAccountErrorV3::State)?;
    let data = rent_credit
        .try_borrow_data()
        .map_err(|_| SeriesAccountErrorV3::State)?;
    let sink = super::lifecycle::SeriesLifecycleRentSinkV3::admit(
        credit_key,
        &data,
        market,
        header.release_set(),
        header.generation(),
        expected_wallet,
    )
    .map_err(|_| SeriesAccountErrorV3::State)?;
    let market_bytes = sink.market().to_bytes();
    let generation = sink.generation().to_le_bytes();
    let bump = [sink.pda_bump()];
    let expected = Pubkey::create_program_address(
        &[
            dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            &market_bytes,
            &generation,
            &bump,
        ],
        rent_program.key,
    )
    .map_err(|_| SeriesAccountErrorV3::State)?;
    if expected != *rent_credit.key {
        return Err(SeriesAccountErrorV3::State.into());
    }
    Ok(sink)
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| SeriesAccountErrorV3::Frame.into())
}
