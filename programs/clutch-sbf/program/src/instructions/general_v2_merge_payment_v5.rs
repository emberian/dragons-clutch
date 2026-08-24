//! Staged-disabled SBF composition for General V2 action 40 merge payment.
//!
//! The exact account order is:
//! 0 writable SettlementRoot, 1 retained Feed, 2 writable ReceiptV5,
//! 3 MarketBindingV2, 4 MarketRuntimeV3, 5 Realm, 6 ProfileV2,
//! 7 collateral policy, 8 Token-2022 program, 9 MarketInstanceV2 artifact,
//! 10 MarketGenesisProfileV2 artifact, 11 Rent sysvar, 12 cash pot,
//! 13 finalized OwnerSettlementV5, 14 frozen OrderPageV5,
//! 15 writable ReservationV9, 16 read-only PositionV3, 17 writable GEN1
//! ReplayV3, and 18 read-only OwnerFeeFinalizationV4 only for the fee-bearing
//! branch. The zero-fee branch has exactly 18 accounts.
//!
//! This action moves no cash and performs no CPI or close. It authenticates
//! every prestate, prepares one indivisible pure plan, pre-borrows all four
//! writable accounts, then writes the root/receipt/reservation/replay bundle.
//! The module is intentionally absent from dispatch while action 40 remains
//! centrally `ReservedDisabled`.

use core::cell::{Ref, RefMut};
use std::boxed::Box;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_settlement_root_payload_v1, GeneralOrderPageSeedTupleV5,
    GeneralReservationSeedTupleV9, Id32, OwnerFeeFinalizationV4AccountV1,
    OwnerSettlementSeedTupleV5, OwnerSettlementV5AccountV1, SettlementCashPotV1AccountV1,
    SettlementRootPayloadV1, OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4,
    OWNER_SETTLEMENT_ACCOUNT_BYTES_V5, SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
};
use clutch_general_v2_runtime::{
    prepare_finalize_merge_receipt_payment_v5, project_owner_settlement_account_v5_readonly,
    FinalizeMergeReceiptPaymentInputV5, FinalizeMergeReceiptPaymentPlanV5,
    MergePaymentFeeFinalizationInputV4, MergePaymentFinalizationSourceV5,
    MergeReceiptPaymentEndpointInputV5, OwnerSettlementAccountProjectionV5,
    OwnerSettlementAccountViewV5, PositionAccountInputV3,
};
use clutch_retirement::PositionPurposeV3;
use clutch_solana_layout::order_page_v5::{verify_page_v5, ORDER_PAGE_V5_BYTES};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::settlement_receipt_v5::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, require_count, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::read_rent;
use crate::seeds;

use super::collateral_position_v3::GeneralPositionReplayAuthorityV4;
use super::general_v2_direct_v5::authenticate_market_collateral_v2;
use super::general_v2_position_replay::authenticate_current_general_position_replay_readonly_v4;
use super::general_v2_receipt_v5::{
    authenticate_general_receipt_v5_writable_root, AuthenticatedGeneralReceiptV5,
    RECEIPT_V5_AUTH_ACCOUNT_COUNT,
};
use super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1;

/// Exact zero-fee action-40 account count.
pub const ZERO_FEE_ACCOUNT_COUNT: usize = 18;
/// Exact fee-bearing action-40 account count.
pub const FEE_BEARING_ACCOUNT_COUNT: usize = 19;

/// Writable counted SettlementRoot.
pub const IX_ROOT: usize = 0;
/// Retained sealed Feed.
pub const IX_FEED: usize = 1;
/// Writable rent-owned ReceiptV5.
pub const IX_RECEIPT: usize = 2;
/// Immutable MarketBindingV2.
pub const IX_MARKET_BINDING: usize = 3;
/// Immutable MarketRuntimeV3.
pub const IX_MARKET_RUNTIME: usize = 4;
/// Realm account.
pub const IX_REALM: usize = 5;
/// Realm-selected collateral profile.
pub const IX_PROFILE: usize = 6;
/// Realm-selected collateral policy.
pub const IX_COLLATERAL_POLICY: usize = 7;
/// Canonical Token-2022 program.
pub const IX_TOKEN_PROGRAM: usize = 8;
/// MarketInstanceV2 immutable artifact.
pub const IX_MARKET_INSTANCE: usize = 9;
/// MarketGenesisProfileV2 immutable artifact.
pub const IX_MARKET_GENESIS: usize = 10;
/// Rent sysvar used only to authenticate the finalized row's rent floor.
pub const IX_RENT_SYSVAR: usize = 11;
/// Read-only candidate cash pot.
pub const IX_CASH_POT: usize = 12;
/// Read-only finalized OwnerSettlementV5 row.
pub const IX_OWNER_ROW: usize = 13;
/// Read-only frozen OrderPageV5.
pub const IX_ORDER_PAGE: usize = 14;
/// Writable seller ReservationV9.
pub const IX_RESERVATION: usize = 15;
/// Read-only seller PositionV3.
pub const IX_POSITION: usize = 16;
/// Writable purpose-owned GEN1 ReplayV3.
pub const IX_REPLAY: usize = 17;
/// Optional read-only durable OwnerFeeFinalizationV4.
pub const IX_FEE_FINALIZATION: usize = 18;

#[derive(Debug)]
struct SellerData<'a> {
    owner_row: Ref<'a, [u8]>,
    order_page: Ref<'a, [u8]>,
    reservation: Ref<'a, [u8]>,
    position: Ref<'a, [u8]>,
    replay: Ref<'a, [u8]>,
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedSellerV5 {
    owner: Id32,
    owner_row: OwnerSettlementAccountProjectionV5,
    replay: GeneralPositionReplayAuthorityV4,
    replay_bump: u8,
}

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn borrow_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn borrow_mut_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<RefMut<'a, [u8]>> {
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
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
}

fn require_exact_account_count(accounts: &[AccountInfo<'_>], expected: usize) -> Outcome<()> {
    require_count(accounts, expected)
}

fn require_disjoint_accounts(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        require(!accounts[left].is_signer, ClutchError::MismatchedState)?;
        let mut right = left + 1;
        while right < accounts.len() {
            require(
                accounts[left].key != accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

#[inline(never)]
fn authenticate_cash_pot_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    receipt: &AuthenticatedGeneralReceiptV5,
) -> Outcome<SettlementCashPotV1AccountV1> {
    let account = &accounts[IX_CASH_POT];
    require_program_state(
        program_id,
        account,
        false,
        SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
    )?;
    let data = borrow_data(account)?;
    let pot = SettlementCashPotV1AccountV1::decode(&data)?;
    let root = receipt.root();
    let canonical = seeds::general_v2_settlement_cash_pot_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
    );
    expect_pda(account.key, canonical, Some(pot.stored_bump))?;
    require(
        id(account.key) == root.settlement_cash_pot() && pot.stored_bump == root.cash_pot_bump(),
        ClutchError::MismatchedState,
    )?;
    Ok(pot)
}

#[inline(never)]
fn authenticate_seller_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    receipt: &AuthenticatedGeneralReceiptV5,
    bound: clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    owner_row_rent_minimum: u64,
    data: &SellerData<'_>,
) -> Outcome<AuthenticatedSellerV5> {
    require_program_state(
        program_id,
        &accounts[IX_OWNER_ROW],
        false,
        OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    )?;
    let row = OwnerSettlementV5AccountV1::decode(&data.owner_row)?;
    let owner = Id32::new(row.semantic.expectation().owner())?;
    let root = receipt.root();
    let row_seed =
        OwnerSettlementSeedTupleV5::new(root.epoch(), root.settlement_candidate_id(), owner)?;
    let row_pda = seeds::general_v2_owner_settlement_v5_pda(
        program_id,
        row_seed.epoch(),
        row_seed.settlement_candidate(),
        row_seed.owner(),
    );
    expect_pda(
        accounts[IX_OWNER_ROW].key,
        row_pda,
        Some(row.stored_bump),
    )?;
    let owner_row = project_owner_settlement_account_v5_readonly(
        OwnerSettlementAccountViewV5 {
            account: id(accounts[IX_OWNER_ROW].key),
            program_owner: id(accounts[IX_OWNER_ROW].owner),
            exact_body: &data.owner_row,
            lamports: accounts[IX_OWNER_ROW].lamports(),
            rent_minimum: owner_row_rent_minimum,
            canonical_bump: row_pda.1,
            writable: false,
        },
        id(program_id),
        row_seed,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require_program_state(
        program_id,
        &accounts[IX_ORDER_PAGE],
        false,
        ORDER_PAGE_V5_BYTES,
    )?;
    let page = verify_page_v5(&data.order_page)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let page_seed = GeneralOrderPageSeedTupleV5::new(root.epoch(), page.page_index)?;
    let page_pda = seeds::general_v2_order_page_v5_pda(
        program_id,
        page_seed.epoch(),
        u16::from_le_bytes(*page_seed.page_index_le()),
    );
    expect_pda(
        accounts[IX_ORDER_PAGE].key,
        page_pda,
        Some(page.stored_bump),
    )?;
    require(
        page.frozen == 1
            && page.market.0 == root.market().bytes()
            && page.epoch.0 == root.epoch().bytes()
            && page.order_set.0 == root.order_set().bytes(),
        ClutchError::MismatchedState,
    )?;

    require_program_state(
        program_id,
        &accounts[IX_RESERVATION],
        true,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    let reservation = ReservationAccountV9::decode(&data.reservation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let reservation_id = Id32::new(reservation.body().reservation.bytes())?;
    let reservation_seed = GeneralReservationSeedTupleV9::new(reservation_id)?;
    let reservation_pda =
        seeds::general_v2_reservation_v9_pda(program_id, reservation_seed.reservation_id());
    expect_pda(
        accounts[IX_RESERVATION].key,
        reservation_pda,
        Some(reservation.body().stored_bump),
    )?;

    let replay = authenticate_current_general_position_replay_readonly_v4(
        program_id,
        bound,
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_POSITION],
        &accounts[IX_REPLAY],
        owner.bytes(),
    )?;
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &accounts[IX_POSITION].key.to_bytes(),
        PositionPurposeV3::General,
        &accounts[IX_MARKET_RUNTIME].key.to_bytes(),
    );
    Ok(AuthenticatedSellerV5 {
        owner,
        owner_row,
        replay,
        replay_bump: replay_pda.1,
    })
}

#[inline(never)]
fn authenticate_fee_finalization_v4<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    receipt: &AuthenticatedGeneralReceiptV5,
    owner: Id32,
    exact_body: &'a [u8],
) -> Outcome<MergePaymentFeeFinalizationInputV4<'a>> {
    let account = &accounts[IX_FEE_FINALIZATION];
    require_program_state(
        program_id,
        account,
        false,
        OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4,
    )?;
    let terminal = OwnerFeeFinalizationV4AccountV1::decode(exact_body)?;
    let canonical = seeds::general_v2_owner_fee_carry_pda(
        program_id,
        &receipt.root().fee_record().bytes(),
        &owner.bytes(),
    );
    expect_pda(account.key, canonical, Some(terminal.stored_bump))?;
    Ok(MergePaymentFeeFinalizationInputV4 {
        account: id(account.key),
        exact_body,
    })
}

#[inline(never)]
fn prepare_plan_boxed(
    input: FinalizeMergeReceiptPaymentInputV5<'_>,
) -> Outcome<Box<FinalizeMergeReceiptPaymentPlanV5>> {
    prepare_finalize_merge_receipt_payment_v5(input)
        .map(Box::new)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

#[inline(never)]
fn write_atomic_bundle(
    accounts: &[AccountInfo<'_>],
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    plan: &FinalizeMergeReceiptPaymentPlanV5,
) -> Outcome<()> {
    require(
        plan.settlement_root_account() == id(accounts[IX_ROOT].key)
            && plan.receipt_account() == id(accounts[IX_RECEIPT].key)
            && plan.reservation_account() == id(accounts[IX_RESERVATION].key)
            && plan.position_account() == id(accounts[IX_POSITION].key)
            && plan.replay().replay_account() == id(accounts[IX_REPLAY].key)
            && plan.owner_settlement_account() == id(accounts[IX_OWNER_ROW].key)
            && plan.order_page_account() == id(accounts[IX_ORDER_PAGE].key)
            && plan.settlement_cash_pot_account() == id(accounts[IX_CASH_POT].key),
        ClutchError::MismatchedState,
    )?;

    let mut root_body = std::vec![0u8; authenticated_root.account_bytes()];
    authenticated_root.encode_merge_payment_successor(
        plan.settlement_root_poststate(),
        &mut root_body,
    )?;
    require(
        accounts[IX_ROOT].data_len() == authenticated_root.account_bytes()
            && accounts[IX_RECEIPT].data_len() == SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5
            && accounts[IX_RESERVATION].data_len() == RESERVATION_ACCOUNT_BYTES_V9
            && accounts[IX_REPLAY].data_len() == contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        ClutchError::WrongDataLength,
    )?;

    // Borrow every mutable destination before the first byte is changed. A
    // borrow failure therefore cannot leave a partial in-instruction image;
    // the SVM transaction rollback remains the outer atomicity boundary.
    let mut root_out = borrow_mut_data(&accounts[IX_ROOT])?;
    let mut receipt_out = borrow_mut_data(&accounts[IX_RECEIPT])?;
    let mut reservation_out = borrow_mut_data(&accounts[IX_RESERVATION])?;
    let mut replay_out = borrow_mut_data(&accounts[IX_REPLAY])?;
    root_out.copy_from_slice(&root_body);
    receipt_out.copy_from_slice(plan.receipt_poststate_body());
    reservation_out.copy_from_slice(plan.reservation_poststate_body());
    replay_out.copy_from_slice(plan.replay().replay_poststate_body());
    Ok(())
}

/// Decode and compose one staged-disabled action-40 request.
///
/// The central capability gate remains false and dispatch owns no action-40
/// arm. This function is complete so enabling the producer chain later does
/// not require inventing a second account or write contract.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::FinalizeMergeReceiptPayment
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let request = match decode_settlement_root_payload_v1(action.tag(), payload)? {
        SettlementRootPayloadV1::FinalizeMergeReceiptPayment(request) => request,
        SettlementRootPayloadV1::InitializeSettlementRoot(_)
        | SettlementRootPayloadV1::ReleaseUnfilledReservation(_) => {
            return Err(ClutchError::UnsupportedInstruction.into())
        }
    };
    let fee_bearing = request.stable_zero_fee_finalization_evidence_id.is_zero();
    require_exact_account_count(
        accounts,
        if fee_bearing {
            FEE_BEARING_ACCOUNT_COUNT
        } else {
            ZERO_FEE_ACCOUNT_COUNT
        },
    )?;
    require_disjoint_accounts(accounts)?;

    let receipt = Box::new(authenticate_general_receipt_v5_writable_root(
        program_id,
        &accounts[..RECEIPT_V5_AUTH_ACCOUNT_COUNT],
    )?);
    require(
        request.epoch == receipt.root().epoch() && request.receipt == receipt.receipt_account(),
        ClutchError::MismatchedState,
    )?;
    let (bound, market_v2) = authenticate_market_collateral_v2(program_id, accounts, &receipt)?;
    let rent = read_rent(&accounts[IX_RENT_SYSVAR])?;
    let owner_row_rent_minimum = rent.minimum_balance(OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;
    let cash_pot = authenticate_cash_pot_v1(program_id, accounts, &receipt)?;

    let seller_data = SellerData {
        owner_row: borrow_data(&accounts[IX_OWNER_ROW])?,
        order_page: borrow_data(&accounts[IX_ORDER_PAGE])?,
        reservation: borrow_data(&accounts[IX_RESERVATION])?,
        position: borrow_data(&accounts[IX_POSITION])?,
        replay: borrow_data(&accounts[IX_REPLAY])?,
    };
    let seller = Box::new(authenticate_seller_v5(
        program_id,
        accounts,
        &receipt,
        bound,
        owner_row_rent_minimum,
        &seller_data,
    )?);
    let fee_data = if fee_bearing {
        Some(borrow_data(&accounts[IX_FEE_FINALIZATION])?)
    } else {
        None
    };
    let fee_finalization = match fee_data.as_ref() {
        Some(data) => Some(authenticate_fee_finalization_v4(
            program_id,
            accounts,
            &receipt,
            seller.owner,
            data,
        )?),
        None => None,
    };
    let feed_data = borrow_data(&accounts[IX_FEED])?;
    let relation_market = market_v2.relation_projection();
    let plan = prepare_plan_boxed(FinalizeMergeReceiptPaymentInputV5 {
        payload: request,
        settlement_root_account: receipt.settlement_root_account(),
        settlement_root: receipt.root(),
        retained_feed_account: receipt.retained_feed_account(),
        retained_feed_body: &feed_data,
        receipt: receipt.receipt(),
        receipt_evidence: receipt.evidence(),
        market_binding_account: id(accounts[IX_MARKET_BINDING].key),
        market_binding: &relation_market,
        collateral: bound,
        settlement_cash_pot_account: id(accounts[IX_CASH_POT].key),
        settlement_cash_pot: cash_pot,
        fee_finalization,
        seller: MergeReceiptPaymentEndpointInputV5 {
            owner_row: seller.owner_row,
            order_page_account: id(accounts[IX_ORDER_PAGE].key),
            order_page_body: &seller_data.order_page,
            reservation_account: id(accounts[IX_RESERVATION].key),
            reservation_body: &seller_data.reservation,
            position: PositionAccountInputV3 {
                account: id(accounts[IX_POSITION].key),
                encoded_body: &seller_data.position,
            },
            replay_account: id(accounts[IX_REPLAY].key),
            replay_bump: seller.replay_bump,
            replay_next_sequence: seller.replay.replay.next_sequence(),
            replay_body: &seller_data.replay,
        },
    })?;
    if fee_bearing {
        require(
            request.stable_zero_fee_finalization_evidence_id.is_zero()
                && plan.finalization_source()
                    == MergePaymentFinalizationSourceV5::FeeFinalizationV4
                && plan.finalization_source_data_id().is_some()
                && plan.settlement_cash_pot_poststate_data_id().is_some(),
            ClutchError::MismatchedState,
        )?;
    } else {
        require(
            plan.finalization_source() == MergePaymentFinalizationSourceV5::ZeroFeeReplay
                && plan.stable_owner_finalization_evidence_id()
                    == request.stable_zero_fee_finalization_evidence_id,
            ClutchError::MismatchedState,
        )?;
    }

    drop(feed_data);
    drop(fee_data);
    drop(seller_data);
    write_atomic_bundle(accounts, receipt.root_authority(), &plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action40_account_frames_are_exact_and_branch_disjoint() {
        let common = [
            IX_ROOT,
            IX_FEED,
            IX_RECEIPT,
            IX_MARKET_BINDING,
            IX_MARKET_RUNTIME,
            IX_REALM,
            IX_PROFILE,
            IX_COLLATERAL_POLICY,
            IX_TOKEN_PROGRAM,
            IX_MARKET_INSTANCE,
            IX_MARKET_GENESIS,
            IX_RENT_SYSVAR,
            IX_CASH_POT,
            IX_OWNER_ROW,
            IX_ORDER_PAGE,
            IX_RESERVATION,
            IX_POSITION,
            IX_REPLAY,
        ];
        assert_eq!(common.len(), ZERO_FEE_ACCOUNT_COUNT);
        for (expected, observed) in common.into_iter().enumerate() {
            assert_eq!(expected, observed);
        }
        assert_eq!(IX_FEE_FINALIZATION + 1, FEE_BEARING_ACCOUNT_COUNT);
        assert_eq!(RECEIPT_V5_AUTH_ACCOUNT_COUNT, 3);
    }
}
