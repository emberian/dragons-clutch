//! Entitlement freeze and generalized portfolio consumption — Tier 2 join 8,
//! T2-8.
//!
//! `Intent::FreezeEntitlement` (tag 58), `Intent::EntitleSlice` (59), and the
//! portfolio full-pair half of the widened `Intent::SettlePage`: the joins
//! that make a SELECTED candidate's verified allocation *consumable*.
//!
//! ## The freeze (58 → 59 × slice)
//!
//! `FreezeEntitlement` opens the freeze: it reads the **completed** clearing
//! checkpoint — whose body still holds the streamed relation verdict — and
//! creates the epoch's [`FinalPotAccount`] funded from the verified summary's
//! rounding/refund scalars.  The first cut refuses, stated in code and mapped
//! onto the honest-stub code `NotYetImplemented`:
//!
//! * a summary carrying `virtual_split`/`virtual_merge` — the VirtualPot
//!   ranked blocker stands, so no receipt may reference a virtual leg;
//! * a summary whose rounding pot is nonzero — the exact-only consumption
//!   seam cannot fund a rounding pot, so a candidate needing the terminal
//!   floor's remainder atoms stands behind the same family.
//!
//! With both refused the pot's scalars are provably zero and the pot is
//! created `POT_PHASE_CLOSED`, whose codec invariant *enforces* emptiness —
//! the V3 precedent's shape placeholder that proves rather than promises.
//!
//! `EntitleSlice` is the resumable per-slice half: one witness slice per
//! invocation, receipt PDA per `(candidate, slice_index)`, replay held by the
//! receipt's own non-existence.  Against the complete digest-verified frozen
//! page set it re-derives the slice's two live orders, requires **full
//! fills** on both ends (the PartialFillLedger blocker stands: a slice that
//! does not cover its ends' whole legs, or an order split across slices,
//! refuses `NotYetImplemented`), re-validates both reservations exactly as
//! the pass-1 walk did, flips them `ACTIVE → ENTITLED`, and creates the
//! receipt(s):
//!
//! * a **single-Egg pair** slice freezes one receipt, with the exact
//!   consideration's divisibility required up front so no unconsumable
//!   receipt is ever minted;
//! * a **portfolio pair** slice freezes the receipts of *every* slice of
//!   that pair atomically — the pair consumes atomically through the
//!   full-pair seam, so its entitlement freezes atomically — with the two
//!   fundings' canonical claims and primitive units required equal;
//! * a **mixed** single/portfolio slice refuses `NotYetImplemented` — a
//!   standing shape of the same partial-fill family.
//!
//! ## Consumption (the portfolio half of `SettlePage`)
//!
//! [`settle_portfolio_pair`] consumes one entitled full pair: every presented
//! receipt binds this exact `(candidate, buy, sell)` pair and is unconsumed,
//! their per-outcome quantities must sum to **exactly** the pair's full Egg
//! vector (recomputed from the frozen records — so a strict subset undersums
//! and refuses, and nothing else can reference the pair), and the value moves
//! through `portfolio_settlement::{prepare,apply}_full_pair` — Tier 1's
//! out-param forms — with the entitlement content rebuilt deterministically
//! from program-authenticated state, never read from a claim.  Both
//! reservations take `CONSUMED` with their `initial_*` envelope intact (the
//! persisted account is the archive of the exact consumed amounts), and every
//! receipt is stamped exhausted in the same instruction.
//!
//! ## Claim plane
//!
//! SBF-EXECUTED (bank), no promotion.  The reference adapter refuses both
//! new intents with `UnsupportedIntent`; the oracle is the layout codec plus
//! the frozen receipt/pot codecs, byte for byte.

use crate::accounts::{
    self, expect_pda, require, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program, RentParameters,
};
use crate::seeds;
use clutch_batch::relation_v1_stream::FeedStatusV1;
use clutch_solana_layout::clearing::{self, canonical_general_book_id, CandidateFeedHeader};
use clutch_solana_layout::portfolio_settlement::{
    canonical_portfolio_entitlement_id, exact_portfolio_value_price_units, prepare_full_pair,
    PortfolioEntitlementV1, PortfolioFundingV1, PortfolioPairInputV1, PortfolioPairPostV1,
    PortfolioSettlementError, PORTFOLIO_ENTITLEMENT_ACTIVE,
};
use clutch_solana_layout::reservation::{
    ReservationAccount, RESERVATION_ACCOUNT_BYTES, RESERVATION_STATE_ENTITLED,
};
use clutch_solana_layout::{
    account_len, stream, CandidateRecord, EpochAccount, FinalPotAccount, Hash32, OrderSlot,
    PortfolioRecord, SettlementReceiptAccount, TermsAccount,
    CANDIDATE_STATUS_SELECTED, EPOCH_PHASE_CLEARED, MAX_OUTCOMES, ORDER_KIND_PORTFOLIO,
    POT_PHASE_CLOSED, RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED,
    RECEIPT_FLAG_SLICE_EXHAUSTED, RECEIPT_LEG_DIRECT,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Accounts in a `FreezeEntitlement` instruction, exactly.
pub const FREEZE_ENTITLEMENT_ACCOUNT_COUNT: usize = 7;
/// Authenticated payer funding the pot's rent.
pub const IX_FREEZE_ENT_PAYER: usize = 0;
/// The CLEARED general epoch (read-only, program-owned).
pub const IX_FREEZE_ENT_EPOCH: usize = 1;
/// The SELECTED candidate record (read-only, program-owned).
pub const IX_FREEZE_ENT_RECORD: usize = 2;
/// The completed clearing checkpoint holding the verified summary (read-only).
pub const IX_FREEZE_ENT_WORK: usize = 3;
/// The canonical, not-yet-created final-pot PDA.
pub const IX_FREEZE_ENT_POT: usize = 4;
/// System program.
pub const IX_FREEZE_ENT_SYSTEM: usize = 5;
/// Rent sysvar.
pub const IX_FREEZE_ENT_RENT: usize = 6;

/// Fixed accounts in an `EntitleSlice` instruction, before the page list and
/// the per-shape tail.
pub const ENTITLE_SLICE_FIXED_ACCOUNT_COUNT: usize = 7;
/// Authenticated payer funding the receipt rent.
pub const IX_ENTITLE_PAYER: usize = 0;
/// The CLEARED general epoch (read-only, program-owned).
pub const IX_ENTITLE_EPOCH: usize = 1;
/// The SELECTED candidate record (read-only, program-owned).
pub const IX_ENTITLE_RECORD: usize = 2;
/// The selected candidate's sealed feed (read-only, program-owned).
pub const IX_ENTITLE_FEED: usize = 3;
/// The epoch's final pot — proof the entitlement freeze opened (read-only).
pub const IX_ENTITLE_POT: usize = 4;
/// System program.
pub const IX_ENTITLE_SYSTEM: usize = 5;
/// Rent sysvar.
pub const IX_ENTITLE_RENT: usize = 6;
/// First frozen page; exactly `epoch.page_count` of them, in index order,
/// then the per-shape tail.
pub const IX_ENTITLE_PAGES: usize = ENTITLE_SLICE_FIXED_ACCOUNT_COUNT;

/// Tail of a single-Egg pair entitlement: buy reservation, sell reservation,
/// and the one creatable receipt.
pub const ENTITLE_SINGLE_TAIL_ACCOUNT_COUNT: usize = 3;
/// Fixed tail prefix of a portfolio pair entitlement: the Terms account and
/// both reservations, then one creatable receipt per pair slice.
pub const ENTITLE_PAIR_TAIL_FIXED_ACCOUNT_COUNT: usize = 3;

/// Fixed accounts in the portfolio full-pair `SettlePage` shape, before the
/// page list and the receipt tail.
pub const SETTLE_PAIR_FIXED_ACCOUNT_COUNT: usize = 7;
/// The CLEARED general epoch (read-only, program-owned).
pub const IX_PAIR_EPOCH: usize = 0;
/// The SELECTED candidate record (read-only, program-owned).
pub const IX_PAIR_CANDIDATE: usize = 1;
/// The immutable Terms account owning the native basis (read-only).
pub const IX_PAIR_TERMS: usize = 2;
/// Buyer Position (writable).
pub const IX_PAIR_BUY_POSITION: usize = 3;
/// Seller Position (writable).
pub const IX_PAIR_SELL_POSITION: usize = 4;
/// Buy order's ENTITLED reservation (writable).
pub const IX_PAIR_BUY_RESERVATION: usize = 5;
/// Sell order's ENTITLED reservation (writable).
pub const IX_PAIR_SELL_RESERVATION: usize = 6;
/// First frozen page holding a pair record; one or two pages (two exactly
/// when the ends live on different pages), then the pair's receipts.
pub const IX_PAIR_PAGES: usize = SETTLE_PAIR_FIXED_ACCOUNT_COUNT;

/// Most receipts one pair consumption may present — one per active outcome
/// is the canonical shape; split legs stay behind the entitlement freeze's
/// own bound.
pub const MAX_PAIR_RECEIPTS: usize = MAX_OUTCOMES;

/// Map a portfolio-settlement fault onto the program's refusal plane.
fn portfolio_fault(error: PortfolioSettlementError) -> Refusal {
    match error {
        PortfolioSettlementError::Codec(inner) => Refusal::Codec(inner),
        PortfolioSettlementError::FeeCarryAuthorityUnavailable => {
            Refusal::Adapter(ClutchError::AuthorizationUnavailable)
        }
        PortfolioSettlementError::ArithmeticOverflow => Refusal::Adapter(ClutchError::Arithmetic),
        PortfolioSettlementError::InsufficientFunding => {
            Refusal::Adapter(ClutchError::AggregateClosureMismatch)
        }
        /* An inexact conversion is a standing consumption gap, not an
         * inconsistency: the entitlement freeze refuses it up front, so
         * reaching it here is the same honest stub. */
        PortfolioSettlementError::InexactConsideration => {
            Refusal::Adapter(ClutchError::NotYetImplemented)
        }
        PortfolioSettlementError::ClaimMismatch
        | PortfolioSettlementError::MismatchedBinding
        | PortfolioSettlementError::AlreadyConsumed => {
            Refusal::Adapter(ClutchError::MismatchedState)
        }
    }
}

/// Decode and authenticate the CLEARED epoch and its SELECTED candidate.
#[inline(never)]
fn load_cleared_plane(
    program_id: &Pubkey,
    epoch_account: &AccountInfo,
    record_account: &AccountInfo,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
) -> Outcome<(Box<EpochAccount>, Box<CandidateRecord>)> {
    accounts::validate_state_role_lengths(program_id, epoch_account, false, &[account_len::EPOCH])?;
    accounts::validate_state_role_lengths(
        program_id,
        record_account,
        false,
        &[account_len::CANDIDATE],
    )?;
    let epoch = super::decode_epoch_boxed(&epoch_account.data.borrow())?;
    require(
        epoch.market == *intent_market && epoch.epoch == *intent_epoch,
        ClutchError::MismatchedState,
    )?;
    require(epoch.phase == EPOCH_PHASE_CLEARED, ClutchError::NotActive)?;
    // Only the general book family entitles here; a direct epoch is a
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
    Ok((epoch, record))
}

/* ------------------------------------------------------------------------ */
/* FreezeEntitlement (tag 58)                                                */
/* ------------------------------------------------------------------------ */

/// Create the epoch's final pot from the selected candidate's verified
/// summary — the entitlement freeze's opening move.
#[inline(never)]
pub(super) fn freeze_entitlement(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
) -> Outcome<()> {
    accounts::require_count(accounts, FREEZE_ENTITLEMENT_ACCOUNT_COUNT)?;
    /* The pot PDA's non-existence is the real replay guard; the zero
     * sequence states the intent's position in the protocol on the wire. */
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[IX_FREEZE_ENT_PAYER])?;
    require(
        accounts[IX_FREEZE_ENT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    let (epoch, record) = load_cleared_plane(
        program_id,
        &accounts[IX_FREEZE_ENT_EPOCH],
        &accounts[IX_FREEZE_ENT_RECORD],
        intent_market,
        intent_epoch,
    )?;
    require(
        record.candidate == *intent_candidate,
        ClutchError::MismatchedState,
    )?;
    record.binds_epoch(&epoch, u16::from(record.order_len))?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_FREEZE_ENT_WORK],
        false,
        &[account_len::CLEAR_WORK],
    )?;
    require_creatable(&accounts[IX_FREEZE_ENT_POT])?;
    require_system_program(&accounts[IX_FREEZE_ENT_SYSTEM])?;
    let rent = read_rent(&accounts[IX_FREEZE_ENT_RENT])?;

    // The completed checkpoint: its layout header must be COMPLETE for this
    // exact triple, and its body — anchored by the consumed fold — still
    // holds the streamed verdict the close persisted the score from.
    let header = {
        let data = accounts[IX_FREEZE_ENT_WORK].data.borrow();
        clearing::verify_clear_work(&data)?
    };
    require(
        header.market == epoch.market
            && header.epoch == epoch.epoch
            && header.candidate == record.candidate
            && header.status == clearing::CLEAR_WORK_STATUS_COMPLETE,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_FREEZE_ENT_WORK].key,
        seeds::clear_work_pda(program_id, &epoch.epoch.bytes(), &record.candidate.bytes()),
        Some(header.stored_bump),
    )?;
    let body = super::clear_walk::decode_body_boxed(&accounts[IX_FREEZE_ENT_WORK])?;
    require(
        body.consumed_fold() == header.consumed_fold,
        ClutchError::ResumeFoldMismatch,
    )?;
    require(
        body.status() == FeedStatusV1::Complete,
        ClutchError::FeedProtocolFault,
    )?;
    let summary = match body.verdict() {
        Some(Ok(summary)) => summary,
        // A SELECTED candidate's checkpoint holds an accepting verdict by
        // construction; stated, not assumed.
        _ => return Err(Refusal::Adapter(ClutchError::FeedProtocolFault)),
    };

    /* The stated first-cut refusals, on the honest-stub code:
     * virtual legs need the funded VirtualPot transition (standing ranked
     * blocker), and a nonzero rounding pot needs a consumption path that can
     * realize remainder atoms, which the exact-only seam cannot. */
    require(
        summary.virtual_split == 0 && summary.virtual_merge == 0,
        ClutchError::NotYetImplemented,
    )?;
    require(
        summary.rounding_pot_price_units == 0,
        ClutchError::NotYetImplemented,
    )?;
    // The pinned policy is fee-free; a nonzero verified fee scalar would be
    // an inconsistency, not a stub.
    require(
        summary.fee_price_units == 0 && summary.fee_carry_bps_units == 0,
        ClutchError::AuthorizationUnavailable,
    )?;

    // The pot, funded from the verified summary's rounding/refund scalars —
    // all provably zero after the refusals above — and created CLOSED, whose
    // codec invariant enforces the emptiness (the V3 pot precedent).
    let epoch_bytes = epoch.epoch.bytes();
    let (pot_address, pot_bump) = seeds::pot_pda(program_id, &epoch_bytes);
    expect_pda(accounts[IX_FREEZE_ENT_POT].key, (pot_address, pot_bump), None)?;
    let bump_seed = [pot_bump];
    let signer = [seeds::SEED_POT, epoch_bytes.as_ref(), bump_seed.as_ref()];
    create_pda_account(
        program_id,
        &accounts[IX_FREEZE_ENT_PAYER],
        &accounts[IX_FREEZE_ENT_POT],
        &accounts[IX_FREEZE_ENT_SYSTEM],
        &rent,
        account_len::FINAL_POT,
        &signer,
    )?;
    let pot = FinalPotAccount {
        epoch: epoch.epoch,
        market: epoch.market,
        candidate: record.candidate,
        pot_internal: [0; MAX_OUTCOMES],
        pot_cash_price_units: 0,
        rounding_pot_price_units: summary.rounding_pot_price_units,
        outcome_count: epoch.outcome_count,
        phase: POT_PHASE_CLOSED,
        stored_bump: pot_bump,
        flags: 0,
    };
    pot.encode(&mut borrow_account_mut(&accounts[IX_FREEZE_ENT_POT])?)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* EntitleSlice (tag 59)                                                     */
/* ------------------------------------------------------------------------ */

/// One live order located by the page walk.
struct Located {
    slot: OrderSlot,
    page_index: u16,
}

/// Walk the complete bound page set once: verify inclusion, count live
/// orders, and capture the two slots at the slice's live ranks.
#[inline(never)]
fn locate_pair(
    pages: &[AccountInfo],
    epoch: &EpochAccount,
    buy_rank: u8,
    sell_rank: u8,
) -> Outcome<(Located, Located, u16)> {
    let borrows: Vec<core::cell::Ref<'_, &mut [u8]>> =
        pages.iter().map(|page| page.data.borrow()).collect();
    let refs: Vec<&[u8]> = borrows.iter().map(|data| &***data as &[u8]).collect();
    // Inclusion is not inferred from headers carrying the same `order_set`:
    // every page is present, digest-verified, and folded into the epoch's
    // frozen commitment here.
    stream::epoch_binds_page_set(epoch, &refs)?;
    let mut live: u16 = 0;
    let mut buy: Option<Located> = None;
    let mut sell: Option<Located> = None;
    for (page_index, page) in refs.iter().enumerate() {
        let header = stream::OrderPageHeader::decode(page)?;
        let mut cursor = stream::OrderSlotCursor::new(page)?;
        let mut index = 0usize;
        while index < header.order_count as usize {
            let slot = match cursor.next_slot() {
                Some(step) => step?,
                None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
            };
            if slot.is_live() {
                if live == u16::from(buy_rank) {
                    buy = Some(Located {
                        slot,
                        page_index: page_index as u16,
                    });
                }
                if live == u16::from(sell_rank) {
                    sell = Some(Located {
                        slot,
                        page_index: page_index as u16,
                    });
                }
                live += 1;
            }
            index += 1;
        }
    }
    match (buy, sell) {
        (Some(buy), Some(sell)) => Ok((buy, sell, live)),
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
}

/// Verify the selected candidate's sealed feed and bind it to the plane.
#[inline(never)]
fn load_bound_feed(
    program_id: &Pubkey,
    feed_account: &AccountInfo,
    epoch: &EpochAccount,
    candidate: Hash32,
) -> Outcome<CandidateFeedHeader> {
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
        feed.candidate == candidate
            && feed.epoch == epoch.epoch
            && feed.market == epoch.market
            && feed.order_set == epoch.order_set
            && feed.outcome_count == epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        feed_account.key,
        seeds::candidate_feed_pda(program_id, &epoch.epoch.bytes(), &candidate.bytes()),
        Some(feed.stored_bump),
    )?;
    Ok(feed)
}

/// Require the epoch's pot to exist for this candidate: proof the entitlement
/// freeze ran, with its summary refusals discharged.
#[inline(never)]
fn require_frozen_pot(
    program_id: &Pubkey,
    pot_account: &AccountInfo,
    epoch: &EpochAccount,
    record: &CandidateRecord,
) -> Outcome<()> {
    accounts::validate_state_role_lengths(program_id, pot_account, false, &[account_len::FINAL_POT])?;
    let pot = FinalPotAccount::decode(&pot_account.data.borrow())?;
    pot.binds_candidate(record)?;
    expect_pda(
        pot_account.key,
        seeds::pot_pda(program_id, &epoch.epoch.bytes()),
        Some(pot.stored_bump),
    )?;
    Ok(())
}

/// What the whole-witness scan concluded about one slice's pair.
struct PairCoverage {
    /// Indices of every slice referencing exactly this (buy, sell) pair,
    /// ascending; the entry slice is `indices[0]`.
    indices: [u16; MAX_PAIR_RECEIPTS],
    count: usize,
}

/// Scan every declared witness slice once.
///
/// For a single-Egg pair the rule is exclusivity: any *other* slice touching
/// either rank is a split order, which stands behind the PartialFillLedger
/// blocker.  For a portfolio pair the rule is completeness: every slice
/// touching either rank must reference exactly this pair, and all of them are
/// collected for atomic entitlement.
#[inline(never)]
fn scan_witness(
    feed_data: &[u8],
    feed: &CandidateFeedHeader,
    buy_rank: u8,
    sell_rank: u8,
) -> Outcome<PairCoverage> {
    let declared = feed
        .declared_slices()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut coverage = PairCoverage {
        indices: [0; MAX_PAIR_RECEIPTS],
        count: 0,
    };
    let mut index = 0u16;
    while index < declared {
        let slice = clearing::slice_at(feed_data, feed, index)?;
        let buys = slice.buy_ref == clearing::LegRef::Order(buy_rank);
        let sells = slice.sell_ref == clearing::LegRef::Order(sell_rank);
        let touches_other = slice.buy_ref == clearing::LegRef::Order(sell_rank)
            || slice.sell_ref == clearing::LegRef::Order(buy_rank);
        // An end appearing on its opposite side elsewhere would have refused
        // at verification (`SliceNotExecutable`); stated, not assumed.
        require(!touches_other, ClutchError::MismatchedState)?;
        if buys && sells {
            require(
                coverage.count < MAX_PAIR_RECEIPTS,
                ClutchError::NotYetImplemented,
            )?;
            coverage.indices[coverage.count] = index;
            coverage.count += 1;
        } else if buys || sells {
            // The pair's ends may not fragment across counterparties in this
            // cut: cumulative per-order consumption state is the standing
            // PartialFillLedger blocker.
            return Err(Refusal::Adapter(ClutchError::NotYetImplemented));
        }
        index += 1;
    }
    require(coverage.count >= 1, ClutchError::MismatchedState)?;
    Ok(coverage)
}

/// Flip one walk-validated reservation `ACTIVE → ENTITLED` and persist it.
#[inline(never)]
fn entitle_reservation(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    page_index: u16,
    slot: &OrderSlot,
) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    // Exactly the pass-1 walk's reservation validation: ACTIVE state, the
    // untouched exact envelope, and the canonical address re-derived from
    // this instruction's own facts.
    super::clear_walk::validate_walk_reservation(program_id, account, epoch, page_index, slot)?;
    let mut reservation = super::decode_reservation_boxed(&account.data.borrow())?;
    reservation.state = RESERVATION_STATE_ENTITLED;
    reservation.validate()?;
    reservation.encode(&mut borrow_account_mut(account)?)?;
    Ok(())
}

/// Create and write one receipt at its canonical PDA.
#[allow(clippy::too_many_arguments)] // one argument per creation coordinate
#[inline(never)]
fn create_receipt<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &RentParameters,
    epoch_id: Hash32,
    candidate_id: Hash32,
    receipt: &SettlementReceiptAccount,
) -> Outcome<()> {
    require_creatable(target)?;
    let epoch_bytes = epoch_id.bytes();
    let candidate_bytes = candidate_id.bytes();
    let (address, bump) = seeds::receipt_pda(
        program_id,
        &epoch_bytes,
        &candidate_bytes,
        receipt.slice_index,
    );
    expect_pda(target.key, (address, bump), None)?;
    let slice_seed = receipt.slice_index.to_le_bytes();
    let bump_seed = [bump];
    let signer = [
        seeds::SEED_RECEIPT,
        epoch_bytes.as_ref(),
        candidate_bytes.as_ref(),
        slice_seed.as_ref(),
        bump_seed.as_ref(),
    ];
    create_pda_account(
        program_id,
        payer,
        target,
        system,
        rent,
        account_len::SETTLEMENT_RECEIPT,
        &signer,
    )?;
    let mut stamped = *receipt;
    stamped.stored_bump = bump;
    stamped.encode(&mut borrow_account_mut(target)?)?;
    Ok(())
}

/// Entitle one witness slice of the selected candidate: create its
/// receipt(s) and flip both referenced reservations `ACTIVE → ENTITLED`.
#[inline(never)]
pub(super) fn entitle_slice(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_candidate: &Hash32,
    slice_index: u16,
) -> Outcome<()> {
    /* The receipt PDA's non-existence is the real replay guard, per slice. */
    require(sequence == 0, ClutchError::Replay)?;
    require(
        accounts.len() > ENTITLE_SLICE_FIXED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    require_signer(&accounts[IX_ENTITLE_PAYER])?;
    require(
        accounts[IX_ENTITLE_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    require_system_program(&accounts[IX_ENTITLE_SYSTEM])?;
    let (epoch, record) = load_cleared_plane(
        program_id,
        &accounts[IX_ENTITLE_EPOCH],
        &accounts[IX_ENTITLE_RECORD],
        intent_market,
        intent_epoch,
    )?;
    require(
        record.candidate == *intent_candidate,
        ClutchError::MismatchedState,
    )?;
    let feed = load_bound_feed(
        program_id,
        &accounts[IX_ENTITLE_FEED],
        &epoch,
        record.candidate,
    )?;
    require_frozen_pot(program_id, &accounts[IX_ENTITLE_POT], &epoch, &record)?;

    let page_count = usize::from(epoch.page_count);
    require(
        accounts.len() > ENTITLE_SLICE_FIXED_ACCOUNT_COUNT + page_count,
        ClutchError::AccountCount,
    )?;
    let pages = &accounts[IX_ENTITLE_PAGES..IX_ENTITLE_PAGES + page_count];
    for page in pages {
        accounts::validate_state_role_lengths(program_id, page, false, &[account_len::ORDER_PAGE])?;
    }
    let tail = &accounts[IX_ENTITLE_PAGES + page_count..];

    // The slice being entitled; both ends must be real orders (a virtual leg
    // cannot exist here — the pot's freeze refused virtual summaries — and
    // is refused as the standing VirtualPot stub if presented anyway).
    let slice = {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        clearing::slice_at(&data, &feed, slice_index)?
    };
    let (buy_rank, sell_rank) = match (slice.buy_ref, slice.sell_ref) {
        (clearing::LegRef::Order(buy), clearing::LegRef::Order(sell)) => (buy, sell),
        _ => return Err(Refusal::Adapter(ClutchError::NotYetImplemented)),
    };
    require(buy_rank != sell_rank, ClutchError::MismatchedState)?;

    let (buy, sell, live_order_count) = locate_pair(pages, &epoch, buy_rank, sell_rank)?;
    // T2-4's exact live-cardinality binding, recomputed from the
    // digest-verified set this instruction just walked.
    record.binds_epoch(&epoch, live_order_count)?;

    // Full fills on both ends, or the PartialFillLedger blocker stands.
    {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        let buy_fill = clearing::fill_at(&data, &feed, buy_rank)?;
        let sell_fill = clearing::fill_at(&data, &feed, sell_rank)?;
        require(
            buy_fill == order_full_size(&buy.slot)? && sell_fill == order_full_size(&sell.slot)?,
            ClutchError::NotYetImplemented,
        )?;
    }
    let coverage = {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        scan_witness(&data, &feed, buy_rank, sell_rank)?
    };

    match (buy.slot, sell.slot) {
        (OrderSlot::Single(_), OrderSlot::Single(_)) => entitle_single_slice(
            program_id,
            accounts,
            tail,
            &epoch,
            &record,
            &slice,
            slice_index,
            &buy,
            &sell,
            &coverage,
        ),
        (OrderSlot::Portfolio(_), OrderSlot::Portfolio(_)) => entitle_portfolio_pair(
            program_id,
            accounts,
            tail,
            &epoch,
            &record,
            slice_index,
            &buy,
            &sell,
            &coverage,
        ),
        /* A mixed single/portfolio pair needs per-leg consumption state the
         * full-pair seam does not model; it stands with the partial-fill
         * family. */
        _ => Err(Refusal::Adapter(ClutchError::NotYetImplemented)),
    }
}

/// One live order's full size in relation units: Egg atoms for singles,
/// lots for portfolios.
fn order_full_size(slot: &OrderSlot) -> Outcome<u64> {
    match slot {
        OrderSlot::Single(order) => Ok(order.quantity),
        OrderSlot::Portfolio(order) => Ok(order.lots),
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        }
    }
}

/// The single-Egg pair half of `EntitleSlice`.
#[allow(clippy::too_many_arguments)] // one argument per authenticated fact
#[inline(never)]
fn entitle_single_slice<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    tail: &[AccountInfo<'a>],
    epoch: &EpochAccount,
    record: &CandidateRecord,
    slice: &clearing::PairingSlice,
    slice_index: u16,
    buy: &Located,
    sell: &Located,
    coverage: &PairCoverage,
) -> Outcome<()> {
    require(
        tail.len() == ENTITLE_SINGLE_TAIL_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
    // Exclusivity: exactly this one slice touches either end, or the order
    // is split across slices and the PartialFillLedger blocker stands.
    require(
        coverage.count == 1 && coverage.indices[0] == slice_index,
        ClutchError::NotYetImplemented,
    )?;
    let (buy_order, sell_order) = match (&buy.slot, &sell.slot) {
        (OrderSlot::Single(buy_order), OrderSlot::Single(sell_order)) => (buy_order, sell_order),
        _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };
    require(
        buy_order.side == 0
            && sell_order.side == 1
            && buy_order.owner != sell_order.owner
            && buy_order.outcome == sell_order.outcome
            && buy_order.outcome == slice.outcome,
        ClutchError::MismatchedState,
    )?;
    // Full one-to-one: the slice covers both whole legs, or the
    // PartialFillLedger blocker stands.
    require(
        slice.quantity != 0
            && slice.quantity == buy_order.quantity
            && slice.quantity == sell_order.quantity,
        ClutchError::NotYetImplemented,
    )?;
    let price = record.prices[usize::from(slice.outcome)];
    require(
        price <= buy_order.limit && price >= sell_order.limit,
        ClutchError::MismatchedState,
    )?;
    let consideration_price_units = u128::from(slice.quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(epoch.price_scale != 0, ClutchError::MismatchedState)?;
    /* Exact-or-stand: an inexact per-slice conversion needs the rounding
     * boundary's pot flow, which the exact-only seam cannot fund yet.  The
     * refusal here is what keeps every minted receipt consumable. */
    require(
        consideration_price_units % u128::from(epoch.price_scale) == 0,
        ClutchError::NotYetImplemented,
    )?;

    // State writes before the creation CPI (the terminal.rs ordering rule).
    entitle_reservation(program_id, &tail[0], epoch, buy.page_index, &buy.slot)?;
    entitle_reservation(program_id, &tail[1], epoch, sell.page_index, &sell.slot)?;

    let receipt = SettlementReceiptAccount {
        epoch: epoch.epoch,
        market: epoch.market,
        candidate: record.candidate,
        buy_order_id: buy_order.order_id,
        sell_order_id: sell_order.order_id,
        consideration_price_units,
        quantity: slice.quantity,
        settled_quantity: 0,
        price,
        sequence: u64::from(slice_index)
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        slice_index,
        outcome: slice.outcome,
        leg_kind: RECEIPT_LEG_DIRECT,
        consumed_flags: 0,
        stored_bump: 0,
        flags: 0,
    };
    create_receipt(
        program_id,
        &accounts[IX_ENTITLE_PAYER],
        &tail[2],
        &accounts[IX_ENTITLE_SYSTEM],
        &read_rent(&accounts[IX_ENTITLE_RENT])?,
        epoch.epoch,
        record.candidate,
        &receipt,
    )
}

/// The exact pair facts shared by entitlement and consumption.
struct PairFunding {
    claim: Hash32,
    primitive_units: u64,
    internal: [u64; MAX_OUTCOMES],
}

/// Recompute both ends' canonical fundings and require them to be one full
/// pair: same claim, same primitive units, same exact Egg vector.
#[inline(never)]
fn pair_funding(
    market: Hash32,
    terms: &TermsAccount,
    price_scale: u64,
    buy: &PortfolioRecord,
    sell: &PortfolioRecord,
) -> Outcome<PairFunding> {
    let buy_funding =
        PortfolioFundingV1::for_order(market, terms, price_scale, buy, 0).map_err(portfolio_fault)?;
    let sell_funding = PortfolioFundingV1::for_order(market, terms, price_scale, sell, 0)
        .map_err(portfolio_fault)?;
    require(
        buy_funding.claim == sell_funding.claim
            && buy_funding.primitive_units == sell_funding.primitive_units
            && buy_funding.internal == sell_funding.internal,
        ClutchError::MismatchedState,
    )?;
    Ok(PairFunding {
        claim: buy_funding.claim.claim,
        primitive_units: buy_funding.primitive_units,
        internal: buy_funding.internal,
    })
}

/// Load and bind the immutable Terms account the epoch names.
#[inline(never)]
fn load_bound_terms(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
) -> Outcome<Box<TermsAccount>> {
    accounts::validate_state_role_lengths(program_id, account, false, &[account_len::TERMS])?;
    let terms = super::decode_terms_boxed(&account.data.borrow())?;
    require(
        terms.terms == epoch.terms
            && terms.price_grid == epoch.price_grid
            && terms.outcome_count == epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes()),
        Some(terms.stored_bump),
    )?;
    Ok(terms)
}

/// The portfolio pair half of `EntitleSlice`: freeze every slice of the pair
/// atomically.
#[allow(clippy::too_many_arguments)] // one argument per authenticated fact
#[inline(never)]
fn entitle_portfolio_pair<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    tail: &[AccountInfo<'a>],
    epoch: &EpochAccount,
    record: &CandidateRecord,
    slice_index: u16,
    buy: &Located,
    sell: &Located,
    coverage: &PairCoverage,
) -> Outcome<()> {
    require(
        tail.len() == ENTITLE_PAIR_TAIL_FIXED_ACCOUNT_COUNT + coverage.count,
        ClutchError::AccountCount,
    )?;
    // The pair entitles exactly once, from its first slice: any other entry
    // point refuses, and the receipts' non-existence guards the replay.
    require(
        coverage.indices[0] == slice_index,
        ClutchError::MismatchedState,
    )?;
    let (buy_record, sell_record) = match (&buy.slot, &sell.slot) {
        (OrderSlot::Portfolio(buy_record), OrderSlot::Portfolio(sell_record)) => {
            (buy_record, sell_record)
        }
        _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };
    require(
        buy_record.side == 0 && sell_record.side == 1 && buy_record.owner != sell_record.owner,
        ClutchError::MismatchedState,
    )?;
    let terms = load_bound_terms(program_id, &tail[0], epoch)?;
    let funding = pair_funding(
        epoch.market,
        &terms,
        epoch.price_scale,
        buy_record,
        sell_record,
    )?;

    // Every collected slice must cover a whole per-outcome leg exactly once
    // in aggregate: the sums re-derive the pair's full Egg vector.  The
    // staged receipts live on the heap: sixteen receipts would fill the SBF
    // frame on their own.
    let mut sums = [0u64; MAX_OUTCOMES];
    let mut prepared = boxed_zero_receipts()?;
    {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        let feed = clearing::CandidateFeedHeader::decode(&data)?;
        let mut at = 0usize;
        while at < coverage.count {
            let index = coverage.indices[at];
            let pair_slice = clearing::slice_at(&data, &feed, index)?;
            let outcome = usize::from(pair_slice.outcome);
            require(
                pair_slice.outcome < epoch.outcome_count,
                ClutchError::MismatchedState,
            )?;
            sums[outcome] = sums[outcome]
                .checked_add(pair_slice.quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            let price = record.prices[outcome];
            prepared[at] = SettlementReceiptAccount {
                epoch: epoch.epoch,
                market: epoch.market,
                candidate: record.candidate,
                buy_order_id: buy_record.order_id,
                sell_order_id: sell_record.order_id,
                consideration_price_units: u128::from(pair_slice.quantity)
                    .checked_mul(u128::from(price))
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
                quantity: pair_slice.quantity,
                settled_quantity: 0,
                price,
                sequence: u64::from(index)
                    .checked_add(1)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
                slice_index: index,
                outcome: pair_slice.outcome,
                leg_kind: RECEIPT_LEG_DIRECT,
                consumed_flags: 0,
                stored_bump: 0,
                flags: 0,
            };
            at += 1;
        }
    }
    require(sums == funding.internal, ClutchError::MismatchedState)?;
    // The pair's exact value must convert to whole collateral atoms, or the
    // rounding-pot gap stands and no unconsumable receipt is minted.
    let consideration = exact_portfolio_value_price_units(
        &funding.internal,
        &record.prices,
        terms.outcome_count,
    )
    .map_err(portfolio_fault)?;
    require(epoch.price_scale != 0, ClutchError::MismatchedState)?;
    require(
        consideration % u128::from(epoch.price_scale) == 0,
        ClutchError::NotYetImplemented,
    )?;

    // State writes before the creation CPIs (the terminal.rs ordering rule).
    entitle_reservation(program_id, &tail[1], epoch, buy.page_index, &buy.slot)?;
    entitle_reservation(program_id, &tail[2], epoch, sell.page_index, &sell.slot)?;

    let rent = read_rent(&accounts[IX_ENTITLE_RENT])?;
    let mut at = 0usize;
    while at < coverage.count {
        create_receipt(
            program_id,
            &accounts[IX_ENTITLE_PAYER],
            &tail[ENTITLE_PAIR_TAIL_FIXED_ACCOUNT_COUNT + at],
            &accounts[IX_ENTITLE_SYSTEM],
            &rent,
            epoch.epoch,
            record.candidate,
            &prepared[at],
        )?;
        at += 1;
    }
    Ok(())
}

/// The all-zero receipt placeholder `entitle_portfolio_pair` stages into.
const ZERO_RECEIPT: SettlementReceiptAccount = SettlementReceiptAccount {
    epoch: Hash32::ZERO,
    market: Hash32::ZERO,
    candidate: Hash32::ZERO,
    buy_order_id: Hash32::ZERO,
    sell_order_id: Hash32::ZERO,
    consideration_price_units: 0,
    quantity: 0,
    settled_quantity: 0,
    price: 0,
    sequence: 0,
    slice_index: 0,
    outcome: 0,
    leg_kind: 0,
    consumed_flags: 0,
    stored_bump: 0,
    flags: 0,
};

/// A fresh all-zero staged-receipt table on the heap.
#[inline(never)]
fn boxed_zero_receipts() -> Outcome<Box<[SettlementReceiptAccount; MAX_PAIR_RECEIPTS]>> {
    static ZERO: [SettlementReceiptAccount; MAX_PAIR_RECEIPTS] =
        [ZERO_RECEIPT; MAX_PAIR_RECEIPTS];
    super::boxed_copy_of(&ZERO)
}

/* ------------------------------------------------------------------------ */
/* Portfolio full-pair consumption (the SettlePage widening)                 */
/* ------------------------------------------------------------------------ */

/// Program-owned roles of the fixed pair-consumption prefix.
const SETTLE_PAIR_STATE_ROLES: [StateRole; SETTLE_PAIR_FIXED_ACCOUNT_COUNT] = [
    StateRole::read_only(IX_PAIR_EPOCH, account_len::EPOCH),
    StateRole::read_only(IX_PAIR_CANDIDATE, account_len::CANDIDATE),
    StateRole::read_only(IX_PAIR_TERMS, account_len::TERMS),
    StateRole::writable(IX_PAIR_BUY_POSITION, account_len::POSITION),
    StateRole::writable(IX_PAIR_SELL_POSITION, account_len::POSITION),
    StateRole::writable(IX_PAIR_BUY_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_PAIR_SELL_RESERVATION, RESERVATION_ACCOUNT_BYTES),
];

/// One ENTITLED portfolio reservation of the pair, decoded and bound.
#[inline(never)]
fn load_pair_reservation(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    side: u8,
) -> Outcome<Box<ReservationAccount>> {
    let reservation = super::decode_reservation_boxed(&account.data.borrow())?;
    require(
        reservation.state == RESERVATION_STATE_ENTITLED
            && reservation.order_kind == ORDER_KIND_PORTFOLIO
            && reservation.side == side
            && reservation.market == epoch.market
            && reservation.epoch == epoch.epoch
            && reservation.terms == epoch.terms
            && reservation.price_grid == epoch.price_grid
            && reservation.policy == epoch.policy
            && reservation.outcome_count == epoch.outcome_count
            && reservation.release_generation == 0
            && reservation.remaining_cash_atoms == reservation.initial_cash_atoms
            && reservation.remaining_internal == reservation.initial_internal,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::reservation_pda(program_id, &reservation.reservation.bytes()),
        Some(reservation.stored_bump),
    )?;
    Ok(reservation)
}

/// One frozen page of the set, verified walk-style and bound to the epoch.
#[inline(never)]
fn require_bound_page(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    expected_index: u16,
) -> Outcome<()> {
    accounts::validate_state_role_lengths(program_id, account, false, &[account_len::ORDER_PAGE])?;
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
    )
}

/// Load one live Portfolio record from a verified page by exact order id —
/// the FrozenPageProvenance obligation, discharged.
#[inline(never)]
fn load_portfolio_record(
    account: &AccountInfo,
    reservation: &ReservationAccount,
) -> Outcome<Box<PortfolioRecord>> {
    let data = account.data.borrow();
    let header = stream::OrderPageHeader::decode(&data)?;
    let mut cursor = stream::OrderSlotCursor::new(&data)?;
    let mut index = 0usize;
    while index < header.order_count as usize {
        let slot = match cursor.next_slot() {
            Some(step) => step?,
            None => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
        if slot.is_live() && slot.order_id() == reservation.order_id {
            return match slot {
                OrderSlot::Portfolio(record) => {
                    require(
                        record.owner == reservation.owner
                            && record.generation == reservation.order_generation,
                        ClutchError::MismatchedState,
                    )?;
                    Ok(Box::new(record))
                }
                _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
            };
        }
        index += 1;
    }
    Err(Refusal::Adapter(ClutchError::MismatchedState))
}

/// Rebuild the pair's entitlement content deterministically from
/// program-authenticated state.
#[allow(clippy::too_many_arguments)] // one argument per authenticated fact
#[inline(never)]
fn rebuild_entitlement(
    epoch: &EpochAccount,
    record: &CandidateRecord,
    terms: &TermsAccount,
    funding: &PairFunding,
    buy_order_id: Hash32,
    sell_order_id: Hash32,
) -> Outcome<Box<PortfolioEntitlementV1>> {
    let consideration =
        exact_portfolio_value_price_units(&funding.internal, &record.prices, terms.outcome_count)
            .map_err(portfolio_fault)?;
    let mut entitlement = Box::new(PortfolioEntitlementV1 {
        entitlement: Hash32::ZERO,
        market: epoch.market,
        epoch: epoch.epoch,
        candidate: record.candidate,
        terms: terms.terms,
        price_grid: terms.price_grid,
        policy: epoch.policy,
        claim: funding.claim,
        buy_order_id,
        sell_order_id,
        prices: record.prices,
        price_scale: epoch.price_scale,
        outcome_count: terms.outcome_count,
        primitive_units: funding.primitive_units,
        consideration_price_units: consideration,
        state: PORTFOLIO_ENTITLEMENT_ACTIVE,
    });
    entitlement.entitlement = canonical_portfolio_entitlement_id(
        entitlement.market,
        entitlement.epoch,
        entitlement.candidate,
        entitlement.terms,
        entitlement.price_grid,
        entitlement.policy,
        entitlement.claim,
        entitlement.buy_order_id,
        entitlement.sell_order_id,
        &entitlement.prices,
        entitlement.price_scale,
        entitlement.outcome_count,
        entitlement.primitive_units,
        entitlement.consideration_price_units,
    );
    entitlement.validate().map_err(portfolio_fault)?;
    Ok(entitlement)
}

/// The staged out-slot for the full-pair seam, built on the heap.
#[inline(never)]
fn boxed_zero_post() -> Outcome<Box<PortfolioPairPostV1>> {
    static ZERO: PortfolioPairPostV1 = PortfolioPairPostV1::ZEROED;
    super::boxed_copy_of(&ZERO)
}

/// Consume one entitled portfolio full pair: every receipt of the pair, both
/// ENTITLED reservations, and the exact vector-for-cash transfer through the
/// full-pair seam.
#[inline(never)]
pub(super) fn settle_portfolio_pair(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_page: u16,
) -> Outcome<()> {
    require(
        accounts.len() >= SETTLE_PAIR_FIXED_ACCOUNT_COUNT + 2,
        ClutchError::AccountCount,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(
        program_id,
        &accounts[..SETTLE_PAIR_FIXED_ACCOUNT_COUNT],
        &SETTLE_PAIR_STATE_ROLES,
    )?;
    let (epoch, record) = load_cleared_plane(
        program_id,
        &accounts[IX_PAIR_EPOCH],
        &accounts[IX_PAIR_CANDIDATE],
        intent_market,
        intent_epoch,
    )?;
    record.binds_epoch(&epoch, u16::from(record.order_len))?;
    let terms = load_bound_terms(program_id, &accounts[IX_PAIR_TERMS], &epoch)?;

    let buy_reservation =
        load_pair_reservation(program_id, &accounts[IX_PAIR_BUY_RESERVATION], &epoch, 0)?;
    let sell_reservation =
        load_pair_reservation(program_id, &accounts[IX_PAIR_SELL_RESERVATION], &epoch, 1)?;
    // The wire's page coordinate is the buy end's page, honestly consumed.
    require(
        buy_reservation.page_index == intent_page,
        ClutchError::MismatchedState,
    )?;
    // Fees need a frozen fee base and a named recipient.  Until that exists,
    // only a signed zero-fee envelope can cross this seam.
    require(
        buy_reservation.max_fee_atoms == 0 && sell_reservation.max_fee_atoms == 0,
        ClutchError::AuthorizationUnavailable,
    )?;

    // One page when both ends share it, two otherwise; then the receipts.
    let page_count = if buy_reservation.page_index == sell_reservation.page_index {
        1
    } else {
        2
    };
    require(
        accounts.len() > SETTLE_PAIR_FIXED_ACCOUNT_COUNT + page_count,
        ClutchError::AccountCount,
    )?;
    let buy_page = &accounts[IX_PAIR_PAGES];
    require_bound_page(program_id, buy_page, &epoch, buy_reservation.page_index)?;
    let sell_page = if page_count == 2 {
        let page = &accounts[IX_PAIR_PAGES + 1];
        require_bound_page(program_id, page, &epoch, sell_reservation.page_index)?;
        page
    } else {
        buy_page
    };
    let buy_record = load_portfolio_record(buy_page, &buy_reservation)?;
    let sell_record = load_portfolio_record(sell_page, &sell_reservation)?;
    require(
        buy_record.side == 0 && sell_record.side == 1,
        ClutchError::MismatchedState,
    )?;
    let funding = pair_funding(
        epoch.market,
        &terms,
        epoch.price_scale,
        &buy_record,
        &sell_record,
    )?;

    // Every presented receipt binds exactly this pair, is unconsumed, and is
    // presented in strictly ascending slice order; their per-outcome sums
    // must re-derive the pair's full Egg vector exactly, which is what makes
    // a partial presentation undersum and refuse.
    let receipts = &accounts[SETTLE_PAIR_FIXED_ACCOUNT_COUNT + page_count..];
    require(
        !receipts.is_empty() && receipts.len() <= MAX_PAIR_RECEIPTS,
        ClutchError::AccountCount,
    )?;
    let mut sums = [0u64; MAX_OUTCOMES];
    let mut previous_slice: Option<u16> = None;
    let mut first_sequence = 0u64;
    for (at, account) in receipts.iter().enumerate() {
        accounts::validate_state_role_lengths(
            program_id,
            account,
            true,
            &[account_len::SETTLEMENT_RECEIPT],
        )?;
        let receipt = super::decode_receipt_boxed(&account.data.borrow())?;
        receipt.binds_candidate(&record)?;
        expect_pda(
            account.key,
            seeds::receipt_pda(
                program_id,
                &receipt.epoch.bytes(),
                &receipt.candidate.bytes(),
                receipt.slice_index,
            ),
            Some(receipt.stored_bump),
        )?;
        require(
            receipt.leg_kind == RECEIPT_LEG_DIRECT
                && receipt.buy_order_id == buy_reservation.order_id
                && receipt.sell_order_id == sell_reservation.order_id
                && receipt.settled_quantity == 0
                && receipt.consumed_flags == 0
                && receipt.outcome < epoch.outcome_count,
            ClutchError::MismatchedState,
        )?;
        if let Some(previous) = previous_slice {
            require(receipt.slice_index > previous, ClutchError::MismatchedState)?;
        }
        previous_slice = Some(receipt.slice_index);
        if at == 0 {
            first_sequence = receipt.sequence;
        }
        sums[usize::from(receipt.outcome)] = sums[usize::from(receipt.outcome)]
            .checked_add(receipt.quantity)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    require(sequence == first_sequence, ClutchError::Replay)?;
    require(sums == funding.internal, ClutchError::MismatchedState)?;

    let entitlement = rebuild_entitlement(
        &epoch,
        &record,
        &terms,
        &funding,
        buy_record.order_id,
        sell_record.order_id,
    )?;

    let mut buyer_position =
        super::decode_position_boxed(&accounts[IX_PAIR_BUY_POSITION].data.borrow())?;
    let mut seller_position =
        super::decode_position_boxed(&accounts[IX_PAIR_SELL_POSITION].data.borrow())?;
    super::validate_position_address(program_id, &accounts[IX_PAIR_BUY_POSITION])?;
    super::validate_position_address(program_id, &accounts[IX_PAIR_SELL_POSITION])?;

    // The full-pair seam: prepare stages the complete post-state through the
    // caller-provided out-slot, then the values are committed and persisted.
    let mut staged = boxed_zero_post()?;
    prepare_full_pair(
        &PortfolioPairInputV1 {
            terms: &terms,
            buy_order: &buy_record,
            sell_order: &sell_record,
            buyer_position: &buyer_position,
            seller_position: &seller_position,
            buyer_reservation: &buy_reservation,
            seller_reservation: &sell_reservation,
            entitlement: &entitlement,
        },
        &mut staged,
    )
    .map_err(portfolio_fault)?;
    *buyer_position = staged.buyer_position;
    *seller_position = staged.seller_position;

    buyer_position.encode(&mut borrow_account_mut(&accounts[IX_PAIR_BUY_POSITION])?)?;
    seller_position.encode(&mut borrow_account_mut(&accounts[IX_PAIR_SELL_POSITION])?)?;
    staged
        .buyer_reservation
        .encode(&mut borrow_account_mut(&accounts[IX_PAIR_BUY_RESERVATION])?)?;
    staged
        .seller_reservation
        .encode(&mut borrow_account_mut(&accounts[IX_PAIR_SELL_RESERVATION])?)?;
    for account in receipts {
        let mut receipt = super::decode_receipt_boxed(&account.data.borrow())?;
        receipt.settled_quantity = receipt.quantity;
        receipt.consumed_flags =
            RECEIPT_FLAG_BUY_CONSUMED | RECEIPT_FLAG_SELL_CONSUMED | RECEIPT_FLAG_SLICE_EXHAUSTED;
        receipt.validate()?;
        receipt.encode(&mut borrow_account_mut(account)?)?;
    }
    Ok(())
}

/// Borrow one account's data mutably, or refuse.
fn borrow_account_mut<'a, 'info>(
    account: &'a AccountInfo<'info>,
) -> Outcome<core::cell::RefMut<'a, &'info mut [u8]>> {
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
}
