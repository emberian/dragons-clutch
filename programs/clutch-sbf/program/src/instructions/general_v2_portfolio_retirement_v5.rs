//! Isolated General action 44: retire one complete portfolio archive set.
//!
//! The route remains deliberately unregistered. Once action 43 has been
//! allocated, shared dispatch may pass the fresh action-44 enum value here.
//! This handler authenticates the counted SettlementRoot, retained sealed
//! Feed, MarketBinding neutral sink, both consumed Reservation V9 endpoints,
//! both Position V3/GEN1 pairs, the complete committed Receipt V5 prefix, and
//! the sorted unique persisted refund-owner suffix. It then applies the one
//! pure retirement plan with direct program-owned lamport mutation only: no
//! CPI, mint, burn, token transfer, or external lamport debit occurs.

use core::cell::Ref;
use std::boxed::Box;

use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
};

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{
    decode_portfolio_settlement_payload_v1, GeneralPositionReplayPrestateV1, Id32,
    PortfolioSettlementPayloadV1, RetirePortfolioPairArchivesPayloadV1, Sha256BackendV1,
};
use clutch_general_v2_runtime::{
    prepare_retire_portfolio_pair_archives_v2, PortfolioArchiveReceiptInputV2,
    PortfolioArchiveRefundOwnerInputV2, PortfolioArchiveReservationInputV2,
    PortfolioPairArchiveTerminalReceiptV2, RetirePortfolioPairArchivesInputV2,
    PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2, PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_retirement::{
    PositionLifecycleV3, PositionPurposeV3, PositionV3Sha256Backend, ReplayV3HashBackend,
    POSITION_V3_BYTES,
};
use clutch_solana_layout::registry::GeneralV2Action;
use clutch_solana_layout::reservation_v9::{ReservationAccountV9, RESERVATION_ACCOUNT_BYTES_V9};
use clutch_solana_layout::settlement_receipt_v5::{
    SettlementReceiptAccountV5, SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;

use super::general_market_current_v5::{
    authenticate_general_market_current_prefix_v5, AuthenticatedGeneralMarketCurrentV5,
    CURRENT_V5_IX_MARKET_BINDING, CURRENT_V5_IX_MARKET_RUNTIME, CURRENT_V5_IX_REALM,
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5,
};
use super::general_v2_position_replay::authenticate_current_general_position_replay_v5;
use super::general_v2_settlement_root::{
    authenticate_writable_general_settlement_root_epoch_v1,
    AuthenticatedGeneralSettlementRootV1,
};

/// Fixed accounts before committed Receipt and refund-owner suffixes.
pub const PORTFOLIO_ARCHIVE_FIXED_ACCOUNTS_V2: usize = 10;
/// Minimum frame: fixed accounts, one Receipt, and one refund owner.
pub const PORTFOLIO_ARCHIVE_MIN_ACCOUNTS_V2: usize = 12;
/// Maximum frozen frame: ten fixed, sixteen Receipts, eighteen refund owners.
pub const PORTFOLIO_ARCHIVE_MAX_ACCOUNTS_V2: usize = 44;

/// Current V5 prefix plus Realm collateral Profile/Policy/Token and nine
/// unique archive roles before the variable Receipt/refund suffix.
pub const PORTFOLIO_ARCHIVE_CURRENT_FIXED_ACCOUNTS_V5: usize =
    GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5 + 3 + 9;
pub const PORTFOLIO_ARCHIVE_CURRENT_MIN_ACCOUNTS_V5: usize =
    PORTFOLIO_ARCHIVE_CURRENT_FIXED_ACCOUNTS_V5 + 2;
/// Solana messages expose at most 64 account indices to one instruction. The
/// independent receipt/refund maxima remain 16/18, while their exact combined
/// width is bounded by this physical transaction ceiling.
pub const PORTFOLIO_ARCHIVE_CURRENT_MAX_ACCOUNTS_V5: usize = 64;

const CURRENT_IX_PROFILE: usize = GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5;
const CURRENT_IX_COLLATERAL_POLICY: usize = CURRENT_IX_PROFILE + 1;
const CURRENT_IX_TOKEN_PROGRAM: usize = CURRENT_IX_COLLATERAL_POLICY + 1;
const CURRENT_IX_ARCHIVE_SUFFIX: usize = CURRENT_IX_TOKEN_PROGRAM + 1;

pub const IX_SETTLEMENT_ROOT: usize = 0;
pub const IX_RETAINED_FEED: usize = 1;
pub const IX_MARKET_BINDING: usize = 2;
pub const IX_NEUTRAL_SINK: usize = 3;
pub const IX_BUYER_RESERVATION_V9: usize = 4;
pub const IX_SELLER_RESERVATION_V9: usize = 5;
pub const IX_BUYER_POSITION_V3: usize = 6;
pub const IX_SELLER_POSITION_V3: usize = 7;
pub const IX_BUYER_REPLAY_GEN1: usize = 8;
pub const IX_SELLER_REPLAY_GEN1: usize = 9;
pub const IX_FIRST_RECEIPT_V5: usize = 10;

#[derive(Clone, Copy, Debug)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; contract::ID_BYTES] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

fn id(key: &Pubkey) -> Id32 { Id32::from_bytes(key.to_bytes()) }

fn borrow_data<'a, 'b>(account: &'a AccountInfo<'b>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn require_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    exact_len: Option<usize>,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable { ClutchError::NotWritable } else { ClutchError::UnexpectedWritable },
    )?;
    if let Some(len) = exact_len {
        require(account.data_len() == len, ClutchError::WrongDataLength)?;
    }
    Ok(())
}

fn require_credit_account(account: &AccountInfo<'_>) -> Outcome<()> {
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(account.is_writable, ClutchError::NotWritable)
}

fn require_distinct_accounts(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
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

fn account_frame(
    account_count: usize,
    receipt_count: u8,
    refund_owner_count: u8,
) -> Outcome<(usize, usize, usize)> {
    let receipts = usize::from(receipt_count);
    let refund_owners = usize::from(refund_owner_count);
    if !(1..=PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2).contains(&receipts)
        || !(1..=PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2).contains(&refund_owners)
    {
        return Err(Refusal::Adapter(ClutchError::AccountCount));
    }
    let first_refund_owner = IX_FIRST_RECEIPT_V5
        .checked_add(receipts)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected = first_refund_owner
        .checked_add(refund_owners)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(account_count == expected, ClutchError::AccountCount)?;
    Ok((receipts, refund_owners, first_refund_owner))
}

fn authenticate_position_replay(
    program_id: &Pubkey,
    current: &AuthenticatedGeneralMarketCurrentV5,
    bound: BoundCollateralProfileV2,
    root: contract::SettlementRootV1AccountV1,
    owner: [u8; 32],
    binding_account: &AccountInfo<'_>,
    runtime_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
) -> Outcome<(AuthenticatedPositionV3, GeneralPositionReplayPrestateV1)> {
    let authority = authenticate_current_general_position_replay_v5(
        program_id,
        current,
        bound,
        binding_account,
        runtime_account,
        position_account,
        replay_account,
        owner,
    )?;
    require(
        authority.market_binding == *current.binding()
            && authority.market_runtime == *current.runtime()
            && authority.position.semantic.fields().market_instance_id.bytes()
                == root.market_instance_v2_id().bytes()
            && authority.position.semantic.fields().purpose == PositionPurposeV3::General
            && authority.position.semantic.fields().lifecycle == PositionLifecycleV3::Open
            && authority.position.semantic.fields().outcome_count == root.outcome_count(),
        ClutchError::MismatchedState,
    )?;
    Ok((authority.position, authority.replay))
}

fn authenticate_current_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    current: &AuthenticatedGeneralMarketCurrentV5,
) -> Outcome<BoundCollateralProfileV2> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        &accounts[CURRENT_V5_IX_REALM],
        &accounts[CURRENT_IX_PROFILE],
        &accounts[CURRENT_IX_COLLATERAL_POLICY],
        &accounts[CURRENT_IX_TOKEN_PROGRAM],
    )?;
    let base = current.binding().base();
    let instance = current.market_instance();
    require(
        current.binding_account() == *accounts[CURRENT_V5_IX_MARKET_BINDING].key
            && current.runtime_account() == *accounts[CURRENT_V5_IX_MARKET_RUNTIME].key
            && current.revenue().realm().bytes() == realm.realm().realm.bytes()
            && current.collateral_profile_id().bytes() == realm.realm().profile.bytes()
            && current.runtime().market_instance_v2_id == base.base().market_instance_v2_id,
        ClutchError::MismatchedState,
    )?;
    let market = base.base().market_instance_v2_id.bytes();
    refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market),
            realm: CollateralId::from_bytes(realm.realm().realm.bytes()),
            profile: CollateralId::from_bytes(realm.realm().profile.bytes()),
            collateral_cap_atoms: instance.collateral_cap,
            hoard_authority: CollateralId::from_bytes(
                seeds::hoard_authority_v2_pda(program_id, &market).0.to_bytes(),
            ),
            hoard_token_account: CollateralId::from_bytes(
                seeds::hoard_token_v2_pda(program_id, &market).0.to_bytes(),
            ),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn current_account_frame(
    accounts: &[AccountInfo<'_>],
    receipt_count: u8,
    refund_owner_count: u8,
) -> Outcome<()> {
    let variable = usize::from(receipt_count)
        .checked_add(usize::from(refund_owner_count))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        (1..=PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2).contains(&usize::from(receipt_count))
            && (1..=PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2)
                .contains(&usize::from(refund_owner_count))
            && variable <= PORTFOLIO_ARCHIVE_CURRENT_MAX_ACCOUNTS_V5
                - PORTFOLIO_ARCHIVE_CURRENT_FIXED_ACCOUNTS_V5
            && accounts.len()
                == PORTFOLIO_ARCHIVE_CURRENT_FIXED_ACCOUNTS_V5
                    .checked_add(variable)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::AccountCount,
    )
}

fn local_archive_accounts<'info>(
    accounts: &[AccountInfo<'info>],
) -> std::vec::Vec<AccountInfo<'info>> {
    let mut local = std::vec::Vec::with_capacity(
        accounts.len() - PORTFOLIO_ARCHIVE_CURRENT_FIXED_ACCOUNTS_V5
            + PORTFOLIO_ARCHIVE_FIXED_ACCOUNTS_V2,
    );
    local.extend_from_slice(&accounts[CURRENT_IX_ARCHIVE_SUFFIX..CURRENT_IX_ARCHIVE_SUFFIX + 2]);
    local.push(accounts[CURRENT_V5_IX_MARKET_BINDING].clone());
    local.extend_from_slice(&accounts[CURRENT_IX_ARCHIVE_SUFFIX + 2..]);
    local
}

/// Current action-44 entrypoint.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    action: GeneralV2Action,
    payload: &[u8],
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        action == GeneralV2Action::RetirePortfolioPairArchives
            && crate::capabilities::extension_intent_action_enabled(74, 1, action.tag()),
        ClutchError::UnsupportedInstruction,
    )?;
    let PortfolioSettlementPayloadV1::RetirePortfolioPairArchives(request) =
        decode_portfolio_settlement_payload_v1(action.tag(), payload)?
    else {
        return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction));
    };
    current_account_frame(accounts, request.receipt_count, request.refund_owner_count)?;
    let current = authenticate_general_market_current_prefix_v5(program_id, accounts)?;
    let bound = authenticate_current_collateral(program_id, accounts, &current)?;
    let local = local_archive_accounts(accounts);
    compose_and_apply(
        program_id,
        &local,
        request,
        &current,
        bound,
        &accounts[CURRENT_V5_IX_MARKET_RUNTIME],
    )
    .map(|_| ())
}

/// Compose, apply, and return the private terminal receipt inside one rollback
/// domain. Shared root-retirement code may consume this capability directly;
/// callers cannot construct it or supply any of its pre/post identities.
#[inline(never)]
pub(crate) fn compose_and_apply(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RetirePortfolioPairArchivesPayloadV1,
    current: &AuthenticatedGeneralMarketCurrentV5,
    bound: BoundCollateralProfileV2,
    runtime_account: &AccountInfo<'_>,
) -> Outcome<PortfolioPairArchiveTerminalReceiptV2> {
    let (receipt_count, refund_owner_count, first_refund_owner) = account_frame(
        accounts.len(),
        request.receipt_count,
        request.refund_owner_count,
    )?;
    require_distinct_accounts(accounts)?;

    require_program_account(
        program_id,
        &accounts[IX_SETTLEMENT_ROOT],
        true,
        Some(contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1),
    )?;
    require_program_account(program_id, &accounts[IX_RETAINED_FEED], false, None)?;
    require_program_account(
        program_id,
        &accounts[IX_MARKET_BINDING],
        false,
        Some(contract::MARKET_BINDING_ACCOUNT_BYTES_V5),
    )?;
    require_credit_account(&accounts[IX_NEUTRAL_SINK])?;
    for index in [
        IX_BUYER_RESERVATION_V9,
        IX_SELLER_RESERVATION_V9,
    ] {
        require_program_account(
            program_id,
            &accounts[index],
            true,
            Some(RESERVATION_ACCOUNT_BYTES_V9),
        )?;
    }
    for index in [IX_BUYER_POSITION_V3, IX_SELLER_POSITION_V3] {
        require_program_account(program_id, &accounts[index], true, Some(POSITION_V3_BYTES))?;
    }
    for index in [IX_BUYER_REPLAY_GEN1, IX_SELLER_REPLAY_GEN1] {
        require_program_account(
            program_id,
            &accounts[index],
            true,
            Some(contract::GENERAL_REPLAY_ACCOUNT_V1_BYTES),
        )?;
    }
    let mut receipt_index = 0usize;
    while receipt_index < receipt_count {
        require_program_account(
            program_id,
            &accounts[IX_FIRST_RECEIPT_V5 + receipt_index],
            true,
            Some(SETTLEMENT_RECEIPT_ACCOUNT_BYTES_V5),
        )?;
        receipt_index += 1;
    }
    let mut refund_index = 0usize;
    while refund_index < refund_owner_count {
        require_credit_account(&accounts[first_refund_owner + refund_index])?;
        refund_index += 1;
    }

    let root_account = id(accounts[IX_SETTLEMENT_ROOT].key);
    let authenticated_root = authenticate_writable_general_settlement_root_epoch_v1(
        program_id,
        core::slice::from_ref(&accounts[IX_SETTLEMENT_ROOT]),
        request.epoch,
    )?;
    require(authenticated_root.is_indexed(), ClutchError::MismatchedState)?;
    let root = *authenticated_root.root();
    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id,
        &root.epoch().bytes(),
        &root.settlement_candidate_id().bytes(),
    );
    require(
        request.epoch == root.epoch()
            && request.settlement_root == root_account
            && *accounts[IX_SETTLEMENT_ROOT].key == root_pda.0
            && root.stored_bump() == root_pda.1,
        ClutchError::MismatchedState,
    )?;

    let feed_account = id(accounts[IX_RETAINED_FEED].key);
    let feed_data = borrow_data(&accounts[IX_RETAINED_FEED])?;
    let (feed, _) = contract::complete_candidate_feed_v2(&feed_data, true)?;
    let feed_pda = seeds::general_v2_feed_pda(program_id, &root.source_admission_node().bytes());
    require(
        feed_account == root.retained_feed()
            && *accounts[IX_RETAINED_FEED].key == feed_pda.0
            && feed.stored_bump == feed_pda.1,
        ClutchError::WrongPda,
    )?;

    let market_binding_account = id(accounts[IX_MARKET_BINDING].key);
    let market_binding = *current.binding().base();
    let binding_pda = seeds::general_v2_market_binding_pda(
        program_id,
        &market_binding.base().market_instance_v2_id.bytes(),
    );
    require(
        market_binding_account == root.market_binding()
            && *accounts[IX_MARKET_BINDING].key == current.binding_account()
            && *accounts[IX_MARKET_BINDING].key == binding_pda.0
            && market_binding.base().stored_bump == binding_pda.1
            && id(accounts[IX_NEUTRAL_SINK].key) == market_binding.base().neutral_sink,
        ClutchError::MismatchedState,
    )?;

    let buyer_reservation = ReservationAccountV9::decode(&borrow_data(
        &accounts[IX_BUYER_RESERVATION_V9],
    )?)?;
    let seller_reservation = ReservationAccountV9::decode(&borrow_data(
        &accounts[IX_SELLER_RESERVATION_V9],
    )?)?;
    authenticate_reservation_pda(
        program_id,
        &accounts[IX_BUYER_RESERVATION_V9],
        buyer_reservation,
    )?;
    authenticate_reservation_pda(
        program_id,
        &accounts[IX_SELLER_RESERVATION_V9],
        seller_reservation,
    )?;
    let (buyer_position, buyer_replay) = authenticate_position_replay(
        program_id,
        current,
        bound,
        root,
        buyer_reservation.body().owner.bytes(),
        &accounts[IX_MARKET_BINDING],
        runtime_account,
        &accounts[IX_BUYER_POSITION_V3],
        &accounts[IX_BUYER_REPLAY_GEN1],
    )?;
    let (seller_position, seller_replay) = authenticate_position_replay(
        program_id,
        current,
        bound,
        root,
        seller_reservation.body().owner.bytes(),
        &accounts[IX_MARKET_BINDING],
        runtime_account,
        &accounts[IX_SELLER_POSITION_V3],
        &accounts[IX_SELLER_REPLAY_GEN1],
    )?;

    let mut receipt_inputs = super::orders_batch::boxed_copy_of(
        &[None; PORTFOLIO_ARCHIVE_MAX_RECEIPTS_V2],
    )?;
    receipt_index = 0;
    while receipt_index < receipt_count {
        let account = &accounts[IX_FIRST_RECEIPT_V5 + receipt_index];
        let receipt = SettlementReceiptAccountV5::decode(&borrow_data(account)?)?;
        let semantic = receipt.semantic();
        let expected_pda = seeds::general_v2_receipt_v5_pda(
            program_id,
            &root.epoch().bytes(),
            &root.settlement_candidate_id().bytes(),
            semantic.slice_index,
        );
        require(
            *account.key == expected_pda.0 && semantic.stored_bump == expected_pda.1,
            ClutchError::WrongPda,
        )?;
        receipt_inputs[receipt_index] = Some(PortfolioArchiveReceiptInputV2 {
            account: id(account.key),
            receipt,
            balance_lamports: account.lamports(),
        });
        receipt_index += 1;
    }
    let mut refund_owners = super::orders_batch::boxed_copy_of(
        &[PortfolioArchiveRefundOwnerInputV2::EMPTY;
            PORTFOLIO_ARCHIVE_MAX_REFUND_OWNERS_V2],
    )?;
    refund_index = 0;
    while refund_index < refund_owner_count {
        let account = &accounts[first_refund_owner + refund_index];
        refund_owners[refund_index] = PortfolioArchiveRefundOwnerInputV2 {
            account: id(account.key),
            balance_lamports: account.lamports(),
        };
        refund_index += 1;
    }

    let plan = Box::new(
        prepare_retire_portfolio_pair_archives_v2(
            RetirePortfolioPairArchivesInputV2 {
                payload: request,
                settlement_root_account: root_account,
                settlement_root: &root,
                retained_feed_account: feed_account,
                retained_feed_body: &feed_data,
                market_binding_account,
                market_binding: &market_binding,
                receipts: &receipt_inputs,
                buyer_reservation: PortfolioArchiveReservationInputV2 {
                    account: id(accounts[IX_BUYER_RESERVATION_V9].key),
                    reservation: buyer_reservation,
                    balance_lamports: accounts[IX_BUYER_RESERVATION_V9].lamports(),
                },
                seller_reservation: PortfolioArchiveReservationInputV2 {
                    account: id(accounts[IX_SELLER_RESERVATION_V9].key),
                    reservation: seller_reservation,
                    balance_lamports: accounts[IX_SELLER_RESERVATION_V9].lamports(),
                },
                buyer_position,
                seller_position,
                buyer_replay,
                seller_replay,
                refund_owners: &refund_owners,
                neutral_sink_account: id(accounts[IX_NEUTRAL_SINK].key),
                neutral_sink_balance_lamports: accounts[IX_NEUTRAL_SINK].lamports(),
            },
            &RuntimeSha256,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    );
    drop(feed_data);
    let terminal = plan.terminal_receipt();
    apply_plan(
        accounts,
        first_refund_owner,
        receipt_count,
        refund_owner_count,
        &authenticated_root,
        &plan,
    )?;
    Ok(terminal)
}

fn authenticate_reservation_pda(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    reservation: ReservationAccountV9,
) -> Outcome<()> {
    let body = reservation.body();
    let expected = seeds::general_v2_reservation_v9_pda(
        program_id,
        &body.reservation.bytes(),
    );
    require(
        *account.key == expected.0 && body.stored_bump == expected.1,
        ClutchError::WrongPda,
    )
}

fn apply_plan(
    accounts: &[AccountInfo<'_>],
    first_refund_owner: usize,
    receipt_count: usize,
    refund_owner_count: usize,
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    plan: &clutch_general_v2_runtime::RetirePortfolioPairArchivesPlanV2,
) -> Outcome<()> {
    let receipt_count_u8 = u8::try_from(receipt_count)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let refund_owner_count_u8 = u8::try_from(refund_owner_count)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        plan.terminal_receipt().receipt_count() == receipt_count_u8
            && plan.terminal_receipt().refund_owner_count() == refund_owner_count_u8
            && plan.terminal_receipt().neutral_sink()
                == id(accounts[IX_NEUTRAL_SINK].key),
        ClutchError::MismatchedState,
    )?;
    let mut root_body = std::vec![0u8; authenticated_root.account_bytes()];
    authenticated_root.encode_portfolio_retirement_successor(
        plan.settlement_root_poststate(),
        receipt_count_u8,
        &mut root_body,
    )?;
    let buyer_position_body = plan.buyer_position_poststate()
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let seller_position_body = plan.seller_position_poststate()
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    // Preflight every deletion and credit before the first state or lamport
    // mutation. Solana still supplies atomic rollback, but this ordering keeps
    // adapter refusal independent of partially applied local effects.
    let mut receipt_index = 0usize;
    while receipt_index < receipt_count {
        let close = plan
            .receipt_close(
                u8::try_from(receipt_index)
                    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            )
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        require_close_prestate(
            &accounts[IX_FIRST_RECEIPT_V5 + receipt_index],
            close,
        )?;
        receipt_index += 1;
    }
    require_close_prestate(
        &accounts[IX_BUYER_RESERVATION_V9],
        &plan.reservation_closes()[0],
    )?;
    require_close_prestate(
        &accounts[IX_SELLER_RESERVATION_V9],
        &plan.reservation_closes()[1],
    )?;
    let mut refund_index = 0usize;
    while refund_index < refund_owner_count {
        let transfer = plan.refund_transfers()[refund_index];
        let account = &accounts[first_refund_owner + refund_index];
        require(
            id(account.key) == transfer.owner()
                && account.lamports() == transfer.balance_before(),
            ClutchError::MismatchedState,
        )?;
        refund_index += 1;
    }
    let sink_after = accounts[IX_NEUTRAL_SINK]
        .lamports()
        .checked_add(plan.terminal_receipt().neutral_sink_credit_lamports())
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        sink_after == plan.neutral_sink_balance_after(),
        ClutchError::MismatchedState,
    )?;

    write_exact(&accounts[IX_SETTLEMENT_ROOT], &root_body)?;
    write_exact(&accounts[IX_BUYER_POSITION_V3], &buyer_position_body)?;
    write_exact(&accounts[IX_SELLER_POSITION_V3], &seller_position_body)?;
    write_exact(
        &accounts[IX_BUYER_REPLAY_GEN1],
        plan.buyer_replay_poststate().replay_poststate_body(),
    )?;
    write_exact(
        &accounts[IX_SELLER_REPLAY_GEN1],
        plan.seller_replay_poststate().replay_poststate_body(),
    )?;

    receipt_index = 0;
    while receipt_index < receipt_count {
        let close = plan
            .receipt_close(
                u8::try_from(receipt_index)
                    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
            )
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        close_program_account(&accounts[IX_FIRST_RECEIPT_V5 + receipt_index], close)?;
        receipt_index += 1;
    }
    close_program_account(
        &accounts[IX_BUYER_RESERVATION_V9],
        &plan.reservation_closes()[0],
    )?;
    close_program_account(
        &accounts[IX_SELLER_RESERVATION_V9],
        &plan.reservation_closes()[1],
    )?;

    refund_index = 0;
    while refund_index < refund_owner_count {
        let transfer = plan.refund_transfers()[refund_index];
        let account = &accounts[first_refund_owner + refund_index];
        require(
            id(account.key) == transfer.owner()
                && account.lamports() == transfer.balance_before(),
            ClutchError::MismatchedState,
        )?;
        set_lamports(account, transfer.balance_after())?;
        refund_index += 1;
    }
    set_lamports(
        &accounts[IX_NEUTRAL_SINK],
        plan.neutral_sink_balance_after(),
    )?;
    require(
        &*borrow_data(&accounts[IX_SETTLEMENT_ROOT])? == root_body.as_slice()
            && &*borrow_data(&accounts[IX_BUYER_POSITION_V3])?
                == buyer_position_body.as_slice()
            && &*borrow_data(&accounts[IX_SELLER_POSITION_V3])?
                == seller_position_body.as_slice()
            && &*borrow_data(&accounts[IX_BUYER_REPLAY_GEN1])?
                == plan.buyer_replay_poststate().replay_poststate_body()
            && &*borrow_data(&accounts[IX_SELLER_REPLAY_GEN1])?
                == plan.seller_replay_poststate().replay_poststate_body()
            && accounts[IX_NEUTRAL_SINK].lamports() == plan.neutral_sink_balance_after(),
        ClutchError::MismatchedState,
    )
}

fn write_exact(account: &AccountInfo<'_>, body: &[u8]) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(data.len() == body.len(), ClutchError::WrongDataLength)?;
    data.copy_from_slice(body);
    Ok(())
}

fn set_lamports(account: &AccountInfo<'_>, value: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = value;
    Ok(())
}

fn close_program_account(
    account: &AccountInfo<'_>,
    close: &clutch_general_v2_runtime::PortfolioArchiveClosePlanV2,
) -> Outcome<()> {
    require_close_prestate(account, close)?;
    set_lamports(account, 0)?;
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(
        account.data_len() == 0
            && account.lamports() == 0
            && *account.owner == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

fn require_close_prestate(
    account: &AccountInfo<'_>,
    close: &clutch_general_v2_runtime::PortfolioArchiveClosePlanV2,
) -> Outcome<()> {
    require(
        id(account.key) == close.account()
            && account.lamports() == close.balance_before()
            && account.is_writable,
        ClutchError::MismatchedState,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_frame_is_exact_at_both_bounds() {
        assert_eq!(account_frame(12, 1, 1), Ok((1, 1, 11)));
        assert_eq!(account_frame(44, 16, 18), Ok((16, 18, 26)));
        assert_eq!(
            account_frame(43, 16, 18),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
        assert_eq!(
            account_frame(12, 0, 2),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
        assert_eq!(
            account_frame(12, 2, 0),
            Err(Refusal::Adapter(ClutchError::AccountCount))
        );
    }

    #[test]
    fn frozen_meta_width_matches_complete_close_set() {
        assert_eq!(PORTFOLIO_ARCHIVE_FIXED_ACCOUNTS_V2, 10);
        assert_eq!(PORTFOLIO_ARCHIVE_MIN_ACCOUNTS_V2, 12);
        assert_eq!(PORTFOLIO_ARCHIVE_MAX_ACCOUNTS_V2, 44);
    }
}
