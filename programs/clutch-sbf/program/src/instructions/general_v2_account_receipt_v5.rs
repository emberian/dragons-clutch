//! General V2 action 25: account one real V5 receipt end.
//!
//! The immutable traversal owns the selected slice, order membership, owner,
//! price, and completion bit. The mutable V5 Receipt, OwnerSettlement row, and
//! Reservation are decoded and compared before any byte changes. A terminal
//! buy transfers the Reservation's exact remaining cash ownership into the
//! owner row; no Position cash or Egg balance moves in this action.

use core::cell::{Ref, RefMut};

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_owner_settlement_payload_v1, GeneralReservationSeedTupleV9, Id32,
    OwnerSettlementPayloadV1, OwnerSettlementSeedTupleV5, OwnerSettlementV5AccountV1,
};
use clutch_general_v2_runtime::{
    project_owner_settlement_account_v5, OwnerSettlementAccountViewV5, SettlementLegV1,
    SettlementRouteV1, SettlementTraversalAccessV5,
};
use clutch_owner_settlement::{
    AuthenticatedReservationHandoffV3, AuthenticatedSettlementReceiptEndV5, OrderKindV1,
    PresentConsiderationV2, SettlementReceiptDataIdV5, SettlementReceiptRouteV4,
    SettlementSideV1,
};
use clutch_retirement::{
    project_general_position_v3, AdapterPositionPurposeBindingV3, Identity32V1,
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, POSITION_V3_BYTES,
};
use clutch_solana_layout::order_page_v5::{verify_page_v5, OrderSlotCursorV5};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation::{ReservationPlan, RESERVATION_STATE_ENTITLED};
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::settlement_receipt_v4::{
    RECEIPT_ACCOUNTED_BUY_END, RECEIPT_ACCOUNTED_SELL_END,
};
use clutch_solana_layout::settlement_receipt_v5::{
    SettlementReceiptAccountV5, SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
};
use clutch_solana_layout::{
    OrderSlot, MAX_ORDER_PAGES, ORDER_KIND_PORTFOLIO, ORDER_KIND_SINGLE, RECEIPT_LEG_DIRECT,
    RECEIPT_LEG_MERGE, RECEIPT_LEG_SPLIT,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::read_rent;
use crate::seeds;

use super::general_v2_receipt_v5::authenticate_general_receipt_v5_accounting_root_traversal;
use super::general_v2_settlement_traversal_v5::{
    authenticate_readonly_root_settlement_traversal_v5, authenticate_settlement_traversal_v5,
    SettlementTraversalAccountFrameV5,
};

/// Fixed shared traversal roles before the one-to-four PageV5 suffix.
pub const ACTION25_TRAVERSAL_PREFIX_ACCOUNTS: usize = 12;
/// Receipt, Rent, OwnerSettlementV5, ReservationV9, and read-only PositionV3.
pub const ACTION25_SUFFIX_ACCOUNTS: usize = 5;

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

fn require_all_distinct(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn page_count_from_account_len(total: usize) -> Result<usize, ClutchError> {
    let fixed = ACTION25_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(ACTION25_SUFFIX_ACCOUNTS)
        .ok_or(ClutchError::Arithmetic)?;
    let pages = total.checked_sub(fixed).ok_or(ClutchError::WrongAccountCount)?;
    if (1..=MAX_ORDER_PAGES).contains(&pages) {
        Ok(pages)
    } else {
        Err(ClutchError::WrongAccountCount)
    }
}

pub(crate) fn route_and_order(
    traversal: &dyn SettlementTraversalAccessV5,
    receipt: &SettlementReceiptAccountV5,
    owner: Id32,
) -> Outcome<(SettlementSideV1, SettlementReceiptRouteV4, u8, bool)> {
    let semantic = receipt.semantic();
    let slice = traversal
        .settlement_slice(semantic.slice_index)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_route = match semantic.leg_kind {
        RECEIPT_LEG_DIRECT => SettlementReceiptRouteV4::Direct,
        RECEIPT_LEG_SPLIT => SettlementReceiptRouteV4::SplitToBuy,
        RECEIPT_LEG_MERGE => SettlementReceiptRouteV4::SellToMerge,
        _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };
    require(
        matches!(
            (slice.route(), expected_route),
            (SettlementRouteV1::Direct, SettlementReceiptRouteV4::Direct)
                | (SettlementRouteV1::SplitToBuy, SettlementReceiptRouteV4::SplitToBuy)
                | (SettlementRouteV1::SellToMerge, SettlementReceiptRouteV4::SellToMerge)
        ),
        ClutchError::MismatchedState,
    )?;
    let candidates = [
        match slice.buy() {
            SettlementLegV1::Order(index) => Some((SettlementSideV1::Buy, index)),
            SettlementLegV1::Split | SettlementLegV1::Merge => None,
        },
        match slice.sell() {
            SettlementLegV1::Order(index) => Some((SettlementSideV1::Sell, index)),
            SettlementLegV1::Split | SettlementLegV1::Merge => None,
        },
    ];
    let mut matched = [None, None];
    for candidate in candidates.into_iter().flatten() {
        let membership = traversal
            .settlement_membership(candidate.1)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        let expected_order = match candidate.0 {
            SettlementSideV1::Buy => semantic.buy_order_id.bytes(),
            SettlementSideV1::Sell => semantic.sell_order_id.bytes(),
        };
        if membership.owner == owner.bytes() && membership.order_id == expected_order {
            let at = match candidate.0 {
                SettlementSideV1::Buy => 0,
                SettlementSideV1::Sell => 1,
            };
            require(matched[at].is_none(), ClutchError::MismatchedState)?;
            matched[at] = Some(candidate.1);
        }
    }
    let (side, order_index) = select_unaccounted_end(matched, semantic.accounted_end_mask)?;
    let mut completes = true;
    let mut next = semantic
        .slice_index
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    while next < traversal.projection().feed().slice_count {
        let later = traversal
            .settlement_slice(next)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        if later.buy() == SettlementLegV1::Order(order_index)
            || later.sell() == SettlementLegV1::Order(order_index)
        {
            completes = false;
            break;
        }
        next = next.checked_add(1).ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    Ok((side, expected_route, order_index, completes))
}

/// Select one receipt end without excluding valid same-owner self-crosses.
///
/// Direct receipts account buy before sell when both ends belong to the same
/// owner. The persisted end mask then makes the second call unambiguous.
fn select_unaccounted_end(
    matched: [Option<u8>; 2],
    accounted_end_mask: u8,
) -> Outcome<(SettlementSideV1, u8)> {
    if accounted_end_mask & RECEIPT_ACCOUNTED_BUY_END == 0 {
        if let Some(order) = matched[0] {
            return Ok((SettlementSideV1::Buy, order));
        }
    }
    if accounted_end_mask & RECEIPT_ACCOUNTED_SELL_END == 0 {
        if let Some(order) = matched[1] {
            return Ok((SettlementSideV1::Sell, order));
        }
    }
    Err(Refusal::Adapter(ClutchError::MismatchedState))
}

pub(crate) fn locate_order_slot(
    pages: &[AccountInfo<'_>],
    order_id: [u8; 32],
) -> Outcome<(OrderSlot, u16, u64)> {
    let mut found = None;
    for page_account in pages {
        let data = borrow_data(page_account)?;
        let page = verify_page_v5(&data)?;
        let mut cursor = OrderSlotCursorV5::new(&data)?;
        while let Some(next) = cursor.next_slot() {
            let slot = next?;
            if slot.slot.is_live() && slot.slot.order_id().bytes() == order_id {
                require(found.is_none(), ClutchError::MismatchedState)?;
                found = Some((slot.slot, page.page_index, slot.position_generation));
            }
        }
    }
    found.ok_or(Refusal::Adapter(ClutchError::MismatchedState))
}

/// Decode and execute exactly one action-25 request.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::AccountReceiptEnd
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let request = match decode_owner_settlement_payload_v1(action.tag(), payload)? {
        OwnerSettlementPayloadV1::AccountReceiptEnd(request) => request,
        OwnerSettlementPayloadV1::FreezeEntitlement(_)
        | OwnerSettlementPayloadV1::FinalizeOwnerSettlement(_) => {
            return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction));
        }
    };
    account_receipt_end_v5(program_id, accounts, request)
}

#[inline(never)]
fn account_receipt_end_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: contract::AccountReceiptEndPayloadV1,
) -> Outcome<()> {
    let page_count = page_count_from_account_len(accounts.len())?;
    require_all_distinct(accounts)?;
    let suffix_at = ACTION25_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(page_count)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(accounts.len() == suffix_at + ACTION25_SUFFIX_ACCOUNTS, ClutchError::WrongAccountCount)?;
    let receipt_account = &accounts[suffix_at];
    let rent_sysvar = &accounts[suffix_at + 1];
    let owner_row_account = &accounts[suffix_at + 2];
    let reservation_account = &accounts[suffix_at + 3];
    let position_account = &accounts[suffix_at + 4];
    let pages = &accounts[ACTION25_TRAVERSAL_PREFIX_ACCOUNTS..suffix_at];
    let traversal_frame = SettlementTraversalAccountFrameV5 {
        retained_feed: &accounts[IX_FEED],
        market_binding: &accounts[IX_MARKET_BINDING],
        market_runtime: &accounts[IX_MARKET_RUNTIME],
        economic_domain: &accounts[IX_ECONOMIC_DOMAIN],
        price_grid: &accounts[IX_PRICE_GRID],
        realm: &accounts[IX_REALM],
        profile: &accounts[IX_PROFILE],
        collateral_policy: &accounts[IX_COLLATERAL_POLICY],
        token_program: &accounts[IX_TOKEN_PROGRAM],
        market_instance: &accounts[IX_MARKET_INSTANCE],
        market_genesis: &accounts[IX_MARKET_GENESIS],
        pages,
    };
    let traversal = authenticate_settlement_traversal_v5(program_id, traversal_frame)?;
    let root_traversal = authenticate_readonly_root_settlement_traversal_v5(
        program_id,
        &accounts[IX_ROOT],
        &traversal,
    )?;
    let root_account = root_traversal.root().account();
    let root_epoch = root_traversal.root().root().epoch();
    let receipt = authenticate_general_receipt_v5_accounting_root_traversal(
        program_id,
        root_traversal,
        receipt_account,
    )?;
    let root = receipt.root();
    require(
        request.epoch == root_epoch
            && request.settlement_root == root_account
            && request.receipt == receipt.receipt_account()
            && request.owner_settlement == id(owner_row_account.key),
        ClutchError::MismatchedState,
    )?;

    require_program_state(
        program_id,
        owner_row_account,
        true,
        contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    )?;
    require_program_state(program_id, reservation_account, true, RESERVATION_ACCOUNT_BYTES_V9)?;
    require_program_state(program_id, position_account, false, POSITION_V3_BYTES)?;
    require_program_state(program_id, receipt_account, true, SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5)?;
    let rent = read_rent(rent_sysvar)?;
    let owner_row_rent = rent.minimum_balance(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;

    let row_data = borrow_data(owner_row_account)?;
    let row_envelope = OwnerSettlementV5AccountV1::decode(&row_data)?;
    let owner = Id32::new(row_envelope.semantic.expectation().owner())?;
    let row_seed = OwnerSettlementSeedTupleV5::new(root.epoch(), root.settlement_candidate_id(), owner)?;
    let row_pda = seeds::general_v2_owner_settlement_v5_pda(
        program_id,
        row_seed.epoch(),
        row_seed.settlement_candidate(),
        row_seed.owner(),
    );
    expect_pda(owner_row_account.key, row_pda, Some(row_envelope.stored_bump))?;
    let row_projection = project_owner_settlement_account_v5(
        OwnerSettlementAccountViewV5 {
            account: id(owner_row_account.key),
            program_owner: id(owner_row_account.owner),
            exact_body: &row_data,
            lamports: owner_row_account.lamports(),
            rent_minimum: owner_row_rent,
            canonical_bump: row_pda.1,
            writable: owner_row_account.is_writable,
        },
        id(program_id),
        row_seed,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expectation = row_projection.envelope().semantic.expectation();
    require(
        expectation.market() == root.market().bytes()
            && expectation.epoch() == root.epoch().bytes()
            && expectation.candidate() == root.settlement_candidate_id().bytes()
            && expectation.owner_order_set_digest() == root.owner_order_set_digest().bytes(),
        ClutchError::MismatchedState,
    )?;

    let traversal_access = traversal.traversal();
    let (side, route, order_index, completes_order) =
        route_and_order(traversal_access, &receipt.receipt(), owner)?;
    let membership = traversal_access
        .settlement_membership(order_index)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    membership
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let (slot, page_index, position_generation) = locate_order_slot(pages, membership.order_id)?;
    require(
        position_generation == membership.position_generation
            && membership.order_generation == slot.generation()
            && membership.owner == owner.bytes(),
        ClutchError::MismatchedState,
    )?;

    let reservation_data = borrow_data(reservation_account)?;
    let reservation_envelope = ReservationAccountV9::decode(&reservation_data)?;
    let mut reservation = reservation_envelope.body();
    let reservation_id = Id32::new(reservation.reservation.bytes())?;
    let reservation_seed = GeneralReservationSeedTupleV9::new(reservation_id)?;
    let reservation_pda =
        seeds::general_v2_reservation_v9_pda(program_id, reservation_seed.reservation_id());
    expect_pda(
        reservation_account.key,
        reservation_pda,
        Some(reservation.stored_bump),
    )?;
    let expected_side = match side { SettlementSideV1::Buy => 0, SettlementSideV1::Sell => 1 };
    let expected_kind = match membership.order_kind {
        OrderKindV1::Single => ORDER_KIND_SINGLE,
        OrderKindV1::Portfolio => ORDER_KIND_PORTFOLIO,
    };
    let reservation_plan = ReservationPlan::for_order(
        &slot,
        traversal.feed().outcome_count,
        traversal.feed().price_scale,
        reservation.max_fee_atoms,
    )?;
    let reservation_min = rent.minimum_balance(RESERVATION_ACCOUNT_BYTES_V9)?;
    let required_reservation_lamports = reservation_envelope
        .rent()
        .refundable_principal
        .checked_add(reservation_envelope.rent().donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        reservation_account.lamports() >= reservation_min
            && reservation_account.lamports() >= required_reservation_lamports
            && reservation.reservation.bytes() == membership.reservation
            && reservation.market.bytes() == root.market().bytes()
            && reservation.epoch.bytes() == root.epoch().bytes()
            && reservation.owner.bytes() == owner.bytes()
            && reservation.order_id.bytes() == membership.order_id
            && reservation.position_generation == membership.position_generation
            && reservation.order_generation == membership.order_generation
            && reservation.page_index == page_index
            && reservation.outcome_count == traversal.feed().outcome_count
            && reservation.side == expected_side
            && reservation.order_kind == expected_kind
            && reservation.state == RESERVATION_STATE_ENTITLED
            && reservation.entitled_units == membership.entitled_units
            && reservation.paid_units == reservation.consumed_units
            && reservation.initial_cash_atoms == reservation_plan.cash_atoms
            && reservation.max_fee_atoms == reservation_plan.max_fee_atoms
            && reservation.initial_internal == reservation_plan.internal,
        ClutchError::MismatchedState,
    )?;

    let position_data = borrow_data(position_account)?;
    let position = PositionAccountV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_pda = seeds::position_v3_pda(
        program_id,
        &root.market_instance_v2_id().bytes(),
        &owner.bytes(),
        PositionPurposeV3::General,
        &root.market().bytes(),
    );
    expect_pda(position_account.key, position_pda, Some(position.stored_bump()))?;
    let owner_identity = Identity32V1::new(owner.bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let purpose_binding = Identity32V1::new(root.market().bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projected_position = project_general_position_v3(
        position,
        traversal.projection().position_market_binding(),
        AdapterPositionPurposeBindingV3 {
            owner: owner_identity,
            controller: owner_identity,
            purpose_binding_id: purpose_binding,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
    .position();
    let position_fields = projected_position.fields();
    require(
        projected_position.lifecycle() == PositionLifecycleV3::Open
            && position_fields.generation == membership.position_generation,
        ClutchError::MismatchedState,
    )?;

    let handoff = match (side, completes_order) {
        (SettlementSideV1::Buy, true) => {
            let cash = reservation.remaining_cash_atoms;
            let accumulated = row_envelope
                .semantic
                .buy_cash_handoff_atoms()
                .checked_add(cash)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            require(
                cash == reservation.initial_cash_atoms
                    && reservation.remaining_internal == [0; clutch_solana_layout::MAX_OUTCOMES]
                    && position_fields.reserved_cash_atoms >= accumulated,
                ClutchError::MismatchedState,
            )?;
            reservation.remaining_cash_atoms = 0;
            Some(
                AuthenticatedReservationHandoffV3::new(
                    reservation_account.key.to_bytes(),
                    reservation.reservation.bytes(),
                    membership.order_id,
                    owner.bytes(),
                    cash,
                )
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            )
        }
        (SettlementSideV1::Buy, false) => {
            require(
                reservation.remaining_cash_atoms == reservation.initial_cash_atoms
                    && reservation.remaining_internal == [0; clutch_solana_layout::MAX_OUTCOMES],
                ClutchError::MismatchedState,
            )?;
            None
        }
        (SettlementSideV1::Sell, _) => {
            require(
                reservation.initial_cash_atoms == 0 && reservation.remaining_cash_atoms == 0,
                ClutchError::MismatchedState,
            )?;
            None
        }
    };

    let semantic = receipt.receipt().semantic();
    let evidence = receipt.evidence();
    let end = AuthenticatedSettlementReceiptEndV5 {
        receipt: receipt.receipt_account().bytes(),
        receipt_data_id: SettlementReceiptDataIdV5::new(evidence.receipt_data_id().bytes())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        receipt_accounting_id: evidence.receipt_accounting_id().bytes(),
        market: root.market().bytes(),
        epoch: root.epoch().bytes(),
        candidate: root.settlement_candidate_id().bytes(),
        owner_order_set_digest: root.owner_order_set_digest().bytes(),
        owner: owner.bytes(),
        order_id: membership.order_id,
        order_index,
        side,
        route,
        consideration_price_units: PresentConsiderationV2::new(
            semantic.consideration_price_units,
        ),
        completes_order,
        slice_index: semantic.slice_index,
        sequence: semantic.sequence,
        accounted_end_mask: semantic.accounted_end_mask,
        expected_end_mask: semantic.expected_end_mask(),
        reservation_handoff: handoff,
    };
    end.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut row_post = row_envelope;
    row_post
        .semantic
        .consume_v5(&end)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let side_mask = match side {
        SettlementSideV1::Buy => RECEIPT_ACCOUNTED_BUY_END,
        SettlementSideV1::Sell => RECEIPT_ACCOUNTED_SELL_END,
    };
    let mut receipt_semantic = semantic;
    receipt_semantic.accounted_end_mask |= side_mask;
    let receipt_post = SettlementReceiptAccountV5::new(
        receipt_semantic,
        receipt.receipt().transition(),
        receipt.receipt().rent(),
    )?;
    let reservation_post = ReservationAccountV9::new(reservation, reservation_envelope.rent())?;
    let row_post_body = row_post.encode_exact()?;
    let receipt_post_body = receipt_post.encode_exact()?;
    let mut reservation_post_body = [0u8; RESERVATION_ACCOUNT_BYTES_V9];
    reservation_post.encode(&mut reservation_post_body)?;

    drop(position_data);
    drop(reservation_data);
    drop(row_data);
    let mut row_out = borrow_mut_data(owner_row_account)?;
    let mut receipt_out = borrow_mut_data(receipt_account)?;
    let mut reservation_out = borrow_mut_data(reservation_account)?;
    row_out.copy_from_slice(&row_post_body);
    receipt_out.copy_from_slice(&receipt_post_body);
    reservation_out.copy_from_slice(&reservation_post_body);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action25_frame_has_exact_current_successors() {
        assert_eq!(ACTION25_TRAVERSAL_PREFIX_ACCOUNTS, 12);
        assert_eq!(ACTION25_SUFFIX_ACCOUNTS, 5);
        assert_eq!(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5, 298);
        assert_eq!(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5, 340);
        assert_eq!(RESERVATION_ACCOUNT_BYTES_V9, 666);
        assert_eq!(POSITION_V3_BYTES, 480);
    }

    #[test]
    fn action25_page_delimiter_refuses_zero_and_five_pages() {
        assert_eq!(
            page_count_from_account_len(
                ACTION25_TRAVERSAL_PREFIX_ACCOUNTS + ACTION25_SUFFIX_ACCOUNTS,
            ),
            Err(ClutchError::WrongAccountCount),
        );
        assert_eq!(
            page_count_from_account_len(
                ACTION25_TRAVERSAL_PREFIX_ACCOUNTS + 5 + ACTION25_SUFFIX_ACCOUNTS,
            ),
            Err(ClutchError::WrongAccountCount),
        );
    }

    #[test]
    fn same_owner_direct_receipt_accounts_each_end_once_in_canonical_order() {
        assert_eq!(
            select_unaccounted_end([Some(3), Some(9)], 0),
            Ok((SettlementSideV1::Buy, 3)),
        );
        assert_eq!(
            select_unaccounted_end(
                [Some(3), Some(9)],
                RECEIPT_ACCOUNTED_BUY_END,
            ),
            Ok((SettlementSideV1::Sell, 9)),
        );
        assert_eq!(
            select_unaccounted_end(
                [Some(3), Some(9)],
                RECEIPT_ACCOUNTED_BUY_END | RECEIPT_ACCOUNTED_SELL_END,
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState)),
        );
    }
}
