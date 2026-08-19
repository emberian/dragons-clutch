//! Finalize, exact settlement, and the three permissionless lapse routes.
//!
//! Finalize requires the complete staged verification mask and exact
//! `REVERIFIED` status for every retained Candidate, entitles both
//! reservations, closes loser accounts to their own payers, and creates the
//! donation-ledger-bound receipt and pot. Settle consumes the model's settle
//! kernel semantics byte-for-byte: outcome, quantity, price, and
//! consideration come only from the selected reverified Candidate, the exact
//! buyer/seller Position pair moves in economic-role order, conservation is
//! checked before any reservation is consumed, and the durable `SETTLED`
//! receipt lands in Epoch V4 before every transient authority closes. Each
//! lapse writes its distinct durable receipt and returns every transient
//! principal exactly once. Long-lived account values are heap-boxed and every
//! large decode happens in its own `#[inline(never)]` frame so each handler
//! stays inside the fixed SBF stack frame.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{read_rent, require_creatable, require_system_program};
use crate::seeds;
use clutch_batch_policy_identity::direct_window_v1::{
    DirectCandidateEntryV1, DIRECT_CANDIDATE_STATUS_SELECTED, DIRECT_WINDOW_PHASE_SELECTED,
    MAX_DIRECT_CANDIDATES,
};
use clutch_solana_layout::{
    account_len,
    direct_selection_v3::{
        DirectCandidateV3Account, DirectEpochV4Account, DirectFundingLedgerV3,
        DirectWindowV3Account, DIRECT_CANDIDATE_STATUS_REVERIFIED,
        DIRECT_CANDIDATE_V3_BYTES, DIRECT_EPOCH_V4_BYTES, DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
        DIRECT_LIFECYCLE_PHASE_SELECTED, DIRECT_LIFECYCLE_PHASE_TERMINAL,
        DIRECT_LIFECYCLE_PHASE_VERIFYING, DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN,
        DIRECT_RESERVATION_V2_BYTES, DIRECT_TERMINAL_REASON_EMPTY_LAPSE,
        DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE, DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE,
        DIRECT_TERMINAL_REASON_SETTLED, DIRECT_WINDOW_PHASE_VERIFYING, DIRECT_WINDOW_V3_BYTES,
        DIRECT_WORK_BUDGET_BYTES,
    },
    reservation::{RESERVATION_STATE_ACTIVE, RESERVATION_STATE_ENTITLED},
    FinalPotAccount, Hash32, PositionAccount, SettlementReceiptAccount, EPOCH_PHASE_CLEARED,
    EPOCH_PHASE_LAPSED, EPOCH_PHASE_SETTLED, MAX_OUTCOMES, POT_PHASE_CLOSED, RECEIPT_LEG_DIRECT,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::common::{
    close_funded_account, frozen_pair, observe_funding, pay_reward, read_epoch_v4_boxed,
    read_reservation_v2_boxed, read_window_v3_boxed, read_work_budget,
    release_reservations_into_positions, require_neutral_sink, sink_hash, write_epoch_v4,
    write_reservation_v2, write_window_v3,
};
use super::staged::require_selection_slot;
use super::{
    create_pda_account_full_principal, direct_creation_funding, observe_direct_funding,
    DIRECT_NEUTRAL_SINK_V3,
};

macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

const FINALIZE_BASE_ACCOUNT_COUNT: usize = 13;
const IX_FINALIZE_PAYER: usize = 0;
const IX_FINALIZE_EPOCH: usize = 1;
const IX_FINALIZE_PAGE: usize = 2;
const IX_FINALIZE_WINDOW: usize = 3;
const IX_FINALIZE_RES_ZERO: usize = 4;
const IX_FINALIZE_RES_ONE: usize = 5;
const IX_FINALIZE_TOP_START: usize = 6;

const SETTLE_ACCOUNT_COUNT: usize = 21;
const IX_SETTLE_KEEPER: usize = 0;
const IX_SETTLE_EPOCH: usize = 1;
const IX_SETTLE_PAGE: usize = 2;
const IX_SETTLE_WINDOW: usize = 3;
const IX_SETTLE_CANDIDATE: usize = 4;
const IX_SETTLE_BUY_POSITION: usize = 5;
const IX_SETTLE_SELL_POSITION: usize = 6;
const IX_SETTLE_BUY_RESERVATION: usize = 7;
const IX_SETTLE_SELL_RESERVATION: usize = 8;
const IX_SETTLE_RECEIPT: usize = 9;
const IX_SETTLE_POT: usize = 10;
const IX_SETTLE_WORK: usize = 11;
const IX_SETTLE_SINK: usize = 12;
const IX_SETTLE_CLOCK: usize = 13;
const IX_SETTLE_CANDIDATE_PAYER: usize = 14;
const IX_SETTLE_WINDOW_PAYER: usize = 15;
const IX_SETTLE_RECEIPT_PAYER: usize = 16;
const IX_SETTLE_POT_PAYER: usize = 17;
const IX_SETTLE_WORK_SPONSOR: usize = 18;
const IX_SETTLE_BUY_RES_PAYER: usize = 19;
const IX_SETTLE_SELL_RES_PAYER: usize = 20;

const LAPSE_EMPTY_ACCOUNT_COUNT: usize = 13;
const LAPSE_SELECTED_ACCOUNT_COUNT: usize = 21;
const IX_LAPSE_KEEPER: usize = 0;
const IX_LAPSE_EPOCH: usize = 1;
const IX_LAPSE_PAGE: usize = 2;

/// `FinalizeDirectSelectionV3`.
#[inline(never)]
pub(super) fn finalize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        accounts.len() > FINALIZE_BASE_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_FINALIZE_PAYER])?;
    require(
        accounts[IX_FINALIZE_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_FINALIZE_WINDOW],
        true,
        &[DIRECT_WINDOW_V3_BYTES],
    )?;
    let top_count = inspect_window_top_count(&accounts[IX_FINALIZE_WINDOW])?;
    let receipt_index = IX_FINALIZE_TOP_START + top_count;
    let pot_index = receipt_index + 1;
    let work_index = receipt_index + 2;
    let sink_index = receipt_index + 3;
    let system = receipt_index + 4;
    let rent_index = receipt_index + 5;
    let clock = receipt_index + 6;
    let loser_payers_at = receipt_index + 7;
    require_count(accounts, loser_payers_at + top_count.saturating_sub(1))?;
    require_distinct(&accounts[..loser_payers_at])?;
    for (index, len, writable) in [
        (IX_FINALIZE_EPOCH, DIRECT_EPOCH_V4_BYTES, true),
        (IX_FINALIZE_PAGE, account_len::ORDER_PAGE, false),
        (IX_FINALIZE_RES_ZERO, DIRECT_RESERVATION_V2_BYTES, true),
        (IX_FINALIZE_RES_ONE, DIRECT_RESERVATION_V2_BYTES, true),
        (work_index, DIRECT_WORK_BUDGET_BYTES, true),
    ] {
        accounts::validate_state_role_lengths(program_id, &accounts[index], writable, &[len])?;
    }
    let mut index = 0usize;
    while index < top_count {
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[IX_FINALIZE_TOP_START + index],
            true,
            &[DIRECT_CANDIDATE_V3_BYTES],
        )?;
        index += 1;
    }
    require_creatable(&accounts[receipt_index])?;
    require_creatable(&accounts[pot_index])?;
    require_neutral_sink(&accounts[sink_index])?;
    require_system_program(&accounts[system])?;
    let rent = read_rent(&accounts[rent_index])?;
    let now = read_clock_slot(&accounts[clock])?;

    let mut epoch = read_epoch_v4_boxed(
        program_id,
        &accounts[IX_FINALIZE_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    require(
        epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_VERIFYING,
        ClutchError::NotActive,
    )?;
    require_selection_slot(&epoch, now)?;
    let mut window = read_window_v3_boxed(program_id, &accounts[IX_FINALIZE_WINDOW], &epoch)?;
    require(
        window.window.phase == DIRECT_WINDOW_PHASE_VERIFYING
            && usize::from(window.window.top_count) == top_count
            && top_count >= 1,
        ClutchError::NotActive,
    )?;
    let required_mask = prefix_mask(top_count as u8)?;
    require(
        window.verification_mask == required_mask,
        ClutchError::MismatchedState,
    )?;

    let orders = frozen_pair(program_id, &accounts[IX_FINALIZE_PAGE], &epoch.direct.common)?;

    // Every retained Candidate must hold exact REVERIFIED status.
    let mut fundings = [DirectFundingLedgerV3::ZERO; MAX_DIRECT_CANDIDATES];
    index = 0;
    while index < top_count {
        fundings[index] = reverified_candidate_funding(
            program_id,
            &accounts[IX_FINALIZE_TOP_START + index],
            &epoch,
            window.window.top[index],
            window.window.relation_domain_digest.0,
        )?;
        index += 1;
    }
    let mut winner = read_candidate_v3_boxed_terminal(
        program_id,
        &accounts[IX_FINALIZE_TOP_START],
        &epoch.direct.common.epoch,
    )?;
    require(
        winner.candidate.outcome == orders.outcome
            && winner.candidate.quantity == orders.quantity
            && winner.candidate.buy_index == orders.buy_index
            && winner.candidate.sell_index == orders.sell_index,
        ClutchError::MismatchedState,
    )?;

    let mut reservation_zero = read_reservation_v2_boxed(
        program_id,
        &accounts[IX_FINALIZE_RES_ZERO],
        &epoch.direct.common,
        orders.zero,
        RESERVATION_STATE_ACTIVE,
    )?;
    let mut reservation_one = read_reservation_v2_boxed(
        program_id,
        &accounts[IX_FINALIZE_RES_ONE],
        &epoch.direct.common,
        orders.one,
        RESERVATION_STATE_ACTIVE,
    )?;

    // Receipt and pot facts from the winner only; callers supply nothing.
    let price = winner.candidate.prices[usize::from(winner.candidate.outcome)];
    let consideration_units = u128::from(winner.candidate.quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        consideration_units % u128::from(epoch.direct.common.price_scale) == 0,
        ClutchError::MismatchedState,
    )?;
    let winner_id = Hash32::from_bytes(winner.candidate.candidate_id.0);
    let (receipt_address, receipt_bump) = seeds::direct_receipt_v3_pda(
        program_id,
        &epoch.direct.common.epoch.bytes(),
        &winner_id.bytes(),
    );
    let (pot_address, pot_bump) = seeds::direct_pot_v3_pda(
        program_id,
        &epoch.direct.common.epoch.bytes(),
        &winner_id.bytes(),
    );
    expect_pda(
        accounts[receipt_index].key,
        (receipt_address, receipt_bump),
        None,
    )?;
    expect_pda(accounts[pot_index].key, (pot_address, pot_bump), None)?;

    let mut budget = read_work_budget(program_id, &accounts[work_index], &epoch)?;

    // Persist monotone donation observations for everything that survives.
    epoch.epoch_funding = observe_funding(epoch.epoch_funding, &accounts[IX_FINALIZE_EPOCH])?;
    epoch.page_funding = observe_funding(epoch.page_funding, &accounts[IX_FINALIZE_PAGE])?;
    window.funding = observe_funding(window.funding, &accounts[IX_FINALIZE_WINDOW])?;
    winner.funding = observe_funding(winner.funding, &accounts[IX_FINALIZE_TOP_START])?;
    reservation_zero.funding =
        observe_funding(reservation_zero.funding, &accounts[IX_FINALIZE_RES_ZERO])?;
    reservation_one.funding =
        observe_funding(reservation_one.funding, &accounts[IX_FINALIZE_RES_ONE])?;
    budget.funding = observe_direct_funding(
        budget.funding,
        accounts[work_index]
            .lamports()
            .checked_sub(budget.reward_balance)
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let reward = budget.rewards.finalize_selection;
    pay_reward(
        &accounts[work_index],
        &accounts[IX_FINALIZE_PAYER],
        &mut budget,
        reward,
    )?;

    let receipt_funding = direct_creation_funding(
        &accounts[IX_FINALIZE_PAYER],
        &accounts[receipt_index],
        &rent,
        account_len::SETTLEMENT_RECEIPT,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let pot_funding = direct_creation_funding(
        &accounts[IX_FINALIZE_PAYER],
        &accounts[pot_index],
        &rent,
        account_len::FINAL_POT,
        DIRECT_NEUTRAL_SINK_V3,
    )?;

    // Reservation entitlement, winner selection, and the exact poststates.
    reservation_zero.reservation.state = RESERVATION_STATE_ENTITLED;
    reservation_one.reservation.state = RESERVATION_STATE_ENTITLED;
    winner.candidate.status = DIRECT_CANDIDATE_STATUS_SELECTED;
    window.window.phase = DIRECT_WINDOW_PHASE_SELECTED;
    window.window.selected_candidate = winner.candidate.candidate_id;
    window.window.selected_slot = now;
    window.verification_mask = required_mask;
    window.live_candidate_mask = 1;
    window.receipt_funding = receipt_funding;
    window.pot_funding = pot_funding;
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_SELECTED;
    epoch.direct.common.phase = EPOCH_PHASE_CLEARED;
    epoch.terminal.selected_slot = now;
    epoch.terminal.candidate = winner_id;
    epoch.terminal.relation_candidate_digest =
        Hash32::from_bytes(winner.candidate.relation_candidate_digest.0);

    /* Existing-state writes, then loser closes, then the create CPIs. */
    write_epoch_v4(&accounts[IX_FINALIZE_EPOCH], &epoch)?;
    write_window_v3(&accounts[IX_FINALIZE_WINDOW], &window)?;
    write_candidate_v3_terminal(&accounts[IX_FINALIZE_TOP_START], &winner)?;
    write_reservation_v2(&accounts[IX_FINALIZE_RES_ZERO], &reservation_zero)?;
    write_reservation_v2(&accounts[IX_FINALIZE_RES_ONE], &reservation_one)?;
    budget.encode(sink_hash(), &mut borrow_mut!(accounts[work_index])?)?;
    let mut loser = 1usize;
    while loser < top_count {
        close_funded_account(
            &accounts[IX_FINALIZE_TOP_START + loser],
            &accounts[loser_payers_at + loser - 1],
            &accounts[sink_index],
            fundings[loser],
            0,
        )?;
        loser += 1;
    }

    create_pda_account_full_principal(
        program_id,
        &accounts[IX_FINALIZE_PAYER],
        &accounts[receipt_index],
        &accounts[system],
        &rent,
        account_len::SETTLEMENT_RECEIPT,
        receipt_funding,
        0,
        &[
            seeds::SEED_DIRECT_RECEIPT_V3,
            &epoch.direct.common.epoch.bytes(),
            &winner_id.bytes(),
            &0u16.to_le_bytes(),
            &[receipt_bump],
        ],
    )?;
    create_pda_account_full_principal(
        program_id,
        &accounts[IX_FINALIZE_PAYER],
        &accounts[pot_index],
        &accounts[system],
        &rent,
        account_len::FINAL_POT,
        pot_funding,
        0,
        &[
            seeds::SEED_DIRECT_POT_V3,
            &epoch.direct.common.epoch.bytes(),
            &winner_id.bytes(),
            &[pot_bump],
        ],
    )?;
    write_receipt_and_pot(
        &accounts[receipt_index],
        &accounts[pot_index],
        &epoch,
        &winner,
        orders,
        consideration_units,
        price,
        receipt_bump,
        pot_bump,
    )?;
    Ok(())
}

/// `SettleDirectV3`.
#[inline(never)]
pub(super) fn settle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require_count(accounts, SETTLE_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_SETTLE_KEEPER])?;
    require(
        accounts[IX_SETTLE_KEEPER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(&accounts[..IX_SETTLE_CANDIDATE_PAYER])?;
    for (index, len, writable) in [
        (IX_SETTLE_EPOCH, DIRECT_EPOCH_V4_BYTES, true),
        (IX_SETTLE_PAGE, account_len::ORDER_PAGE, false),
        (IX_SETTLE_WINDOW, DIRECT_WINDOW_V3_BYTES, true),
        (IX_SETTLE_CANDIDATE, DIRECT_CANDIDATE_V3_BYTES, true),
        (IX_SETTLE_BUY_POSITION, account_len::POSITION, true),
        (IX_SETTLE_SELL_POSITION, account_len::POSITION, true),
        (IX_SETTLE_BUY_RESERVATION, DIRECT_RESERVATION_V2_BYTES, true),
        (IX_SETTLE_SELL_RESERVATION, DIRECT_RESERVATION_V2_BYTES, true),
        (IX_SETTLE_RECEIPT, account_len::SETTLEMENT_RECEIPT, true),
        (IX_SETTLE_POT, account_len::FINAL_POT, true),
        (IX_SETTLE_WORK, DIRECT_WORK_BUDGET_BYTES, true),
    ] {
        accounts::validate_state_role_lengths(program_id, &accounts[index], writable, &[len])?;
    }
    require_neutral_sink(&accounts[IX_SETTLE_SINK])?;
    let now = read_clock_slot(&accounts[IX_SETTLE_CLOCK])?;

    let mut epoch = read_epoch_v4_boxed(
        program_id,
        &accounts[IX_SETTLE_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    require(
        epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_SELECTED,
        ClutchError::NotActive,
    )?;
    require(
        now >= epoch.terminal.selected_slot && now < epoch.settlement_deadline_slot,
        ClutchError::NotActive,
    )?;
    let window = read_window_v3_boxed(program_id, &accounts[IX_SETTLE_WINDOW], &epoch)?;
    require(
        window.window.phase == DIRECT_WINDOW_PHASE_SELECTED
            && window.window.selected_candidate.0 == epoch.terminal.candidate.bytes()
            && window.window.selected_slot == epoch.terminal.selected_slot,
        ClutchError::NotActive,
    )?;
    let selected = read_candidate_v3_boxed_terminal(
        program_id,
        &accounts[IX_SETTLE_CANDIDATE],
        &epoch.direct.common.epoch,
    )?;
    require(
        selected.candidate.candidate_id == window.window.selected_candidate
            && selected.candidate.status == DIRECT_CANDIDATE_STATUS_SELECTED
            && selected.candidate.entry() == window.window.top[0]
            && selected.candidate.relation_domain_digest == window.window.relation_domain_digest
            && selected.candidate.relation_candidate_digest.0
                == epoch.terminal.relation_candidate_digest.bytes(),
        ClutchError::MismatchedState,
    )?;

    let orders = frozen_pair(program_id, &accounts[IX_SETTLE_PAGE], &epoch.direct.common)?;
    require(
        selected.candidate.outcome == orders.outcome
            && selected.candidate.quantity == orders.quantity
            && selected.candidate.buy_index == orders.buy_index
            && selected.candidate.sell_index == orders.sell_index
            && selected.candidate.fills
                == [selected.candidate.quantity, selected.candidate.quantity],
        ClutchError::MismatchedState,
    )?;
    let outcome = usize::from(selected.candidate.outcome);
    let quantity = selected.candidate.quantity;
    let price = selected.candidate.prices[outcome];
    let buy = orders.buy();
    let sell = orders.sell();
    require(
        quantity != 0 && price != 0 && price <= buy.limit && price >= sell.limit,
        ClutchError::MismatchedState,
    )?;
    // The direct policy's rounding boundary is `None`: exact division or the
    // settlement refuses, so the buyer debit equals the seller credit.
    let consideration_units = u128::from(quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let scale = u128::from(epoch.direct.common.price_scale);
    require(
        scale != 0 && consideration_units % scale == 0,
        ClutchError::MismatchedState,
    )?;
    let consideration_atoms = u64::try_from(consideration_units / scale)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;

    let buy_reservation = read_reservation_v2_boxed(
        program_id,
        &accounts[IX_SETTLE_BUY_RESERVATION],
        &epoch.direct.common,
        buy,
        RESERVATION_STATE_ENTITLED,
    )?;
    let sell_reservation = read_reservation_v2_boxed(
        program_id,
        &accounts[IX_SETTLE_SELL_RESERVATION],
        &epoch.direct.common,
        sell,
        RESERVATION_STATE_ENTITLED,
    )?;
    // Stray compartments refuse; deficits refuse, never clamp.
    require(
        buy_reservation.reservation.remaining_internal == [0; MAX_OUTCOMES]
            && sell_reservation.reservation.remaining_cash_atoms == 0
            && buy_reservation.reservation.remaining_cash_atoms >= consideration_atoms,
        ClutchError::AggregateClosureMismatch,
    )?;
    let mut stray = 0usize;
    while stray < MAX_OUTCOMES {
        require(
            stray == outcome || sell_reservation.reservation.remaining_internal[stray] == 0,
            ClutchError::AggregateClosureMismatch,
        )?;
        stray += 1;
    }
    let seller_remainder = sell_reservation.reservation.remaining_internal[outcome]
        .checked_sub(quantity)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;

    // Exact buyer/seller Positions in economic-role order.
    let mut buyer = read_position(
        program_id,
        &accounts[IX_SETTLE_BUY_POSITION],
        &epoch.direct.common,
        buy.owner,
        buy_reservation.reservation.position_generation,
    )?;
    let mut seller = read_position(
        program_id,
        &accounts[IX_SETTLE_SELL_POSITION],
        &epoch.direct.common,
        sell.owner,
        sell_reservation.reservation.position_generation,
    )?;
    require(buyer.owner != seller.owner, ClutchError::MismatchedState)?;

    authenticate_settlement_receipt(
        program_id,
        &accounts[IX_SETTLE_RECEIPT],
        &epoch,
        &window,
        consideration_units,
        quantity,
        price,
        buy.order_id,
        sell.order_id,
    )?;
    authenticate_settlement_pot(program_id, &accounts[IX_SETTLE_POT], &epoch, &window)?;

    // The model's exact Position transfer legs, conservation checked before
    // either reservation is consumed.
    let cash_before = buyer
        .cash_atoms
        .checked_add(seller.cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    buyer.cash_atoms = buyer
        .cash_atoms
        .checked_sub(consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    buyer.reserved_cash_atoms = buyer
        .reserved_cash_atoms
        .checked_sub(buy_reservation.reservation.remaining_cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    buyer.internal[outcome] = buyer.internal[outcome]
        .checked_add(quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    seller.cash_atoms = seller
        .cash_atoms
        .checked_add(consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    seller.internal[outcome] = seller.internal[outcome]
        .checked_add(seller_remainder)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let cash_after = buyer
        .cash_atoms
        .checked_add(seller.cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        cash_before == cash_after,
        ClutchError::AggregateClosureMismatch,
    )?;
    buyer.validate()?;
    seller.validate()?;

    // Durable receipt into the Epoch archive before transient closes.
    epoch.epoch_funding = observe_funding(epoch.epoch_funding, &accounts[IX_SETTLE_EPOCH])?;
    epoch.page_funding = observe_funding(epoch.page_funding, &accounts[IX_SETTLE_PAGE])?;
    let mut budget = read_work_budget(program_id, &accounts[IX_SETTLE_WORK], &epoch)?;
    budget.funding = observe_direct_funding(
        budget.funding,
        accounts[IX_SETTLE_WORK]
            .lamports()
            .checked_sub(budget.reward_balance)
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let reward = budget.rewards.settle;
    pay_reward(
        &accounts[IX_SETTLE_WORK],
        &accounts[IX_SETTLE_KEEPER],
        &mut budget,
        reward,
    )?;

    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_TERMINAL;
    epoch.direct.common.phase = EPOCH_PHASE_SETTLED;
    epoch.terminal.reason = DIRECT_TERMINAL_REASON_SETTLED;
    epoch.terminal.outcome = selected.candidate.outcome;
    epoch.terminal.terminal_reservation_count = 2;
    epoch.terminal.quantity = quantity;
    epoch.terminal.price = price;
    epoch.terminal.consideration_price_units = consideration_units;
    epoch.terminal.terminal_slot = now;

    buyer.encode(&mut borrow_mut!(accounts[IX_SETTLE_BUY_POSITION])?)?;
    seller.encode(&mut borrow_mut!(accounts[IX_SETTLE_SELL_POSITION])?)?;
    write_epoch_v4(&accounts[IX_SETTLE_EPOCH], &epoch)?;

    close_funded_account(
        &accounts[IX_SETTLE_CANDIDATE],
        &accounts[IX_SETTLE_CANDIDATE_PAYER],
        &accounts[IX_SETTLE_SINK],
        selected.funding,
        0,
    )?;
    close_funded_account(
        &accounts[IX_SETTLE_WINDOW],
        &accounts[IX_SETTLE_WINDOW_PAYER],
        &accounts[IX_SETTLE_SINK],
        window.funding,
        0,
    )?;
    close_funded_account(
        &accounts[IX_SETTLE_RECEIPT],
        &accounts[IX_SETTLE_RECEIPT_PAYER],
        &accounts[IX_SETTLE_SINK],
        window.receipt_funding,
        0,
    )?;
    close_funded_account(
        &accounts[IX_SETTLE_POT],
        &accounts[IX_SETTLE_POT_PAYER],
        &accounts[IX_SETTLE_SINK],
        window.pot_funding,
        0,
    )?;
    close_funded_account(
        &accounts[IX_SETTLE_WORK],
        &accounts[IX_SETTLE_WORK_SPONSOR],
        &accounts[IX_SETTLE_SINK],
        budget.funding,
        budget.reward_balance,
    )?;
    close_funded_account(
        &accounts[IX_SETTLE_BUY_RESERVATION],
        &accounts[IX_SETTLE_BUY_RES_PAYER],
        &accounts[IX_SETTLE_SINK],
        buy_reservation.funding,
        0,
    )?;
    close_funded_account(
        &accounts[IX_SETTLE_SELL_RESERVATION],
        &accounts[IX_SETTLE_SELL_RES_PAYER],
        &accounts[IX_SETTLE_SINK],
        sell_reservation.funding,
        0,
    )?;
    Ok(())
}

/// `LapseEmptyDirectV3`.
#[inline(never)]
pub(super) fn lapse_empty(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require_count(accounts, LAPSE_EMPTY_ACCOUNT_COUNT)?;
    // [keeper, epoch, page, res0, res1, pos0, pos1, work, sink, clock,
    //  work sponsor, res0 payer, res1 payer]
    let res_at = 3usize;
    let pos_at = 5usize;
    let work_index = 7usize;
    let sink_index = 8usize;
    let clock = 9usize;
    let recipients_at = 10usize;
    require_distinct(&accounts[..recipients_at])?;
    let context = lapse_shared(
        program_id,
        accounts,
        intent_market,
        intent_epoch,
        res_at,
        pos_at,
        work_index,
        sink_index,
        clock,
        RESERVATION_STATE_ACTIVE,
    )?;
    require(
        context.epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_FROZEN_EMPTY,
        ClutchError::NotActive,
    )?;
    require(
        context.now >= context.epoch.selection_deadline_slot,
        ClutchError::NotActive,
    )?;
    finish_lapse(
        program_id,
        accounts,
        context,
        DIRECT_TERMINAL_REASON_EMPTY_LAPSE,
        recipients_at,
    )
}

/// `LapseUnselectedDirectV3`.
#[inline(never)]
pub(super) fn lapse_unselected(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    // [keeper, epoch, page, window, candidates.., res0, res1, pos0, pos1,
    //  work, sink, clock, window payer, candidate payers.., work sponsor,
    //  res0 payer, res1 payer]
    require(accounts.len() >= 4, ClutchError::AccountCount)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[3],
        true,
        &[DIRECT_WINDOW_V3_BYTES],
    )?;
    let top_count = inspect_window_top_count(&accounts[3])?;
    let res_at = 4 + top_count;
    let pos_at = res_at + 2;
    let work_index = pos_at + 2;
    let sink_index = work_index + 1;
    let clock = sink_index + 1;
    let recipients_at = clock + 1;
    require_count(accounts, recipients_at + 1 + top_count + 3)?;
    require_distinct(&accounts[..recipients_at])?;
    let context = lapse_shared(
        program_id,
        accounts,
        intent_market,
        intent_epoch,
        res_at,
        pos_at,
        work_index,
        sink_index,
        clock,
        RESERVATION_STATE_ACTIVE,
    )?;
    require(
        matches!(
            context.epoch.lifecycle_phase,
            DIRECT_LIFECYCLE_PHASE_WINDOW_OPEN | DIRECT_LIFECYCLE_PHASE_VERIFYING
        ),
        ClutchError::NotActive,
    )?;
    require(
        context.now >= context.epoch.selection_deadline_slot,
        ClutchError::NotActive,
    )?;
    let window = read_window_v3_boxed(program_id, &accounts[3], &context.epoch)?;
    require(
        usize::from(window.window.top_count) == top_count && top_count >= 1,
        ClutchError::MismatchedState,
    )?;

    // Close every live Candidate, then the Window, each to its own payer.
    let mut index = 0usize;
    while index < top_count {
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[4 + index],
            true,
            &[DIRECT_CANDIDATE_V3_BYTES],
        )?;
        close_candidate_entry(
            program_id,
            &accounts[4 + index],
            &accounts[recipients_at + 1 + index],
            &accounts[sink_index],
            &context.epoch.direct.common.epoch,
            window.window.top[index],
        )?;
        index += 1;
    }
    close_funded_account(
        &accounts[3],
        &accounts[recipients_at],
        &accounts[sink_index],
        window.funding,
        0,
    )?;
    finish_lapse(
        program_id,
        accounts,
        context,
        DIRECT_TERMINAL_REASON_PRESELECTION_LAPSE,
        recipients_at + 1 + top_count,
    )
}

/// `LapseSelectedDirectV3`.
#[inline(never)]
pub(super) fn lapse_selected(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require_count(accounts, LAPSE_SELECTED_ACCOUNT_COUNT)?;
    // [keeper, epoch, page, window, candidate, receipt, pot, res0, res1,
    //  pos0, pos1, work, sink, clock, window payer, candidate payer,
    //  receipt payer, pot payer, work sponsor, res0 payer, res1 payer]
    let window_index = 3usize;
    let candidate_index = 4usize;
    let receipt_index = 5usize;
    let pot_index = 6usize;
    let res_at = 7usize;
    let pos_at = 9usize;
    let work_index = 11usize;
    let sink_index = 12usize;
    let clock = 13usize;
    let recipients_at = 14usize;
    require_distinct(&accounts[..recipients_at])?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[window_index],
        true,
        &[DIRECT_WINDOW_V3_BYTES],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[candidate_index],
        true,
        &[DIRECT_CANDIDATE_V3_BYTES],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[receipt_index],
        true,
        &[account_len::SETTLEMENT_RECEIPT],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[pot_index],
        true,
        &[account_len::FINAL_POT],
    )?;
    let context = lapse_shared(
        program_id,
        accounts,
        intent_market,
        intent_epoch,
        res_at,
        pos_at,
        work_index,
        sink_index,
        clock,
        RESERVATION_STATE_ENTITLED,
    )?;
    require(
        context.epoch.lifecycle_phase == DIRECT_LIFECYCLE_PHASE_SELECTED,
        ClutchError::NotActive,
    )?;
    require(
        context.now >= context.epoch.settlement_deadline_slot,
        ClutchError::NotActive,
    )?;
    let window = read_window_v3_boxed(program_id, &accounts[window_index], &context.epoch)?;
    require(
        window.window.phase == DIRECT_WINDOW_PHASE_SELECTED
            && window.window.selected_candidate.0 == context.epoch.terminal.candidate.bytes()
            && window.window.selected_slot == context.epoch.terminal.selected_slot,
        ClutchError::MismatchedState,
    )?;
    let selected_funding = selected_candidate_funding(
        program_id,
        &accounts[candidate_index],
        &context.epoch.direct.common.epoch,
        window.window.selected_candidate.0,
    )?;
    let winner_bytes = context.epoch.terminal.candidate.bytes();
    expect_pda(
        accounts[receipt_index].key,
        seeds::direct_receipt_v3_pda(
            program_id,
            &context.epoch.direct.common.epoch.bytes(),
            &winner_bytes,
        ),
        None,
    )?;
    expect_pda(
        accounts[pot_index].key,
        seeds::direct_pot_v3_pda(
            program_id,
            &context.epoch.direct.common.epoch.bytes(),
            &winner_bytes,
        ),
        None,
    )?;

    close_funded_account(
        &accounts[candidate_index],
        &accounts[recipients_at + 1],
        &accounts[sink_index],
        selected_funding,
        0,
    )?;
    close_funded_account(
        &accounts[window_index],
        &accounts[recipients_at],
        &accounts[sink_index],
        window.funding,
        0,
    )?;
    close_funded_account(
        &accounts[receipt_index],
        &accounts[recipients_at + 2],
        &accounts[sink_index],
        window.receipt_funding,
        0,
    )?;
    close_funded_account(
        &accounts[pot_index],
        &accounts[recipients_at + 3],
        &accounts[sink_index],
        window.pot_funding,
        0,
    )?;
    finish_lapse(
        program_id,
        accounts,
        context,
        DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE,
        recipients_at + 4,
    )
}

/// Shared lapse skeleton: keeper, epoch, frozen page, exact reservation pair
/// with the expected state, both distinct-owner Positions, WorkBudget, sink,
/// and Clock, all authenticated before any release. The full reservation
/// decodes stay in helper frames; only their funding ledgers survive here.
struct LapseContext {
    epoch: Box<DirectEpochV4Account>,
    orders: crate::instructions::direct_selection::DirectOrders,
    reservation_fundings: [DirectFundingLedgerV3; 2],
    budget: clutch_solana_layout::direct_selection_v3::DirectWorkBudgetV1Account,
    now: u64,
    res_at: usize,
    pos_at: usize,
    work_index: usize,
    sink_index: usize,
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn lapse_shared(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent_market: Hash32,
    intent_epoch: Hash32,
    res_at: usize,
    pos_at: usize,
    work_index: usize,
    sink_index: usize,
    clock: usize,
    reservation_state: u8,
) -> Outcome<LapseContext> {
    require_signer(&accounts[IX_LAPSE_KEEPER])?;
    require(
        accounts[IX_LAPSE_KEEPER].is_writable,
        ClutchError::NotWritable,
    )?;
    for (index, len, writable) in [
        (IX_LAPSE_EPOCH, DIRECT_EPOCH_V4_BYTES, true),
        (IX_LAPSE_PAGE, account_len::ORDER_PAGE, false),
        (res_at, DIRECT_RESERVATION_V2_BYTES, true),
        (res_at + 1, DIRECT_RESERVATION_V2_BYTES, true),
        (work_index, DIRECT_WORK_BUDGET_BYTES, true),
    ] {
        accounts::validate_state_role_lengths(program_id, &accounts[index], writable, &[len])?;
    }
    require_neutral_sink(&accounts[sink_index])?;
    let now = read_clock_slot(&accounts[clock])?;
    let epoch = read_epoch_v4_boxed(
        program_id,
        &accounts[IX_LAPSE_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    let orders = frozen_pair(program_id, &accounts[IX_LAPSE_PAGE], &epoch.direct.common)?;
    // Each full 618-byte reservation is decoded, bound, and dropped in its
    // own frame; only the 48-byte funding ledgers survive in this context.
    let mut reservation_fundings = [DirectFundingLedgerV3::ZERO; 2];
    let mut index = 0usize;
    while index < 2 {
        let order = if index == 0 { orders.zero } else { orders.one };
        reservation_fundings[index] = read_reservation_v2_boxed(
            program_id,
            &accounts[res_at + index],
            &epoch.direct.common,
            order,
            reservation_state,
        )?
        .funding;
        index += 1;
    }
    let budget = read_work_budget(program_id, &accounts[work_index], &epoch)?;
    Ok(LapseContext {
        epoch,
        orders,
        reservation_fundings,
        budget,
        now,
        res_at,
        pos_at,
        work_index,
        sink_index,
    })
}

/// Release both reservations, pay the lapse reward, close WorkBudget and both
/// reservations, and write the distinct durable receipt.
#[inline(never)]
fn finish_lapse(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    context: LapseContext,
    reason: u8,
    reservation_payers_at_plus_sponsor: usize,
) -> Outcome<()> {
    let LapseContext {
        mut epoch,
        orders,
        reservation_fundings,
        mut budget,
        now,
        res_at,
        pos_at,
        work_index,
        sink_index,
    } = context;
    let sponsor_index = reservation_payers_at_plus_sponsor;
    let reservation_payers_at = sponsor_index + 1;

    // Release exact remaining envelopes into both distinct-owner Positions.
    let order_pair = [orders.zero, orders.one];
    release_reservations_into_positions(
        program_id,
        &epoch.direct.common,
        &order_pair,
        &accounts[res_at..res_at + 2],
        &accounts[pos_at..pos_at + 2],
    )?;

    budget.funding = observe_direct_funding(
        budget.funding,
        accounts[work_index]
            .lamports()
            .checked_sub(budget.reward_balance)
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let reward = budget.rewards.lapse;
    pay_reward(
        &accounts[work_index],
        &accounts[IX_LAPSE_KEEPER],
        &mut budget,
        reward,
    )?;
    close_funded_account(
        &accounts[work_index],
        &accounts[sponsor_index],
        &accounts[sink_index],
        budget.funding,
        budget.reward_balance,
    )?;
    let mut index = 0usize;
    while index < 2 {
        close_funded_account(
            &accounts[res_at + index],
            &accounts[reservation_payers_at + index],
            &accounts[sink_index],
            reservation_fundings[index],
            0,
        )?;
        index += 1;
    }

    epoch.epoch_funding = observe_funding(epoch.epoch_funding, &accounts[IX_LAPSE_EPOCH])?;
    epoch.page_funding = observe_funding(epoch.page_funding, &accounts[IX_LAPSE_PAGE])?;
    epoch.lifecycle_phase = DIRECT_LIFECYCLE_PHASE_TERMINAL;
    epoch.direct.common.phase = EPOCH_PHASE_LAPSED;
    epoch.terminal.reason = reason;
    epoch.terminal.terminal_reservation_count = 2;
    epoch.terminal.terminal_slot = now;
    if reason != DIRECT_TERMINAL_REASON_POSTSELECTION_LAPSE {
        epoch.terminal.selected_slot = 0;
        epoch.terminal.candidate = Hash32::ZERO;
        epoch.terminal.relation_candidate_digest = Hash32::ZERO;
    }
    write_epoch_v4(&accounts[IX_LAPSE_EPOCH], &epoch)?;
    Ok(())
}

/// One retained candidate account, required to hold exact REVERIFIED status;
/// the full decode stays in this frame and only its funding survives.
#[inline(never)]
fn reverified_candidate_funding(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &DirectEpochV4Account,
    expected_entry: DirectCandidateEntryV1,
    expected_domain_digest: [u8; 32],
) -> Outcome<DirectFundingLedgerV3> {
    let value = DirectCandidateV3Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::direct_candidate_v3_pda(
            program_id,
            &epoch.direct.common.epoch.bytes(),
            &value.candidate.candidate_id.0,
        ),
        Some(value.candidate.stored_bump),
    )?;
    require(
        value.candidate.status == DIRECT_CANDIDATE_STATUS_REVERIFIED
            && value.candidate.entry() == expected_entry
            && value.candidate.relation_domain_digest.0 == expected_domain_digest,
        ClutchError::MismatchedState,
    )?;
    Ok(value.funding)
}

/// Boxed decode of one V3 Candidate account with its PDA authenticated.
#[inline(never)]
fn read_candidate_v3_boxed_terminal(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch_id: &Hash32,
) -> Outcome<Box<DirectCandidateV3Account>> {
    let value = DirectCandidateV3Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::direct_candidate_v3_pda(
            program_id,
            &epoch_id.bytes(),
            &value.candidate.candidate_id.0,
        ),
        Some(value.candidate.stored_bump),
    )?;
    Ok(Box::new(value))
}

/// Persist one V3 Candidate value; encode temporaries stay in this frame.
#[inline(never)]
fn write_candidate_v3_terminal(
    account: &AccountInfo,
    value: &DirectCandidateV3Account,
) -> Outcome<()> {
    let mut data = borrow_mut!(account)?;
    value.encode(sink_hash(), &mut data)?;
    Ok(())
}

/// The SELECTED candidate's funding for a post-deadline lapse close.
#[inline(never)]
fn selected_candidate_funding(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch_id: &Hash32,
    expected_id: [u8; 32],
) -> Outcome<DirectFundingLedgerV3> {
    let value = DirectCandidateV3Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::direct_candidate_v3_pda(
            program_id,
            &epoch_id.bytes(),
            &value.candidate.candidate_id.0,
        ),
        Some(value.candidate.stored_bump),
    )?;
    require(
        value.candidate.candidate_id.0 == expected_id
            && value.candidate.status == DIRECT_CANDIDATE_STATUS_SELECTED,
        ClutchError::MismatchedState,
    )?;
    Ok(value.funding)
}

/// Close one live retained Candidate to its own payer during unselected lapse.
#[inline(never)]
fn close_candidate_entry(
    program_id: &Pubkey,
    account: &AccountInfo,
    payer: &AccountInfo,
    sink: &AccountInfo,
    epoch_id: &Hash32,
    expected_entry: DirectCandidateEntryV1,
) -> Outcome<()> {
    let value = DirectCandidateV3Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::direct_candidate_v3_pda(
            program_id,
            &epoch_id.bytes(),
            &value.candidate.candidate_id.0,
        ),
        Some(value.candidate.stored_bump),
    )?;
    require(
        value.candidate.entry() == expected_entry,
        ClutchError::MismatchedState,
    )?;
    close_funded_account(account, payer, sink, value.funding, 0)
}

/// The Window's retained count without holding the full decode in the caller.
#[inline(never)]
fn inspect_window_top_count(account: &AccountInfo) -> Outcome<usize> {
    let window = DirectWindowV3Account::decode(&account.data.borrow(), sink_hash())?;
    Ok(usize::from(window.window.top_count))
}

/// Build, validate, and persist the receipt and pot; their temporaries stay
/// in this frame.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn write_receipt_and_pot(
    receipt_account: &AccountInfo,
    pot_account: &AccountInfo,
    epoch: &DirectEpochV4Account,
    winner: &DirectCandidateV3Account,
    orders: crate::instructions::direct_selection::DirectOrders,
    consideration_units: u128,
    price: u64,
    receipt_bump: u8,
    pot_bump: u8,
) -> Outcome<()> {
    let winner_id = Hash32::from_bytes(winner.candidate.candidate_id.0);
    let receipt = SettlementReceiptAccount {
        epoch: epoch.direct.common.epoch,
        market: epoch.direct.common.market,
        candidate: winner_id,
        buy_order_id: orders.buy().order_id,
        sell_order_id: orders.sell().order_id,
        consideration_price_units: consideration_units,
        quantity: winner.candidate.quantity,
        settled_quantity: 0,
        price,
        sequence: 1,
        slice_index: 0,
        outcome: winner.candidate.outcome,
        leg_kind: RECEIPT_LEG_DIRECT,
        consumed_flags: 0,
        stored_bump: receipt_bump,
        flags: 0,
    };
    receipt.validate()?;
    let pot = FinalPotAccount {
        epoch: epoch.direct.common.epoch,
        market: epoch.direct.common.market,
        candidate: winner_id,
        pot_internal: [0; MAX_OUTCOMES],
        pot_cash_price_units: 0,
        rounding_pot_price_units: 0,
        outcome_count: 2,
        phase: POT_PHASE_CLOSED,
        stored_bump: pot_bump,
        flags: 0,
    };
    pot.validate()?;
    receipt.encode(&mut borrow_mut!(receipt_account)?)?;
    pot.encode(&mut borrow_mut!(pot_account)?)?;
    Ok(())
}

/// Authenticate the pre-frozen V3 settlement receipt byte-for-byte.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_settlement_receipt(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &DirectEpochV4Account,
    window: &DirectWindowV3Account,
    consideration_units: u128,
    quantity: u64,
    price: u64,
    buy_order_id: Hash32,
    sell_order_id: Hash32,
) -> Outcome<()> {
    let receipt = SettlementReceiptAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::direct_receipt_v3_pda(
            program_id,
            &receipt.epoch.bytes(),
            &receipt.candidate.bytes(),
        ),
        Some(receipt.stored_bump),
    )?;
    require(
        receipt.epoch == epoch.direct.common.epoch
            && receipt.market == epoch.direct.common.market
            && receipt.candidate.bytes() == window.window.selected_candidate.0
            && receipt.buy_order_id == buy_order_id
            && receipt.sell_order_id == sell_order_id
            && receipt.consideration_price_units == consideration_units
            && receipt.quantity == quantity
            && receipt.settled_quantity == 0
            && receipt.price == price
            && receipt.sequence == 1
            && receipt.slice_index == 0
            && receipt.leg_kind == RECEIPT_LEG_DIRECT
            && receipt.consumed_flags == 0,
        ClutchError::MismatchedState,
    )
}

/// Authenticate the zero pot bound to the selected Candidate.
#[inline(never)]
fn authenticate_settlement_pot(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &DirectEpochV4Account,
    window: &DirectWindowV3Account,
) -> Outcome<()> {
    let pot = FinalPotAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::direct_pot_v3_pda(program_id, &pot.epoch.bytes(), &pot.candidate.bytes()),
        Some(pot.stored_bump),
    )?;
    require(
        pot.epoch == epoch.direct.common.epoch
            && pot.market == epoch.direct.common.market
            && pot.candidate.bytes() == window.window.selected_candidate.0
            && pot.phase == POT_PHASE_CLOSED
            && pot.pot_internal == [0; MAX_OUTCOMES]
            && pot.pot_cash_price_units == 0
            && pot.rounding_pot_price_units == 0,
        ClutchError::MismatchedState,
    )
}

/// One authenticated live Position bound to a settlement role.
#[inline(never)]
fn read_position(
    program_id: &Pubkey,
    account: &AccountInfo,
    common: &clutch_solana_layout::EpochAccount,
    owner: Hash32,
    generation: u64,
) -> Outcome<PositionAccount> {
    let position = PositionAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::position_pda(
            program_id,
            &position.market.bytes(),
            &position.owner.bytes(),
        ),
        Some(position.stored_bump),
    )?;
    require(
        position.market == common.market
            && position.owner == owner
            && position.generation == generation
            && position.close_state == 0,
        ClutchError::MismatchedState,
    )?;
    Ok(position)
}

fn prefix_mask(count: u8) -> Outcome<u8> {
    if usize::from(count) > MAX_DIRECT_CANDIDATES {
        return Err(Refusal::Adapter(ClutchError::MismatchedState));
    }
    Ok(if count == 0 { 0 } else { (1u8 << count) - 1 })
}
