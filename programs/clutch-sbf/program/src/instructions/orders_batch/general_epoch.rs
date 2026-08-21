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
//!   The freeze also **opens the candidate window** (T2-7): it stamps
//!   `selection_deadline_slot = freeze_slot + CANDIDATE_WINDOW_SLOTS` and the
//!   frozen set's exact live cardinality into the window — the `order_len`
//!   every submitted candidate must bind — which is why the window account
//!   is writable here from this revision on.
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

use super::terminal_closure;
use crate::accounts::{
    self, expect_pda, require, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program,
};
use crate::seeds;
use clutch_batch_policy_identity::revenue_policy_v1::{
    revenue_policy_digest, treasury_admits_fee_bearing, REVENUE_POLICY_V1,
};
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy,
    general_clearing_v1::{GENERAL_CLEARING_FEE_SHAPE_V1, GENERAL_CLEARING_POLICY_V1},
    BATCH_POLICY_BYTES,
};
use clutch_solana_layout::clearing::{
    canonical_general_book_id, open_general_epoch, EpochWindowAccount, CANDIDATE_WINDOW_SLOTS,
    EPOCH_WINDOW_ACCOUNT_BYTES, FUNDING_COVERS_EPOCH_PAIR, MAX_RETAINED_CANDIDATES,
};
use clutch_solana_layout::projection::OwnerInterner;
use clutch_solana_layout::revenue::{RevenuePolicyRecordV1, REVENUE_POLICY_RECORD_BYTES};
use clutch_solana_layout::{
    account_len, canonical_epoch_id, stream, EpochAccount, Hash32, OrderSlot, PositionAccount,
    EPOCH_PHASE_FROZEN, EPOCH_PHASE_OPEN, MAX_ORDER_PAGES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Accounts in an `InitEpoch` instruction without funding registration.
pub const INIT_EPOCH_ACCOUNT_COUNT: usize = 10;
/// Accounts in an `InitEpoch` instruction that also registers its funding:
/// one optional trailing `GeneralFundingLedgerV1` PDA covering the epoch and
/// window together, written in the same transition that debits the payer so
/// the recorded payer and exact post-prefund outlay are ground truth
/// (TerminalClosure's exact-principal-to-payer input).
pub const INIT_EPOCH_LEDGERED_ACCOUNT_COUNT: usize = INIT_EPOCH_ACCOUNT_COUNT + 1;
/// The optional funding-ledger PDA (zero-fee shape; the fee-bearing shape
/// carries it at [`IX_INIT_EPOCH_FEE_LEDGER`]).
pub const IX_INIT_EPOCH_LEDGER: usize = 10;
/// Accounts in a FEE-BEARING `InitEpoch` — one whose sealed policy artifact
/// wraps [`GENERAL_CLEARING_FEE_SHAPE_V1`]: the base list plus the Realm's
/// revenue-policy record and the Market's treasury Position, both read-only
/// (`docs/design/REVENUE_POLICY_V1.md` §5/§8; the admission seam of
/// `ADOPTED_2026-08-20.md` item 8).
pub const INIT_EPOCH_FEE_ACCOUNT_COUNT: usize = INIT_EPOCH_ACCOUNT_COUNT + 2;
/// A fee-bearing `InitEpoch` that also registers the pair's funding.
pub const INIT_EPOCH_FEE_LEDGERED_ACCOUNT_COUNT: usize = INIT_EPOCH_FEE_ACCOUNT_COUNT + 1;
/// The Realm's revenue-policy record (read-only, program-owned).  Fee shape.
pub const IX_INIT_EPOCH_REVENUE_RECORD: usize = 10;
/// The Market's treasury-owned Position (read-only, program-owned).  Fee
/// shape.
pub const IX_INIT_EPOCH_TREASURY_POSITION: usize = 11;
/// The optional funding-ledger PDA of the fee-bearing shape.
pub const IX_INIT_EPOCH_FEE_LEDGER: usize = 12;
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
/// The epoch's schedule window (writable, program-owned): the freeze stamps
/// the candidate-window deadline and the frozen set's live cardinality.
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
    require(
        accounts.len() == INIT_EPOCH_ACCOUNT_COUNT
            || accounts.len() == INIT_EPOCH_LEDGERED_ACCOUNT_COUNT
            || accounts.len() == INIT_EPOCH_FEE_ACCOUNT_COUNT
            || accounts.len() == INIT_EPOCH_FEE_LEDGERED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
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
    let fee_bearing = read_general_batch_policy(
        program_id,
        &accounts[IX_INIT_EPOCH_POLICY],
        epoch_id,
        *intent_policy,
    )?;
    /* The revenue admission seam (REVENUE_POLICY_V1.md §5/§8): a fee-bearing
     * epoch refuses unless the Realm's revenue record names a real treasury
     * and a treasury-owned Position exists for this Market.  Under the one
     * pinned V1 const the treasury is the structural UNSET sentinel, so this
     * walk ALWAYS stops at RevenueTreasuryUnset today — the B4a deferral as
     * a refusal, not a comment.  A zero-fee epoch must not smuggle the fee
     * shape's account tail. */
    if fee_bearing {
        require(
            accounts.len() >= INIT_EPOCH_FEE_ACCOUNT_COUNT,
            ClutchError::AccountCount,
        )?;
        require_fee_bearing_admission(
            program_id,
            &accounts[IX_INIT_EPOCH_REVENUE_RECORD],
            &accounts[IX_INIT_EPOCH_TREASURY_POSITION],
            market.realm,
            intent_market,
        )?;
    } else {
        require(
            accounts.len() <= INIT_EPOCH_LEDGERED_ACCOUNT_COUNT,
            ClutchError::AccountCount,
        )?;
    }
    let ledgered = if fee_bearing {
        accounts.len() == INIT_EPOCH_FEE_LEDGERED_ACCOUNT_COUNT
    } else {
        accounts.len() == INIT_EPOCH_LEDGERED_ACCOUNT_COUNT
    };
    let ledger_index = if fee_bearing {
        IX_INIT_EPOCH_FEE_LEDGER
    } else {
        IX_INIT_EPOCH_LEDGER
    };

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
    // The candidate-window fields open at their unstamped zeros: the freeze
    // that seals the book is what stamps the selection schedule and the
    // frozen set's live cardinality.
    let window = EpochWindowAccount {
        epoch: epoch_id,
        market: *intent_market,
        epoch_index,
        freeze_deadline_slot,
        selection_deadline_slot: 0,
        selected_slot: 0,
        selected_candidate: Hash32::ZERO,
        retained: [Hash32::ZERO; MAX_RETAINED_CANDIDATES],
        live_order_count: 0,
        retained_count: 0,
        stored_bump: window_bump,
        flags: 0,
    };
    window.validate()?;

    /* The pre-creation balances, captured before any lamport moves: the
     * optional funding ledger records the payer's exact post-prefund outlay
     * across the pair, never the rent minimum a prefund already discounted
     * (the artifact-prefund-windfall rule). */
    let epoch_prior = accounts[IX_INIT_EPOCH_EPOCH].lamports();
    let window_prior = accounts[IX_INIT_EPOCH_WINDOW].lamports();
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
    if ledgered {
        let outlay = terminal_closure::creation_shortfall(
            rent.minimum_balance(account_len::EPOCH)?,
            epoch_prior,
        )
        .checked_add(terminal_closure::creation_shortfall(
            rent.minimum_balance(EPOCH_WINDOW_ACCOUNT_BYTES)?,
            window_prior,
        ))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let floor = epoch_prior
            .checked_add(window_prior)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        terminal_closure::create_funding_ledger(
            program_id,
            &accounts[IX_INIT_EPOCH_PAYER],
            &accounts[ledger_index],
            &accounts[IX_INIT_EPOCH_SYSTEM],
            &rent,
            accounts[IX_INIT_EPOCH_EPOCH].key,
            FUNDING_COVERS_EPOCH_PAIR,
            outlay,
            floor,
        )?;
    }
    Ok(())
}

/// The fee-bearing admission requirements of `REVENUE_POLICY_V1.md` §5/§8,
/// checked before any byte is created.
///
/// In dependency order, each with its own refusal:
///
/// 1. **The record exists.**  Its absence is the zero-take state (D4) and
///    refuses with [`ClutchError::RevenuePolicyRecordMissing`] — permanently,
///    for every Realm created without the election.
/// 2. **The record binds this Realm and pins the frozen const's digest.**
/// 3. **The treasury is real.**  The pinned V1 const defers the key as the
///    structural UNSET sentinel, so this refusal
///    ([`ClutchError::RevenueTreasuryUnset`]) fires on every fee-bearing
///    admission until ember binds a key in a new const.
/// 4. **The named recipient exists as a live Position of this Market** —
///    treasury-owned, generation-live, `close_state == 0` — so the first
///    chargeable intent can never be admitted toward an absent or closing
///    destination (the mid-epoch-close grief discipline's admission half).
#[inline(never)]
fn require_fee_bearing_admission(
    program_id: &Pubkey,
    record_account: &AccountInfo,
    position_account: &AccountInfo,
    realm: Hash32,
    intent_market: &Hash32,
) -> Outcome<()> {
    require(
        *record_account.owner == *program_id && record_account.data_len() != 0,
        ClutchError::RevenuePolicyRecordMissing,
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        record_account,
        false,
        &[REVENUE_POLICY_RECORD_BYTES],
    )?;
    let record = RevenuePolicyRecordV1::decode(&record_account.data.borrow())?;
    require(record.realm == realm, ClutchError::MismatchedState)?;
    expect_pda(
        record_account.key,
        seeds::revenue_policy_pda(program_id, &realm.bytes()),
        Some(record.stored_bump),
    )?;
    let digest = revenue_policy_digest(&REVENUE_POLICY_V1)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        record.policy_digest.bytes() == digest.0,
        ClutchError::MismatchedState,
    )?;
    require(
        treasury_admits_fee_bearing(&record.treasury.bytes()),
        ClutchError::RevenueTreasuryUnset,
    )?;
    /* Unreachable until a const binds a real treasury; the seam is complete
     * so that binding one is a const + digest decision, not new plumbing. */
    accounts::validate_state_role_lengths(
        program_id,
        position_account,
        false,
        &[account_len::POSITION],
    )?;
    let position = PositionAccount::decode(&position_account.data.borrow())?;
    require(
        position.market == *intent_market
            && position.owner == record.treasury
            && position.close_state == 0,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        position_account.key,
        seeds::position_pda(program_id, &intent_market.bytes(), &record.treasury.bytes()),
        Some(position.stored_bump),
    )?;
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
        true,
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

    let mut window = EpochWindowAccount::decode(&accounts[IX_FREEZE_EPOCH_WINDOW].data.borrow())?;
    require(
        window.epoch == epoch.epoch
            && window.market == epoch.market
            && window.epoch_index == epoch.epoch_index,
        ClutchError::MismatchedState,
    )?;
    // An OPEN epoch's window is unstamped by construction; stated so a
    // fabricated pre-stamped window cannot smuggle a schedule past the freeze.
    require(
        window.selection_deadline_slot == 0 && window.live_order_count == 0,
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
    let (order_set, set_order_count, live_order_count, head_first, tail_last) = {
        let borrows: Vec<core::cell::Ref<'_, &mut [u8]>> = accounts[IX_FREEZE_EPOCH_PAGES..]
            .iter()
            .map(|page| page.data.borrow())
            .collect();
        let refs: Vec<&[u8]> = borrows.iter().map(|data| &***data as &[u8]).collect();
        let (order_set, set_order_count) = stream::frozen_set_commitment(&refs)?;
        // Live records are counted in the same walk that interns owners: the
        // count the window stamps is the exact number of orders the pass-1
        // walk will feed, which is the `order_len` every admitted candidate
        // must bind (T2-4's live-cardinality rule).
        let mut live_order_count: u16 = 0;
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
                        live_order_count += 1;
                    }
                    OrderSlot::Portfolio(record) => {
                        owners.intern(record.owner)?;
                        live_order_count += 1;
                    }
                    // A retired record is never fed, so it mints no tag.
                    OrderSlot::Tombstone(_) => {}
                    OrderSlot::Empty => {
                        return Err(Refusal::Codec(
                            clutch_solana_layout::CodecError::ZeroIdentity,
                        ))
                    }
                }
                index += 1;
            }
        }
        let head = stream::OrderPageHeader::decode(refs[0])?;
        let tail = stream::OrderPageHeader::decode(refs[refs.len() - 1])?;
        (
            order_set,
            set_order_count,
            live_order_count,
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

    // The freeze that closed placement opens the candidate window: the
    // selection deadline is the pinned schedule span from *this* slot (the
    // freeze may legitimately run after its deadline; the window's length is
    // fixed, its position rides the freeze), and the exact live cardinality
    // is what every submitted candidate's `order_len` must equal.
    window.selection_deadline_slot = now
        .checked_add(CANDIDATE_WINDOW_SLOTS)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    window.live_order_count = live_order_count;
    window.validate()?;

    epoch.encode(&mut borrow_account_mut(&accounts[IX_FREEZE_EPOCH_EPOCH])?)?;
    window.encode(&mut borrow_account_mut(&accounts[IX_FREEZE_EPOCH_WINDOW])?)?;
    Ok(())
}

/// Read and authenticate the general 64-byte batch-policy artifact.
///
/// The general sibling of `direct_selection::read_batch_policy`: the account
/// at `seeds::batch_policy_pda(epoch, digest)` must decode to exactly one of
/// the two enumerated consts — the frozen zero-fee
/// [`GENERAL_CLEARING_POLICY_V1`] or its fee-bearing sibling
/// [`GENERAL_CLEARING_FEE_SHAPE_V1`] (shape only, both rates zero) — and
/// re-derive the expected digest.  Nothing dynamic is ever admitted.
/// Returns whether the epoch is fee-bearing, which is what obliges the
/// revenue admission seam.
#[inline(never)]
fn read_general_batch_policy(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: Hash32,
    expected_digest: Hash32,
) -> Outcome<bool> {
    let policy = decode_batch_policy(&account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let digest =
        batch_policy_digest(&policy).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        (policy == GENERAL_CLEARING_POLICY_V1 || policy == GENERAL_CLEARING_FEE_SHAPE_V1)
            && digest.0 == expected_digest.bytes(),
        ClutchError::AuthorizationUnavailable,
    )?;
    expect_pda(
        account.key,
        seeds::batch_policy_pda(program_id, &epoch.bytes(), &expected_digest.bytes()),
        None,
    )?;
    Ok(policy == GENERAL_CLEARING_FEE_SHAPE_V1)
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
    super::boxed_copy_of(&EMPTY)
}
