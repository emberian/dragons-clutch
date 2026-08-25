//! Vertical SVM adapter for finite, presently capitalized Series.
//!
//! All four Series actions are executable. ConsumeTicket authenticates its
//! complete Series and Found inputs before mutation, derives the Found wire
//! solely from immutable Series obligations, and commits Found plus ticket
//! retirement in one SVM rollback domain.
//!
//! Physical V1 roles are exact and ordered:
//! - Create: payer, recipe, aggregate, CapacityProfile, root, escrow, guard,
//!   RentCredit, recipe/aggregate/capacity cursors, capability template and
//!   cursor, System, and Rent.
//! - Instantiate: actor, root, recipe, aggregate, CapacityProfile, derived
//!   occurrence, occurrence capitalization, escrow, ticket, then the five
//!   existing finalization cursors, occurrence Source material and its cursor,
//!   capability template/cursor, realized manifest/cursor, System, and Rent.
//! - Consume: the exact Found18 frame first, with its sponsor role interpreted
//!   only as the temporary permissionless payer, followed by root, recipe,
//!   aggregate, derived occurrence, occurrence capitalization, ticket, and the
//!   four Series-only finalization cursors, then capability template/cursor.
//!   CapacityProfile, Source material, realized manifest, their cursors,
//!   RentCredit, System, and Rent are shared with Found.
//! - Close: actor, root, escrow, guard, RentCredit, and Rent.

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
};
use dclutch_collateral_contract::FoundMarketAndFundV1;
use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity};
use dclutch_product_contract::capacity::CapacityProfileV1;
use dclutch_rent_contract::RentCreditV1;
use dclutch_series_contract::{
    AccountMetaV1, CapitalizationAggregateV1, CloseExhaustedFrameV1, CloseExhaustedV1,
    ConsumeTicketV1, CreateSeriesFrameV1, CreateSeriesV1, DerivedOccurrenceV1, IdentityV1,
    InstantiateNextFrameV1, InstantiateNextV1, OCCURRENCE_TICKET_BYTES_V1,
    OccurrenceCapabilityManifestV1, OccurrenceCapitalizationV1, OccurrenceSourceMaterialV1,
    OccurrenceTicketV1, SERIES_ESCROW_BYTES_V1, SERIES_ESCROW_PDA_DOMAIN_V1,
    SERIES_INSTRUCTION_MAGIC_V1, SERIES_OCCURRENCE_ARTIFACT_BYTES_V1, SERIES_REPLAY_GUARD_BYTES_V1,
    SERIES_REPLAY_GUARD_PDA_DOMAIN_V1, SERIES_ROOT_BYTES_V1, SERIES_ROOT_PDA_DOMAIN_V1,
    SERIES_TICKET_PDA_DOMAIN_V1, SeriesEscrowV1, SeriesRecipeV1, SeriesReplayGuardV1, SeriesRootV1,
    VacantAccountFactsV1, authenticate_occurrence_capability_manifest_v1,
    authenticate_occurrence_source_material_v1, authenticate_series_capability_template_v1,
    plan_close_exhausted_v1, plan_consume_ticket_v1, plan_create_series_v1,
    plan_instantiate_next_v1,
};
use dclutch_source_contract::SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1;
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
    found_market::{
        FoundingPlan, execute_preflighted_found_market_and_fund, preflight_found_market_and_fund,
    },
    records::{
        CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1, SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
        SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1, SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V3, authenticate_rent_credit, refund_authority,
        require_unchanged_rent_credit, with_authenticated_finalized_record_v1,
    },
};

const CREATE_ACCOUNTS_V1: usize = 15;
const INSTANTIATE_ACCOUNTS_V1: usize = 22;
const FOUND_ACCOUNTS_V1: usize = 18;
const CONSUME_ACCOUNTS_V1: usize = 30;
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

/// Return true only for exact, completely implemented Series wires.
pub(crate) fn is_routable_instruction(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(SERIES_INSTRUCTION_MAGIC_V1.as_slice())
        && instruction_data
            .get(ACTION_OFFSET_V1)
            .is_some_and(|action| {
                matches!(
                    *action,
                    CREATE_ACTION_V1 | INSTANTIATE_ACTION_V1 | CONSUME_ACTION_V1 | CLOSE_ACTION_V1
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
        Some(CONSUME_ACTION_V1) => ConsumeTicketV1::decode(instruction_data)
            .map_err(map_wire_error)
            .and_then(|instruction| process_consume(program_id, accounts, instruction)),
        Some(CLOSE_ACTION_V1) => CloseExhaustedV1::decode(instruction_data)
            .map_err(map_wire_error)
            .and_then(|instruction| process_close(program_id, accounts, instruction)),
        _ => Err(AdapterError::InvalidInstruction.into()),
    }
}

/// Exact physical action-3 frame.
///
/// Accounts 0..18 are passed unchanged to the Found semantic owner. Shared
/// Found/Series roles are physical, not repeated: actor/sponsor=0,
/// RentCredit=3, CapacityProfile=7, SourceMaterial=8,
/// CapabilityManifest=9, CapacityProfileCursor=13, SourceMaterialCursor=14,
/// CapabilityManifestCursor=15, System=16, Rent=17.
struct ConsumeFrame<'a, 'info> {
    found_accounts: &'a [AccountInfo<'info>],
    actor: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    capacity_profile: &'a AccountInfo<'info>,
    capacity_profile_cursor: &'a AccountInfo<'info>,
    resolution_material: &'a AccountInfo<'info>,
    resolution_material_cursor: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    capability_manifest_cursor: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
    root: &'a AccountInfo<'info>,
    recipe: &'a AccountInfo<'info>,
    aggregate: &'a AccountInfo<'info>,
    derived: &'a AccountInfo<'info>,
    capitalization: &'a AccountInfo<'info>,
    ticket: &'a AccountInfo<'info>,
    recipe_cursor: &'a AccountInfo<'info>,
    aggregate_cursor: &'a AccountInfo<'info>,
    derived_cursor: &'a AccountInfo<'info>,
    capitalization_cursor: &'a AccountInfo<'info>,
    capability_template: &'a AccountInfo<'info>,
    capability_template_cursor: &'a AccountInfo<'info>,
}

impl<'a, 'info> ConsumeFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CONSUME_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let found_accounts = accounts
            .get(..FOUND_ACCOUNTS_V1)
            .ok_or(AdapterError::AccountFrameLength)?;
        let frame = Self {
            found_accounts,
            actor: account(accounts, 0)?,
            rent_credit: account(accounts, 3)?,
            capacity_profile: account(accounts, 7)?,
            capacity_profile_cursor: account(accounts, 13)?,
            resolution_material: account(accounts, 8)?,
            resolution_material_cursor: account(accounts, 14)?,
            capability_manifest: account(accounts, 9)?,
            capability_manifest_cursor: account(accounts, 15)?,
            rent_sysvar: account(accounts, 17)?,
            root: account(accounts, 18)?,
            recipe: account(accounts, 19)?,
            aggregate: account(accounts, 20)?,
            derived: account(accounts, 21)?,
            capitalization: account(accounts, 22)?,
            ticket: account(accounts, 23)?,
            recipe_cursor: account(accounts, 24)?,
            aggregate_cursor: account(accounts, 25)?,
            derived_cursor: account(accounts, 26)?,
            capitalization_cursor: account(accounts, 27)?,
            capability_template: account(accounts, 28)?,
            capability_template_cursor: account(accounts, 29)?,
        };
        require_system_payer(frame.actor)?;
        if !frame.actor.is_signer || !frame.actor.is_writable || frame.actor.executable {
            return Err(AdapterError::AccountPrivilege.into());
        }
        require_writable_protocol(frame.root)?;
        require_writable_protocol(frame.ticket)?;
        if frame.rent_credit.is_signer || !frame.rent_credit.is_writable {
            return Err(AdapterError::AccountPrivilege.into());
        }
        for immutable in [
            frame.recipe,
            frame.aggregate,
            frame.derived,
            frame.capitalization,
            frame.resolution_material,
            frame.recipe_cursor,
            frame.aggregate_cursor,
            frame.derived_cursor,
            frame.capitalization_cursor,
            frame.resolution_material_cursor,
            frame.capability_manifest,
            frame.capability_manifest_cursor,
            frame.capability_template,
            frame.capability_template_cursor,
        ] {
            require_readonly(immutable)?;
        }
        require_distinct(accounts)?;
        Ok(frame)
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
    capability_template: &'a AccountInfo<'info>,
    capability_template_cursor: &'a AccountInfo<'info>,
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
            capability_template: account(accounts, 11)?,
            capability_template_cursor: account(accounts, 12)?,
            system_program: account(accounts, 13)?,
            rent_sysvar: account(accounts, 14)?,
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
        require_readonly(frame.capability_template)?;
        require_readonly(frame.capability_template_cursor)?;
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
    resolution_material: &'a AccountInfo<'info>,
    resolution_material_cursor: &'a AccountInfo<'info>,
    capability_template: &'a AccountInfo<'info>,
    capability_template_cursor: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    capability_manifest_cursor: &'a AccountInfo<'info>,
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
            resolution_material: account(accounts, 14)?,
            resolution_material_cursor: account(accounts, 15)?,
            capability_template: account(accounts, 16)?,
            capability_template_cursor: account(accounts, 17)?,
            capability_manifest: account(accounts, 18)?,
            capability_manifest_cursor: account(accounts, 19)?,
            system_program: account(accounts, 20)?,
            rent_sysvar: account(accounts, 21)?,
        };
        InstantiateNextFrameV1::validate(&[
            meta(frame.actor),
            meta(frame.root),
            meta(frame.recipe),
            meta(frame.derived),
            meta(frame.capitalization),
            meta(frame.resolution_material),
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
        require_readonly(frame.resolution_material_cursor)?;
        require_readonly(frame.capability_template)?;
        require_readonly(frame.capability_template_cursor)?;
        require_readonly(frame.capability_manifest)?;
        require_readonly(frame.capability_manifest_cursor)?;
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
    let capability_template = authenticate_series_template(
        program_id,
        frame.capability_template,
        frame.capability_template_cursor,
        frame.rent_sysvar,
        recipe.capability_template_id,
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
        capability_template,
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
    let resolution_material_digest = record_digest(frame.resolution_material)?;
    let source_material = authenticate_series_source_material(
        program_id,
        frame.resolution_material,
        frame.resolution_material_cursor,
        frame.rent_sysvar,
        resolution_material_digest,
    )?;
    let capability_manifest = authenticate_series_capability_manifest(
        program_id,
        frame.capability_template,
        frame.capability_template_cursor,
        frame.capability_manifest,
        frame.capability_manifest_cursor,
        frame.rent_sysvar,
        &recipe,
        source_material,
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
        source_material,
        capability_manifest,
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

#[derive(Clone, Copy)]
struct ConsumePlan {
    semantic: dclutch_series_contract::TicketConsumptionPlanV1,
    found: FoundingPlan,
    actor_lamports_before: u64,
    root_lamports_before: u64,
    rent_credit: RentCreditV1,
}

#[inline(never)]
fn process_consume(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: ConsumeTicketV1,
) -> Result<(), ProgramError> {
    let frame = ConsumeFrame::parse(accounts)?;
    let plan = authenticate_consume(program_id, &frame, instruction)?;

    // The actor is only a transient System payer. This exact credit is removed
    // by the authenticated Found plan in the same instruction, and the actor
    // must return to its pre-instruction balance.
    move_lamports(
        frame.ticket,
        frame.actor,
        plan.semantic.market_principal,
        AdapterError::SeriesTransition,
    )?;
    execute_preflighted_found_market_and_fund(program_id, frame.found_accounts, plan.found)?;
    if frame.actor.lamports() != plan.actor_lamports_before {
        return Err(AdapterError::SeriesPostcondition.into());
    }

    let ticket_refund = plan
        .semantic
        .ticket_lamports_before
        .checked_sub(plan.semantic.market_principal)
        .ok_or(AdapterError::Arithmetic)?;
    move_lamports(
        frame.ticket,
        frame.rent_credit,
        ticket_refund,
        AdapterError::SeriesTransition,
    )?;
    persist_exact(frame.root, &plan.semantic.root_after.to_bytes())?;
    frame
        .ticket
        .resize(0)
        .map_err(|_| AdapterError::SeriesClose)?;
    frame.ticket.assign(&system_program::ID);
    require_consume_post(program_id, &frame, plan)
}

#[inline(never)]
fn authenticate_consume(
    program_id: &Pubkey,
    frame: &ConsumeFrame<'_, '_>,
    instruction: ConsumeTicketV1,
) -> Result<ConsumePlan, ProgramError> {
    let rent = authenticated_rent(frame.rent_sysvar)?;
    let root = decode_program_record::<SERIES_ROOT_BYTES_V1, SeriesRootV1>(
        program_id,
        frame.root,
        SeriesRootV1::decode,
    )?;
    authenticate_root_pda(program_id, frame.root, root)?;
    if frame.root.lamports() < rent.minimum_balance(SERIES_ROOT_BYTES_V1) {
        return Err(AdapterError::SeriesAuthentication.into());
    }

    let recipe_digest = root.recipe_id.to_bytes();
    let aggregate_digest = root.aggregate_id.to_bytes();
    let derived_digest = record_digest(frame.derived)?;
    let capitalization_digest = record_digest(frame.capitalization)?;
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
    let resolution_material_digest = record_digest(frame.resolution_material)?;
    let source_material = authenticate_series_source_material(
        program_id,
        frame.resolution_material,
        frame.resolution_material_cursor,
        frame.rent_sysvar,
        resolution_material_digest,
    )?;
    let capability_manifest = authenticate_series_capability_manifest(
        program_id,
        frame.capability_template,
        frame.capability_template_cursor,
        frame.capability_manifest,
        frame.capability_manifest_cursor,
        frame.rent_sysvar,
        &recipe,
        source_material,
    )?;
    let ticket = decode_program_record::<OCCURRENCE_TICKET_BYTES_V1, OccurrenceTicketV1>(
        program_id,
        frame.ticket,
        OccurrenceTicketV1::decode,
    )?;
    authenticate_ticket_pda(program_id, frame.ticket, frame.root.key, ticket)?;

    let authority = Pubkey::new_from_array(root.refund_authority.to_bytes());
    let minimum_credit = rent.minimum_balance(dclutch_rent_contract::RENT_CREDIT_BYTES_V1);
    let rent_credit = authenticate_rent_credit(
        program_id,
        frame.rent_credit,
        refund_authority(&authority)?,
        Some(minimum_credit),
    )?;
    let semantic = plan_consume_ticket_v1(
        root,
        identity(frame.root.key.to_bytes())?,
        root.recipe_id,
        &recipe,
        root.aggregate_id,
        &aggregate,
        identity(derived_digest)?,
        &derived,
        source_material,
        capability_manifest,
        identity(capitalization_digest)?,
        &capitalization,
        ticket,
        instruction,
        frame.ticket.lamports(),
        frame.rent_credit.lamports(),
    )
    .map_err(map_transition_error)?;
    let found_authority =
        Pubkey::new_from_array(semantic.found_obligations.refund_authority.to_bytes());
    if found_authority != authority {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    let found_instruction = synthesize_found_instruction(&semantic.found_obligations, &recipe)?;
    let found = preflight_found_market_and_fund(
        program_id,
        frame.found_accounts,
        found_instruction,
        found_authority,
        semantic.market_principal,
    )?;
    if found.required_payer_debit()? != semantic.market_principal {
        return Err(AdapterError::SeriesAuthentication.into());
    }

    // No fallible mutable borrow may first appear after Found has executed.
    preflight_mutable(&[frame.root, frame.ticket, frame.rent_credit])?;
    Ok(ConsumePlan {
        semantic,
        found,
        actor_lamports_before: frame.actor.lamports(),
        root_lamports_before: frame.root.lamports(),
        rent_credit,
    })
}

fn synthesize_found_instruction(
    obligations: &dclutch_series_contract::FoundCompositionObligationsV1,
    recipe: &SeriesRecipeV1,
) -> Result<FoundMarketAndFundV1, ProgramError> {
    if obligations.claim_basis_id != recipe.claim_basis_id
        || obligations.capacity_profile_id != recipe.capacity_profile_id
        || obligations.realm_id != recipe.realm_id
        || obligations.generation
            != recipe
                .generation_at(obligations.occurrence_index)
                .map_err(map_transition_error)?
    {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    let identity = MarketIdentity::new(
        core_content_id(obligations.realm_id)?,
        core_content_id(obligations.product_instance_id)?,
        core_content_id(obligations.claim_basis_id)?,
        core_content_id(obligations.resolution_policy_id)?,
        core_content_id(obligations.capability_manifest_id)?,
        obligations.generation,
    );
    if hash(&identity.to_bytes()).to_bytes() != obligations.market_identity_id.to_bytes() {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    let outcome_count = u8::try_from(recipe.outcome_count).map_err(|_| AdapterError::Arithmetic)?;
    FoundMarketAndFundV1::new(identity, outcome_count)
        .map_err(|_| AdapterError::SeriesAuthentication.into())
}

fn core_content_id(identity: IdentityV1) -> Result<CoreContentId, ProgramError> {
    CoreContentId::new(identity.to_bytes()).map_err(|_| AdapterError::SeriesAuthentication.into())
}

fn authenticate_ticket_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &Pubkey,
    ticket: OccurrenceTicketV1,
) -> Result<(), ProgramError> {
    let root = root.to_bytes();
    let index = ticket.occurrence_index.to_le_bytes();
    let bump = [ticket.pda_bump];
    let expected = Pubkey::create_program_address(
        &[SERIES_TICKET_PDA_DOMAIN_V1, &root, &index, &bump],
        program_id,
    )
    .map_err(|_| AdapterError::SeriesAuthentication)?;
    if account.key != &expected {
        return Err(AdapterError::SeriesAuthentication.into());
    }
    Ok(())
}

fn require_consume_post(
    program_id: &Pubkey,
    frame: &ConsumeFrame<'_, '_>,
    plan: ConsumePlan,
) -> Result<(), ProgramError> {
    if frame.actor.lamports() != plan.actor_lamports_before
        || frame.root.lamports() != plan.root_lamports_before
        || frame.ticket.lamports() != plan.semantic.ticket_lamports_after
        || frame.ticket.owner != &system_program::ID
        || !frame.ticket.data_is_empty()
        || frame.rent_credit.lamports() != plan.semantic.rent_credit_after
        || decode_program_record::<SERIES_ROOT_BYTES_V1, SeriesRootV1>(
            program_id,
            frame.root,
            SeriesRootV1::decode,
        )? != plan.semantic.root_after
    {
        return Err(AdapterError::SeriesPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, frame.rent_credit, plan.rent_credit)
}

fn move_lamports(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    amount: u64,
    error: AdapterError,
) -> Result<(), ProgramError> {
    let source_after = source
        .lamports()
        .checked_sub(amount)
        .ok_or(AdapterError::Arithmetic)?;
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(AdapterError::Arithmetic)?;
    let mut source_lamports = source.try_borrow_mut_lamports().map_err(|_| error)?;
    let mut destination_lamports = destination.try_borrow_mut_lamports().map_err(|_| error)?;
    **source_lamports = source_after;
    **destination_lamports = destination_after;
    Ok(())
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

#[inline(never)]
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
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V3,
        digest,
        |record| {
            SeriesRecipeV1::decode(record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )
}

#[inline(never)]
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

#[inline(never)]
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

#[inline(never)]
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

#[inline(never)]
fn authenticate_series_source_material<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    digest: [u8; 32],
) -> Result<OccurrenceSourceMaterialV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        account,
        cursor,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        digest,
        |record| {
            authenticate_occurrence_source_material_v1(identity(digest)?, record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )
}

#[inline(never)]
fn authenticate_series_template<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    expected_template_id: IdentityV1,
) -> Result<dclutch_series_contract::SeriesCapabilityTemplateV1, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        account,
        cursor,
        rent,
        CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
        expected_template_id.to_bytes(),
        |record| {
            authenticate_series_capability_template_v1(expected_template_id, record.exact_content())
                .map_err(|_| AdapterError::SeriesAuthentication.into())
        },
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_series_capability_manifest<'info>(
    program_id: &Pubkey,
    template: &AccountInfo<'info>,
    template_cursor: &AccountInfo<'info>,
    manifest: &AccountInfo<'info>,
    manifest_cursor: &AccountInfo<'info>,
    rent: &AccountInfo<'info>,
    recipe: &SeriesRecipeV1,
    source_material: OccurrenceSourceMaterialV1,
) -> Result<OccurrenceCapabilityManifestV1, ProgramError> {
    let manifest_digest = record_digest(manifest)?;
    with_authenticated_finalized_record_v1(
        program_id,
        template,
        template_cursor,
        rent,
        CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
        recipe.capability_template_id.to_bytes(),
        |template_record| {
            with_authenticated_finalized_record_v1(
                program_id,
                manifest,
                manifest_cursor,
                rent,
                CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                manifest_digest,
                |manifest_record| {
                    authenticate_occurrence_capability_manifest_v1(
                        recipe.capability_template_id,
                        template_record.exact_content(),
                        source_material.material_id(),
                        identity(manifest_digest)?,
                        manifest_record.exact_content(),
                    )
                    .map_err(|_| AdapterError::SeriesAuthentication.into())
                },
            )
        },
    )
}

#[inline(never)]
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

fn require_writable_protocol(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.is_signer || !account.is_writable || account.executable {
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

    fn test_identity(byte: u8) -> IdentityV1 {
        IdentityV1::new([byte; 32]).expect("nonzero identity")
    }

    fn test_recipe() -> SeriesRecipeV1 {
        SeriesRecipeV1 {
            realm_id: test_identity(1),
            terms_id: test_identity(2),
            claim_basis_id: test_identity(3),
            result_domain_id: test_identity(4),
            capacity_profile_id: test_identity(4),
            compiler_release_id: IdentityV1::new(
                dclutch_series_contract::PRODUCT_COMPILER_RELEASE_ID_V1,
            )
            .expect("nonzero release"),
            occurrence_schedule_id: test_identity(5),
            source_schedule_id: test_identity(6),
            capability_template_id: test_identity(7),
            occurrence_derivation_release_id: IdentityV1::new(
                dclutch_series_contract::OCCURRENCE_DERIVATION_RELEASE_ID_V1,
            )
            .expect("nonzero release"),
            source_derivation_release_id: IdentityV1::new(
                dclutch_series_contract::SOURCE_DERIVATION_RELEASE_ID_V1,
            )
            .expect("nonzero release"),
            capability_derivation_release_id: IdentityV1::new(
                dclutch_series_contract::CAPABILITY_DERIVATION_RELEASE_ID_V1,
            )
            .expect("nonzero release"),
            market_derivation_release_id: IdentityV1::new(
                dclutch_series_contract::MARKET_DERIVATION_RELEASE_ID_V1,
            )
            .expect("nonzero release"),
            capitalization_schedule_id: test_identity(8),
            first_occurrence_time: 100,
            cadence_seconds: 60,
            occurrence_count: 2,
            first_generation: 9,
            outcome_count: 2,
        }
    }

    fn test_found_obligations(
        recipe: SeriesRecipeV1,
    ) -> dclutch_series_contract::FoundCompositionObligationsV1 {
        let product_instance_id = test_identity(10);
        let resolution_policy_id = test_identity(11);
        let manifest_id = test_identity(12);
        let market = MarketIdentity::new(
            core_content_id(recipe.realm_id).expect("realm"),
            core_content_id(product_instance_id).expect("product"),
            core_content_id(recipe.claim_basis_id).expect("claim"),
            core_content_id(resolution_policy_id).expect("policy"),
            core_content_id(manifest_id).expect("manifest"),
            recipe.first_generation,
        );
        dclutch_series_contract::FoundCompositionObligationsV1 {
            realm_id: recipe.realm_id,
            terms_id: recipe.terms_id,
            claim_basis_id: recipe.claim_basis_id,
            capacity_profile_id: recipe.capacity_profile_id,
            compiler_release_id: recipe.compiler_release_id,
            occurrence_artifact_id: test_identity(13),
            occurrence_id: test_identity(14),
            product_instance_id,
            source_spec_id: test_identity(15),
            source_window_id: test_identity(16),
            statistic_id: test_identity(17),
            resolution_policy_id,
            capability_manifest_id: manifest_id,
            market_identity_id: IdentityV1::new(hash(&market.to_bytes()).to_bytes())
                .expect("market identity"),
            occurrence_index: 0,
            occurrence_time: recipe.first_occurrence_time,
            generation: recipe.first_generation,
            market_principal: 10,
            refund_authority: test_identity(20),
        }
    }

    #[test]
    fn router_admits_all_four_complete_wires_and_refuses_partial_wires() {
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
        assert!(is_routable_instruction(&wire));
        assert!(!is_routable_instruction(
            wire.get(..7).expect("fixed short span")
        ));
        *wire.get_mut(0).expect("fixed magic byte") ^= 1;
        assert!(!is_routable_instruction(&wire));
    }

    #[test]
    fn capability_successor_frames_have_one_exact_physical_shape() {
        assert_eq!(CREATE_ACCOUNTS_V1, 15);
        assert_eq!(INSTANTIATE_ACCOUNTS_V1, 22);
        assert_eq!(FOUND_ACCOUNTS_V1, 18);
        assert_eq!(CONSUME_ACCOUNTS_V1, 30);
        assert_eq!(CLOSE_ACCOUNTS_V1, 6);
    }

    #[test]
    fn series_record_release_ids_are_pinned_to_labels() {
        assert_eq!(
            hash(b"dclutch/schema/series-recipe-v3").to_bytes(),
            SERIES_RECIPE_SCHEMA_RELEASE_ID_V3
        );
        assert_eq!(
            hash(b"dclutch/schema/capability-template-v1").to_bytes(),
            CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1
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
    fn found_wire_is_derived_and_refuses_semantic_substitution() {
        let recipe = test_recipe();
        let obligations = test_found_obligations(recipe);
        let found = synthesize_found_instruction(&obligations, &recipe).expect("exact Found wire");
        assert_eq!(
            hash(&found.identity().to_bytes()).to_bytes(),
            obligations.market_identity_id.to_bytes()
        );
        assert_eq!(found.outcome_count(), 2);

        let mut substituted = obligations;
        substituted.product_instance_id = test_identity(18);
        assert_eq!(
            synthesize_found_instruction(&substituted, &recipe),
            Err(AdapterError::SeriesAuthentication.into())
        );
        let mut substituted_recipe = recipe;
        substituted_recipe.claim_basis_id = test_identity(19);
        assert_eq!(
            synthesize_found_instruction(&obligations, &substituted_recipe),
            Err(AdapterError::SeriesAuthentication.into())
        );
    }

    #[test]
    fn executable_consume_decodes_before_exact_frame_access() {
        let instruction = dclutch_series_contract::ConsumeTicketV1 { expected_index: 0 }.to_bytes();
        assert_eq!(
            dispatch(&Pubkey::new_unique(), &[], &instruction),
            Err(AdapterError::AccountFrameLength.into())
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
