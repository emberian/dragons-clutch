//! General V2 action 38: realize one accounting-complete owner row.
//!
//! The counted root and exhaustive retained traversal derive every economic
//! fact. The selector only names the row, Position, and cash pot. The
//! zero-fee and fee-bearing account suffixes are disjoint; the latter consumes
//! the authenticated payer snapshot, grows its carry PDA to the terminal V4
//! receipt, and disposes only persisted native-rent ownership.

use core::cell::{Ref, RefMut};
use std::boxed::Box;

use clutch_batch_policy_identity::revenue_policy_v1::{
    decode_revenue_policy, RevenuePolicyV1, REVENUE_POLICY_BYTES,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_owner_settlement_payload_v1, Id32, OwnerSettlementPayloadV1,
    OwnerSettlementV5AccountV1, SettlementCashPotV1AccountV1,
};
use clutch_solana_layout::registry::GeneralV2Action;
use solana_account_info::AccountInfo;
use solana_cpi::invoke;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    read_rent, require_system_program, transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::seeds;

use super::general_v2_fee_v5::{
    compose_owner_settlement_action38_v5, OwnerFeeAccountInputV5,
    OwnerFeeRentAccountFrameV5, OwnerFeeSnapshotAccountFrameV5,
    OwnerFeeTerminalRentInputV5, PreparedOwnerSettlementAction38V5,
};
use super::general_v2_position_replay::authenticate_current_general_position_replay_v2;
use super::general_v2_settlement_traversal_v5::{
    authenticate_settlement_traversal_v5, authenticate_writable_root_settlement_traversal_v5,
    SettlementTraversalAccountFrameV5,
};

/// Fixed traversal accounts before the one-to-four PageV5 suffix.
pub const ACTION38_TRAVERSAL_PREFIX_ACCOUNTS: usize = 12;
/// Rent, row, cash pot, Position, Replay.
pub const ACTION38_ZERO_FEE_SUFFIX_ACCOUNTS: usize = 5;
/// Zero-fee suffix plus six fee facts, three rent destinations, and System.
pub const ACTION38_FEE_SUFFIX_ACCOUNTS: usize = 15;

const IX_ROOT: usize = 0;
const IX_FEED: usize = 1;
const IX_BINDING: usize = 2;
const IX_RUNTIME: usize = 3;
const IX_DOMAIN: usize = 4;
const IX_GRID: usize = 5;
const IX_REALM: usize = 6;
const IX_PROFILE: usize = 7;
const IX_POLICY: usize = 8;
const IX_TOKEN: usize = 9;
const IX_MARKET_INSTANCE: usize = 10;
const IX_GENESIS: usize = 11;

const REL_RENT: usize = 0;
const REL_OWNER_ROW: usize = 1;
const REL_CASH_POT: usize = 2;
const REL_POSITION: usize = 3;
const REL_REPLAY: usize = 4;
const REL_SELECTED_FEE: usize = 5;
const REL_FEE_CARRY: usize = 6;
const REL_PAYER_ALLOCATION: usize = 7;
const REL_BATCH_POLICY: usize = 8;
const REL_REVENUE_RECORD: usize = 9;
const REL_REVENUE_PREIMAGE: usize = 10;
const REL_CARRY_RENT_PAYER: usize = 11;
const REL_PAYER_REFUND_OWNER: usize = 12;
const REL_NEUTRAL_SINK: usize = 13;
const REL_SYSTEM_PROGRAM: usize = 14;

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn borrow_mut_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<RefMut<'a, [u8]>> {
    let data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(RefMut::map(data, |bytes| &mut **bytes))
}

fn require_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable { ClutchError::NotWritable } else { ClutchError::UnexpectedWritable },
    )?;
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
}

fn require_all_distinct(accounts: &[AccountInfo<'_>], suffix_at: usize, fee: bool) -> Outcome<()> {
    let signer = if fee { Some(suffix_at + REL_CARRY_RENT_PAYER) } else { None };
    let mut left = 0usize;
    while left < accounts.len() {
        if Some(left) != signer {
            require(!accounts[left].is_signer, ClutchError::MismatchedState)?;
        }
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn action38_frame(total: usize) -> Outcome<(usize, bool)> {
    let mut pages = 1usize;
    while pages <= clutch_solana_layout::MAX_ORDER_PAGES {
        let no_fee = ACTION38_TRAVERSAL_PREFIX_ACCOUNTS + pages
            + ACTION38_ZERO_FEE_SUFFIX_ACCOUNTS;
        if total == no_fee {
            return Ok((pages, false));
        }
        let fee = ACTION38_TRAVERSAL_PREFIX_ACCOUNTS + pages
            + ACTION38_FEE_SUFFIX_ACCOUNTS;
        if total == fee {
            return Ok((pages, true));
        }
        pages += 1;
    }
    Err(Refusal::Adapter(ClutchError::WrongAccountCount))
}

fn authenticate_cash_pot(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &contract::SettlementRootV1AccountV1,
    rent_minimum: u64,
) -> Outcome<SettlementCashPotV1AccountV1> {
    require_program_state(
        program_id,
        account,
        true,
        contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
    )?;
    let pot = SettlementCashPotV1AccountV1::decode(&borrow_data(account)?)?;
    let pda = seeds::general_v2_settlement_cash_pot_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
    );
    expect_pda(account.key, pda, Some(pot.stored_bump))?;
    let persisted = root.cash_pot_rent();
    let recorded = persisted
        .refundable_principal
        .checked_add(persisted.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        id(account.key) == root.settlement_cash_pot()
            && pot.stored_bump == root.cash_pot_bump()
            && account.lamports() >= rent_minimum
            && account.lamports() >= recorded,
        ClutchError::MismatchedState,
    )?;
    Ok(pot)
}

fn transfer_from_signer<'a>(
    payer: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    amount: u64,
) -> Outcome<()> {
    if amount == 0 {
        return Ok(());
    }
    let payer_after = payer
        .lamports()
        .checked_sub(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let instruction = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(amount),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke(
        &instruction,
        &[payer.clone(), destination.clone(), system_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        payer.lamports() == payer_after && destination.lamports() == destination_after,
        ClutchError::AccountCreationFailed,
    )
}

fn encode_common_successors(
    authenticated_root: &super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1,
    plan: &PreparedOwnerSettlementAction38V5,
    pot_before: SettlementCashPotV1AccountV1,
) -> Outcome<(std::vec::Vec<u8>, [u8; clutch_retirement::POSITION_V3_BYTES], [u8; contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES])> {
    let mut root_body = std::vec![0u8; authenticated_root.account_bytes()];
    authenticated_root.encode_owner_finalization_successor(
        plan.realization().settlement_root_poststate(),
        plan.fee_finalization_required(),
        &mut root_body,
    )?;
    let position_body = plan
        .realization()
        .position()
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut pot_body = [0u8; contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES];
    SettlementCashPotV1AccountV1 {
        semantic: plan.realization().settlement_cash_pot(),
        stored_bump: pot_before.stored_bump,
        flags: pot_before.flags,
    }
    .encode(&mut pot_body)?;
    Ok((root_body, position_body, pot_body))
}

fn write_zero_fee_bundle(
    accounts: &[AccountInfo<'_>],
    suffix_at: usize,
    authenticated_root: &super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1,
    plan: &PreparedOwnerSettlementAction38V5,
    pot_before: SettlementCashPotV1AccountV1,
) -> Outcome<()> {
    let (root_body, position_body, pot_body) =
        encode_common_successors(authenticated_root, plan, pot_before)?;
    let mut root_out = borrow_mut_data(&accounts[IX_ROOT])?;
    let mut row_out = borrow_mut_data(&accounts[suffix_at + REL_OWNER_ROW])?;
    let mut pot_out = borrow_mut_data(&accounts[suffix_at + REL_CASH_POT])?;
    let mut position_out = borrow_mut_data(&accounts[suffix_at + REL_POSITION])?;
    let mut replay_out = borrow_mut_data(&accounts[suffix_at + REL_REPLAY])?;
    root_out.copy_from_slice(&root_body);
    row_out.copy_from_slice(plan.realization().owner_settlement_poststate_body());
    pot_out.copy_from_slice(&pot_body);
    position_out.copy_from_slice(&position_body);
    replay_out.copy_from_slice(plan.replay().replay_poststate_body());
    Ok(())
}

fn write_fee_bundle<'a>(
    accounts: &'a [AccountInfo<'a>],
    suffix_at: usize,
    authenticated_root: &super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1,
    plan: &PreparedOwnerSettlementAction38V5,
    pot_before: SettlementCashPotV1AccountV1,
) -> Outcome<()> {
    let finalization = plan
        .finalization()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent = finalization.rent();
    let carry = &accounts[suffix_at + REL_FEE_CARRY];
    let payer_allocation = &accounts[suffix_at + REL_PAYER_ALLOCATION];
    let carry_payer = &accounts[suffix_at + REL_CARRY_RENT_PAYER];
    let refund_owner = &accounts[suffix_at + REL_PAYER_REFUND_OWNER];
    let neutral_sink = &accounts[suffix_at + REL_NEUTRAL_SINK];
    let system_program = &accounts[suffix_at + REL_SYSTEM_PROGRAM];
    require_system_program(system_program)?;
    let top_up = rent.carry_top_up();
    let principal = rent.payer_principal_refund();
    let donation = rent.payer_donation_credit();
    require(
        top_up.source() == id(carry_payer.key)
            && top_up.destination() == id(carry.key)
            && principal.source() == id(payer_allocation.key)
            && principal.destination() == id(refund_owner.key)
            && donation.source() == id(payer_allocation.key)
            && donation.destination() == id(neutral_sink.key)
            && payer_allocation.lamports() == rent.payer_balance_before_lamports()
            && principal
                .lamports()
                .checked_add(donation.lamports())
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                == payer_allocation.lamports(),
        ClutchError::MismatchedState,
    )?;
    let refund_after = refund_owner
        .lamports()
        .checked_add(principal.lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = neutral_sink
        .lamports()
        .checked_add(donation.lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let (root_body, position_body, pot_body) =
        encode_common_successors(authenticated_root, plan, pot_before)?;
    let mut terminal_body = std::vec![0u8; finalization.terminal_account_bytes()];
    finalization.terminal().encode(&mut terminal_body)?;

    // Preflight every data/lamport destination before the CPI or realloc.
    {
        let _root = borrow_mut_data(&accounts[IX_ROOT])?;
        let _row = borrow_mut_data(&accounts[suffix_at + REL_OWNER_ROW])?;
        let _pot = borrow_mut_data(&accounts[suffix_at + REL_CASH_POT])?;
        let _position = borrow_mut_data(&accounts[suffix_at + REL_POSITION])?;
        let _replay = borrow_mut_data(&accounts[suffix_at + REL_REPLAY])?;
        let _carry = borrow_mut_data(carry)?;
        let _payer_data = borrow_mut_data(payer_allocation)?;
        let _payer_lamports = payer_allocation
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let _refund_lamports = refund_owner
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let _sink_lamports = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    }

    transfer_from_signer(carry_payer, carry, system_program, top_up.lamports())?;
    require(carry.lamports() == rent.carry_balance_after_lamports(), ClutchError::MismatchedState)?;
    carry
        .resize(finalization.terminal_account_bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;

    {
        let mut root_out = borrow_mut_data(&accounts[IX_ROOT])?;
        let mut row_out = borrow_mut_data(&accounts[suffix_at + REL_OWNER_ROW])?;
        let mut pot_out = borrow_mut_data(&accounts[suffix_at + REL_CASH_POT])?;
        let mut position_out = borrow_mut_data(&accounts[suffix_at + REL_POSITION])?;
        let mut replay_out = borrow_mut_data(&accounts[suffix_at + REL_REPLAY])?;
        let mut carry_out = borrow_mut_data(carry)?;
        let mut payer_lamports = payer_allocation
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut refund_lamports = refund_owner
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_lamports = neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        root_out.copy_from_slice(&root_body);
        row_out.copy_from_slice(plan.realization().owner_settlement_poststate_body());
        pot_out.copy_from_slice(&pot_body);
        position_out.copy_from_slice(&position_body);
        replay_out.copy_from_slice(plan.replay().replay_poststate_body());
        carry_out.copy_from_slice(&terminal_body);
        **payer_lamports = 0;
        **refund_lamports = refund_after;
        **sink_lamports = sink_after;
    }
    payer_allocation
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    payer_allocation.assign(&SYSTEM_PROGRAM_ID);
    require(
        payer_allocation.data_len() == 0
            && payer_allocation.lamports() == 0
            && *payer_allocation.owner == SYSTEM_PROGRAM_ID
            && carry.data_len() == finalization.terminal_account_bytes(),
        ClutchError::MismatchedState,
    )
}

/// Decode and execute exactly one action-38 request.
pub fn process<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::FinalizeOwnerSettlement
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let request = match decode_owner_settlement_payload_v1(action.tag(), payload)? {
        OwnerSettlementPayloadV1::FinalizeOwnerSettlement(value) => value,
        OwnerSettlementPayloadV1::AccountReceiptEnd(_)
        | OwnerSettlementPayloadV1::FreezeEntitlement(_) => {
            return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction));
        }
    };
    let (page_count, fee_bearing) = action38_frame(accounts.len())?;
    let suffix_at = ACTION38_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(page_count)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require_all_distinct(accounts, suffix_at, fee_bearing)?;
    let traversal = authenticate_settlement_traversal_v5(
        program_id,
        SettlementTraversalAccountFrameV5 {
            retained_feed: &accounts[IX_FEED],
            market_binding: &accounts[IX_BINDING],
            market_runtime: &accounts[IX_RUNTIME],
            economic_domain: &accounts[IX_DOMAIN],
            price_grid: &accounts[IX_GRID],
            realm: &accounts[IX_REALM],
            profile: &accounts[IX_PROFILE],
            collateral_policy: &accounts[IX_POLICY],
            token_program: &accounts[IX_TOKEN],
            market_instance: &accounts[IX_MARKET_INSTANCE],
            market_genesis: &accounts[IX_GENESIS],
            pages: &accounts[ACTION38_TRAVERSAL_PREFIX_ACCOUNTS..suffix_at],
        },
    )?;
    let root_traversal = authenticate_writable_root_settlement_traversal_v5(
        program_id,
        &accounts[IX_ROOT],
        &traversal,
    )?;
    let root = root_traversal.root();
    let rent = read_rent(&accounts[suffix_at + REL_RENT])?;
    let owner_row_account = &accounts[suffix_at + REL_OWNER_ROW];
    require_program_state(
        program_id,
        owner_row_account,
        true,
        contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    )?;
    let row = OwnerSettlementV5AccountV1::decode(&borrow_data(owner_row_account)?)?;
    let owner = row.semantic.expectation().owner();
    let position_replay = authenticate_current_general_position_replay_v2(
        program_id,
        traversal.collateral(),
        &accounts[IX_BINDING],
        &accounts[IX_RUNTIME],
        &accounts[suffix_at + REL_POSITION],
        &accounts[suffix_at + REL_REPLAY],
        owner,
    )?;
    let cash_pot_account = &accounts[suffix_at + REL_CASH_POT];
    let cash_pot = authenticate_cash_pot(
        program_id,
        cash_pot_account,
        root.root(),
        rent.minimum_balance(contract::SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?,
    )?;
    require(
        request.epoch == root.root().epoch()
            && request.settlement_root == root.account()
            && request.owner_settlement == id(owner_row_account.key)
            && request.position == id(accounts[suffix_at + REL_POSITION].key)
            && request.settlement_cash_pot == id(cash_pot_account.key),
        ClutchError::MismatchedState,
    )?;

    let revenue: Option<RevenuePolicyV1> = if fee_bearing {
        let preimage = &accounts[suffix_at + REL_REVENUE_PREIMAGE];
        require(
            preimage.executable
                && !preimage.is_writable
                && !preimage.is_signer
                && preimage.data_len() == REVENUE_POLICY_BYTES,
            ClutchError::MismatchedState,
        )?;
        Some(
            decode_revenue_policy(&borrow_data(preimage)?)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        )
    } else {
        None
    };
    let fee_accounts = match revenue.as_ref() {
        Some(policy) => OwnerFeeAccountInputV5::CandidateFee {
            frame: OwnerFeeSnapshotAccountFrameV5 {
                owner_row: owner_row_account,
                selected_fee_record: &accounts[suffix_at + REL_SELECTED_FEE],
                owner_fee_carry: &accounts[suffix_at + REL_FEE_CARRY],
                payer_allocation: &accounts[suffix_at + REL_PAYER_ALLOCATION],
                batch_policy: &accounts[suffix_at + REL_BATCH_POLICY],
                revenue_policy_record: &accounts[suffix_at + REL_REVENUE_RECORD],
            },
            revenue_policy: policy,
        },
        None => OwnerFeeAccountInputV5::NoFeeRecord,
    };
    let fee_rent = if fee_bearing {
        OwnerFeeTerminalRentInputV5::CandidateFee {
            frame: OwnerFeeRentAccountFrameV5 {
                market_binding: &accounts[IX_BINDING],
                carry_rent_payer: &accounts[suffix_at + REL_CARRY_RENT_PAYER],
                payer_rent_refund_owner: &accounts[suffix_at + REL_PAYER_REFUND_OWNER],
                neutral_sink: &accounts[suffix_at + REL_NEUTRAL_SINK],
            },
            carry_terminal_rent_minimum_lamports: rent
                .minimum_balance(contract::OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4)?,
        }
    } else {
        OwnerFeeTerminalRentInputV5::NoFeeRecord
    };
    let plan = Box::new(compose_owner_settlement_action38_v5(
        program_id,
        root,
        traversal.traversal(),
        owner_row_account,
        rent.minimum_balance(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?,
        fee_accounts,
        fee_rent,
        id(cash_pot_account.key),
        position_replay.replay,
        cash_pot.semantic,
    )?);
    require(
        plan.realization().owner_settlement_account() == id(owner_row_account.key)
            && plan.realization().settlement_cash_pot_account() == id(cash_pot_account.key)
            && plan.realization().position().account
                == accounts[suffix_at + REL_POSITION].key.to_bytes()
            && plan.replay().replay_account() == id(accounts[suffix_at + REL_REPLAY].key)
            && plan.fee_finalization_required() == fee_bearing,
        ClutchError::MismatchedState,
    )?;

    if fee_bearing {
        write_fee_bundle(accounts, suffix_at, root, &plan, cash_pot)
    } else {
        write_zero_fee_bundle(accounts, suffix_at, root, &plan, cash_pot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action38_frames_are_exact_and_branch_disjoint() {
        for pages in 1..=clutch_solana_layout::MAX_ORDER_PAGES {
            let no_fee = ACTION38_TRAVERSAL_PREFIX_ACCOUNTS + pages
                + ACTION38_ZERO_FEE_SUFFIX_ACCOUNTS;
            let fee = ACTION38_TRAVERSAL_PREFIX_ACCOUNTS + pages
                + ACTION38_FEE_SUFFIX_ACCOUNTS;
            assert_eq!(action38_frame(no_fee), Ok((pages, false)));
            assert_eq!(action38_frame(fee), Ok((pages, true)));
        }
        assert!(action38_frame(ACTION38_TRAVERSAL_PREFIX_ACCOUNTS).is_err());
    }

    #[test]
    fn fee_suffix_names_every_mutation_and_system_authority() {
        assert_eq!(REL_RENT, 0);
        assert_eq!(REL_REPLAY, 4);
        assert_eq!(REL_SELECTED_FEE, 5);
        assert_eq!(REL_SYSTEM_PROGRAM + 1, ACTION38_FEE_SUFFIX_ACCOUNTS);
        assert!(ACTION38_FEE_SUFFIX_ACCOUNTS > ACTION38_ZERO_FEE_SUFFIX_ACCOUNTS);
    }
}
