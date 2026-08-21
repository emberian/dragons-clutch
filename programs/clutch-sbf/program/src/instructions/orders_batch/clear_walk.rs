//! The on-chain streaming walk — Tier 2 joins 1 and 4 (owner half), T2-6b.
//!
//! `Intent::AdvanceClearWork` (tag 51): the instruction that joins everything
//! Wave 1 built.  Per invocation it decodes the checkpoint's layout header
//! ([`clearing::verify_clear_work`]), boxes the 47,846-byte `ClearWorkV1`
//! onto the transaction-requested heap frame (the program crate is outside
//! the no_alloc boundary and boxes deliberately; the codec's entry points are
//! frame-measured at 64/1,280 bytes), decodes the body in place, and then —
//! for up to `max_orders` live orders from the monotone
//! `(page_cursor, slot_cursor)` position — walks the one digest-verified page
//! the cursor names: tombstones are skipped without consuming a rank,
//! [`OwnerInterner::intern`] mints or reproduces each live owner's tag,
//! [`projection::project_slot`] maps the record, the candidate fill arrives
//! by live rank from the bound feed ([`clearing::fill_at`]), and
//! `push_order` folds the pair.  The updated body is re-encoded and the
//! layout cursor advances.
//!
//! ## The reservation sweep (join 1)
//!
//! On pass 1 — and exactly there — every pushed order presents its canonical
//! [`clutch_solana_layout::reservation::ReservationAccount`]: the PDA is
//! re-derived through [`canonical_reservation_id`] from this walk's own facts
//! (only the position
//! generation is read off the account, whose decoder already binds it into
//! the stored identity), the state must be `RESERVATION_STATE_ACTIVE`, the
//! stored envelope must equal [`ReservationPlan::for_order`] re-derived from
//! the projected record at zero fee, untouched.  Pass-1 completion therefore
//! *is* the sweep: it proves every live order of the frozen set holds exactly
//! one ACTIVE, exactly-funded reservation at bind time — and nothing releases
//! an ACTIVE reservation of a FROZEN epoch, because cancellation requires
//! OPEN.
//!
//! ## Anchoring (design §10, the three layers)
//!
//! * body edits that change consumed-fold state → the codec's own seal
//!   refuses the pass (`FeedErrorV1::ResumeFoldMismatch`, surfaced as
//!   [`ClutchError::ResumeFoldMismatch`]; the failing transaction rolls back,
//!   so the poison is never persisted);
//! * header edits → [`clearing::ClearWorkHeader::validate`] and
//!   [`clearing::require_continuation`] against the epoch's frozen
//!   `order_set`;
//! * wholesale body substitution with another internally consistent
//!   checkpoint → the anchor comparison this module runs at **every** bound
//!   resume: `body.consumed_fold() == header.consumed_fold`, the check that
//!   catches the codec's 29 documented residual tamper regions.
//!
//! Pass-1 completion runs `end_pass`, refuses unless the interned
//! distinct-owner count equals the epoch's frozen `owner_count`, and stamps
//! [`clearing::bind_order_set`]`(epoch.order_set, body.consumed_fold())`.
//!
//! ## Begin
//!
//! The first advance on an OPEN checkpoint performs `begin` with the T2-5
//! zero-sentinel domain — the four u64 identity tags zero, everything
//! economic at full fidelity from the frozen epoch, the policy the pinned
//! [`GENERAL_CLEARING_POLICY_V1`] whose recomputed `batch_policy_digest` must
//! equal `epoch.policy` — and a `StreamCandidateV1` built from the bound
//! [`clearing::CandidateFeedHeader`] with explicit zero claims
//! (`strict_claims: false`; the claimed u128 digest is never trusted).
//!
//! ## Replay
//!
//! Deliberately none beyond the state machine: the walk is permissionless
//! keeper work whose every authority is account state.  A replayed advance
//! either performs the next legitimate batch (which anyone could) or refuses
//! against the cursor/page/reservation shape it presents; the envelope
//! sequence is pinned to zero like the other keeper transitions.
//!
//! ## Claim plane
//!
//! SBF-EXECUTED (bank), explicitly PROFILE-ADMITTED: no.  The reference
//! adapter refuses the intent with `UnsupportedIntent`; the oracle is the
//! layout codec plus the host relation, byte for byte and verdict for
//! verdict.

use crate::accounts::{self, expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_batch::relation_v1::{RelationDomainV1, ScoreV1};
use clutch_batch::relation_v1_stream::{
    ClearWorkV1, CodecFaultV1, FeedErrorV1, FeedStatusV1, StreamCandidateV1,
};
use clutch_batch_policy_identity::{
    batch_policy_digest, general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
};
use clutch_solana_layout::clearing::{self, canonical_general_book_id, CandidateFeedHeader};
use clutch_solana_layout::projection::{self, OwnerInterner};
use clutch_solana_layout::reservation::{
    canonical_reservation_id, ReservationPlan, RESERVATION_ACCOUNT_BYTES,
    RESERVATION_STATE_ACTIVE,
};
use clutch_solana_layout::{
    account_len, stream, EpochAccount, Hash32, OrderSlot, EPOCH_PHASE_FROZEN,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Fixed accounts in an `AdvanceClearWork` instruction, before the pass-1
/// reservation list.
pub const ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT: usize = 4;
/// The frozen epoch (read-only, program-owned).
pub const IX_ADVANCE_EPOCH: usize = 0;
/// The bound candidate feed (read-only, program-owned).
pub const IX_ADVANCE_FEED: usize = 1;
/// The resumable checkpoint (writable, program-owned).
pub const IX_ADVANCE_WORK: usize = 2;
/// The one page the cursor sits on (read-only, program-owned).
pub const IX_ADVANCE_PAGE: usize = 3;
/// First pass-1 reservation; one per live order this batch pushes, in walk
/// order.  Later passes take none.
pub const IX_ADVANCE_RESERVATIONS: usize = ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT;

/// Collapse a checkpoint-codec fault onto its stable numeric code.
fn codec_fault(_: CodecFaultV1) -> Refusal {
    Refusal::Adapter(ClutchError::CheckpointCodecFault)
}

/// Map a feed-protocol fault, keeping the resumption mismatch distinguishable.
fn feed_fault(error: FeedErrorV1) -> Refusal {
    Refusal::Adapter(match error {
        FeedErrorV1::ResumeFoldMismatch => ClutchError::ResumeFoldMismatch,
        _ => ClutchError::FeedProtocolFault,
    })
}

/// The T2-5 domain construction, verbatim: four u64 identity tags as zero
/// sentinels, everything economic carried at full fidelity from the frozen
/// epoch.  Sound because the feed runs `strict_claims: false`, so the u64
/// tags feed only the legacy digest that is never compared; authoritative
/// identity binding is full-width, in the accounts themselves.
fn zero_sentinel_domain(epoch: &EpochAccount) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: epoch.relation_version,
        market_id: 0,
        book_id: 0,
        epoch: epoch.epoch_index,
        policy_id: 0,
        order_set_id: 0,
        outcome_count: epoch.outcome_count,
        owner_count: epoch.owner_count,
        price_scale: epoch.price_scale,
        remainder_seed: epoch.remainder_seed,
        policy: GENERAL_CLEARING_POLICY_V1,
    }
}

/// The stream candidate the feed header carries, with explicit zero claims.
///
/// Exactly the shape the T2-5 gate drives: `strict_claims: false` means the
/// claimed `ScoreV1` and the u128 digest are never consulted, so they travel
/// as zeros rather than as copies of numbers nothing may trust.
fn stream_candidate_of(feed: &CandidateFeedHeader) -> StreamCandidateV1 {
    StreamCandidateV1 {
        order_len: feed.order_len,
        prices: feed.prices,
        virtual_split: feed.virtual_split,
        virtual_merge: feed.virtual_merge,
        honored_aon_mask: feed.honored_aon_mask,
        claimed_score: ScoreV1::ZERO,
        canonical_candidate_digest: 0,
        declared_slices: feed.declared_slices(),
    }
}

/// Everything the walk decides before it touches the checkpoint body.
pub(super) struct WalkFrame {
    /// The frozen epoch, boxed off the frame.
    pub epoch: Box<EpochAccount>,
    /// The checkpoint's layout header.
    pub header: clearing::ClearWorkHeader,
    /// The verified feed header.
    pub feed: CandidateFeedHeader,
}

/// Authenticate the fixed accounts every clearing instruction shares:
/// epoch, feed, and checkpoint — identities, PDAs, phase, and (for a bound
/// checkpoint) the frozen-set continuation.
#[inline(never)]
pub(super) fn load_clearing_plane(
    program_id: &Pubkey,
    epoch_account: &AccountInfo,
    feed_account: &AccountInfo,
    work_account: &AccountInfo,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
) -> Outcome<WalkFrame> {
    accounts::validate_state_role_lengths(program_id, epoch_account, false, &[account_len::EPOCH])?;
    accounts::validate_state_role_lengths(
        program_id,
        feed_account,
        false,
        &[account_len::CANDIDATE_FEED],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        work_account,
        true,
        &[account_len::CLEAR_WORK],
    )?;

    let epoch = super::decode_epoch_boxed(&epoch_account.data.borrow())?;
    require(
        epoch.market == *intent_market && epoch.epoch == *intent_epoch,
        ClutchError::MismatchedState,
    )?;
    require(epoch.phase == EPOCH_PHASE_FROZEN, ClutchError::NotActive)?;
    // Only the general book family clears here; a direct epoch is a
    // different account length, and a general epoch's book identity is a
    // total function of its epoch identity.
    require(
        epoch.book == canonical_general_book_id(epoch.epoch),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        epoch_account.key,
        seeds::epoch_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;

    let header = {
        let data = work_account.data.borrow();
        clearing::verify_clear_work(&data)?
    };
    require(
        header.market == epoch.market
            && header.epoch == epoch.epoch
            && header.candidate == *intent_candidate,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        work_account.key,
        seeds::clear_work_pda(program_id, &epoch.epoch.bytes(), &intent_candidate.bytes()),
        Some(header.stored_bump),
    )?;
    require(
        header.status != clearing::CLEAR_WORK_STATUS_COMPLETE,
        ClutchError::MismatchedState,
    )?;
    // A bound checkpoint continues only on the exact frozen set it bound.
    if header.status == clearing::CLEAR_WORK_STATUS_BOUND {
        clearing::require_continuation(&header, epoch.order_set)?;
    }

    let feed = {
        let data = feed_account.data.borrow();
        clearing::verify_candidate_feed(&data)?
    };
    // The feed binding matrix (the settlement preflight's template): one
    // candidate, one epoch, one market, one frozen set, one width.
    require(
        feed.candidate == *intent_candidate
            && feed.epoch == epoch.epoch
            && feed.market == epoch.market
            && feed.order_set == epoch.order_set
            && feed.outcome_count == epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        feed_account.key,
        seeds::candidate_feed_pda(program_id, &epoch.epoch.bytes(), &intent_candidate.bytes()),
        Some(feed.stored_bump),
    )?;

    Ok(WalkFrame {
        epoch,
        header,
        feed,
    })
}

/// Decode the checkpoint body onto the heap, refusing a poisoned feed.
#[inline(never)]
pub(super) fn decode_body_boxed(work_account: &AccountInfo) -> Outcome<Box<ClearWorkV1>> {
    let mut body = boxed_idle_checkpoint()?;
    {
        let data = work_account.data.borrow();
        body.decode_into(clearing::clear_work_body(&data)?)
            .map_err(codec_fault)?;
    }
    require(!body.is_poisoned(), ClutchError::FeedProtocolFault)?;
    Ok(body)
}

/// The idle checkpoint, built on the heap from static storage — no frame
/// ever holds the ~48 KiB value.  Requires the transaction to have requested
/// a heap frame (`ComputeBudgetInstruction::request_heap_frame`); without
/// one, the write behind this allocation aborts the transaction at the
/// mapping boundary.
#[inline(never)]
fn boxed_idle_checkpoint() -> Outcome<Box<ClearWorkV1>> {
    static IDLE: ClearWorkV1 = ClearWorkV1::NEW;
    let layout = core::alloc::Layout::new::<ClearWorkV1>();
    // SAFETY: `ClearWorkV1` is plain data — integers, fixed arrays, and
    // field-less enums; no heap pointers, no `Drop` — so a byte copy of the
    // valid static idle value is a valid value (it is `Clone` but
    // deliberately not `Copy`, purely so a 48-KiB value is never duplicated
    // by an accidental move-out).  The pointer is freshly allocated for
    // exactly its layout.
    unsafe {
        let pointer = std::alloc::alloc(layout) as *mut ClearWorkV1;
        if pointer.is_null() {
            return Err(Refusal::Adapter(ClutchError::AccountCreationFailed));
        }
        core::ptr::copy_nonoverlapping(&IDLE as *const ClearWorkV1, pointer, 1);
        Ok(Box::from_raw(pointer))
    }
}

/// Read the persisted owner-interning table onto the heap.
#[inline(never)]
fn read_interner_boxed(work_account: &AccountInfo) -> Outcome<Box<OwnerInterner>> {
    static EMPTY: OwnerInterner = OwnerInterner::NEW;
    let mut owners = super::boxed_copy_of(&EMPTY)?;
    let data = work_account.data.borrow();
    clearing::read_owner_interner_into(&data, &mut owners)?;
    Ok(owners)
}

/// Verify one pushed order's canonical reservation: join 1's per-order step.
///
/// `pub(super)` because the entitlement freeze (T2-8) re-runs exactly this
/// validation — with a writable role, since it then flips the reservation
/// `ACTIVE → ENTITLED` — while the walk itself presents read-only roles.
#[inline(never)]
pub(super) fn validate_walk_reservation(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    page_index: u16,
    slot: &OrderSlot,
    writable: bool,
) -> Outcome<()> {
    accounts::validate_state_role_lengths(
        program_id,
        account,
        writable,
        &[RESERVATION_ACCOUNT_BYTES],
    )?;
    let reservation = super::decode_reservation_boxed(&account.data.borrow())?;
    // The envelope, re-derived from the projected record at the walk's own
    // frozen coordinates and zero fee, must match the stored one exactly and
    // untouched.
    let plan = ReservationPlan::for_order(slot, epoch.outcome_count, epoch.price_scale, 0)?;
    require(
        reservation.state == RESERVATION_STATE_ACTIVE
            && reservation.market == epoch.market
            && reservation.epoch == epoch.epoch
            && reservation.owner == slot.owner()
            && reservation.order_id == slot.order_id()
            && reservation.order_generation == slot.generation()
            && reservation.page_index == page_index
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
    )?;
    // The address is re-derived through the canonical identity from this
    // walk's own facts; only the position generation is the account's word,
    // and its decoder already bound that into the stored identity.
    let reservation_id = canonical_reservation_id(
        epoch.market,
        epoch.epoch,
        slot.owner(),
        reservation.position_generation,
        slot.order_id(),
    );
    expect_pda(
        account.key,
        seeds::reservation_pda(program_id, &reservation_id.bytes()),
        Some(reservation.stored_bump),
    )
}

/// What one batch of the walk decided.
struct BatchOutcome {
    /// Slot index one past the last slot this batch consumed.
    next_slot: usize,
    /// Live orders visited this pass, after the batch.
    live: u16,
    /// The feed refused an order at admission and reached its verdict early.
    completed_early: bool,
    /// Reservations consumed off the instruction's account list.
    reservations_consumed: usize,
}

/// Advance one clearing checkpoint's order pass by up to `max_orders` live
/// orders.
#[inline(never)]
pub(super) fn advance_clear_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
    max_orders: u16,
) -> Outcome<()> {
    require(
        accounts.len() >= ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_ADVANCE_PAGE],
        false,
        &[account_len::ORDER_PAGE],
    )?;

    let frame = load_clearing_plane(
        program_id,
        &accounts[IX_ADVANCE_EPOCH],
        &accounts[IX_ADVANCE_FEED],
        &accounts[IX_ADVANCE_WORK],
        intent_market,
        intent_epoch,
        intent_candidate,
    )?;
    let epoch = &frame.epoch;
    let header = frame.header;
    require(
        header.page_cursor < epoch.page_count,
        ClutchError::MismatchedState,
    )?;

    // The one page the cursor names, digest-verified and bound to the set.
    let page_data = accounts[IX_ADVANCE_PAGE].data.borrow();
    let page_header = stream::verify_page(&page_data)?;
    require(
        page_header.page_index == header.page_cursor
            && page_header.market == epoch.market
            && page_header.epoch == epoch.epoch
            && page_header.frozen == 1
            && page_header.order_set == epoch.order_set
            && page_header.page_count == epoch.page_count
            && page_header.set_order_count == epoch.order_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_ADVANCE_PAGE].key,
        seeds::page_pda(program_id, &epoch.epoch.bytes(), page_header.page_index),
        Some(page_header.stored_bump),
    )?;

    let mut body = decode_body_boxed(&accounts[IX_ADVANCE_WORK])?;
    let mut owners = read_interner_boxed(&accounts[IX_ADVANCE_WORK])?;

    // Begin, on the first advance of an OPEN checkpoint.
    let status = if body.is_idle() {
        require(
            header.status == clearing::CLEAR_WORK_STATUS_OPEN
                && header.page_cursor == 0
                && header.slot_cursor == 0
                && header.live_rank == 0
                && owners.count() == 0,
            ClutchError::MismatchedState,
        )?;
        // The pinned policy is the one the epoch froze: the digest is
        // recomputed here, so the walk does not inherit init's honesty.
        let policy_digest = batch_policy_digest(&GENERAL_CLEARING_POLICY_V1)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            epoch.policy.bytes() == policy_digest.0,
            ClutchError::AuthorizationUnavailable,
        )?;
        body.begin(
            &zero_sentinel_domain(epoch),
            &stream_candidate_of(&frame.feed),
            false,
        )
        .map_err(feed_fault)?
    } else {
        body.status()
    };

    let pass = match status {
        FeedStatusV1::NeedOrders { pass } => pass,
        FeedStatusV1::Complete => {
            // Either `begin` latched a V0-complete refusal just now (bind and
            // stop; the close persists the verdict), or the walk is simply
            // over and this instruction has nothing to advance.
            return if header.status == clearing::CLEAR_WORK_STATUS_OPEN
                && header.page_cursor == 0
                && header.slot_cursor == 0
            {
                require(
                    accounts.len() == ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
                    ClutchError::AccountCount,
                )?;
                drop(page_data);
                let mut work_data = borrow_account_mut(&accounts[IX_ADVANCE_WORK])?;
                body.encode_into(clearing::clear_work_body_mut(&mut work_data)?)
                    .map_err(codec_fault)?;
                clearing::bind_order_set(&mut work_data, epoch.order_set, body.consumed_fold())?;
                Ok(())
            } else {
                Err(Refusal::Adapter(ClutchError::FeedProtocolFault))
            };
        }
        FeedStatusV1::NeedSlices => {
            // The slice pass is `AdvanceClearSlices`' instruction, not this
            // one's.
            return Err(Refusal::Adapter(ClutchError::FeedProtocolFault));
        }
    };

    // Header/body phase coherence, and the anchor at every bound resume.
    if pass == 1 {
        require(
            header.status == clearing::CLEAR_WORK_STATUS_OPEN,
            ClutchError::MismatchedState,
        )?;
    } else {
        require(
            header.status == clearing::CLEAR_WORK_STATUS_BOUND,
            ClutchError::MismatchedState,
        )?;
        require(
            body.consumed_fold() == header.consumed_fold,
            ClutchError::ResumeFoldMismatch,
        )?;
    }
    // The body's per-pass cursor and the header's live rank are one number.
    require(
        body.orders_consumed() == header.live_rank,
        ClutchError::MismatchedState,
    )?;

    let batch = walk_batch(
        program_id,
        accounts,
        epoch,
        &header,
        &page_header,
        &page_data,
        &frame.feed,
        &mut body,
        &mut owners,
        pass,
        max_orders,
    )?;
    // Exactly the reservations the batch consumed, no extras — unless the
    // feed refused early, in which case the tail was legitimately never
    // reached.
    if !batch.completed_early {
        require(
            IX_ADVANCE_RESERVATIONS + batch.reservations_consumed == accounts.len(),
            ClutchError::AccountCount,
        )?;
    }

    // Decide the header transition before touching the account.
    let page_exhausted = batch.next_slot == page_header.order_count as usize;
    let last_page = header.page_cursor + 1 == epoch.page_count;
    let mut bind_now = false;
    let mut rewind_now = false;
    let (next_page, next_slot) = if batch.completed_early {
        bind_now = header.status == clearing::CLEAR_WORK_STATUS_OPEN;
        (header.page_cursor, batch.next_slot as u8)
    } else if page_exhausted && last_page {
        // The pass is complete: close it in the same instruction, so the
        // cursor never rests one past the set without the pass having ended.
        match body.end_pass() {
            Ok(next_status) => {
                if pass == 1 {
                    // Join 4's owner half: the frozen epoch's owner count is
                    // exactly the interning count of the walked set.
                    require(
                        owners.count() == epoch.owner_count,
                        ClutchError::MismatchedState,
                    )?;
                    bind_now = true;
                }
                rewind_now = matches!(next_status, FeedStatusV1::NeedOrders { .. });
            }
            Err(error) => return Err(feed_fault(error)),
        }
        (epoch.page_count, 0)
    } else if page_exhausted {
        (header.page_cursor + 1, 0)
    } else {
        (header.page_cursor, batch.next_slot as u8)
    };

    drop(page_data);
    let mut work_data = borrow_account_mut(&accounts[IX_ADVANCE_WORK])?;
    body.encode_into(clearing::clear_work_body_mut(&mut work_data)?)
        .map_err(codec_fault)?;
    if pass == 1 {
        clearing::write_owner_interner(&mut work_data, &owners)?;
    }
    clearing::advance_walk(&mut work_data, next_page, next_slot, batch.live)?;
    if bind_now {
        clearing::bind_order_set(&mut work_data, epoch.order_set, body.consumed_fold())?;
    }
    if rewind_now {
        clearing::rewind_walk(&mut work_data)?;
    }
    Ok(())
}

/// The batch walk itself: skip to the cursor, then feed up to `max_orders`
/// live orders (skipping retirements) with their fills and — on pass 1 —
/// their reservations.
#[allow(clippy::too_many_arguments)] // one argument per authenticated fact
#[inline(never)]
fn walk_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    epoch: &EpochAccount,
    header: &clearing::ClearWorkHeader,
    page_header: &stream::OrderPageHeader,
    page_data: &[u8],
    feed: &CandidateFeedHeader,
    body: &mut ClearWorkV1,
    owners: &mut OwnerInterner,
    pass: u8,
    max_orders: u16,
) -> Outcome<BatchOutcome> {
    let feed_data = accounts[IX_ADVANCE_FEED].data.borrow();
    let mut cursor = stream::OrderSlotCursor::new(page_data)?;
    let mut index = 0usize;
    while index < header.slot_cursor as usize {
        match cursor.next_slot() {
            Some(step) => {
                step?;
            }
            None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        }
        index += 1;
    }

    let mut live = header.live_rank;
    let mut pushed: u16 = 0;
    let mut next_reservation = IX_ADVANCE_RESERVATIONS;
    let mut completed_early = false;
    while index < page_header.order_count as usize && pushed < max_orders && !completed_early {
        let slot = match cursor.next_slot() {
            Some(step) => step?,
            None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
        index += 1;
        let projected = projection::project_slot(&slot, live as u64 + 1, owners)?;
        let Some(order) = projected else {
            // A retirement: skipped, no rank consumed, no reservation owed.
            continue;
        };
        let fill = clearing::fill_at(&feed_data, feed, live as u8)?;
        if pass == 1 {
            require(next_reservation < accounts.len(), ClutchError::AccountCount)?;
            validate_walk_reservation(
                program_id,
                &accounts[next_reservation],
                epoch,
                page_header.page_index,
                &slot,
                false,
            )?;
            next_reservation += 1;
        }
        live += 1;
        pushed += 1;
        if body.push_order(&order, fill).map_err(feed_fault)? == FeedStatusV1::Complete {
            // A V0 admission refusal ends the feed at once; the verdict is
            // recorded and the close will persist it.
            completed_early = true;
        }
    }
    Ok(BatchOutcome {
        next_slot: index,
        live,
        completed_early,
        reservations_consumed: next_reservation - IX_ADVANCE_RESERVATIONS,
    })
}

/// Borrow one account's data mutably, or refuse.
fn borrow_account_mut<'a, 'info>(
    account: &'a AccountInfo<'info>,
) -> Outcome<core::cell::RefMut<'a, &'info mut [u8]>> {
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
}

/* ------------------------------------------------------------------------ */
/* The slice pass (tag 52) and the close (tag 53) — T2-6c                    */
/* ------------------------------------------------------------------------ */

/// Accounts in an `AdvanceClearSlices` instruction, exactly.
pub const ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT: usize = 3;
/// Accounts in a `CompleteClearWork` instruction, exactly.
pub const COMPLETE_CLEAR_WORK_ACCOUNT_COUNT: usize = 4;
/// The candidate record the close persists the verdict onto (writable).
pub const IX_COMPLETE_CANDIDATE: usize = 3;

/// One layout pairing slice as the relation's slice type.
fn relation_slice(slice: &clearing::PairingSlice) -> clutch_batch::relation_v1::PairingSliceV1 {
    fn leg(leg: clearing::LegRef) -> clutch_batch::relation_v1::LegRefV1 {
        match leg {
            clearing::LegRef::Order(index) => {
                clutch_batch::relation_v1::LegRefV1::Order(index)
            }
            clearing::LegRef::Split => clutch_batch::relation_v1::LegRefV1::Split,
            clearing::LegRef::Merge => clutch_batch::relation_v1::LegRefV1::Merge,
        }
    }
    clutch_batch::relation_v1::PairingSliceV1 {
        buy_ref: leg(slice.buy_ref),
        sell_ref: leg(slice.sell_ref),
        outcome: slice.outcome,
        quantity: slice.quantity,
    }
}

/// Advance the checkpoint's slice pass by up to `max_slices` declared
/// witness slices, closing the pass when the last one is fed.
#[inline(never)]
pub(super) fn advance_clear_slices(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
    max_slices: u16,
) -> Outcome<()> {
    accounts::require_count(accounts, ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    let frame = load_clearing_plane(
        program_id,
        &accounts[IX_ADVANCE_EPOCH],
        &accounts[IX_ADVANCE_FEED],
        &accounts[IX_ADVANCE_WORK],
        intent_market,
        intent_epoch,
        intent_candidate,
    )?;
    // Slices happen strictly after pass 1 bound the frozen set.
    require(
        frame.header.status == clearing::CLEAR_WORK_STATUS_BOUND,
        ClutchError::MismatchedState,
    )?;

    let mut body = decode_body_boxed(&accounts[IX_ADVANCE_WORK])?;
    require(
        body.consumed_fold() == frame.header.consumed_fold,
        ClutchError::ResumeFoldMismatch,
    )?;
    require(
        body.status() == FeedStatusV1::NeedSlices,
        ClutchError::FeedProtocolFault,
    )?;
    // `NeedSlices` implies a declared witness; stated, not assumed.
    let declared = frame
        .feed
        .declared_slices()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;

    let mut ended = false;
    {
        let feed_data = accounts[IX_ADVANCE_FEED].data.borrow();
        let mut pushed: u16 = 0;
        while pushed < max_slices && !ended {
            let index = body.slices_consumed();
            let slice = clearing::slice_at(&feed_data, &frame.feed, index)?;
            body.push_slice(&relation_slice(&slice)).map_err(feed_fault)?;
            pushed += 1;
            if body.slices_consumed() == declared {
                // The pass closes itself: the cursor never rests one past the
                // declared witness without the pass having ended.
                body.end_pass().map_err(feed_fault)?;
                ended = true;
            }
        }
    }

    let mut work_data = borrow_account_mut(&accounts[IX_ADVANCE_WORK])?;
    body.encode_into(clearing::clear_work_body_mut(&mut work_data)?)
        .map_err(codec_fault)?;
    if ended {
        // The next order pass walks the set from the top.
        clearing::rewind_walk(&mut work_data)?;
    }
    Ok(())
}

/// Close one complete checkpoint: persist the verdict onto the candidate
/// record, then complete the checkpoint so no pass may resume it.
#[inline(never)]
pub(super) fn complete_clear_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
) -> Outcome<()> {
    accounts::require_count(accounts, COMPLETE_CLEAR_WORK_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_distinct(accounts)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_COMPLETE_CANDIDATE],
        true,
        &[account_len::CANDIDATE],
    )?;
    let frame = load_clearing_plane(
        program_id,
        &accounts[IX_ADVANCE_EPOCH],
        &accounts[IX_ADVANCE_FEED],
        &accounts[IX_ADVANCE_WORK],
        intent_market,
        intent_epoch,
        intent_candidate,
    )?;
    // Only a bound checkpoint closes: an early V0-complete refusal was bound
    // by the advance that latched it, so an OPEN checkpoint here is a
    // checkpoint whose walk never ran.
    require(
        frame.header.status == clearing::CLEAR_WORK_STATUS_BOUND,
        ClutchError::MismatchedState,
    )?;

    let mut record = super::decode_candidate_boxed(&accounts[IX_COMPLETE_CANDIDATE].data.borrow())?;
    require(
        record.candidate == *intent_candidate
            && record.epoch == frame.epoch.epoch
            && record.market == frame.epoch.market
            && record.status == clutch_solana_layout::CANDIDATE_STATUS_SUBMITTED,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_COMPLETE_CANDIDATE].key,
        seeds::candidate_pda(
            program_id,
            &frame.epoch.epoch.bytes(),
            &intent_candidate.bytes(),
        ),
        Some(record.stored_bump),
    )?;

    let body = decode_body_boxed(&accounts[IX_ADVANCE_WORK])?;
    require(
        body.consumed_fold() == frame.header.consumed_fold,
        ClutchError::ResumeFoldMismatch,
    )?;
    require(
        body.status() == FeedStatusV1::Complete,
        ClutchError::FeedProtocolFault,
    )?;
    match body.verdict() {
        Some(Ok(summary)) => {
            /* Acceptance: the verified score is the streamed summary's
             * recomputed components plus the full-width relation-candidate
             * tie digest, recomputed here over the full-width domain and the
             * verified feed's stored regions.  The claimed u128 digest and
             * the claimed component fields are never consulted — they are
             * overwritten. */
            let tie_digest = {
                let feed_data = accounts[IX_ADVANCE_FEED].data.borrow();
                recompute_tie_digest(&frame.epoch, &frame.feed, &feed_data)?
            };
            record.status = clutch_solana_layout::CANDIDATE_STATUS_VERIFIED;
            record.weighted_direct_volume = summary.score.weighted_direct_volume;
            record.limit_surplus_price_units = summary.score.limit_surplus_price_units;
            record.distinct_owners = summary.score.distinct_owners;
            record.churn = summary.score.churn;
            record.score_digest = tie_digest;
        }
        Some(Err(_relation_refusal)) => {
            /* A relation refusal is the verdict, not a fault: the candidate
             * is marked REFUSED (its claimed components stay the claims they
             * always were, and its tie digest stays zero — a refused
             * candidate competes for nothing) and the checkpoint completes
             * so no pass may relitigate it. */
            record.status = clutch_solana_layout::CANDIDATE_STATUS_REFUSED;
        }
        // A complete feed always holds a verdict; stated, not assumed.
        None => return Err(Refusal::Adapter(ClutchError::FeedProtocolFault)),
    }

    record.encode(&mut borrow_account_mut(&accounts[IX_COMPLETE_CANDIDATE])?)?;
    clearing::complete_clear_work(&mut borrow_account_mut(&accounts[IX_ADVANCE_WORK])?)?;
    Ok(())
}

/// A layout identity as the policy crate's full-width identity type.
fn identity(hash: Hash32) -> clutch_batch_policy_identity::Identity32V1 {
    clutch_batch_policy_identity::Identity32V1(hash.bytes())
}

/// Recompute the full-width relation-candidate tie digest of one feed's
/// stored regions under the epoch's full-width domain.
///
/// The one construction of the verified tie identity, shared by the two
/// consumers with opposite duties: `complete_clear_work` *stamps* its result
/// onto the accepted record, and `FinalizeSelection` *refuses* any record
/// whose stored digest a fresh recomputation over the presented feed bytes
/// does not reproduce.  Never the claimed u128, in either place.
#[inline(never)]
pub(super) fn recompute_tie_digest(
    epoch: &EpochAccount,
    feed: &CandidateFeedHeader,
    feed_data: &[u8],
) -> Outcome<Hash32> {
    let full_domain = clutch_batch_policy_identity::FullRelationDomainV1 {
        relation_version: epoch.relation_version,
        market_id: identity(epoch.market),
        book_id: identity(epoch.book),
        epoch_id: identity(epoch.epoch),
        policy_id: identity(epoch.policy),
        order_set_id: identity(epoch.order_set),
        epoch_index: epoch.epoch_index,
        outcome_count: epoch.outcome_count,
        owner_count: epoch.owner_count,
        price_scale: epoch.price_scale,
        remainder_seed: epoch.remainder_seed,
        policy: GENERAL_CLEARING_POLICY_V1,
    };
    let domain_digest = full_domain
        .digest()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let fills = clearing::candidate_feed_fill_region(feed_data)?;
    let digest = match feed.declared_slices() {
        Some(declared) => {
            let slices = clearing::candidate_feed_slice_region(feed_data)?;
            clutch_batch_policy_identity::full_relation_candidate_digest_from_regions(
                domain_digest,
                identity(feed.candidate),
                fills,
                feed.honored_aon_mask,
                Some((declared, slices)),
            )
        }
        None => clutch_batch_policy_identity::full_relation_candidate_digest_from_regions(
            domain_digest,
            identity(feed.candidate),
            fills,
            feed.honored_aon_mask,
            None,
        ),
    };
    let digest = digest.map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(Hash32::from_bytes(digest.0))
}
