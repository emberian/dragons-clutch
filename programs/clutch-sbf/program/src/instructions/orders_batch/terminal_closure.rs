//! TerminalClosure — the general clearing plane's lifecycle end (tags 60-67).
//!
//! The ratified terminal conventions (`docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md`,
//! adopted 2026-08-20 item 7) applied to the general plane: **economic close
//! strictly precedes rent close**, every rent close pays **exactly the
//! recorded principal to the exact recorded payer**, and **every other live
//! lamport burns at the frozen program-wide neutral sink** — the same
//! incinerator the ResolutionWork and Direct V3 planes froze.  The V3
//! precedent (`direct_selection_v3::common::close_funded_account`) is the
//! shape every close here follows: refuse before any byte moves, zero the
//! account, resize to nothing, reassign to the System program.
//!
//! ## The close DAG
//!
//! Terminality is `EPOCH_PHASE_CLEARED` or `EPOCH_PHASE_LAPSED`, stamped once
//! by `FinalizeSelection` (tag 57).  From there the admitted order is a
//! dependency DAG, and every edge below is enforced by a refusal in this
//! module — never by convention:
//!
//! ```text
//! FinalizeSelection (CLEARED | LAPSED)
//!   settle every entitled slice            (economic close: SettlePage)
//!   release every zero-fill / lapsed ACTIVE reservation   (tag 60, owner-signed)
//!   close consumed receipts                (tag 61: requires the receipt exhausted)
//!   close pages                            (tag 63: requires every live record's
//!                                           reservation RELEASED or CONSUMED —
//!                                           the executable proof that no
//!                                           entitlement, consumption, or
//!                                           release-rank proof remains pending)
//!   close reservation archives             (tag 62: requires its page absent,
//!                                           which is what keeps the page's
//!                                           reservation sweep provable)
//!   close the pot                          (tag 64: requires every page absent)
//!   close candidates                       (tag 65: non-selected at terminality;
//!                                           the SELECTED pair only after pot and
//!                                           pages are absent — its feed carries
//!                                           the fill proofs releases consume)
//!   close clearing checkpoints             (tag 66: a standing SELECTED record
//!                                           demands the pot present or every
//!                                           page absent — never out from under
//!                                           a pending entitlement freeze)
//!   close the epoch + window (the root)    (tag 67: pot absent, every page
//!                                           absent, every retained candidate
//!                                           closed or unclosable-by-design)
//! ```
//!
//! Two consequences of the DAG are deliberate and recorded rather than
//! papered over:
//!
//! * **An abandoned ACTIVE reservation keeps its page — and therefore the
//!   epoch root — alive at recorded rent cost.**  Release is owner-signed
//!   (value returns to an owner), so owner abandonment holds machinery open
//!   exactly as the ratified design's abandoned-claim rule holds a market
//!   open (§3(18)): "retirement guaranteed" is not a promise, and no sweep
//!   right is invented to make it one.  Completion of a cleared allocation
//!   (freeze, entitle, settle) is permissionless, so any keeper — including
//!   the owners themselves — can always drive a CLEARED epoch to closable.
//! * **Unenumerable stragglers close before the root or strand by their own
//!   payer's choice.**  Superseded records and never-sealed staged pairs are
//!   not in the window's registry, so the root close cannot enumerate them;
//!   their close routes require the epoch account, which the root close
//!   removes.  The registry members the window *does* record are checked.
//!
//! ## Who paid: the general funding ledger
//!
//! The general plane's account shapes predate the terminal-identity header
//! and record no payer, so their principal was unattributable — the exact
//! `RENT.ACCOUNT_REFUND_UNOWNED` shape the V1/V2 direct rows still carry.
//! The gap is closed **at creation**: every creating instruction of the
//! family (`InitEpoch`, `InitOrderPage`, `InitClearWork`, `SubmitCandidate`,
//! `FreezeEntitlement`, `EntitleSlice`) accepts one optional trailing
//! [`clutch_solana_layout::clearing::GeneralFundingLedgerV1`] sibling per
//! created group and writes it in the same transition that debits the payer:
//! the recorded payer is the actual funding wallet and the recorded principal
//! is the payer's exact outlay after prefund normalization, so a hostile
//! prefund of a predictable PDA stays in the donation floor and burns at
//! close instead of inflating anyone's refund (the artifact-prefund-windfall
//! rule).  An account created **without** the ledger keeps the unowned
//! blocker: its close refuses forever, no payer is ever guessed, and the
//! root close tolerates exactly that unclosable-by-design state.
//!
//! Reservations are the one family with no sibling: the placement actor is
//! the reservation's recorded `owner` and its rent payer in the same
//! transition, so the close pays the exact rent-exempt principal to the
//! stored owner and burns any surplus.  Residual, recorded honestly: a
//! third-party prefund of a predictable reservation PDA below the rent
//! minimum is invisible after the shortfall-style creation and rides to the
//! owner — a donation *to the value owner*, the self-defeating shape of
//! donation-griefing, retired for good by the versioned family's embedded
//! header (§9.5 successor work).
//!
//! ## What remains per epoch, by design
//!
//! After every close lands, the general epoch's residual on-chain footprint
//! is exactly the declared-permanent set: the sealed 64-byte batch-policy
//! artifact at `seeds::batch_policy_pda(epoch, digest)` (the general sibling
//! of the V3 policy-artifact permanent row, whose shrink successor is
//! recorded, not built) — plus owner state (Positions) and market
//! infrastructure, which are not epoch machinery.  The SVM terminal-closure
//! walk computes and asserts that residual to the lamport.
//!
//! ## Claim plane
//!
//! SBF-EXECUTED (bank), no promotion.  The reference adapter refuses every
//! tag of this family with `UnsupportedIntent`; the oracle is the layout
//! codec plus lamport conservation on a real bank.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_system_program, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_solana_layout::clearing::{
    self, canonical_general_book_id, CandidateFeedHeader, ClearWorkGrowStage,
    GeneralFundingLedgerV1, FUNDING_COVERS_CANDIDATE_PAIR, FUNDING_COVERS_CLEAR_WORK,
    FUNDING_COVERS_FINAL_POT, FUNDING_COVERS_PAGE, FUNDING_COVERS_RECEIPT,
    GENERAL_FUNDING_LEDGER_BYTES,
};
use clutch_solana_layout::reservation::{
    RESERVATION_ACCOUNT_BYTES, RESERVATION_STATE_ACTIVE, RESERVATION_STATE_CONSUMED,
    RESERVATION_STATE_RELEASED,
};
use clutch_solana_layout::{
    account_len, stream, EpochAccount, FinalPotAccount, Hash32, CANDIDATE_STATUS_SELECTED,
    EPOCH_PHASE_CLEARED, EPOCH_PHASE_LAPSED, MAX_OUTCOMES, RECEIPT_FLAG_BUY_CONSUMED,
    RECEIPT_FLAG_SELL_CONSUMED, RECEIPT_FLAG_SLICE_EXHAUSTED,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use solana_sdk_ids::incinerator;

/// The frozen program-wide neutral sink of the general plane's closes: the
/// canonical incinerator — the same identity `RESOLUTION_WORK_NEUTRAL_SINK_V1`
/// and `DIRECT_NEUTRAL_SINK_V3` froze, restated here so the general plane's
/// close family names its sink in its own vocabulary.
pub const GENERAL_NEUTRAL_SINK_V1: Pubkey = incinerator::ID;

/// Accounts in a LAPSED-epoch `ReleaseTerminalReservation`, exactly.
pub const RELEASE_LAPSED_ACCOUNT_COUNT: usize = 4;
/// Fixed accounts in a CLEARED-epoch release, before the page prefix.
pub const RELEASE_CLEARED_FIXED_ACCOUNT_COUNT: usize = 6;
/// Accounts in a `CloseGeneralReceipt`, exactly.
pub const CLOSE_RECEIPT_ACCOUNT_COUNT: usize = 5;
/// Accounts in a `CloseGeneralReservation`, exactly.
pub const CLOSE_RESERVATION_ACCOUNT_COUNT: usize = 6;
/// Fixed accounts in a `CloseGeneralPage`, before the live reservations.
pub const CLOSE_PAGE_FIXED_ACCOUNT_COUNT: usize = 5;
/// Fixed accounts in a `CloseGeneralPot`, before the page-absence slots.
pub const CLOSE_POT_FIXED_ACCOUNT_COUNT: usize = 5;
/// Fixed accounts in a `CloseGeneralCandidate`, before the selected tail.
pub const CLOSE_CANDIDATE_FIXED_ACCOUNT_COUNT: usize = 6;
/// Fixed accounts in a `CloseGeneralClearWork`, before the selected tail.
pub const CLOSE_CLEAR_WORK_FIXED_ACCOUNT_COUNT: usize = 6;
/// Fixed accounts in a `CloseGeneralEpoch`, before pages and the registry.
pub const CLOSE_EPOCH_FIXED_ACCOUNT_COUNT: usize = 6;

/* ------------------------------------------------------------------------ */
/* Shared authenticated loads                                                */
/* ------------------------------------------------------------------------ */

/// Decode and authenticate one **terminal** general epoch (read-only role).
#[inline(never)]
fn load_terminal_epoch(
    program_id: &Pubkey,
    account: &AccountInfo,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<Box<EpochAccount>> {
    accounts::validate_state_role_lengths(program_id, account, false, &[account_len::EPOCH])?;
    let epoch = super::decode_epoch_boxed(&account.data.borrow())?;
    require(
        epoch.market == *intent_market && epoch.epoch == *intent_epoch,
        ClutchError::MismatchedState,
    )?;
    require(
        epoch.phase == EPOCH_PHASE_CLEARED || epoch.phase == EPOCH_PHASE_LAPSED,
        ClutchError::NotActive,
    )?;
    // Only the general book family closes here; a direct epoch is a
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

/// The sink account of a closing transition: the exact incinerator, writable.
fn require_sink(account: &AccountInfo) -> Outcome<()> {
    require(
        *account.key == GENERAL_NEUTRAL_SINK_V1,
        ClutchError::MismatchedState,
    )?;
    require(account.is_writable, ClutchError::NotWritable)
}

/// Require one presented slot to be a **closed** (or never-created) account:
/// zero data, System-owned.  Lamports are deliberately not consulted — a
/// post-close donation to a dead address proves nothing about state.
fn require_absent(account: &AccountInfo) -> Outcome<()> {
    require(
        account.data_len() == 0 && *account.owner == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

/// Require every page slot of the epoch's frozen set to be closed, each at
/// its canonical address in index order.
fn require_pages_absent(
    program_id: &Pubkey,
    epoch: &EpochAccount,
    slots: &[AccountInfo],
) -> Outcome<()> {
    require(
        slots.len() == usize::from(epoch.page_count),
        ClutchError::AccountCount,
    )?;
    let epoch_bytes = epoch.epoch.bytes();
    for (index, slot) in slots.iter().enumerate() {
        let (address, _) = seeds::page_pda(program_id, &epoch_bytes, index as u16);
        require(*slot.key == address, ClutchError::WrongPda)?;
        require_absent(slot)?;
    }
    Ok(())
}

/// Decode and bind one funding ledger to the exact account it must cover.
#[inline(never)]
pub(in crate::instructions) fn read_funding_ledger(
    program_id: &Pubkey,
    account: &AccountInfo,
    target: &Pubkey,
    covered: u8,
) -> Outcome<GeneralFundingLedgerV1> {
    accounts::validate_state_role_lengths(
        program_id,
        account,
        true,
        &[GENERAL_FUNDING_LEDGER_BYTES],
    )?;
    let ledger = GeneralFundingLedgerV1::decode(&account.data.borrow())?;
    require(
        ledger.target == Hash32::from_bytes(target.to_bytes()) && ledger.covered == covered,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::general_funding_pda(program_id, target),
        Some(ledger.stored_bump),
    )?;
    Ok(ledger)
}

/* ------------------------------------------------------------------------ */
/* Exact lamport primitives                                                  */
/* ------------------------------------------------------------------------ */

/// Zero one account's lamports and return what it held.
fn drain(account: &AccountInfo) -> Outcome<u64> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let held = **lamports;
    **lamports = 0;
    Ok(held)
}

/// Credit one exact amount into an already-named account.
fn credit(account: &AccountInfo, amount: u64) -> Outcome<()> {
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_add(amount)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

/// Release one closed account's bytes and ownership.
fn release_bytes(account: &AccountInfo) -> Outcome<()> {
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    Ok(())
}

/// Close one ledgered group with the exact recorded split: the recorded
/// principal to the exact recorded payer, everything else to the sink, every
/// member zeroed, resized, and reassigned.  A live total below principal
/// plus the recorded donation floor refuses before any byte moves.
#[inline(never)]
pub(in crate::instructions) fn close_ledgered_group(
    targets: &[&AccountInfo],
    ledger_account: &AccountInfo,
    ledger: &GeneralFundingLedgerV1,
    recipient: &AccountInfo,
    sink: &AccountInfo,
) -> Outcome<()> {
    require(
        Hash32::from_bytes(recipient.key.to_bytes()) == ledger.payer,
        ClutchError::MismatchedState,
    )?;
    require(recipient.is_writable, ClutchError::NotWritable)?;
    require_sink(sink)?;
    let mut observed = ledger_account.lamports();
    for target in targets {
        require(target.is_writable, ClutchError::NotWritable)?;
        observed = observed
            .checked_add(target.lamports())
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    let owed = ledger.payer_principal_lamports;
    let neutral = observed
        .checked_sub(owed)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    require(
        neutral >= ledger.donation_floor_lamports,
        ClutchError::AggregateClosureMismatch,
    )?;
    for target in targets {
        drain(target)?;
        release_bytes(target)?;
    }
    drain(ledger_account)?;
    release_bytes(ledger_account)?;
    credit(recipient, owed)?;
    credit(sink, neutral)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Creation-side registration (called by the creating instructions)          */
/* ------------------------------------------------------------------------ */

/// The payer's exact outlay for one shortfall-style creation: the rent-exempt
/// minimum less whatever the target already held.
pub(in crate::instructions) fn creation_shortfall(minimum: u64, prior: u64) -> u64 {
    minimum.saturating_sub(prior)
}

/// Create and write one general funding ledger for a group this instruction
/// is creating, in the same transition that debits the payer.
///
/// `group_outlay` is the payer's exact lamport outlay across the covered
/// group (after prefund normalization), `group_floor` the pre-existing
/// lamports observed across the group before creation.  The ledger's own
/// shortfall-style creation folds into both totals here, so one recorded
/// pair covers the whole payout at close.
#[allow(clippy::too_many_arguments)] // one argument per creation coordinate
#[inline(never)]
pub(in crate::instructions) fn create_funding_ledger<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    ledger: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    target: &Pubkey,
    covered: u8,
    group_outlay: u64,
    group_floor: u64,
) -> Outcome<()> {
    require_system_program(system_program)?;
    require(
        *payer.key != GENERAL_NEUTRAL_SINK_V1,
        ClutchError::MismatchedState,
    )?;
    let (address, bump) = seeds::general_funding_pda(program_id, target);
    expect_pda(ledger.key, (address, bump), None)?;
    let self_prior = ledger.lamports();
    let self_minimum = rent.minimum_balance(GENERAL_FUNDING_LEDGER_BYTES)?;
    let value = GeneralFundingLedgerV1 {
        target: Hash32::from_bytes(target.to_bytes()),
        payer: Hash32::from_bytes(payer.key.to_bytes()),
        payer_principal_lamports: group_outlay
            .checked_add(creation_shortfall(self_minimum, self_prior))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        donation_floor_lamports: group_floor
            .checked_add(self_prior)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        covered,
        stored_bump: bump,
        flags: 0,
    };
    value.validate()?;
    let target_bytes = target.to_bytes();
    create_pda_account(
        program_id,
        payer,
        ledger,
        system_program,
        rent,
        GENERAL_FUNDING_LEDGER_BYTES,
        &[seeds::SEED_GENERAL_FUNDING, &target_bytes, &[bump]],
    )?;
    let mut data = ledger
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Tag 60 — ReleaseTerminalReservation                                       */
/* ------------------------------------------------------------------------ */

/// Verify one frozen page of the set walk-style and return its header.
#[inline(never)]
fn load_bound_frozen_page(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    expected_index: u16,
    writable: bool,
) -> Outcome<stream::OrderPageHeader> {
    accounts::validate_state_role_lengths(
        program_id,
        account,
        writable,
        &[account_len::ORDER_PAGE],
    )?;
    let data = account.data.borrow();
    let header = stream::verify_page(&data)?;
    require(
        header.page_index == expected_index
            && header.market == epoch.market
            && header.epoch == epoch.epoch
            && header.frozen == 1
            && header.order_set == epoch.order_set
            && header.page_count == epoch.page_count
            && header.set_order_count == epoch.order_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::page_pda(program_id, &epoch.epoch.bytes(), header.page_index),
        Some(header.stored_bump),
    )?;
    Ok(header)
}

/// Find one order's zero-based live rank across the page prefix `0..=k`.
///
/// The order must be found **live** on its own page; a retirement released at
/// cancellation and can never be ACTIVE here anyway — stated, not assumed.
#[inline(never)]
fn live_rank_of_order(
    program_id: &Pubkey,
    pages: &[AccountInfo],
    epoch: &EpochAccount,
    page_index: u16,
    order_id: Hash32,
) -> Outcome<u16> {
    let mut live: u16 = 0;
    for (index, page) in pages.iter().enumerate() {
        let header = load_bound_frozen_page(program_id, page, epoch, index as u16, false)?;
        let data = page.data.borrow();
        let mut cursor = stream::OrderSlotCursor::new(&data)?;
        let mut at = 0usize;
        while at < header.order_count as usize {
            let slot = match cursor.next_slot() {
                Some(step) => step?,
                None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
            };
            if slot.is_live() {
                if index as u16 == page_index && slot.order_id() == order_id {
                    return Ok(live);
                }
                live = live
                    .checked_add(1)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            }
            at += 1;
        }
    }
    Err(Refusal::Adapter(ClutchError::MismatchedState))
}

/// Verify the selected candidate's sealed feed and bind it to the plane.
#[inline(never)]
fn load_selected_feed(
    program_id: &Pubkey,
    record_account: &AccountInfo,
    feed_account: &AccountInfo,
    epoch: &EpochAccount,
) -> Outcome<CandidateFeedHeader> {
    accounts::validate_state_role_lengths(
        program_id,
        record_account,
        false,
        &[account_len::CANDIDATE],
    )?;
    let record = super::decode_candidate_boxed(&record_account.data.borrow())?;
    require(
        record.epoch == epoch.epoch
            && record.market == epoch.market
            && record.status == CANDIDATE_STATUS_SELECTED,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        record_account.key,
        seeds::candidate_pda(program_id, &epoch.epoch.bytes(), &record.candidate.bytes()),
        Some(record.stored_bump),
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        feed_account,
        false,
        &[account_len::CANDIDATE_FEED],
    )?;
    let feed = {
        let data = feed_account.data.borrow();
        clearing::verify_candidate_feed(&data)?
    };
    require(
        feed.candidate == record.candidate
            && feed.epoch == epoch.epoch
            && feed.market == epoch.market
            && feed.order_set == epoch.order_set
            && feed.outcome_count == epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        feed_account.key,
        seeds::candidate_feed_pda(program_id, &epoch.epoch.bytes(), &record.candidate.bytes()),
        Some(feed.stored_bump),
    )?;
    Ok(feed)
}

/// Release one still-ACTIVE reservation of a terminal epoch back into its
/// owner's Position — the owner-signed economic half of TerminalClosure.
///
/// On a LAPSED epoch every ACTIVE reservation is releasable.  On a CLEARED
/// epoch the selected candidate's verified feed must give the order a zero
/// fill: a filled order's envelope is spoken for by entitlement and
/// consumption, and releasing it would unmake the verified allocation.
#[inline(never)]
pub(super) fn release_terminal_reservation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<()> {
    require(
        accounts.len() >= RELEASE_LAPSED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[0])?;
    require_distinct(accounts)?;
    let epoch = load_terminal_epoch(program_id, &accounts[1], intent_market, intent_epoch)?;

    accounts::validate_state_role_lengths(
        program_id,
        &accounts[2],
        true,
        &[RESERVATION_ACCOUNT_BYTES],
    )?;
    let mut reservation = super::decode_reservation_boxed(&accounts[2].data.borrow())?;
    require(
        reservation.state == RESERVATION_STATE_ACTIVE
            && reservation.market == epoch.market
            && reservation.epoch == epoch.epoch,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[2].key,
        seeds::reservation_pda(program_id, &reservation.reservation.bytes()),
        Some(reservation.stored_bump),
    )?;
    let actor = Hash32::from_bytes(accounts[0].key.to_bytes());
    require(actor == reservation.owner, ClutchError::UnauthorizedActor)?;

    accounts::validate_state_role_lengths(
        program_id,
        &accounts[3],
        true,
        &[account_len::POSITION],
    )?;
    let mut position = super::decode_position_boxed(&accounts[3].data.borrow())?;
    require(
        position.owner == actor
            && position.market == epoch.market
            && position.generation == reservation.position_generation
            && position.close_state == 0,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[3].key,
        seeds::position_pda(
            program_id,
            &position.market.bytes(),
            &position.owner.bytes(),
        ),
        Some(position.stored_bump),
    )?;

    /* The release generation is the request envelope's sequence, exactly the
     * cancellation rule; the codec's `released` transition re-refuses any
     * value at or below the order generation, and the ACTIVE -> RELEASED
     * flip is what makes a replay refuse on state. */
    require(sequence > reservation.order_generation, ClutchError::Replay)?;

    if epoch.phase == EPOCH_PHASE_CLEARED {
        /* The zero-fill proof: the selected candidate's sealed feed, plus the
         * digest-verified page prefix that recovers this order's live rank. */
        let pages_needed = usize::from(reservation.page_index) + 1;
        require_count(accounts, RELEASE_CLEARED_FIXED_ACCOUNT_COUNT + pages_needed)?;
        let feed = load_selected_feed(program_id, &accounts[4], &accounts[5], &epoch)?;
        let rank = live_rank_of_order(
            program_id,
            &accounts[RELEASE_CLEARED_FIXED_ACCOUNT_COUNT
                ..RELEASE_CLEARED_FIXED_ACCOUNT_COUNT + pages_needed],
            &epoch,
            reservation.page_index,
            reservation.order_id,
        )?;
        require(rank <= u16::from(u8::MAX), ClutchError::MismatchedState)?;
        let fill = {
            let data = accounts[5].data.borrow();
            clearing::fill_at(&data, &feed, rank as u8)?
        };
        require(fill == 0, ClutchError::MismatchedState)?;
    } else {
        require_count(accounts, RELEASE_LAPSED_ACCOUNT_COUNT)?;
    }

    /* Apply: the reservation's own validated envelope returns to the same
     * compartments placement debited — reserved cash unwinds, Eggs return. */
    position.reserved_cash_atoms = position
        .reserved_cash_atoms
        .checked_sub(reservation.remaining_cash_atoms)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        position.internal[outcome] = position.internal[outcome]
            .checked_add(reservation.remaining_internal[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        outcome += 1;
    }
    *reservation = reservation.released(sequence)?;
    position.validate()?;
    {
        let mut data = borrow_account_mut(&accounts[3])?;
        position.encode(&mut data)?;
    }
    let mut data = borrow_account_mut(&accounts[2])?;
    reservation.encode(&mut data)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Tag 61 — CloseGeneralReceipt                                              */
/* ------------------------------------------------------------------------ */

/// Close one exhausted settlement receipt: recorded principal to the recorded
/// payer, surplus to the sink.  An unconsumed receipt refuses — economic
/// close precedes rent close.
#[inline(never)]
pub(super) fn close_general_receipt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
    slice_index: u16,
) -> Outcome<()> {
    require_count(accounts, CLOSE_RECEIPT_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    let epoch = load_terminal_epoch(program_id, &accounts[0], intent_market, intent_epoch)?;
    require(epoch.phase == EPOCH_PHASE_CLEARED, ClutchError::NotActive)?;

    accounts::validate_state_role_lengths(
        program_id,
        &accounts[1],
        true,
        &[account_len::SETTLEMENT_RECEIPT],
    )?;
    {
        let receipt = super::decode_receipt_boxed(&accounts[1].data.borrow())?;
        require(
            receipt.epoch == epoch.epoch
                && receipt.market == epoch.market
                && receipt.candidate == *intent_candidate
                && receipt.slice_index == slice_index,
            ClutchError::MismatchedState,
        )?;
        // Economic zero first: only a fully consumed entitlement closes.
        require(
            receipt.consumed_flags
                == RECEIPT_FLAG_BUY_CONSUMED
                    | RECEIPT_FLAG_SELL_CONSUMED
                    | RECEIPT_FLAG_SLICE_EXHAUSTED
                && receipt.settled_quantity == receipt.quantity,
            ClutchError::MismatchedState,
        )?;
        expect_pda(
            accounts[1].key,
            seeds::receipt_pda(
                program_id,
                &receipt.epoch.bytes(),
                &receipt.candidate.bytes(),
                receipt.slice_index,
            ),
            Some(receipt.stored_bump),
        )?;
    }
    let ledger = read_funding_ledger(
        program_id,
        &accounts[2],
        accounts[1].key,
        FUNDING_COVERS_RECEIPT,
    )?;
    close_ledgered_group(
        &[&accounts[1]],
        &accounts[2],
        &ledger,
        &accounts[3],
        &accounts[4],
    )
}

/* ------------------------------------------------------------------------ */
/* Tag 62 — CloseGeneralReservation                                          */
/* ------------------------------------------------------------------------ */

/// Close one RELEASED or CONSUMED reservation archive of a terminal epoch:
/// the exact rent-exempt principal to the stored owner (the placement actor
/// who funded it), surplus to the sink.  Requires the reservation's page
/// already closed so the page's reservation sweep stays provable.
#[inline(never)]
pub(super) fn close_general_reservation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<()> {
    require_count(accounts, CLOSE_RESERVATION_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    let epoch = load_terminal_epoch(program_id, &accounts[0], intent_market, intent_epoch)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[1],
        true,
        &[RESERVATION_ACCOUNT_BYTES],
    )?;
    let rent = read_rent(&accounts[5])?;
    let (owner, page_index) = {
        let reservation = super::decode_reservation_boxed(&accounts[1].data.borrow())?;
        require(
            reservation.market == epoch.market && reservation.epoch == epoch.epoch,
            ClutchError::MismatchedState,
        )?;
        // Economic zero first: an ACTIVE or ENTITLED envelope still owns
        // assets and refuses; both admitted states are remaining-zero by
        // their codec invariants.
        require(
            reservation.state == RESERVATION_STATE_RELEASED
                || reservation.state == RESERVATION_STATE_CONSUMED,
            ClutchError::MismatchedState,
        )?;
        expect_pda(
            accounts[1].key,
            seeds::reservation_pda(program_id, &reservation.reservation.bytes()),
            Some(reservation.stored_bump),
        )?;
        (reservation.owner, reservation.page_index)
    };
    // The page-first DAG edge: the reservation archive outlives its page.
    let (page_address, _) = seeds::page_pda(program_id, &epoch.epoch.bytes(), page_index);
    require(*accounts[2].key == page_address, ClutchError::WrongPda)?;
    require_absent(&accounts[2])?;

    require(
        Hash32::from_bytes(accounts[3].key.to_bytes()) == owner,
        ClutchError::MismatchedState,
    )?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_sink(&accounts[4])?;
    let owed = rent.minimum_balance(RESERVATION_ACCOUNT_BYTES)?;
    let observed = accounts[1].lamports();
    let neutral = observed
        .checked_sub(owed)
        .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?;
    drain(&accounts[1])?;
    release_bytes(&accounts[1])?;
    credit(&accounts[3], owed)?;
    credit(&accounts[4], neutral)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Tag 69 — ClosePosition, and the revenue plane's grief rider               */
/* ------------------------------------------------------------------------ */

/// Accounts in a `ClosePosition` instruction, exactly.
///
/// Owner (signer, writable), Market (read-only), the Position (writable), the
/// Realm's revenue-policy-record PDA (present **or provably absent**), and the
/// frozen neutral sink.
pub const CLOSE_POSITION_ACCOUNT_COUNT: usize = 5;

/// Map a liveness-kernel refusal onto the program's refusal plane.
fn treasury_fault(error: clutch_liveness::Error) -> Refusal {
    match error {
        clutch_liveness::Error::TreasuryServiceOutstanding => {
            Refusal::Adapter(ClutchError::TreasuryServiceOutstanding)
        }
        clutch_liveness::Error::ArithmeticOverflow => Refusal::Adapter(ClutchError::Arithmetic),
        _ => Refusal::Adapter(ClutchError::MismatchedState),
    }
}

/// The B4b mid-epoch-close grief rider, decided by the liveness kernel.
///
/// The rider (`ADOPTED_2026-08-20.md` item 8, stress point 1): a treasury
/// Position serving an unsettled fee-bearing epoch refuses close, because
/// closing it mid-epoch would let the fee recipient halt *other parties'*
/// settlement.  `clutch_liveness::TreasuryServiceLedger` is that rule as
/// fixed-memory bookkeeping, and this is where its transitions are wired:
///
/// * **begin_service** — one per fee-bearing epoch admitted against this
///   treasury.  A Realm's revenue-policy record naming this Position *is* the
///   standing election, so it counts as one begun service.
/// * **settle_service** — one per served epoch fully settled.  It has no
///   caller and cannot have one yet: fee-bearing admission refuses at
///   `RevenueTreasuryUnset`, so no fee-bearing epoch can open, let alone
///   settle.  The counter therefore cannot come back down and the refusal
///   below is the boundary itself, not a placeholder standing in for it.
/// * **close** — refuses while any service is outstanding, and lands once.
///
/// The record is presented at its canonical address and is either *absent* —
/// a record's absence IS the zero-take state (D4), so no treasury exists and
/// no service can be outstanding — or decoded and compared against this
/// Position's owner.
#[inline(never)]
fn require_no_treasury_service(
    program_id: &Pubkey,
    record_account: &AccountInfo,
    realm: Hash32,
    owner: Hash32,
) -> Outcome<()> {
    let (record_address, _) = seeds::revenue_policy_pda(program_id, &realm.bytes());
    require(*record_account.key == record_address, ClutchError::WrongPda)?;
    let serving = if record_account.data_len() == 0 && *record_account.owner == SYSTEM_PROGRAM_ID {
        false
    } else {
        accounts::validate_state_role_lengths(
            program_id,
            record_account,
            false,
            &[clutch_solana_layout::revenue::REVENUE_POLICY_RECORD_BYTES],
        )?;
        let record = clutch_solana_layout::revenue::RevenuePolicyRecordV1::decode(
            &record_account.data.borrow(),
        )?;
        require(record.realm == realm, ClutchError::MismatchedState)?;
        record.names_treasury(owner)
    };
    let ledger = clutch_liveness::TreasuryServiceLedger::admit(clutch_liveness::Id::from_bytes(
        owner.bytes(),
    ))
    .map_err(treasury_fault)?;
    let ledger = if serving {
        ledger.begin_service().map_err(treasury_fault)?
    } else {
        ledger
    };
    ledger.close().map_err(treasury_fault)?;
    Ok(())
}

/// Refuse closing one owner's Position until the account plane can prove that
/// no reservation still owns assets on its behalf.
///
/// A seller's admitted Eggs move out of its Position and into a separately
/// addressed ACTIVE reservation.  An all-in seller therefore has a locally
/// economic-zero Position while still owning live assets through that
/// reservation.  `ClosePosition` receives neither a canonical reservation
/// census nor a persisted outstanding-reservation counter, so local zero is
/// not an aggregate closure proof.  Until one of those proofs is part of the
/// state transition, every otherwise-valid close refuses after authentication
/// and before any lamport or byte mutation.
#[inline(never)]
pub(super) fn close_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_owner: &Hash32,
) -> Outcome<()> {
    require_count(accounts, CLOSE_POSITION_ACCOUNT_COUNT)?;
    /* The still-disabled transition has only its canonical initial sequence. */
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    // The signer *is* the owner identity: a Position is owner-seeded, so no
    // separate authority claim exists to be transposed.
    let owner = Hash32::from_bytes(accounts[0].key.to_bytes());
    require(owner == *intent_owner, ClutchError::UnauthorizedActor)?;

    accounts::validate_state_role_lengths(program_id, &accounts[1], false, &[account_len::MARKET])?;
    let realm = {
        let market = clutch_solana_layout::MarketAccount::decode(&accounts[1].data.borrow())?;
        require(
            market.market == *intent_market,
            ClutchError::MismatchedState,
        )?;
        expect_pda(
            accounts[1].key,
            seeds::market_pda(program_id, &market.realm.bytes(), &market.market.bytes()),
            Some(market.stored_bump),
        )?;
        market.realm
    };

    accounts::validate_state_role_lengths(
        program_id,
        &accounts[2],
        true,
        &[account_len::POSITION],
    )?;
    {
        let position = super::decode_position_boxed(&accounts[2].data.borrow())?;
        require(
            position.market == *intent_market && position.owner == owner,
            ClutchError::MismatchedState,
        )?;
        /* Economic zero first, as everywhere in this family: a Position
         * holding cash, encumbered cash, or any claim is not closable, and
         * the padding beyond the market's width is refused by the codec. */
        require(
            position.cash_atoms == 0
                && position.reserved_cash_atoms == 0
                && position.internal == [0; MAX_OUTCOMES],
            ClutchError::AggregateClosureMismatch,
        )?;
        require(position.close_state == 0, ClutchError::NotActive)?;
        expect_pda(
            accounts[2].key,
            seeds::position_pda(program_id, &intent_market.bytes(), &owner.bytes()),
            Some(position.stored_bump),
        )?;
    }

    require_no_treasury_service(program_id, &accounts[3], realm, owner)?;
    require_sink(&accounts[4])?;

    // Fail closed: local Position zero does not enumerate assets held by live
    // reservation PDAs.  Reusing AggregateClosureMismatch is deliberate: the
    // absent fact is the aggregate ownership closure proof, not authority or
    // local canonicality.  Nothing above mutates any account.
    Err(ClutchError::AggregateClosureMismatch.into())
}

/* ------------------------------------------------------------------------ */
/* Tag 63 — CloseGeneralPage                                                 */
/* ------------------------------------------------------------------------ */

/// Close one frozen page after proving every live record's reservation is no
/// longer ACTIVE or ENTITLED — the executable proof that no entitlement,
/// consumption, or release-rank proof still needs the page's bytes.
#[inline(never)]
pub(super) fn close_general_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    page_index: u16,
) -> Outcome<()> {
    require(
        accounts.len() >= CLOSE_PAGE_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    let epoch = load_terminal_epoch(program_id, &accounts[0], intent_market, intent_epoch)?;
    let header = load_bound_frozen_page(program_id, &accounts[1], &epoch, page_index, true)?;

    /* Every live record, in slot order, against the presented reservation in
     * the same order: bound by order id, owner, and order generation, with
     * the reservation's own digest-bound identity re-deriving its address.
     * ACTIVE and ENTITLED refuse — their envelopes still own value. */
    let reservations = &accounts[CLOSE_PAGE_FIXED_ACCOUNT_COUNT..];
    {
        let data = accounts[1].data.borrow();
        let mut cursor = stream::OrderSlotCursor::new(&data)?;
        let mut at = 0usize;
        let mut presented = 0usize;
        while at < header.order_count as usize {
            let slot = match cursor.next_slot() {
                Some(step) => step?,
                None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
            };
            if slot.is_live() {
                require(presented < reservations.len(), ClutchError::AccountCount)?;
                let account = &reservations[presented];
                accounts::validate_state_role_lengths(
                    program_id,
                    account,
                    false,
                    &[RESERVATION_ACCOUNT_BYTES],
                )?;
                let reservation = super::decode_reservation_boxed(&account.data.borrow())?;
                require(
                    reservation.market == epoch.market
                        && reservation.epoch == epoch.epoch
                        && reservation.order_id == slot.order_id()
                        && reservation.owner == slot.owner()
                        && reservation.order_generation == slot.generation()
                        && (reservation.state == RESERVATION_STATE_RELEASED
                            || reservation.state == RESERVATION_STATE_CONSUMED),
                    ClutchError::MismatchedState,
                )?;
                expect_pda(
                    account.key,
                    seeds::reservation_pda(program_id, &reservation.reservation.bytes()),
                    Some(reservation.stored_bump),
                )?;
                presented += 1;
            }
            at += 1;
        }
        require(presented == reservations.len(), ClutchError::AccountCount)?;
    }

    let ledger = read_funding_ledger(
        program_id,
        &accounts[2],
        accounts[1].key,
        FUNDING_COVERS_PAGE,
    )?;
    close_ledgered_group(
        &[&accounts[1]],
        &accounts[2],
        &ledger,
        &accounts[3],
        &accounts[4],
    )
}

/* ------------------------------------------------------------------------ */
/* Tag 64 — CloseGeneralPot                                                  */
/* ------------------------------------------------------------------------ */

/// Close the CLEARED epoch's provably empty final pot once every page is
/// absent — page closure is the executable proof that no entitlement or
/// consumption remains pending, so removing the freeze's replay anchor can
/// strand nothing.
#[inline(never)]
pub(super) fn close_general_pot(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<()> {
    require(
        accounts.len() >= CLOSE_POT_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    let epoch = load_terminal_epoch(program_id, &accounts[0], intent_market, intent_epoch)?;
    require(epoch.phase == EPOCH_PHASE_CLEARED, ClutchError::NotActive)?;

    accounts::validate_state_role_lengths(
        program_id,
        &accounts[1],
        true,
        &[account_len::FINAL_POT],
    )?;
    {
        let pot = FinalPotAccount::decode(&accounts[1].data.borrow())?;
        require(
            pot.epoch == epoch.epoch && pot.market == epoch.market,
            ClutchError::MismatchedState,
        )?;
        // Economic zero, stated even though the CLOSED-phase codec enforces
        // it: a pot holding value never closes as surplus.
        require(
            pot.pot_internal == [0; MAX_OUTCOMES]
                && pot.pot_cash_price_units == 0
                && pot.rounding_pot_price_units == 0,
            ClutchError::AggregateClosureMismatch,
        )?;
        expect_pda(
            accounts[1].key,
            seeds::pot_pda(program_id, &epoch.epoch.bytes()),
            Some(pot.stored_bump),
        )?;
    }
    require_pages_absent(
        program_id,
        &epoch,
        &accounts[CLOSE_POT_FIXED_ACCOUNT_COUNT..],
    )?;
    let ledger = read_funding_ledger(
        program_id,
        &accounts[2],
        accounts[1].key,
        FUNDING_COVERS_FINAL_POT,
    )?;
    close_ledgered_group(
        &[&accounts[1]],
        &accounts[2],
        &ledger,
        &accounts[3],
        &accounts[4],
    )
}

/* ------------------------------------------------------------------------ */
/* Tag 65 — CloseGeneralCandidate                                            */
/* ------------------------------------------------------------------------ */

/// Close one candidate record and its feed together: one recorded payer
/// funded both in one `SubmitCandidate`, so one ledger pays one payout.
///
/// Current displacement preserves the displaced feed, so current candidate
/// pairs normally close together here. The absent-feed form remains accepted
/// for historical account shapes whose displacement moved the feed principal
/// onto the record. The SELECTED candidate's pair closes only after the pot
/// and every page are absent: its feed carries the fill proofs post-clear
/// releases consume, and its record is the consumption seam's selection
/// authority.
#[inline(never)]
pub(super) fn close_general_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
) -> Outcome<()> {
    require(
        accounts.len() >= CLOSE_CANDIDATE_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    let epoch = load_terminal_epoch(program_id, &accounts[0], intent_market, intent_epoch)?;

    accounts::validate_state_role_lengths(
        program_id,
        &accounts[1],
        true,
        &[account_len::CANDIDATE],
    )?;
    let status = {
        let record = super::decode_candidate_boxed(&accounts[1].data.borrow())?;
        require(
            record.epoch == epoch.epoch
                && record.market == epoch.market
                && record.candidate == *intent_candidate,
            ClutchError::MismatchedState,
        )?;
        expect_pda(
            accounts[1].key,
            seeds::candidate_pda(program_id, &epoch.epoch.bytes(), &record.candidate.bytes()),
            Some(record.stored_bump),
        )?;
        record.status
    };

    /* The feed slot: the canonical feed PDA, normally still program-owned at
     * the feed length (sealed, verified, displaced, and selected all close
     * here). The absent legacy form covers the historical displacement shape
     * whose lamports were parked on the surviving record. */
    let (feed_address, _) =
        seeds::candidate_feed_pda(program_id, &epoch.epoch.bytes(), &intent_candidate.bytes());
    require(*accounts[2].key == feed_address, ClutchError::WrongPda)?;
    let feed_present = *accounts[2].owner == *program_id;
    if feed_present {
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[2],
            true,
            &[account_len::CANDIDATE_FEED],
        )?;
    } else {
        require_absent(&accounts[2])?;
    }

    if status == CANDIDATE_STATUS_SELECTED {
        // The selected pair closes last among candidates: pot absent, every
        // page absent — nothing can still need the feed's fills or the
        // record's selection authority.
        let tail = &accounts[CLOSE_CANDIDATE_FIXED_ACCOUNT_COUNT..];
        require(!tail.is_empty(), ClutchError::AccountCount)?;
        let (pot_address, _) = seeds::pot_pda(program_id, &epoch.epoch.bytes());
        require(*tail[0].key == pot_address, ClutchError::WrongPda)?;
        require_absent(&tail[0])?;
        require_pages_absent(program_id, &epoch, &tail[1..])?;
    } else {
        require_count(accounts, CLOSE_CANDIDATE_FIXED_ACCOUNT_COUNT)?;
    }

    let ledger = read_funding_ledger(
        program_id,
        &accounts[3],
        accounts[1].key,
        FUNDING_COVERS_CANDIDATE_PAIR,
    )?;
    if feed_present {
        close_ledgered_group(
            &[&accounts[1], &accounts[2]],
            &accounts[3],
            &ledger,
            &accounts[4],
            &accounts[5],
        )
    } else {
        close_ledgered_group(
            &[&accounts[1]],
            &accounts[3],
            &ledger,
            &accounts[4],
            &accounts[5],
        )
    }
}

/* ------------------------------------------------------------------------ */
/* Tag 66 — CloseGeneralClearWork                                            */
/* ------------------------------------------------------------------------ */

/// Close one clearing checkpoint — complete, refused, or half-grown — of a
/// terminal epoch.
///
/// While the SELECTED candidate's record still stands, its checkpoint closes
/// only once the pot exists (the entitlement freeze already consumed the
/// verdict) or every page is absent (nothing can still freeze) — never out
/// from under a pending freeze.  A closed record settled that question at
/// its own close, and a non-selected candidate's checkpoint is dead weight
/// from terminality on.
#[inline(never)]
pub(super) fn close_general_clear_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
) -> Outcome<()> {
    require(
        accounts.len() >= CLOSE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    let epoch = load_terminal_epoch(program_id, &accounts[0], intent_market, intent_epoch)?;

    /* The checkpoint, at either of its two honest lengths: the finished
     * account authenticates through its layout header, a half-grown one
     * through its grow-stage prefix.  Both bind the exact triple and PDA. */
    let work = &accounts[1];
    require(work.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!work.executable, ClutchError::ExecutableAccount)?;
    require(work.is_writable, ClutchError::NotWritable)?;
    if work.data_len() == account_len::CLEAR_WORK {
        let header = {
            let data = work.data.borrow();
            clearing::verify_clear_work(&data)?
        };
        require(
            header.market == epoch.market
                && header.epoch == epoch.epoch
                && header.candidate == *intent_candidate,
            ClutchError::MismatchedState,
        )?;
        expect_pda(
            work.key,
            seeds::clear_work_pda(program_id, &epoch.epoch.bytes(), &intent_candidate.bytes()),
            Some(header.stored_bump),
        )?;
    } else {
        let stage = {
            let data = work.data.borrow();
            ClearWorkGrowStage::decode(&data)?
        };
        require(
            stage.market == epoch.market
                && stage.epoch == epoch.epoch
                && stage.candidate == *intent_candidate,
            ClutchError::MismatchedState,
        )?;
        expect_pda(
            work.key,
            seeds::clear_work_pda(program_id, &epoch.epoch.bytes(), &intent_candidate.bytes()),
            Some(stage.stored_bump),
        )?;
    }

    /* The record slot: the canonical record PDA, absent (its own close
     * already discharged the selected gating) or present. */
    let (record_address, _) =
        seeds::candidate_pda(program_id, &epoch.epoch.bytes(), &intent_candidate.bytes());
    require(*accounts[2].key == record_address, ClutchError::WrongPda)?;
    let tail = &accounts[CLOSE_CLEAR_WORK_FIXED_ACCOUNT_COUNT..];
    let selected = if *accounts[2].owner == *program_id {
        accounts::validate_state_role_lengths(
            program_id,
            &accounts[2],
            false,
            &[account_len::CANDIDATE],
        )?;
        let record = super::decode_candidate_boxed(&accounts[2].data.borrow())?;
        require(
            record.candidate == *intent_candidate
                && record.epoch == epoch.epoch
                && record.market == epoch.market,
            ClutchError::MismatchedState,
        )?;
        record.status == CANDIDATE_STATUS_SELECTED
    } else {
        require_absent(&accounts[2])?;
        false
    };
    if selected {
        let (pot_address, _) = seeds::pot_pda(program_id, &epoch.epoch.bytes());
        if tail.len() == 1 && *tail[0].key == pot_address && *tail[0].owner == *program_id {
            // The freeze already ran: the pot exists and binds this epoch and
            // candidate, so nothing still needs the checkpoint's verdict.
            accounts::validate_state_role_lengths(
                program_id,
                &tail[0],
                false,
                &[account_len::FINAL_POT],
            )?;
            let pot = FinalPotAccount::decode(&tail[0].data.borrow())?;
            require(
                pot.epoch == epoch.epoch
                    && pot.market == epoch.market
                    && pot.candidate == *intent_candidate,
                ClutchError::MismatchedState,
            )?;
        } else {
            // No pot proof: every page must be absent, so no freeze remains
            // reachable and the checkpoint strands nothing.
            require_pages_absent(program_id, &epoch, tail)?;
        }
    } else {
        require_count(accounts, CLOSE_CLEAR_WORK_FIXED_ACCOUNT_COUNT)?;
    }

    let ledger = read_funding_ledger(
        program_id,
        &accounts[3],
        work.key,
        FUNDING_COVERS_CLEAR_WORK,
    )?;
    close_ledgered_group(&[work], &accounts[3], &ledger, &accounts[4], &accounts[5])
}

/* ------------------------------------------------------------------------ */
/* Tag 67 — CloseGeneralEpoch                                                */
/* ------------------------------------------------------------------------ */

/// Refuse closure of the terminal epoch/window root until its replacement
/// protocol persists an exhaustive candidate count and a versioned tombstone.
///
/// `EpochWindowAccount::retained` enumerates only verified candidates. A
/// sealed-but-unverified candidate can therefore survive outside that registry;
/// deleting the epoch and window would erase the only durable occupation of
/// their canonical PDAs and let `InitEpoch` replay the same epoch index around
/// the residual candidate. Dependency closes remain available, but the root
/// must stay occupied until a format/version migration can prove exhaustive
/// closure and leave a replay-resistant tombstone.
#[inline(never)]
pub(super) fn close_general_epoch(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _sequence: u64,
    _intent_market: &Hash32,
    _intent_epoch: &Hash32,
) -> Outcome<()> {
    Err(ClutchError::NotYetImplemented.into())
}

/// Borrow one account's data mutably, or refuse.
fn borrow_account_mut<'a, 'info>(
    account: &'a AccountInfo<'info>,
) -> Outcome<core::cell::RefMut<'a, &'info mut [u8]>> {
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions::direct_selection_v3::DIRECT_NEUTRAL_SINK_V3;
    use crate::instructions::resolution_work::RESOLUTION_WORK_NEUTRAL_SINK_V1;

    #[test]
    fn one_frozen_sink_across_all_three_planes() {
        // The general plane's sink is the same frozen incinerator the
        // ResolutionWork and Direct V3 planes already burned into the ELF.
        assert_eq!(GENERAL_NEUTRAL_SINK_V1, DIRECT_NEUTRAL_SINK_V3);
        assert_eq!(GENERAL_NEUTRAL_SINK_V1, RESOLUTION_WORK_NEUTRAL_SINK_V1);
    }

    #[test]
    fn creation_shortfall_is_the_prefund_normalized_outlay() {
        // No prefund: the payer's outlay is the whole minimum.
        assert_eq!(creation_shortfall(1_000, 0), 1_000);
        // A partial prefund discounts the outlay exactly.
        assert_eq!(creation_shortfall(1_000, 400), 600);
        // A prefund at or past the minimum leaves a zero outlay — the whole
        // balance is donation floor, and the close burns it.
        assert_eq!(creation_shortfall(1_000, 1_000), 0);
        assert_eq!(creation_shortfall(1_000, 5_000), 0);
    }
}
