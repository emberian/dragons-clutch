//! Atomic Freeze of exact two-reservation authority and the permissionless
//! pre-freeze abort.
//!
//! Freeze is the only transition that creates a WorkBudget: rent principal
//! and the complete reward deposit arrive in one authenticated create delta,
//! so a never-frozen Epoch cannot trap a work reserve. Abort is the bounded
//! terminal route for an Epoch that reached submission open without ever
//! freezing: the exact zero-, one-, or two-account Reservation prefix is
//! released into its live Positions and closed, each recorded principal
//! returns only to its payer, and every other lamport goes to the immutable
//! neutral sink.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::direct_selection::load_direct_orders;
use crate::instructions::genesis::{read_rent, require_creatable, require_system_program};
use crate::seeds;
use clutch_solana_layout::{
    account_len,
    direct_selection_v3::{
        DirectBatchPolicyV3, DirectKeeperRewardsV3, DirectTerminalReceiptV3,
        DirectWorkBudgetV1Account, DIRECT_BATCH_POLICY_V3_BYTES, DIRECT_EPOCH_V4_BYTES,
        DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY, DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN,
        DIRECT_LIFECYCLE_PHASE_TERMINAL, DIRECT_RESERVATION_V2_BYTES,
        DIRECT_TERMINAL_REASON_PREFREEZE_ABORT, DIRECT_WORK_BUDGET_BYTES,
        DIRECT_WORK_BUDGET_PHASE_ACTIVE,
    },
    reservation::RESERVATION_STATE_ACTIVE,
    stream, CodecError, Hash32, OrderRecord, OrderSlot, PriceGridAccount, EPOCH_PHASE_FROZEN,
    EPOCH_PHASE_OPEN,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::common::{
    close_funded_account, observe_funding, read_epoch_v4, read_reservation_v2,
    release_reservations_into_positions, require_neutral_sink, sink_hash,
};
use super::{
    create_pda_account_full_principal, direct_creation_funding, require_exact_direct_policy,
    DIRECT_NEUTRAL_SINK_V3, DIRECT_VERIFIER_RELEASE_ID_V3,
};

macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

const FREEZE_ACCOUNT_COUNT: usize = 11;
const IX_FREEZE_SPONSOR: usize = 0;
const IX_FREEZE_EPOCH: usize = 1;
const IX_FREEZE_PAGE: usize = 2;
const IX_FREEZE_GRID: usize = 3;
const IX_FREEZE_POLICY: usize = 4;
const IX_FREEZE_RES_ZERO: usize = 5;
const IX_FREEZE_RES_ONE: usize = 6;
const IX_FREEZE_WORK: usize = 7;
const IX_FREEZE_SYSTEM: usize = 8;
const IX_FREEZE_RENT: usize = 9;
const IX_FREEZE_CLOCK: usize = 10;

const FREEZE_STATE_ROLES: [StateRole; 6] = [
    StateRole::writable(IX_FREEZE_EPOCH, DIRECT_EPOCH_V4_BYTES),
    StateRole::writable(IX_FREEZE_PAGE, account_len::ORDER_PAGE),
    StateRole::read_only(IX_FREEZE_GRID, account_len::PRICE_GRID),
    StateRole::read_only(IX_FREEZE_POLICY, DIRECT_BATCH_POLICY_V3_BYTES),
    StateRole::writable(IX_FREEZE_RES_ZERO, DIRECT_RESERVATION_V2_BYTES),
    StateRole::writable(IX_FREEZE_RES_ONE, DIRECT_RESERVATION_V2_BYTES),
];

const IX_ABORT_EPOCH: usize = 0;
const IX_ABORT_CLOCK: usize = 1;
const IX_ABORT_SINK: usize = 2;
const IX_ABORT_PAGE: usize = 3;
const ABORT_FIXED_ACCOUNTS: usize = 3;

/// `FreezeDirectEpochV4`: exact two ACTIVE Reservation V2 accounts become the
/// frozen order authority and the WorkBudget is created fully capitalized.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn freeze(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    reward_deposit: u64,
    rewards: DirectKeeperRewardsV3,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require_count(accounts, FREEZE_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_FREEZE_SPONSOR])?;
    require(
        accounts[IX_FREEZE_SPONSOR].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &FREEZE_STATE_ROLES)?;
    require_system_program(&accounts[IX_FREEZE_SYSTEM])?;
    require_creatable(&accounts[IX_FREEZE_WORK])?;
    let rent = read_rent(&accounts[IX_FREEZE_RENT])?;
    let now = read_clock_slot(&accounts[IX_FREEZE_CLOCK])?;

    let mut epoch = read_epoch_v4(
        program_id,
        &accounts[IX_FREEZE_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    require(
        epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN
            && epoch.direct.common.phase == EPOCH_PHASE_OPEN
            && epoch.terminal == DirectTerminalReceiptV3::EMPTY
            && epoch.direct.common.page_count == 1,
        ClutchError::NotActive,
    )?;
    require(
        now < epoch.direct.submission_opens_slot,
        ClutchError::NotActive,
    )?;

    let supplied_policy = DirectBatchPolicyV3::decode(&accounts[IX_FREEZE_POLICY].data.borrow())?;
    require_exact_direct_policy(
        epoch.direct.common.epoch,
        epoch.direct_policy_v3_id,
        supplied_policy,
    )?;
    expect_pda(
        accounts[IX_FREEZE_POLICY].key,
        seeds::direct_batch_policy_v3_pda(
            program_id,
            &epoch.direct.common.epoch.bytes(),
            &epoch.direct_policy_v3_id.bytes(),
        ),
        None,
    )?;

    let grid = PriceGridAccount::decode(&accounts[IX_FREEZE_GRID].data.borrow())?;
    expect_pda(
        accounts[IX_FREEZE_GRID].key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;
    require(
        grid.grid == epoch.direct.common.price_grid
            && grid.price_scale == epoch.direct.common.price_scale,
        ClutchError::MismatchedState,
    )?;

    let page_data = accounts[IX_FREEZE_PAGE].data.borrow();
    let page_header = stream::OrderPageHeader::decode(&page_data)?;
    expect_pda(
        accounts[IX_FREEZE_PAGE].key,
        seeds::page_pda(
            program_id,
            &page_header.epoch.bytes(),
            page_header.page_index,
        ),
        Some(page_header.stored_bump),
    )?;
    // Two-order pair profile: distinct owners, opposite single-Egg sides on
    // one outcome, equal quantities, zero minimum-fill/flags, crossed limits.
    let orders = load_direct_orders(&page_data, &grid, &epoch.direct.common, false)?;
    let header = stream::verify_page_on_grid(&page_data, &grid)?;
    let (order_set, set_order_count) = stream::frozen_set_commitment(&[&page_data])?;
    drop(page_data);

    let mut reservation_zero = read_reservation_v2(
        program_id,
        &accounts[IX_FREEZE_RES_ZERO],
        &epoch.direct.common,
        orders.zero,
        RESERVATION_STATE_ACTIVE,
    )?;
    let mut reservation_one = read_reservation_v2(
        program_id,
        &accounts[IX_FREEZE_RES_ONE],
        &epoch.direct.common,
        orders.one,
        RESERVATION_STATE_ACTIVE,
    )?;

    // Persist the canonical monotone donation observation for every account
    // this transition touches before its authority freezes.
    epoch.epoch_funding = observe_funding(epoch.epoch_funding, &accounts[IX_FREEZE_EPOCH])?;
    epoch.page_funding = observe_funding(epoch.page_funding, &accounts[IX_FREEZE_PAGE])?;
    reservation_zero.funding =
        observe_funding(reservation_zero.funding, &accounts[IX_FREEZE_RES_ZERO])?;
    reservation_one.funding =
        observe_funding(reservation_one.funding, &accounts[IX_FREEZE_RES_ONE])?;

    // WorkBudget target and its one exact create delta: rent principal plus
    // the complete reward-only deposit from the same authenticated sponsor.
    let (work_address, work_bump) =
        seeds::direct_work_v3_pda(program_id, &epoch.direct.common.epoch.bytes());
    expect_pda(accounts[IX_FREEZE_WORK].key, (work_address, work_bump), None)?;
    let work_funding = direct_creation_funding(
        &accounts[IX_FREEZE_SPONSOR],
        &accounts[IX_FREEZE_WORK],
        &rent,
        DIRECT_WORK_BUDGET_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let budget = DirectWorkBudgetV1Account {
        epoch: epoch.direct.common.epoch,
        policy: epoch.direct_policy_v3_id,
        verifier_release_id: DIRECT_VERIFIER_RELEASE_ID_V3,
        reward_sponsor: work_funding.payer,
        funding: work_funding,
        reward_balance: reward_deposit,
        initial_reward_balance: reward_deposit,
        rewards_paid: 0,
        rewards,
        stored_bump: work_bump,
        phase: DIRECT_WORK_BUDGET_PHASE_ACTIVE,
        flags: 0,
        reserved: [0; 2],
    };
    budget.validate(sink_hash())?;

    epoch.direct.common.phase = EPOCH_PHASE_FROZEN;
    epoch.direct.common.order_set = order_set;
    epoch.direct.common.first_order_id = header.first_order_id;
    epoch.direct.common.last_order_id = header.last_order_id;
    epoch.direct.common.order_count = set_order_count;
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY;

    /* Existing-state writes precede the WorkBudget System CPI deliberately: a
     * failed full-deposit transfer is a real late-failure rollback case, and
     * no account-data borrow survives into that CPI. */
    stream::seal_page(
        &mut borrow_mut!(accounts[IX_FREEZE_PAGE])?,
        order_set,
        set_order_count,
    )?;
    reservation_zero.encode(sink_hash(), &mut borrow_mut!(accounts[IX_FREEZE_RES_ZERO])?)?;
    reservation_one.encode(sink_hash(), &mut borrow_mut!(accounts[IX_FREEZE_RES_ONE])?)?;
    epoch.encode(&mut borrow_mut!(accounts[IX_FREEZE_EPOCH])?)?;

    create_pda_account_full_principal(
        program_id,
        &accounts[IX_FREEZE_SPONSOR],
        &accounts[IX_FREEZE_WORK],
        &accounts[IX_FREEZE_SYSTEM],
        &rent,
        DIRECT_WORK_BUDGET_BYTES,
        work_funding,
        reward_deposit,
        &[
            seeds::SEED_DIRECT_WORK_V3,
            &epoch.direct.common.epoch.bytes(),
            &[work_bump],
        ],
    )?;
    budget.encode(sink_hash(), &mut borrow_mut!(accounts[IX_FREEZE_WORK])?)?;
    Ok(())
}

/// `AbortUnfrozenDirectV4`: permissionless terminal route for an Epoch that
/// reached submission open while still unfrozen.
#[inline(never)]
pub(super) fn abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        accounts.len() >= ABORT_FIXED_ACCOUNTS,
        ClutchError::AccountCount,
    )?;
    accounts::validate_state_roles(
        program_id,
        accounts,
        &[StateRole::writable(IX_ABORT_EPOCH, DIRECT_EPOCH_V4_BYTES)],
    )?;
    let now = read_clock_slot(&accounts[IX_ABORT_CLOCK])?;
    require_neutral_sink(&accounts[IX_ABORT_SINK])?;

    let mut epoch = read_epoch_v4(
        program_id,
        &accounts[IX_ABORT_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    require(
        epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN
            && epoch.direct.common.phase == EPOCH_PHASE_OPEN,
        ClutchError::NotActive,
    )?;
    require(
        now >= epoch.direct.submission_opens_slot,
        ClutchError::NotActive,
    )?;

    let mut orders = [OrderRecord {
        owner: Hash32::ZERO,
        order_id: Hash32::ZERO,
        outcome: 0,
        side: 0,
        quantity: 0,
        limit: 0,
        minimum_fill: 0,
        flags: 0,
        generation: 0,
        expiry_epoch: 0,
    }; 2];
    let mut reservation_count = 0usize;
    let page_present = epoch.direct.common.page_count == 1;
    if page_present {
        require(
            accounts.len() > IX_ABORT_PAGE,
            ClutchError::AccountCount,
        )?;
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[IX_ABORT_PAGE],
            false,
            &[account_len::ORDER_PAGE],
        )?;
        let page_data = accounts[IX_ABORT_PAGE].data.borrow();
        let header = stream::verify_page(&page_data)?;
        expect_pda(
            accounts[IX_ABORT_PAGE].key,
            seeds::page_pda(program_id, &header.epoch.bytes(), header.page_index),
            Some(header.stored_bump),
        )?;
        require(
            header.market == epoch.direct.common.market
                && header.epoch == epoch.direct.common.epoch
                && header.page_index == 0
                && header.page_count == 1
                && header.frozen == 0
                && header.tombstone_count == 0
                && header.set_order_count == 0
                && header.order_count <= 2,
            ClutchError::MismatchedState,
        )?;
        reservation_count = usize::from(header.order_count);
        let mut cursor = stream::OrderSlotCursor::new(&page_data)?;
        let mut index = 0usize;
        while index < reservation_count {
            let slot = cursor
                .next_slot()
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
            orders[index] = match slot {
                OrderSlot::Single(order) => order,
                _ => return Err(CodecError::InvalidEnum.into()),
            };
            index += 1;
        }
    }

    // Distinct live Positions in first-owner-appearance order.
    let mut position_count = 0usize;
    let mut index = 0usize;
    while index < reservation_count {
        let mut seen = false;
        let mut prior = 0usize;
        while prior < index {
            if orders[prior].owner == orders[index].owner {
                seen = true;
            }
            prior += 1;
        }
        if !seen {
            position_count += 1;
        }
        index += 1;
    }

    let page_len = usize::from(page_present);
    let reservations_at = ABORT_FIXED_ACCOUNTS + page_len;
    let positions_at = reservations_at + reservation_count;
    let payers_at = positions_at + position_count;
    require_count(accounts, payers_at + reservation_count)?;
    require_distinct(&accounts[..payers_at])?;

    let mut reservations = [None, None];
    index = 0;
    while index < reservation_count {
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[reservations_at + index],
            true,
            &[DIRECT_RESERVATION_V2_BYTES],
        )?;
        reservations[index] = Some(read_reservation_v2(
            program_id,
            &accounts[reservations_at + index],
            &epoch.direct.common,
            orders[index],
            RESERVATION_STATE_ACTIVE,
        )?);
        index += 1;
    }

    if reservation_count > 0 {
        let first = reservations[0].ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let held = [first, reservations[1].unwrap_or(first)];
        release_reservations_into_positions(
            program_id,
            &epoch.direct.common,
            &orders[..reservation_count],
            &held[..reservation_count],
            &accounts[positions_at..payers_at],
        )?;
        index = 0;
        while index < reservation_count {
            let reservation =
                reservations[index].ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
            close_funded_account(
                &accounts[reservations_at + index],
                &accounts[payers_at + index],
                &accounts[IX_ABORT_SINK],
                reservation.funding,
                0,
            )?;
            index += 1;
        }
    }

    epoch.epoch_funding = observe_funding(epoch.epoch_funding, &accounts[IX_ABORT_EPOCH])?;
    if page_present {
        epoch.page_funding = observe_funding(epoch.page_funding, &accounts[IX_ABORT_PAGE])?;
    }
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_TERMINAL;
    epoch.terminal = DirectTerminalReceiptV3 {
        reason: DIRECT_TERMINAL_REASON_PREFREEZE_ABORT,
        outcome: 0,
        terminal_reservation_count: reservation_count as u8,
        selected_slot: 0,
        candidate: if reservation_count > 0 {
            reservations[0]
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
                .reservation
                .reservation
        } else {
            Hash32::ZERO
        },
        relation_candidate_digest: if reservation_count > 1 {
            reservations[1]
                .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?
                .reservation
                .reservation
        } else {
            Hash32::ZERO
        },
        quantity: 0,
        price: 0,
        consideration_price_units: 0,
        terminal_slot: now,
    };
    epoch.encode(&mut borrow_mut!(accounts[IX_ABORT_EPOCH])?)?;
    Ok(())
}
