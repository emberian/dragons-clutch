//! General V2 actions 36/37 virtual complete-set consumption.
//!
//! Both routes authenticate the counted root through the exhaustive retained
//! Feed/Page traversal, then compose the existing pure virtual inventory and
//! real-end delivery state machines. Complete-set conversion is an internal
//! Hoard liability reclassification: no collateral token moves, and the exact
//! Realm-selected mint, Hoard token account, token deployment ProgramData,
//! Hoard, and ClaimLedger are nevertheless authenticated before either
//! liability owner changes. Action 37 creates the merge cash pot only on the
//! unique terminal inventory transition and advances the version-aware root
//! writer in that same instruction.

use core::cell::{Ref, RefMut};
use std::boxed::Box;

use clutch_collateral_adapter_v2::{
    admit_collateral_account_v2, admit_collateral_mint_v2, ClaimLedgerV3, HoardV2,
    Id as CollateralId, RuntimeAccountViewV2, TokenAccountRoleV2, CLAIM_LEDGER_V3_BYTES,
    HOARD_V2_BYTES,
};
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_virtual_settlement_payload_v1, FinalPotSettlementRootBindingV1,
    FinalPotV1AccountV1, GeneralReservationSeedTupleV9, Id32, OwnerSettlementSeedTupleV5,
    OwnerSettlementV5AccountV1, SettlementCashPotV1AccountV1, VirtualSettlementPayloadV1,
    FINAL_POT_ACCOUNT_BYTES, GENERAL_REPLAY_ACCOUNT_V1_BYTES,
    OWNER_SETTLEMENT_ACCOUNT_BYTES_V5, SETTLEMENT_CASH_POT_ACCOUNT_BYTES,
};
use clutch_general_v2_runtime::{
    project_owner_settlement_account_v5, project_owner_settlement_account_v5_readonly,
    OwnerSettlementAccountProjectionV5, OwnerSettlementAccountViewV5,
    SettlementTraversalAccessV5,
};
use clutch_owner_settlement::{
    prepare_virtual_merge_composite_v1, prepare_virtual_split_composite_v1,
    AuthenticatedMarketClaimLedgerV1, AuthenticatedOrderMembershipV1,
    AuthenticatedOwnerSettlementAccountV1, AuthenticatedReservationV1,
    AuthenticatedVirtualMergeReceiptV1, AuthenticatedVirtualReceiptAuthorityV1,
    AuthenticatedVirtualSplitReceiptV1, OrderKindV1, OwnerSettlementAccumulatorV1,
    OwnerSettlementExpectationV1, OwnerSettlementStateV4, ReservationStateV1,
    SettlementSideV1, VirtualMergeCashPotPostV1, VirtualMergeCompositeInputV1,
    VirtualMergeCompositePlanV1, VirtualMergeReceiptInputV1, VirtualReceiptKindV1,
    VirtualSplitCompositeInputV1, VirtualSplitCompositePlanV1, VirtualSplitReceiptInputV1,
};
use clutch_retirement::POSITION_V3_BYTES;
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation::{
    ReservationPlan, RESERVATION_STATE_CONSUMED, RESERVATION_STATE_ENTITLED,
};
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::settlement_receipt_v4::{
    RECEIPT_ACCOUNTED_BUY_END, RECEIPT_ACCOUNTED_SELL_END,
};
use clutch_solana_layout::settlement_receipt_v5::{
    SettlementReceiptAccountV5, SettlementReceiptTransitionCommitmentV5,
    SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
};
use clutch_solana_layout::{
    OrderSlot, MAX_ORDER_PAGES, ORDER_KIND_PORTFOLIO, ORDER_KIND_SINGLE,
    RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_MERGE, RECEIPT_LEG_SPLIT,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID;

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::read_rent;
use crate::seeds;

use super::collateral_position_v3::{
    authenticate_general_market_value_authority_v2, RuntimeSha256,
};
use super::general_v2_account_receipt_v5::{locate_order_slot, route_and_order};
use super::general_v2_position_replay::authenticate_current_general_position_replay_v2;
use super::general_v2_receipt_v5::{
    authenticate_general_receipt_v5_root_traversal, AuthenticatedGeneralReceiptV5,
};
use super::general_v2_settlement_producer_v5::{create_from_payer, rent_owner};
use super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1;
use super::general_v2_settlement_traversal_v5::{
    authenticate_readonly_root_settlement_traversal_v5,
    authenticate_settlement_traversal_v5, authenticate_writable_root_settlement_traversal_v5,
    SettlementTraversalAccountFrameV5,
};

pub const VIRTUAL_TRAVERSAL_PREFIX_ACCOUNTS: usize = 12;
pub const ACTION36_SUFFIX_ACCOUNTS: usize = 13;
pub const ACTION37_SUFFIX_ACCOUNTS: usize = 15;

const IX_ROOT: usize = 0;
const IX_FEED: usize = 1;
const IX_MARKET_BINDING: usize = 2;
const IX_MARKET_RUNTIME: usize = 3;
const IX_ECONOMIC_DOMAIN: usize = 4;
const IX_PRICE_GRID: usize = 5;
const IX_REALM: usize = 6;
const IX_PROFILE: usize = 7;
const IX_COLLATERAL_POLICY: usize = 8;
const IX_TOKEN_PROGRAM: usize = 9;
const IX_MARKET_INSTANCE: usize = 10;
const IX_MARKET_GENESIS: usize = 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VirtualActionV5 {
    Split,
    Merge,
}

#[derive(Clone, Copy, Debug)]
struct VirtualFrameV5<'a, 'info> {
    receipt: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
    owner_row: &'a AccountInfo<'info>,
    reservation: &'a AccountInfo<'info>,
    position: &'a AccountInfo<'info>,
    replay: &'a AccountInfo<'info>,
    final_pot: &'a AccountInfo<'info>,
    token_programdata: &'a AccountInfo<'info>,
    hoard: &'a AccountInfo<'info>,
    claim_ledger: &'a AccountInfo<'info>,
    collateral_mint: &'a AccountInfo<'info>,
    hoard_token: &'a AccountInfo<'info>,
    cash_pot: &'a AccountInfo<'info>,
    payer: Option<&'a AccountInfo<'info>>,
    system_program: Option<&'a AccountInfo<'info>>,
}

#[derive(Debug)]
struct EndpointData<'a> {
    owner_row: Ref<'a, [u8]>,
    reservation: Ref<'a, [u8]>,
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedEndpointV5 {
    owner: Id32,
    row: OwnerSettlementAccountProjectionV5,
    row_envelope: OwnerSettlementV5AccountV1,
    reservation: ReservationAccountV9,
    membership: clutch_owner_settlement::AuthenticatedOrderMembershipV2,
    legacy_order: AuthenticatedOrderMembershipV1,
    legacy_reservation: AuthenticatedReservationV1,
    replay: super::collateral_position_v3::GeneralPositionReplayAuthorityV2,
}

#[derive(Clone, Copy, Debug)]
struct LiabilitySuccessorsV5 {
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
}

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

fn runtime_view<'a>(account: &AccountInfo<'_>, data: &'a [u8]) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: CollateralId::from_bytes(account.key.to_bytes()),
        owner_program: CollateralId::from_bytes(account.owner.to_bytes()),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

fn require_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable && !account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable { ClutchError::NotWritable } else { ClutchError::UnexpectedWritable },
    )?;
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
}

fn require_distinct(accounts: &[AccountInfo<'_>], payer_index: Option<usize>) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        require(
            accounts[left].is_signer == (payer_index == Some(left)),
            ClutchError::MismatchedState,
        )?;
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn page_count(total: usize, action: VirtualActionV5) -> Result<usize, ClutchError> {
    let suffix = match action {
        VirtualActionV5::Split => ACTION36_SUFFIX_ACCOUNTS,
        VirtualActionV5::Merge => ACTION37_SUFFIX_ACCOUNTS,
    };
    let fixed = VIRTUAL_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(suffix)
        .ok_or(ClutchError::Arithmetic)?;
    let pages = total.checked_sub(fixed).ok_or(ClutchError::WrongAccountCount)?;
    if (1..=MAX_ORDER_PAGES).contains(&pages) {
        Ok(pages)
    } else {
        Err(ClutchError::WrongAccountCount)
    }
}

fn frame_at<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    suffix_at: usize,
    action: VirtualActionV5,
) -> VirtualFrameV5<'a, 'info> {
    VirtualFrameV5 {
        receipt: &accounts[suffix_at],
        rent_sysvar: &accounts[suffix_at + 1],
        owner_row: &accounts[suffix_at + 2],
        reservation: &accounts[suffix_at + 3],
        position: &accounts[suffix_at + 4],
        replay: &accounts[suffix_at + 5],
        final_pot: &accounts[suffix_at + 6],
        token_programdata: &accounts[suffix_at + 7],
        hoard: &accounts[suffix_at + 8],
        claim_ledger: &accounts[suffix_at + 9],
        collateral_mint: &accounts[suffix_at + 10],
        hoard_token: &accounts[suffix_at + 11],
        cash_pot: &accounts[suffix_at + 12],
        payer: (action == VirtualActionV5::Merge).then_some(&accounts[suffix_at + 13]),
        system_program: (action == VirtualActionV5::Merge)
            .then_some(&accounts[suffix_at + 14]),
    }
}

fn legacy_owner_row(
    row: OwnerSettlementV5AccountV1,
    projection: OwnerSettlementAccountProjectionV5,
    finalized: bool,
) -> Outcome<AuthenticatedOwnerSettlementAccountV1> {
    let semantic = row.semantic;
    let expectation = semantic.expectation();
    let buy = expectation.expected_buy_price_units();
    let sell = expectation.expected_sell_price_units();
    let accumulator = OwnerSettlementAccumulatorV1 {
        expectation: OwnerSettlementExpectationV1 {
            market: expectation.market(),
            epoch: expectation.epoch(),
            candidate: expectation.candidate(),
            owner: expectation.owner(),
            owner_order_set_digest: expectation.owner_order_set_digest(),
            price_scale: expectation.price_scale(),
            expected_buy_order_mask: expectation.expected_buy_order_mask(),
            expected_sell_order_mask: expectation.expected_sell_order_mask(),
            expected_slice_count: expectation.expected_slice_count(),
            expected_buy_price_units: buy.value,
            expected_sell_price_units: sell.value,
            expected_buy_price_units_present: buy.present,
            expected_sell_price_units_present: sell.present,
            selected_fee_atoms: expectation.selected_fee_atoms(),
            reserved_cash_atoms: semantic.buy_cash_handoff_atoms(),
        },
        consumed_buy_price_units: semantic.consumed_buy_price_units().value,
        consumed_sell_price_units: semantic.consumed_sell_price_units().value,
        completed_buy_order_mask: semantic.completed_buy_order_mask(),
        completed_sell_order_mask: semantic.completed_sell_order_mask(),
        consumed_slice_count: expectation.expected_slice_count(),
        state: u8::from(finalized),
    };
    accumulator
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedOwnerSettlementAccountV1 {
        address: projection.account().bytes(),
        program_id: projection.program_owner().bytes(),
        lamports: projection.lamports(),
        rent_minimum: projection.rent_minimum(),
        accumulator,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_endpoint(
    program_id: &Pubkey,
    root: &AuthenticatedGeneralSettlementRootV1,
    traversal: &dyn SettlementTraversalAccessV5,
    traversal_frame: SettlementTraversalAccountFrameV5<'_, '_>,
    bound: clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    pages: &[AccountInfo<'_>],
    frame: VirtualFrameV5<'_, '_>,
    action: VirtualActionV5,
    owner_row_rent: u64,
    reservation_rent: u64,
    data: &EndpointData<'_>,
    receipt: SettlementReceiptAccountV5,
) -> Outcome<AuthenticatedEndpointV5> {
    let row_envelope = OwnerSettlementV5AccountV1::decode(&data.owner_row)?;
    let owner = Id32::new(row_envelope.semantic.expectation().owner())?;
    let row_seed = OwnerSettlementSeedTupleV5::new(
        root.root().epoch(),
        root.root().settlement_candidate_id(),
        owner,
    )?;
    let row_pda = seeds::general_v2_owner_settlement_v5_pda(
        program_id,
        row_seed.epoch(),
        row_seed.settlement_candidate(),
        row_seed.owner(),
    );
    expect_pda(frame.owner_row.key, row_pda, Some(row_envelope.stored_bump))?;
    let row_view = OwnerSettlementAccountViewV5 {
        account: id(frame.owner_row.key),
        program_owner: id(frame.owner_row.owner),
        exact_body: &data.owner_row,
        lamports: frame.owner_row.lamports(),
        rent_minimum: owner_row_rent,
        canonical_bump: row_pda.1,
        writable: frame.owner_row.is_writable,
    };
    let row = match action {
        VirtualActionV5::Split => project_owner_settlement_account_v5_readonly(
            row_view,
            id(program_id),
            row_seed,
        ),
        VirtualActionV5::Merge => project_owner_settlement_account_v5(
            row_view,
            id(program_id),
            row_seed,
        ),
    }
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        matches!(
            (action, row_envelope.semantic.state()),
            (VirtualActionV5::Split, OwnerSettlementStateV4::Finalized)
                | (VirtualActionV5::Merge, OwnerSettlementStateV4::AccountingComplete)
        ),
        ClutchError::MismatchedState,
    )?;

    let (side, route, order_index, _) = route_and_order(traversal, &receipt, owner)?;
    require(
        matches!(
            (action, side, route),
            (
                VirtualActionV5::Split,
                SettlementSideV1::Buy,
                clutch_owner_settlement::SettlementReceiptRouteV4::SplitToBuy
            ) | (
                VirtualActionV5::Merge,
                SettlementSideV1::Sell,
                clutch_owner_settlement::SettlementReceiptRouteV4::SellToMerge
            )
        ),
        ClutchError::MismatchedState,
    )?;
    let membership = traversal
        .settlement_membership(order_index)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    membership
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let (slot, page_index, position_generation) = locate_order_slot(pages, membership.order_id)?;
    let single_outcome = match slot {
        OrderSlot::Single(order) => order.outcome,
        OrderSlot::Portfolio(_) => u8::MAX,
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            return Err(Refusal::Adapter(ClutchError::MismatchedState))
        }
    };
    require(
        position_generation == membership.position_generation
            && slot.generation() == membership.order_generation
            && membership.owner == owner.bytes(),
        ClutchError::MismatchedState,
    )?;

    let reservation_envelope = ReservationAccountV9::decode(&data.reservation)?;
    let reservation = reservation_envelope.body();
    let reservation_id = Id32::new(reservation.reservation.bytes())?;
    let reservation_seed = GeneralReservationSeedTupleV9::new(reservation_id)?;
    let reservation_pda =
        seeds::general_v2_reservation_v9_pda(program_id, reservation_seed.reservation_id());
    expect_pda(frame.reservation.key, reservation_pda, Some(reservation.stored_bump))?;
    let expected_side = match action { VirtualActionV5::Split => 0, VirtualActionV5::Merge => 1 };
    let expected_kind = match membership.order_kind {
        OrderKindV1::Single => ORDER_KIND_SINGLE,
        OrderKindV1::Portfolio => ORDER_KIND_PORTFOLIO,
    };
    let reservation_plan = ReservationPlan::for_order(
        &slot,
        traversal.projection().feed().outcome_count,
        traversal.projection().feed().price_scale,
        reservation.max_fee_atoms,
    )?;
    let required_rent = reservation_envelope
        .rent()
        .refundable_principal
        .checked_add(reservation_envelope.rent().donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        frame.reservation.lamports() >= reservation_rent
            && frame.reservation.lamports() >= required_rent
            && reservation.reservation.bytes() == membership.reservation
            && reservation.market.bytes() == root.root().market().bytes()
            && reservation.epoch.bytes() == root.root().epoch().bytes()
            && reservation.owner.bytes() == owner.bytes()
            && reservation.order_id.bytes() == membership.order_id
            && reservation.position_generation == membership.position_generation
            && reservation.order_generation == membership.order_generation
            && reservation.page_index == page_index
            && reservation.outcome_count == root.root().outcome_count()
            && reservation.side == expected_side
            && reservation.order_kind == expected_kind
            && reservation.state == RESERVATION_STATE_ENTITLED
            && reservation.entitled_units == membership.entitled_units
            && (match action {
                VirtualActionV5::Split => reservation.paid_units == reservation.consumed_units,
                VirtualActionV5::Merge => reservation.paid_units == 0,
            })
            && reservation.initial_cash_atoms == reservation_plan.cash_atoms
            && reservation.max_fee_atoms == reservation_plan.max_fee_atoms
            && reservation.initial_internal == reservation_plan.internal,
        ClutchError::MismatchedState,
    )?;

    let replay = authenticate_current_general_position_replay_v2(
        program_id,
        bound,
        traversal_frame.market_binding,
        traversal_frame.market_runtime,
        frame.position,
        frame.replay,
        owner.bytes(),
    )?;
    require(
        replay.position.semantic.fields().generation == membership.position_generation,
        ClutchError::MismatchedState,
    )?;
    let entitled = membership.entitled_consideration_price_units;
    let legacy_order = AuthenticatedOrderMembershipV1 {
        market: membership.market,
        epoch: membership.epoch,
        candidate: membership.candidate,
        owner_order_set_digest: membership.owner_order_set_digest,
        order_id: membership.order_id,
        reservation: membership.reservation,
        owner: membership.owner,
        order_index: membership.order_index,
        order_generation: membership.order_generation,
        position_generation: membership.position_generation,
        side: membership.side,
        order_kind: membership.order_kind,
        outcome_count: membership.outcome_count,
        single_outcome,
        entitled_units: membership.entitled_units,
        entitled_consideration_price_units: entitled.value,
        entitled_consideration_present: entitled.present,
    };
    let legacy_reservation = AuthenticatedReservationV1 {
        account: frame.reservation.key.to_bytes(),
        reservation: reservation.reservation.bytes(),
        market: reservation.market.bytes(),
        epoch: reservation.epoch.bytes(),
        owner: reservation.owner.bytes(),
        order_id: reservation.order_id.bytes(),
        position: frame.position.key.to_bytes(),
        position_generation: reservation.position_generation,
        order_generation: reservation.order_generation,
        outcome_count: reservation.outcome_count,
        order_kind: membership.order_kind,
        side,
        state: ReservationStateV1::Entitled,
        initial_cash_atoms: reservation.initial_cash_atoms,
        remaining_cash_atoms: reservation.remaining_cash_atoms,
        initial_internal: reservation.initial_internal,
        remaining_internal: reservation.remaining_internal,
        entitled_units: reservation.entitled_units,
        consumed_units: reservation.consumed_units,
        accounted_units: reservation.entitled_units,
        entitled_consideration_price_units: entitled.value,
        accounted_consideration_price_units: entitled.value,
        entitled_consideration_present: entitled.present,
        accounted_consideration_present: entitled.present,
        writable: true,
    };
    Ok(AuthenticatedEndpointV5 {
        owner,
        row,
        row_envelope,
        reservation: reservation_envelope,
        membership,
        legacy_order,
        legacy_reservation,
        replay,
    })
}

fn market_ledger(
    authority: super::collateral_position_v3::GeneralMarketValueAuthorityV2,
    ledger_account: Id32,
    hoard_account: Id32,
    market: Id32,
) -> Outcome<AuthenticatedMarketClaimLedgerV1> {
    let mut total = [0u64; clutch_owner_settlement::MAX_OUTCOMES];
    let mut at = 0usize;
    while at < usize::from(authority.liabilities.claim_ledger.outcome_count) {
        total[at] = authority.liabilities.claim_ledger.aggregate_internal_supply[at]
            .checked_add(authority.liabilities.claim_ledger.aggregate_materialized_supply[at])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        at += 1;
    }
    Ok(AuthenticatedMarketClaimLedgerV1 {
        ledger: ledger_account.bytes(),
        hoard: hoard_account.bytes(),
        market: market.bytes(),
        hoard_collateral_atoms: authority.liabilities.hoard.locked_claim_principal_atoms,
        internal_supply: authority.liabilities.claim_ledger.aggregate_internal_supply,
        total_supply: total,
        outcome_count: authority.liabilities.claim_ledger.outcome_count,
        market_phase: 0,
        writable: true,
    })
}

fn liability_successors(
    authority: super::collateral_position_v3::GeneralMarketValueAuthorityV2,
    post: AuthenticatedMarketClaimLedgerV1,
) -> Outcome<LiabilitySuccessorsV5> {
    let mut hoard = authority.liabilities.hoard;
    let mut claim = authority.liabilities.claim_ledger;
    let before_total = hoard.required_custody_atoms()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    hoard.locked_claim_principal_atoms = post.hoard_collateral_atoms;
    hoard.cash_liability_atoms = before_total
        .checked_sub(post.hoard_collateral_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    claim.aggregate_internal_supply = post.internal_supply;
    require(hoard.required_custody_atoms().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))? == before_total, ClutchError::MismatchedState)?;
    let mut at = 0usize;
    while at < usize::from(claim.outcome_count) {
        require(
            claim.aggregate_internal_supply[at]
                .checked_add(claim.aggregate_materialized_supply[at])
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                == post.total_supply[at],
            ClutchError::MismatchedState,
        )?;
        at += 1;
    }
    Ok(LiabilitySuccessorsV5 { hoard, claim_ledger: claim })
}

/// Decode and execute one action 36 or 37 request.
pub fn process<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    let route = match action {
        GeneralV2Action::ConsumeVirtualSplitReceiptEggs => VirtualActionV5::Split,
        GeneralV2Action::ConsumeVirtualMergeReceiptEggs => VirtualActionV5::Merge,
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };
    require(
        capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let request = decode_virtual_settlement_payload_v1(action.tag(), payload)?;
    consume_virtual(program_id, accounts, route, request)
}

#[inline(never)]
fn consume_virtual<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    action: VirtualActionV5,
    request: VirtualSettlementPayloadV1,
) -> Outcome<()> {
    let pages_len = page_count(accounts.len(), action)?;
    let suffix_at = VIRTUAL_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(pages_len)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let suffix = match action { VirtualActionV5::Split => ACTION36_SUFFIX_ACCOUNTS, VirtualActionV5::Merge => ACTION37_SUFFIX_ACCOUNTS };
    require(accounts.len() == suffix_at + suffix, ClutchError::WrongAccountCount)?;
    let payer_index = (action == VirtualActionV5::Merge).then_some(suffix_at + 13);
    require_distinct(accounts, payer_index)?;
    let pages = &accounts[VIRTUAL_TRAVERSAL_PREFIX_ACCOUNTS..suffix_at];
    let frame = frame_at(accounts, suffix_at, action);
    require_program_state(program_id, frame.receipt, true, SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5)?;
    require_program_state(program_id, frame.owner_row, action == VirtualActionV5::Merge, OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;
    require_program_state(program_id, frame.reservation, true, RESERVATION_ACCOUNT_BYTES_V9)?;
    require_program_state(program_id, frame.position, true, POSITION_V3_BYTES)?;
    require_program_state(program_id, frame.replay, true, GENERAL_REPLAY_ACCOUNT_V1_BYTES)?;
    require_program_state(program_id, frame.final_pot, true, FINAL_POT_ACCOUNT_BYTES)?;
    require_program_state(program_id, frame.hoard, true, HOARD_V2_BYTES)?;
    require_program_state(program_id, frame.claim_ledger, true, CLAIM_LEDGER_V3_BYTES)?;
    if action == VirtualActionV5::Split {
        require_program_state(program_id, frame.cash_pot, true, SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?;
    } else {
        let payer = frame.payer.ok_or(Refusal::Adapter(ClutchError::WrongAccountCount))?;
        let system = frame.system_program.ok_or(Refusal::Adapter(ClutchError::WrongAccountCount))?;
        require_signer(payer)?;
        require(*system.key == SYSTEM_PROGRAM_ID && system.executable, ClutchError::MismatchedState)?;
        require(frame.cash_pot.owner == &SYSTEM_PROGRAM_ID && frame.cash_pot.data_len() == 0 && frame.cash_pot.is_writable && !frame.cash_pot.is_signer, ClutchError::MismatchedState)?;
    }

    let traversal_frame = SettlementTraversalAccountFrameV5 {
        retained_feed: &accounts[IX_FEED], market_binding: &accounts[IX_MARKET_BINDING],
        market_runtime: &accounts[IX_MARKET_RUNTIME], economic_domain: &accounts[IX_ECONOMIC_DOMAIN],
        price_grid: &accounts[IX_PRICE_GRID], realm: &accounts[IX_REALM],
        profile: &accounts[IX_PROFILE], collateral_policy: &accounts[IX_COLLATERAL_POLICY],
        token_program: &accounts[IX_TOKEN_PROGRAM], market_instance: &accounts[IX_MARKET_INSTANCE],
        market_genesis: &accounts[IX_MARKET_GENESIS], pages,
    };
    let traversal_auth = authenticate_settlement_traversal_v5(program_id, traversal_frame)?;
    let root_traversal = match action {
        VirtualActionV5::Split => authenticate_readonly_root_settlement_traversal_v5(program_id, &accounts[IX_ROOT], &traversal_auth)?,
        VirtualActionV5::Merge => authenticate_writable_root_settlement_traversal_v5(program_id, &accounts[IX_ROOT], &traversal_auth)?,
    };
    let traversal_access = traversal_auth.traversal();
    let bound = traversal_auth.collateral();
    let receipt = authenticate_general_receipt_v5_root_traversal(
        program_id,
        root_traversal,
        frame.receipt,
    )?;
    compose_and_apply(
        program_id,
        accounts,
        action,
        request,
        receipt,
        traversal_access,
        bound,
        traversal_frame,
        pages,
        frame,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn compose_and_apply<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    action: VirtualActionV5,
    request: VirtualSettlementPayloadV1,
    receipt: AuthenticatedGeneralReceiptV5,
    traversal_access: &dyn SettlementTraversalAccessV5,
    bound: clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    traversal_frame: SettlementTraversalAccountFrameV5<'info, 'info>,
    pages: &'info [AccountInfo<'info>],
    frame: VirtualFrameV5<'info, 'info>,
) -> Outcome<()> {
    let root = receipt.root();
    let request_epoch = match request {
        VirtualSettlementPayloadV1::ConsumeVirtualSplitReceiptEggs(value) => {
            require(action == VirtualActionV5::Split && value.receipt == receipt.receipt_account(), ClutchError::MismatchedState)?;
            value.epoch
        }
        VirtualSettlementPayloadV1::ConsumeVirtualMergeReceiptEggs(value) => {
            require(action == VirtualActionV5::Merge && value.receipt == receipt.receipt_account(), ClutchError::MismatchedState)?;
            value.epoch
        }
    };
    require(request_epoch == root.epoch(), ClutchError::MismatchedState)?;
    let cash_pda = seeds::general_v2_settlement_cash_pot_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
    );
    expect_pda(frame.cash_pot.key, cash_pda, Some(root.cash_pot_bump()))?;
    require(
        id(frame.cash_pot.key) == root.settlement_cash_pot(),
        ClutchError::MismatchedState,
    )?;
    let semantic = receipt.receipt().semantic();
    require(
        receipt.receipt().transition() == SettlementReceiptTransitionCommitmentV5::None
            && semantic.accounted_end_mask == semantic.expected_end_mask()
            && semantic.delivered_end_mask() == 0
            && semantic.settled_quantity == 0
            && matches!((action, semantic.leg_kind), (VirtualActionV5::Split, RECEIPT_LEG_SPLIT) | (VirtualActionV5::Merge, RECEIPT_LEG_MERGE))
            && matches!((action, root.virtual_cash_direction()), (VirtualActionV5::Split, clutch_owner_settlement::VirtualCashDirectionV1::Split) | (VirtualActionV5::Merge, clutch_owner_settlement::VirtualCashDirectionV1::Merge))
            && matches!((action, root.cash_pot_state()), (VirtualActionV5::Split, contract::SettlementRootChildStateV1::Live) | (VirtualActionV5::Merge, contract::SettlementRootChildStateV1::ExpectedUncreated)),
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(frame.rent_sysvar)?;
    let row_rent = rent.minimum_balance(OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;
    let reservation_rent = rent.minimum_balance(RESERVATION_ACCOUNT_BYTES_V9)?;
    let final_rent = rent.minimum_balance(FINAL_POT_ACCOUNT_BYTES)?;
    let receipt_rent = rent.minimum_balance(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5)?;
    let recorded_receipt_rent = receipt
        .receipt()
        .rent()
        .refundable_principal
        .checked_add(receipt.receipt().rent().donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        frame.receipt.lamports() >= receipt_rent
            && frame.receipt.lamports() >= recorded_receipt_rent,
        ClutchError::MismatchedState,
    )?;
    let endpoint_data = EndpointData { owner_row: borrow_data(frame.owner_row)?, reservation: borrow_data(frame.reservation)? };

    let endpoint = authenticate_endpoint(
        program_id, receipt.root_authority(), traversal_access,
        traversal_frame, bound, pages, frame, action, row_rent, reservation_rent,
        &endpoint_data, receipt.receipt(),
    )?;
    let replay = endpoint.replay;
    let row_legacy = legacy_owner_row(endpoint.row_envelope, endpoint.row, action == VirtualActionV5::Split)?;

    let final_seed = contract::FinalPotSeedTupleV1::new(root.epoch(), root.settlement_candidate_id())?;
    let final_pda = seeds::find(program_id, &[final_seed.domain(), final_seed.epoch(), final_seed.settlement_candidate()]);
    expect_pda(frame.final_pot.key, final_pda, Some(root.final_pot_bump()))?;
    let final_required = root.final_pot_rent()?.ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    require(frame.final_pot.lamports() >= final_rent && frame.final_pot.lamports() >= final_required.refundable_principal.checked_add(final_required.donation_floor).ok_or(Refusal::Adapter(ClutchError::Arithmetic))?, ClutchError::MismatchedState)?;
    let final_binding = FinalPotSettlementRootBindingV1 {
        root, final_pot: id(frame.final_pot.key), derived_bump: final_pda.1,
        program_owner_authenticated: frame.final_pot.owner == program_id, writable: true,
    };
    let final_data = borrow_data(frame.final_pot)?;
    let final_pot = FinalPotV1AccountV1::decode_against_settlement_root(&final_data, final_binding)?;

    let value_authority = authenticate_general_market_value_authority_v2(
        program_id, traversal_frame.realm, traversal_frame.profile, traversal_frame.collateral_policy,
        traversal_frame.token_program, frame.token_programdata, traversal_frame.market_binding,
        traversal_frame.market_runtime, traversal_frame.market_instance, frame.hoard,
        frame.claim_ledger, true, true,
    )?;
    require(value_authority.liabilities.bound == bound, ClutchError::MismatchedState)?;
    let mint_data = borrow_data(frame.collateral_mint)?;
    let hoard_token_data = borrow_data(frame.hoard_token)?;
    admit_collateral_mint_v2(bound, runtime_view(frame.collateral_mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let visible = admit_collateral_account_v2(bound, runtime_view(frame.hoard_token, &hoard_token_data), TokenAccountRoleV2::Hoard)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(visible.amount_atoms >= value_authority.liabilities.hoard.required_custody_atoms().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?, ClutchError::MismatchedState)?;
    let ledger = market_ledger(
        value_authority,
        id(frame.claim_ledger.key),
        id(frame.hoard.key),
        root.market(),
    )?;

    let evidence = receipt.evidence();
    let real_order = match action { VirtualActionV5::Split => semantic.buy_order_id.bytes(), VirtualActionV5::Merge => semantic.sell_order_id.bytes() };
    let kind = match action { VirtualActionV5::Split => VirtualReceiptKindV1::Split, VirtualActionV5::Merge => VirtualReceiptKindV1::Merge };
    let authority = AuthenticatedVirtualReceiptAuthorityV1 {
        account: receipt.settlement_root_account().bytes(),
        relation_witness_digest: root.settlement_witness_digest().bytes(),
        market: root.market().bytes(), epoch: root.epoch().bytes(), candidate: root.settlement_candidate_id().bytes(),
        owner_order_set_digest: root.owner_order_set_digest().bytes(), receipt: receipt.receipt_account().bytes(),
        receipt_accounting_id: evidence.receipt_accounting_id().bytes(), delivery_transition_id: evidence.delivery_transition_id().bytes(),
        real_order_id: real_order, kind, outcome: semantic.outcome, quantity: semantic.quantity,
        consideration_price_units: semantic.consideration_price_units, consideration_present: true,
        slice_index: semantic.slice_index, verifier_authorized: true,
    };
    let replay_kind = match action { VirtualActionV5::Split => contract::GeneralReplayTransitionKindV1::VirtualSplitBuyer, VirtualActionV5::Merge => contract::GeneralReplayTransitionKindV1::VirtualMergeSeller };

    // Every decoded value above is owned. Release all hostile-byte borrows
    // before either branch begins its mutable-borrow preflight or System CPI.
    drop(hoard_token_data);
    drop(mint_data);
    drop(final_data);
    drop(endpoint_data);

    let result = match action {
        VirtualActionV5::Split => {
            require(root.counts().completed_owner_finalizations == root.counts().expected_owner_rows, ClutchError::MismatchedState)?;
            let cash_data = borrow_data(frame.cash_pot)?;
            let cash_outer = SettlementCashPotV1AccountV1::decode(&cash_data)?;
            expect_pda(frame.cash_pot.key, cash_pda, Some(cash_outer.stored_bump))?;
            let recorded_cash_rent = root
                .cash_pot_rent()
                .refundable_principal
                .checked_add(root.cash_pot_rent().donation_floor)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            require(
                cash_outer.stored_bump == root.cash_pot_bump()
                    && cash_outer.semantic.expectation == root.cash_pot_expectation()?
                    && frame.cash_pot.lamports()
                        >= rent.minimum_balance(SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?
                    && frame.cash_pot.lamports() >= recorded_cash_rent,
                ClutchError::MismatchedState,
            )?;
            let receipt_input = AuthenticatedVirtualSplitReceiptV1 {
                receipt: receipt.receipt_account().bytes(), receipt_accounting_id: evidence.receipt_accounting_id().bytes(),
                delivery_transition_id: evidence.delivery_transition_id().bytes(), market: root.market().bytes(), epoch: root.epoch().bytes(),
                candidate: root.settlement_candidate_id().bytes(), owner_order_set_digest: root.owner_order_set_digest().bytes(),
                buy_order_id: semantic.buy_order_id.bytes(), outcome: semantic.outcome, quantity: semantic.quantity,
                price: semantic.price, price_present: true, consideration_price_units: semantic.consideration_price_units,
                consideration_present: true, slice_index: semantic.slice_index, sequence: semantic.sequence,
                settled_quantity: semantic.settled_quantity, accounted_end_mask: semantic.accounted_end_mask,
                delivered_end_mask: semantic.delivered_end_mask(), expected_end_mask: semantic.expected_end_mask(),
            };
            let plan = Box::new(prepare_virtual_split_composite_v1(VirtualSplitCompositeInputV1 {
                receipt: VirtualSplitReceiptInputV1 { authority, receipt: receipt_input, order: endpoint.legacy_order, position: replay.position, reservation: endpoint.legacy_reservation, owner_row: row_legacy, final_pot: final_pot.semantic },
                market_ledger: ledger, cash_pot: cash_outer.semantic,
            }).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?);
            drop(cash_data);
            apply_split(frame, receipt.root_authority(), endpoint, final_binding, final_pot, value_authority, &plan, replay_kind, evidence.receipt_data_id().bytes())?;
            Ok(())
        }
        VirtualActionV5::Merge => {
            let receipt_input = AuthenticatedVirtualMergeReceiptV1 {
                receipt: receipt.receipt_account().bytes(), receipt_accounting_id: evidence.receipt_accounting_id().bytes(),
                delivery_transition_id: evidence.delivery_transition_id().bytes(), market: root.market().bytes(), epoch: root.epoch().bytes(),
                candidate: root.settlement_candidate_id().bytes(), owner_order_set_digest: root.owner_order_set_digest().bytes(),
                sell_order_id: semantic.sell_order_id.bytes(), outcome: semantic.outcome, quantity: semantic.quantity,
                price: semantic.price, price_present: true, consideration_price_units: semantic.consideration_price_units,
                consideration_present: true, slice_index: semantic.slice_index, sequence: semantic.sequence,
                settled_quantity: semantic.settled_quantity, accounted_end_mask: semantic.accounted_end_mask,
                delivered_end_mask: semantic.delivered_end_mask(), expected_end_mask: semantic.expected_end_mask(),
            };
            let plan = Box::new(prepare_virtual_merge_composite_v1(VirtualMergeCompositeInputV1 {
                receipt: VirtualMergeReceiptInputV1 { authority, receipt: receipt_input, order: endpoint.legacy_order, position: replay.position, reservation: endpoint.legacy_reservation, owner_row: row_legacy, final_pot: final_pot.semantic },
                market_ledger: ledger, cash_expectation: root.cash_pot_expectation()?,
            }).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?);
            apply_merge(program_id, accounts, frame, receipt.root_authority(), endpoint, final_binding, final_pot, value_authority, &plan, replay_kind, evidence.receipt_data_id().bytes(), &rent)?;
            Ok(())
        }
    };
    result
}

#[allow(clippy::too_many_arguments)]
fn common_poststates(
    frame: VirtualFrameV5<'_, '_>,
    endpoint: AuthenticatedEndpointV5,
    final_binding: FinalPotSettlementRootBindingV1<'_>,
    final_outer: FinalPotV1AccountV1,
    value_authority: super::collateral_position_v3::GeneralMarketValueAuthorityV2,
    receipt_delivery_mask: u8,
    exhaust_receipt: bool,
    position: clutch_owner_settlement::PositionSettlementPoststateV3,
    reservation: AuthenticatedReservationV1,
    final_semantic: clutch_owner_settlement::AuthenticatedFinalPotV1,
    ledger: AuthenticatedMarketClaimLedgerV1,
    replay_kind: contract::GeneralReplayTransitionKindV1,
    delivery_id: [u8; 32],
    receipt_data_id: [u8; 32],
) -> Outcome<([u8; SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5], [u8; RESERVATION_ACCOUNT_BYTES_V9], [u8; POSITION_V3_BYTES], [u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES], [u8; FINAL_POT_ACCOUNT_BYTES], [u8; HOARD_V2_BYTES], [u8; CLAIM_LEDGER_V3_BYTES])> {
    let mut receipt_semantic = SettlementReceiptAccountV5::decode(&borrow_data(frame.receipt)?)?.semantic();
    receipt_semantic.settled_quantity = receipt_semantic.quantity;
    receipt_semantic.consumed_flags = receipt_delivery_mask | if exhaust_receipt { RECEIPT_FLAG_SLICE_EXHAUSTED } else { 0 };
    let receipt_post = SettlementReceiptAccountV5::new(receipt_semantic, SettlementReceiptTransitionCommitmentV5::None, SettlementReceiptAccountV5::decode(&borrow_data(frame.receipt)?)?.rent())?;
    let mut reservation_body = endpoint.reservation.body();
    reservation_body.remaining_cash_atoms = reservation.remaining_cash_atoms;
    reservation_body.remaining_internal = reservation.remaining_internal;
    reservation_body.consumed_units = reservation.consumed_units;
    if receipt_delivery_mask == RECEIPT_FLAG_BUY_CONSUMED { reservation_body.paid_units = reservation.consumed_units; }
    reservation_body.state = if receipt_delivery_mask == RECEIPT_FLAG_BUY_CONSUMED
        && reservation.state == ReservationStateV1::Consumed
    {
        RESERVATION_STATE_CONSUMED
    } else {
        RESERVATION_STATE_ENTITLED
    };
    let reservation_post = ReservationAccountV9::new(reservation_body, endpoint.reservation.rent())?;
    let mut reservation_bytes = [0u8; RESERVATION_ACCOUNT_BYTES_V9]; reservation_post.encode(&mut reservation_bytes)?;
    let position_bytes = position.semantic.encode()?;
    let replay = contract::project_general_replay_transition_v1(endpoint.replay.replay, position, replay_kind, Id32::new(delivery_id)?, Id32::new(receipt_data_id)?, &RuntimeSha256)?;
    let mut final_bytes = [0u8; FINAL_POT_ACCOUNT_BYTES];
    FinalPotV1AccountV1 { semantic: final_semantic, ..final_outer }.encode_against_settlement_root(final_binding, &mut final_bytes)?;
    let liabilities = liability_successors(value_authority, ledger)?;
    let mut hoard_bytes = [0u8; HOARD_V2_BYTES]; liabilities.hoard.encode(&mut hoard_bytes).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut claim_bytes = [0u8; CLAIM_LEDGER_V3_BYTES]; liabilities.claim_ledger.encode(&mut claim_bytes).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((receipt_post.encode_exact()?, reservation_bytes, position_bytes, *replay.replay_poststate_body(), final_bytes, hoard_bytes, claim_bytes))
}

#[allow(clippy::too_many_arguments)]
fn apply_split(
    frame: VirtualFrameV5<'_, '_>,
    _root: &AuthenticatedGeneralSettlementRootV1,
    endpoint: AuthenticatedEndpointV5,
    final_binding: FinalPotSettlementRootBindingV1<'_>,
    final_outer: FinalPotV1AccountV1,
    value_authority: super::collateral_position_v3::GeneralMarketValueAuthorityV2,
    plan: &VirtualSplitCompositePlanV1,
    replay_kind: contract::GeneralReplayTransitionKindV1,
    receipt_data_id: [u8; 32],
) -> Outcome<()> {
    let bodies = common_poststates(frame, endpoint, final_binding, final_outer, value_authority,
        RECEIPT_FLAG_BUY_CONSUMED, true, plan.receipt.position, plan.receipt.reservation,
        plan.receipt.final_pot, plan.market_ledger, replay_kind, plan.delivery_transition_id, receipt_data_id)?;
    let cash_outer = SettlementCashPotV1AccountV1::decode(&borrow_data(frame.cash_pot)?)?;
    let cash_bytes = SettlementCashPotV1AccountV1 { semantic: plan.cash_pot, ..cash_outer };
    let mut cash_body = [0u8; SETTLEMENT_CASH_POT_ACCOUNT_BYTES]; cash_bytes.encode(&mut cash_body)?;
    let mut receipt_out = borrow_mut_data(frame.receipt)?; let mut reservation_out = borrow_mut_data(frame.reservation)?;
    let mut position_out = borrow_mut_data(frame.position)?; let mut replay_out = borrow_mut_data(frame.replay)?;
    let mut final_out = borrow_mut_data(frame.final_pot)?; let mut hoard_out = borrow_mut_data(frame.hoard)?;
    let mut claim_out = borrow_mut_data(frame.claim_ledger)?; let mut cash_out = borrow_mut_data(frame.cash_pot)?;
    receipt_out.copy_from_slice(&bodies.0); reservation_out.copy_from_slice(&bodies.1); position_out.copy_from_slice(&bodies.2);
    replay_out.copy_from_slice(&bodies.3); final_out.copy_from_slice(&bodies.4); hoard_out.copy_from_slice(&bodies.5);
    claim_out.copy_from_slice(&bodies.6); cash_out.copy_from_slice(&cash_body); Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_merge<'info>(
    program_id: &Pubkey,
    accounts: &'info [AccountInfo<'info>],
    frame: VirtualFrameV5<'info, 'info>,
    root: &AuthenticatedGeneralSettlementRootV1,
    endpoint: AuthenticatedEndpointV5,
    final_binding: FinalPotSettlementRootBindingV1<'_>,
    final_outer: FinalPotV1AccountV1,
    value_authority: super::collateral_position_v3::GeneralMarketValueAuthorityV2,
    plan: &VirtualMergeCompositePlanV1,
    replay_kind: contract::GeneralReplayTransitionKindV1,
    receipt_data_id: [u8; 32],
    rent: &crate::instructions::genesis::RentParameters,
) -> Outcome<()> {
    let bodies = common_poststates(frame, endpoint, final_binding, final_outer, value_authority,
        RECEIPT_FLAG_SELL_CONSUMED, false, plan.receipt.position, plan.receipt.reservation,
        plan.receipt.final_pot, plan.market_ledger, replay_kind, plan.delivery_transition_id, receipt_data_id)?;
    let mut row_post = endpoint.row_envelope; row_post.semantic.record_merge_delivery().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let row_bytes = row_post.encode_exact()?;
    let funded = match plan.cash_pot { VirtualMergeCashPotPostV1::AwaitingCompleteSet => None, VirtualMergeCashPotPostV1::Funded(value) => Some(value) };
    let mut root_bytes = None; let mut cash_bytes = None;
    if let Some(cash) = funded {
        let activation = contract::prepare_activate_merge_cash_pot_v1(root.root())?;
        require(activation.cash_pot() == cash && activation.cash_pot_account() == id(frame.cash_pot.key), ClutchError::MismatchedState)?;
        let mut bytes = std::vec![0u8; root.account_bytes()]; root.encode_merge_cash_activation_successor(activation.root(), &mut bytes)?;
        let outer = SettlementCashPotV1AccountV1 { semantic: cash, stored_bump: activation.stored_bump(), flags: 0 };
        let mut pot = [0u8; SETTLEMENT_CASH_POT_ACCOUNT_BYTES]; outer.encode(&mut pot)?;
        root_bytes = Some(bytes); cash_bytes = Some(pot);
        for destination in [frame.receipt, frame.owner_row, frame.reservation, frame.position, frame.replay, frame.final_pot, frame.hoard, frame.claim_ledger, &accounts[IX_ROOT]] { drop(borrow_mut_data(destination)?); }
        let payer = frame.payer.ok_or(Refusal::Adapter(ClutchError::WrongAccountCount))?;
        let system = frame.system_program.ok_or(Refusal::Adapter(ClutchError::WrongAccountCount))?;
        let owner = rent_owner(payer, frame.cash_pot, rent, SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?;
        require(owner == root.root().cash_pot_rent(), ClutchError::MismatchedState)?;
        let bump = [activation.stored_bump()];
        create_from_payer(program_id, payer, frame.cash_pot, system, rent, SETTLEMENT_CASH_POT_ACCOUNT_BYTES, owner,
            &[seeds::SEED_GENERAL_V2_SETTLEMENT_CASH_POT, &root.root().epoch().bytes(), &root.root().settlement_candidate_id().bytes(), &bump])?;
    }
    let mut receipt_out = borrow_mut_data(frame.receipt)?; let mut row_out = borrow_mut_data(frame.owner_row)?;
    let mut reservation_out = borrow_mut_data(frame.reservation)?; let mut position_out = borrow_mut_data(frame.position)?;
    let mut replay_out = borrow_mut_data(frame.replay)?; let mut final_out = borrow_mut_data(frame.final_pot)?;
    let mut hoard_out = borrow_mut_data(frame.hoard)?; let mut claim_out = borrow_mut_data(frame.claim_ledger)?;
    let mut root_out = if funded.is_some() { Some(borrow_mut_data(&accounts[IX_ROOT])?) } else { None };
    let mut cash_out = if funded.is_some() { Some(borrow_mut_data(frame.cash_pot)?) } else { None };
    receipt_out.copy_from_slice(&bodies.0); row_out.copy_from_slice(&row_bytes); reservation_out.copy_from_slice(&bodies.1);
    position_out.copy_from_slice(&bodies.2); replay_out.copy_from_slice(&bodies.3); final_out.copy_from_slice(&bodies.4);
    hoard_out.copy_from_slice(&bodies.5); claim_out.copy_from_slice(&bodies.6);
    if let (Some(root_body), Some(pot_body), Some(root_dest), Some(pot_dest)) = (root_bytes.as_ref(), cash_bytes.as_ref(), root_out.as_mut(), cash_out.as_mut()) { root_dest.copy_from_slice(root_body); pot_dest.copy_from_slice(pot_body); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_frames_have_disjoint_branch_suffixes() {
        assert_eq!(VIRTUAL_TRAVERSAL_PREFIX_ACCOUNTS, 12);
        assert_eq!(ACTION36_SUFFIX_ACCOUNTS, 13);
        assert_eq!(ACTION37_SUFFIX_ACCOUNTS, 15);
        assert_eq!(page_count(12 + 1 + 13, VirtualActionV5::Split), Ok(1));
        assert_eq!(page_count(12 + 4 + 15, VirtualActionV5::Merge), Ok(4));
    }

    #[test]
    fn virtual_frame_delimiter_refuses_zero_and_fifth_pages() {
        assert_eq!(page_count(12 + 13, VirtualActionV5::Split), Err(ClutchError::WrongAccountCount));
        assert_eq!(page_count(12 + 5 + 15, VirtualActionV5::Merge), Err(ClutchError::WrongAccountCount));
    }

    #[test]
    fn zero_selected_price_is_presence_explicit() {
        let value = AuthenticatedOrderMembershipV1 {
            market: [1; 32], epoch: [2; 32], candidate: [3; 32], owner_order_set_digest: [4; 32],
            order_id: [5; 32], reservation: [6; 32], owner: [7; 32], order_index: 0,
            order_generation: 1, position_generation: 1, side: SettlementSideV1::Buy,
            order_kind: OrderKindV1::Single, outcome_count: 2, single_outcome: 0,
            entitled_units: 1, entitled_consideration_price_units: 0,
            entitled_consideration_present: true,
        };
        assert_eq!(value.entitled_consideration_price_units, 0);
        assert!(value.entitled_consideration_present);
    }
}
