//! The joint clearing's solver: off chain, and the chain checks its certificate.
//!
//! `MECHANISM_JOINT_CLEARING_2026_09_04.md` §1.6 states the algorithm for a
//! single-outcome book and §1.2 the certificate the chain verifies; decision
//! 0032 §2b fixes the tie-break. This module produces, for one closed batch's
//! book, the ONE clearing the verifier admits: the greedy fills, the
//! lexicographic-minimum price vector over the box those fills induce
//! (`JointClearingV1.lexMinFrom`), and pro-rata rationing among the orders
//! exactly marginal at the price with the remainder in ascending order-id
//! order. It then lays the clearing out as the Candidate and Page records the
//! streamed verifier consumes, one row per LIVE order -- zero-lot rows
//! included, because the certificate must enumerate every order the batch
//! holds (`RuntimeVerifyErrorV2::OrderOmitted`).
//!
//! Nothing here is an authority. A solver that submits anything else pays the
//! work escrow to be refused by name;
//! `tests/general_joint_clearing_v1_differential.rs` streams every clearing
//! this module produces through the chain's own verifier.

use dclutch_trading::general::collection_v1::{
    GeneralBatchV2, GeneralClearingFixedV1, GeneralClearingMoveV1, GeneralCollectionErrorV1,
    general_batch_len_v2,
};
use dclutch_trading::general::runtime_verify::{
    OrderSideV2, RuntimeCompleteSetMoveV2, RuntimeVerifyErrorV2, runtime_identity_precedes_v2,
    runtime_verified_balance_v2, runtime_verified_residual_v2,
};
use dclutch_trading::general::runtime_width::{
    CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
    RuntimeWidthErrorV2, VerifiedCandidateV2, candidate_len, execution_len, page_len,
};

/// One live order of a closed batch, as the solver reads it off the chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BookOrderV1 {
    /// Immutable order content identity (`general_order_identity_v2`).
    pub order_id: [u8; 32],
    /// Maker identity.
    pub owner_id: [u8; 32],
    /// Owner-scoped nonce.
    pub nonce: u64,
    /// Which way the claims flow.
    pub side: OrderSideV2,
    /// The single outcome of the order's interval (cohort-18 admits `lo == hi`).
    pub outcome: u32,
    /// Claims one lot moves.
    pub claims_per_lot: u64,
    /// Candidate-wide maximum fill, in lots.
    pub max_lots: u64,
    /// Buyer's cap in quote atoms per lot; zero for a sell.
    pub max_quote_debit_per_lot: u64,
    /// Seller's floor in quote atoms per lot; zero for a buy or a floorless sell.
    pub min_quote_credit_per_lot: u64,
}

/// A closed batch's book.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookV1 {
    /// Runtime outcome width.
    pub outcome_count: u32,
    /// The batch's price scale: prices are per claim in these units.
    pub price_scale: u64,
    /// Every LIVE order of the batch, in any order.
    pub orders: Vec<BookOrderV1>,
}

/// One clearing: the certificate's data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClearingV1 {
    /// One price per outcome, summing to the scale.
    pub prices: Vec<u64>,
    /// One fill per live order, in the verifier's identity order; zero lots
    /// is a row too.
    pub fills: Vec<FillV1>,
    /// Signed complete-set count in claims: positive mints, negative merges.
    pub sets: i128,
}

/// One order's fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillV1 {
    /// The order.
    pub order: BookOrderV1,
    /// Lots filled, at most `order.max_lots`.
    pub lots: u64,
}

/// Why a book cannot be cleared by this solver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverErrorV1 {
    /// An order names an outcome outside the width, moves no claims, or has
    /// no lots.
    Shape,
    /// The width or scale is zero, or the book is empty.
    Book,
    /// Two orders share one identity.
    DuplicateOrder,
    /// Checked arithmetic overflowed.
    Arithmetic,
    /// The greedy left a box the scale cannot be spread over -- a defect in
    /// the greedy, not in the book; reported rather than hidden.
    Infeasible,
}

/// `limit × scale / magnitude` rounded down (a buyer's cap in price units).
fn cap_price(order: &BookOrderV1, scale: u64) -> Result<u64, SolverErrorV1> {
    let product = u128::from(order.max_quote_debit_per_lot) * u128::from(scale);
    let quotient = product / u128::from(order.claims_per_lot);
    Ok(u64::try_from(quotient.min(u128::from(scale))).map_err(|_| SolverErrorV1::Arithmetic)?)
}

/// `floor × scale / magnitude` rounded up (a seller's floor in price units).
fn floor_price(order: &BookOrderV1, scale: u64) -> Result<u64, SolverErrorV1> {
    let magnitude = u128::from(order.claims_per_lot);
    let product = u128::from(order.min_quote_credit_per_lot) * u128::from(scale);
    let quotient = (product + magnitude - 1) / magnitude;
    Ok(
        u64::try_from(quotient.min(u128::from(scale) + 1))
            .map_err(|_| SolverErrorV1::Arithmetic)?,
    )
}

/// The limit in price units, on the side the order trades.
fn limit_price(order: &BookOrderV1, scale: u64) -> Result<u64, SolverErrorV1> {
    match order.side {
        OrderSideV2::Buy => cap_price(order, scale),
        OrderSideV2::Sell => floor_price(order, scale),
    }
}

struct Working {
    order: BookOrderV1,
    limit: u64,
    /// Claims filled so far.
    claims: u128,
}

impl Working {
    fn capacity(&self) -> u128 {
        u128::from(self.order.max_lots) * u128::from(self.order.claims_per_lot)
    }

    fn remaining(&self) -> u128 {
        self.capacity() - self.claims
    }
}

/// Clear one book: the note's §1.6 greedy, then the minimal price vector,
/// then pro-rata rationing at the margin.
pub fn clear_book_v1(book: &BookV1) -> Result<ClearingV1, SolverErrorV1> {
    let scale = book.price_scale;
    let count = usize::try_from(book.outcome_count).map_err(|_| SolverErrorV1::Arithmetic)?;
    if count == 0 || scale == 0 || book.orders.is_empty() {
        return Err(SolverErrorV1::Book);
    }
    for (index, order) in book.orders.iter().enumerate() {
        if order.outcome >= book.outcome_count || order.claims_per_lot == 0 || order.max_lots == 0 {
            return Err(SolverErrorV1::Shape);
        }
        if book.orders[..index]
            .iter()
            .any(|other| other.order_id == order.order_id)
        {
            return Err(SolverErrorV1::DuplicateOrder);
        }
    }
    let mut working: Vec<Working> = book
        .orders
        .iter()
        .map(|order| {
            Ok(Working {
                order: *order,
                limit: limit_price(order, scale)?,
                claims: 0,
            })
        })
        .collect::<Result<_, SolverErrorV1>>()?;

    // Per outcome: buys by limit descending, sells by limit ascending, ties by
    // ascending identity so the walk is a function of the book.
    let by_id = |left: &Working, right: &Working| {
        if runtime_identity_precedes_v2(&left.order.order_id, &right.order.order_id) {
            core::cmp::Ordering::Less
        } else if left.order.order_id == right.order.order_id {
            core::cmp::Ordering::Equal
        } else {
            core::cmp::Ordering::Greater
        }
    };
    let mut buys: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut sells: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (slot, item) in working.iter().enumerate() {
        let outcome = usize::try_from(item.order.outcome).map_err(|_| SolverErrorV1::Arithmetic)?;
        match item.order.side {
            OrderSideV2::Buy => buys[outcome].push(slot),
            OrderSideV2::Sell => sells[outcome].push(slot),
        }
    }
    for outcome in 0..count {
        buys[outcome].sort_by(|left, right| {
            working[*right]
                .limit
                .cmp(&working[*left].limit)
                .then_with(|| by_id(&working[*left], &working[*right]))
        });
        sells[outcome].sort_by(|left, right| {
            working[*left]
                .limit
                .cmp(&working[*right].limit)
                .then_with(|| by_id(&working[*left], &working[*right]))
        });
    }

    // 1. Transfers: cross buys and sells at one outcome while the buy's cap
    //    is at or above the sell's floor.
    for outcome in 0..count {
        let (mut b, mut s) = (0, 0);
        while b < buys[outcome].len() && s < sells[outcome].len() {
            let (buy, sell) = (buys[outcome][b], sells[outcome][s]);
            if working[buy].limit < working[sell].limit {
                break;
            }
            let step = working[buy].remaining().min(working[sell].remaining());
            working[buy].claims += step;
            working[sell].claims += step;
            if working[buy].remaining() == 0 {
                b += 1;
            }
            if working[sell].remaining() == 0 {
                s += 1;
            }
        }
    }

    // 2. Mint while the marginal unmatched buyers across the outcomes together
    //    pay at least one set; merge while the marginal unmatched sellers
    //    together ask at most one set. Each step consumes the smallest
    //    marginal remainder so every step is a function of the book.
    let marginal =
        |working: &Vec<Working>, side: &Vec<Vec<usize>>, outcome: usize| -> Option<usize> {
            side[outcome]
                .iter()
                .copied()
                .find(|slot| working[*slot].remaining() > 0)
        };
    let mut sets: i128 = 0;
    loop {
        let mut total: u128 = 0;
        let mut step: u128 = u128::MAX;
        let mut chosen: Vec<Option<usize>> = vec![None; count];
        for outcome in 0..count {
            if let Some(slot) = marginal(&working, &buys, outcome) {
                total += u128::from(working[slot].limit);
                step = step.min(working[slot].remaining());
                chosen[outcome] = Some(slot);
            }
        }
        if step == u128::MAX || total < u128::from(scale) {
            break;
        }
        for slot in chosen.into_iter().flatten() {
            working[slot].claims += step;
        }
        sets += i128::try_from(step).map_err(|_| SolverErrorV1::Arithmetic)?;
    }
    if sets == 0 {
        loop {
            let mut total: u128 = 0;
            let mut step: u128 = u128::MAX;
            let mut chosen: Vec<usize> = Vec::with_capacity(count);
            for outcome in 0..count {
                match marginal(&working, &sells, outcome) {
                    Some(slot) => {
                        total += u128::from(working[slot].limit);
                        step = step.min(working[slot].remaining());
                        chosen.push(slot);
                    }
                    None => {
                        step = u128::MAX;
                        break;
                    }
                }
            }
            if step == u128::MAX || chosen.len() != count || total > u128::from(scale) {
                break;
            }
            for slot in chosen {
                working[slot].claims += step;
            }
            sets -= i128::try_from(step).map_err(|_| SolverErrorV1::Arithmetic)?;
        }
    }

    // 3. The box each fill status induces, the residual outcomes pinned to
    //    zero, and the greedy lexicographic minimum on the simplex.
    let mut net: Vec<i128> = vec![0; count];
    for item in &working {
        let outcome = usize::try_from(item.order.outcome).map_err(|_| SolverErrorV1::Arithmetic)?;
        let claims = i128::try_from(item.claims).map_err(|_| SolverErrorV1::Arithmetic)?;
        match item.order.side {
            OrderSideV2::Buy => net[outcome] += claims,
            OrderSideV2::Sell => net[outcome] -= claims,
        }
    }
    // `M` is the verifier's own derivation -- the greatest net claim flow over
    // the outcomes (`runtime_verify::signed_sets`) -- so the solver may not
    // carry a second one. The mint walk's chosen set only ever shrinks (an
    // outcome leaves it when its marginal order is exhausted and never
    // returns), so the outcome chosen at the last step was chosen at every
    // step and its net IS the accumulated total. A book where that fails is a
    // defect in the walk, reported rather than papered over.
    let derived = *net.iter().max().ok_or(SolverErrorV1::Book)?;
    if derived != sets {
        return Err(SolverErrorV1::Infeasible);
    }
    let mut lo: Vec<u64> = vec![0; count];
    let mut hi: Vec<u64> = (0..count)
        .map(|outcome| if net[outcome] < sets { 0 } else { scale })
        .collect();
    for item in &working {
        let outcome = usize::try_from(item.order.outcome).map_err(|_| SolverErrorV1::Arithmetic)?;
        let filled = item.claims > 0;
        let short = item.remaining() > 0;
        match item.order.side {
            OrderSideV2::Buy => {
                if filled {
                    hi[outcome] = hi[outcome].min(item.limit);
                }
                if short {
                    lo[outcome] = lo[outcome].max(item.limit);
                }
            }
            OrderSideV2::Sell => {
                if filled {
                    lo[outcome] = lo[outcome].max(item.limit);
                }
                if short {
                    hi[outcome] = hi[outcome].min(item.limit);
                }
            }
        }
    }
    let prices = lex_min_prices(scale, &lo, &hi)?;

    // 4. Pro-rata at the margin: among orders on one outcome and side whose
    //    limit equals the price and which the greedy filled in identity
    //    order, spread the same total by capacity with the remainder in
    //    ascending identity.
    for outcome in 0..count {
        for side in [&buys, &sells] {
            let group: Vec<usize> = side[outcome]
                .iter()
                .copied()
                .filter(|slot| working[*slot].limit == prices[outcome])
                .collect();
            if group.len() < 2 {
                continue;
            }
            let total: u128 = group.iter().map(|slot| working[*slot].claims).sum();
            let capacity: u128 = group.iter().map(|slot| working[*slot].capacity()).sum();
            if total == 0 || total == capacity {
                continue;
            }
            let mut assigned: u128 = 0;
            let mut ordered = group.clone();
            ordered.sort_by(|left, right| by_id(&working[*left], &working[*right]));
            for slot in &ordered {
                let share = total * working[*slot].capacity() / capacity;
                let lots = u128::from(working[*slot].order.claims_per_lot);
                let share = share / lots * lots;
                working[*slot].claims = share;
                assigned += share;
            }
            let mut remainder = total - assigned;
            for slot in &ordered {
                let lots = u128::from(working[*slot].order.claims_per_lot);
                while remainder >= lots && working[*slot].remaining() >= lots {
                    working[*slot].claims += lots;
                    remainder -= lots;
                }
            }
        }
    }

    working.sort_by(by_id);
    let fills = working
        .iter()
        .map(|item| {
            let lots = item.claims / u128::from(item.order.claims_per_lot);
            Ok(FillV1 {
                order: item.order,
                lots: u64::try_from(lots).map_err(|_| SolverErrorV1::Arithmetic)?,
            })
        })
        .collect::<Result<Vec<_>, SolverErrorV1>>()?;
    Ok(ClearingV1 {
        prices,
        fills,
        sets,
    })
}

/// The greedy lexicographic minimum over `{p | lo ≤ p ≤ hi, Σ p = scale}`:
/// `JointClearingV1.lexMinFrom`, with the infeasible case reported.
pub fn lex_min_prices(scale: u64, lo: &[u64], hi: &[u64]) -> Result<Vec<u64>, SolverErrorV1> {
    if lo.len() != hi.len() {
        return Err(SolverErrorV1::Book);
    }
    let mut suffix: u128 = hi.iter().map(|value| u128::from(*value)).sum();
    let mut remaining = u128::from(scale);
    let mut prices = Vec::with_capacity(lo.len());
    for (floor, ceiling) in lo.iter().zip(hi.iter()) {
        suffix -= u128::from(*ceiling);
        let forced = remaining.saturating_sub(suffix);
        let price = u128::from(*ceiling).min(u128::from(*floor).max(forced));
        if price < u128::from(*floor) || price > remaining {
            return Err(SolverErrorV1::Infeasible);
        }
        remaining -= price;
        prices.push(u64::try_from(price).map_err(|_| SolverErrorV1::Arithmetic)?);
    }
    if remaining != 0 {
        return Err(SolverErrorV1::Infeasible);
    }
    Ok(prices)
}

/// The Candidate and its Pages for one clearing, in the verifier's wire.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateRecordsV1 {
    /// The `136 + 8N`-byte Candidate record.
    pub candidate: Vec<u8>,
    /// The Pages, in coordinate order, each holding at most `rows_per_page` rows.
    pub pages: Vec<Vec<u8>>,
    /// The content identity the candidate carries.
    pub candidate_id: [u8; 32],
}

/// Lay one clearing out as the records the streamed verifier consumes.
///
/// One Execution row per live order in identity order, zero-lot rows
/// included, so the terminal row's completeness conjunct sees every order;
/// `live_order_count` is the book's size, which `SubmitCandidate` holds to
/// the batch. `page_revision(page_index)` is the revision each Page is pinned
/// at by the authenticated state that admits it.
#[allow(clippy::too_many_arguments)]
pub fn build_candidate_records_v1(
    book: &BookV1,
    clearing: &ClearingV1,
    candidate_id: [u8; 32],
    product_id: [u8; 32],
    batch_id: [u8; 32],
    candidate_coordinate: u32,
    rows_per_page: u32,
    mut page_revision: impl FnMut(u32) -> u64,
) -> Result<CandidateRecordsV1, SolverErrorV1> {
    let width = book.outcome_count;
    let count = usize::try_from(width).map_err(|_| SolverErrorV1::Arithmetic)?;
    let rows_per_page =
        usize::try_from(rows_per_page.max(1)).map_err(|_| SolverErrorV1::Arithmetic)?;
    let live_order_count =
        u32::try_from(clearing.fills.len()).map_err(|_| SolverErrorV1::Arithmetic)?;
    let page_count = u32::try_from(clearing.fills.len().div_ceil(rows_per_page))
        .map_err(|_| SolverErrorV1::Arithmetic)?;
    let mut candidate = vec![0; candidate_len(width).map_err(|_| SolverErrorV1::Book)?];
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            outcome_count: width,
            page_count,
            candidate_coordinate,
            price_scale: book.price_scale,
            candidate_id,
            product_id,
            batch_id,
            live_order_count,
        },
        &clearing.prices,
        &mut candidate,
    )
    .map_err(|_| SolverErrorV1::Book)?;
    let mut pages = Vec::with_capacity(clearing.fills.len().div_ceil(rows_per_page));
    for (page_index, chunk) in clearing.fills.chunks(rows_per_page).enumerate() {
        let page_index = u32::try_from(page_index).map_err(|_| SolverErrorV1::Arithmetic)?;
        let mut rows: Vec<Vec<u8>> = Vec::with_capacity(chunk.len());
        for (row_index, fill) in chunk.iter().enumerate() {
            let outcome =
                usize::try_from(fill.order.outcome).map_err(|_| SolverErrorV1::Arithmetic)?;
            let mut receive = vec![0_u64; count];
            let mut deliver = vec![0_u64; count];
            match fill.order.side {
                OrderSideV2::Buy => receive[outcome] = fill.order.claims_per_lot,
                OrderSideV2::Sell => deliver[outcome] = fill.order.claims_per_lot,
            }
            let mut row = vec![0; execution_len(width).map_err(|_| SolverErrorV1::Book)?];
            ExecutionV2::encode_into(
                ExecutionHeaderV2 {
                    outcome_count: width,
                    page_coordinate: page_index + 1,
                    execution_coordinate: u32::try_from(row_index + 1)
                        .map_err(|_| SolverErrorV1::Arithmetic)?,
                    nonce: fill.order.nonce,
                    order_id: fill.order.order_id,
                    owner_id: fill.order.owner_id,
                    max_lots: fill.order.max_lots,
                    lots: fill.lots,
                },
                &receive,
                &deliver,
                &mut row,
            )
            .map_err(|_| SolverErrorV1::Shape)?;
            rows.push(row);
        }
        let row_refs: Vec<&[u8]> = rows.iter().map(Vec::as_slice).collect();
        let mut page = vec![
            0;
            page_len(
                width,
                u32::try_from(rows.len()).map_err(|_| SolverErrorV1::Arithmetic)?
            )
            .map_err(|_| SolverErrorV1::Book)?
        ];
        PageV2::encode_into(
            PageHeaderV2 {
                outcome_count: width,
                page_coordinate: page_index + 1,
                page_count,
                revision: page_revision(page_index),
                candidate_id,
            },
            &row_refs,
            &mut page,
        )
        .map_err(|_| SolverErrorV1::Book)?;
        pages.push(page);
    }
    Ok(CandidateRecordsV1 {
        candidate,
        pages,
        candidate_id,
    })
}

/// Why one clearing cannot be published onto its batch record.
///
/// Every variant carries the refusal its owner raised, because "the clearing
/// did not publish" is four different accusations: the batch is in the wrong
/// phase, the certificate is not canonical, the certificate is not this
/// batch's, or the certificate's own balance does not derive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishClearingErrorV1 {
    /// The batch record refused: not closed, already cleared, or the clearing
    /// disagrees with the batch's own counters.
    Batch(GeneralCollectionErrorV1),
    /// The verified-candidate certificate is not canonical.
    Certificate(RuntimeWidthErrorV2),
    /// The certificate's balance or residual could not be derived.
    Verify(RuntimeVerifyErrorV2),
    /// The certificate settles a DIFFERENT batch than the record given.
    Substitution,
}

/// Project one settled clearing onto the batch record it cleared.
///
/// This is the host's untrusted producer of the `ClearingPriceV1` tail: it
/// takes the closed batch record and the verified certificate the selection
/// froze, and returns the exact `296 + 16N` bytes a cleared batch holds. The
/// batch is the sole author of `live_order_count` (admitted less cancelled) and
/// [`GeneralBatchV2::clear`] holds the published copy to it; the certificate is
/// the sole author of the prices, the move and the residual.
///
/// Nothing here is an authority. It exists so a reader — the page, the cohort
/// runbook, a differential test — can say what a cleared batch WILL look like
/// from facts already on chain, and so the two record functions this family
/// added have a caller that exercises the whole tail law.
///
/// # Why this is off chain, precisely
///
/// Not for want of a writer: the batch record has no SEAT in the frame that
/// would do the writing. `Action::Close`'s AccountProfile carries two writable
/// local-state coordinates — the settlement cursor it closes and the terminal
/// record it creates — and the batch is neither. Its only readonly evidence is
/// `GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate`; `ClosedBatch` is
/// selected by `SubmitCandidate`, `VerifyCandidateRow`, `CloseCandidate` and
/// `Freeze`, none of which may write. So `GeneralBatchV2::clear` has no on-chain
/// caller, and the account the chain actually holds carries a VACANT clearing
/// tail — asserted by
/// `collection_v1::tests::the_batch_account_the_chain_writes_carries_a_vacant_clearing_tail`.
///
/// Seating it is a cohort-18 AccountProfile change and there are two shapes,
/// both real and neither this lane's to choose: a 66th account in `Action::Close`
/// (the family's heaviest frame, and the precedent exists — `CancelOrder`
/// already writes the order record at one coordinate and the batch's counters
/// at another), or a sixteenth `Action::ClearBatch` shaped like `CloseBatch`,
/// which already has the batch writable, taking the selected certificate as
/// readonly evidence. Decision 0032 §0.2 ruled against a sixteenth action FOR
/// THE STRAND and says nothing about publication, so the second shape is open.
///
/// One constraint binds either: the tail carries `live_order_count`, the
/// verified certificate does not, and the batch is that field's one author. A
/// frame holding only the certificate cannot write the tail honestly.
pub fn publish_clearing_v1(
    batch_bytes: &[u8],
    verified: &[u8],
    cleared_slot: u64,
) -> Result<Vec<u8>, PublishClearingErrorV1> {
    let mut batch = GeneralBatchV2::decode(batch_bytes).map_err(PublishClearingErrorV1::Batch)?;
    let certificate =
        VerifiedCandidateV2::decode(verified).map_err(PublishClearingErrorV1::Certificate)?;
    let header = certificate.header();
    if header.batch_id != batch.batch_id() {
        return Err(PublishClearingErrorV1::Substitution);
    }
    let balance = runtime_verified_balance_v2(verified).map_err(PublishClearingErrorV1::Verify)?;
    let width = batch.opening().outcome_count;
    let mut prices = Vec::new();
    let mut residual = Vec::new();
    for outcome in 0..width {
        prices.push(
            certificate
                .price(outcome)
                .map_err(PublishClearingErrorV1::Certificate)?,
        );
        residual.push(
            runtime_verified_residual_v2(verified, outcome)
                .map_err(PublishClearingErrorV1::Verify)?,
        );
    }
    batch
        .clear(GeneralClearingFixedV1 {
            cleared_candidate_id: header.candidate_id,
            cleared_slot,
            sets_move: match balance.complete_set_move {
                RuntimeCompleteSetMoveV2::None => GeneralClearingMoveV1::None,
                RuntimeCompleteSetMoveV2::Mint => GeneralClearingMoveV1::Mint,
                RuntimeCompleteSetMoveV2::Merge => GeneralClearingMoveV1::Merge,
            },
            sets_quantity: balance.complete_set_quantity,
            filled_lots: header.filled_lots,
            live_order_count: batch.live_order_count(),
        })
        .map_err(PublishClearingErrorV1::Batch)?;
    let mut output = vec![0; general_batch_len_v2(width).map_err(PublishClearingErrorV1::Batch)?];
    let cell = |values: &[u64], outcome: u32| -> Result<u64, GeneralCollectionErrorV1> {
        usize::try_from(outcome)
            .ok()
            .and_then(|index| values.get(index).copied())
            .ok_or(GeneralCollectionErrorV1::InvalidLength)
    };
    batch
        .encode_clearing_into(
            |outcome| cell(&prices, outcome),
            |outcome| cell(&residual, outcome),
            &mut output,
        )
        .map_err(PublishClearingErrorV1::Batch)?;
    Ok(output)
}
