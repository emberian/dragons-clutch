//! Vertical SVM adapter for finite, presently capitalized Series.
//!
//! This module deliberately routes only Create, InstantiateNext, and
//! CloseExhausted. ConsumeTicket is not an executable SBF action until its
//! ticket/root/RentCredit changes can compose atomically with Found.
//!
//! Physical V1 roles are exact and ordered:
//! - Create: payer, recipe, aggregate, CapacityProfile, root, escrow, guard,
//!   RentCredit, then recipe/aggregate/capacity finalization cursors, System,
//!   and Rent.
//! - Instantiate: actor, root, recipe, aggregate, CapacityProfile, derived
//!   occurrence, occurrence capitalization, escrow, ticket, then the five
//!   finalization cursors in matching record order, System, and Rent.
//! - Close: actor, root, escrow, guard, RentCredit, and Rent.

use dclutch_product_contract::capacity::CapacityProfileV1;
use dclutch_rent_contract::RentCreditV1;
use dclutch_series_contract::{
    AccountMetaV1, CapitalizationAggregateV1, CloseExhaustedFrameV1, CloseExhaustedV1,
    CreateSeriesFrameV1, CreateSeriesV1, DerivedOccurrenceV1, IdentityV1, InstantiateNextFrameV1,
    InstantiateNextV1, OCCURRENCE_TICKET_BYTES_V1, OccurrenceCapitalizationV1,
    SERIES_ESCROW_BYTES_V1, SERIES_ESCROW_PDA_DOMAIN_V1, SERIES_INSTRUCTION_MAGIC_V1,
    SERIES_OCCURRENCE_ARTIFACT_BYTES_V1, SERIES_REPLAY_GUARD_BYTES_V1,
    SERIES_REPLAY_GUARD_PDA_DOMAIN_V1, SERIES_ROOT_BYTES_V1, SERIES_ROOT_PDA_DOMAIN_V1,
    SERIES_TICKET_PDA_DOMAIN_V1, SeriesEscrowV1, SeriesRecipeV1, SeriesReplayGuardV1, SeriesRootV1,
    VacantAccountFactsV1, plan_close_exhausted_v1, plan_create_series_v1, plan_instantiate_next_v1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    AdapterError,
    records::{
        CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1, SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
        SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1, SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V1, authenticate_rent_credit, refund_authority,
        require_unchanged_rent_credit, with_authenticated_finalized_record_v1,
    },
};

const CREATE_ACCOUNTS_V1: usize = 13;
const INSTANTIATE_ACCOUNTS_V1: usize = 16;
const CLOSE_ACCOUNTS_V1: usize = 6;

const CREATE_ACTION_V1: u8 = 1;
const INSTANTIATE_ACTION_V1: u8 = 2;
const CONSUME_ACTION_V1: u8 = 3;
const CLOSE_ACTION_V1: u8 = 4;
const ACTION_OFFSET_V1: usize = 10;

const _: () = assert!(SERIES_ROOT_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SERIES_ESCROW_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SERIES_REPLAY_GUARD_PDA_DOMAIN_V1.len() <= 32);
const _: () = assert!(SERIES_TICKET_PDA_DOMAIN_V1.len() <= 32);

/// Return true only for the three Series actions with complete SBF execution.
///
/// The top-level router must call this before selecting this module. In
/// particular, action 3 must remain wholly unrouted rather than entering an
/// adapter that can only refuse.
pub(crate) fn is_routable_instruction(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(SERIES_INSTRUCTION_MAGIC_V1.as_slice())
        && instruction_data
            .get(ACTION_OFFSET_V1)
            .is_some_and(|action| {
                matches!(
                    *action,
                    CREATE_ACTION_V1 | INSTANTIATE_ACTION_V1 | CLOSE_ACTION_V1
                )
            })
}

/// Decode and execute one completely implemented Series action.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match instruction_data.get(ACTION_OFFSET_V1).copied() {
        Some(CREATE_ACTION_V1) => CreateSeriesV1::decode(instruction_data)
            .map_err(map_wire_error)
            .and_then(|instruction| process_create(program_id, accounts, instruction)),
        Some(INSTANTIATE_ACTION_V1) => InstantiateNextV1::decode(instruction_data)
            .map_err(map_wire_error)
            .and_then(|instruction| process_instantiate(program_id, accounts, instruction)),
        Some(CLOSE_ACTION_V1) => CloseExhaustedV1::decode(instruction_data)
            .map_err(map_wire_error)
            .and_then(|instruction| process_close(program_id, accounts, instruction)),
        Some(CONSUME_ACTION_V1) => Err(AdapterError::InvalidInstruction.into()),
        _ => Err(AdapterError::InvalidInstruction.into()),
    }
}

struct CreateFrame<'a, 'info> {
    payer: &'a AccountInfo<'info>,
    recipe: &'a AccountInfo<'info>,
    aggregate: &'a AccountInfo<'info>,
    capacity_profile: &'a AccountInfo<'info>,
    root: &'a AccountInfo<'info>,
    escrow: &'a AccountInfo<'info>,
    guard: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    recipe_cursor: &'a AccountInfo<'info>,
    aggregate_cursor: &'a AccountInfo<'info>,
    capacity_profile_cursor: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> CreateFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CREATE_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            payer: account(accounts, 0)?,
            recipe: account(accounts, 1)?,
            aggregate: account(accounts, 2)?,
            capacity_profile: account(accounts, 3)?,
            root: account(accounts, 4)?,
            escrow: account(accounts, 5)?,
            guard: account(accounts, 6)?,
            rent_credit: account(accounts, 7)?,
            recipe_cursor: account(accounts, 8)?,
            aggregate_cursor: account(accounts, 9)?,
            capacity_profile_cursor: account(accounts, 10)?,
            system_program: account(accounts, 11)?,
            rent_sysvar: account(accounts, 12)?,
        };
        CreateSeriesFrameV1::validate(&[
            meta(frame.payer),
            meta(frame.recipe),
            meta(frame.aggregate),
            meta(frame.root),
            meta(frame.escrow),
            meta(frame.guard),
            meta(frame.rent_credit),
            meta(frame.system_program),
            meta(frame.rent_sysvar),
        ])
        .map_err(map_frame_error)?;
        require_readonly(frame.capacity_profile)?;
        require_readonly(frame.recipe_cursor)?;
        require_readonly(frame.aggregate_cursor)?;
        require_readonly(frame.capacity_profile_cursor)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct InstantiateFrame<'a, 'info> {
    actor: &'a AccountInfo<'info>,
    root: &'a AccountInfo<'info>,
    recipe: &'a AccountInfo<'info>,
    aggregate: &'a AccountInfo<'info>,
    capacity_profile: &'a AccountInfo<'info>,
    derived: &'a AccountInfo<'info>,
    capitalization: &'a AccountInfo<'info>,
    escrow: &'a AccountInfo<'info>,
    ticket: &'a AccountInfo<'info>,
    recipe_cursor: &'a AccountInfo<'info>,
    aggregate_cursor: &'a AccountInfo<'info>,
    capacity_profile_cursor: &'a AccountInfo<'info>,
    derived_cursor: &'a AccountInfo<'info>,
    capitalization_cursor: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> InstantiateFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != INSTANTIATE_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            actor: account(accounts, 0)?,
            root: account(accounts, 1)?,
            recipe: account(accounts, 2)?,
            aggregate: account(accounts, 3)?,
            capacity_profile: account(accounts, 4)?,
            derived: account(accounts, 5)?,
            capitalization: account(accounts, 6)?,
            escrow: account(accounts, 7)?,
            ticket: account(accounts, 8)?,
            recipe_cursor: account(accounts, 9)?,
            aggregate_cursor: account(accounts, 10)?,
            capacity_profile_cursor: account(accounts, 11)?,
            derived_cursor: account(accounts, 12)?,
            capitalization_cursor: account(accounts, 13)?,
            system_program: account(accounts, 14)?,
            rent_sysvar: account(accounts, 15)?,
        };
        InstantiateNextFrameV1::validate(&[
            meta(frame.actor),
            meta(frame.root),
            meta(frame.recipe),
            meta(frame.derived),
            meta(frame.capitalization),
            meta(frame.escrow),
            meta(frame.ticket),
            meta(frame.system_program),
            meta(frame.rent_sysvar),
        ])
        .map_err(map_frame_error)?;
        require_readonly(frame.aggregate)?;
        require_readonly(frame.capacity_profile)?;
        require_readonly(frame.recipe_cursor)?;
        require_readonly(frame.aggregate_cursor)?;
        require_readonly(frame.capacity_profile_cursor)?;
        require_readonly(frame.derived_cursor)?;
        require_readonly(frame.capitalization_cursor)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct CloseFrame<'a, 'info> {
    actor: &'a AccountInfo<'info>,
    root: &'a AccountInfo<'info>,
    escrow: &'a AccountInfo<'info>,
    guard: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> CloseFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CLOSE_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            actor: account(accounts, 0)?,
            root: account(accounts, 1)?,
            escrow: account(accounts, 2)?,
            guard: account(accounts, 3)?,
            rent_credit: account(accounts, 4)?,
            rent_sysvar: account(accounts, 5)?,
        };
        CloseExhaustedFrameV1::validate(&[
            meta(frame.actor),
            meta(frame.root),
            meta(frame.escrow),
            meta(frame.guard),
            meta(frame.rent_credit),
            meta(frame.rent_sysvar),
        ])
        .map_err(map_frame_error)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
struct CreatePlan {
    semantic: dclutch_series_contract::CreateSeriesPlanV1,
    recipe_id: IdentityV1,
    aggregate_id: IdentityV1,
    rent_credit: RentCreditV1,
    rent_credit_lamports: u64,
}

#[inline(never)]
fn process_create(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CreateSeriesV1,
) -> Result<(), ProgramError> {
    let frame = CreateFrame::parse(accounts)?;
    let plan = authenticate_create(program_id, &frame, instruction)?;
    let root_bump = [plan.semantic.root.pda_bump];
    let escrow_bump = [plan.semantic.escrow.pda_bump];
    let guard_bump = [plan.semantic.replay_guard.pda_bump];
    let refund = instruction.refund_authority.to_bytes();
    let recipe_id = plan.recipe_id.to_bytes();
    let aggregate_id = plan.aggregate_id.to_bytes();
    let root_signer = [
        SERIES_ROOT_PDA_DOMAIN_V1,
        recipe_id.as_slice(),
        aggregate_id.as_slice(),
        refund.as_slice(),
        root_bump.as_slice(),
    ];
    let root_key = frame.root.key.to_bytes();
    let escrow_signer = [
        SERIES_ESCROW_PDA_DOMAIN_V1,
        root_key.as_slice(),
        escrow_bump.as_slice(),
    ];
    let guard_signer = [
        SERIES_REPLAY_GUARD_PDA_DOMAIN_V1,
        root_key.as_slice(),
        guard_bump.as_slice(),
    ];

    let root_top_up = plan
        .semantic
        .root_after
        .checked_sub(plan.semantic.root_before)
        .ok_or(AdapterError::Arithmetic)?;
    let escrow_top_up = plan
        .semantic
        .escrow_after
        .checked_sub(plan.semantic.escrow_before)
        .ok_or(AdapterError::Arithmetic)?;
    let guard_top_up = plan
        .semantic
        .replay_guard_after
        .checked_sub(plan.semantic.replay_guard_before)
        .ok_or(AdapterError::Arithmetic)?;
    fund_allocate_assign(
        program_id,
        frame.payer,
        frame.root,
        frame.system_program,
        root_top_up,
        SERIES_ROOT_BYTES_V1,
        &root_signer,
    )?;
    fund_allocate_assign(
        program_id,
        frame.payer,
        frame.escrow,
        frame.system_program,
        escrow_top_up,
        SERIES_ESCROW_BYTES_V1,
        &escrow_signer,
    )?;
    fund_allocate_assign(
        program_id,
        frame.payer,
        frame.guard,
        frame.system_program,
        guard_top_up,
        SERIES_REPLAY_GUARD_BYTES_V1,
        &guard_signer,
    )?;

    persist_exact(frame.root, &plan.semantic.root.to_bytes())?;
    persist_exact(frame.escrow, &plan.semantic.escrow.to_bytes())?;
    persist_exact(frame.guard, &plan.semantic.replay_guard.to_bytes())?;
    require_create_post(program_id, &frame, plan)
}

#[inline(never)]
fn authenticate_create(
    program_id: &Pubkey,
    frame: &CreateFrame<'_, '_>,
    instruction: CreateSeriesV1,
) -> Result<CreatePlan, ProgramError> {
    require_system_payer(frame.payer)?;
    let rent = authenticated_rent(frame.rent_sysvar)?;
    let recipe_digest = record_digest(frame.recipe)?;
    let aggregate_digest = record_digest(frame.aggregate)?;
    let recipe_id = identity(recipe_digest)?;
    let aggregate_id = identity(aggregate_digest)?;
    let recipe = authenticate_recipe(
        program_id,
        frame.recipe,
        frame.recipe_cursor,
        frame.rent_sysvar,
        recipe_digest,
    )?;
    let aggregate = authenticate_aggregate(
        program_id,
        frame.aggregate,
        frame.aggregate_cursor,
        frame.rent_sysvar,
        aggregate_digest,
    )?;
    authenticate_capacity_profile(
        program_id,
        frame.capacity_profile,
        frame.capacity_profile_cursor,
        frame.rent_sysvar,
        &recipe,
    )?;

    let refund = instruction.refund_authority.to_bytes();
    let (expected_root, root_bump) = Pubkey::find_program_address(
        &[
            SERIES_ROOT_PDA_DOMAIN_V1,
            recipe_digest.as_slice(),
            aggregate_digest.as_slice(),
            refund.as_slice(),
        ],
        program_id,
    );
    if frame.root.key != &expected_root || instruction.root_bump != root_bump {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    let root_key = expected_root.to_bytes();
    let (expected_escrow, escrow_bump) =
        Pubkey::find_program_address(&[SERIES_ESCROW_PDA_DOMAIN_V1, &root_key], program_id);
    let (expected_guard, guard_bump) =
        Pubkey::find_program_address(&[SERIES_REPLAY_GUARD_PDA_DOMAIN_V1, &root_key], program_id);
    if frame.escrow.key != &expected_escrow
        || frame.guard.key != &expected_guard
        || instruction.escrow_bump != escrow_bump
        || instruction.replay_guard_bump != guard_bump
    {
        return Err(AdapterError::SeriesAuthentication.into());
    }

    let minimum_credit = rent.minimum_balance(dclutch_rent_contract::RENT_CREDIT_BYTES_V1);
    let rent_credit = authenticate_rent_credit(
        program_id,
        frame.rent_credit,
        refund_authority(&Pubkey::new_from_array(refund))?,
        Some(minimum_credit),
    )?;
    let semantic = plan_create_series_v1(
        identity(root_key)?,
        recipe_id,
        aggregate_id,
        &recipe,
        &aggregate,
        instruction,
        frame.payer.lamports(),
        vacancy(frame.root)?,
        vacancy(frame.escrow)?,
        vacancy(frame.guard)?,
        rent.minimum_balance(SERIES_ROOT_BYTES_V1),
        rent.minimum_balance(SERIES_ESCROW_BYTES_V1),
        rent.minimum_balance(SERIES_REPLAY_GUARD_BYTES_V1),
    )
    .map_err(map_transition_error)?;
    preflight_mutable(&[frame.payer, frame.root, frame.escrow, frame.guard])?;
    Ok(CreatePlan {
        semantic,
        recipe_id,
        aggregate_id,
        rent_credit,
        rent_credit_lamports: frame.rent_credit.lamports(),
    })
}

#[derive(Clone, Copy)]
struct InstantiatePlan {
    semantic: dclutch_series_contract::InstantiationPlanV1,
    root_lamports_before: u64,
    root_data_before: [u8; SERIES_ROOT_BYTES_V1],
    escrow_data_before: [u8; SERIES_ESCROW_BYTES_V1],
    ticket_bump: u8,
}

#[inline(never)]
fn process_instantiate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: InstantiateNextV1,
) -> Result<(), ProgramError> {
    let frame = InstantiateFrame::parse(accounts)?;
    let plan = authenticate_instantiate(program_id, &frame, instruction)?;
    let root_key = frame.root.key.to_bytes();
    let index = instruction.expected_index.to_le_bytes();
    let bump = [plan.ticket_bump];
    let ticket_signer = [
        SERIES_TICKET_PDA_DOMAIN_V1,
        root_key.as_slice(),
        index.as_slice(),
        bump.as_slice(),
    ];
    allocate_assign(
        program_id,
        frame.ticket,
        frame.system_program,
        OCCURRENCE_TICKET_BYTES_V1,
        &ticket_signer,
    )?;
    {
        let mut escrow_lamports = frame
            .escrow
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::SeriesTransition)?;
        let mut ticket_lamports = frame
            .ticket
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::SeriesTransition)?;
        **escrow_lamports = plan.semantic.escrow_lamports_after;
        **ticket_lamports = plan.semantic.ticket_lamports_after;
    }
    persist_exact(frame.root, &plan.semantic.root_after.to_bytes())?;
    persist_exact(frame.ticket, &plan.semantic.ticket.to_bytes())?;
    require_instantiate_post(program_id, &frame, plan)
}

#[inline(never)]
fn authenticate_instantiate(
    program_id: &Pubkey,
    frame: &InstantiateFrame<'_, '_>,
    instruction: InstantiateNextV1,
) -> Result<InstantiatePlan, ProgramError> {
    let rent = authenticated_rent(frame.rent_sysvar)?;
    let root = decode_program_record::<SERIES_ROOT_BYTES_V1, SeriesRootV1>(
        program_id,
        frame.root,
        SeriesRootV1::decode,
    )?;
    let escrow = decode_program_record::<SERIES_ESCROW_BYTES_V1, SeriesEscrowV1>(
        program_id,
        frame.escrow,
        SeriesEscrowV1::decode,
    )?;
    let root_address = identity(frame.root.key.to_bytes())?;
    authenticate_root_pda(program_id, frame.root, root)?;
    authenticate_escrow_pda(program_id, frame.escrow, frame.root.key, escrow)?;
    if frame.root.lamports() < rent.minimum_balance(SERIES_ROOT_BYTES_V1) {
        return Err(AdapterError::SeriesAuthentication.into());
    }

    let recipe_digest = root.recipe_id.to_bytes();
    let aggregate_digest = root.aggregate_id.to_bytes();
    let recipe = authenticate_recipe(
        program_id,
        frame.recipe,
        frame.recipe_cursor,
        frame.rent_sysvar,
        recipe_digest,
    )?;
    let aggregate = authenticate_aggregate(
        program_id,
        frame.aggregate,
        frame.aggregate_cursor,
        frame.rent_sysvar,
        aggregate_digest,
    )?;
    authenticate_capacity_profile(
        program_id,
        frame.capacity_profile,
        frame.capacity_profile_cursor,
        frame.rent_sysvar,
        &recipe,
    )?;
    let derived_digest = record_digest(frame.derived)?;
    let capitalization_digest = record_digest(frame.capitalization)?;
    let derived = authenticate_derived(
        program_id,
        frame.derived,
        frame.derived_cursor,
        frame.rent_sysvar,
        derived_digest,
    )?;
    let capitalization = authenticate_capitalization(
        program_id,
        frame.capitalization,
        frame.capitalization_cursor,
        frame.rent_sysvar,
        capitalization_digest,
    )?;

    let index = instruction.expected_index.to_le_bytes();
    let root_key = frame.root.key.to_bytes();
    let (expected_ticket, ticket_bump) = Pubkey::find_program_address(
        &[SERIES_TICKET_PDA_DOMAIN_V1, &root_key, &index],
        program_id,
    );
    if frame.ticket.key != &expected_ticket || instruction.ticket_bump != ticket_bump {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    let semantic = plan_instantiate_next_v1(
        root,
        root_address,
        escrow,
        root.recipe_id,
        &recipe,
        root.aggregate_id,
        &aggregate,
        identity(derived_digest)?,
        &derived,
        identity(capitalization_digest)?,
        &capitalization,
        instruction,
        Clock::get()
            .map_err(|_| AdapterError::SeriesAuthentication)?
            .unix_timestamp,
        rent.minimum_balance(SERIES_ESCROW_BYTES_V1),
        rent.minimum_balance(OCCURRENCE_TICKET_BYTES_V1),
        frame.escrow.lamports(),
        vacancy(frame.ticket)?,
    )
    .map_err(map_transition_error)?;

    let root_data_before = copy_data::<SERIES_ROOT_BYTES_V1>(frame.root)?;
    let escrow_data_before = copy_data::<SERIES_ESCROW_BYTES_V1>(frame.escrow)?;
    preflight_mutable(&[frame.root, frame.escrow, frame.ticket])?;
    Ok(InstantiatePlan {
        semantic,
        root_lamports_before: frame.root.lamports(),
        root_data_before,
        escrow_data_before,
        ticket_bump,
    })
}

#[inline(never)]
fn process_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CloseExhaustedV1,
) -> Result<(), ProgramError> {
    let frame = CloseFrame::parse(accounts)?;
    let (plan, rent_credit) = authenticate_close(program_id, &frame, instruction)?;
    {
        let mut root = frame
            .root
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::SeriesClose)?;
        let mut escrow = frame
            .escrow
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::SeriesClose)?;
        let mut guard = frame
            .guard
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::SeriesClose)?;
        let mut credit = frame
            .rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::SeriesClose)?;
        **root = plan.root_lamports_after;
        **escrow = plan.escrow_lamports_after;
        **guard = plan.replay_guard_lamports_after;
        **credit = plan.rent_credit_after;
    }
    frame
        .root
        .resize(0)
        .map_err(|_| AdapterError::SeriesClose)?;
    frame.root.assign(&system_program::ID);
    frame
        .escrow
        .resize(0)
        .map_err(|_| AdapterError::SeriesClose)?;
    frame.escrow.assign(&system_program::ID);
    require_unchanged_rent_credit(program_id, frame.rent_credit, rent_credit)?;
    let retained_guard = decode_program_record::<SERIES_REPLAY_GUARD_BYTES_V1, SeriesReplayGuardV1>(
        program_id,
        frame.guard,
        SeriesReplayGuardV1::decode,
    )?;
    authenticate_guard_pda(program_id, frame.guard, frame.root.key, retained_guard)?;
    if frame.root.lamports() != 0
        || frame.escrow.lamports() != 0
        || frame.guard.lamports() != plan.replay_guard_lamports_after
        || frame.rent_credit.lamports() != plan.rent_credit_after
        || frame.root.owner != &system_program::ID
        || frame.escrow.owner != &system_program::ID
        || !frame.root.data_is_empty()
        || !frame.escrow.data_is_empty()
    {
        return Err(AdapterError::SeriesPostcondition.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_close(
    program_id: &Pubkey,
    frame: &CloseFrame<'_, '_>,
    instruction: CloseExhaustedV1,
) -> Result<(dclutch_series_contract::CloseExhaustedPlanV1, RentCreditV1), ProgramError> {
    let rent = authenticated_rent(frame.rent_sysvar)?;
    let root = decode_program_record::<SERIES_ROOT_BYTES_V1, SeriesRootV1>(
        program_id,
        frame.root,
        SeriesRootV1::decode,
    )?;
    let escrow = decode_program_record::<SERIES_ESCROW_BYTES_V1, SeriesEscrowV1>(
        program_id,
        frame.escrow,
        SeriesEscrowV1::decode,
    )?;
    let guard = decode_program_record::<SERIES_REPLAY_GUARD_BYTES_V1, SeriesReplayGuardV1>(
        program_id,
        frame.guard,
        SeriesReplayGuardV1::decode,
    )?;
    authenticate_root_pda(program_id, frame.root, root)?;
    authenticate_escrow_pda(program_id, frame.escrow, frame.root.key, escrow)?;
    authenticate_guard_pda(program_id, frame.guard, frame.root.key, guard)?;
    let authority = Pubkey::new_from_array(root.refund_authority.to_bytes());
    let minimum_credit = rent.minimum_balance(dclutch_rent_contract::RENT_CREDIT_BYTES_V1);
    let rent_credit = authenticate_rent_credit(
        program_id,
        frame.rent_credit,
        refund_authority(&authority)?,
        Some(minimum_credit),
    )?;
    let plan = plan_close_exhausted_v1(
        root,
        identity(frame.root.key.to_bytes())?,
        escrow,
        guard,
        instruction,
        frame.root.lamports(),
        frame.escrow.lamports(),
        frame.guard.lamports(),
        rent.minimum_balance(SERIES_REPLAY_GUARD_BYTES_V1),
        frame.rent_credit.lamports(),
    )
    .map_err(map_transition_error)?;
    preflight_mutable(&[frame.root, frame.escrow, frame.guard, frame.rent_credit])?;
    Ok((plan, rent_credit))
}

fn authenticate_recipe<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    digest: [u8; 32],
) -> Result<SeriesRecipeV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        account,
        cursor,
        rent,
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V1,
        digest,
        |record| {
            SeriesRecipeV1::decode(record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )
}

fn authenticate_aggregate<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    digest: [u8; 32],
) -> Result<CapitalizationAggregateV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        account,
        cursor,
        rent,
        SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
        digest,
        |record| {
            CapitalizationAggregateV1::decode(record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )
}

fn authenticate_capacity_profile<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    recipe: &SeriesRecipeV1,
) -> Result<CapacityProfileV1, ProgramError> {
    let profile = with_authenticated_finalized_record_v1(
        program_id,
        account,
        cursor,
        rent,
        CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
        recipe.capacity_profile_id.to_bytes(),
        |record| {
            CapacityProfileV1::decode(record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )?;
    let artifact_bytes =
        u32::try_from(SERIES_OCCURRENCE_ARTIFACT_BYTES_V1).map_err(|_| AdapterError::Arithmetic)?;
    profile
        .validate_artifact(artifact_bytes, 1)
        .map_err(|_| AdapterError::SeriesAuthentication)?;
    profile
        .validate_partition(u32::from(recipe.outcome_count))
        .map_err(|_| AdapterError::SeriesAuthentication)?;
    Ok(profile)
}

fn authenticate_derived<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    digest: [u8; 32],
) -> Result<DerivedOccurrenceV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        account,
        cursor,
        rent,
        SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
        digest,
        |record| {
            DerivedOccurrenceV1::decode(record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )
}

fn authenticate_capitalization<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    digest: [u8; 32],
) -> Result<OccurrenceCapitalizationV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        account,
        cursor,
        rent,
        SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1,
        digest,
        |record| {
            OccurrenceCapitalizationV1::decode(record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )
}

fn fund_allocate_assign<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    top_up: u64,
    space: usize,
    signer: &[&[u8]],
) -> Result<(), ProgramError> {
    if top_up != 0 {
        invoke_signed(
            &transfer(payer.key, destination.key, top_up),
            &[payer.clone(), destination.clone(), system.clone()],
            &[],
        )
        .map_err(|_| AdapterError::SeriesCreateCpi)?;
    }
    allocate_assign(program_id, destination, system, space, signer)
}

fn allocate_assign<'info>(
    program_id: &Pubkey,
    destination: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    space: usize,
    signer: &[&[u8]],
) -> Result<(), ProgramError> {
    let space_u64 = u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?;
    invoke_signed(
        &allocate(destination.key, space_u64),
        &[destination.clone(), system.clone()],
        &[signer],
    )
    .map_err(|_| AdapterError::SeriesCreateCpi)?;
    invoke_signed(
        &assign(destination.key, program_id),
        &[destination.clone(), system.clone()],
        &[signer],
    )
    .map_err(|_| AdapterError::SeriesCreateCpi)?;
    if destination.owner != program_id || destination.data_len() != space {
        return Err(AdapterError::SeriesPostcondition.into());
    }
    Ok(())
}

fn require_create_post(
    program_id: &Pubkey,
    frame: &CreateFrame<'_, '_>,
    plan: CreatePlan,
) -> Result<(), ProgramError> {
    if frame.payer.lamports() != plan.semantic.payer_after
        || frame.root.lamports() != plan.semantic.root_after
        || frame.escrow.lamports() != plan.semantic.escrow_after
        || frame.guard.lamports() != plan.semantic.replay_guard_after
        || frame.rent_credit.lamports() != plan.rent_credit_lamports
        || decode_program_record::<SERIES_ROOT_BYTES_V1, SeriesRootV1>(
            program_id,
            frame.root,
            SeriesRootV1::decode,
        )? != plan.semantic.root
        || decode_program_record::<SERIES_ESCROW_BYTES_V1, SeriesEscrowV1>(
            program_id,
            frame.escrow,
            SeriesEscrowV1::decode,
        )? != plan.semantic.escrow
        || decode_program_record::<SERIES_REPLAY_GUARD_BYTES_V1, SeriesReplayGuardV1>(
            program_id,
            frame.guard,
            SeriesReplayGuardV1::decode,
        )? != plan.semantic.replay_guard
    {
        return Err(AdapterError::SeriesPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, frame.rent_credit, plan.rent_credit)
}

fn require_instantiate_post(
    program_id: &Pubkey,
    frame: &InstantiateFrame<'_, '_>,
    plan: InstantiatePlan,
) -> Result<(), ProgramError> {
    if frame.root.lamports() != plan.root_lamports_before
        || frame.escrow.lamports() != plan.semantic.escrow_lamports_after
        || frame.ticket.lamports() != plan.semantic.ticket_lamports_after
        || frame.ticket.owner != program_id
        || decode_program_record::<SERIES_ROOT_BYTES_V1, SeriesRootV1>(
            program_id,
            frame.root,
            SeriesRootV1::decode,
        )? != plan.semantic.root_after
        || decode_program_record::<
            OCCURRENCE_TICKET_BYTES_V1,
            dclutch_series_contract::OccurrenceTicketV1,
        >(
            program_id,
            frame.ticket,
            dclutch_series_contract::OccurrenceTicketV1::decode,
        )? != plan.semantic.ticket
        || copy_data::<SERIES_ESCROW_BYTES_V1>(frame.escrow)? != plan.escrow_data_before
        || plan.root_data_before == plan.semantic.root_after.to_bytes()
    {
        return Err(AdapterError::SeriesPostcondition.into());
    }
    Ok(())
}

fn authenticate_root_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: SeriesRootV1,
) -> Result<(), ProgramError> {
    let bump = [root.pda_bump];
    let recipe = root.recipe_id.to_bytes();
    let aggregate = root.aggregate_id.to_bytes();
    let refund = root.refund_authority.to_bytes();
    let expected = Pubkey::create_program_address(
        &[
            SERIES_ROOT_PDA_DOMAIN_V1,
            &recipe,
            &aggregate,
            &refund,
            &bump,
        ],
        program_id,
    )
    .map_err(|_| AdapterError::SeriesAuthentication)?;
    if account.key != &expected {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    Ok(())
}

fn authenticate_escrow_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &Pubkey,
    escrow: SeriesEscrowV1,
) -> Result<(), ProgramError> {
    let bump = [escrow.pda_bump];
    let root = root.to_bytes();
    let expected =
        Pubkey::create_program_address(&[SERIES_ESCROW_PDA_DOMAIN_V1, &root, &bump], program_id)
            .map_err(|_| AdapterError::SeriesAuthentication)?;
    if account.key != &expected {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    Ok(())
}

fn authenticate_guard_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &Pubkey,
    guard: SeriesReplayGuardV1,
) -> Result<(), ProgramError> {
    let bump = [guard.pda_bump];
    let root = root.to_bytes();
    let expected = Pubkey::create_program_address(
        &[SERIES_REPLAY_GUARD_PDA_DOMAIN_V1, &root, &bump],
        program_id,
    )
    .map_err(|_| AdapterError::SeriesAuthentication)?;
    if account.key != &expected {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    Ok(())
}

fn authenticated_rent(account: &AccountInfo<'_>) -> Result<Rent, ProgramError> {
    if account.key != &sysvar::rent::ID
        || account.is_signer
        || account.is_writable
        || account.executable
    {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    Rent::from_account_info(account).map_err(|_| AdapterError::SeriesAuthentication.into())
}

fn require_system_payer(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || !account.data_is_empty() || account.executable {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    Ok(())
}

fn vacancy(account: &AccountInfo<'_>) -> Result<VacantAccountFactsV1, ProgramError> {
    Ok(VacantAccountFactsV1 {
        lamports: account.lamports(),
        owner: account.owner.to_bytes(),
        data_len: u64::try_from(account.data_len()).map_err(|_| AdapterError::Arithmetic)?,
        is_executable: account.executable,
    })
}

fn record_digest(account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::SeriesAuthentication)?;
    Ok(hash(&data).to_bytes())
}

fn decode_program_record<const N: usize, T: Copy>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    decode: fn(&[u8]) -> dclutch_series_contract::Result<T>,
) -> Result<T, ProgramError> {
    if account.owner != program_id || account.executable || account.data_len() != N {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::SeriesAuthentication)?;
    let value = decode(&data).map_err(map_transition_error)?;
    Ok(value)
}

fn copy_data<const N: usize>(account: &AccountInfo<'_>) -> Result<[u8; N], ProgramError> {
    account
        .try_borrow_data()
        .map_err(|_| AdapterError::SeriesAuthentication)?
        .as_ref()
        .try_into()
        .map_err(|_| AdapterError::SeriesAuthentication.into())
}

fn persist_exact(account: &AccountInfo<'_>, bytes: &[u8]) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::SeriesPostcondition)?;
    if data.len() != bytes.len() {
        return Err(AdapterError::SeriesPostcondition.into());
    }
    data.copy_from_slice(bytes);
    Ok(())
}

fn preflight_mutable(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for account in accounts {
        drop(
            account
                .try_borrow_mut_lamports()
                .map_err(|_| AdapterError::SeriesAuthentication)?,
        );
        drop(
            account
                .try_borrow_mut_data()
                .map_err(|_| AdapterError::SeriesAuthentication)?,
        );
    }
    Ok(())
}

fn require_readonly(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.is_signer || account.is_writable || account.executable {
        return Err(AdapterError::AccountPrivilege.into());
    }
    Ok(())
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(index.saturating_add(1)) {
            if left.key == right.key {
                return Err(AdapterError::AccountIdentity.into());
            }
        }
    }
    Ok(())
}

fn identity(bytes: [u8; 32]) -> Result<IdentityV1, ProgramError> {
    IdentityV1::new(bytes).map_err(map_transition_error)
}

fn meta(account: &AccountInfo<'_>) -> AccountMetaV1 {
    AccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| AdapterError::AccountFrameLength.into())
}

fn map_wire_error(_: dclutch_series_contract::Error) -> ProgramError {
    AdapterError::InvalidInstruction.into()
}

fn map_frame_error(_: dclutch_series_contract::Error) -> ProgramError {
    AdapterError::AccountPrivilege.into()
}

fn map_transition_error(_: dclutch_series_contract::Error) -> ProgramError {
    AdapterError::SeriesTransition.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_never_admits_consume_or_partial_wires() {
        let mut wire = [0u8; dclutch_series_contract::INSTANTIATE_NEXT_BYTES_V1];
        wire.get_mut(..8)
            .expect("fixed magic span")
            .copy_from_slice(&SERIES_INSTRUCTION_MAGIC_V1);
        wire.get_mut(8..10)
            .expect("fixed schema span")
            .copy_from_slice(&1u16.to_le_bytes());
        *wire.get_mut(ACTION_OFFSET_V1).expect("fixed action") = CREATE_ACTION_V1;
        assert!(is_routable_instruction(&wire));
        *wire.get_mut(ACTION_OFFSET_V1).expect("fixed action") = INSTANTIATE_ACTION_V1;
        assert!(is_routable_instruction(&wire));
        *wire.get_mut(ACTION_OFFSET_V1).expect("fixed action") = CLOSE_ACTION_V1;
        assert!(is_routable_instruction(&wire));
        *wire.get_mut(ACTION_OFFSET_V1).expect("fixed action") = CONSUME_ACTION_V1;
        assert!(!is_routable_instruction(&wire));
        assert!(!is_routable_instruction(
            wire.get(..7).expect("fixed short span")
        ));
        *wire.get_mut(0).expect("fixed magic byte") ^= 1;
        assert!(!is_routable_instruction(&wire));
    }

    #[test]
    fn series_record_release_ids_are_pinned_to_labels() {
        assert_eq!(
            hash(b"dclutch/schema/series-recipe-v1").to_bytes(),
            SERIES_RECIPE_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            hash(b"dclutch/schema/series-capitalization-aggregate-v1").to_bytes(),
            SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            hash(b"dclutch/schema/series-derived-occurrence-v1").to_bytes(),
            SERIES_DERIVED_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            hash(b"dclutch/schema/series-occurrence-capitalization-v1").to_bytes(),
            SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1
        );
    }

    #[test]
    fn executable_dispatch_rejects_consume_before_account_access() {
        let instruction = dclutch_series_contract::ConsumeTicketV1 { expected_index: 0 }.to_bytes();
        assert_eq!(
            dispatch(&Pubkey::new_unique(), &[], &instruction),
            Err(AdapterError::InvalidInstruction.into())
        );
    }

    #[test]
    fn executable_wires_decode_before_exact_frame_access() {
        let program_id = Pubkey::new_unique();
        let refund_authority = IdentityV1::new([9; 32]).expect("nonzero identity");
        let create = CreateSeriesV1 {
            refund_authority,
            root_bump: 1,
            escrow_bump: 2,
            replay_guard_bump: 3,
        }
        .to_bytes();
        assert_eq!(
            dispatch(&program_id, &[], &create),
            Err(AdapterError::AccountFrameLength.into())
        );

        let instantiate = InstantiateNextV1 {
            expected_index: 4,
            expected_time: 5,
            ticket_bump: 6,
        }
        .to_bytes();
        assert_eq!(
            dispatch(&program_id, &[], &instantiate),
            Err(AdapterError::AccountFrameLength.into())
        );
        let close = CloseExhaustedV1 {
            expected_released_allocations: 7,
        }
        .to_bytes();
        assert_eq!(
            dispatch(&program_id, &[], &close),
            Err(AdapterError::AccountFrameLength.into())
        );

        let mut hostile = instantiate;
        *hostile.get_mut(39).expect("fixed reserved byte") = 1;
        assert_eq!(
            dispatch(&program_id, &[], &hostile),
            Err(AdapterError::InvalidInstruction.into())
        );
        assert_eq!(
            dispatch(
                &program_id,
                &[],
                hostile.get(..39).expect("fixed short wire")
            ),
            Err(AdapterError::InvalidInstruction.into())
        );
    }
}
