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
//! creates the epoch's [`FinalPotAccount`] carrying the verified summary's
//! rounding and churn expectations.  One refusal stands, stated in code and
//! mapped onto the honest-stub code `NotYetImplemented`: a summary carrying
//! `virtual_merge`, whose payee credit would fall due before the burn that
//! funds it (`settlement::SETTLEMENT_BLOCKERS`).
//!
//! An epoch expecting neither residue nor churn is created
//! `POT_PHASE_CLOSED`, whose codec invariant *enforces* emptiness — the V3
//! precedent's shape placeholder that proves rather than promises.  One
//! expecting either is created `POT_PHASE_OPEN` and economically empty, and
//! `SettlePage` is what fills and drains it.
//!
//! `EntitleSlice` is the resumable per-slice half: one witness slice per
//! invocation, receipt PDA per `(candidate, slice_index)`, replay held by the
//! receipt's own non-existence.  Against the complete digest-verified frozen
//! page set it re-derives the slice's two live orders, scans the whole
//! declared witness once to total each end's Egg atoms *per order*, requires
//! those totals to equal the verified fills, and routes on shape:
//!
//! * an **exclusive portfolio pair** — no slice touching either end names a
//!   different counterparty — freezes the receipts of *every* slice of that
//!   pair atomically, with the two fundings' canonical claims and primitive
//!   units required equal.  It consumes atomically through the full-pair
//!   seam, so it freezes atomically, and its reservations carry no per-order
//!   ledger because nothing else has a claim on either envelope.
//! * **everything else** — a fragmented or partially filled single-Egg pair,
//!   a mixed single/portfolio pair, a portfolio end pairing with more than
//!   one counterparty — takes the generalized per-slice route: one receipt at
//!   this slice's canonical PDA, and each end's reservation stamped with its
//!   *whole order's* entitled total.  The first touch flips
//!   `ACTIVE → ENTITLED` and writes the stamp; every later touch re-derives
//!   the same total and requires equality, so a forged stamp cannot survive
//!   recomputation.  A full one-to-one fill is the one-slice special case.
//!
//! * a **virtual** slice, whose sell end is the global split: one real end,
//!   one `RECEIPT_LEG_SPLIT` receipt whose counterparty is the epoch pot, and
//!   the same per-order stamp on that end's reservation.  Its virtual side
//!   converts nothing — the pot carries it in exact price units — so it is
//!   admissible at prices no ordinary pair could convert.
//!
//! All routes require the exact consideration's divisibility of their *real*
//! ends up front, so no unconsumable receipt is ever minted.
//!
//! ## Consumption (the portfolio half of `SettlePage`)
//!
//! The `pub(super)` `settle_portfolio_pair` consumes one entitled full pair:
//! every presented
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
    canonical_reservation_id, ReservationAccount, ReservationPlan, RESERVATION_ACCOUNT_BYTES,
    RESERVATION_STATE_ACTIVE, RESERVATION_STATE_ENTITLED,
};
use clutch_solana_layout::{
    account_len, stream, CandidateRecord, EpochAccount, FinalPotAccount, Hash32, OrderSlot,
    PortfolioRecord, SettlementReceiptAccount, TermsAccount, CANDIDATE_STATUS_SELECTED,
    EPOCH_PHASE_CLEARED, MAX_OUTCOMES, ORDER_KIND_PORTFOLIO, POT_PHASE_CLOSED, POT_PHASE_OPEN,
    RECEIPT_FLAG_BUY_CONSUMED, RECEIPT_FLAG_SELL_CONSUMED, RECEIPT_FLAG_SLICE_EXHAUSTED,
    RECEIPT_LEG_DIRECT, RECEIPT_LEG_SPLIT,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Accounts in a `FreezeEntitlement` instruction without funding registration.
pub const FREEZE_ENTITLEMENT_ACCOUNT_COUNT: usize = 7;
/// Accounts in a `FreezeEntitlement` instruction that also registers the
/// pot's funding: one optional trailing `GeneralFundingLedgerV1` PDA,
/// written in the same transition that debits the payer.
pub const FREEZE_ENTITLEMENT_LEDGERED_ACCOUNT_COUNT: usize = FREEZE_ENTITLEMENT_ACCOUNT_COUNT + 1;
/// The optional funding-ledger PDA.
pub const IX_FREEZE_ENT_LEDGER: usize = 7;
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

/// Tail of one per-slice entitlement: buy reservation, sell reservation, and
/// the one creatable receipt.  The universal shape — every route but the
/// atomic portfolio full pair presents exactly this.
pub const ENTITLE_SINGLE_TAIL_ACCOUNT_COUNT: usize = 3;
/// Tail of one virtual-slice entitlement: the single real end's reservation
/// and the one creatable receipt.  The pot is not presented — the freeze's
/// pot is already read at [`IX_ENTITLE_POT`] and nothing here writes it.
pub const ENTITLE_VIRTUAL_TAIL_ACCOUNT_COUNT: usize = 2;
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
    require(
        accounts.len() == FREEZE_ENTITLEMENT_ACCOUNT_COUNT
            || accounts.len() == FREEZE_ENTITLEMENT_LEDGERED_ACCOUNT_COUNT,
        ClutchError::AccountCount,
    )?;
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

    /* The virtual legs, recorded rather than refused — but only in the one
     * direction the pot can fund without ever holding an unbacked claim.
     *
     * A virtual leg is a complete-set **mint or burn**: `relation_v1.rs:
     * 3830-3832` requires the split to serve `sigma` Egg atoms on *every*
     * outcome and the merge to absorb `mu` on every outcome, and
     * `relation_v1.rs:2749-2757` prices that at `sigma * price_scale` /
     * `mu * price_scale` — exactly one collateral atom per complete set,
     * because prices lie on the scaled simplex (`relation_v1.rs:1218-1250`).
     * `clutch_kernel::MarketState::split` moves every outcome together and
     * `required_collateral_for` is `max_i total_supply[i]`, so a mint is a
     * *complete set* and never a leg.
     *
     * Who backs it, derived rather than asserted.  Write `V_e` for one end's
     * cumulative exact consumed value in price units.  Every consumed slice
     * adds `q * p` to its buy end and the same `q * p` to its sell end; a
     * split slice has no sell end and a merge slice no buy end, so
     *
     *     sum_buys V - sum_sells V  =  (split value consumed)
     *                               -  (merge value consumed).
     *
     * The seam debits a payer `ceil(V/S)` atoms and credits a payee
     * `floor(V/S)` (`settlement::convert_leg`), realizing the gap once at
     * completion.  So the pot's exact cash identity is
     *
     *     pot_cash  =  (sum_buys V - sum_sells V)
     *               +  sum over still-open ends of their pending gap.
     *
     * At the freeze both terms are zero: **the pot has no net worth here, so
     * the freeze cannot mint.**  Minting `sigma` sets now would raise
     * `HoardAccount::collateral_atoms` against atoms that still belong to the
     * buyers' Positions (`reserved_cash_atoms` is a *sub-field* of
     * `cash_atoms`, see `settlement::apply_entitled_slice_consumption`) — the
     * same Hoard token atoms attributed twice, which is exactly the unbacked
     * mint this join must not perform.  The freeze therefore only *records*
     * the expectation, and `SettlePage`'s virtual shape mints later, after it
     * has collected `sigma * price_scale` price units of really unallocated
     * collateral.  `settlement.rs`'s `VirtualPot` retirement note carries the
     * rest of the order.
     *
     * The merge direction is the one that stays refused, and narrowly: with
     * `mu != 0` the identity above is strictly *negative* before the burn —
     * the pot must pay a payee before it holds the complete sets whose burn
     * would fund the payment — and deferring a payee's credit past its Egg
     * delivery needs a `paid_units` term the v2 `ReservationAccount` schema
     * does not carry. */
    require(summary.virtual_merge == 0, ClutchError::NotYetImplemented)?;
    /* The record and the streamed verdict must agree on the churn
     * coordinates: the record is what `SettlePage` reads `sigma` from, and a
     * record that overstated it would size a mint the summary never priced. */
    require(
        summary.virtual_split == record.virtual_split
            && summary.virtual_merge == record.virtual_merge,
        ClutchError::MismatchedState,
    )?;
    // The pinned policy is fee-free; a nonzero verified fee scalar would be
    // an inconsistency, not a stub.
    require(
        summary.fee_price_units == 0 && summary.fee_carry_bps_units == 0,
        ClutchError::AuthorizationUnavailable,
    )?;
    /* The rounding pot, funded.  Under `TerminalOwnerFloor`
     * (`relation_v1.rs:2482-2497`) a payer's owed price units convert *up* to
     * collateral atoms and a payee's convert *down*, and every remainder atom
     * lands in one non-negative pot — so the pot is funded by nobody outside
     * the book: it is exactly the payers' round-up excess plus the payees'
     * round-down shortfall, value the floor-side owners did not receive.
     *
     * The V8 closure (`relation_v1.rs:2749-2757`) gives
     * `(debit_atoms - credit_atoms) * price_scale == sigma * price_scale
     * + rounding_pot`, whose left side is a whole number of atoms — so
     * `rounding_pot` is a multiple of the price scale whether or not the
     * candidate carries churn.  Requiring it here is a real cross-check of
     * the streamed verdict, and it is what lets the consumption seam realize
     * the residue by simply never crediting it. */
    require(epoch.price_scale != 0, ClutchError::MismatchedState)?;
    let rounding_pot_price_units = summary.rounding_pot_price_units;
    require(
        rounding_pot_price_units.is_multiple_of(u128::from(epoch.price_scale)),
        ClutchError::AggregateClosureMismatch,
    )?;

    /* The pot carries the verified expectations and starts economically
     * empty: its Egg vector and cash field are canonically zero here because
     * nothing has been collected yet, and `SettlePage` is what fills them.
     * An epoch that expects neither residue nor churn is created CLOSED,
     * whose codec invariant *enforces* the emptiness (the V3 pot precedent);
     * an epoch that expects either is created OPEN — each completing order
     * draws its residue share down and each virtual leg pays into and then
     * draws from the pot's inventory, so the pot reaching zero on both terms
     * when the last receipt consumes is the whole-plane closure. */
    let epoch_bytes = epoch.epoch.bytes();
    let (pot_address, pot_bump) = seeds::pot_pda(program_id, &epoch_bytes);
    expect_pda(
        accounts[IX_FREEZE_ENT_POT].key,
        (pot_address, pot_bump),
        None,
    )?;
    let pot_prior = accounts[IX_FREEZE_ENT_POT].lamports();
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
        rounding_pot_price_units,
        outcome_count: epoch.outcome_count,
        phase: if rounding_pot_price_units == 0 && record.virtual_split == 0 {
            POT_PHASE_CLOSED
        } else {
            POT_PHASE_OPEN
        },
        stored_bump: pot_bump,
        flags: 0,
    };
    pot.encode(&mut borrow_account_mut(&accounts[IX_FREEZE_ENT_POT])?)?;
    if accounts.len() == FREEZE_ENTITLEMENT_LEDGERED_ACCOUNT_COUNT {
        super::terminal_closure::create_funding_ledger(
            program_id,
            &accounts[IX_FREEZE_ENT_PAYER],
            &accounts[IX_FREEZE_ENT_LEDGER],
            &accounts[IX_FREEZE_ENT_SYSTEM],
            &rent,
            accounts[IX_FREEZE_ENT_POT].key,
            clearing::FUNDING_COVERS_FINAL_POT,
            super::terminal_closure::creation_shortfall(
                rent.minimum_balance(account_len::FINAL_POT)?,
                pot_prior,
            ),
            pot_prior,
        )?;
    }
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
///
/// Returns the pot's verified residue expectation, which is what decides
/// whether this epoch's conversions are exact and therefore whether the
/// per-order realization needs the per-owner coincidence below.
#[inline(never)]
fn require_frozen_pot(
    program_id: &Pubkey,
    pot_account: &AccountInfo,
    epoch: &EpochAccount,
    record: &CandidateRecord,
) -> Outcome<u128> {
    accounts::validate_state_role_lengths(
        program_id,
        pot_account,
        false,
        &[account_len::FINAL_POT],
    )?;
    let pot = FinalPotAccount::decode(&pot_account.data.borrow())?;
    pot.binds_candidate(record)?;
    expect_pda(
        pot_account.key,
        seeds::pot_pda(program_id, &epoch.epoch.bytes()),
        Some(pot.stored_bump),
    )?;
    Ok(pot.rounding_pot_price_units)
}

/// Count the selected candidate's filled orders, from the digest-verified feed.
///
/// One number, compared against the relation's recomputed
/// `distinct_participating_owners`: equality says every participating owner
/// holds exactly **one** filled order, which is exactly when the frozen
/// `TerminalOwnerFloor` boundary's per-owner conversion
/// (`relation_v1.rs:2482-2497`, which sums an owner's orders into
/// `debit_units[slot]`/`credit_units[slot]` *before* converting) coincides
/// with the per-order conversion the consumption seam can realize from a
/// reservation's own ledger.  Without that equality an owner's two inexact
/// orders would each round on their own and the epoch would leave more
/// unallocated than the relation verified, so the receipts refuse to exist.
#[inline(never)]
fn filled_order_count(feed_data: &[u8], feed: &CandidateFeedHeader) -> Outcome<u16> {
    let mut count = 0u16;
    let mut rank = 0u8;
    while rank < feed.order_len {
        if clearing::fill_at(feed_data, feed, rank)? != 0 {
            count = count
                .checked_add(1)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        rank = rank
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    Ok(count)
}

/// What the whole-witness scan concluded about one slice's two ends.
struct PairCoverage {
    /// Indices of slices referencing exactly this (buy, sell) pair, ascending.
    ///
    /// Filled only up to [`MAX_PAIR_RECEIPTS`]; `stored < pair_count` means
    /// the pair fragments past the atomic route's own bound and only the
    /// per-slice route can serve it.
    indices: [u16; MAX_PAIR_RECEIPTS],
    stored: usize,
    /// How many slices reference exactly this pair, whether stored or not.
    pair_count: u16,
    /// Egg atoms the whole witness moves through the buy end.
    buy_total: u64,
    /// Egg atoms the whole witness moves through the sell end.
    sell_total: u64,
    /// Whether the buy end also pairs with a different counterparty.
    buy_elsewhere: bool,
    /// Whether the sell end also pairs with a different counterparty.
    sell_elsewhere: bool,
}

impl PairCoverage {
    /// Whether every slice touching either end references exactly this pair.
    ///
    /// The atomic portfolio full-pair route is admissible only here: it
    /// consumes both whole envelopes in one instruction, which is sound
    /// exactly when nothing else has a claim on either end.
    fn exclusive(&self) -> bool {
        !self.buy_elsewhere && !self.sell_elsewhere && self.stored == usize::from(self.pair_count)
    }
}

/// Scan every declared witness slice once, totalling per *order*.
///
/// Two facts come out of one pass: which slices reference exactly this
/// (buy, sell) pair — the atomic route's receipt set — and how many Egg atoms
/// the whole witness moves through each end, which is what the per-order
/// entitlement stamp is checked against.  Fragmentation is no longer a
/// refusal; it is a route.
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
        stored: 0,
        pair_count: 0,
        buy_total: 0,
        sell_total: 0,
        buy_elsewhere: false,
        sell_elsewhere: false,
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
        if buys {
            coverage.buy_total = coverage
                .buy_total
                .checked_add(slice.quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        if sells {
            coverage.sell_total = coverage
                .sell_total
                .checked_add(slice.quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        if buys && sells {
            coverage.pair_count = coverage
                .pair_count
                .checked_add(1)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            if coverage.stored < MAX_PAIR_RECEIPTS {
                coverage.indices[coverage.stored] = index;
                coverage.stored += 1;
            }
        } else {
            coverage.buy_elsewhere |= buys;
            coverage.sell_elsewhere |= sells;
        }
        index += 1;
    }
    require(coverage.pair_count >= 1, ClutchError::MismatchedState)?;
    Ok(coverage)
}

/// One live order's entitled total in Egg atoms, from its verified fill.
///
/// A single-Egg order's fill is already Egg atoms; a portfolio order's fill is
/// lots, whose Egg content is the coefficient sum times the lots.  Both ends
/// of every slice, and therefore the per-order ledger, speak this one unit.
fn entitled_total(slot: &OrderSlot, fill: u64) -> Outcome<u64> {
    match slot {
        OrderSlot::Single(_) => Ok(fill),
        OrderSlot::Portfolio(order) => {
            let mut sum = 0u64;
            let mut i = 0usize;
            while i < usize::from(order.active_len) {
                sum = sum
                    .checked_add(order.coefficients[i])
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
                i += 1;
            }
            fill.checked_mul(sum)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
        }
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        }
    }
}

/// Flip one walk-validated reservation `ACTIVE → ENTITLED` and persist it.
///
/// The atomic portfolio full-pair route's entry: it consumes both whole
/// envelopes in one later instruction and so carries no per-order ledger, and
/// its reservations stay unstamped.
#[inline(never)]
fn entitle_reservation(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    page_index: u16,
    slot: &OrderSlot,
) -> Outcome<()> {
    // Exactly the pass-1 walk's reservation validation — ACTIVE state, the
    // untouched exact envelope, and the canonical address re-derived from
    // this instruction's own facts — under a writable role.
    super::clear_walk::validate_walk_reservation(
        program_id, account, epoch, page_index, slot, true,
    )?;
    let mut reservation = super::decode_reservation_boxed(&account.data.borrow())?;
    reservation.state = RESERVATION_STATE_ENTITLED;
    reservation.validate()?;
    reservation.encode(&mut borrow_account_mut(account)?)?;
    Ok(())
}

/// Re-bind one already-`ENTITLED` reservation to this instruction's own facts.
///
/// Deliberately *not* `validate_walk_reservation`: a later slice of the same
/// order may arrive after an earlier slice of it was already consumed, so the
/// remaining envelope is not required untouched.  Everything that identifies
/// the account — owner, order, generation, page, the frozen coordinates, the
/// zero fee, the *initial* envelope, and the canonical address — is rebound.
#[inline(never)]
fn validate_entitled_touch(
    program_id: &Pubkey,
    epoch: &EpochAccount,
    page_index: u16,
    slot: &OrderSlot,
    account_key: &Pubkey,
    reservation: &ReservationAccount,
) -> Outcome<()> {
    let plan = ReservationPlan::for_order(slot, epoch.outcome_count, epoch.price_scale, 0)?;
    require(
        reservation.market == epoch.market
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
            && reservation.initial_internal == plan.internal
            && reservation.order_kind == plan.order_kind
            && reservation.side == plan.side,
        ClutchError::MismatchedState,
    )?;
    let reservation_id = canonical_reservation_id(
        epoch.market,
        epoch.epoch,
        slot.owner(),
        reservation.position_generation,
        slot.order_id(),
    );
    expect_pda(
        account_key,
        seeds::reservation_pda(program_id, &reservation_id.bytes()),
        Some(reservation.stored_bump),
    )
}

/// Stamp one end of a per-slice entitlement with its whole order's total.
///
/// First touch flips `ACTIVE → ENTITLED` and writes the stamp; every later
/// touch re-derives the same total from the digest-verified feed and requires
/// equality, so a forged stamp cannot survive recomputation and no entry slice
/// can widen the order's admission.
#[inline(never)]
fn stamp_reservation(
    program_id: &Pubkey,
    account: &AccountInfo,
    epoch: &EpochAccount,
    page_index: u16,
    slot: &OrderSlot,
    entitled_units: u64,
) -> Outcome<()> {
    accounts::validate_state_role_lengths(program_id, account, true, &[RESERVATION_ACCOUNT_BYTES])?;
    let reservation = super::decode_reservation_boxed(&account.data.borrow())?;
    match reservation.state {
        RESERVATION_STATE_ACTIVE => {
            super::clear_walk::validate_walk_reservation(
                program_id, account, epoch, page_index, slot, true,
            )?;
            let next = reservation.entitled(entitled_units)?;
            next.encode(&mut borrow_account_mut(account)?)?;
            Ok(())
        }
        RESERVATION_STATE_ENTITLED => {
            validate_entitled_touch(
                program_id,
                epoch,
                page_index,
                slot,
                account.key,
                &reservation,
            )?;
            reservation.requires_stamp(entitled_units)?;
            Ok(())
        }
        // RELEASED and CONSUMED are terminal; neither can take a new slice.
        _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
    }
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
    let expected_residue =
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

    /* The slice being entitled, routed on its shape.  A slice whose sell end
     * is the global virtual split has one real end and takes the virtual
     * route; a slice whose buy end is the global virtual merge is the one
     * direction the pot cannot fund and keeps the honest stub (the freeze
     * already refused any summary carrying `mu`, so this is defence in
     * depth); `Split` on the buy side or `Merge` on the sell side would have
     * refused at verification as `SliceNotExecutable`
     * (`relation_v1.rs:3784`, `:3806`) and is a state mismatch here. */
    let slice = {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        clearing::slice_at(&data, &feed, slice_index)?
    };
    let (buy_rank, sell_rank) = match (slice.buy_ref, slice.sell_ref) {
        (clearing::LegRef::Order(buy), clearing::LegRef::Order(sell)) => (buy, sell),
        (clearing::LegRef::Order(buy), clearing::LegRef::Split) => {
            return entitle_virtual_slice(
                program_id,
                accounts,
                tail,
                &epoch,
                &record,
                &VirtualSliceEnds {
                    slice,
                    slice_index,
                    rank: buy,
                    side: 0,
                    expected_residue,
                    feed: &feed,
                },
                pages,
            );
        }
        (clearing::LegRef::Merge, clearing::LegRef::Order(_)) => {
            return Err(Refusal::Adapter(ClutchError::NotYetImplemented));
        }
        _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
    };
    require(buy_rank != sell_rank, ClutchError::MismatchedState)?;

    let (buy, sell, live_order_count) = locate_pair(pages, &epoch, buy_rank, sell_rank)?;
    // T2-4's exact live-cardinality binding, recomputed from the
    // digest-verified set this instruction just walked.
    record.binds_epoch(&epoch, live_order_count)?;

    // One pass over the whole declared witness: the pair's own slices, and
    // each end's per-order Egg total.
    let coverage = {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        scan_witness(&data, &feed, buy_rank, sell_rank)?
    };
    /* The per-order entitlement totals, re-derived from the verified fills
     * rather than read off the witness: the witness says how the fills are
     * paired, the fill vector says how much each order is entitled to, and
     * requiring the two to agree is what makes a stamp unforgeable. */
    let (buy_entitled, sell_entitled) = {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        let buy_fill = clearing::fill_at(&data, &feed, buy_rank)?;
        let sell_fill = clearing::fill_at(&data, &feed, sell_rank)?;
        (
            entitled_total(&buy.slot, buy_fill)?,
            entitled_total(&sell.slot, sell_fill)?,
        )
    };
    require(
        buy_entitled != 0
            && sell_entitled != 0
            && coverage.buy_total == buy_entitled
            && coverage.sell_total == sell_entitled,
        ClutchError::MismatchedState,
    )?;

    /* Four shapes, one seam.  The exclusive portfolio pair keeps the atomic
     * full-pair route verbatim — moving it per-slice would narrow admission —
     * and everything else is the generalized per-slice route: a fragmented or
     * partially filled single pair, a mixed single/portfolio pair, and a
     * portfolio end that pairs with more than one counterparty. */
    match (buy.slot, sell.slot) {
        (OrderSlot::Portfolio(_), OrderSlot::Portfolio(_)) if coverage.exclusive() => {
            entitle_portfolio_pair(
                program_id,
                accounts,
                tail,
                &epoch,
                &record,
                slice_index,
                &buy,
                &sell,
                &coverage,
            )
        }
        _ => entitle_per_slice(
            program_id,
            accounts,
            tail,
            &epoch,
            &record,
            &PerSliceEnds {
                slice,
                slice_index,
                buy: &buy,
                sell: &sell,
                buy_entitled,
                sell_entitled,
                expected_residue,
                feed: &feed,
            },
        ),
    }
}

/// Everything the per-slice route knows about the slice it is entitling.
#[derive(Clone, Copy)]
struct PerSliceEnds<'a> {
    slice: clearing::PairingSlice,
    slice_index: u16,
    buy: &'a Located,
    sell: &'a Located,
    buy_entitled: u64,
    sell_entitled: u64,
    /// The pot's verified residue expectation for this epoch.
    expected_residue: u128,
    feed: &'a CandidateFeedHeader,
}

/// One end's shape check against the slice it is being entitled for.
///
/// A single-Egg end must sit on the slice's own outcome; a portfolio end must
/// carry a nonzero coefficient there, which is what makes the outcome one of
/// its legs.  Limits are re-checked where they are per-outcome — a portfolio's
/// limit is collateral per whole lot, which the relation verified against the
/// basket, not against one leg.
fn validate_slice_end(
    slot: &OrderSlot,
    side: u8,
    outcome: u8,
    price: u64,
    outcome_count: u8,
) -> Outcome<()> {
    match slot {
        OrderSlot::Single(order) => require(
            order.side == side
                && order.outcome == outcome
                && if side == 0 {
                    price <= order.limit
                } else {
                    price >= order.limit
                },
            ClutchError::MismatchedState,
        ),
        OrderSlot::Portfolio(order) => require(
            order.side == side
                && outcome < order.active_len
                && order.active_len <= outcome_count
                && order.coefficients[usize::from(outcome)] != 0,
            ClutchError::MismatchedState,
        ),
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        }
    }
}

/// The generalized per-slice half of `EntitleSlice`.
///
/// One witness slice, one receipt, and the per-order stamp on each end.  This
/// is the universal route: a fragmented or partially filled single-Egg pair, a
/// mixed single/portfolio pair, and a portfolio order that pairs with more
/// than one counterparty all arrive here, and a full one-to-one fill is simply
/// its one-slice special case.
/// One slice's conversion coordinates.
///
/// A `None` end is a **virtual** one — the epoch pot standing in for the
/// global split or merge.  It has no order, no reservation and no cumulative
/// ledger, so it converts nothing: its whole side of the slice is carried in
/// exact price units by `FinalPotAccount::pot_cash_price_units` and never
/// rounds.  Both rules below therefore skip it, which is why a virtual slice
/// can be entitled at prices no ordinary pair could convert.
struct ConvertibleEnds<'a> {
    buy_slot: Option<&'a OrderSlot>,
    sell_slot: Option<&'a OrderSlot>,
    buy_entitled: u64,
    sell_entitled: u64,
    price: u64,
    consideration_price_units: u128,
    expected_residue: u128,
}

/// Refuse any pair whose collateral conversion the consumption seam cannot
/// realize — before a receipt exists, so no unconsumable receipt is minted.
///
/// Two rules, and only two.
///
/// * A **portfolio** end spans several frozen prices, so its whole-order value
///   is not a function of its consumed units and the seam converts its slices
///   one at a time: those must be exact, as they always were.
/// * A **single-Egg** end lives on one outcome at one price, so the seam
///   converts its *cumulative* value and its slices need not be exact at all.
///   What it does need is that the frozen boundary's per-owner conversion be
///   the per-order conversion: `relation_v1.rs:2482-2497` sums an owner's
///   orders into `debit_units[slot]`/`credit_units[slot]` and converts once,
///   which coincides with converting each order once exactly when every
///   participating owner holds one filled order.  The relation recomputes that
///   owner count (`SummaryV1::distinct_participating_owners`), so requiring it
///   equal to the feed's filled-order count is a checked coincidence, never an
///   assumption.
#[inline(never)]
fn require_convertible_ends(
    feed_account: &AccountInfo,
    feed: &CandidateFeedHeader,
    record: &CandidateRecord,
    price_scale: u64,
    ends: ConvertibleEnds<'_>,
) -> Outcome<()> {
    let scale = u128::from(price_scale);
    let buy_single = matches!(ends.buy_slot, Some(OrderSlot::Single(_)));
    let sell_single = matches!(ends.sell_slot, Some(OrderSlot::Single(_)));
    let buy_portfolio = ends.buy_slot.is_some() && !buy_single;
    let sell_portfolio = ends.sell_slot.is_some() && !sell_single;
    if buy_portfolio || sell_portfolio {
        require(
            ends.consideration_price_units.is_multiple_of(scale),
            ClutchError::NotYetImplemented,
        )?;
    }
    let order_value = |units: u64| -> Outcome<u128> {
        u128::from(units)
            .checked_mul(u128::from(ends.price))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
    };
    let buy_inexact = buy_single && !order_value(ends.buy_entitled)?.is_multiple_of(scale);
    let sell_inexact = sell_single && !order_value(ends.sell_entitled)?.is_multiple_of(scale);
    if !buy_inexact && !sell_inexact && ends.expected_residue == 0 {
        return Ok(());
    }
    let filled = {
        let data = feed_account.data.borrow();
        filled_order_count(&data, feed)?
    };
    require(
        record.distinct_owners == filled,
        ClutchError::NotYetImplemented,
    )
}

#[inline(never)]
fn entitle_per_slice<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    tail: &[AccountInfo<'a>],
    epoch: &EpochAccount,
    record: &CandidateRecord,
    ends: &PerSliceEnds<'_>,
) -> Outcome<()> {
    let PerSliceEnds {
        slice,
        slice_index,
        buy,
        sell,
        buy_entitled,
        sell_entitled,
        expected_residue,
        feed,
    } = *ends;
    // With the optional trailing funding ledger, the tail grows by one.
    require(
        tail.len() == ENTITLE_SINGLE_TAIL_ACCOUNT_COUNT
            || tail.len() == ENTITLE_SINGLE_TAIL_ACCOUNT_COUNT + 1,
        ClutchError::AccountCount,
    )?;
    require(
        slice.quantity != 0
            && slice.outcome < epoch.outcome_count
            && buy.slot.owner() != sell.slot.owner(),
        ClutchError::MismatchedState,
    )?;
    let price = record.prices[usize::from(slice.outcome)];
    validate_slice_end(&buy.slot, 0, slice.outcome, price, epoch.outcome_count)?;
    validate_slice_end(&sell.slot, 1, slice.outcome, price, epoch.outcome_count)?;
    let consideration_price_units = u128::from(slice.quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(epoch.price_scale != 0, ClutchError::MismatchedState)?;
    require_convertible_ends(
        &accounts[IX_ENTITLE_FEED],
        feed,
        record,
        epoch.price_scale,
        ConvertibleEnds {
            buy_slot: Some(&buy.slot),
            sell_slot: Some(&sell.slot),
            buy_entitled,
            sell_entitled,
            price,
            consideration_price_units,
            expected_residue,
        },
    )?;

    // State writes before the creation CPI (the terminal.rs ordering rule).
    stamp_reservation(
        program_id,
        &tail[0],
        epoch,
        buy.page_index,
        &buy.slot,
        buy_entitled,
    )?;
    stamp_reservation(
        program_id,
        &tail[1],
        epoch,
        sell.page_index,
        &sell.slot,
        sell_entitled,
    )?;

    let receipt_prior = tail[2].lamports();
    let receipt = SettlementReceiptAccount {
        epoch: epoch.epoch,
        market: epoch.market,
        candidate: record.candidate,
        buy_order_id: buy.slot.order_id(),
        sell_order_id: sell.slot.order_id(),
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
    let rent = read_rent(&accounts[IX_ENTITLE_RENT])?;
    create_receipt(
        program_id,
        &accounts[IX_ENTITLE_PAYER],
        &tail[2],
        &accounts[IX_ENTITLE_SYSTEM],
        &rent,
        epoch.epoch,
        record.candidate,
        &receipt,
    )?;
    if tail.len() == ENTITLE_SINGLE_TAIL_ACCOUNT_COUNT + 1 {
        super::terminal_closure::create_funding_ledger(
            program_id,
            &accounts[IX_ENTITLE_PAYER],
            &tail[3],
            &accounts[IX_ENTITLE_SYSTEM],
            &rent,
            tail[2].key,
            clearing::FUNDING_COVERS_RECEIPT,
            super::terminal_closure::creation_shortfall(
                rent.minimum_balance(account_len::SETTLEMENT_RECEIPT)?,
                receipt_prior,
            ),
            receipt_prior,
        )?;
    }
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* The virtual route: one real end, the epoch pot on the other               */
/* ------------------------------------------------------------------------ */

/// Everything the virtual route knows about the slice it is entitling.
#[derive(Clone, Copy)]
struct VirtualSliceEnds<'a> {
    slice: clearing::PairingSlice,
    slice_index: u16,
    /// Live rank of the slice's one real end.
    rank: u8,
    /// Which side that end is: zero a buy served by the global virtual split.
    side: u8,
    /// The pot's verified residue expectation for this epoch.
    expected_residue: u128,
    feed: &'a CandidateFeedHeader,
}

/// Locate one live order at its rank, and the frozen set's live count.
///
/// The same complete, digest-verified page walk [`locate_pair`] performs —
/// asking for one rank twice is what makes it one walk rather than a second
/// copy of it, and the returned count is the exact live cardinality
/// `CandidateRecord::binds_epoch` is re-checked against.
#[inline(never)]
fn locate_one(pages: &[AccountInfo], epoch: &EpochAccount, rank: u8) -> Outcome<(Located, u16)> {
    let (located, _, live) = locate_pair(pages, epoch, rank, rank)?;
    Ok((located, live))
}

/// Total the Egg atoms the whole declared witness moves through one end.
///
/// The one-ended twin of [`scan_witness`], and it discharges the same two
/// obligations: the per-order total the entitlement stamp is checked against,
/// and the statement that this rank never appears on its opposite side — an
/// arrangement the relation refuses as `SliceNotExecutable`, restated here so
/// the stamp cannot be widened by a slice that would not have verified.
#[inline(never)]
fn scan_end_total(
    feed_data: &[u8],
    feed: &CandidateFeedHeader,
    rank: u8,
    side: u8,
) -> Outcome<u64> {
    let declared = feed
        .declared_slices()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let reference = clearing::LegRef::Order(rank);
    let mut total = 0u64;
    let mut index = 0u16;
    while index < declared {
        let slice = clearing::slice_at(feed_data, feed, index)?;
        let (own, opposite) = if side == 0 {
            (slice.buy_ref, slice.sell_ref)
        } else {
            (slice.sell_ref, slice.buy_ref)
        };
        require(opposite != reference, ClutchError::MismatchedState)?;
        if own == reference {
            total = total
                .checked_add(slice.quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        index += 1;
    }
    Ok(total)
}

/// Entitle one virtual slice: one receipt whose counterparty is the epoch
/// pot, and the per-order stamp on its single real end.
///
/// The receipt's virtual side carries `Hash32::ZERO` for its order id, which
/// is exactly what the frozen `SettlementReceiptAccount` codec requires of a
/// `RECEIPT_LEG_SPLIT` row — the layout has always had the shape; this is the
/// first seam that mints one.
#[inline(never)]
fn entitle_virtual_slice<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    tail: &[AccountInfo<'a>],
    epoch: &EpochAccount,
    record: &CandidateRecord,
    ends: &VirtualSliceEnds<'_>,
    pages: &[AccountInfo<'a>],
) -> Outcome<()> {
    let VirtualSliceEnds {
        slice,
        slice_index,
        rank,
        side,
        expected_residue,
        feed,
    } = *ends;
    // With the optional trailing funding ledger, the tail grows by one.
    require(
        tail.len() == ENTITLE_VIRTUAL_TAIL_ACCOUNT_COUNT
            || tail.len() == ENTITLE_VIRTUAL_TAIL_ACCOUNT_COUNT + 1,
        ClutchError::AccountCount,
    )?;
    /* A slice may reference the global virtual split only when the selected
     * candidate carries one.  The freeze already cross-checked the record's
     * churn against the streamed verdict, so this is the one place the
     * witness's shape is bound to the priced quantity. */
    require(
        side == 0 && record.virtual_split != 0 && record.virtual_merge == 0,
        ClutchError::MismatchedState,
    )?;
    require(
        slice.quantity != 0 && slice.outcome < epoch.outcome_count,
        ClutchError::MismatchedState,
    )?;

    let (located, live_order_count) = locate_one(pages, epoch, rank)?;
    // T2-4's exact live-cardinality binding, recomputed from the
    // digest-verified set this instruction just walked.
    record.binds_epoch(epoch, live_order_count)?;

    /* The per-order entitlement total, re-derived from the verified fill
     * rather than read off the witness. */
    let (entitled, witness_total) = {
        let data = accounts[IX_ENTITLE_FEED].data.borrow();
        let fill = clearing::fill_at(&data, feed, rank)?;
        (
            entitled_total(&located.slot, fill)?,
            scan_end_total(&data, feed, rank, side)?,
        )
    };
    require(
        entitled != 0 && witness_total == entitled,
        ClutchError::MismatchedState,
    )?;

    let price = record.prices[usize::from(slice.outcome)];
    validate_slice_end(
        &located.slot,
        side,
        slice.outcome,
        price,
        epoch.outcome_count,
    )?;
    let consideration_price_units = u128::from(slice.quantity)
        .checked_mul(u128::from(price))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(epoch.price_scale != 0, ClutchError::MismatchedState)?;
    require_convertible_ends(
        &accounts[IX_ENTITLE_FEED],
        feed,
        record,
        epoch.price_scale,
        ConvertibleEnds {
            buy_slot: Some(&located.slot),
            sell_slot: None,
            buy_entitled: entitled,
            sell_entitled: 0,
            price,
            consideration_price_units,
            expected_residue,
        },
    )?;

    // State writes before the creation CPI (the terminal.rs ordering rule).
    stamp_reservation(
        program_id,
        &tail[0],
        epoch,
        located.page_index,
        &located.slot,
        entitled,
    )?;

    let receipt_prior = tail[1].lamports();
    let receipt = SettlementReceiptAccount {
        epoch: epoch.epoch,
        market: epoch.market,
        candidate: record.candidate,
        buy_order_id: located.slot.order_id(),
        sell_order_id: Hash32::ZERO,
        consideration_price_units,
        quantity: slice.quantity,
        settled_quantity: 0,
        price,
        sequence: u64::from(slice_index)
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        slice_index,
        outcome: slice.outcome,
        leg_kind: RECEIPT_LEG_SPLIT,
        consumed_flags: 0,
        stored_bump: 0,
        flags: 0,
    };
    let rent = read_rent(&accounts[IX_ENTITLE_RENT])?;
    create_receipt(
        program_id,
        &accounts[IX_ENTITLE_PAYER],
        &tail[1],
        &accounts[IX_ENTITLE_SYSTEM],
        &rent,
        epoch.epoch,
        record.candidate,
        &receipt,
    )?;
    if tail.len() == ENTITLE_VIRTUAL_TAIL_ACCOUNT_COUNT + 1 {
        super::terminal_closure::create_funding_ledger(
            program_id,
            &accounts[IX_ENTITLE_PAYER],
            &tail[2],
            &accounts[IX_ENTITLE_SYSTEM],
            &rent,
            tail[1].key,
            clearing::FUNDING_COVERS_RECEIPT,
            super::terminal_closure::creation_shortfall(
                rent.minimum_balance(account_len::SETTLEMENT_RECEIPT)?,
                receipt_prior,
            ),
            receipt_prior,
        )?;
    }
    Ok(())
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
    let buy_funding = PortfolioFundingV1::for_order(market, terms, price_scale, buy, 0)
        .map_err(portfolio_fault)?;
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
    // With the optional funding ledgers, one per receipt follows the
    // receipts in the same slice order.
    let ledgered = tail.len() == ENTITLE_PAIR_TAIL_FIXED_ACCOUNT_COUNT + (2 * coverage.stored);
    require(
        tail.len() == ENTITLE_PAIR_TAIL_FIXED_ACCOUNT_COUNT + coverage.stored || ledgered,
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
        while at < coverage.stored {
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
    let consideration =
        exact_portfolio_value_price_units(&funding.internal, &record.prices, terms.outcome_count)
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
    let receipt_minimum = rent.minimum_balance(account_len::SETTLEMENT_RECEIPT)?;
    let mut at = 0usize;
    while at < coverage.stored {
        let target = &tail[ENTITLE_PAIR_TAIL_FIXED_ACCOUNT_COUNT + at];
        let prior = target.lamports();
        create_receipt(
            program_id,
            &accounts[IX_ENTITLE_PAYER],
            target,
            &accounts[IX_ENTITLE_SYSTEM],
            &rent,
            epoch.epoch,
            record.candidate,
            &prepared[at],
        )?;
        if ledgered {
            super::terminal_closure::create_funding_ledger(
                program_id,
                &accounts[IX_ENTITLE_PAYER],
                &tail[ENTITLE_PAIR_TAIL_FIXED_ACCOUNT_COUNT + coverage.stored + at],
                &accounts[IX_ENTITLE_SYSTEM],
                &rent,
                target.key,
                clearing::FUNDING_COVERS_RECEIPT,
                super::terminal_closure::creation_shortfall(receipt_minimum, prior),
                prior,
            )?;
        }
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
    static ZERO: [SettlementReceiptAccount; MAX_PAIR_RECEIPTS] = [ZERO_RECEIPT; MAX_PAIR_RECEIPTS];
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
            && reservation.remaining_internal == reservation.initial_internal
            /* Unstamped, and therefore atomically entitled: a reservation
             * carrying a per-order ledger has slices with other
             * counterparties outstanding and may only be consumed one slice
             * at a time, or this seam would hand away an envelope other
             * receipts still have a claim on. */
            && reservation.entitled_units == 0
            && reservation.consumed_units == 0,
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
    staged.seller_reservation.encode(&mut borrow_account_mut(
        &accounts[IX_PAIR_SELL_RESERVATION],
    )?)?;
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
