//! Staged-disabled action-41 SBF composition and atomic Reservation close.
//!
//! The positional ABI is frozen but staged-disabled. Action 41 consumes the
//! single shared action-39/24/41 traversal authenticator, then authenticates
//! every mutable endpoint and close destination itself and owns the final
//! root/Position/Replay write plus ReservationV9 close.
//!
//! The selected 4,140-byte page is always borrowed. The returned pure bundle
//! is boxed, and all four data destinations plus all three lamport destinations
//! are borrowed before the first mutation. No dispatch arm or capability is
//! exposed while action 41 remains `ReservedDisabled`.

use core::cell::{Ref, RefMut};
use std::boxed::Box;

use clutch_collateral_adapter_v2::BoundCollateralProfileV2;
use clutch_general_v2_contract::{
    decode_settlement_root_payload_v1, GeneralOrderPageSeedTupleV5,
    GeneralReservationSeedTupleV9, Id32, ReleaseUnfilledReservationPayloadV1,
    SettlementRootPayloadV1, SettlementRootV1AccountV1,
    GENERAL_REPLAY_ACCOUNT_V1_BYTES, MARKET_BINDING_ACCOUNT_BYTES_V2,
    MARKET_RUNTIME_ACCOUNT_BYTES, SETTLEMENT_ROOT_ACCOUNT_BYTES,
};
use clutch_general_v2_runtime::{
    prepare_release_unfilled_reservation_v1, PositionAccountInputV3,
    ReleaseUnfilledReservationInputV1, ReleaseUnfilledReservationPlanV1,
    SettlementTraversalAccessV5, UnfilledReservationRentBalancesV1,
};
use clutch_retirement::{PositionPurposeV3, POSITION_V3_BYTES};
use clutch_solana_layout::order_page_v5::{verify_page_v5, ORDER_PAGE_V5_BYTES};
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::MAX_ORDER_PAGES;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;

use super::collateral_position_v3::GeneralPositionReplayAuthorityV2;
use super::general_v2_position_replay::authenticate_current_general_position_replay_v2;
use super::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1;
use super::general_v2_settlement_traversal_v5::{
    authenticate_settlement_traversal_v5, authenticate_writable_root_settlement_traversal_v5,
    AuthenticatedRootSettlementTraversalV5, SettlementTraversalAccountFrameV5,
};

/// Fixed shared traversal roles before its one-to-four PageV5 suffix.
pub const ACTION41_TRAVERSAL_PREFIX_ACCOUNTS: usize = 12;
/// Exact action-41 endpoint/close roles after the complete PageV5 suffix.
pub const ACTION41_ENDPOINT_SUFFIX_ACCOUNTS: usize = 5;

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

/// Exact action-41 endpoint and close suffix after the shared page set.
#[derive(Clone, Copy, Debug)]
pub struct ReleaseUnfilledReservationAccountFrameV1<'a, 'info> {
    /// Writable active ReservationV9 that will be terminalized and closed.
    pub reservation: &'a AccountInfo<'info>,
    /// Writable canonical General PositionV3 receiving released value.
    pub position: &'a AccountInfo<'info>,
    /// Writable purpose-owned GEN1 ReplayV3.
    pub replay: &'a AccountInfo<'info>,
    /// Writable persisted Reservation rent-principal payer.
    pub rent_payer: &'a AccountInfo<'info>,
    /// Writable MarketBinding-owned donation/surplus sink.
    pub neutral_sink: &'a AccountInfo<'info>,
}

#[derive(Debug)]
struct ReleaseData<'a> {
    selected_page: Ref<'a, [u8]>,
    reservation: Ref<'a, [u8]>,
    position: Ref<'a, [u8]>,
    replay: Ref<'a, [u8]>,
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedReleaseEndpointV1 {
    reservation: ReservationAccountV9,
    replay: GeneralPositionReplayAuthorityV2,
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

fn require_readonly_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.data_is_empty(), ClutchError::WrongDataLength)
}

fn require_distinct_frame(frame: ReleaseUnfilledReservationAccountFrameV1<'_, '_>) -> Outcome<()> {
    let accounts = [
        frame.reservation,
        frame.position,
        frame.replay,
        frame.rent_payer,
        frame.neutral_sink,
    ];
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

fn require_all_distinct(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
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

fn action41_page_count_from_account_len(total: usize) -> Result<usize, ClutchError> {
    let fixed = ACTION41_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(ACTION41_ENDPOINT_SUFFIX_ACCOUNTS)
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

#[inline(never)]
fn authenticate_release_endpoint_v1(
    program_id: &Pubkey,
    root: &AuthenticatedGeneralSettlementRootV1,
    traversal: &dyn SettlementTraversalAccessV5,
    collateral: BoundCollateralProfileV2,
    selected_page: &AccountInfo<'_>,
    market_binding: &AccountInfo<'_>,
    market_runtime: &AccountInfo<'_>,
    frame: ReleaseUnfilledReservationAccountFrameV1<'_, '_>,
    data: &ReleaseData<'_>,
) -> Outcome<AuthenticatedReleaseEndpointV1> {
    require_program_state(
        program_id,
        selected_page,
        false,
        ORDER_PAGE_V5_BYTES,
    )?;
    let page = verify_page_v5(&data.selected_page)
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

    let owner = reservation.body().owner.bytes();
    let replay = authenticate_current_general_position_replay_v2(
        program_id,
        collateral,
        market_binding,
        market_runtime,
        frame.position,
        frame.replay,
        owner,
    )?;
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &frame.position.key.to_bytes(),
        PositionPurposeV3::General,
        &market_runtime.key.to_bytes(),
    );
    Ok(AuthenticatedReleaseEndpointV1 {
        reservation,
        replay,
        replay_bump: replay_pda.1,
    })
}

#[inline(never)]
fn prepare_plan_boxed(
    input: ReleaseUnfilledReservationInputV1<'_>,
) -> Outcome<Box<ReleaseUnfilledReservationPlanV1>> {
    prepare_release_unfilled_reservation_v1(input)
        .map(Box::new)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

#[inline(never)]
fn apply_release_bundle_v1(
    settlement_root: &AccountInfo<'_>,
    retained_feed: &AccountInfo<'_>,
    selected_page: &AccountInfo<'_>,
    frame: ReleaseUnfilledReservationAccountFrameV1<'_, '_>,
    plan: &ReleaseUnfilledReservationPlanV1,
) -> Outcome<()> {
    let close = plan.reservation();
    require(
        plan.settlement_root_account() == id(settlement_root.key)
            && plan.retained_feed_account() == id(retained_feed.key)
            && plan.order_page_account() == id(selected_page.key)
            && close.account() == id(frame.reservation.key)
            && plan.position_account() == id(frame.position.key)
            && plan.replay().replay_account() == id(frame.replay.key)
            && close.payer() == id(frame.rent_payer.key)
            && close.neutral_sink() == id(frame.neutral_sink.key)
            && close.balance_before() == frame.reservation.lamports()
            && close.payer_balance_after()
                == frame
                    .rent_payer
                    .lamports()
                    .checked_add(close.payer_refund_lamports())
                    .ok_or(ClutchError::Arithmetic)?
            && close.neutral_sink_balance_after()
                == frame
                    .neutral_sink
                    .lamports()
                    .checked_add(close.neutral_sink_credit_lamports())
                    .ok_or(ClutchError::Arithmetic)?
            && close.payer_refund_lamports()
                .checked_add(close.neutral_sink_credit_lamports())
                .ok_or(ClutchError::Arithmetic)?
                == frame.reservation.lamports(),
        ClutchError::MismatchedState,
    )?;

    let mut root_body = std::vec![0u8; SETTLEMENT_ROOT_ACCOUNT_BYTES];
    plan.settlement_root_poststate().encode(&mut root_body)?;
    require(
        settlement_root.data_len() == SETTLEMENT_ROOT_ACCOUNT_BYTES
            && frame.reservation.data_len() == RESERVATION_ACCOUNT_BYTES_V9
            && frame.position.data_len() == POSITION_V3_BYTES
            && frame.replay.data_len() == GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        ClutchError::WrongDataLength,
    )?;

    {
        // Acquire every fallible borrow before the first byte or lamport is
        // changed. The close resize occurs only after these guards are dropped.
        let mut root_out = borrow_mut_data(settlement_root)?;
        let mut reservation_out = borrow_mut_data(frame.reservation)?;
        let mut position_out = borrow_mut_data(frame.position)?;
        let mut replay_out = borrow_mut_data(frame.replay)?;
        let mut reservation_lamports = frame
            .reservation
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut payer_lamports = frame
            .rent_payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_lamports = frame
            .neutral_sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;

        root_out.copy_from_slice(&root_body);
        reservation_out.copy_from_slice(close.terminal_body());
        position_out.copy_from_slice(plan.position_poststate_body());
        replay_out.copy_from_slice(plan.replay().replay_poststate_body());
        **reservation_lamports = close.balance_after();
        **payer_lamports = close.payer_balance_after();
        **sink_lamports = close.neutral_sink_balance_after();
    }
    frame
        .reservation
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    frame.reservation.assign(&SYSTEM_PROGRAM_ID);
    require(
        frame.reservation.data_len() == 0
            && frame.reservation.lamports() == 0
            && *frame.reservation.owner == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

/// Strict staged-disabled action-41 positional entrypoint.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::ReleaseUnfilledReservation
            && capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let SettlementRootPayloadV1::ReleaseUnfilledReservation(request) =
        decode_settlement_root_payload_v1(action.tag(), payload)?
    else {
        return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction));
    };
    release_unfilled_reservation(program_id, accounts, request)
}

#[inline(never)]
fn release_unfilled_reservation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    payload: ReleaseUnfilledReservationPayloadV1,
) -> Outcome<()> {
    require(
        accounts.len()
            >= ACTION41_TRAVERSAL_PREFIX_ACCOUNTS + 1 + ACTION41_ENDPOINT_SUFFIX_ACCOUNTS
            && accounts.len()
                <= ACTION41_TRAVERSAL_PREFIX_ACCOUNTS
                    + MAX_ORDER_PAGES
                    + ACTION41_ENDPOINT_SUFFIX_ACCOUNTS,
        ClutchError::WrongAccountCount,
    )?;
    require_all_distinct(accounts)?;
    require_program_state(
        program_id,
        &accounts[IX_ROOT],
        true,
        SETTLEMENT_ROOT_ACCOUNT_BYTES,
    )?;
    let page_count = action41_page_count_from_account_len(accounts.len())?;
    let endpoint_at = ACTION41_TRAVERSAL_PREFIX_ACCOUNTS
        .checked_add(page_count)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        accounts.len() == endpoint_at + ACTION41_ENDPOINT_SUFFIX_ACCOUNTS,
        ClutchError::WrongAccountCount,
    )?;
    let endpoint_frame = ReleaseUnfilledReservationAccountFrameV1 {
        reservation: &accounts[endpoint_at],
        position: &accounts[endpoint_at + 1],
        replay: &accounts[endpoint_at + 2],
        rent_payer: &accounts[endpoint_at + 3],
        neutral_sink: &accounts[endpoint_at + 4],
    };
    require_program_state(
        program_id,
        endpoint_frame.reservation,
        true,
        RESERVATION_ACCOUNT_BYTES_V9,
    )?;
    let selected_page_index = usize::from(
        ReservationAccountV9::decode(&borrow_data(endpoint_frame.reservation)?)?
            .body()
            .page_index,
    );
    require(selected_page_index < page_count, ClutchError::MismatchedState)?;
    let pages = &accounts[ACTION41_TRAVERSAL_PREFIX_ACCOUNTS..endpoint_at];
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
    let authenticated = authenticate_writable_root_settlement_traversal_v5(
        program_id,
        &accounts[IX_ROOT],
        &authenticated_traversal,
    )?;
    compose_and_apply_release_unfilled_reservation_v1(
        program_id,
        payload,
        authenticated,
        &accounts[IX_ROOT],
        traversal_frame,
        &pages[selected_page_index],
        endpoint_frame,
    )
}

/// Compose and atomically apply action 41 after the single shared General
/// authenticator has equality-bound the writable root and exhaustive traversal.
#[inline(never)]
fn compose_and_apply_release_unfilled_reservation_v1(
    program_id: &Pubkey,
    payload: ReleaseUnfilledReservationPayloadV1,
    authenticated: AuthenticatedRootSettlementTraversalV5<'_, '_>,
    settlement_root: &AccountInfo<'_>,
    traversal_frame: SettlementTraversalAccountFrameV5<'_, '_>,
    selected_page: &AccountInfo<'_>,
    frame: ReleaseUnfilledReservationAccountFrameV1<'_, '_>,
) -> Outcome<()> {
    let authenticated_root = authenticated.root();
    let authenticated_traversal = authenticated.traversal();
    let traversal = authenticated_traversal.traversal();
    let collateral = authenticated_traversal.collateral();
    require_distinct_frame(frame)?;
    require_program_state(
        program_id,
        settlement_root,
        true,
        SETTLEMENT_ROOT_ACCOUNT_BYTES,
    )?;
    let observed_root = SettlementRootV1AccountV1::decode(&borrow_data(settlement_root)?)?;
    require(
        authenticated_root.account() == id(settlement_root.key)
            && authenticated_root.root() == &observed_root
            && payload.epoch == authenticated_root.root().epoch()
            && payload.settlement_root == authenticated_root.account(),
        ClutchError::MismatchedState,
    )?;
    require_readonly_program_state(program_id, traversal_frame.retained_feed)?;
    require(
        id(traversal_frame.retained_feed.key) == traversal.projection().selected_feed_account()
            && id(traversal_frame.market_binding.key) == authenticated_root.root().market_binding()
            && id(traversal_frame.market_runtime.key) == authenticated_root.root().market(),
        ClutchError::MismatchedState,
    )?;
    require_program_state(
        program_id,
        traversal_frame.market_binding,
        false,
        MARKET_BINDING_ACCOUNT_BYTES_V2,
    )?;
    require_program_state(
        program_id,
        traversal_frame.market_runtime,
        false,
        MARKET_RUNTIME_ACCOUNT_BYTES,
    )?;
    for destination in [frame.rent_payer, frame.neutral_sink] {
        require(!destination.executable, ClutchError::ExecutableAccount)?;
        require(!destination.is_signer, ClutchError::MismatchedState)?;
        require(destination.is_writable, ClutchError::NotWritable)?;
    }

    let data = ReleaseData {
        selected_page: borrow_data(selected_page)?,
        reservation: borrow_data(frame.reservation)?,
        position: borrow_data(frame.position)?,
        replay: borrow_data(frame.replay)?,
    };
    let endpoint = Box::new(authenticate_release_endpoint_v1(
        program_id,
        authenticated_root,
        traversal,
        collateral,
        selected_page,
        traversal_frame.market_binding,
        traversal_frame.market_runtime,
        frame,
        &data,
    )?);
    let rent = endpoint.reservation.rent();
    require(
        rent.payer.bytes() == frame.rent_payer.key.to_bytes()
            && traversal.projection().neutral_sink() == id(frame.neutral_sink.key),
        ClutchError::MismatchedState,
    )?;
    let plan = prepare_plan_boxed(ReleaseUnfilledReservationInputV1 {
        payload,
        settlement_root_account: authenticated_root.account(),
        settlement_root: authenticated_root.root(),
        traversal,
        order_page_account: id(selected_page.key),
        order_page_body: &data.selected_page,
        reservation_account: id(frame.reservation.key),
        reservation_body: &data.reservation,
        rent_balances: UnfilledReservationRentBalancesV1 {
            reservation_lamports: frame.reservation.lamports(),
            payer_lamports: frame.rent_payer.lamports(),
            neutral_sink_lamports: frame.neutral_sink.lamports(),
        },
        position: PositionAccountInputV3 {
            account: id(frame.position.key),
            encoded_body: &data.position,
        },
        replay_account: id(frame.replay.key),
        replay_bump: endpoint.replay_bump,
        replay_next_sequence: endpoint.replay.replay.next_sequence(),
        replay_body: &data.replay,
    })?;
    require(
        plan.position_prestate_semantic_id()
            == Id32::from_bytes(endpoint.replay.position.semantic_id),
        ClutchError::MismatchedState,
    )?;

    drop(data);
    apply_release_bundle_v1(
        settlement_root,
        traversal_frame.retained_feed,
        selected_page,
        frame,
        &plan,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action41_widths_are_current_successors_only() {
        assert_eq!(RESERVATION_ACCOUNT_BYTES_V9, 666);
        assert_eq!(SETTLEMENT_ROOT_ACCOUNT_BYTES, 980);
        assert_eq!(POSITION_V3_BYTES, 480);
        assert_eq!(GENERAL_REPLAY_ACCOUNT_V1_BYTES, 344);
        assert_eq!(ACTION41_TRAVERSAL_PREFIX_ACCOUNTS, 12);
        assert_eq!(ACTION41_ENDPOINT_SUFFIX_ACCOUNTS, 5);
    }

    #[test]
    fn sparse_and_churned_page_sets_keep_the_account_delimiter() {
        assert_eq!(
            action41_page_count_from_account_len(
                ACTION41_TRAVERSAL_PREFIX_ACCOUNTS + 3 + ACTION41_ENDPOINT_SUFFIX_ACCOUNTS,
            ),
            Ok(3)
        );
        assert_eq!(
            action41_page_count_from_account_len(
                ACTION41_TRAVERSAL_PREFIX_ACCOUNTS + 4 + ACTION41_ENDPOINT_SUFFIX_ACCOUNTS,
            ),
            Ok(4)
        );
    }

    #[test]
    fn action41_page_delimiter_refuses_missing_and_fifth_pages() {
        assert_eq!(
            action41_page_count_from_account_len(
                ACTION41_TRAVERSAL_PREFIX_ACCOUNTS + ACTION41_ENDPOINT_SUFFIX_ACCOUNTS,
            ),
            Err(ClutchError::WrongAccountCount)
        );
        assert_eq!(
            action41_page_count_from_account_len(
                ACTION41_TRAVERSAL_PREFIX_ACCOUNTS + 5 + ACTION41_ENDPOINT_SUFFIX_ACCOUNTS,
            ),
            Err(ClutchError::WrongAccountCount)
        );
    }
}
