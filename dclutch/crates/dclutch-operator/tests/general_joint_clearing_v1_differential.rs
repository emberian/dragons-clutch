//! The differential test the solver's module doc promises: every clearing
//! `clear_book_v1` produces is streamed through the chain's own verifier.
//!
//! The solver and the verifier are two authors of one fact -- which clearing a
//! book has -- and only one of them is authoritative. This file is the thing
//! that would catch them disagreeing: it runs the solver, lays the clearing out
//! as the Candidate and Page records with `build_candidate_records_v1`, and
//! feeds every row to `evaluate_runtime_consider_row_v2`, asserting the
//! terminal row ACCEPTS and that the certificate it emits carries the solver's
//! own price vector.
//!
//! Every refusal asserted below names its exact `RuntimeVerifyErrorV2` variant.

use dclutch_operator::general_joint_clearing_v1::{
    BookOrderV1, BookV1, ClearingV1, PublishClearingErrorV1, build_candidate_records_v1,
    clear_book_v1, lex_min_prices, publish_clearing_v1,
};
use dclutch_trading::general::collection_v1::{
    BatchStatusV1, GeneralBatchOpeningV1, GeneralBatchV2, GeneralClearingMoveV1,
    GeneralCollectionErrorV1, GeneralOrderHeaderV2, GeneralOrderPhaseV1, GeneralOrderStateV1,
    GeneralOrderV2, MakerFundingV1, general_batch_len_v2, general_batch_price_v2,
    general_batch_residual_v2, general_order_identity_v2, general_order_len_v2,
};
use dclutch_trading::general::runtime_verify::{
    AuthenticatedOrderTermsV2, OrderSideV2, RuntimeCompleteSetMoveV2, RuntimeConsiderRowBuffersV2,
    RuntimeConsiderRowViewV2, RuntimeVerifyErrorV2, evaluate_runtime_consider_row_v2,
    runtime_verified_balance_v2, runtime_verified_residual_v2, runtime_verifier_len_v2,
};
use dclutch_trading::general::runtime_width::{
    PageV2, VerifiedCandidateV2, verified_candidate_len,
};
use dclutch_trading::general_config::root::GeneralRootV2;

const PRODUCT: [u8; 32] = [2; 32];
const BATCH: [u8; 32] = [3; 32];
const CANDIDATE: [u8; 32] = [7; 32];
const PAGE_REVISION: u64 = 11;

/// One lot is one whole complete set's worth of claims, so a limit stated in
/// quote atoms per lot is the same number as a price in scale units per claim.
const CLAIMS_PER_LOT: u64 = 100;
const SCALE: u64 = 100;

fn identity(low: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = low;
    value
}

fn buy(low: u8, outcome: u32, max_lots: u64, cap: u64) -> BookOrderV1 {
    BookOrderV1 {
        order_id: identity(low),
        owner_id: identity(low.wrapping_add(0x40)),
        nonce: u64::from(low),
        side: OrderSideV2::Buy,
        outcome,
        claims_per_lot: CLAIMS_PER_LOT,
        max_lots,
        max_quote_debit_per_lot: cap,
        min_quote_credit_per_lot: 0,
    }
}

fn sell(low: u8, outcome: u32, max_lots: u64, floor: u64) -> BookOrderV1 {
    BookOrderV1 {
        order_id: identity(low),
        owner_id: identity(low.wrapping_add(0x40)),
        nonce: u64::from(low),
        side: OrderSideV2::Sell,
        outcome,
        claims_per_lot: CLAIMS_PER_LOT,
        max_lots,
        max_quote_debit_per_lot: 0,
        min_quote_credit_per_lot: floor,
    }
}

fn book(outcome_count: u32, orders: Vec<BookOrderV1>) -> BookV1 {
    BookV1 {
        outcome_count,
        price_scale: SCALE,
        orders,
    }
}

fn terms(order: &BookOrderV1) -> AuthenticatedOrderTermsV2 {
    AuthenticatedOrderTermsV2 {
        order_id: order.order_id,
        owner_id: order.owner_id,
        nonce: order.nonce,
        max_lots: order.max_lots,
        max_quote_debit_per_lot: order.max_quote_debit_per_lot,
        min_quote_credit_per_lot: order.min_quote_credit_per_lot,
        side: order.side,
        outcome_lo: order.outcome,
        outcome_hi: order.outcome,
        claims_per_lot: order.claims_per_lot,
    }
}

/// Stream one clearing's records through the chain verifier and return the
/// certificate the terminal row emitted.
///
/// `rows_per_page` is a parameter because the streaming boundary is where a
/// cursor is persisted and re-decoded, and a clearing that verifies as one page
/// but not as three would be a cursor defect the single-page shape hides.
fn stream(
    book: &BookV1,
    clearing: &ClearingV1,
    rows_per_page: u32,
) -> Result<Vec<u8>, RuntimeVerifyErrorV2> {
    stream_for(BATCH, book, clearing, rows_per_page)
}

/// [`stream`], against one named batch identity.
///
/// The certificate carries the batch it settles, and the publisher below holds
/// it to the record, so the end-to-end test needs a real `batch_id` rather than
/// the constant the arithmetic fixtures share.
fn stream_for(
    batch_id: [u8; 32],
    book: &BookV1,
    clearing: &ClearingV1,
    rows_per_page: u32,
) -> Result<Vec<u8>, RuntimeVerifyErrorV2> {
    let width = book.outcome_count;
    let records = build_candidate_records_v1(
        book,
        clearing,
        CANDIDATE,
        PRODUCT,
        batch_id,
        1,
        rows_per_page,
        |_| PAGE_REVISION,
    )
    .expect("the solver lays its own clearing out");
    let cursor_len = runtime_verifier_len_v2(width).expect("verifier width");
    let verified_len = verified_candidate_len(width).expect("certificate width");
    let zero_verified = vec![0_u8; verified_len];
    let mut cursor = vec![0_u8; cursor_len];
    let mut certificate: Option<Vec<u8>> = None;
    let mut revision = 0_u64;
    let max_orders = u32::try_from(clearing.fills.len()).expect("live orders") + 4;

    for (page_index, page_bytes) in records.pages.iter().enumerate() {
        let page_index = u32::try_from(page_index).expect("page index");
        let page = PageV2::decode(page_bytes).expect("the solver's own page decodes");
        let row_count = page.row_count();
        for row_index in 0..row_count {
            let execution = page.execution(row_index).expect("row");
            let order = clearing
                .fills
                .iter()
                .map(|fill| fill.order)
                .find(|order| order.order_id == execution.header().order_id)
                .expect("every row names a book order");
            let mut cursor_scratch = vec![0_u8; cursor_len];
            let mut cursor_output = vec![0xa5_u8; cursor_len];
            let mut verified_scratch = vec![0_u8; verified_len];
            let mut verified_output = zero_verified.clone();
            let summary = evaluate_runtime_consider_row_v2(
                RuntimeConsiderRowViewV2 {
                    candidate: &records.candidate,
                    page: page_bytes,
                    cursor_before: &cursor,
                    verified_before: &zero_verified,
                    authenticated_order: terms(&order),
                    expected_page_index: page_index,
                    expected_row_index: row_index,
                    expected_page_revision: PAGE_REVISION,
                    expected_revision: revision,
                    max_orders,
                },
                RuntimeConsiderRowBuffersV2 {
                    cursor_scratch: &mut cursor_scratch,
                    cursor_output: &mut cursor_output,
                    verified_scratch: &mut verified_scratch,
                    verified_output: &mut verified_output,
                },
            )?;
            cursor = cursor_output;
            revision = summary.revision;
            if summary.complete {
                certificate = Some(verified_output);
            }
        }
    }
    Ok(certificate.expect("the last row of the last page completes the candidate"))
}

/// The whole differential: solve, stream, and hold the certificate to the
/// solver's own answer.
fn agrees(book: &BookV1, rows_per_page: u32) -> Vec<u8> {
    let clearing = clear_book_v1(book).expect("the book clears");
    let bytes = match stream(book, &clearing, rows_per_page) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the chain refused the solver's own clearing: {error:?}"),
    };
    let certificate = VerifiedCandidateV2::decode(&bytes).expect("certificate decodes");
    for outcome in 0..book.outcome_count {
        let index = usize::try_from(outcome).expect("outcome index");
        assert_eq!(
            certificate.price(outcome).expect("certificate price"),
            clearing.prices[index],
            "the certificate's price at outcome {outcome} is not the solver's",
        );
    }
    let balance = runtime_verified_balance_v2(&bytes).expect("balance");
    let expected_move = match clearing.sets {
        0 => RuntimeCompleteSetMoveV2::None,
        sets if sets > 0 => RuntimeCompleteSetMoveV2::Mint,
        _ => RuntimeCompleteSetMoveV2::Merge,
    };
    assert_eq!(balance.complete_set_move, expected_move);
    assert_eq!(
        i128::from(balance.complete_set_quantity),
        clearing.sets.abs(),
    );
    // The strand law, read off the certificate the chain itself wrote: a
    // residual exists only where the clearing priced the outcome at zero.
    for outcome in 0..book.outcome_count {
        let index = usize::try_from(outcome).expect("outcome index");
        let residual = runtime_verified_residual_v2(&bytes, outcome).expect("residual");
        if clearing.prices[index] != 0 {
            assert_eq!(residual, 0, "a priced outcome carries a residual");
        }
    }
    bytes
}

fn transfer_book() -> BookV1 {
    book(2, vec![buy(1, 0, 3, 60), sell(2, 0, 2, 30)])
}

fn mint_book() -> BookV1 {
    book(2, vec![buy(1, 0, 2, 60), buy(2, 1, 3, 50)])
}

fn strand_book() -> BookV1 {
    book(3, vec![buy(1, 0, 2, 60), buy(2, 1, 2, 50)])
}

/// Two buyers, each capped at a whole set, each fully filled: the box is the
/// entire simplex and only the tie-break chooses.
fn open_box_book() -> BookV1 {
    book(2, vec![buy(1, 0, 2, SCALE), buy(2, 1, 2, SCALE)])
}

fn wide_book(width: u32) -> BookV1 {
    let mut orders = Vec::new();
    for outcome in 0..width {
        let low = u8::try_from(outcome).expect("width fits a byte") + 1;
        orders.push(buy(low, outcome, 2, SCALE / u64::from(width) + 1));
    }
    book(width, orders)
}

#[test]
fn a_crossing_book_clears_at_the_price_the_chain_derives() {
    let book = transfer_book();
    let clearing = clear_book_v1(&book).expect("clears");
    assert_eq!(clearing.prices, vec![60, 40]);
    assert_eq!(clearing.sets, 0);
    assert_eq!(clearing.fills.iter().map(|fill| fill.lots).sum::<u64>(), 4);
    agrees(&book, 8);
}

#[test]
fn a_book_whose_buyers_together_pay_a_set_mints_and_the_chain_agrees() {
    let book = mint_book();
    let clearing = clear_book_v1(&book).expect("clears");
    assert_eq!(clearing.prices, vec![50, 50]);
    assert_eq!(clearing.sets, i128::from(2 * CLAIMS_PER_LOT));
    agrees(&book, 8);
}

#[test]
fn an_outcome_nobody_bid_on_is_stranded_at_zero_and_the_chain_accepts_it() {
    let book = strand_book();
    let clearing = clear_book_v1(&book).expect("clears");
    assert_eq!(clearing.prices, vec![50, 50, 0]);
    assert_eq!(clearing.sets, i128::from(2 * CLAIMS_PER_LOT));
    let bytes = agrees(&book, 8);
    assert_eq!(
        runtime_verified_residual_v2(&bytes, 2).expect("residual"),
        2 * CLAIMS_PER_LOT,
        "the outcome nobody bid on holds the whole minted supply",
    );
}

#[test]
fn the_chain_accepts_the_same_clearing_streamed_one_row_per_page() {
    // The cursor is persisted and re-decoded at every page boundary, so a
    // clearing that verifies as one page and not as N is a cursor defect.
    agrees(&transfer_book(), 1);
    agrees(&mint_book(), 1);
    agrees(&strand_book(), 1);
}

#[test]
fn a_thirteen_outcome_book_clears_and_verifies() {
    let book = wide_book(13);
    let clearing = clear_book_v1(&book).expect("clears");
    assert_eq!(clearing.prices.iter().sum::<u64>(), SCALE);
    agrees(&book, 13);
}

#[test]
fn the_lex_min_greedy_is_the_leans_own_witness() {
    // `JointClearingV1.lexMin 100 [0,0] [60,60] = [40,60]`.
    assert_eq!(
        lex_min_prices(100, &[0, 0], &[60, 60]).expect("feasible"),
        vec![40, 60],
    );
    // A box the scale cannot be spread over is reported, never rounded away.
    assert_eq!(
        lex_min_prices(100, &[0, 0], &[10, 10]),
        Err(dclutch_operator::general_joint_clearing_v1::SolverErrorV1::Infeasible),
    );
}

#[test]
fn a_price_vector_off_the_solvers_lexicographic_minimum_is_refused_by_name() {
    // The tie-break is a conjunct, not a preference. Two buyers each willing
    // to pay a whole set, each filled to the brim, leave the box as wide as
    // the simplex -- every vector on it certifies these fills -- so this is
    // the book where the tie-break is the only thing choosing. The solver
    // picks the lexicographic minimum and the chain refuses everything else.
    let book = open_box_book();
    let mut clearing = clear_book_v1(&book).expect("clears");
    assert_eq!(clearing.prices, vec![0, 100]);
    agrees(&book, 8);
    clearing.prices = vec![50, 50];
    assert_eq!(
        stream(&book, &clearing, 8),
        Err(RuntimeVerifyErrorV2::NonMinimalPriceVector),
    );
}

#[test]
fn a_certificate_that_omits_a_live_order_is_refused_by_name() {
    let book = transfer_book();
    let mut clearing = clear_book_v1(&book).expect("clears");
    clearing.fills.truncate(1);
    // The Candidate still declares the batch's live order count, so the
    // terminal row sees one order where the batch holds two.
    assert_eq!(
        stream_with_declared_live_orders(&book, &clearing, 2),
        Err(RuntimeVerifyErrorV2::OrderOmitted),
    );
}

/// Stream a clearing whose Candidate declares MORE live orders than its pages
/// enumerate: the completeness conjunct's exact hostile.
fn stream_with_declared_live_orders(
    book: &BookV1,
    clearing: &ClearingV1,
    declared: u32,
) -> Result<Vec<u8>, RuntimeVerifyErrorV2> {
    use dclutch_trading::general::runtime_width::{CandidateHeaderV2, CandidateV2, candidate_len};

    let width = book.outcome_count;
    let mut records =
        build_candidate_records_v1(book, clearing, CANDIDATE, PRODUCT, BATCH, 1, 8, |_| {
            PAGE_REVISION
        })
        .expect("records");
    let page_count = u32::try_from(records.pages.len()).expect("page count");
    let mut candidate = vec![0_u8; candidate_len(width).expect("candidate width")];
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            outcome_count: width,
            page_count,
            candidate_coordinate: 1,
            price_scale: book.price_scale,
            candidate_id: CANDIDATE,
            product_id: PRODUCT,
            batch_id: BATCH,
            live_order_count: declared,
        },
        &clearing.prices,
        &mut candidate,
    )
    .expect("candidate");
    records.candidate = candidate;

    let cursor_len = runtime_verifier_len_v2(width).expect("verifier width");
    let verified_len = verified_candidate_len(width).expect("certificate width");
    let zero_verified = vec![0_u8; verified_len];
    let mut cursor = vec![0_u8; cursor_len];
    let mut certificate: Option<Vec<u8>> = None;
    let mut revision = 0_u64;

    for (page_index, page_bytes) in records.pages.iter().enumerate() {
        let page_index = u32::try_from(page_index).expect("page index");
        let page = PageV2::decode(page_bytes).expect("page decodes");
        for row_index in 0..page.row_count() {
            let execution = page.execution(row_index).expect("row");
            let order = clearing
                .fills
                .iter()
                .map(|fill| fill.order)
                .find(|order| order.order_id == execution.header().order_id)
                .expect("every row names a book order");
            let mut cursor_scratch = vec![0_u8; cursor_len];
            let mut cursor_output = vec![0xa5_u8; cursor_len];
            let mut verified_scratch = vec![0_u8; verified_len];
            let mut verified_output = zero_verified.clone();
            let summary = evaluate_runtime_consider_row_v2(
                RuntimeConsiderRowViewV2 {
                    candidate: &records.candidate,
                    page: page_bytes,
                    cursor_before: &cursor,
                    verified_before: &zero_verified,
                    authenticated_order: terms(&order),
                    expected_page_index: page_index,
                    expected_row_index: row_index,
                    expected_page_revision: PAGE_REVISION,
                    expected_revision: revision,
                    max_orders: 8,
                },
                RuntimeConsiderRowBuffersV2 {
                    cursor_scratch: &mut cursor_scratch,
                    cursor_output: &mut cursor_output,
                    verified_scratch: &mut verified_scratch,
                    verified_output: &mut verified_output,
                },
            )?;
            cursor = cursor_output;
            revision = summary.revision;
            if summary.complete {
                certificate = Some(verified_output);
            }
        }
    }
    Ok(certificate.expect("terminal row"))
}

// ---------------------------------------------------------------------------
// The whole arc, on real records: open a batch, admit and escrow two signed
// orders, close it, solve the book those orders make, verify the clearing
// through the chain, and publish it onto the batch record.
// ---------------------------------------------------------------------------

const MARKET: [u8; 32] = [0x11; 32];
const CONFIG: [u8; 32] = [0x12; 32];
const PRODUCT_RECORD: [u8; 32] = [0x13; 32];
const GENERATION: u64 = 7;
const COLLECTION_CLOSE: u64 = 100;
const SETTLEMENT_CLOSE: u64 = 200;
const ADMISSION_SLOT: u64 = 10;
const CLEARED_SLOT: u64 = 150;

fn order_record(batch_id: [u8; 32], order: &BookOrderV1, width: u32) -> Vec<u8> {
    let header = GeneralOrderHeaderV2 {
        outcome_count: width,
        nonce: order.nonce,
        owner_id: order.owner_id,
        market: MARKET,
        batch_id,
        generation: GENERATION,
        max_lots: order.max_lots,
        max_quote_debit_per_lot: order.max_quote_debit_per_lot,
        min_quote_credit_per_lot: order.min_quote_credit_per_lot,
        valid_until_slot: SETTLEMENT_CLOSE,
        side: order.side,
        outcome_lo: order.outcome,
        outcome_hi: order.outcome,
        claims_per_lot: order.claims_per_lot,
    };
    let count = usize::try_from(width).expect("width");
    let mut receive = vec![0_u64; count];
    let mut deliver = vec![0_u64; count];
    for outcome in 0..width {
        let index = usize::try_from(outcome).expect("outcome index");
        let (row_receive, row_deliver) = header.derived_row(outcome);
        receive[index] = row_receive;
        deliver[index] = row_deliver;
    }
    let mut bytes = vec![0_u8; general_order_len_v2(width).expect("order width")];
    GeneralOrderV2::encode_into(
        header,
        &receive,
        &deliver,
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot: ADMISSION_SLOT,
            released_slot: 0,
        },
        &mut bytes,
    )
    .expect("order record encodes");
    bytes
}

#[test]
fn a_closed_batch_carries_the_clearing_its_own_book_and_certificate_produce() {
    let width = 2;
    let mut root = GeneralRootV2::active(MARKET, CONFIG, GENERATION).expect("active root");
    let revision = root.revision();
    let mut batch = GeneralBatchV2::open(
        &mut root,
        GeneralBatchOpeningV1 {
            outcome_count: width,
            sequence: 0,
            generation: GENERATION,
            market: MARKET,
            product_id: PRODUCT_RECORD,
            config_id: CONFIG,
            price_scale: SCALE,
            collection_close_slot: COLLECTION_CLOSE,
            settlement_close_slot: SETTLEMENT_CLOSE,
            max_orders: 4,
        },
        revision,
        ADMISSION_SLOT,
    )
    .expect("open batch");
    let batch_id = batch.batch_id();

    // The book is the crossing pair, but its identities are now the records'
    // own digests rather than fixture constants.
    let drafts = [buy(1, 0, 3, 60), sell(2, 0, 2, 30)];
    let mut orders = Vec::new();
    for draft in &drafts {
        let bytes = order_record(batch_id, draft, width);
        let order = GeneralOrderV2::decode(&bytes).expect("order decodes");
        batch
            .admit(
                order,
                MakerFundingV1 {
                    owner_id: draft.owner_id,
                    available_quote: 100_000,
                    available_claims: &[100_000, 100_000],
                },
                ADMISSION_SLOT,
            )
            .expect("admit and escrow");
        orders.push(BookOrderV1 {
            order_id: general_order_identity_v2(&bytes).expect("order identity"),
            ..*draft
        });
    }
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close batch");
    assert_eq!(batch.live_order_count(), 2);

    let book = book(width, orders);
    let clearing = clear_book_v1(&book).expect("the batch's own book clears");
    assert_eq!(clearing.prices, vec![60, 40]);
    let certificate = stream_for(batch_id, &book, &clearing, 8).expect("the chain verifies it");

    let mut closed = vec![0_u8; general_batch_len_v2(width).expect("batch width")];
    batch
        .encode_clearing_into(|_| Ok(0), |_| Ok(0), &mut closed)
        .expect("a closed batch encodes with a vacant tail");

    let published =
        publish_clearing_v1(&closed, &certificate, CLEARED_SLOT).expect("the clearing publishes");
    let decoded = GeneralBatchV2::decode(&published).expect("the published record decodes");
    let state = decoded.state();
    assert_eq!(state.status, BatchStatusV1::Cleared);
    assert_eq!(state.clearing.cleared_slot, CLEARED_SLOT);
    assert_eq!(state.clearing.live_order_count, 2);
    assert_eq!(state.clearing.sets_move, GeneralClearingMoveV1::None);
    assert_eq!(state.clearing.sets_quantity, 0);
    assert_eq!(state.clearing.filled_lots, 4);
    for (outcome, expected) in clearing.prices.iter().enumerate() {
        let index = u32::try_from(outcome).expect("outcome index");
        assert_eq!(
            general_batch_price_v2(&published, index).expect("published price"),
            *expected,
        );
        assert_eq!(
            general_batch_residual_v2(&published, index).expect("published residual"),
            0,
            "a book that crosses strands nothing",
        );
    }

    // A BATCH CLEARS ONCE.
    assert_eq!(
        publish_clearing_v1(&published, &certificate, CLEARED_SLOT),
        Err(PublishClearingErrorV1::Batch(
            GeneralCollectionErrorV1::AlreadyCleared
        )),
    );
}

#[test]
fn a_certificate_for_another_batch_is_refused_as_substitution() {
    let width = 2;
    let mut root = GeneralRootV2::active(MARKET, CONFIG, GENERATION).expect("active root");
    let revision = root.revision();
    let mut batch = GeneralBatchV2::open(
        &mut root,
        GeneralBatchOpeningV1 {
            outcome_count: width,
            sequence: 0,
            generation: GENERATION,
            market: MARKET,
            product_id: PRODUCT_RECORD,
            config_id: CONFIG,
            price_scale: SCALE,
            collection_close_slot: COLLECTION_CLOSE,
            settlement_close_slot: SETTLEMENT_CLOSE,
            max_orders: 4,
        },
        revision,
        ADMISSION_SLOT,
    )
    .expect("open batch");
    let batch_id = batch.batch_id();
    let drafts = [buy(1, 0, 3, 60), sell(2, 0, 2, 30)];
    let mut orders = Vec::new();
    for draft in &drafts {
        let bytes = order_record(batch_id, draft, width);
        batch
            .admit(
                GeneralOrderV2::decode(&bytes).expect("order decodes"),
                MakerFundingV1 {
                    owner_id: draft.owner_id,
                    available_quote: 100_000,
                    available_claims: &[100_000, 100_000],
                },
                ADMISSION_SLOT,
            )
            .expect("admit and escrow");
        orders.push(BookOrderV1 {
            order_id: general_order_identity_v2(&bytes).expect("order identity"),
            ..*draft
        });
    }
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close batch");
    let book = book(width, orders);
    let clearing = clear_book_v1(&book).expect("clears");
    // The certificate is verified, canonical, and about a DIFFERENT batch.
    let foreign = stream_for(BATCH, &book, &clearing, 8).expect("the chain verifies it");
    let mut closed = vec![0_u8; general_batch_len_v2(width).expect("batch width")];
    batch
        .encode_clearing_into(|_| Ok(0), |_| Ok(0), &mut closed)
        .expect("closed batch encodes");
    assert_eq!(
        publish_clearing_v1(&closed, &foreign, CLEARED_SLOT),
        Err(PublishClearingErrorV1::Substitution),
    );
}
