//! Shared authenticated readers and exact lamport-ledger primitives for the
//! routed Direct V3 lifecycle family.
//!
//! Every reader decodes with the canonical hostile-byte codec, re-derives the
//! account's PDA from its own decoded bytes, and binds it to the exact V4
//! Epoch supplied by the instruction. Every lamport move here is a checked,
//! zero-sum split among accounts already named by the transaction; nothing is
//! inferred from a caller-selected index.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::direct_selection::validate_order_reservation;
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;
use clutch_solana_layout::{
    direct_selection_v3::{
        DirectEpochV4Account, DirectFundingLedgerV3, DirectReservationV2Account,
        DirectWindowV3Account, DirectWorkBudgetV1Account,
    },
    Hash32, OrderRecord, OrderSlot, PositionAccount, MAX_OUTCOMES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::{observe_direct_funding, DIRECT_NEUTRAL_SINK_V3, DIRECT_VERIFIER_RELEASE_ID_V3};

/// The immutable neutral sink as the 32-byte identity the codecs bind.
pub(super) fn sink_hash() -> Hash32 {
    Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes())
}

/// The sink account of a closing transition: the exact incinerator, writable.
pub(super) fn require_neutral_sink(account: &AccountInfo) -> Outcome<()> {
    require(
        *account.key == DIRECT_NEUTRAL_SINK_V3,
        ClutchError::MismatchedState,
    )?;
    require(account.is_writable, ClutchError::NotWritable)
}

/// Decode and authenticate the exact 672-byte Direct Epoch V4.
///
/// `DirectEpochV4Account::decode` already validates the full hostile shape,
/// including recomputing the epoch-bound DirectBatchPolicy V3 identity from
/// the persisted `verifier_release_id`; the release equality below therefore
/// carries `validate_for_release` semantics without a second digest pass.
pub(super) fn read_epoch_v4(
    program_id: &Pubkey,
    account: &AccountInfo,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<DirectEpochV4Account> {
    let epoch = DirectEpochV4Account::decode(&account.data.borrow())?;
    require(
        epoch.verifier_release_id == DIRECT_VERIFIER_RELEASE_ID_V3
            && epoch.neutral_lamport_sink == sink_hash()
            && epoch.direct.common.market == intent_market
            && epoch.direct.common.epoch == intent_epoch,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::epoch_pda(
            program_id,
            &epoch.direct.common.market.bytes(),
            epoch.direct.common.epoch_index,
        ),
        Some(epoch.direct.common.stored_bump),
    )?;
    Ok(epoch)
}

/// Decode and bind the Direct Window V3 to its exact frozen Epoch.
pub(super) fn read_window_v3(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &DirectEpochV4Account,
) -> Outcome<DirectWindowV3Account> {
    let window = DirectWindowV3Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::direct_window_v3_pda(program_id, &epoch.direct.common.epoch.bytes()),
        Some(window.window.stored_bump),
    )?;
    require(
        window.window.epoch_id.0 == epoch.direct.common.epoch.bytes()
            && window.window.market_id.0 == epoch.direct.common.market.bytes()
            && window.window.order_set_id.0 == epoch.direct.common.order_set.bytes()
            && window.window.policy_id.0 == epoch.direct.common.policy.bytes()
            && window.window.opens_slot == epoch.direct.submission_opens_slot
            && window.window.closes_slot == epoch.direct.submission_closes_slot
            && window.selection_deadline_slot == epoch.selection_deadline_slot
            && window.settlement_deadline_slot == epoch.settlement_deadline_slot,
        ClutchError::MismatchedState,
    )?;
    Ok(window)
}

/// Decode and bind the finite WorkBudget to its exact Epoch and release.
pub(super) fn read_work_budget(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &DirectEpochV4Account,
) -> Outcome<DirectWorkBudgetV1Account> {
    let budget = DirectWorkBudgetV1Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::direct_work_v3_pda(program_id, &epoch.direct.common.epoch.bytes()),
        Some(budget.stored_bump),
    )?;
    require(
        budget.epoch == epoch.direct.common.epoch
            && budget.policy == epoch.direct_policy_v3_id
            && budget.verifier_release_id == DIRECT_VERIFIER_RELEASE_ID_V3,
        ClutchError::MismatchedState,
    )?;
    Ok(budget)
}

/// Decode one exact 618-byte Reservation V2 and bind it to one page order.
pub(super) fn read_reservation_v2(
    program_id: &Pubkey,
    account: &AccountInfo,
    common: &clutch_solana_layout::EpochAccount,
    order: OrderRecord,
    state: u8,
) -> Outcome<DirectReservationV2Account> {
    let value = DirectReservationV2Account::decode(&account.data.borrow(), sink_hash())?;
    expect_pda(
        account.key,
        seeds::reservation_pda(program_id, &value.reservation.reservation.bytes()),
        Some(value.reservation.stored_bump),
    )?;
    validate_order_reservation(&value.reservation, common, &OrderSlot::Single(order), state)?;
    Ok(value)
}

/// Observe one surviving funded account's live balance into its ledger.
pub(super) fn observe_funding(
    funding: DirectFundingLedgerV3,
    account: &AccountInfo,
) -> Outcome<DirectFundingLedgerV3> {
    observe_direct_funding(funding, account.lamports(), DIRECT_NEUTRAL_SINK_V3)
}

/// Move exact lamports between two accounts this transaction already named.
///
/// The debited account is program-owned state authenticated by the caller;
/// the credited account only receives, so aliasing there is harmless.
pub(super) fn move_lamports(from: &AccountInfo, to: &AccountInfo, amount: u64) -> Outcome<()> {
    require(to.is_writable, ClutchError::NotWritable)?;
    {
        let mut lamports = from
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = lamports
            .checked_sub(amount)
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    }
    let mut lamports = to
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

/// Debit one frozen keeper reward from spendable WorkBudget rewards only.
pub(super) fn pay_reward(
    budget_account: &AccountInfo,
    keeper: &AccountInfo,
    budget: &mut DirectWorkBudgetV1Account,
    reward: u64,
) -> Outcome<()> {
    budget.reward_balance = budget
        .reward_balance
        .checked_sub(reward)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    budget.rewards_paid = budget
        .rewards_paid
        .checked_add(reward)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    move_lamports(budget_account, keeper, reward)
}

/// Close one funded transient account with the exact DonationLedger split.
///
/// The recorded payer receives exactly its principal plus `extra_to_recipient`
/// (the WorkBudget reward refund, whose sponsor is its payer); every other
/// live lamport — the monotone donation and any later surplus — goes only to
/// the immutable neutral sink. A live balance below the accounted
/// compartments plus the prior donation refuses before any byte moves.
pub(super) fn close_funded_account(
    target: &AccountInfo,
    recipient: &AccountInfo,
    sink: &AccountInfo,
    funding: DirectFundingLedgerV3,
    extra_to_recipient: u64,
) -> Outcome<()> {
    require(
        Hash32::from_bytes(recipient.key.to_bytes()) == funding.payer,
        ClutchError::MismatchedState,
    )?;
    require(recipient.is_writable, ClutchError::NotWritable)?;
    let observed = target.lamports();
    let owed = funding
        .payer_principal_lamports
        .checked_add(extra_to_recipient)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let neutral = observed
        .checked_sub(owed)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    require(
        neutral >= funding.prior_donation_lamports,
        ClutchError::AggregateClosureMismatch,
    )?;
    {
        let mut lamports = target
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = 0;
    }
    {
        let mut lamports = recipient
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = lamports
            .checked_add(owed)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    {
        let mut lamports = sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = lamports
            .checked_add(neutral)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    target
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    target.assign(&SYSTEM_PROGRAM_ID);
    Ok(())
}

/// Release the exact remaining envelopes of a Reservation prefix back into
/// the matching live Positions and persist the Position poststates.
///
/// `orders` and `reservations` are the authenticated page prefix in exact
/// page order. `position_accounts` are the distinct owners' Positions in
/// first-appearance order; a repeated owner aggregates both releases into one
/// poststate. A release that changes nothing refuses, mirroring the model's
/// unchanged-poststate rule.
pub(super) fn release_reservations_into_positions(
    program_id: &Pubkey,
    common: &clutch_solana_layout::EpochAccount,
    orders: &[OrderRecord],
    reservations: &[DirectReservationV2Account],
    position_accounts: &[AccountInfo],
) -> Outcome<()> {
    require(
        orders.len() == reservations.len(),
        ClutchError::MismatchedState,
    )?;
    // The distinct-owner count expected from the order prefix.
    let mut expected_owners = 0usize;
    let mut index = 0usize;
    while index < orders.len() {
        let mut seen = false;
        let mut prior = 0usize;
        while prior < index {
            if orders[prior].owner == orders[index].owner {
                seen = true;
            }
            prior += 1;
        }
        if !seen {
            expected_owners += 1;
        }
        index += 1;
    }
    require(
        position_accounts.len() == expected_owners && expected_owners <= 2,
        ClutchError::AccountCount,
    )?;

    let mut before = [zero_position(); 2];
    let mut positions = [zero_position(); 2];
    let mut owner_cursor = 0usize;
    index = 0;
    while index < orders.len() {
        let mut seen = false;
        let mut prior = 0usize;
        while prior < index {
            if orders[prior].owner == orders[index].owner {
                seen = true;
            }
            prior += 1;
        }
        if !seen {
            let account = &position_accounts[owner_cursor];
            crate::accounts::validate_state_role_lengths(
                program_id,
                account,
                true,
                &[clutch_solana_layout::account_len::POSITION],
            )?;
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
                position.owner == orders[index].owner
                    && position.market == common.market
                    && position.close_state == 0,
                ClutchError::MismatchedState,
            )?;
            before[owner_cursor] = position;
            positions[owner_cursor] = position;
            owner_cursor += 1;
        }
        index += 1;
    }

    index = 0;
    while index < reservations.len() {
        let reservation = &reservations[index].reservation;
        let mut found = None;
        let mut position_index = 0usize;
        while position_index < expected_owners {
            if positions[position_index].owner == reservation.owner {
                require(found.is_none(), ClutchError::MismatchedState)?;
                found = Some(position_index);
            }
            position_index += 1;
        }
        let position_index = found.ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            positions[position_index].generation == reservation.position_generation,
            ClutchError::MismatchedState,
        )?;
        positions[position_index].reserved_cash_atoms = positions[position_index]
            .reserved_cash_atoms
            .checked_sub(reservation.remaining_cash_atoms)
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
        let mut outcome = 0usize;
        while outcome < MAX_OUTCOMES {
            positions[position_index].internal[outcome] = positions[position_index].internal
                [outcome]
                .checked_add(reservation.remaining_internal[outcome])
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            outcome += 1;
        }
        index += 1;
    }

    let mut position_index = 0usize;
    while position_index < expected_owners {
        require(
            positions[position_index] != before[position_index],
            ClutchError::MismatchedState,
        )?;
        positions[position_index].validate()?;
        position_index += 1;
    }
    position_index = 0;
    while position_index < expected_owners {
        let mut data = position_accounts[position_index]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        positions[position_index].encode(&mut data)?;
        position_index += 1;
    }
    Ok(())
}

/// Read the exact frozen two-order pair from an already-sealed page.
///
/// Terminal transitions carry no grid account: the pair's grid admission was
/// fixed at Freeze and the sealed page digest plus the Epoch's order-set
/// commitment authenticate the exact bytes read here.
pub(super) fn frozen_pair(
    program_id: &Pubkey,
    page: &AccountInfo,
    common: &clutch_solana_layout::EpochAccount,
) -> Outcome<crate::instructions::direct_selection::DirectOrders> {
    let page_data = page.data.borrow();
    let header = clutch_solana_layout::stream::verify_page(&page_data)?;
    expect_pda(
        page.key,
        seeds::page_pda(program_id, &header.epoch.bytes(), header.page_index),
        Some(header.stored_bump),
    )?;
    require(
        header.market == common.market
            && header.epoch == common.epoch
            && header.page_index == 0
            && header.page_count == 1
            && header.order_count == 2
            && header.tombstone_count == 0
            && header.frozen == 1
            && header.order_set == common.order_set
            && header.set_order_count == common.order_count,
        ClutchError::MismatchedState,
    )?;
    let mut cursor = clutch_solana_layout::stream::OrderSlotCursor::new(&page_data)?;
    let zero = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let one = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let (zero, one) = match (zero, one) {
        (OrderSlot::Single(zero), OrderSlot::Single(one)) => (zero, one),
        _ => return Err(clutch_solana_layout::CodecError::InvalidEnum.into()),
    };
    let (buy_index, sell_index, buy, sell) = match (zero.side, one.side) {
        (0, 1) => (0, 1, zero, one),
        (1, 0) => (1, 0, one, zero),
        _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };
    require(
        zero.owner != one.owner
            && zero.outcome == one.outcome
            && zero.quantity == one.quantity
            && zero.quantity != 0
            && zero.minimum_fill == 0
            && one.minimum_fill == 0
            && zero.flags == 0
            && one.flags == 0
            && buy.limit >= sell.limit,
        ClutchError::MismatchedState,
    )?;
    Ok(crate::instructions::direct_selection::DirectOrders {
        zero,
        one,
        buy_index,
        sell_index,
        outcome: zero.outcome,
        quantity: zero.quantity,
        buy_limit: buy.limit,
        sell_limit: sell.limit,
    })
}

/// Canonical inactive Position placeholder for fixed release scratch space.
fn zero_position() -> PositionAccount {
    PositionAccount {
        market: Hash32::ZERO,
        owner: Hash32::ZERO,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: 0,
        close_state: 0,
    }
}
