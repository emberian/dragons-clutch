//! General epoch lifecycle — Tier 2 join 6's init half and the general freeze.
//!
//! `Intent::InitEpoch` (tag 49) and `Intent::FreezeEpoch` (tag 50): the gap
//! named at `instructions/genesis.rs` ("No `InitEpoch`") and backlog 6.1,
//! closed on the general plane.  The template is the Direct V4 lifecycle with
//! every `== 2` gate removed:
//!
//! * **InitEpoch** creates the general [`EpochAccount`] (4-page geometry,
//!   `outcome_count` up to 16) at `seeds::epoch_pda`, phase OPEN, binding the
//!   market, terms, grid, and policy artifacts exactly as
//!   `direct_selection_v3::init_epoch` binds them — minus the binary-width
//!   requirements.  The deadline slot rides a small companion
//!   [`EpochWindowAccount`] (the V3 window precedent), **not** an
//!   `EpochAccount` format bump, so every existing epoch consumer stays
//!   byte-stable.
//! * **FreezeEpoch** is permissionless keeper work at or after the window's
//!   deadline: all pages of the set ride one instruction,
//!   [`stream::frozen_set_commitment`] recomputes the set identity,
//!   [`stream::seal_page`] stamps each page, the epoch takes
//!   `order_set`/first/last/`page_count`/`order_count`/phase FROZEN, and
//!   `owner_count` is **rewritten** with the exact distinct-owner count
//!   interned over the frozen set's live records — the value the pass-1 walk
//!   (T2-6b) is later refused against.  [`stream::epoch_binds_page_set`] runs
//!   over the sealed set as the post-state check, so a set the binding would
//!   refuse (an expired live record, a non-dense page) is never frozen.
//!
//! ## The policy account
//!
//! The presented policy artifact is the canonical 64-byte `BatchPolicyV1`
//! account at `seeds::batch_policy_pda(epoch, digest)` — the exact shape and
//! address `direct_selection::read_batch_policy` consumes — required to wrap
//! [`GENERAL_CLEARING_POLICY_V1`] exactly, with `epoch.policy ==
//! batch_policy_digest(artifact)` enforced.  One digest is one truth: the
//! PDA's digest seed, the intent's `policy`, and the persisted `epoch.policy`
//! are all the same recomputed value.  (The plan's "DirectBatchPolicyV3-shaped"
//! phrasing is read as this precedent role — a sealed policy artifact bound at
//! init; the 96-byte V3 wrapper would carry a second digest and an unpinned
//! verifier release id, and has no seal path at `SEED_BATCH_POLICY`.)
//!
//! ## Claim plane
//!
//! SBF-EXECUTED (bank), no promotion.  The reference adapter refuses both
//! intents with `UnsupportedIntent`, so the SVM oracle for this family is the
//! layout codec byte-for-byte, per the genesis precedent.

use crate::accounts::{
    self, expect_pda, require, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program,
};
use crate::seeds;
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy, general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
    BATCH_POLICY_BYTES,
};
use clutch_solana_layout::clearing::{
    canonical_general_book_id, open_general_epoch, EpochWindowAccount,
    EPOCH_WINDOW_ACCOUNT_BYTES,
};
use clutch_solana_layout::projection::OwnerInterner;
use clutch_solana_layout::{
    account_len, canonical_epoch_id, stream, EpochAccount, Hash32, OrderSlot, EPOCH_PHASE_FROZEN,
    EPOCH_PHASE_OPEN, MAX_ORDER_PAGES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Accounts in an `InitEpoch` instruction, exactly.
pub const INIT_EPOCH_ACCOUNT_COUNT: usize = 10;
/// Authenticated payer funding both creations.
pub const IX_INIT_EPOCH_PAYER: usize = 0;
/// The market this epoch belongs to (read-only, program-owned).
pub const IX_INIT_EPOCH_MARKET: usize = 1;
/// The immutable terms the epoch clears under (read-only, program-owned).
pub const IX_INIT_EPOCH_TERMS: usize = 2;
/// The frozen price grid the terms name (read-only, program-owned).
pub const IX_INIT_EPOCH_GRID: usize = 3;
/// The sealed 64-byte batch-policy artifact (read-only, program-owned).
pub const IX_INIT_EPOCH_POLICY: usize = 4;
/// The canonical, not-yet-created epoch PDA.
pub const IX_INIT_EPOCH_EPOCH: usize = 5;
/// The canonical, not-yet-created window PDA.
pub const IX_INIT_EPOCH_WINDOW: usize = 6;
/// System program.
pub const IX_INIT_EPOCH_SYSTEM: usize = 7;
/// Rent sysvar.
pub const IX_INIT_EPOCH_RENT: usize = 8;
/// Clock sysvar.
pub const IX_INIT_EPOCH_CLOCK: usize = 9;

/// Fixed accounts in a `FreezeEpoch` instruction, before the page set.
///
/// Deliberately no price grid: every live record was tick-verified at
/// placement against the grid the epoch immutably names, so a freeze-time
/// grid re-verification would re-fold every page a third time for a fact no
/// path can have changed — and the third fold is what would push a full
/// four-page freeze past the 1.4M-CU transaction ceiling.
pub const FREEZE_EPOCH_FIXED_ACCOUNT_COUNT: usize = 3;
/// The open epoch being frozen (writable, program-owned).
pub const IX_FREEZE_EPOCH_EPOCH: usize = 0;
/// The epoch's deadline window (read-only, program-owned).
pub const IX_FREEZE_EPOCH_WINDOW: usize = 1;
/// Clock sysvar.
pub const IX_FREEZE_EPOCH_CLOCK: usize = 2;
/// First page of the set; pages `0..page_count` follow in index order.
pub const IX_FREEZE_EPOCH_PAGES: usize = FREEZE_EPOCH_FIXED_ACCOUNT_COUNT;

const INIT_EPOCH_STATE_ROLES: [StateRole; 4] = [
    StateRole::read_only(IX_INIT_EPOCH_MARKET, account_len::MARKET),
    StateRole::read_only(IX_INIT_EPOCH_TERMS, account_len::TERMS),
    StateRole::read_only(IX_INIT_EPOCH_GRID, account_len::PRICE_GRID),
    StateRole::read_only(IX_INIT_EPOCH_POLICY, BATCH_POLICY_BYTES),
];

/// Create the general epoch and its deadline window, bound to everything.
#[inline(never)]
pub(super) fn init_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    epoch_index: u64,
    intent_policy: &Hash32,
    freeze_deadline_slot: u64,
) -> Outcome<()> {
    accounts::require_count(accounts, INIT_EPOCH_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[IX_INIT_EPOCH_PAYER])?;
    require(
        accounts[IX_INIT_EPOCH_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &INIT_EPOCH_STATE_ROLES)?;
    require_creatable(&accounts[IX_INIT_EPOCH_EPOCH])?;
    require_creatable(&accounts[IX_INIT_EPOCH_WINDOW])?;
    require_system_program(&accounts[IX_INIT_EPOCH_SYSTEM])?;
    let rent = read_rent(&accounts[IX_INIT_EPOCH_RENT])?;
    let now = read_clock_slot(&accounts[IX_INIT_EPOCH_CLOCK])?;
    // A deadline already behind the clock would admit an immediate freeze of
    // a book nobody could have placed into; refuse it at creation.
    require(freeze_deadline_slot > now, ClutchError::NotActive)?;

    let market = accounts::read_market(&accounts[IX_INIT_EPOCH_MARKET].data.borrow())?;
    let terms = accounts::read_terms(&accounts[IX_INIT_EPOCH_TERMS].data.borrow())?;
    let grid = accounts::read_price_grid(&accounts[IX_INIT_EPOCH_GRID].data.borrow())?;
    // The V3 template's binding matrix, minus its `== 2` gates: the general
    // plane admits exactly the width the terms froze.
    require(
        market.market == *intent_market
            && market.lifecycle == 0
            && terms.terms == market.terms
            && terms.realm == market.realm
            && terms.profile == market.profile
            && market.outcome_count == terms.outcome_count
            && terms.price_grid == grid.grid
            && grid.realm == market.realm,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_INIT_EPOCH_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market.market.bytes()),
        Some(market.stored_bump),
    )?;
    expect_pda(
        accounts[IX_INIT_EPOCH_TERMS].key,
        seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes()),
        Some(terms.stored_bump),
    )?;
    expect_pda(
        accounts[IX_INIT_EPOCH_GRID].key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;

    let epoch_id = canonical_epoch_id(*intent_market, epoch_index);
    read_general_batch_policy(
        program_id,
        &accounts[IX_INIT_EPOCH_POLICY],
        epoch_id,
        *intent_policy,
    )?;

    let market_bytes = intent_market.bytes();
    let index_bytes = epoch_index.to_le_bytes();
    let (epoch_address, epoch_bump) = seeds::epoch_pda(program_id, &market_bytes, epoch_index);
    expect_pda(
        accounts[IX_INIT_EPOCH_EPOCH].key,
        (epoch_address, epoch_bump),
        None,
    )?;
    let (window_address, window_bump) =
        seeds::epoch_window_pda(program_id, &market_bytes, epoch_index);
    expect_pda(
        accounts[IX_INIT_EPOCH_WINDOW].key,
        (window_address, window_bump),
        None,
    )?;

    let epoch = open_general_epoch(
        *intent_market,
        terms.terms,
        grid.grid,
        *intent_policy,
        epoch_index,
        grid.price_scale,
        terms.outcome_count,
        epoch_bump,
    )?;
    let window = EpochWindowAccount {
        epoch: epoch_id,
        market: *intent_market,
        epoch_index,
        freeze_deadline_slot,
        stored_bump: window_bump,
    flags: 0,
    };
    window.validate()?;

    create_pda_account(
        program_id,
        &accounts[IX_INIT_EPOCH_PAYER],
        &accounts[IX_INIT_EPOCH_EPOCH],
        &accounts[IX_INIT_EPOCH_SYSTEM],
        &rent,
        account_len::EPOCH,
        &[
            seeds::SEED_EPOCH,
            &market_bytes,
            &index_bytes,
            &[epoch_bump],
        ],
    )?;
    create_pda_account(
        program_id,
        &accounts[IX_INIT_EPOCH_PAYER],
        &accounts[IX_INIT_EPOCH_WINDOW],
        &accounts[IX_INIT_EPOCH_SYSTEM],
        &rent,
        EPOCH_WINDOW_ACCOUNT_BYTES,
        &[
            seeds::SEED_EPOCH_WINDOW,
            &market_bytes,
            &index_bytes,
            &[window_bump],
        ],
    )?;
    epoch.encode(&mut borrow_account_mut(&accounts[IX_INIT_EPOCH_EPOCH])?)?;
    window.encode(&mut borrow_account_mut(&accounts[IX_INIT_EPOCH_WINDOW])?)?;
    Ok(())
}

/// Freeze the general epoch's complete page set, at or after the deadline.
#[inline(never)]
pub(super) fn freeze_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        accounts.len() > FREEZE_EPOCH_FIXED_ACCOUNT_COUNT
            && accounts.len() <= FREEZE_EPOCH_FIXED_ACCOUNT_COUNT + MAX_ORDER_PAGES,
        ClutchError::AccountCount,
    )?;
    let page_count = accounts.len() - FREEZE_EPOCH_FIXED_ACCOUNT_COUNT;
    require_distinct(accounts)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_FREEZE_EPOCH_EPOCH],
        true,
        &[account_len::EPOCH],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_FREEZE_EPOCH_WINDOW],
        false,
        &[EPOCH_WINDOW_ACCOUNT_BYTES],
    )?;
    for page in &accounts[IX_FREEZE_EPOCH_PAGES..] {
        accounts::validate_state_role_lengths(program_id, page, true, &[account_len::ORDER_PAGE])?;
    }
    let now = read_clock_slot(&accounts[IX_FREEZE_EPOCH_CLOCK])?;

    let mut epoch = decode_epoch_boxed(&accounts[IX_FREEZE_EPOCH_EPOCH].data.borrow())?;
    require(
        epoch.market == *intent_market && epoch.epoch == *intent_epoch,
        ClutchError::MismatchedState,
    )?;
    // Only the general book family freezes here: a direct epoch is a
    // different account length and never reaches this branch, and a general
    // epoch that is already frozen (or lapsed) refuses as inactive.
    require(epoch.phase == EPOCH_PHASE_OPEN, ClutchError::NotActive)?;
    require(
        epoch.book == canonical_general_book_id(epoch.epoch),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_FREEZE_EPOCH_EPOCH].key,
        seeds::epoch_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;

    let window = EpochWindowAccount::decode(&accounts[IX_FREEZE_EPOCH_WINDOW].data.borrow())?;
    require(
        window.epoch == epoch.epoch
            && window.market == epoch.market
            && window.epoch_index == epoch.epoch_index,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_FREEZE_EPOCH_WINDOW].key,
        seeds::epoch_window_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(window.stored_bump),
    )?;
    // The deadline is the whole authority: before it, everyone refuses; at or
    // after it, anyone may freeze.
    require(now >= window.freeze_deadline_slot, ClutchError::NotActive)?;

    // Every page of the set, by canonical address, in index order.  Record
    // bytes are digest-verified inside `frozen_set_commitment` below; the
    // per-record grid half was verified at each placement and is not
    // re-folded here (see the account-plane note above).
    for (index, page) in accounts[IX_FREEZE_EPOCH_PAGES..].iter().enumerate() {
        let data = page.data.borrow();
        let header = stream::OrderPageHeader::decode(&data)?;
        require(
            header.page_index as usize == index
                && header.market == epoch.market
                && header.epoch == epoch.epoch,
            ClutchError::MismatchedState,
        )?;
        expect_pda(
            page.key,
            seeds::page_pda(program_id, &epoch.epoch.bytes(), header.page_index),
            Some(header.stored_bump),
        )?;
    }

    // The set commitment and the exact distinct-owner interning, over the
    // same borrowed bytes.  `frozen_set_commitment` re-verifies every page's
    // digest and density; the interning walk mints tags over live records
    // only, exactly as the projection will during the pass-1 walk.
    let mut owners = boxed_empty_interner()?;
    let (order_set, set_order_count, head_first, tail_last) = {
        let borrows: Vec<core::cell::Ref<'_, &mut [u8]>> = accounts[IX_FREEZE_EPOCH_PAGES..]
            .iter()
            .map(|page| page.data.borrow())
            .collect();
        let refs: Vec<&[u8]> = borrows.iter().map(|data| &***data as &[u8]).collect();
        let (order_set, set_order_count) = stream::frozen_set_commitment(&refs)?;
        for page in &refs {
            let header = stream::OrderPageHeader::decode(page)?;
            let mut cursor = stream::OrderSlotCursor::new(page)?;
            let mut index = 0usize;
            while index < header.order_count as usize {
                let slot = match cursor.next_slot() {
                    Some(step) => step?,
                    None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
                };
                match slot {
                    OrderSlot::Single(record) => {
                        owners.intern(record.owner)?;
                    }
                    OrderSlot::Portfolio(record) => {
                        owners.intern(record.owner)?;
                    }
                    // A retired record is never fed, so it mints no tag.
                    OrderSlot::Tombstone(_) => {}
                    OrderSlot::Empty => return Err(Refusal::Codec(
                        clutch_solana_layout::CodecError::ZeroIdentity,
                    )),
                }
                index += 1;
            }
        }
        let head = stream::OrderPageHeader::decode(refs[0])?;
        let tail = stream::OrderPageHeader::decode(refs[refs.len() - 1])?;
        (
            order_set,
            set_order_count,
            head.first_order_id,
            tail.last_order_id,
        )
    };

    epoch.phase = EPOCH_PHASE_FROZEN;
    epoch.order_set = order_set;
    epoch.first_order_id = head_first;
    epoch.last_order_id = tail_last;
    epoch.page_count = page_count as u16;
    epoch.order_count = set_order_count;
    epoch.owner_count = owners.count();
    epoch.validate()?;

    for page in &accounts[IX_FREEZE_EPOCH_PAGES..] {
        stream::seal_page(&mut borrow_account_mut(page)?, order_set, set_order_count)?;
    }

    // The post-state check the plan names: the sealed set, complete and in
    // order, binds to exactly the epoch value about to be persisted — width
    // and horizon walks included.  A set it refuses is never frozen, because
    // this refusal rolls the whole instruction back.
    {
        let borrows: Vec<core::cell::Ref<'_, &mut [u8]>> = accounts[IX_FREEZE_EPOCH_PAGES..]
            .iter()
            .map(|page| page.data.borrow())
            .collect();
        let refs: Vec<&[u8]> = borrows.iter().map(|data| &***data as &[u8]).collect();
        stream::epoch_binds_page_set(&epoch, &refs)?;
    }

    epoch.encode(&mut borrow_account_mut(&accounts[IX_FREEZE_EPOCH_EPOCH])?)?;
    Ok(())
}

/// Read and authenticate the general 64-byte batch-policy artifact.
///
/// The general sibling of `direct_selection::read_batch_policy`: the account
/// at `seeds::batch_policy_pda(epoch, digest)` must decode to exactly
/// [`GENERAL_CLEARING_POLICY_V1`] and re-derive the expected digest.
#[inline(never)]
fn read_general_batch_policy(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: Hash32,
    expected_digest: Hash32,
) -> Outcome<()> {
    let policy = decode_batch_policy(&account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let digest =
        batch_policy_digest(&policy).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        policy == GENERAL_CLEARING_POLICY_V1 && digest.0 == expected_digest.bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    expect_pda(
        account.key,
        seeds::batch_policy_pda(program_id, &epoch.bytes(), &expected_digest.bytes()),
        None,
    )
}

/// Borrow one account's data mutably, or refuse.
fn borrow_account_mut<'a, 'info>(
    account: &'a AccountInfo<'info>,
) -> Outcome<core::cell::RefMut<'a, &'info mut [u8]>> {
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
}

/// Decode one whole epoch account onto the heap.
#[inline(never)]
fn decode_epoch_boxed(bytes: &[u8]) -> Outcome<Box<EpochAccount>> {
    Ok(Box::new(EpochAccount::decode(bytes)?))
}

/// A fresh, empty owner-interning table on the heap.
///
/// The table is 2,050 bytes — half an SBF frame — so it is built in place from
/// static storage rather than moved through this frame.
#[inline(never)]
fn boxed_empty_interner() -> Outcome<Box<OwnerInterner>> {
    static EMPTY: OwnerInterner = OwnerInterner::NEW;
    boxed_copy_of(&EMPTY)
}

/// Copy one static value onto the heap without materializing it on a frame.
fn boxed_copy_of<T: Copy>(source: &'static T) -> Outcome<Box<T>> {
    let layout = core::alloc::Layout::new::<T>();
    // SAFETY: `T: Copy` has no drop obligations and no interior references;
    // a byte copy of a valid static value is a valid value, and the pointer
    // is freshly allocated for exactly `T`'s layout.
    unsafe {
        let pointer = std::alloc::alloc(layout) as *mut T;
        if pointer.is_null() {
            return Err(Refusal::Adapter(ClutchError::AccountCreationFailed));
        }
        core::ptr::copy_nonoverlapping(source as *const T, pointer, 1);
        Ok(Box::from_raw(pointer))
    }
}
