//! Live authority chain for the bounded one-page/two-order direct profile.
//!
//! This family is deliberately narrower than the general relation: exactly two
//! distinct owners, opposite single-Egg sides on one outcome, equal full-fill
//! quantities, no fee, no partial, one page, and exact price-scale division.
//! It consumes the full-width BatchPolicy/domain/candidate model and never
//! projects authority through the legacy CandidateFeed account.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program,
};
use crate::instructions::orders_batch::settlement::{apply_direct_full_slice, DirectFullSlicePlan};
use crate::seeds;
use clutch_batch::relation_v1::FrozenPolicyV1;
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy,
    direct_window_v1::{
        plan_admission_retained, plan_selection_retained, verify_direct_two_order_candidate,
        DirectCandidateV2, DirectCandidateWindowV1, DirectRetainedCandidateV1,
        DirectTwoOrderInputV1, DirectWindowBindingV1, DirectWindowErrorV1, ValidatedDirectDomainV1,
        DIRECT_CANDIDATE_ACCOUNT_BYTES, DIRECT_CANDIDATE_STATUS_SELECTED,
        DIRECT_CANDIDATE_STATUS_SUPERSEDED, DIRECT_CANDIDATE_STATUS_VERIFIED, DIRECT_POLICY_V1,
        DIRECT_WINDOW_ACCOUNT_BYTES, DIRECT_WINDOW_PHASE_OPEN, DIRECT_WINDOW_PHASE_SELECTED,
        MAX_DIRECT_CANDIDATES,
    },
    FullRelationDomainV1, Identity32V1, BATCH_POLICY_BYTES,
};
use clutch_solana_layout::{
    account_len,
    direct_selection::{
        decode_direct_candidate, decode_direct_window, encode_direct_candidate,
        encode_direct_window, DirectEpochV3Account, DIRECT_EPOCH_BYTES,
    },
    reservation::{
        ReservationAccount, ReservationPlan, RESERVATION_ACCOUNT_BYTES, RESERVATION_STATE_ACTIVE,
        RESERVATION_STATE_ENTITLED,
    },
    stream, CodecError, FinalPotAccount, Hash32, Intent, OrderRecord, OrderSlot, PositionAccount,
    PriceGridAccount, SettlementReceiptAccount, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN,
    EPOCH_PHASE_OPEN, MAX_OUTCOMES, POT_PHASE_CLOSED, RECEIPT_LEG_DIRECT,
};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

const INIT_ACCOUNT_COUNT: usize = 9;
const FREEZE_ACCOUNT_COUNT: usize = 7;
const SUBMIT_BASE_ACCOUNT_COUNT: usize = 12;
const SELECT_BASE_ACCOUNT_COUNT: usize = 13;
const SETTLE_ACCOUNT_COUNT: usize = 10;

const IX_INIT_PAYER: usize = 0;
const IX_INIT_EPOCH: usize = 1;
const IX_INIT_MARKET: usize = 2;
const IX_INIT_TERMS: usize = 3;
const IX_INIT_GRID: usize = 4;
const IX_INIT_POLICY: usize = 5;
const IX_INIT_SYSTEM: usize = 6;
const IX_INIT_RENT: usize = 7;
const IX_INIT_CLOCK: usize = 8;

const IX_FREEZE_EPOCH: usize = 0;
const IX_FREEZE_PAGE: usize = 1;
const IX_FREEZE_GRID: usize = 2;
const IX_FREEZE_POLICY: usize = 3;
const IX_FREEZE_RES_ZERO: usize = 4;
const IX_FREEZE_RES_ONE: usize = 5;
const IX_FREEZE_CLOCK: usize = 6;

const IX_SUBMIT_PAYER: usize = 0;
const IX_SUBMIT_EPOCH: usize = 1;
const IX_SUBMIT_POLICY: usize = 2;
const IX_SUBMIT_GRID: usize = 3;
const IX_SUBMIT_PAGE: usize = 4;
const IX_SUBMIT_RES_ZERO: usize = 5;
const IX_SUBMIT_RES_ONE: usize = 6;
const IX_SUBMIT_WINDOW: usize = 7;
const IX_SUBMIT_CANDIDATE: usize = 8;
const IX_SUBMIT_TOP_START: usize = 9;

const IX_SELECT_PAYER: usize = 0;
const IX_SELECT_EPOCH: usize = 1;
const IX_SELECT_POLICY: usize = 2;
const IX_SELECT_GRID: usize = 3;
const IX_SELECT_PAGE: usize = 4;
const IX_SELECT_RES_ZERO: usize = 5;
const IX_SELECT_RES_ONE: usize = 6;
const IX_SELECT_WINDOW: usize = 7;
const IX_SELECT_TOP_START: usize = 8;

const IX_SETTLE_EPOCH: usize = 0;
const IX_SETTLE_WINDOW: usize = 1;
const IX_SETTLE_CANDIDATE: usize = 2;
const IX_SETTLE_PAGE: usize = 3;
const IX_SETTLE_BUY_POSITION: usize = 4;
const IX_SETTLE_SELL_POSITION: usize = 5;
const IX_SETTLE_BUY_RESERVATION: usize = 6;
const IX_SETTLE_SELL_RESERVATION: usize = 7;
const IX_SETTLE_RECEIPT: usize = 8;
const IX_SETTLE_POT: usize = 9;

const INIT_ROLES: [StateRole; 3] = [
    StateRole::read_only(IX_INIT_MARKET, account_len::MARKET),
    StateRole::read_only(IX_INIT_TERMS, account_len::TERMS),
    StateRole::read_only(IX_INIT_GRID, account_len::PRICE_GRID),
];

const FREEZE_ROLES: [StateRole; 6] = [
    StateRole::writable(IX_FREEZE_EPOCH, DIRECT_EPOCH_BYTES),
    StateRole::writable(IX_FREEZE_PAGE, account_len::ORDER_PAGE),
    StateRole::read_only(IX_FREEZE_GRID, account_len::PRICE_GRID),
    StateRole::read_only(IX_FREEZE_POLICY, BATCH_POLICY_BYTES),
    StateRole::read_only(IX_FREEZE_RES_ZERO, RESERVATION_ACCOUNT_BYTES),
    StateRole::read_only(IX_FREEZE_RES_ONE, RESERVATION_ACCOUNT_BYTES),
];

const SETTLE_ROLES: [StateRole; SETTLE_ACCOUNT_COUNT] = [
    StateRole::read_only(IX_SETTLE_EPOCH, DIRECT_EPOCH_BYTES),
    StateRole::read_only(IX_SETTLE_WINDOW, DIRECT_WINDOW_ACCOUNT_BYTES),
    StateRole::read_only(IX_SETTLE_CANDIDATE, DIRECT_CANDIDATE_ACCOUNT_BYTES),
    StateRole::read_only(IX_SETTLE_PAGE, account_len::ORDER_PAGE),
    StateRole::writable(IX_SETTLE_BUY_POSITION, account_len::POSITION),
    StateRole::writable(IX_SETTLE_SELL_POSITION, account_len::POSITION),
    StateRole::writable(IX_SETTLE_BUY_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_SETTLE_SELL_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_SETTLE_RECEIPT, account_len::SETTLEMENT_RECEIPT),
    StateRole::read_only(IX_SETTLE_POT, account_len::FINAL_POT),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::instructions) struct DirectOrders {
    pub(in crate::instructions) zero: OrderRecord,
    pub(in crate::instructions) one: OrderRecord,
    pub(in crate::instructions) buy_index: u8,
    pub(in crate::instructions) sell_index: u8,
    pub(in crate::instructions) outcome: u8,
    pub(in crate::instructions) quantity: u64,
    pub(in crate::instructions) buy_limit: u64,
    pub(in crate::instructions) sell_limit: u64,
}

#[derive(Clone, Copy)]
struct SelectionSource {
    epoch: DirectEpochV3Account,
    orders: DirectOrders,
}

#[derive(Clone, Copy)]
struct SubmissionCommit {
    candidate: DirectCandidateV2,
    window: DirectCandidateWindowV1,
    displaced: Identity32V1,
}

#[derive(Clone, Copy)]
struct SubmissionAuthority {
    candidate: DirectCandidateV2,
    opens_slot: u64,
    closes_slot: u64,
}

#[derive(Clone, Copy)]
struct SelectionCommit {
    epoch: DirectEpochV3Account,
    window: DirectCandidateWindowV1,
    candidate_id: Identity32V1,
    receipt: SettlementReceiptAccount,
    pot: FinalPotAccount,
}

#[derive(Clone, Copy)]
struct SelectedFacts {
    candidate_id: Identity32V1,
    prices: [u64; MAX_OUTCOMES],
    quantity: u64,
    buy_index: u8,
    sell_index: u8,
    outcome: u8,
}

#[derive(Clone, Copy)]
struct SettleFacts {
    epoch: clutch_solana_layout::EpochAccount,
    candidate_id: Hash32,
    buy: OrderRecord,
    sell: OrderRecord,
    outcome: u8,
    quantity: u64,
    price: u64,
    consideration_units: u128,
    consideration_atoms: u64,
}

impl DirectOrders {
    pub(in crate::instructions) fn buy(self) -> OrderRecord {
        if self.buy_index == 0 {
            self.zero
        } else {
            self.one
        }
    }

    pub(in crate::instructions) fn sell(self) -> OrderRecord {
        if self.sell_index == 0 {
            self.zero
        } else {
            self.one
        }
    }
}

fn map_window(error: DirectWindowErrorV1) -> Refusal {
    match error {
        DirectWindowErrorV1::BeforeOpen
        | DirectWindowErrorV1::SubmissionClosed
        | DirectWindowErrorV1::SelectionEarly
        | DirectWindowErrorV1::AlreadySelected => ClutchError::NotActive.into(),
        DirectWindowErrorV1::Replay => ClutchError::Replay.into(),
        DirectWindowErrorV1::ArithmeticOverflow => ClutchError::Arithmetic.into(),
        _ => ClutchError::MismatchedState.into(),
    }
}

fn require_zero_sequence(sequence: u64) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)
}

/// Route one already-decoded direct authority request.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::InitDirectEpochV3 {
            market,
            epoch_index,
            policy,
            submission_opens_slot,
            submission_closes_slot,
        }) => init_epoch(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch_index,
            policy,
            submission_opens_slot,
            submission_closes_slot,
        ),
        Action::Layout(Intent::FreezeDirectEpochV3 { market, epoch }) => {
            freeze_epoch(program_id, accounts, request.sequence, market, epoch)
        }
        Action::Layout(Intent::SubmitDirectCandidateV2 {
            market,
            epoch,
            outcome_price,
        }) => submit_candidate(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            outcome_price,
        ),
        Action::Layout(Intent::SelectDirectWindowV1 { market, epoch }) => {
            select_window(program_id, accounts, request.sequence, market, epoch)
        }
        Action::Layout(Intent::SettleDirectV2 { market, epoch }) => {
            settle(program_id, accounts, request.sequence, market, epoch)
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn init_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    epoch_index: u64,
    policy_digest: Hash32,
    opens_slot: u64,
    closes_slot: u64,
) -> Outcome<()> {
    require_count(accounts, INIT_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require_signer(&accounts[IX_INIT_PAYER])?;
    require(
        accounts[IX_INIT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &INIT_ROLES)?;
    require_creatable(&accounts[IX_INIT_EPOCH])?;
    require_system_program(&accounts[IX_INIT_SYSTEM])?;
    let rent = read_rent(&accounts[IX_INIT_RENT])?;
    let now = read_clock_slot(&accounts[IX_INIT_CLOCK])?;
    require(
        opens_slot > now && opens_slot < closes_slot,
        ClutchError::NotActive,
    )?;

    let market = accounts::read_market(&accounts[IX_INIT_MARKET].data.borrow())?;
    let terms = accounts::read_terms(&accounts[IX_INIT_TERMS].data.borrow())?;
    let grid = accounts::read_price_grid(&accounts[IX_INIT_GRID].data.borrow())?;
    require(
        market.market == intent_market
            && market.lifecycle == 0
            && market.outcome_count == 2
            && terms.terms == market.terms
            && terms.realm == market.realm
            && terms.profile == market.profile
            && terms.outcome_count == 2
            && terms.price_grid == grid.grid
            && grid.realm == market.realm,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_INIT_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market.market.bytes()),
        Some(market.stored_bump),
    )?;
    expect_pda(
        accounts[IX_INIT_TERMS].key,
        seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes()),
        Some(terms.stored_bump),
    )?;
    expect_pda(
        accounts[IX_INIT_GRID].key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;

    let epoch_id = clutch_solana_layout::canonical_epoch_id(intent_market, epoch_index);
    read_batch_policy(
        program_id,
        &accounts[IX_INIT_POLICY],
        epoch_id,
        policy_digest,
    )?;
    let (epoch_address, epoch_bump) =
        seeds::epoch_pda(program_id, &intent_market.bytes(), epoch_index);
    expect_pda(
        accounts[IX_INIT_EPOCH].key,
        (epoch_address, epoch_bump),
        None,
    )?;
    let epoch = DirectEpochV3Account::open(
        intent_market,
        terms.terms,
        grid.grid,
        policy_digest,
        epoch_index,
        grid.price_scale,
        opens_slot,
        closes_slot,
        epoch_bump,
    )?;
    create_pda_account(
        program_id,
        &accounts[IX_INIT_PAYER],
        &accounts[IX_INIT_EPOCH],
        &accounts[IX_INIT_SYSTEM],
        &rent,
        DIRECT_EPOCH_BYTES,
        &[
            seeds::SEED_EPOCH,
            &intent_market.bytes(),
            &epoch_index.to_le_bytes(),
            &[epoch_bump],
        ],
    )?;
    epoch.encode(&mut borrow_mut!(accounts[IX_INIT_EPOCH])?)?;
    Ok(())
}

#[inline(never)]
fn freeze_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require_count(accounts, FREEZE_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &FREEZE_ROLES)?;
    let now = read_clock_slot(&accounts[IX_FREEZE_CLOCK])?;
    let mut epoch = accounts::read_direct_epoch(&accounts[IX_FREEZE_EPOCH].data.borrow())?;
    require(
        epoch.common.market == intent_market
            && epoch.common.epoch == intent_epoch
            && epoch.common.phase == EPOCH_PHASE_OPEN
            && now < epoch.submission_opens_slot,
        ClutchError::NotActive,
    )?;
    validate_epoch_address(program_id, &accounts[IX_FREEZE_EPOCH], &epoch)?;
    read_batch_policy(
        program_id,
        &accounts[IX_FREEZE_POLICY],
        epoch.common.epoch,
        epoch.common.policy,
    )?;
    let grid = PriceGridAccount::decode(&accounts[IX_FREEZE_GRID].data.borrow())?;
    validate_grid_address(program_id, &accounts[IX_FREEZE_GRID], &grid)?;
    require(
        grid.grid == epoch.common.price_grid && grid.price_scale == epoch.common.price_scale,
        ClutchError::MismatchedState,
    )?;
    validate_page_address(program_id, &accounts[IX_FREEZE_PAGE])?;
    validate_reservation_address(program_id, &accounts[IX_FREEZE_RES_ZERO])?;
    validate_reservation_address(program_id, &accounts[IX_FREEZE_RES_ONE])?;

    let page_data = accounts[IX_FREEZE_PAGE].data.borrow();
    let orders = load_direct_orders(&page_data, &grid, &epoch.common, false)?;
    let header = stream::verify_page_on_grid(&page_data, &grid)?;
    let (order_set, set_order_count) = stream::frozen_set_commitment(&[&page_data])?;
    drop(page_data);
    let reservation_zero = ReservationAccount::decode(&accounts[IX_FREEZE_RES_ZERO].data.borrow())?;
    let reservation_one = ReservationAccount::decode(&accounts[IX_FREEZE_RES_ONE].data.borrow())?;
    validate_order_reservation(
        &reservation_zero,
        &epoch.common,
        &OrderSlot::Single(orders.zero),
        RESERVATION_STATE_ACTIVE,
    )?;
    validate_order_reservation(
        &reservation_one,
        &epoch.common,
        &OrderSlot::Single(orders.one),
        RESERVATION_STATE_ACTIVE,
    )?;

    epoch.common.phase = EPOCH_PHASE_FROZEN;
    epoch.common.order_set = order_set;
    epoch.common.first_order_id = header.first_order_id;
    epoch.common.last_order_id = header.last_order_id;
    epoch.common.page_count = 1;
    epoch.common.order_count = set_order_count;
    epoch.validate()?;

    stream::seal_page(
        &mut borrow_mut!(accounts[IX_FREEZE_PAGE])?,
        order_set,
        set_order_count,
    )?;
    epoch.encode(&mut borrow_mut!(accounts[IX_FREEZE_EPOCH])?)?;
    Ok(())
}

#[inline(never)]
fn submit_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    outcome_price: u64,
) -> Outcome<()> {
    require_zero_sequence(sequence)?;
    require(
        accounts.len() >= SUBMIT_BASE_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_SUBMIT_PAYER])?;
    require(
        accounts[IX_SUBMIT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;

    let (existing_window, top_count) =
        inspect_submission_window(program_id, &accounts[IX_SUBMIT_WINDOW])?;
    require_count(accounts, SUBMIT_BASE_ACCOUNT_COUNT + top_count)?;
    require_distinct(accounts)?;
    validate_submit_fixed_roles(program_id, accounts)?;
    require_creatable(&accounts[IX_SUBMIT_CANDIDATE])?;
    for account in &accounts[IX_SUBMIT_TOP_START..IX_SUBMIT_TOP_START + top_count] {
        accounts::validate_state_role_lengths(
            program_id,
            account,
            true,
            &[DIRECT_CANDIDATE_ACCOUNT_BYTES],
        )?;
    }
    let system = IX_SUBMIT_TOP_START + top_count;
    let rent_index = system + 1;
    let clock = system + 2;
    require_system_program(&accounts[system])?;
    let rent = read_rent(&accounts[rent_index])?;
    let now = read_clock_slot(&accounts[clock])?;

    let commit = prepare_submission_commit(
        program_id,
        accounts,
        intent_market,
        intent_epoch,
        outcome_price,
        now,
        existing_window,
        top_count,
    )?;

    create_pda_account(
        program_id,
        &accounts[IX_SUBMIT_PAYER],
        &accounts[IX_SUBMIT_CANDIDATE],
        &accounts[system],
        &rent,
        DIRECT_CANDIDATE_ACCOUNT_BYTES,
        &[
            seeds::SEED_DIRECT_CANDIDATE,
            &intent_epoch.bytes(),
            &commit.candidate.candidate_id.0,
            &[commit.candidate.stored_bump],
        ],
    )?;
    if !existing_window {
        create_pda_account(
            program_id,
            &accounts[IX_SUBMIT_PAYER],
            &accounts[IX_SUBMIT_WINDOW],
            &accounts[system],
            &rent,
            DIRECT_WINDOW_ACCOUNT_BYTES,
            &[
                seeds::SEED_DIRECT_WINDOW,
                &intent_epoch.bytes(),
                &[commit.window.stored_bump],
            ],
        )?;
    }
    encode_direct_candidate(
        &commit.candidate,
        &mut borrow_mut!(accounts[IX_SUBMIT_CANDIDATE])?,
    )?;
    encode_direct_window(
        &commit.window,
        &mut borrow_mut!(accounts[IX_SUBMIT_WINDOW])?,
    )?;
    mark_displaced_candidate(
        program_id,
        accounts,
        IX_SUBMIT_TOP_START,
        top_count,
        commit.displaced,
    )?;
    Ok(())
}

#[inline(never)]
fn select_window(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require_zero_sequence(sequence)?;
    require(
        accounts.len() >= SELECT_BASE_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_SELECT_PAYER])?;
    require(
        accounts[IX_SELECT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    let (top_count, closes_slot) = inspect_selected_window(
        program_id,
        &accounts[IX_SELECT_WINDOW],
        intent_market,
        intent_epoch,
    )?;
    require_count(accounts, SELECT_BASE_ACCOUNT_COUNT + top_count)?;
    require_distinct(accounts)?;
    validate_select_fixed_roles(program_id, accounts)?;
    let receipt_index = IX_SELECT_TOP_START + top_count;
    let pot_index = receipt_index + 1;
    let system = receipt_index + 2;
    let rent_index = receipt_index + 3;
    let clock = receipt_index + 4;
    require_creatable(&accounts[receipt_index])?;
    require_creatable(&accounts[pot_index])?;
    require_system_program(&accounts[system])?;
    let rent = read_rent(&accounts[rent_index])?;
    let now = read_clock_slot(&accounts[clock])?;
    require(now >= closes_slot, ClutchError::NotActive)?;
    for account in &accounts[IX_SELECT_TOP_START..IX_SELECT_TOP_START + top_count] {
        accounts::validate_state_role_lengths(
            program_id,
            account,
            true,
            &[DIRECT_CANDIDATE_ACCOUNT_BYTES],
        )?;
    }

    let commit = prepare_selection_commit(
        program_id,
        accounts,
        intent_market,
        intent_epoch,
        top_count,
        now,
        receipt_index,
        pot_index,
    )?;

    create_pda_account(
        program_id,
        &accounts[IX_SELECT_PAYER],
        &accounts[receipt_index],
        &accounts[system],
        &rent,
        account_len::SETTLEMENT_RECEIPT,
        &[
            seeds::SEED_DIRECT_RECEIPT,
            &commit.epoch.common.epoch.bytes(),
            &commit.candidate_id.0,
            &0u16.to_le_bytes(),
            &[commit.receipt.stored_bump],
        ],
    )?;
    create_pda_account(
        program_id,
        &accounts[IX_SELECT_PAYER],
        &accounts[pot_index],
        &accounts[system],
        &rent,
        account_len::FINAL_POT,
        &[
            seeds::SEED_DIRECT_POT,
            &commit.epoch.common.epoch.bytes(),
            &commit.candidate_id.0,
            &[commit.pot.stored_bump],
        ],
    )?;

    commit
        .epoch
        .encode(&mut borrow_mut!(accounts[IX_SELECT_EPOCH])?)?;
    encode_direct_window(
        &commit.window,
        &mut borrow_mut!(accounts[IX_SELECT_WINDOW])?,
    )?;
    write_candidate_statuses(program_id, accounts, IX_SELECT_TOP_START, top_count)?;
    entitle_reservation(&accounts[IX_SELECT_RES_ZERO])?;
    entitle_reservation(&accounts[IX_SELECT_RES_ONE])?;
    commit
        .receipt
        .encode(&mut borrow_mut!(accounts[receipt_index])?)?;
    commit.pot.encode(&mut borrow_mut!(accounts[pot_index])?)?;
    Ok(())
}

#[inline(never)]
fn settle(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<()> {
    require_count(accounts, SETTLE_ACCOUNT_COUNT)?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &SETTLE_ROLES)?;
    let facts =
        authenticate_settlement(program_id, accounts, sequence, intent_market, intent_epoch)?;
    execute_settlement(program_id, accounts, facts)
}

#[inline(never)]
fn authenticate_settlement(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<SettleFacts> {
    let epoch = accounts::read_direct_epoch(&accounts[IX_SETTLE_EPOCH].data.borrow())?;
    require(
        epoch.common.market == intent_market
            && epoch.common.epoch == intent_epoch
            && epoch.common.phase == EPOCH_PHASE_CLEARED,
        ClutchError::NotActive,
    )?;
    validate_epoch_address(program_id, &accounts[IX_SETTLE_EPOCH], &epoch)?;
    let window = decode_direct_window(&accounts[IX_SETTLE_WINDOW].data.borrow())?;
    expect_pda(
        accounts[IX_SETTLE_WINDOW].key,
        seeds::direct_window_pda(program_id, &epoch.common.epoch.bytes()),
        Some(window.stored_bump),
    )?;
    require(
        window.phase == DIRECT_WINDOW_PHASE_SELECTED
            && window.epoch_id.0 == epoch.common.epoch.bytes()
            && window.market_id.0 == epoch.common.market.bytes()
            && window.order_set_id.0 == epoch.common.order_set.bytes()
            && window.policy_id.0 == epoch.common.policy.bytes(),
        ClutchError::NotActive,
    )?;
    let candidate = decode_direct_candidate(&accounts[IX_SETTLE_CANDIDATE].data.borrow())?;
    validate_direct_candidate_address(program_id, &accounts[IX_SETTLE_CANDIDATE], &candidate)?;
    require(
        window.selected_candidate == candidate.candidate_id
            && candidate.status == DIRECT_CANDIDATE_STATUS_SELECTED
            && candidate.epoch_id == window.epoch_id
            && candidate.market_id == window.market_id
            && candidate.order_set_id == window.order_set_id
            && candidate.policy_id == window.policy_id
            && candidate.relation_domain_digest == window.relation_domain_digest,
        ClutchError::NotActive,
    )?;
    validate_page_address(program_id, &accounts[IX_SETTLE_PAGE])?;
    let page = accounts[IX_SETTLE_PAGE].data.borrow();
    let header = stream::verify_page(&page)?;
    require(
        header.market == epoch.common.market
            && header.epoch == epoch.common.epoch
            && header.frozen == 1
            && header.page_index == 0
            && header.page_count == 1
            && header.order_count == 2
            && header.tombstone_count == 0
            && header.order_set == epoch.common.order_set
            && header.set_order_count == epoch.common.order_count,
        ClutchError::MismatchedState,
    )?;
    let mut cursor = stream::OrderSlotCursor::new(&page)?;
    let zero = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let one = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let (zero, one) = match (zero, one) {
        (OrderSlot::Single(zero), OrderSlot::Single(one)) => (zero, one),
        _ => return Err(CodecError::InvalidEnum.into()),
    };
    let buy = if candidate.buy_index == 0 { zero } else { one };
    let sell = if candidate.sell_index == 0 { zero } else { one };
    require(
        candidate.buy_index != candidate.sell_index
            && buy.side == 0
            && sell.side == 1
            && buy.owner != sell.owner
            && buy.outcome == candidate.outcome
            && sell.outcome == candidate.outcome
            && buy.quantity == candidate.quantity
            && sell.quantity == candidate.quantity
            && candidate.fills == [candidate.quantity, candidate.quantity],
        ClutchError::MismatchedState,
    )?;
    let price = candidate.prices[usize::from(candidate.outcome)];
    let consideration_units = u128::from(candidate.quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let scale = u128::from(epoch.common.price_scale);
    require(
        scale != 0 && consideration_units % scale == 0 && price <= buy.limit && price >= sell.limit,
        ClutchError::MismatchedState,
    )?;
    let consideration_atoms = u64::try_from(consideration_units / scale)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let facts = SettleFacts {
        epoch: epoch.common,
        candidate_id: Hash32::from_bytes(candidate.candidate_id.0),
        buy,
        sell,
        outcome: candidate.outcome,
        quantity: candidate.quantity,
        price,
        consideration_units,
        consideration_atoms,
    };
    authenticate_settlement_receipt(program_id, &accounts[IX_SETTLE_RECEIPT], sequence, &facts)?;
    authenticate_settlement_pot(program_id, &accounts[IX_SETTLE_POT], &facts)?;
    Ok(facts)
}

#[inline(never)]
fn authenticate_settlement_receipt(
    program_id: &Pubkey,
    account: &AccountInfo,
    sequence: u64,
    facts: &SettleFacts,
) -> Outcome<()> {
    let receipt = SettlementReceiptAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::direct_receipt_pda(
            program_id,
            &receipt.epoch.bytes(),
            &receipt.candidate.bytes(),
        ),
        Some(receipt.stored_bump),
    )?;
    require(
        sequence == receipt.sequence
            && receipt.epoch == facts.epoch.epoch
            && receipt.market == facts.epoch.market
            && receipt.candidate == facts.candidate_id
            && receipt.buy_order_id == facts.buy.order_id
            && receipt.sell_order_id == facts.sell.order_id
            && receipt.consideration_price_units == facts.consideration_units
            && receipt.quantity == facts.quantity
            && receipt.settled_quantity == 0
            && receipt.price == facts.price
            && receipt.sequence == 1
            && receipt.slice_index == 0
            && receipt.outcome == facts.outcome
            && receipt.leg_kind == RECEIPT_LEG_DIRECT
            && receipt.consumed_flags == 0,
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn authenticate_settlement_pot(
    program_id: &Pubkey,
    account: &AccountInfo,
    facts: &SettleFacts,
) -> Outcome<()> {
    let pot = FinalPotAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::direct_pot_pda(program_id, &pot.epoch.bytes(), &pot.candidate.bytes()),
        Some(pot.stored_bump),
    )?;
    require(
        pot.epoch == facts.epoch.epoch
            && pot.market == facts.epoch.market
            && pot.candidate == facts.candidate_id
            && pot.phase == POT_PHASE_CLOSED
            && pot.pot_internal == [0; MAX_OUTCOMES]
            && pot.pot_cash_price_units == 0
            && pot.rounding_pot_price_units == 0,
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn execute_settlement(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    facts: SettleFacts,
) -> Outcome<()> {
    validate_position_address(program_id, &accounts[IX_SETTLE_BUY_POSITION])?;
    validate_position_address(program_id, &accounts[IX_SETTLE_SELL_POSITION])?;
    validate_reservation_address(program_id, &accounts[IX_SETTLE_BUY_RESERVATION])?;
    validate_reservation_address(program_id, &accounts[IX_SETTLE_SELL_RESERVATION])?;
    let mut buyer = PositionAccount::decode(&accounts[IX_SETTLE_BUY_POSITION].data.borrow())?;
    let mut seller = PositionAccount::decode(&accounts[IX_SETTLE_SELL_POSITION].data.borrow())?;
    let mut buy_reservation =
        ReservationAccount::decode(&accounts[IX_SETTLE_BUY_RESERVATION].data.borrow())?;
    let mut sell_reservation =
        ReservationAccount::decode(&accounts[IX_SETTLE_SELL_RESERVATION].data.borrow())?;
    let mut receipt = SettlementReceiptAccount::decode(&accounts[IX_SETTLE_RECEIPT].data.borrow())?;
    validate_order_reservation(
        &buy_reservation,
        &facts.epoch,
        &OrderSlot::Single(facts.buy),
        RESERVATION_STATE_ENTITLED,
    )?;
    validate_order_reservation(
        &sell_reservation,
        &facts.epoch,
        &OrderSlot::Single(facts.sell),
        RESERVATION_STATE_ENTITLED,
    )?;
    require(
        buyer.market == facts.epoch.market
            && seller.market == facts.epoch.market
            && buyer.owner == facts.buy.owner
            && seller.owner == facts.sell.owner
            && buyer.generation == buy_reservation.position_generation
            && seller.generation == sell_reservation.position_generation
            && buyer.close_state == 0
            && seller.close_state == 0
            && buy_reservation.max_fee_atoms == 0
            && sell_reservation.max_fee_atoms == 0
            && buy_reservation.remaining_cash_atoms >= facts.consideration_atoms
            && sell_reservation.remaining_internal[usize::from(facts.outcome)] == facts.quantity,
        ClutchError::AuthorizationUnavailable,
    )?;
    buyer
        .cash_atoms
        .checked_sub(facts.consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    buyer
        .reserved_cash_atoms
        .checked_sub(buy_reservation.remaining_cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    buyer.internal[usize::from(facts.outcome)]
        .checked_add(facts.quantity)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    seller
        .cash_atoms
        .checked_add(facts.consideration_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let buyer_reserved_release = buy_reservation.remaining_cash_atoms;
    apply_direct_full_slice(
        &mut buyer,
        &mut seller,
        &mut buy_reservation,
        &mut sell_reservation,
        &mut receipt,
        DirectFullSlicePlan {
            slice_index: 0,
            outcome: facts.outcome,
            quantity: facts.quantity,
            consideration_atoms: facts.consideration_atoms,
            buyer_reserved_release,
        },
    );
    buyer.validate()?;
    seller.validate()?;
    buy_reservation.validate()?;
    sell_reservation.validate()?;
    receipt.validate()?;
    buyer.encode(&mut borrow_mut!(accounts[IX_SETTLE_BUY_POSITION])?)?;
    seller.encode(&mut borrow_mut!(accounts[IX_SETTLE_SELL_POSITION])?)?;
    buy_reservation.encode(&mut borrow_mut!(accounts[IX_SETTLE_BUY_RESERVATION])?)?;
    sell_reservation.encode(&mut borrow_mut!(accounts[IX_SETTLE_SELL_RESERVATION])?)?;
    receipt.encode(&mut borrow_mut!(accounts[IX_SETTLE_RECEIPT])?)?;
    Ok(())
}

#[inline(never)]
fn prepare_submission_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent_market: Hash32,
    intent_epoch: Hash32,
    outcome_price: u64,
    now: u64,
) -> Outcome<SubmissionAuthority> {
    let epoch = accounts::read_direct_epoch(&accounts[IX_SUBMIT_EPOCH].data.borrow())?;
    require(
        epoch.common.market == intent_market
            && epoch.common.epoch == intent_epoch
            && epoch.common.phase == EPOCH_PHASE_FROZEN,
        ClutchError::NotActive,
    )?;
    validate_epoch_address(program_id, &accounts[IX_SUBMIT_EPOCH], &epoch)?;
    let policy = read_batch_policy(
        program_id,
        &accounts[IX_SUBMIT_POLICY],
        epoch.common.epoch,
        epoch.common.policy,
    )?;
    let domain = full_domain(&epoch, policy)?;
    let orders = authenticate_direct_source(
        program_id,
        accounts,
        IX_SUBMIT_GRID,
        IX_SUBMIT_PAGE,
        IX_SUBMIT_RES_ZERO,
        IX_SUBMIT_RES_ONE,
        &epoch.common,
        RESERVATION_STATE_ACTIVE,
    )?;
    let mut prices = [0u64; MAX_OUTCOMES];
    let complement = epoch
        .common
        .price_scale
        .checked_sub(outcome_price)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    prices[usize::from(orders.outcome)] = outcome_price;
    prices[1usize - usize::from(orders.outcome)] = complement;
    require_grid_ticks(
        program_id,
        &accounts[IX_SUBMIT_GRID],
        outcome_price,
        complement,
    )?;
    let candidate_id =
        clutch_batch_policy_identity::direct_window_v1::canonical_account_candidate_id(
            domain.epoch_id,
            domain.market_id,
            &prices,
        );
    let derived =
        seeds::direct_candidate_pda(program_id, &epoch.common.epoch.bytes(), &candidate_id.0);
    expect_pda(accounts[IX_SUBMIT_CANDIDATE].key, derived, None)?;
    let candidate = verify_direct_two_order_candidate(
        &domain,
        DirectTwoOrderInputV1 {
            prices,
            buy_limit: orders.buy_limit,
            sell_limit: orders.sell_limit,
            quantity: orders.quantity,
            submitted_slot: now,
            buy_index: orders.buy_index,
            sell_index: orders.sell_index,
            outcome: orders.outcome,
            stored_bump: derived.1,
        },
    )
    .map_err(map_window)?;
    Ok(SubmissionAuthority {
        candidate,
        opens_slot: epoch.submission_opens_slot,
        closes_slot: epoch.submission_closes_slot,
    })
}

#[inline(never)]
fn inspect_submission_window(program_id: &Pubkey, account: &AccountInfo) -> Outcome<(bool, usize)> {
    if account.owner == program_id {
        accounts::validate_state_role_lengths(
            program_id,
            account,
            true,
            &[DIRECT_WINDOW_ACCOUNT_BYTES],
        )?;
        let window = decode_direct_window(&account.data.borrow())?;
        Ok((true, usize::from(window.top_count)))
    } else {
        require_creatable(account)?;
        Ok((false, 0))
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare_submission_commit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent_market: Hash32,
    intent_epoch: Hash32,
    outcome_price: u64,
    now: u64,
    existing_window: bool,
    top_count: usize,
) -> Outcome<SubmissionCommit> {
    let authority = prepare_submission_candidate(
        program_id,
        accounts,
        intent_market,
        intent_epoch,
        outcome_price,
        now,
    )?;
    let mut candidate = authority.candidate;
    let derived = seeds::direct_window_pda(program_id, &intent_epoch.bytes());
    if !existing_window {
        expect_pda(accounts[IX_SUBMIT_WINDOW].key, derived, None)?;
        let window = DirectCandidateWindowV1::first(
            DirectWindowBindingV1 {
                epoch_id: candidate.epoch_id,
                market_id: candidate.market_id,
                order_set_id: candidate.order_set_id,
                policy_id: candidate.policy_id,
                relation_domain_digest: candidate.relation_domain_digest,
                opens_slot: authority.opens_slot,
                closes_slot: authority.closes_slot,
            },
            &candidate,
            now,
            derived.1,
        )
        .map_err(map_window)?;
        return Ok(SubmissionCommit {
            candidate,
            window,
            displaced: Identity32V1::ZERO,
        });
    }
    let current = decode_direct_window(&accounts[IX_SUBMIT_WINDOW].data.borrow())?;
    expect_pda(
        accounts[IX_SUBMIT_WINDOW].key,
        derived,
        Some(current.stored_bump),
    )?;
    require(
        usize::from(current.top_count) == top_count
            && current.opens_slot == authority.opens_slot
            && current.closes_slot == authority.closes_slot,
        ClutchError::MismatchedState,
    )?;
    let mut retained = [DirectRetainedCandidateV1::ZERO; MAX_DIRECT_CANDIDATES];
    load_retained_registry(
        program_id,
        accounts,
        IX_SUBMIT_TOP_START,
        &current,
        &mut retained[..top_count],
    )?;
    let plan = plan_admission_retained(&current, &retained[..top_count], &candidate, now)
        .map_err(map_window)?;
    candidate.status = plan.submitted_status;
    Ok(SubmissionCommit {
        candidate,
        window: plan.post_window,
        displaced: plan.displaced_candidate,
    })
}

#[inline(never)]
fn require_grid_ticks(
    program_id: &Pubkey,
    account: &AccountInfo,
    first: u64,
    second: u64,
) -> Outcome<()> {
    let grid = PriceGridAccount::decode(&account.data.borrow())?;
    validate_grid_address(program_id, account, &grid)?;
    grid.tick_of(first)?;
    grid.tick_of(second)?;
    Ok(())
}

// The indices are compile-time routing coordinates for the same authenticated
// source shape; grouping them would only hide the fixed ABI behind another
// aggregate copied into the SBF frame.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_direct_source(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    grid_index: usize,
    page_index: usize,
    reservation_zero_index: usize,
    reservation_one_index: usize,
    epoch: &clutch_solana_layout::EpochAccount,
    reservation_state: u8,
) -> Outcome<DirectOrders> {
    let grid = PriceGridAccount::decode(&accounts[grid_index].data.borrow())?;
    validate_grid_address(program_id, &accounts[grid_index], &grid)?;
    require(
        grid.grid == epoch.price_grid && grid.price_scale == epoch.price_scale,
        ClutchError::MismatchedState,
    )?;
    validate_page_address(program_id, &accounts[page_index])?;
    validate_reservation_address(program_id, &accounts[reservation_zero_index])?;
    validate_reservation_address(program_id, &accounts[reservation_one_index])?;
    let orders = load_direct_orders(&accounts[page_index].data.borrow(), &grid, epoch, true)?;
    authenticate_one_reservation(
        &accounts[reservation_zero_index],
        epoch,
        orders.zero,
        reservation_state,
    )?;
    authenticate_one_reservation(
        &accounts[reservation_one_index],
        epoch,
        orders.one,
        reservation_state,
    )?;
    Ok(orders)
}

#[inline(never)]
fn authenticate_one_reservation(
    account: &AccountInfo,
    epoch: &clutch_solana_layout::EpochAccount,
    order: OrderRecord,
    state: u8,
) -> Outcome<()> {
    let reservation = ReservationAccount::decode(&account.data.borrow())?;
    validate_order_reservation(&reservation, epoch, &OrderSlot::Single(order), state)
}

#[inline(never)]
fn load_retained_registry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    start: usize,
    window: &DirectCandidateWindowV1,
    out: &mut [DirectRetainedCandidateV1],
) -> Outcome<()> {
    require(
        out.len() == usize::from(window.top_count),
        ClutchError::MismatchedState,
    )?;
    let mut index = 0usize;
    while index < out.len() {
        let candidate = decode_direct_candidate(&accounts[start + index].data.borrow())?;
        validate_direct_candidate_address(program_id, &accounts[start + index], &candidate)?;
        require(
            candidate.epoch_id == window.epoch_id
                && candidate.market_id == window.market_id
                && candidate.order_set_id == window.order_set_id
                && candidate.policy_id == window.policy_id
                && candidate.relation_domain_digest == window.relation_domain_digest
                && candidate.status == DIRECT_CANDIDATE_STATUS_VERIFIED
                && candidate.entry() == window.top[index],
            ClutchError::MismatchedState,
        )?;
        // `decode_direct_candidate` validates the complete fixed-layout value,
        // including its content-derived candidate identity.  The PDA check and
        // exact full-width Window binding above authenticate the persisted
        // Submit result.  Compact the already-authenticated ranking fields here;
        // Select independently re-executes the complete relation for every
        // retained candidate before it can become settlement authority.
        out[index] = DirectRetainedCandidateV1 {
            entry: candidate.entry(),
            score: candidate.score(),
        };
        index += 1;
    }
    Ok(())
}

#[inline(never)]
fn mark_displaced_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    start: usize,
    count: usize,
    displaced: Identity32V1,
) -> Outcome<()> {
    if displaced.is_zero() {
        return Ok(());
    }
    require(count == MAX_DIRECT_CANDIDATES, ClutchError::MismatchedState)?;
    // A nonzero displacement from `plan_admission_retained` can only be the
    // old registry's exact worst entry. `load_retained_registry` already
    // authenticated every full entry and score in best-to-worst order, so the
    // plan closes the index as well as the identity. Decode only that one
    // 440-byte authority account instead of rescanning all three.
    let index = start + count - 1;
    let mut candidate = decode_direct_candidate(&accounts[index].data.borrow())?;
    validate_direct_candidate_address(program_id, &accounts[index], &candidate)?;
    require(
        candidate.candidate_id == displaced && candidate.status == DIRECT_CANDIDATE_STATUS_VERIFIED,
        ClutchError::MismatchedState,
    )?;
    candidate.status = DIRECT_CANDIDATE_STATUS_SUPERSEDED;
    candidate.validate().map_err(map_window)?;
    encode_direct_candidate(&candidate, &mut borrow_mut!(accounts[index])?)?;
    Ok(())
}

#[inline(never)]
fn authenticate_selection_source(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<SelectionSource> {
    let window = decode_direct_window(&accounts[IX_SELECT_WINDOW].data.borrow())?;
    let epoch = accounts::read_direct_epoch(&accounts[IX_SELECT_EPOCH].data.borrow())?;
    require(
        epoch.common.market == intent_market
            && epoch.common.epoch == intent_epoch
            && epoch.common.phase == EPOCH_PHASE_FROZEN,
        ClutchError::NotActive,
    )?;
    validate_epoch_address(program_id, &accounts[IX_SELECT_EPOCH], &epoch)?;
    let policy = read_batch_policy(
        program_id,
        &accounts[IX_SELECT_POLICY],
        epoch.common.epoch,
        epoch.common.policy,
    )?;
    let domain = full_domain(&epoch, policy)?;
    require(
        window.epoch_id == domain.epoch_id
            && window.market_id == domain.market_id
            && window.order_set_id == domain.order_set_id
            && window.policy_id == domain.policy_id
            && window.opens_slot == epoch.submission_opens_slot
            && window.closes_slot == epoch.submission_closes_slot
            && window.relation_domain_digest.0
                == domain
                    .digest()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .0,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_SELECT_WINDOW].key,
        seeds::direct_window_pda(program_id, &epoch.common.epoch.bytes()),
        Some(window.stored_bump),
    )?;
    let orders = authenticate_direct_source(
        program_id,
        accounts,
        IX_SELECT_GRID,
        IX_SELECT_PAGE,
        IX_SELECT_RES_ZERO,
        IX_SELECT_RES_ONE,
        &epoch.common,
        RESERVATION_STATE_ACTIVE,
    )?;
    Ok(SelectionSource { epoch, orders })
}

#[inline(never)]
fn inspect_selected_window(
    program_id: &Pubkey,
    account: &AccountInfo,
    intent_market: Hash32,
    intent_epoch: Hash32,
) -> Outcome<(usize, u64)> {
    accounts::validate_state_role_lengths(
        program_id,
        account,
        true,
        &[DIRECT_WINDOW_ACCOUNT_BYTES],
    )?;
    let window = decode_direct_window(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::direct_window_pda(program_id, &intent_epoch.bytes()),
        Some(window.stored_bump),
    )?;
    require(
        window.epoch_id.0 == intent_epoch.bytes()
            && window.market_id.0 == intent_market.bytes()
            && window.phase == DIRECT_WINDOW_PHASE_OPEN,
        ClutchError::NotActive,
    )?;
    Ok((usize::from(window.top_count), window.closes_slot))
}

#[inline(never)]
fn prepare_verified_selected_window(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    source: &SelectionSource,
    top_count: usize,
    now: u64,
) -> Outcome<DirectCandidateWindowV1> {
    let domain = ValidatedDirectDomainV1::new(&full_domain(&source.epoch, DIRECT_POLICY_V1)?)
        .map_err(map_window)?;
    let window = decode_direct_window(&accounts[IX_SELECT_WINDOW].data.borrow())?;
    require(
        usize::from(window.top_count) == top_count,
        ClutchError::MismatchedState,
    )?;
    let grid = PriceGridAccount::decode(&accounts[IX_SELECT_GRID].data.borrow())?;
    let mut retained = [DirectRetainedCandidateV1::ZERO; MAX_DIRECT_CANDIDATES];
    let mut index = 0usize;
    while index < top_count {
        let candidate =
            decode_direct_candidate(&accounts[IX_SELECT_TOP_START + index].data.borrow())?;
        validate_direct_candidate_address(
            program_id,
            &accounts[IX_SELECT_TOP_START + index],
            &candidate,
        )?;
        require(
            candidate.epoch_id == window.epoch_id
                && candidate.market_id == window.market_id
                && candidate.order_set_id == window.order_set_id
                && candidate.policy_id == window.policy_id
                && candidate.relation_domain_digest == window.relation_domain_digest
                && candidate.status == DIRECT_CANDIDATE_STATUS_VERIFIED
                && candidate.entry() == window.top[index]
                && candidate.submitted_slot >= window.opens_slot
                && candidate.submitted_slot < window.closes_slot,
            ClutchError::MismatchedState,
        )?;
        // The exact Grid account was authenticated while loading `source` and
        // no CPI or mutation occurs between that check and this loop.
        grid.tick_of(candidate.prices[0])?;
        grid.tick_of(candidate.prices[1])?;
        domain
            .reverify_decoded_candidate(
                &candidate,
                DirectTwoOrderInputV1 {
                    prices: candidate.prices,
                    buy_limit: source.orders.buy_limit,
                    sell_limit: source.orders.sell_limit,
                    quantity: source.orders.quantity,
                    submitted_slot: candidate.submitted_slot,
                    buy_index: source.orders.buy_index,
                    sell_index: source.orders.sell_index,
                    outcome: source.orders.outcome,
                    stored_bump: candidate.stored_bump,
                },
            )
            .map_err(map_window)?;
        retained[index] = DirectRetainedCandidateV1 {
            entry: candidate.entry(),
            score: candidate.score(),
        };
        index += 1;
    }
    Ok(
        plan_selection_retained(&window, &retained[..top_count], now)
            .map_err(map_window)?
            .post_window,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare_selection_commit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    intent_market: Hash32,
    intent_epoch: Hash32,
    top_count: usize,
    now: u64,
    receipt_index: usize,
    pot_index: usize,
) -> Outcome<SelectionCommit> {
    let source = authenticate_selection_source(program_id, accounts, intent_market, intent_epoch)?;
    let window = prepare_verified_selected_window(program_id, accounts, &source, top_count, now)?;
    let selected = selected_facts(program_id, &accounts[IX_SELECT_TOP_START])?;
    require(
        selected.candidate_id == window.selected_candidate
            && selected.buy_index == source.orders.buy_index
            && selected.sell_index == source.orders.sell_index
            && selected.outcome == source.orders.outcome
            && selected.quantity == source.orders.quantity,
        ClutchError::MismatchedState,
    )?;
    let receipt_derived = seeds::direct_receipt_pda(
        program_id,
        &source.epoch.common.epoch.bytes(),
        &selected.candidate_id.0,
    );
    let pot_derived = seeds::direct_pot_pda(
        program_id,
        &source.epoch.common.epoch.bytes(),
        &selected.candidate_id.0,
    );
    expect_pda(accounts[receipt_index].key, receipt_derived, None)?;
    expect_pda(accounts[pot_index].key, pot_derived, None)?;
    let buy = source.orders.buy();
    let sell = source.orders.sell();
    let price = selected.prices[usize::from(selected.outcome)];
    let consideration = u128::from(selected.quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        consideration % u128::from(source.epoch.common.price_scale) == 0,
        ClutchError::MismatchedState,
    )?;
    let receipt = SettlementReceiptAccount {
        epoch: source.epoch.common.epoch,
        market: source.epoch.common.market,
        candidate: Hash32::from_bytes(selected.candidate_id.0),
        buy_order_id: buy.order_id,
        sell_order_id: sell.order_id,
        consideration_price_units: consideration,
        quantity: selected.quantity,
        settled_quantity: 0,
        price,
        sequence: 1,
        slice_index: 0,
        outcome: selected.outcome,
        leg_kind: RECEIPT_LEG_DIRECT,
        consumed_flags: 0,
        stored_bump: receipt_derived.1,
        flags: 0,
    };
    receipt.validate()?;
    let pot = FinalPotAccount {
        epoch: source.epoch.common.epoch,
        market: source.epoch.common.market,
        candidate: Hash32::from_bytes(selected.candidate_id.0),
        pot_internal: [0; MAX_OUTCOMES],
        pot_cash_price_units: 0,
        rounding_pot_price_units: 0,
        outcome_count: 2,
        phase: POT_PHASE_CLOSED,
        stored_bump: pot_derived.1,
        flags: 0,
    };
    pot.validate()?;
    let mut epoch = source.epoch;
    epoch.common.phase = EPOCH_PHASE_CLEARED;
    epoch.validate()?;
    Ok(SelectionCommit {
        epoch,
        window,
        candidate_id: selected.candidate_id,
        receipt,
        pot,
    })
}

#[inline(never)]
fn selected_facts(program_id: &Pubkey, account: &AccountInfo) -> Outcome<SelectedFacts> {
    let candidate = decode_direct_candidate(&account.data.borrow())?;
    validate_direct_candidate_address(program_id, account, &candidate)?;
    require(
        candidate.status == DIRECT_CANDIDATE_STATUS_VERIFIED,
        ClutchError::NotActive,
    )?;
    Ok(SelectedFacts {
        candidate_id: candidate.candidate_id,
        prices: candidate.prices,
        quantity: candidate.quantity,
        buy_index: candidate.buy_index,
        sell_index: candidate.sell_index,
        outcome: candidate.outcome,
    })
}

#[inline(never)]
fn write_candidate_statuses(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    start: usize,
    count: usize,
) -> Outcome<()> {
    let mut index = 0usize;
    while index < count {
        let mut candidate = decode_direct_candidate(&accounts[start + index].data.borrow())?;
        validate_direct_candidate_address(program_id, &accounts[start + index], &candidate)?;
        require(
            candidate.status == DIRECT_CANDIDATE_STATUS_VERIFIED,
            ClutchError::NotActive,
        )?;
        candidate.status = if index == 0 {
            DIRECT_CANDIDATE_STATUS_SELECTED
        } else {
            DIRECT_CANDIDATE_STATUS_SUPERSEDED
        };
        candidate.validate().map_err(map_window)?;
        encode_direct_candidate(&candidate, &mut borrow_mut!(accounts[start + index])?)?;
        index += 1;
    }
    Ok(())
}

#[inline(never)]
fn entitle_reservation(account: &AccountInfo) -> Outcome<()> {
    let mut reservation = ReservationAccount::decode(&account.data.borrow())?;
    require(
        reservation.state == RESERVATION_STATE_ACTIVE,
        ClutchError::NotActive,
    )?;
    reservation.state = RESERVATION_STATE_ENTITLED;
    reservation.validate()?;
    reservation.encode(&mut borrow_mut!(account)?)?;
    Ok(())
}

fn validate_submit_fixed_roles(program_id: &Pubkey, accounts: &[AccountInfo]) -> Outcome<()> {
    for (index, len) in [
        (IX_SUBMIT_EPOCH, DIRECT_EPOCH_BYTES),
        (IX_SUBMIT_POLICY, BATCH_POLICY_BYTES),
        (IX_SUBMIT_GRID, account_len::PRICE_GRID),
        (IX_SUBMIT_PAGE, account_len::ORDER_PAGE),
        (IX_SUBMIT_RES_ZERO, RESERVATION_ACCOUNT_BYTES),
        (IX_SUBMIT_RES_ONE, RESERVATION_ACCOUNT_BYTES),
    ] {
        accounts::validate_state_role_lengths(program_id, &accounts[index], false, &[len])?;
    }
    Ok(())
}

fn validate_select_fixed_roles(program_id: &Pubkey, accounts: &[AccountInfo]) -> Outcome<()> {
    for (index, len, writable) in [
        (IX_SELECT_EPOCH, DIRECT_EPOCH_BYTES, true),
        (IX_SELECT_POLICY, BATCH_POLICY_BYTES, false),
        (IX_SELECT_GRID, account_len::PRICE_GRID, false),
        (IX_SELECT_PAGE, account_len::ORDER_PAGE, false),
        (IX_SELECT_RES_ZERO, RESERVATION_ACCOUNT_BYTES, true),
        (IX_SELECT_RES_ONE, RESERVATION_ACCOUNT_BYTES, true),
    ] {
        accounts::validate_state_role_lengths(program_id, &accounts[index], writable, &[len])?;
    }
    Ok(())
}

#[inline(never)]
fn read_batch_policy(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: Hash32,
    expected_digest: Hash32,
) -> Outcome<FrozenPolicyV1> {
    accounts::validate_state_role_lengths(program_id, account, false, &[BATCH_POLICY_BYTES])?;
    let policy = decode_batch_policy(&account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let digest =
        batch_policy_digest(&policy).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        policy == DIRECT_POLICY_V1 && digest.0 == expected_digest.bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    expect_pda(
        account.key,
        seeds::batch_policy_pda(program_id, &epoch.bytes(), &expected_digest.bytes()),
        None,
    )?;
    Ok(policy)
}

pub(in crate::instructions) fn full_domain(
    epoch: &DirectEpochV3Account,
    policy: FrozenPolicyV1,
) -> Outcome<FullRelationDomainV1> {
    let domain = FullRelationDomainV1 {
        relation_version: epoch.common.relation_version,
        market_id: Identity32V1(epoch.common.market.bytes()),
        book_id: Identity32V1(epoch.common.book.bytes()),
        epoch_id: Identity32V1(epoch.common.epoch.bytes()),
        policy_id: Identity32V1(epoch.common.policy.bytes()),
        order_set_id: Identity32V1(epoch.common.order_set.bytes()),
        epoch_index: epoch.common.epoch_index,
        outcome_count: epoch.common.outcome_count,
        owner_count: epoch.common.owner_count,
        price_scale: epoch.common.price_scale,
        remainder_seed: epoch.common.remainder_seed,
        policy,
    };
    domain
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(domain)
}

pub(in crate::instructions) fn load_direct_orders(
    page: &[u8],
    grid: &PriceGridAccount,
    epoch: &clutch_solana_layout::EpochAccount,
    frozen: bool,
) -> Outcome<DirectOrders> {
    let header = stream::verify_page_on_grid(page, grid)?;
    require(
        header.market == epoch.market
            && header.epoch == epoch.epoch
            && header.page_index == 0
            && header.page_count == 1
            && header.order_count == 2
            && header.tombstone_count == 0
            && header.frozen == u8::from(frozen)
            && (!frozen
                || (header.order_set == epoch.order_set
                    && header.set_order_count == epoch.order_count)),
        ClutchError::MismatchedState,
    )?;
    let mut cursor = stream::OrderSlotCursor::new(page)?;
    let zero = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let one = cursor
        .next_slot()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))??;
    let (zero, one) = match (zero, one) {
        (OrderSlot::Single(zero), OrderSlot::Single(one)) => (zero, one),
        _ => return Err(CodecError::InvalidEnum.into()),
    };
    let (buy_index, sell_index, buy, sell) = match (zero.side, one.side) {
        (0, 1) => (0, 1, zero, one),
        (1, 0) => (1, 0, one, zero),
        _ => return Err(ClutchError::MismatchedState.into()),
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
            && zero.expiry_epoch >= epoch.epoch_index
            && one.expiry_epoch >= epoch.epoch_index
            && buy.limit >= sell.limit,
        ClutchError::MismatchedState,
    )?;
    Ok(DirectOrders {
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

pub(in crate::instructions) fn validate_order_reservation(
    reservation: &ReservationAccount,
    epoch: &clutch_solana_layout::EpochAccount,
    order: &OrderSlot,
    state: u8,
) -> Outcome<()> {
    reservation.validate()?;
    let plan = ReservationPlan::for_order(order, epoch.outcome_count, epoch.price_scale, 0)?;
    require(
        reservation.state == state
            && reservation.market == epoch.market
            && reservation.epoch == epoch.epoch
            && reservation.owner == order.owner()
            && reservation.order_id == order.order_id()
            && reservation.order_generation == order.generation()
            && reservation.page_index == 0
            && reservation.terms == epoch.terms
            && reservation.price_grid == epoch.price_grid
            && reservation.policy == epoch.policy
            && reservation.outcome_count == epoch.outcome_count
            && reservation.max_fee_atoms == 0
            && reservation.release_generation == 0
            && reservation.initial_cash_atoms == plan.cash_atoms
            && reservation.remaining_cash_atoms == plan.cash_atoms
            && reservation.initial_internal == plan.internal
            && reservation.remaining_internal == plan.internal
            && reservation.order_kind == plan.order_kind
            && reservation.side == plan.side,
        ClutchError::MismatchedState,
    )
}

fn validate_epoch_address(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &DirectEpochV3Account,
) -> Outcome<()> {
    expect_pda(
        account.key,
        seeds::epoch_pda(
            program_id,
            &epoch.common.market.bytes(),
            epoch.common.epoch_index,
        ),
        Some(epoch.common.stored_bump),
    )
}

fn validate_grid_address(
    program_id: &Pubkey,
    account: &AccountInfo,
    grid: &PriceGridAccount,
) -> Outcome<()> {
    expect_pda(
        account.key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )
}

fn validate_page_address(program_id: &Pubkey, account: &AccountInfo) -> Outcome<()> {
    let header = stream::OrderPageHeader::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::page_pda(program_id, &header.epoch.bytes(), header.page_index),
        Some(header.stored_bump),
    )
}

fn validate_reservation_address(program_id: &Pubkey, account: &AccountInfo) -> Outcome<()> {
    let reservation = ReservationAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::reservation_pda(program_id, &reservation.reservation.bytes()),
        Some(reservation.stored_bump),
    )
}

fn validate_direct_candidate_address(
    program_id: &Pubkey,
    account: &AccountInfo,
    candidate: &DirectCandidateV2,
) -> Outcome<()> {
    expect_pda(
        account.key,
        seeds::direct_candidate_pda(program_id, &candidate.epoch_id.0, &candidate.candidate_id.0),
        Some(candidate.stored_bump),
    )
}

fn validate_position_address(program_id: &Pubkey, account: &AccountInfo) -> Outcome<()> {
    let position = PositionAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::position_pda(
            program_id,
            &position.market.bytes(),
            &position.owner.bytes(),
        ),
        Some(position.stored_bump),
    )
}
