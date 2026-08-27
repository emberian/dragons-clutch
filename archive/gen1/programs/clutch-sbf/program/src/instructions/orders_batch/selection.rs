//! Candidate window closure and selection — Tier 2 join 7, T2-7.
//!
//! `Intent::SubmitCandidate` (tag 54), `Intent::WriteCandidateFeed` (55),
//! `Intent::SealCandidate` (56), and `Intent::FinalizeSelection` (57): the
//! general submission and selection lifecycle over the accounts the walk
//! (tags 51-53) verifies.
//!
//! ## The staged submission (54 → 55 × n → 56)
//!
//! A candidate feed's content — up to 64 fills and 416 witness slices,
//! 6,266 bytes — cannot ride one transaction, so submission is staged with
//! the two in-family precedents composed: the artifact transport's
//! begin/write/seal shape carries the content, and the checkpoint's
//! staged-creation discipline makes the intermediate state inert.
//! `SubmitCandidate` creates the `SUBMITTED` [`CandidateRecord`] and a
//! full-length feed account carrying a [`clearing::CandidateFeedStage`]
//! prefix — a different account **tag**, so every feed consumer (the walk
//! included) refuses the incomplete feed with `WrongTag` and zero new code.
//! `WriteCandidateFeed` appends content chunks at the stage's one sequential
//! cursor (fills, then slices; replay held by state: the envelope sequence is
//! the elements-written count).  `SealCandidate` is the one-way door: the
//! stage prefix becomes the real [`clearing::CandidateFeedHeader`] and the whole
//! account re-verifies.  After the seal no stage write can land, which is what
//! keeps the bytes the walk streams and the bytes the tie digest folds one
//! region.  Sealing does **not** admit a score claim into the retained registry:
//! [`super::clear_walk::complete_clear_work`] inserts only after the streaming
//! verifier has produced the full verified score.
//!
//! The candidate identity never travels: the program recomputes it from the
//! submitted free coordinates exactly as both account codecs do, and
//! `order_len` is the frozen window's stamped live cardinality (T2-4's
//! live-cardinality binding, via [`CandidateRecord::binds_epoch`]), never a
//! claim.
//!
//! ## The verified-only bounded registry
//!
//! The window retains at most [`MAX_RETAINED_CANDIDATES`] candidates (the V3
//! window's bound), and every retained record is `VERIFIED`.  A successful
//! `CompleteClearWork` appends while capacity remains.  Against a full registry
//! it compares the complete [`FullScoreV1`] — including the verified full-width
//! digest — and replaces the worst only when the incoming score is strictly
//! better.  The displaced record becomes `SUPERSEDED` with a zero digest.  Its
//! feed deliberately remains present for the terminal close family; verified
//! admission never needs a writable feed whose bytes it has already checked.
//!
//! ## Selection (57)
//!
//! Permissionless at or after the window's `selection_deadline_slot`
//! (stamped by the freeze; before it, everyone refuses).  The `VERIFIED`
//! retained candidates compete by `FullScoreV1::total_order` over their
//! **persisted verified components** and the full-width tie digest — never a
//! claimed score, and never trusting the stored digest either: for every
//! verified candidate the tie digest is **re-derived** from the full-width
//! domain and the presented feed's stored regions
//! (`super::clear_walk::recompute_tie_digest`, `pub(super)`; the same
//! construction the
//! close stamped), and a mismatch refuses with
//! [`ClutchError::ScoreDigestMismatch`].  The winner takes
//! `CANDIDATE_STATUS_SELECTED`, the window records it, and the epoch takes
//! `EPOCH_PHASE_CLEARED`.  With zero verified candidates the epoch **lapses
//! honestly** to `EPOCH_PHASE_LAPSED` — the V3 pre-selection lapse projected
//! onto the general plane: nothing is selected, and either terminal phase is
//! what admits the whole [`super::terminal_closure`] family (owner-signed
//! release of every lapsed ACTIVE reservation, then the dependency-ordered
//! rent closes down to the epoch root).  The second finalize refuses on the
//! phase either way.
//!
//! This is the best valid submitted candidate verified and admitted before the
//! shared deadline.  A sealed candidate whose verification does not finish
//! before the deadline does not compete.
//!
//! ## Claim plane
//!
//! SBF-EXECUTED (bank), no promotion.  The reference adapter refuses all
//! four intents with `UnsupportedIntent`, so the SVM oracle for this family
//! is the layout codec byte-for-byte, per the genesis precedent.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program,
};
use crate::seeds;
use clutch_batch_policy_identity::{FullScoreV1, Identity32V1};
use clutch_solana_layout::clearing::{
    self, canonical_general_book_id, CandidateFeedHeader, CandidateFeedStage, EpochWindowAccount,
    EPOCH_WINDOW_ACCOUNT_BYTES, MAX_RETAINED_CANDIDATES,
};
use clutch_solana_layout::{
    account_len, CandidateFeedChunk, CandidateRecord, EpochAccount, Hash32,
    CANDIDATE_STATUS_SELECTED, CANDIDATE_STATUS_SUBMITTED, CANDIDATE_STATUS_SUPERSEDED,
    CANDIDATE_STATUS_VERIFIED, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN, EPOCH_PHASE_LAPSED,
    MAX_EPOCH_ORDERS, MAX_OUTCOMES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Accounts in a `SubmitCandidate` instruction without funding registration.
pub const SUBMIT_CANDIDATE_ACCOUNT_COUNT: usize = 8;
/// Accounts in a `SubmitCandidate` instruction that also registers its
/// funding: one optional trailing `GeneralFundingLedgerV1` PDA covering the
/// record and feed together, written in the same transition that debits the
/// payer (TerminalClosure's exact-principal-to-payer input).
pub const SUBMIT_CANDIDATE_LEDGERED_ACCOUNT_COUNT: usize = SUBMIT_CANDIDATE_ACCOUNT_COUNT + 1;
/// The optional funding-ledger PDA.
pub const IX_SUBMIT_CAND_LEDGER: usize = 8;
/// Authenticated payer funding both creations.
pub const IX_SUBMIT_CAND_PAYER: usize = 0;
/// The frozen general epoch (read-only, program-owned).
pub const IX_SUBMIT_CAND_EPOCH: usize = 1;
/// The epoch's stamped schedule window (read-only, program-owned).
pub const IX_SUBMIT_CAND_WINDOW: usize = 2;
/// The canonical, not-yet-created candidate-record PDA.
pub const IX_SUBMIT_CAND_RECORD: usize = 3;
/// The canonical, not-yet-created candidate-feed PDA.
pub const IX_SUBMIT_CAND_FEED: usize = 4;
/// System program.
pub const IX_SUBMIT_CAND_SYSTEM: usize = 5;
/// Rent sysvar.
pub const IX_SUBMIT_CAND_RENT: usize = 6;
/// Clock sysvar.
pub const IX_SUBMIT_CAND_CLOCK: usize = 7;

/// Accounts in a `WriteCandidateFeed` instruction, exactly.
///
/// One: the staging feed.  Its stage prefix is the whole authority — the
/// stored triple must match the wire and re-derive the address — and the
/// sequence-by-state cursor is the replay guard, exactly the `GrowClearWork`
/// shape.
pub const WRITE_CANDIDATE_FEED_ACCOUNT_COUNT: usize = 1;
/// The staging feed PDA.
pub const IX_WRITE_FEED_FEED: usize = 0;

/// Accounts in a `SealCandidate` instruction, exactly.
pub const SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT: usize = 4;
/// The frozen general epoch (read-only, program-owned).
pub const IX_SEAL_EPOCH: usize = 0;
/// The epoch's schedule window (read-only, program-owned).
pub const IX_SEAL_WINDOW: usize = 1;
/// The complete staging feed being sealed (writable, program-owned).
pub const IX_SEAL_FEED: usize = 2;
/// Clock sysvar.
pub const IX_SEAL_CLOCK: usize = 3;

/// Fixed accounts in a `FinalizeSelection` instruction, before the
/// per-candidate pairs.
pub const FINALIZE_SELECTION_FIXED_ACCOUNT_COUNT: usize = 3;
/// The frozen general epoch (writable, program-owned).
pub const IX_FINALIZE_SEL_EPOCH: usize = 0;
/// The epoch's schedule window (writable, program-owned).
pub const IX_FINALIZE_SEL_WINDOW: usize = 1;
/// Clock sysvar.
pub const IX_FINALIZE_SEL_CLOCK: usize = 2;
/// First `(record, feed)` pair; one pair per retained candidate, in registry
/// order.
pub const IX_FINALIZE_SEL_PAIRS: usize = FINALIZE_SELECTION_FIXED_ACCOUNT_COUNT;

/// Create one general candidate submission: the `SUBMITTED` record and the
/// staging feed, at their canonical PDAs.
#[allow(clippy::too_many_arguments)] // one argument per wire coordinate
#[inline(never)]
pub(super) fn submit_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    prices: &[u64; MAX_OUTCOMES],
    virtual_split: u64,
    virtual_merge: u64,
    honored_aon_mask: u64,
    declared_slices: Option<u16>,
    weighted_direct_volume: i128,
    limit_surplus_price_units: u128,
    distinct_owners: u16,
) -> Outcome<()> {
    require(
        accounts.len() == SUBMIT_CANDIDATE_ACCOUNT_COUNT
            || accounts.len() == SUBMIT_CANDIDATE_LEDGERED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    /* The real replay guard is both targets' non-existence
     * (`require_creatable` below); the zero sequence states the intent's
     * position in the protocol on the wire, as the other creators do. */
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[IX_SUBMIT_CAND_PAYER])?;
    require(
        accounts[IX_SUBMIT_CAND_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_SUBMIT_CAND_EPOCH],
        false,
        &[account_len::EPOCH],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_SUBMIT_CAND_WINDOW],
        false,
        &[EPOCH_WINDOW_ACCOUNT_BYTES],
    )?;
    require_creatable(&accounts[IX_SUBMIT_CAND_RECORD])?;
    require_creatable(&accounts[IX_SUBMIT_CAND_FEED])?;
    require_system_program(&accounts[IX_SUBMIT_CAND_SYSTEM])?;
    let rent = read_rent(&accounts[IX_SUBMIT_CAND_RENT])?;
    let now = read_clock_slot(&accounts[IX_SUBMIT_CAND_CLOCK])?;

    let epoch = load_frozen_epoch(
        program_id,
        &accounts[IX_SUBMIT_CAND_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    let window = load_bound_window(program_id, &accounts[IX_SUBMIT_CAND_WINDOW], &epoch)?;
    // The candidate window: open from the freeze's stamp, closed at the
    // selection deadline.
    require(
        window.selection_deadline_slot != 0 && now < window.selection_deadline_slot,
        ClutchError::NotActive,
    )?;
    // The frozen set's exact live cardinality is the only admitted
    // `order_len`; a book with nothing live takes no candidates.
    let live = window.live_order_count;
    require(
        live >= 1 && live as usize <= MAX_EPOCH_ORDERS,
        ClutchError::MismatchedState,
    )?;

    let record = build_submitted_record(
        &epoch,
        live,
        prices,
        virtual_split,
        virtual_merge,
        honored_aon_mask,
        weighted_direct_volume,
        limit_surplus_price_units,
        distinct_owners,
        now,
    )?;

    let epoch_bytes = epoch.epoch.bytes();
    let candidate_bytes = record.candidate.bytes();
    let (record_address, record_bump) =
        seeds::candidate_pda(program_id, &epoch_bytes, &candidate_bytes);
    expect_pda(
        accounts[IX_SUBMIT_CAND_RECORD].key,
        (record_address, record_bump),
        None,
    )?;

    // Both creations, then both writes; a later refusal rolls the whole
    // transaction — creations included — back at the boundary.  The
    // pre-creation balances are captured first: the optional funding ledger
    // records the payer's exact post-prefund outlay, never a discounted
    // minimum (the artifact-prefund-windfall rule).
    let record_prior = accounts[IX_SUBMIT_CAND_RECORD].lamports();
    let feed_prior = accounts[IX_SUBMIT_CAND_FEED].lamports();
    let record_bump_seed = [record_bump];
    let record_signer = [
        seeds::SEED_CANDIDATE,
        epoch_bytes.as_ref(),
        candidate_bytes.as_ref(),
        record_bump_seed.as_ref(),
    ];
    create_pda_account(
        program_id,
        &accounts[IX_SUBMIT_CAND_PAYER],
        &accounts[IX_SUBMIT_CAND_RECORD],
        &accounts[IX_SUBMIT_CAND_SYSTEM],
        &rent,
        account_len::CANDIDATE,
        &record_signer,
    )?;
    let feed_bump = super::clear_work::create_candidate_feed_account(
        program_id,
        &accounts[IX_SUBMIT_CAND_PAYER],
        &accounts[IX_SUBMIT_CAND_FEED],
        &accounts[IX_SUBMIT_CAND_SYSTEM],
        &rent,
        &epoch.epoch,
        &record.candidate,
    )?;

    write_submitted_pair(
        &accounts[IX_SUBMIT_CAND_RECORD],
        &accounts[IX_SUBMIT_CAND_FEED],
        &epoch,
        &record,
        record_bump,
        feed_bump,
        declared_slices,
    )?;
    if accounts.len() == SUBMIT_CANDIDATE_LEDGERED_ACCOUNT_COUNT {
        /* The record is shortfall-created; the feed moves its full principal
         * regardless of prefund (`create_pda_account_full_principal`). */
        let outlay = super::terminal_closure::creation_shortfall(
            rent.minimum_balance(account_len::CANDIDATE)?,
            record_prior,
        )
        .checked_add(rent.minimum_balance(account_len::CANDIDATE_FEED)?)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let floor = record_prior
            .checked_add(feed_prior)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        super::terminal_closure::create_funding_ledger(
            program_id,
            &accounts[IX_SUBMIT_CAND_PAYER],
            &accounts[IX_SUBMIT_CAND_LEDGER],
            &accounts[IX_SUBMIT_CAND_SYSTEM],
            &rent,
            accounts[IX_SUBMIT_CAND_RECORD].key,
            clearing::FUNDING_COVERS_CANDIDATE_PAIR,
            outlay,
            floor,
        )?;
    }
    Ok(())
}

/// Append one content chunk to a staging feed at its sequential cursor.
#[inline(never)]
pub(super) fn write_candidate_feed(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
    chunk: &CandidateFeedChunk,
) -> Outcome<()> {
    require_count(accounts, WRITE_CANDIDATE_FEED_ACCOUNT_COUNT)?;
    let feed = &accounts[IX_WRITE_FEED_FEED];
    require(feed.is_writable, ClutchError::NotWritable)?;
    require(!feed.executable, ClutchError::ExecutableAccount)?;
    require(feed.owner == program_id, ClutchError::WrongProgramOwner)?;

    /* The stage prefix authenticates the account: the stage tag refuses every
     * sealed feed (and every other family), and the stored triple must both
     * match the wire and re-derive this exact address. */
    let stage = read_stage(feed)?;
    require(
        stage.market == *intent_market
            && stage.epoch == *intent_epoch
            && stage.candidate == *intent_candidate,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        feed.key,
        seeds::candidate_feed_pda(program_id, &intent_epoch.bytes(), &intent_candidate.bytes()),
        Some(stage.stored_bump),
    )?;
    /* Replay by state: the stage's own element cursor is the counter, so a
     * replayed or out-of-order chunk refuses against what the account already
     * holds, exactly as a checkpoint grow refuses against its length. */
    require(sequence == stage.written(), ClutchError::Replay)?;

    let mut data = borrow_account_mut(feed)?;
    match chunk {
        CandidateFeedChunk::Fills { count, fills } => {
            clearing::stage_append_fills(&mut data, &fills[..*count as usize])?;
        }
        CandidateFeedChunk::Slices { count, slices } => {
            clearing::stage_append_slices(&mut data, &slices[..*count as usize])?;
        }
    }
    Ok(())
}

/// Seal one complete staging feed.
///
/// The feed becomes immutable and walkable here.  Its unverified score claims
/// never enter the retained registry; successful `CompleteClearWork` owns that
/// later transition after it has recomputed the complete score.
#[inline(never)]
pub(super) fn seal_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require_count(accounts, SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT)?;
    require_distinct(accounts)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_SEAL_WINDOW],
        false,
        &[EPOCH_WINDOW_ACCOUNT_BYTES],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_SEAL_EPOCH],
        false,
        &[account_len::EPOCH],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_SEAL_FEED],
        true,
        &[account_len::CANDIDATE_FEED],
    )?;

    let epoch = load_frozen_epoch(
        program_id,
        &accounts[IX_SEAL_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    let window = load_bound_window(program_id, &accounts[IX_SEAL_WINDOW], &epoch)?;
    let now = read_clock_slot(&accounts[IX_SEAL_CLOCK])?;
    // Admission closes with the candidate window, exactly like submission.
    require(
        window.selection_deadline_slot != 0 && now < window.selection_deadline_slot,
        ClutchError::NotActive,
    )?;

    // The sealing feed: a complete stage for this exact candidate, computed
    // against this exact frozen set.
    let stage = read_stage(&accounts[IX_SEAL_FEED])?;
    require(
        stage.market == epoch.market
            && stage.epoch == epoch.epoch
            && stage.candidate == *intent_candidate
            && stage.order_set == epoch.order_set
            && stage.order_len as u16 == window.live_order_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_SEAL_FEED].key,
        seeds::candidate_feed_pda(program_id, &epoch.epoch.bytes(), &intent_candidate.bytes()),
        Some(stage.stored_bump),
    )?;
    // The one-way door: stage prefix becomes the real header and the whole feed
    // re-verifies.  No score claim or registry state is touched.
    {
        let mut data = borrow_account_mut(&accounts[IX_SEAL_FEED])?;
        clearing::seal_candidate_feed(&mut data)?;
    }
    Ok(())
}

/// Close one epoch's candidate window: select the best verified candidate,
/// or lapse honestly when none verified.
#[inline(never)]
pub(super) fn finalize_selection(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    require(
        accounts.len() >= FINALIZE_SELECTION_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_FINALIZE_SEL_EPOCH],
        true,
        &[account_len::EPOCH],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_FINALIZE_SEL_WINDOW],
        true,
        &[EPOCH_WINDOW_ACCOUNT_BYTES],
    )?;

    // A second finalize refuses here: the phase left FROZEN the first time,
    // whether it cleared or lapsed.
    let mut epoch = load_frozen_epoch(
        program_id,
        &accounts[IX_FINALIZE_SEL_EPOCH],
        intent_market,
        intent_epoch,
    )?;
    let mut window = load_bound_window(program_id, &accounts[IX_FINALIZE_SEL_WINDOW], &epoch)?;
    let retained_count = window.retained_count as usize;
    require_count(
        accounts,
        FINALIZE_SELECTION_FIXED_ACCOUNT_COUNT + (2 * retained_count),
    )?;
    require_distinct(accounts)?;
    let mut index = 0usize;
    while index < retained_count {
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[IX_FINALIZE_SEL_PAIRS + (2 * index)],
            true,
            &[account_len::CANDIDATE],
        )?;
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[IX_FINALIZE_SEL_PAIRS + (2 * index) + 1],
            false,
            &[account_len::CANDIDATE_FEED],
        )?;
        index += 1;
    }
    let now = read_clock_slot(&accounts[IX_FINALIZE_SEL_CLOCK])?;
    // The deadline is the whole authority: before it, everyone refuses; at
    // or after it, anyone may finalize.
    require(
        window.selection_deadline_slot != 0 && now >= window.selection_deadline_slot,
        ClutchError::NotActive,
    )?;

    // Every retained candidate is authenticated; only VERIFIED ones compete,
    // each with its tie digest re-derived from the presented feed bytes.
    let mut best: Option<(usize, FullScoreV1)> = None;
    index = 0;
    while index < retained_count {
        let score = candidate_selection_score(
            program_id,
            &accounts[IX_FINALIZE_SEL_PAIRS + (2 * index)],
            &accounts[IX_FINALIZE_SEL_PAIRS + (2 * index) + 1],
            &epoch,
            window.retained[index],
        )?;
        if let Some(score) = score {
            best = match best {
                Some((_, leading)) if !score.is_better_than(&leading) => best,
                _ => Some((index, score)),
            };
        }
        index += 1;
    }

    match best {
        Some((winner_index, _)) => {
            let winner = select_winner(&accounts[IX_FINALIZE_SEL_PAIRS + (2 * winner_index)])?;
            window.selected_candidate = winner;
            window.selected_slot = now;
            epoch.phase = EPOCH_PHASE_CLEARED;
        }
        None => {
            /* The honest empty lapse (V3's pre-selection lapse projected
             * onto the general plane): no verified candidate exists at the
             * deadline, so nothing is selected, nothing is invented, and the
             * epoch's terminal phase says so.  The lapse is exactly what
             * admits the terminal-closure family: every ACTIVE reservation
             * becomes owner-releasable (tag 60) and the machinery closes in
             * dependency order down to the epoch root (tags 61-67). */
            epoch.phase = EPOCH_PHASE_LAPSED;
        }
    }
    epoch.validate()?;
    window.validate()?;
    epoch.encode(&mut borrow_account_mut(&accounts[IX_FINALIZE_SEL_EPOCH])?)?;
    window.encode(&mut borrow_account_mut(&accounts[IX_FINALIZE_SEL_WINDOW])?)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Shared loads and frames                                                   */
/* ------------------------------------------------------------------------ */

/// Decode and authenticate the frozen general epoch.
#[inline(never)]
fn load_frozen_epoch(
    program_id: &Pubkey,
    account: &AccountInfo,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<Box<EpochAccount>> {
    let epoch = super::decode_epoch_boxed(&account.data.borrow())?;
    require(
        epoch.market == *intent_market && epoch.epoch == *intent_epoch,
        ClutchError::MismatchedState,
    )?;
    require(epoch.phase == EPOCH_PHASE_FROZEN, ClutchError::NotActive)?;
    // Only the general book family selects here; a direct epoch is a
    // different account length, and a general epoch's book identity is a
    // total function of its epoch identity.
    require(
        epoch.book == canonical_general_book_id(epoch.epoch),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::epoch_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;
    Ok(epoch)
}

/// Decode and authenticate the epoch's schedule window.
#[inline(never)]
fn load_bound_window(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
) -> Outcome<EpochWindowAccount> {
    let window = EpochWindowAccount::decode(&account.data.borrow())?;
    require(
        window.epoch == epoch.epoch
            && window.market == epoch.market
            && window.epoch_index == epoch.epoch_index,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::epoch_window_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(window.stored_bump),
    )?;
    Ok(window)
}

/// A prepared verified-only registry mutation.
///
/// All authentication and score comparisons finish before the close writes any
/// account.  When `displaced` is present, its index names the writable retained
/// record that must receive the returned `SUPERSEDED` body.
pub(super) struct VerifiedAdmission {
    pub(super) window: EpochWindowAccount,
    pub(super) displaced: Option<(usize, Box<CandidateRecord>)>,
}

/// Load the writable window and enforce the shared verification/admission
/// deadline.
///
/// Clearing may stream for several transactions, but a verdict only enters the
/// selection set before the same deadline at which finalization opens.  This
/// removes the old post-deadline `CompleteClearWork`/`FinalizeSelection` race.
#[inline(never)]
pub(super) fn load_open_admission_window(
    program_id: &Pubkey,
    window_account: &AccountInfo,
    clock_account: &AccountInfo,
    epoch: &EpochAccount,
) -> Outcome<EpochWindowAccount> {
    accounts::validate_state_role_lengths(
        program_id,
        window_account,
        true,
        &[EPOCH_WINDOW_ACCOUNT_BYTES],
    )?;
    let window = load_bound_window(program_id, window_account, epoch)?;
    let now = read_clock_slot(clock_account)?;
    require(
        window.selection_deadline_slot != 0 && now < window.selection_deadline_slot,
        ClutchError::NotActive,
    )?;
    Ok(window)
}

/// Authenticate the verified registry and prepare insertion of one newly
/// verified candidate by the complete five-coordinate score order.
#[inline(never)]
pub(super) fn prepare_verified_admission(
    program_id: &Pubkey,
    retained_accounts: &[AccountInfo],
    epoch: &EpochAccount,
    incoming_record: &CandidateRecord,
    mut window: EpochWindowAccount,
) -> Outcome<Option<VerifiedAdmission>> {
    let retained_count = usize::from(window.retained_count);
    require(
        retained_accounts.len() == retained_count,
        ClutchError::AccountCount,
    )?;
    require(
        incoming_record.status == CANDIDATE_STATUS_VERIFIED
            && incoming_record.epoch == epoch.epoch
            && incoming_record.market == epoch.market,
        ClutchError::MismatchedState,
    )?;
    incoming_record.binds_epoch(epoch, window.live_order_count)?;
    let incoming = score_of(incoming_record);

    let mut scores = [FullScoreV1::ZERO; MAX_RETAINED_CANDIDATES];
    let mut index = 0usize;
    while index < retained_count {
        accounts::validate_state_role_lengths(
            program_id,
            &retained_accounts[index],
            true,
            &[account_len::CANDIDATE],
        )?;
        let retained = load_retained_record(
            program_id,
            &retained_accounts[index],
            epoch,
            window.retained[index],
        )?;
        scores[index] = score_of(&retained);
        index += 1;
    }

    let displaced = if retained_count == MAX_RETAINED_CANDIDATES {
        let mut worst = 0usize;
        index = 1;
        while index < retained_count {
            if scores[worst].is_better_than(&scores[index]) {
                worst = index;
            }
            index += 1;
        }
        if !incoming.is_better_than(&scores[worst]) {
            // Verification is still a durable fact even when this candidate is
            // outside the bounded top three.  CompleteClearWork stamps VERIFIED
            // and closes its checkpoint, but the registry remains byte-stable.
            return Ok(None);
        }
        let mut record = load_retained_record(
            program_id,
            &retained_accounts[worst],
            epoch,
            window.retained[worst],
        )?;
        record.status = CANDIDATE_STATUS_SUPERSEDED;
        record.score_digest = Hash32::ZERO;
        window.retained[worst] = incoming_record.candidate;
        Some((worst, record))
    } else {
        window.retained[retained_count] = incoming_record.candidate;
        window.retained_count += 1;
        None
    };

    window.validate()?;
    Ok(Some(VerifiedAdmission { window, displaced }))
}

/// Build the `SUBMITTED` record for one submission's coordinates and claims.
///
/// The candidate identity is recomputed, never claimed; `binds_epoch` is
/// T2-4's exact live-cardinality binding against the frozen domain, which is
/// also where the simplex and the mask width are decided.
#[allow(clippy::too_many_arguments)] // one argument per wire coordinate
#[inline(never)]
fn build_submitted_record(
    epoch: &EpochAccount,
    live_order_count: u16,
    prices: &[u64; MAX_OUTCOMES],
    virtual_split: u64,
    virtual_merge: u64,
    honored_aon_mask: u64,
    weighted_direct_volume: i128,
    limit_surplus_price_units: u128,
    distinct_owners: u16,
    now: u64,
) -> Outcome<Box<CandidateRecord>> {
    let churn = virtual_split
        .checked_add(virtual_merge)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut record = Box::new(CandidateRecord {
        candidate: Hash32::ZERO,
        epoch: epoch.epoch,
        market: epoch.market,
        prices: *prices,
        virtual_split,
        virtual_merge,
        honored_aon_mask,
        weighted_direct_volume,
        limit_surplus_price_units,
        score_digest: Hash32::ZERO,
        churn,
        submitted_slot: now,
        distinct_owners,
        order_len: live_order_count as u8,
        outcome_count: epoch.outcome_count,
        status: CANDIDATE_STATUS_SUBMITTED,
        stored_bump: 0,
        flags: 0,
    });
    record.candidate = record.recomputed_candidate_digest()?;
    record.binds_epoch(epoch, live_order_count)?;
    Ok(record)
}

/// Persist the submission pair: the record, then the stage prefix.
#[inline(never)]
fn write_submitted_pair(
    record_account: &AccountInfo,
    feed_account: &AccountInfo,
    epoch: &EpochAccount,
    record: &CandidateRecord,
    record_bump: u8,
    feed_bump: u8,
    declared_slices: Option<u16>,
) -> Outcome<()> {
    let mut stamped = *record;
    stamped.stored_bump = record_bump;
    stamped.encode(&mut borrow_account_mut(record_account)?)?;
    let stage = CandidateFeedStage {
        candidate: record.candidate,
        epoch: record.epoch,
        market: record.market,
        order_set: epoch.order_set,
        prices: record.prices,
        virtual_split: record.virtual_split,
        virtual_merge: record.virtual_merge,
        honored_aon_mask: record.honored_aon_mask,
        weighted_direct_volume: record.weighted_direct_volume,
        limit_surplus_price_units: record.limit_surplus_price_units,
        declared_slices: declared_slices.unwrap_or(0),
        distinct_owners: record.distinct_owners,
        fills_written: 0,
        slices_written: 0,
        order_len: record.order_len,
        outcome_count: record.outcome_count,
        stored_bump: feed_bump,
        flags: u8::from(declared_slices.is_some()) * clearing::CANDIDATE_FEED_FLAG_SLICES_DECLARED,
    };
    let mut data = borrow_account_mut(feed_account)?;
    clearing::init_candidate_feed_stage(&mut data, &stage)?;
    Ok(())
}

/// Decode the stage prefix; the borrow never outlives this frame.
#[inline(never)]
fn read_stage(account: &AccountInfo) -> Outcome<CandidateFeedStage> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(CandidateFeedStage::decode(&data)?)
}

/// The complete verified score persisted on one candidate record.
fn score_of(record: &CandidateRecord) -> FullScoreV1 {
    FullScoreV1 {
        weighted_direct_volume: record.weighted_direct_volume,
        limit_surplus_price_units: record.limit_surplus_price_units,
        distinct_owners: record.distinct_owners,
        churn: record.churn,
        digest: Identity32V1(record.score_digest.bytes()),
    }
}

/// Decode one retained candidate record and bind it to its registry slot.
#[inline(never)]
fn load_retained_record(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    expected: Hash32,
) -> Outcome<Box<CandidateRecord>> {
    let record = super::decode_candidate_boxed(&account.data.borrow())?;
    require(
        record.candidate == expected
            && record.epoch == epoch.epoch
            && record.market == epoch.market,
        ClutchError::MismatchedState,
    )?;
    // Verified admission is the registry's only writer.  SUPERSEDED left the
    // registry when displaced, and SELECTED exists only after the one finalize,
    // which the epoch phase already forbids repeating.
    require(
        record.status == CANDIDATE_STATUS_VERIFIED,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::candidate_pda(program_id, &epoch.epoch.bytes(), &expected.bytes()),
        Some(record.stored_bump),
    )?;
    Ok(record)
}

/// One retained candidate's verified selection score.
///
/// Every retained candidate is authenticated — record against registry entry,
/// feed against candidate and frozen set — whether or not it competes.  A
/// VERIFIED candidate additionally has its tie digest re-derived from the
/// presented feed bytes; a mismatch refuses the whole instruction rather than
/// excluding the candidate, because tampered state is a fault, not a loser.
#[inline(never)]
fn candidate_selection_score(
    program_id: &Pubkey,
    record_account: &AccountInfo,
    feed_account: &AccountInfo,
    epoch: &EpochAccount,
    expected: Hash32,
) -> Outcome<Option<FullScoreV1>> {
    let record = load_retained_record(program_id, record_account, epoch, expected)?;
    let data = feed_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let feed = CandidateFeedHeader::decode(&data)?;
    // The feed binding matrix, as the walk binds it: one candidate, one
    // epoch, one market, one frozen set, one width.
    require(
        feed.candidate == expected
            && feed.epoch == epoch.epoch
            && feed.market == epoch.market
            && feed.order_set == epoch.order_set
            && feed.outcome_count == epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        feed_account.key,
        seeds::candidate_feed_pda(program_id, &epoch.epoch.bytes(), &expected.bytes()),
        Some(feed.stored_bump),
    )?;
    let recomputed = super::clear_walk::recompute_tie_digest(epoch, &feed, &data)?;
    require(
        recomputed == record.score_digest,
        ClutchError::ScoreDigestMismatch,
    )?;
    Ok(Some(score_of(&record)))
}

/// Stamp the winner `SELECTED` and return its identity.
#[inline(never)]
fn select_winner(account: &AccountInfo) -> Outcome<Hash32> {
    let mut record = super::decode_candidate_boxed(&account.data.borrow())?;
    record.status = CANDIDATE_STATUS_SELECTED;
    let winner = record.candidate;
    record.encode(&mut borrow_account_mut(account)?)?;
    Ok(winner)
}

/// Borrow one account's data mutably, or refuse.
fn borrow_account_mut<'a, 'info>(
    account: &'a AccountInfo<'info>,
) -> Outcome<core::cell::RefMut<'a, &'info mut [u8]>> {
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
}
