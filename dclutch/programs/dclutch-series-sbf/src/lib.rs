#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(unexpected_cfgs)]

//! Authenticated SBF adapter for recurring Series.
//!
//! The adapter owns Template, Series, and Ticket PDA persistence. It invokes
//! Registry/Core to reauthenticate the current Core deployment, then stages one
//! canonical Core envelope and Series request. Core owns downstream Claims and
//! Custody dispatch. State bytes are committed only after Core returns the exact
//! normalized acknowledgment. Solana's atomic transaction boundary rolls back
//! earlier account creation, CPI, and lamport movements on any later refusal.

extern crate alloc;

use alloc::vec::Vec;
use dclutch_market_core_codec::{
    Identity as CoreIdentity, SERIES_CORE_REQUEST_BYTES_V1, SeriesCoreAckV1, SeriesCoreActionV1,
    SeriesCoreCallerSeedsV1, SeriesCoreRequestV1,
};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_series_codec::{
    Action, InvocationV1, Limits, Phase, ReleaseReceiptV1, RequestV1, SeriesStateV1, TemplateV1,
    TicketPhase, TicketV1, interpret,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer as system_transfer};

/// PDA domain for immutable Series templates.
pub const TEMPLATE_PDA_DOMAIN_V1: &[u8] = b"dclutch-series-template-v1";
/// PDA domain for one template's replay cursor.
pub const SERIES_PDA_DOMAIN_V1: &[u8] = b"dclutch-series-state-v1";
/// PDA domain for each occurrence Ticket.
pub const TICKET_PDA_DOMAIN_V1: &[u8] = b"dclutch-series-ticket-v1";
/// Exact Series-owned account prefix before the opaque Core-owned account tail.
pub const ACCOUNT_PREFIX_V1: usize = 14;
/// Bootstrap instruction is Template || Series || Ticket.
pub const BOOTSTRAP_BYTES_V1: usize = dclutch_series_codec::TEMPLATE_BYTES
    + dclutch_series_codec::SERIES_STATE_BYTES
    + dclutch_series_codec::TICKET_BYTES;

const ACTOR: usize = 0;
const TEMPLATE: usize = 1;
const SERIES: usize = 2;
const TICKET: usize = 3;
const REGISTRY_PROGRAM: usize = 4;
const ACTIVATION_CACHE: usize = 5;
const CORE_PROGRAM: usize = 6;
const CORE_PROGRAMDATA: usize = 7;
const SYSTEM_PROGRAM: usize = 8;
const RENT_SYSVAR: usize = 9;
const CLOCK_SYSVAR: usize = 10;
const MARKET: usize = 11;
const BENEFICIARY: usize = 12;
const CORE_CALLER_AUTHORITY: usize = 13;

/// Stable physical refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesSbfError {
    /// Account count or privilege projection was not canonical.
    AccountFrame = 0,
    /// Instruction bytes did not match bootstrap, ticket funding, or execution.
    Instruction = 1,
    /// Template, Series, or Ticket bytes refused hostile decoding.
    Codec = 2,
    /// PDA identity, owner, width, or initialization state was wrong.
    AccountIdentity = 3,
    /// Rent, Clock, or System program authentication refused.
    Sysvar = 4,
    /// Registry invocation, return-data producer, or role receipt refused.
    Release = 5,
    /// The generated Series interpreter refused the transition.
    Semantic = 6,
    /// System account creation refused.
    Create = 7,
    /// Core, Claims, or Custody staging CPI refused.
    RoleCpi = 8,
    /// Exact prepaid lamport projection refused.
    Funding = 9,
    /// Account data or lamports could not be borrowed.
    Borrow = 10,
    /// Candidate bytes could not be committed after all effects.
    Commit = 11,
}

impl From<SeriesSbfError> for ProgramError {
    fn from(value: SeriesSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Bootstrap, fund the next Ticket, or execute one transition.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != BOOTSTRAP_BYTES_V1
        && instruction_data.len() != dclutch_series_codec::TICKET_BYTES
        && instruction_data.len() != dclutch_series_codec::REQUEST_BYTES
    {
        return Err(SeriesSbfError::Instruction.into());
    }
    validate_frame(accounts, instruction_data.len())?;
    let rent = authenticate_rent(accounts)?;
    let clock = authenticate_clock(accounts)?;
    let core_receipt = authenticate_core(accounts)?;
    match instruction_data.len() {
        BOOTSTRAP_BYTES_V1 => bootstrap(program_id, accounts, instruction_data, rent, core_receipt),
        dclutch_series_codec::TICKET_BYTES => {
            fund_ticket(program_id, accounts, instruction_data, rent, core_receipt)
        }
        dclutch_series_codec::REQUEST_BYTES => execute_transition(
            program_id,
            accounts,
            instruction_data,
            rent,
            clock,
            core_receipt,
        ),
        _ => Err(SeriesSbfError::Instruction.into()),
    }
}

#[inline(never)]
fn bootstrap(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
    rent: Rent,
    core_receipt: AuthenticatedRoleReceiptV1,
) -> ProgramResult {
    let template_end = dclutch_series_codec::TEMPLATE_BYTES;
    let series_end = template_end + dclutch_series_codec::SERIES_STATE_BYTES;
    let template = TemplateV1::decode(
        bytes
            .get(..template_end)
            .ok_or(SeriesSbfError::Instruction)?,
    )
    .map_err(|_| SeriesSbfError::Codec)?;
    validate_template_schedule(template)?;
    let series = SeriesStateV1::decode(
        bytes
            .get(template_end..series_end)
            .ok_or(SeriesSbfError::Instruction)?,
    )
    .map_err(|_| SeriesSbfError::Codec)?;
    let ticket = TicketV1::decode(bytes.get(series_end..).ok_or(SeriesSbfError::Instruction)?)
        .map_err(|_| SeriesSbfError::Codec)?;
    validate_initial(template, series, ticket)?;
    authenticate_release_set(template, core_receipt)?;
    authenticate_pdas(program_id, accounts, template, series, ticket, true)?;
    create_state_account(
        program_id,
        accounts,
        TEMPLATE,
        dclutch_series_codec::TEMPLATE_BYTES,
        rent.minimum_balance(dclutch_series_codec::TEMPLATE_BYTES),
        &[TEMPLATE_PDA_DOMAIN_V1, &template.template_id],
    )?;
    let series_lamports = rent
        .minimum_balance(dclutch_series_codec::SERIES_STATE_BYTES)
        .checked_add(template.series_close_rent_lamports)
        .ok_or(SeriesSbfError::Funding)?;
    create_state_account(
        program_id,
        accounts,
        SERIES,
        dclutch_series_codec::SERIES_STATE_BYTES,
        series_lamports,
        &[SERIES_PDA_DOMAIN_V1, &template.template_id],
    )?;
    let ticket_lamports = rent.minimum_balance(dclutch_series_codec::TICKET_BYTES);
    let occurrence = ticket.occurrence.to_le_bytes();
    create_state_account(
        program_id,
        accounts,
        TICKET,
        dclutch_series_codec::TICKET_BYTES,
        ticket_lamports,
        &[TICKET_PDA_DOMAIN_V1, &template.template_id, &occurrence],
    )?;
    write_state(
        accounts,
        TEMPLATE,
        &template.to_bytes().map_err(|_| SeriesSbfError::Codec)?,
    )?;
    write_state(
        accounts,
        SERIES,
        &series.to_bytes().map_err(|_| SeriesSbfError::Codec)?,
    )?;
    write_state(
        accounts,
        TICKET,
        &ticket.to_bytes().map_err(|_| SeriesSbfError::Codec)?,
    )?;
    invoke_core(
        program_id,
        accounts,
        template,
        series,
        ticket,
        SeriesCoreActionV1::Prepare,
        ticket.refund_owner,
        core_receipt,
    )
}

#[inline(never)]
fn fund_ticket(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
    rent: Rent,
    core_receipt: AuthenticatedRoleReceiptV1,
) -> ProgramResult {
    let template = read_template(accounts)?;
    validate_template_schedule(template)?;
    let series = read_series(accounts)?;
    let ticket = TicketV1::decode(bytes).map_err(|_| SeriesSbfError::Codec)?;
    if series.phase != Phase::Active
        || ticket.phase != TicketPhase::Ready
        || ticket.template_id != template.template_id
        || ticket.occurrence != series.next_occurrence
        || ticket.funds.hoard_principal != template.seed_quantity
        || ticket.funds.market_rent != template.market_rent_lamports
        || ticket.funds.capability_rent != template.capability_rent_lamports
        || ticket.funds.founding_work != template.founding_work_lamports
    {
        return Err(SeriesSbfError::Semantic.into());
    }
    authenticate_release_set(template, core_receipt)?;
    authenticate_existing_template_series(program_id, accounts, template, series)?;
    authenticate_ticket_pda(program_id, accounts, template, ticket, true)?;
    let ticket_lamports = rent.minimum_balance(dclutch_series_codec::TICKET_BYTES);
    let occurrence = ticket.occurrence.to_le_bytes();
    create_state_account(
        program_id,
        accounts,
        TICKET,
        dclutch_series_codec::TICKET_BYTES,
        ticket_lamports,
        &[TICKET_PDA_DOMAIN_V1, &template.template_id, &occurrence],
    )?;
    write_state(
        accounts,
        TICKET,
        &ticket.to_bytes().map_err(|_| SeriesSbfError::Codec)?,
    )?;
    invoke_core(
        program_id,
        accounts,
        template,
        series,
        ticket,
        SeriesCoreActionV1::Prepare,
        ticket.refund_owner,
        core_receipt,
    )
}

#[inline(never)]
fn execute_transition(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
    rent: Rent,
    clock: Clock,
    core_receipt: AuthenticatedRoleReceiptV1,
) -> ProgramResult {
    let prepared = derive_transition(program_id, accounts, bytes, rent, clock, core_receipt)?;
    stage_prepared(program_id, accounts, bytes, prepared.market, core_receipt)?;
    apply_terminal_close_lamports(accounts, bytes)?;
    commit_candidate_bytes(accounts, &prepared.series, &prepared.ticket)
}

struct PreparedTransition {
    series: [u8; dclutch_series_codec::SERIES_STATE_BYTES],
    ticket: [u8; dclutch_series_codec::TICKET_BYTES],
    market: Option<dclutch_series_codec::MarketFoundingV1>,
}

#[inline(never)]
fn derive_transition(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
    rent: Rent,
    clock: Clock,
    core_receipt: AuthenticatedRoleReceiptV1,
) -> Result<PreparedTransition, ProgramError> {
    let template = read_template(accounts)?;
    validate_template_schedule(template)?;
    let series = read_series(accounts)?;
    let ticket = read_ticket(accounts)?;
    let request = RequestV1::decode(bytes).map_err(|_| SeriesSbfError::Codec)?;
    if request.now_slot != clock.slot {
        return Err(SeriesSbfError::Sysvar.into());
    }
    authenticate_release_set(template, core_receipt)?;
    authenticate_pdas(program_id, accounts, template, series, ticket, false)?;
    validate_lamport_floors(accounts, template, series, rent)?;
    authenticate_market_and_beneficiary(accounts, template, ticket, request)?;
    let release_receipt = ReleaseReceiptV1 {
        registry_program: accounts[REGISTRY_PROGRAM].key.to_bytes(),
        release_set_id: *core_receipt.execution_release_set_id().as_bytes(),
        observed_program: *core_receipt.program().as_bytes(),
        artifact_release: *core_receipt.artifact_release_id().as_bytes(),
        semantic_release: *core_receipt.semantic_release_id().as_bytes(),
    };
    let candidate = interpret(InvocationV1 {
        template,
        series,
        ticket,
        release_receipt,
        request,
        limits: Limits {
            slot_limit: u64::MAX,
            lamport_limit: u64::MAX,
            revision_limit: u64::MAX,
        },
    })
    .map_err(|_| SeriesSbfError::Semantic)?;
    let series = candidate
        .series
        .to_bytes()
        .map_err(|_| SeriesSbfError::Commit)?;
    let ticket = candidate
        .ticket
        .to_bytes()
        .map_err(|_| SeriesSbfError::Commit)?;
    Ok(PreparedTransition {
        series,
        ticket,
        market: candidate.market,
    })
}

#[inline(never)]
fn stage_prepared(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request_bytes: &[u8],
    market: Option<dclutch_series_codec::MarketFoundingV1>,
    core_receipt: AuthenticatedRoleReceiptV1,
) -> ProgramResult {
    let template = read_template(accounts)?;
    let series = read_series(accounts)?;
    let ticket = read_ticket(accounts)?;
    let request = RequestV1::decode(request_bytes).map_err(|_| SeriesSbfError::Codec)?;
    let (action, beneficiary) = match request.action {
        Action::Consume => {
            let _ = market.ok_or(SeriesSbfError::Semantic)?;
            (SeriesCoreActionV1::Consume, request.work_recipient)
        }
        Action::Expire => (SeriesCoreActionV1::Expire, ticket.refund_owner),
        Action::Close => (SeriesCoreActionV1::Close, template.series_refund_owner),
    };
    invoke_core(
        program_id,
        accounts,
        template,
        series,
        ticket,
        action,
        beneficiary,
        core_receipt,
    )
}

#[inline(never)]
fn apply_terminal_close_lamports(
    accounts: &[AccountInfo<'_>],
    request_bytes: &[u8],
) -> ProgramResult {
    let template = read_template(accounts)?;
    let request = RequestV1::decode(request_bytes).map_err(|_| SeriesSbfError::Codec)?;
    if request.action != Action::Close || template.series_close_rent_lamports == 0 {
        return Ok(());
    }
    transfer_lamports(
        accounts,
        SERIES,
        BENEFICIARY,
        template.series_close_rent_lamports,
    )
}

fn validate_frame(accounts: &[AccountInfo<'_>], instruction_len: usize) -> ProgramResult {
    if accounts.len() < ACCOUNT_PREFIX_V1 {
        return Err(SeriesSbfError::AccountFrame.into());
    }
    let actor = &accounts[ACTOR];
    if !actor.is_signer || !actor.is_writable || actor.executable {
        return Err(SeriesSbfError::AccountFrame.into());
    }
    for index in [TICKET, MARKET, BENEFICIARY] {
        if !accounts[index].is_writable || accounts[index].is_signer || accounts[index].executable {
            return Err(SeriesSbfError::AccountFrame.into());
        }
    }
    let bootstrap = instruction_len == BOOTSTRAP_BYTES_V1;
    let fund = instruction_len == dclutch_series_codec::TICKET_BYTES;
    let series_writable = !fund;
    if accounts[TEMPLATE].is_writable != bootstrap
        || accounts[TEMPLATE].is_signer
        || accounts[TEMPLATE].executable
        || accounts[SERIES].is_writable != series_writable
        || accounts[SERIES].is_signer
        || accounts[SERIES].executable
    {
        return Err(SeriesSbfError::AccountFrame.into());
    }
    for index in [REGISTRY_PROGRAM, CORE_PROGRAM, SYSTEM_PROGRAM] {
        if !accounts[index].executable || accounts[index].is_writable || accounts[index].is_signer {
            return Err(SeriesSbfError::AccountFrame.into());
        }
    }
    for index in [
        ACTIVATION_CACHE,
        CORE_PROGRAMDATA,
        RENT_SYSVAR,
        CLOCK_SYSVAR,
        CORE_CALLER_AUTHORITY,
    ] {
        if accounts[index].is_writable || accounts[index].is_signer || accounts[index].executable {
            return Err(SeriesSbfError::AccountFrame.into());
        }
    }
    if accounts[SYSTEM_PROGRAM].key != &system_program::ID {
        return Err(SeriesSbfError::AccountFrame.into());
    }
    Ok(())
}

fn authenticate_rent(accounts: &[AccountInfo<'_>]) -> Result<Rent, ProgramError> {
    if accounts[RENT_SYSVAR].key != &sysvar::rent::ID {
        return Err(SeriesSbfError::Sysvar.into());
    }
    Rent::from_account_info(&accounts[RENT_SYSVAR]).map_err(|_| SeriesSbfError::Sysvar.into())
}

fn authenticate_clock(accounts: &[AccountInfo<'_>]) -> Result<Clock, ProgramError> {
    if accounts[CLOCK_SYSVAR].key != &sysvar::clock::ID {
        return Err(SeriesSbfError::Sysvar.into());
    }
    Clock::from_account_info(&accounts[CLOCK_SYSVAR]).map_err(|_| SeriesSbfError::Sysvar.into())
}

#[inline(never)]
fn authenticate_core(
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    reauthenticate(
        accounts,
        ExecutionRoleV1::Core,
        CORE_PROGRAM,
        CORE_PROGRAMDATA,
    )
}

fn reauthenticate(
    accounts: &[AccountInfo<'_>],
    role: ExecutionRoleV1,
    role_program: usize,
    role_programdata: usize,
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    let registry = &accounts[REGISTRY_PROGRAM];
    let cache = &accounts[ACTIVATION_CACHE];
    let program = &accounts[role_program];
    let programdata = &accounts[role_programdata];
    let instruction = Instruction {
        program_id: *registry.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*cache.key, false),
            AccountMeta::new_readonly(*program.key, false),
            AccountMeta::new_readonly(*programdata.key, false),
        ]),
        data: RegistryInstructionV1::Reauthenticate(role)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            cache.clone(),
            program.clone(),
            programdata.clone(),
            registry.clone(),
        ],
    )
    .map_err(|_| SeriesSbfError::Release)?;
    let (producer, bytes) = get_return_data().ok_or(SeriesSbfError::Release)?;
    if producer != *registry.key {
        return Err(SeriesSbfError::Release.into());
    }
    let receipt =
        AuthenticatedRoleReceiptV1::decode(&bytes).map_err(|_| SeriesSbfError::Release)?;
    if receipt.role() != role || receipt.program().as_bytes() != &program.key.to_bytes() {
        return Err(SeriesSbfError::Release.into());
    }
    Ok(receipt)
}

fn authenticate_release_set(
    template: TemplateV1,
    core_receipt: AuthenticatedRoleReceiptV1,
) -> ProgramResult {
    if core_receipt.execution_release_set_id().as_bytes() != &template.release_set_id {
        return Err(SeriesSbfError::Release.into());
    }
    Ok(())
}

fn authenticate_pdas(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    template: TemplateV1,
    series: SeriesStateV1,
    ticket: TicketV1,
    vacant: bool,
) -> ProgramResult {
    let template_key =
        Pubkey::find_program_address(&[TEMPLATE_PDA_DOMAIN_V1, &template.template_id], program_id)
            .0;
    let series_key =
        Pubkey::find_program_address(&[SERIES_PDA_DOMAIN_V1, &template.template_id], program_id).0;
    if accounts[TEMPLATE].key != &template_key
        || accounts[SERIES].key != &series_key
        || series.template_id != template.template_id
    {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    if !vacant {
        authenticate_existing_template_series(program_id, accounts, template, series)?;
    }
    authenticate_ticket_pda(program_id, accounts, template, ticket, vacant)
}

fn authenticate_existing_template_series(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    template: TemplateV1,
    series: SeriesStateV1,
) -> ProgramResult {
    authenticate_owned(
        accounts,
        TEMPLATE,
        program_id,
        dclutch_series_codec::TEMPLATE_BYTES,
    )?;
    authenticate_owned(
        accounts,
        SERIES,
        program_id,
        dclutch_series_codec::SERIES_STATE_BYTES,
    )?;
    let expected_template =
        Pubkey::find_program_address(&[TEMPLATE_PDA_DOMAIN_V1, &template.template_id], program_id)
            .0;
    let expected_series =
        Pubkey::find_program_address(&[SERIES_PDA_DOMAIN_V1, &template.template_id], program_id).0;
    if accounts[TEMPLATE].key != &expected_template
        || accounts[SERIES].key != &expected_series
        || series.template_id != template.template_id
    {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    Ok(())
}

fn authenticate_ticket_pda(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    template: TemplateV1,
    ticket: TicketV1,
    vacant: bool,
) -> ProgramResult {
    let occurrence = ticket.occurrence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[TICKET_PDA_DOMAIN_V1, &template.template_id, &occurrence],
        program_id,
    )
    .0;
    if accounts[TICKET].key != &expected {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    if vacant {
        validate_vacant_state_account(&accounts[TICKET])?;
    } else {
        authenticate_owned(
            accounts,
            TICKET,
            program_id,
            dclutch_series_codec::TICKET_BYTES,
        )?;
    }
    Ok(())
}

fn validate_initial(
    template: TemplateV1,
    series: SeriesStateV1,
    ticket: TicketV1,
) -> ProgramResult {
    if series.template_id != template.template_id
        || series.phase != Phase::Active
        || series.next_occurrence != 0
        || series.revision != 0
        || series.close_rent_lamports != template.series_close_rent_lamports
        || ticket.template_id != template.template_id
        || ticket.phase != TicketPhase::Ready
        || ticket.occurrence != 0
        || ticket.revision != 0
        || ticket.funds.hoard_principal != template.seed_quantity
        || ticket.funds.market_rent != template.market_rent_lamports
        || ticket.funds.capability_rent != template.capability_rent_lamports
        || ticket.funds.founding_work != template.founding_work_lamports
    {
        return Err(SeriesSbfError::Semantic.into());
    }
    Ok(())
}

fn validate_template_schedule(template: TemplateV1) -> ProgramResult {
    let last = template
        .occurrence_count
        .checked_sub(1)
        .ok_or(SeriesSbfError::Semantic)?;
    let retry_through = template
        .period_slots
        .checked_mul(u64::from(last))
        .and_then(|offset| template.first_occurrence_slot.checked_add(offset))
        .and_then(|due| due.checked_add(template.retry_window_slots))
        .ok_or(SeriesSbfError::Semantic)?;
    if retry_through == u64::MAX {
        return Err(SeriesSbfError::Semantic.into());
    }
    Ok(())
}

fn read_template(accounts: &[AccountInfo<'_>]) -> Result<TemplateV1, ProgramError> {
    let data = accounts[TEMPLATE]
        .try_borrow_data()
        .map_err(|_| SeriesSbfError::Borrow)?;
    TemplateV1::decode(&data).map_err(|_| SeriesSbfError::Codec.into())
}

fn read_series(accounts: &[AccountInfo<'_>]) -> Result<SeriesStateV1, ProgramError> {
    let data = accounts[SERIES]
        .try_borrow_data()
        .map_err(|_| SeriesSbfError::Borrow)?;
    SeriesStateV1::decode(&data).map_err(|_| SeriesSbfError::Codec.into())
}

fn read_ticket(accounts: &[AccountInfo<'_>]) -> Result<TicketV1, ProgramError> {
    let data = accounts[TICKET]
        .try_borrow_data()
        .map_err(|_| SeriesSbfError::Borrow)?;
    TicketV1::decode(&data).map_err(|_| SeriesSbfError::Codec.into())
}

fn authenticate_owned(
    accounts: &[AccountInfo<'_>],
    index: usize,
    owner: &Pubkey,
    width: usize,
) -> ProgramResult {
    if accounts[index].owner != owner || accounts[index].data_len() != width {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    Ok(())
}

fn validate_lamport_floors(
    accounts: &[AccountInfo<'_>],
    template: TemplateV1,
    series: SeriesStateV1,
    rent: Rent,
) -> ProgramResult {
    let expected_series = rent
        .minimum_balance(dclutch_series_codec::SERIES_STATE_BYTES)
        .checked_add(series.close_rent_lamports)
        .ok_or(SeriesSbfError::Funding)?;
    let expected_ticket = rent.minimum_balance(dclutch_series_codec::TICKET_BYTES);
    if accounts[TEMPLATE].lamports() < rent.minimum_balance(dclutch_series_codec::TEMPLATE_BYTES)
        || accounts[SERIES].lamports() < expected_series
        || accounts[TICKET].lamports() < expected_ticket
        || series.close_rent_lamports > template.series_close_rent_lamports
    {
        return Err(SeriesSbfError::Funding.into());
    }
    Ok(())
}

fn authenticate_market_and_beneficiary(
    accounts: &[AccountInfo<'_>],
    template: TemplateV1,
    ticket: TicketV1,
    request: RequestV1,
) -> ProgramResult {
    if accounts[MARKET].key.to_bytes() != ticket.committed_market_id {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    let beneficiary = match request.action {
        Action::Consume => request.work_recipient,
        Action::Expire => ticket.refund_owner,
        Action::Close => template.series_refund_owner,
    };
    if accounts[BENEFICIARY].key.to_bytes() != beneficiary {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PreparedCoreCall {
    request: SeriesCoreRequestV1,
    request_bytes: [u8; SERIES_CORE_REQUEST_BYTES_V1],
    request_digest: CoreIdentity,
    caller_authority: Pubkey,
    caller_bump: u8,
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn prepare_core_call(
    program_id: &Pubkey,
    template: TemplateV1,
    series: SeriesStateV1,
    ticket: TicketV1,
    action: SeriesCoreActionV1,
    beneficiary: [u8; 32],
) -> Result<PreparedCoreCall, ProgramError> {
    let release_set = core_identity(template.release_set_id)?;
    let template_id = core_identity(template.template_id)?;
    let market = core_identity(ticket.committed_market_id)?;
    let request = if action == SeriesCoreActionV1::Close {
        SeriesCoreRequestV1::close(
            release_set,
            template_id,
            core_identity(beneficiary)?,
            series.revision,
            series.close_rent_lamports,
        )
    } else {
        SeriesCoreRequestV1::occurrence(
            action,
            release_set,
            template_id,
            core_identity(ticket.ticket_id)?,
            market,
            core_identity(template.realm_id)?,
            core_identity(template.product_id)?,
            core_identity(beneficiary)?,
            core_identity(ticket.founder)?,
            ticket.occurrence,
            series.revision,
            ticket.revision,
            ticket.funds.market_rent,
            ticket.funds.capability_rent,
            ticket.funds.founding_work,
            ticket.funds.hoard_principal,
        )
    }
    .map_err(|_| SeriesSbfError::RoleCpi)?;
    let request_bytes = request.encode().map_err(|_| SeriesSbfError::RoleCpi)?;
    let request_digest = core_identity(hash(&request_bytes).to_bytes())?;
    let caller_seeds = SeriesCoreCallerSeedsV1::new(request, request_digest);
    let (caller_authority, caller_bump) =
        Pubkey::find_program_address(&caller_seeds.as_slices(), program_id);
    Ok(PreparedCoreCall {
        request,
        request_bytes,
        request_digest,
        caller_authority,
        caller_bump,
    })
}

fn core_identity(bytes: [u8; 32]) -> Result<CoreIdentity, ProgramError> {
    CoreIdentity::new(bytes).map_err(|_| SeriesSbfError::RoleCpi.into())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn invoke_core(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    template: TemplateV1,
    series: SeriesStateV1,
    ticket: TicketV1,
    action: SeriesCoreActionV1,
    beneficiary: [u8; 32],
    core_receipt: AuthenticatedRoleReceiptV1,
) -> ProgramResult {
    if accounts[MARKET].key.to_bytes() != ticket.committed_market_id
        || accounts[BENEFICIARY].key.to_bytes() != beneficiary
    {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    let prepared = prepare_core_call(program_id, template, series, ticket, action, beneficiary)?;
    if accounts[CORE_CALLER_AUTHORITY].key != &prepared.caller_authority {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    let forwarded = accounts
        .get(CORE_CALLER_AUTHORITY..)
        .ok_or(SeriesSbfError::AccountFrame)?;
    let mut metas = Vec::with_capacity(forwarded.len());
    let mut infos = Vec::with_capacity(forwarded.len().saturating_add(1));
    for (offset, account) in forwarded.iter().enumerate() {
        let signer = offset == 0 || account.is_signer;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
        infos.push(account.clone());
    }
    let program = &accounts[CORE_PROGRAM];
    let instruction = Instruction {
        program_id: *program.key,
        accounts: metas,
        data: prepared.request_bytes.to_vec(),
    };
    infos.push(program.clone());
    let caller_seeds = SeriesCoreCallerSeedsV1::new(prepared.request, prepared.request_digest);
    let seed_slices = caller_seeds.as_slices();
    let bump = [prepared.caller_bump];
    let mut signer = Vec::from(seed_slices);
    signer.push(bump.as_slice());
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| SeriesSbfError::RoleCpi)?;
    let (producer, bytes) = get_return_data().ok_or(SeriesSbfError::RoleCpi)?;
    if producer != *program.key {
        return Err(SeriesSbfError::RoleCpi.into());
    }
    let ack = SeriesCoreAckV1::decode(&bytes).map_err(|_| SeriesSbfError::RoleCpi)?;
    ack.validate_for(
        prepared.request,
        core_identity(*core_receipt.program().as_bytes())?,
        prepared.request_digest,
        ack.post_resource_digest(),
    )
    .map_err(|_| SeriesSbfError::RoleCpi)?;
    Ok(())
}

fn transfer_lamports(
    accounts: &[AccountInfo<'_>],
    source_index: usize,
    destination_index: usize,
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    let source_after = accounts[source_index]
        .lamports()
        .checked_sub(amount)
        .ok_or(SeriesSbfError::Funding)?;
    let destination_after = accounts[destination_index]
        .lamports()
        .checked_add(amount)
        .ok_or(SeriesSbfError::Funding)?;
    **accounts[source_index]
        .try_borrow_mut_lamports()
        .map_err(|_| SeriesSbfError::Borrow)? = source_after;
    **accounts[destination_index]
        .try_borrow_mut_lamports()
        .map_err(|_| SeriesSbfError::Borrow)? = destination_after;
    Ok(())
}

#[cfg(test)]
fn stage_and_encode<F>(
    candidate: dclutch_series_codec::AtomicCandidateV1,
    stage: F,
) -> Result<
    (
        [u8; dclutch_series_codec::SERIES_STATE_BYTES],
        [u8; dclutch_series_codec::TICKET_BYTES],
    ),
    ProgramError,
>
where
    F: FnOnce() -> ProgramResult,
{
    stage()?;
    let series = candidate
        .series
        .to_bytes()
        .map_err(|_| SeriesSbfError::Commit)?;
    let ticket = candidate
        .ticket
        .to_bytes()
        .map_err(|_| SeriesSbfError::Commit)?;
    Ok((series, ticket))
}

fn commit_candidate_bytes(
    accounts: &[AccountInfo<'_>],
    series: &[u8; dclutch_series_codec::SERIES_STATE_BYTES],
    ticket: &[u8; dclutch_series_codec::TICKET_BYTES],
) -> ProgramResult {
    write_state(accounts, SERIES, series)?;
    write_state(accounts, TICKET, ticket)
}

fn write_state(accounts: &[AccountInfo<'_>], index: usize, bytes: &[u8]) -> ProgramResult {
    let mut data = accounts[index]
        .try_borrow_mut_data()
        .map_err(|_| SeriesSbfError::Borrow)?;
    if data.len() != bytes.len() {
        return Err(SeriesSbfError::Commit.into());
    }
    data.copy_from_slice(bytes);
    Ok(())
}

fn create_state_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    index: usize,
    width: usize,
    required_lamports: u64,
    seeds: &[&[u8]],
) -> ProgramResult {
    validate_vacant_state_account(&accounts[index])?;
    let top_up = required_top_up(accounts[index].lamports(), required_lamports);
    if top_up != 0 {
        let instruction = system_transfer(accounts[ACTOR].key, accounts[index].key, top_up);
        invoke(
            &instruction,
            &[
                accounts[ACTOR].clone(),
                accounts[index].clone(),
                accounts[SYSTEM_PROGRAM].clone(),
            ],
        )
        .map_err(|_| SeriesSbfError::Create)?;
    }
    let space = u64::try_from(width).map_err(|_| SeriesSbfError::Create)?;
    let (_, bump) = Pubkey::find_program_address(seeds, program_id);
    let bump_seed = [bump];
    let mut signer = Vec::from(seeds);
    signer.push(&bump_seed);
    invoke_signed(
        &allocate(accounts[index].key, space),
        &[accounts[index].clone(), accounts[SYSTEM_PROGRAM].clone()],
        &[&signer],
    )
    .map_err(|_| SeriesSbfError::Create)?;
    invoke_signed(
        &assign(accounts[index].key, program_id),
        &[accounts[index].clone(), accounts[SYSTEM_PROGRAM].clone()],
        &[&signer],
    )
    .map_err(|_| SeriesSbfError::Create.into())
}

fn validate_vacant_state_account(account: &AccountInfo<'_>) -> ProgramResult {
    if account.owner != &system_program::ID
        || account.data_len() != 0
        || account.executable
        || account.is_signer
    {
        return Err(SeriesSbfError::AccountIdentity.into());
    }
    Ok(())
}

const fn required_top_up(observed_lamports: u64, required_lamports: u64) -> u64 {
    required_lamports.saturating_sub(observed_lamports)
}

#[cfg(test)]
mod tests;
