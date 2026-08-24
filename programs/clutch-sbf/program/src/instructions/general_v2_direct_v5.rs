//! Staged-disabled SBF composition for General V2 action 26 direct Egg delivery.
//!
//! The positional frame starts with the single shared settlement traversal:
//! read-only SettlementRoot, retained Feed, Market/Domain/Grid/Realm/Product
//! facts, and the complete one-to-four PageV5 suffix. The action-specific
//! suffix is one writable ReceiptV5, Rent, and buyer/seller groups containing
//! a read-only OwnerRowV5 plus writable ReservationV9, PositionV3, and GEN1
//! ReplayV3 accounts. Selected pages are references into the canonical shared
//! page suffix; they are never repeated as caller-selected endpoint accounts.
//!
//! Direct delivery is custody- and supply-neutral. It transfers already-backed
//! internal inventory from the seller Reservation to the buyer Position and
//! mutates only Receipt/Reservation/Position/Replay terminal bytes. Root,
//! Hoard, and Token-2022 custody are not rewritten. All seven mutable data
//! destinations are borrowed before the first byte changes.

use core::cell::{Ref, RefMut};
use std::boxed::Box;

use clutch_collateral_adapter_v2::BoundCollateralProfileV2;
use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_direct_settlement_payload_v1, DirectSettlementPayloadV1,
    GeneralOrderPageSeedTupleV5, GeneralReservationSeedTupleV9, Id32,
    OwnerSettlementSeedTupleV5, OwnerSettlementV5AccountV1,
    GENERAL_REPLAY_ACCOUNT_V1_BYTES,
};
use clutch_general_v2_runtime::{
    prepare_consume_direct_receipt_eggs_v5, project_owner_settlement_account_v5_readonly,
    ConsumeDirectReceiptEggsInputV5, ConsumeDirectReceiptEggsPlanV5,
    DirectEggDeliveryEndpointInputV5, OwnerSettlementAccountProjectionV5,
    OwnerSettlementAccountViewV5, PositionAccountInputV3, SettlementTraversalAccessV5,
};
use clutch_retirement::{PositionPurposeV3, POSITION_V3_BYTES};
use clutch_solana_layout::order_page_v5::{verify_page_v5, ORDER_PAGE_V5_BYTES};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::settlement_receipt_v5::SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5;
use clutch_solana_layout::MAX_ORDER_PAGES;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::read_rent;
use crate::seeds;

use super::collateral_position_v3::GeneralPositionReplayAuthorityV2;
use super::general_v2_position_replay::authenticate_current_general_position_replay_v2;
use super::general_v2_receipt_v5::{
    authenticate_general_receipt_v5_root_traversal,
};
use super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1;
use super::general_v2_settlement_traversal_v5::{
    authenticate_readonly_root_settlement_traversal_v5, authenticate_settlement_traversal_v5,
    AuthenticatedRootSettlementTraversalV5, SettlementTraversalAccountFrameV5,
};

/// Fixed shared traversal roles before its one-to-four PageV5 suffix.
pub const ACTION26_TRAVERSAL_PREFIX_ACCOUNTS: usize = 12;
/// Exact action-26 roles after the complete PageV5 suffix.
pub const ACTION26_DIRECT_SUFFIX_ACCOUNTS: usize = 10;

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

#[derive(Clone, Copy, Debug)]
pub struct DirectEndpointAccountFrameV5<'a, 'info> {
    /// Read-only finalized rent-owned owner row.
    pub owner_row: &'a AccountInfo<'info>,
    /// Writable canonical ReservationV9 endpoint.
    pub reservation: &'a AccountInfo<'info>,
    /// Writable canonical General PositionV3.
    pub position: &'a AccountInfo<'info>,
    /// Writable purpose-owned GEN1 ReplayV3.
    pub replay: &'a AccountInfo<'info>,
}

/// Exact action-specific suffix following the complete canonical page set.
#[derive(Clone, Copy, Debug)]
pub struct DirectDeliveryAccountFrameV5<'a, 'info> {
    /// Writable current rent-owned ReceiptV5.
    pub receipt: &'a AccountInfo<'info>,
    /// Read-only canonical Rent sysvar.
    pub rent_sysvar: &'a AccountInfo<'info>,
    /// Real buyer endpoint.
    pub buyer: DirectEndpointAccountFrameV5<'a, 'info>,
    /// Real seller endpoint.
    pub seller: DirectEndpointAccountFrameV5<'a, 'info>,
}

#[derive(Debug)]
struct EndpointData<'a> {
    owner_row: Ref<'a, [u8]>,
    page: Ref<'a, [u8]>,
    reservation: Ref<'a, [u8]>,
    position: Ref<'a, [u8]>,
    replay: Ref<'a, [u8]>,
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedEndpointV5 {
    owner: Id32,
    owner_row: OwnerSettlementAccountProjectionV5,
    replay: GeneralPositionReplayAuthorityV2,
    replay_bump: u8,
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

fn require_all_distinct(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
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

fn action26_page_count_from_account_len(total: usize) -> Result<usize, ClutchError> {
    let fixed = ACTION26_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(ACTION26_DIRECT_SUFFIX_ACCOUNTS)
        .ok_or(ClutchError::Arithmetic)?;
    let page_count = total
        .checked_sub(fixed)
        .ok_or(ClutchError::WrongAccountCount)?;
    if (1..=MAX_ORDER_PAGES).contains(&page_count) {
        Ok(page_count)
    } else {
        Err(ClutchError::WrongAccountCount)
    }
}

fn endpoint_page_index(frame: DirectEndpointAccountFrameV5<'_, '_>) -> Outcome<usize> {
    let reservation_data = borrow_data(frame.reservation)?;
    let reservation = ReservationAccountV9::decode(&reservation_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(usize::from(reservation.body().page_index))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_endpoint_v5(
    program_id: &Pubkey,
    root: &AuthenticatedGeneralSettlementRootV1,
    traversal: &dyn SettlementTraversalAccessV5,
    collateral: BoundCollateralProfileV2,
    market_binding: &AccountInfo<'_>,
    market_runtime: &AccountInfo<'_>,
    selected_page: &AccountInfo<'_>,
    frame: DirectEndpointAccountFrameV5<'_, '_>,
    rent_minimum: u64,
    data: &EndpointData<'_>,
) -> Outcome<AuthenticatedEndpointV5> {
    require_program_state(
        program_id,
        frame.owner_row,
        false,
        contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    )?;
    let row = OwnerSettlementV5AccountV1::decode(&data.owner_row)?;
    let expectation = row.semantic.expectation();
    let owner = Id32::new(expectation.owner())?;
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
    expect_pda(frame.owner_row.key, row_pda, Some(row.stored_bump))?;
    let owner_row = project_owner_settlement_account_v5_readonly(
        OwnerSettlementAccountViewV5 {
            account: id(frame.owner_row.key),
            program_owner: id(frame.owner_row.owner),
            exact_body: &data.owner_row,
            lamports: frame.owner_row.lamports(),
            rent_minimum,
            canonical_bump: row_pda.1,
            writable: frame.owner_row.is_writable,
        },
        id(program_id),
        row_seed,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require_program_state(program_id, selected_page, false, ORDER_PAGE_V5_BYTES)?;
    let page = verify_page_v5(&data.page)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let page_seed = GeneralOrderPageSeedTupleV5::new(root.root().epoch(), page.page_index)?;
    let page_pda = seeds::general_v2_order_page_v5_pda(
        program_id,
        page_seed.epoch(),
        u16::from_le_bytes(*page_seed.page_index_le()),
    );
    expect_pda(selected_page.key, page_pda, Some(page.stored_bump))?;
    require(
        page.frozen == 1
            && page.market.0 == root.root().market().bytes()
            && page.epoch.0 == root.root().epoch().bytes()
            && page.order_set.0 == root.root().order_set().bytes()
            && traversal.projection().page_account(page.page_index)
                == Some(id(selected_page.key)),
        ClutchError::MismatchedState,
    )?;

    require_program_state(
        program_id,
        frame.reservation,
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
        frame.reservation.key,
        reservation_pda,
        Some(reservation.body().stored_bump),
    )?;
    require(
        reservation.body().page_index == page.page_index,
        ClutchError::MismatchedState,
    )?;

    let replay = authenticate_current_general_position_replay_v2(
        program_id,
        collateral,
        market_binding,
        market_runtime,
        frame.position,
        frame.replay,
        owner.bytes(),
    )?;
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &frame.position.key.to_bytes(),
        PositionPurposeV3::General,
        &market_runtime.key.to_bytes(),
    );
    Ok(AuthenticatedEndpointV5 {
        owner,
        owner_row,
        replay,
        replay_bump: replay_pda.1,
    })
}

fn endpoint_input<'a>(
    frame: DirectEndpointAccountFrameV5<'_, '_>,
    selected_page: &AccountInfo<'_>,
    authenticated: AuthenticatedEndpointV5,
    data: &'a EndpointData<'_>,
) -> DirectEggDeliveryEndpointInputV5<'a> {
    DirectEggDeliveryEndpointInputV5 {
        owner_row: authenticated.owner_row,
        order_page_account: id(selected_page.key),
        order_page_body: &data.page,
        reservation_account: id(frame.reservation.key),
        reservation_body: &data.reservation,
        position: PositionAccountInputV3 {
            account: id(frame.position.key),
            encoded_body: &data.position,
        },
        replay_account: id(frame.replay.key),
        replay_bump: authenticated.replay_bump,
        replay_next_sequence: authenticated.replay.replay.next_sequence(),
        replay_body: &data.replay,
    }
}

#[inline(never)]
fn prepare_plan_boxed(
    input: ConsumeDirectReceiptEggsInputV5<'_>,
) -> Outcome<Box<ConsumeDirectReceiptEggsPlanV5>> {
    prepare_consume_direct_receipt_eggs_v5(input)
        .map(Box::new)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

#[inline(never)]
fn apply_direct_delivery_bundle_v5(
    retained_feed: &AccountInfo<'_>,
    frame: DirectDeliveryAccountFrameV5<'_, '_>,
    buyer_page: &AccountInfo<'_>,
    seller_page: &AccountInfo<'_>,
    plan: &ConsumeDirectReceiptEggsPlanV5,
) -> Outcome<()> {
    require(
        plan.retained_feed_account() == id(retained_feed.key)
            && plan.receipt_account() == id(frame.receipt.key)
            && plan.buyer().owner_settlement_account() == id(frame.buyer.owner_row.key)
            && plan.buyer().order_page_account() == id(buyer_page.key)
            && plan.buyer().reservation_account() == id(frame.buyer.reservation.key)
            && plan.buyer().position_account() == id(frame.buyer.position.key)
            && plan.buyer().replay().replay_account() == id(frame.buyer.replay.key)
            && plan.seller().owner_settlement_account() == id(frame.seller.owner_row.key)
            && plan.seller().order_page_account() == id(seller_page.key)
            && plan.seller().reservation_account() == id(frame.seller.reservation.key)
            && plan.seller().position_account() == id(frame.seller.position.key)
            && plan.seller().replay().replay_account() == id(frame.seller.replay.key),
        ClutchError::MismatchedState,
    )?;
    require(
        frame.receipt.data_len() == SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5
            && frame.buyer.reservation.data_len() == RESERVATION_ACCOUNT_BYTES_V9
            && frame.buyer.position.data_len() == POSITION_V3_BYTES
            && frame.buyer.replay.data_len() == GENERAL_REPLAY_ACCOUNT_V1_BYTES
            && frame.seller.reservation.data_len() == RESERVATION_ACCOUNT_BYTES_V9
            && frame.seller.position.data_len() == POSITION_V3_BYTES
            && frame.seller.replay.data_len() == GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        ClutchError::WrongDataLength,
    )?;

    // Acquire every fallible mutable borrow before the first poststate byte is
    // installed. No CPI, allocation, or other fallible step follows.
    let mut receipt_out = borrow_mut_data(frame.receipt)?;
    let mut buyer_reservation_out = borrow_mut_data(frame.buyer.reservation)?;
    let mut buyer_position_out = borrow_mut_data(frame.buyer.position)?;
    let mut buyer_replay_out = borrow_mut_data(frame.buyer.replay)?;
    let mut seller_reservation_out = borrow_mut_data(frame.seller.reservation)?;
    let mut seller_position_out = borrow_mut_data(frame.seller.position)?;
    let mut seller_replay_out = borrow_mut_data(frame.seller.replay)?;

    receipt_out.copy_from_slice(plan.receipt_poststate_body());
    buyer_reservation_out.copy_from_slice(plan.buyer().reservation_poststate_body());
    buyer_position_out.copy_from_slice(plan.buyer().position_poststate_body());
    buyer_replay_out.copy_from_slice(plan.buyer().replay().replay_poststate_body());
    seller_reservation_out.copy_from_slice(plan.seller().reservation_poststate_body());
    seller_position_out.copy_from_slice(plan.seller().position_poststate_body());
    seller_replay_out.copy_from_slice(plan.seller().replay().replay_poststate_body());
    Ok(())
}

/// Decode and execute exactly one staged-disabled current action-26 request.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::ConsumeDirectReceiptEggs
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let request = match decode_direct_settlement_payload_v1(action.tag(), payload)? {
        DirectSettlementPayloadV1::ConsumeDirectReceiptEggs(request) => request,
    };
    consume_direct_receipt_eggs_v5(program_id, accounts, request)
}

#[inline(never)]
fn consume_direct_receipt_eggs_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: contract::ConsumeDirectReceiptEggsPayloadV1,
) -> Outcome<()> {
    require(
        accounts.len()
            >= ACTION26_TRAVERSAL_PREFIX_ACCOUNTS + 1 + ACTION26_DIRECT_SUFFIX_ACCOUNTS
            && accounts.len()
                <= ACTION26_TRAVERSAL_PREFIX_ACCOUNTS
                    + MAX_ORDER_PAGES
                    + ACTION26_DIRECT_SUFFIX_ACCOUNTS,
        ClutchError::WrongAccountCount,
    )?;
    require_all_distinct(accounts)?;
    let page_count = action26_page_count_from_account_len(accounts.len())?;
    let suffix_at = ACTION26_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(page_count)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        accounts.len() == suffix_at + ACTION26_DIRECT_SUFFIX_ACCOUNTS,
        ClutchError::WrongAccountCount,
    )?;
    let frame = DirectDeliveryAccountFrameV5 {
        receipt: &accounts[suffix_at],
        rent_sysvar: &accounts[suffix_at + 1],
        buyer: DirectEndpointAccountFrameV5 {
            owner_row: &accounts[suffix_at + 2],
            reservation: &accounts[suffix_at + 3],
            position: &accounts[suffix_at + 4],
            replay: &accounts[suffix_at + 5],
        },
        seller: DirectEndpointAccountFrameV5 {
            owner_row: &accounts[suffix_at + 6],
            reservation: &accounts[suffix_at + 7],
            position: &accounts[suffix_at + 8],
            replay: &accounts[suffix_at + 9],
        },
    };
    require_program_state(
        program_id,
        frame.receipt,
        true,
        SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
    )?;
    require_program_state(
        program_id,
        frame.buyer.reservation,
        true,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    require_program_state(
        program_id,
        frame.seller.reservation,
        true,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    let buyer_page_index = endpoint_page_index(frame.buyer)?;
    let seller_page_index = endpoint_page_index(frame.seller)?;
    require(
        buyer_page_index < page_count && seller_page_index < page_count,
        ClutchError::MismatchedState,
    )?;

    let pages = &accounts[ACTION26_TRAVERSAL_PREFIX_ACCOUNTS..suffix_at];
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
    let authenticated_traversal =
        authenticate_settlement_traversal_v5(program_id, traversal_frame)?;
    let authenticated = authenticate_readonly_root_settlement_traversal_v5(
        program_id,
        &accounts[IX_ROOT],
        &authenticated_traversal,
    )?;
    compose_and_apply_direct_delivery_v5(
        program_id,
        request,
        authenticated,
        traversal_frame,
        frame,
        &pages[buyer_page_index],
        &pages[seller_page_index],
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn compose_and_apply_direct_delivery_v5(
    program_id: &Pubkey,
    request: contract::ConsumeDirectReceiptEggsPayloadV1,
    authenticated: AuthenticatedRootSettlementTraversalV5<'_, '_>,
    traversal_frame: SettlementTraversalAccountFrameV5<'_, '_>,
    frame: DirectDeliveryAccountFrameV5<'_, '_>,
    buyer_page: &AccountInfo<'_>,
    seller_page: &AccountInfo<'_>,
) -> Outcome<()> {
    let receipt = authenticate_general_receipt_v5_root_traversal(
        program_id,
        authenticated,
        frame.receipt,
    )?;
    let authenticated_root = authenticated.root();
    let authenticated_traversal = authenticated.traversal();
    let traversal = authenticated_traversal.traversal();
    let collateral = authenticated_traversal.collateral();
    require(
        request.epoch == authenticated_root.root().epoch()
            && request.receipt == receipt.receipt_account()
            && receipt.settlement_root_account() == authenticated_root.account()
            && receipt.retained_feed_account() == authenticated_traversal.feed_account(),
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(frame.rent_sysvar)?;
    let owner_row_rent = rent.minimum_balance(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;

    let buyer_data = EndpointData {
        owner_row: borrow_data(frame.buyer.owner_row)?,
        page: borrow_data(buyer_page)?,
        reservation: borrow_data(frame.buyer.reservation)?,
        position: borrow_data(frame.buyer.position)?,
        replay: borrow_data(frame.buyer.replay)?,
    };
    let seller_data = EndpointData {
        owner_row: borrow_data(frame.seller.owner_row)?,
        page: borrow_data(seller_page)?,
        reservation: borrow_data(frame.seller.reservation)?,
        position: borrow_data(frame.seller.position)?,
        replay: borrow_data(frame.seller.replay)?,
    };
    let buyer = authenticate_endpoint_v5(
        program_id,
        authenticated_root,
        traversal,
        collateral,
        traversal_frame.market_binding,
        traversal_frame.market_runtime,
        buyer_page,
        frame.buyer,
        owner_row_rent,
        &buyer_data,
    )?;
    let seller = authenticate_endpoint_v5(
        program_id,
        authenticated_root,
        traversal,
        collateral,
        traversal_frame.market_binding,
        traversal_frame.market_runtime,
        seller_page,
        frame.seller,
        owner_row_rent,
        &seller_data,
    )?;
    require(buyer.owner != seller.owner, ClutchError::AccountAlias)?;

    let feed_data = borrow_data(traversal_frame.retained_feed)?;
    let relation_market = authenticated_traversal.market().relation_projection();
    let plan = prepare_plan_boxed(ConsumeDirectReceiptEggsInputV5 {
        payload: request,
        settlement_root_account: authenticated_root.account(),
        settlement_root: authenticated_root.root(),
        retained_feed_account: authenticated_traversal.feed_account(),
        retained_feed_body: &feed_data,
        receipt: receipt.receipt(),
        receipt_evidence: receipt.evidence(),
        market_binding_account: id(traversal_frame.market_binding.key),
        market_binding: &relation_market,
        collateral,
        buyer: endpoint_input(frame.buyer, buyer_page, buyer, &buyer_data),
        seller: endpoint_input(frame.seller, seller_page, seller, &seller_data),
    })?;
    require(
        plan.settlement_root_account() == authenticated_root.account()
            && plan.buyer().position_prestate_semantic_id()
                == Id32::from_bytes(buyer.replay.position.semantic_id)
            && plan.seller().position_prestate_semantic_id()
                == Id32::from_bytes(seller.replay.position.semantic_id),
        ClutchError::MismatchedState,
    )?;

    drop(feed_data);
    drop(buyer_data);
    drop(seller_data);
    apply_direct_delivery_bundle_v5(
        traversal_frame.retained_feed,
        frame,
        buyer_page,
        seller_page,
        &plan,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_v5_frame_is_current_successors_only() {
        assert_eq!(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5, 298);
        assert_eq!(contract::OWNER_SETTLEMENT_ACCOUNT_BYTES_V5, 340);
        assert_eq!(RESERVATION_ACCOUNT_BYTES_V9, 666);
        assert_eq!(POSITION_V3_BYTES, 480);
        assert_eq!(GENERAL_REPLAY_ACCOUNT_V1_BYTES, 344);
        assert_eq!(ACTION26_TRAVERSAL_PREFIX_ACCOUNTS, 12);
        assert_eq!(ACTION26_DIRECT_SUFFIX_ACCOUNTS, 10);
    }

    #[test]
    fn sparse_and_churned_page_sets_keep_the_account_delimiter() {
        assert_eq!(
            action26_page_count_from_account_len(
                ACTION26_TRAVERSAL_PREFIX_ACCOUNTS + 3 + ACTION26_DIRECT_SUFFIX_ACCOUNTS,
            ),
            Ok(3),
        );
        assert_eq!(
            action26_page_count_from_account_len(
                ACTION26_TRAVERSAL_PREFIX_ACCOUNTS + 4 + ACTION26_DIRECT_SUFFIX_ACCOUNTS,
            ),
            Ok(4),
        );
    }

    #[test]
    fn direct_page_delimiter_refuses_missing_and_fifth_pages() {
        assert_eq!(
            action26_page_count_from_account_len(
                ACTION26_TRAVERSAL_PREFIX_ACCOUNTS + ACTION26_DIRECT_SUFFIX_ACCOUNTS,
            ),
            Err(ClutchError::WrongAccountCount),
        );
        assert_eq!(
            action26_page_count_from_account_len(
                ACTION26_TRAVERSAL_PREFIX_ACCOUNTS + 5 + ACTION26_DIRECT_SUFFIX_ACCOUNTS,
            ),
            Err(ClutchError::WrongAccountCount),
        );
    }

    #[test]
    fn endpoint_suffix_order_is_exact() {
        let receipt = 0usize;
        let rent = receipt + 1;
        let buyer_row = rent + 1;
        let buyer_reservation = buyer_row + 1;
        let buyer_position = buyer_reservation + 1;
        let buyer_replay = buyer_position + 1;
        let seller_row = buyer_replay + 1;
        let seller_reservation = seller_row + 1;
        let seller_position = seller_reservation + 1;
        let seller_replay = seller_position + 1;
        assert_eq!(seller_replay + 1, ACTION26_DIRECT_SUFFIX_ACCOUNTS);
    }
}
