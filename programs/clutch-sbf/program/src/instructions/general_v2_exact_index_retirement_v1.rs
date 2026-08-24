//! Current V5 SBF retirement adapters for the compact indexed General root.
//!
//! The child and Feed transitions are action-specific, hostile-authenticated,
//! and complete every write/credit preflight before mutating account state.

use core::cell::{Ref, RefMut};
use std::boxed::Box;

use clutch_general_v2_contract as contract;
use clutch_general_v2_contract::{Id32, MarketBindingV5};
use clutch_general_v2_runtime::{
    stream_retire_counted_exact_feed_v1, stream_retire_counted_exact_index_root_v1,
    AuthenticateCountedExactIndexReadInputV1, CloseExactIndexPlaneInputV1,
    ExactIndexCloseAccountInputV1, ExactIndexReadAccountInputV1,
    RetireCountedExactFeedInputV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;

pub(crate) const RETIRE_INDEX_CHILDREN_ACCOUNT_COUNT_V1: usize = 7;
pub(crate) const RETIRE_RETAINED_FEED_ACCOUNT_COUNT_V1: usize = 6;

const IX_ROOT: usize = 0;
const IX_LOCATOR: usize = 1;
const IX_ADJACENCY: usize = 2;
const IX_FEED: usize = 3;
const IX_BINDING: usize = 4;
const IX_CHILD_PAYER: usize = 5;
const IX_CHILD_SINK: usize = 6;

const IX_FEED_RETIRE_ROOT: usize = 0;
const IX_FEED_RETIRE_FEED: usize = 1;
const IX_FEED_RETIRE_BINDING: usize = 2;
const IX_FEED_RETIRE_PAYER: usize = 3;
const IX_FEED_RETIRE_SINK: usize = 4;
const IX_FEED_RETIRE_KEEPER: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CloseCreditV1 {
    recipient: Id32,
    amount: u64,
}

impl CloseCreditV1 {
    const fn new(recipient: Id32, amount: u64) -> Self {
        Self { recipient, amount }
    }
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

fn require_destination(account: &AccountInfo<'_>) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)
}

fn require_pairwise_distinct(accounts: &[&AccountInfo<'_>]) -> Outcome<()> {
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

fn require_destinations_disjoint_from_state(
    state: &[&AccountInfo<'_>],
    destinations: &[&AccountInfo<'_>],
) -> Outcome<()> {
    require_pairwise_distinct(state)?;
    for destination in destinations {
        for account in state {
            require(destination.key != account.key, ClutchError::AccountAlias)?;
        }
    }
    Ok(())
}

fn decode_binding(program_id: &Pubkey, account: &AccountInfo<'_>) -> Outcome<Box<MarketBindingV5>> {
    require_program_account(
        program_id,
        account,
        false,
        Some(contract::MARKET_BINDING_ACCOUNT_BYTES_V5),
    )?;
    let binding = Box::new(MarketBindingV5::decode(&borrow_data(account)?)?);
    let canonical = seeds::general_v2_market_binding_pda(
        program_id,
        &binding.base().market_instance_v2_id.bytes(),
    );
    require(
        *account.key == canonical.0 && binding.base().stored_bump == canonical.1,
        ClutchError::WrongPda,
    )?;
    Ok(binding)
}

fn decode_complete_feed_boxed(body: &[u8]) -> Outcome<Box<contract::CandidateFeedHeaderV2>> {
    let (feed, _) = contract::complete_candidate_feed_v2(body, true)?;
    Ok(Box::new(feed))
}

fn set_lamports(account: &AccountInfo<'_>, value: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = value;
    Ok(())
}

fn close_program_account(account: &AccountInfo<'_>) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
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

fn preflight_writes(accounts: &[&AccountInfo<'_>]) -> Outcome<()> {
    for account in accounts {
        let data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(data);
        let lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        drop(lamports);
    }
    Ok(())
}

fn checked_credit_total(recipient: Id32, credits: &[CloseCreditV1]) -> Outcome<u64> {
    let mut total = 0u64;
    for credit in credits {
        if credit.recipient == recipient {
            total = total
                .checked_add(credit.amount)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
    }
    Ok(total)
}

fn preflight_credits(destinations: &[&AccountInfo<'_>], credits: &[CloseCreditV1]) -> Outcome<()> {
    for credit in credits {
        require(
            destinations.iter().any(|account| id(account.key) == credit.recipient),
            ClutchError::MismatchedState,
        )?;
    }
    let mut index = 0usize;
    while index < destinations.len() {
        require_destination(destinations[index])?;
        let mut prior = 0usize;
        while prior < index && destinations[prior].key != destinations[index].key {
            prior += 1;
        }
        if prior == index {
            destinations[index]
                .lamports()
                .checked_add(checked_credit_total(id(destinations[index].key), credits)?)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        index += 1;
    }
    Ok(())
}

fn apply_credits(destinations: &[&AccountInfo<'_>], credits: &[CloseCreditV1]) -> Outcome<()> {
    let mut index = 0usize;
    while index < destinations.len() {
        let mut prior = 0usize;
        while prior < index && destinations[prior].key != destinations[index].key {
            prior += 1;
        }
        if prior == index {
            let after = destinations[index]
                .lamports()
                .checked_add(checked_credit_total(id(destinations[index].key), credits)?)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            set_lamports(destinations[index], after)?;
        }
        index += 1;
    }
    Ok(())
}

/// Action-45 body. The central route is added only with the complete V5 chain.
#[inline(never)]
pub(crate) fn retire_exact_index_children_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: contract::CountedSettlementRootSelectorV1,
) -> Outcome<()> {
    require_count(accounts, RETIRE_INDEX_CHILDREN_ACCOUNT_COUNT_V1)?;
    require_program_account(program_id, &accounts[IX_ROOT], true,
        Some(contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1))?;
    require_program_account(program_id, &accounts[IX_LOCATOR], true, None)?;
    require_program_account(program_id, &accounts[IX_ADJACENCY], true, None)?;
    require_program_account(program_id, &accounts[IX_FEED], false, None)?;
    require_destination(&accounts[IX_CHILD_PAYER])?;
    require_destination(&accounts[IX_CHILD_SINK])?;
    require_destinations_disjoint_from_state(
        &[&accounts[IX_ROOT], &accounts[IX_LOCATOR], &accounts[IX_ADJACENCY],
            &accounts[IX_FEED], &accounts[IX_BINDING]],
        &[&accounts[IX_CHILD_PAYER], &accounts[IX_CHILD_SINK]],
    )?;
    let binding = decode_binding(program_id, &accounts[IX_BINDING])?;
    require(id(accounts[IX_CHILD_SINK].key) == binding.base().neutral_sink,
        ClutchError::MismatchedState)?;
    let feed_body = borrow_data(&accounts[IX_FEED])?;
    let feed = decode_complete_feed_boxed(&feed_body)?;
    require(feed.epoch == selector.epoch, ClutchError::MismatchedState)?;
    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id, &selector.epoch.bytes(), &feed.settlement_candidate_id.bytes());
    require(*accounts[IX_ROOT].key == root_pda.0
        && selector.settlement_root == id(accounts[IX_ROOT].key), ClutchError::WrongPda)?;
    let locator_pda = seeds::general_v2_frozen_order_locator_pda(program_id, &root_pda.0.to_bytes());
    let adjacency_pda = seeds::general_v2_candidate_slice_index_pda(program_id, &root_pda.0.to_bytes());
    let feed_pda = seeds::general_v2_feed_pda(program_id, &feed.node.bytes());
    let root_body = borrow_data(&accounts[IX_ROOT])?;
    let locator_body = borrow_data(&accounts[IX_LOCATOR])?;
    let adjacency_body = borrow_data(&accounts[IX_ADJACENCY])?;
    let mut root_output = std::vec![0u8; contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1];
    let result = stream_retire_counted_exact_index_root_v1(
        AuthenticateCountedExactIndexReadInputV1 {
            program_id: id(program_id),
            root: ExactIndexReadAccountInputV1 { account: id(accounts[IX_ROOT].key), body: &root_body,
                owner: id(accounts[IX_ROOT].owner), canonical_account: id(&root_pda.0),
                canonical_bump: root_pda.1, writable: true, executable: accounts[IX_ROOT].executable },
            locator: ExactIndexReadAccountInputV1 { account: id(accounts[IX_LOCATOR].key), body: &locator_body,
                owner: id(accounts[IX_LOCATOR].owner), canonical_account: id(&locator_pda.0),
                canonical_bump: locator_pda.1, writable: true, executable: accounts[IX_LOCATOR].executable },
            adjacency: ExactIndexReadAccountInputV1 { account: id(accounts[IX_ADJACENCY].key), body: &adjacency_body,
                owner: id(accounts[IX_ADJACENCY].owner), canonical_account: id(&adjacency_pda.0),
                canonical_bump: adjacency_pda.1, writable: true, executable: accounts[IX_ADJACENCY].executable },
            feed: ExactIndexReadAccountInputV1 { account: id(accounts[IX_FEED].key), body: &feed_body,
                owner: id(accounts[IX_FEED].owner), canonical_account: id(&feed_pda.0),
                canonical_bump: feed_pda.1, writable: false, executable: accounts[IX_FEED].executable },
        },
        CloseExactIndexPlaneInputV1 {
            market_binding_account: id(accounts[IX_BINDING].key),
            market_binding: binding.base(),
            locator: ExactIndexCloseAccountInputV1 { account: id(accounts[IX_LOCATOR].key),
                lamports: accounts[IX_LOCATOR].lamports(), owner: id(accounts[IX_LOCATOR].owner),
                program_id: id(program_id), writable: true, executable: accounts[IX_LOCATOR].executable },
            adjacency: ExactIndexCloseAccountInputV1 { account: id(accounts[IX_ADJACENCY].key),
                lamports: accounts[IX_ADJACENCY].lamports(), owner: id(accounts[IX_ADJACENCY].owner),
                program_id: id(program_id), writable: true, executable: accounts[IX_ADJACENCY].executable },
        },
        &mut root_output,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let close = result.close_postwrites();
    let credits = [
        CloseCreditV1::new(close.locator_principal_credit().recipient(), close.locator_principal_credit().amount()),
        CloseCreditV1::new(close.adjacency_principal_credit().recipient(), close.adjacency_principal_credit().amount()),
        CloseCreditV1::new(close.locator_donation_credit().recipient(), close.locator_donation_credit().amount()),
        CloseCreditV1::new(close.adjacency_donation_credit().recipient(), close.adjacency_donation_credit().amount()),
    ];
    require(credits[0].recipient == id(accounts[IX_CHILD_PAYER].key)
        && credits[1].recipient == id(accounts[IX_CHILD_PAYER].key)
        && credits[2].recipient == id(accounts[IX_CHILD_SINK].key)
        && credits[3].recipient == id(accounts[IX_CHILD_SINK].key), ClutchError::MismatchedState)?;
    preflight_credits(&[&accounts[IX_CHILD_PAYER], &accounts[IX_CHILD_SINK]], &credits)?;
    drop(adjacency_body); drop(locator_body); drop(root_body); drop(feed_body);
    preflight_writes(&[&accounts[IX_ROOT], &accounts[IX_LOCATOR], &accounts[IX_ADJACENCY],
        &accounts[IX_CHILD_PAYER], &accounts[IX_CHILD_SINK]])?;
    borrow_mut_data(&accounts[IX_ROOT])?.copy_from_slice(&root_output);
    close_program_account(&accounts[IX_LOCATOR])?;
    close_program_account(&accounts[IX_ADJACENCY])?;
    apply_credits(&[&accounts[IX_CHILD_PAYER], &accounts[IX_CHILD_SINK]], &credits)
}

/// Action-46 body. Feed bytes remain authenticated through the root-bound ID.
#[inline(never)]
pub(crate) fn retire_retained_feed_v5(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    selector: contract::CountedSettlementRootSelectorV1,
) -> Outcome<()> {
    require_count(accounts, RETIRE_RETAINED_FEED_ACCOUNT_COUNT_V1)?;
    require_program_account(program_id, &accounts[IX_FEED_RETIRE_ROOT], true,
        Some(contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1))?;
    require_program_account(program_id, &accounts[IX_FEED_RETIRE_FEED], true, None)?;
    let binding = decode_binding(program_id, &accounts[IX_FEED_RETIRE_BINDING])?;
    for destination in [&accounts[IX_FEED_RETIRE_PAYER], &accounts[IX_FEED_RETIRE_SINK],
        &accounts[IX_FEED_RETIRE_KEEPER]] { require_destination(destination)?; }
    require(accounts[IX_FEED_RETIRE_KEEPER].is_signer, ClutchError::MissingSignature)?;
    require_destinations_disjoint_from_state(
        &[&accounts[IX_FEED_RETIRE_ROOT], &accounts[IX_FEED_RETIRE_FEED],
            &accounts[IX_FEED_RETIRE_BINDING]],
        &[&accounts[IX_FEED_RETIRE_PAYER], &accounts[IX_FEED_RETIRE_SINK],
            &accounts[IX_FEED_RETIRE_KEEPER]],
    )?;
    let feed_body = borrow_data(&accounts[IX_FEED_RETIRE_FEED])?;
    let feed = decode_complete_feed_boxed(&feed_body)?;
    require(feed.epoch == selector.epoch, ClutchError::MismatchedState)?;
    let root_pda = seeds::general_v2_settlement_root_pda(
        program_id, &selector.epoch.bytes(), &feed.settlement_candidate_id.bytes());
    let feed_pda = seeds::general_v2_feed_pda(program_id, &feed.node.bytes());
    require(*accounts[IX_FEED_RETIRE_ROOT].key == root_pda.0
        && selector.settlement_root == id(accounts[IX_FEED_RETIRE_ROOT].key)
        && *accounts[IX_FEED_RETIRE_FEED].key == feed_pda.0
        && feed.stored_bump == feed_pda.1
        && id(accounts[IX_FEED_RETIRE_SINK].key) == binding.base().neutral_sink,
        ClutchError::WrongPda)?;
    let root_body = borrow_data(&accounts[IX_FEED_RETIRE_ROOT])?;
    let mut root_output = std::vec![0u8; contract::INDEXED_SETTLEMENT_ROOT_BYTES_V1];
    let result = stream_retire_counted_exact_feed_v1(
        RetireCountedExactFeedInputV1 {
            program_id: id(program_id), root_account: id(accounts[IX_FEED_RETIRE_ROOT].key),
            root_body: &root_body, market_binding_account: id(accounts[IX_FEED_RETIRE_BINDING].key),
            market_binding: binding.base(), feed_account: id(accounts[IX_FEED_RETIRE_FEED].key),
            feed_body: &feed_body, feed_lamports: accounts[IX_FEED_RETIRE_FEED].lamports(),
            feed_owner: id(accounts[IX_FEED_RETIRE_FEED].owner), feed_writable: true,
            feed_executable: accounts[IX_FEED_RETIRE_FEED].executable,
            keeper_destination: id(accounts[IX_FEED_RETIRE_KEEPER].key),
        },
        &mut root_output,
    ).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let credits = [
        CloseCreditV1::new(result.feed_principal_credit().recipient(), result.feed_principal_credit().amount()),
        CloseCreditV1::new(result.feed_donation_credit().recipient(), result.feed_donation_credit().amount()),
        CloseCreditV1::new(result.feed_keeper_reward_credit().recipient(), result.feed_keeper_reward_credit().amount()),
    ];
    require(credits[0].recipient == id(accounts[IX_FEED_RETIRE_PAYER].key)
        && credits[1].recipient == id(accounts[IX_FEED_RETIRE_SINK].key)
        && credits[2].recipient == id(accounts[IX_FEED_RETIRE_KEEPER].key),
        ClutchError::MismatchedState)?;
    preflight_credits(&[&accounts[IX_FEED_RETIRE_PAYER], &accounts[IX_FEED_RETIRE_SINK],
        &accounts[IX_FEED_RETIRE_KEEPER]], &credits)?;
    drop(root_body); drop(feed_body);
    preflight_writes(&[&accounts[IX_FEED_RETIRE_ROOT], &accounts[IX_FEED_RETIRE_FEED],
        &accounts[IX_FEED_RETIRE_PAYER], &accounts[IX_FEED_RETIRE_SINK],
        &accounts[IX_FEED_RETIRE_KEEPER]])?;
    borrow_mut_data(&accounts[IX_FEED_RETIRE_ROOT])?.copy_from_slice(&root_output);
    close_program_account(&accounts[IX_FEED_RETIRE_FEED])?;
    apply_credits(&[&accounts[IX_FEED_RETIRE_PAYER], &accounts[IX_FEED_RETIRE_SINK],
        &accounts[IX_FEED_RETIRE_KEEPER]], &credits)
}
