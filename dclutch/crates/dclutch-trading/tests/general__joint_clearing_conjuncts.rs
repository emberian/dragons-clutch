//! The joint clearing's KKT conjuncts, each refused by its own name.
//!
//! `MECHANISM_JOINT_CLEARING_2026_09_04.md` states the certificate the chain
//! verifies; decision 0032 fixes the strand and the tie-break. Five of the
//! conjuncts were declared and raised by `runtime_verify.rs` with nothing
//! asserting them. This file is that assertion: one hostile per conjunct,
//! each `assert_eq!`d against the exact `RuntimeVerifyErrorV2` variant, so a
//! change that keeps the verifier refusing for a DIFFERENT reason is a red
//! test rather than a silent reinterpretation.
//!
//! Every fixture is `N = 2` or `N = 3` with `price_scale = 100` and one lot
//! moving `100` claims, so a limit in quote atoms per lot is numerically the
//! price in scale units per claim and every conjunct can be checked by hand.

use dclutch_trading::general::runtime_verify::{
    AuthenticatedOrderTermsV2, OrderSideV2, RuntimeConsiderRowBuffersV2, RuntimeConsiderRowViewV2,
    RuntimeVerifyErrorV2, evaluate_runtime_consider_row_v2, runtime_verifier_len_v2,
};
use dclutch_trading::general::runtime_width::{
    CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
    RuntimeWidthErrorV2, candidate_len, execution_len, page_len, verified_candidate_len,
};

const PRODUCT: [u8; 32] = [2; 32];
const BATCH: [u8; 32] = [3; 32];
const CANDIDATE: [u8; 32] = [7; 32];
const PAGE_REVISION: u64 = 11;
const SCALE: u64 = 100;
const CLAIMS_PER_LOT: u64 = 100;

fn identity(low: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = low;
    value
}

/// One order and the fill a hostile candidate claims for it.
#[derive(Clone, Copy)]
struct Row {
    low: u8,
    side: OrderSideV2,
    outcome: u32,
    max_lots: u64,
    cap: u64,
    floor: u64,
    lots: u64,
}

fn buy(low: u8, outcome: u32, max_lots: u64, cap: u64, lots: u64) -> Row {
    Row {
        low,
        side: OrderSideV2::Buy,
        outcome,
        max_lots,
        cap,
        floor: 0,
        lots,
    }
}

fn sell(low: u8, outcome: u32, max_lots: u64, floor: u64, lots: u64) -> Row {
    Row {
        low,
        side: OrderSideV2::Sell,
        outcome,
        max_lots,
        cap: 0,
        floor,
        lots,
    }
}

fn terms(row: Row) -> AuthenticatedOrderTermsV2 {
    AuthenticatedOrderTermsV2 {
        order_id: identity(row.low),
        owner_id: identity(row.low.wrapping_add(0x40)),
        nonce: u64::from(row.low),
        max_lots: row.max_lots,
        max_quote_debit_per_lot: row.cap,
        min_quote_credit_per_lot: row.floor,
        side: row.side,
        outcome_lo: row.outcome,
        outcome_hi: row.outcome,
        claims_per_lot: CLAIMS_PER_LOT,
    }
}

fn execution_bytes(width: u32, row: Row, page_coordinate: u32, row_index: u32) -> Vec<u8> {
    let count = usize::try_from(width).expect("width");
    let index = usize::try_from(row.outcome).expect("outcome");
    let mut receive = vec![0_u64; count];
    let mut deliver = vec![0_u64; count];
    match row.side {
        OrderSideV2::Buy => receive[index] = CLAIMS_PER_LOT,
        OrderSideV2::Sell => deliver[index] = CLAIMS_PER_LOT,
    }
    let mut bytes = vec![0_u8; execution_len(width).expect("execution width")];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: width,
            page_coordinate,
            execution_coordinate: row_index + 1,
            nonce: u64::from(row.low),
            order_id: identity(row.low),
            owner_id: identity(row.low.wrapping_add(0x40)),
            max_lots: row.max_lots,
            lots: row.lots,
        },
        &receive,
        &deliver,
        &mut bytes,
    )
    .expect("execution encodes");
    bytes
}

/// Build one Candidate and one Page, then stream every row.
///
/// `live_order_count` is the Candidate's declaration, separate from the rows
/// actually laid out, because the completeness conjunct is exactly the
/// disagreement between the two.
fn verify(
    width: u32,
    prices: &[u64],
    live_order_count: u32,
    rows: &[Row],
) -> Result<Vec<u8>, RuntimeVerifyErrorV2> {
    let mut candidate = vec![0_u8; candidate_len(width).expect("candidate width")];
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            outcome_count: width,
            page_count: 1,
            candidate_coordinate: 1,
            price_scale: SCALE,
            candidate_id: CANDIDATE,
            product_id: PRODUCT,
            batch_id: BATCH,
            live_order_count,
        },
        prices,
        &mut candidate,
    )
    .expect("candidate encodes");

    let row_bytes: Vec<Vec<u8>> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            execution_bytes(width, *row, 1, u32::try_from(index).expect("row index"))
        })
        .collect();
    let row_refs: Vec<&[u8]> = row_bytes.iter().map(Vec::as_slice).collect();
    let mut page = vec![
        0_u8;
        page_len(width, u32::try_from(rows.len()).expect("row count"))
            .expect("page width")
    ];
    PageV2::encode_into(
        PageHeaderV2 {
            outcome_count: width,
            page_coordinate: 1,
            page_count: 1,
            revision: PAGE_REVISION,
            candidate_id: CANDIDATE,
        },
        &row_refs,
        &mut page,
    )
    .expect("page encodes");

    let cursor_len = runtime_verifier_len_v2(width).expect("verifier width");
    let verified_len = verified_candidate_len(width).expect("certificate width");
    let zero_verified = vec![0_u8; verified_len];
    let mut cursor = vec![0_u8; cursor_len];
    let mut certificate: Option<Vec<u8>> = None;

    for (index, row) in rows.iter().enumerate() {
        let row_index = u32::try_from(index).expect("row index");
        let mut cursor_scratch = vec![0_u8; cursor_len];
        let mut cursor_output = vec![0xa5_u8; cursor_len];
        let mut verified_scratch = vec![0_u8; verified_len];
        let mut verified_output = zero_verified.clone();
        let summary = evaluate_runtime_consider_row_v2(
            RuntimeConsiderRowViewV2 {
                candidate: &candidate,
                page: &page,
                cursor_before: &cursor,
                verified_before: &zero_verified,
                authenticated_order: terms(*row),
                expected_page_index: 0,
                expected_row_index: row_index,
                expected_page_revision: PAGE_REVISION,
                expected_revision: u64::from(row_index),
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
        if summary.complete {
            certificate = Some(verified_output);
        }
    }
    Ok(certificate.expect("the last row completes the candidate"))
}

/// The book both hostiles below start from: one buy capped at 60 and one sell
/// with a floor of 30, crossing for two lots at outcome 0 of a two-outcome
/// market. The price the box admits is exactly `[60, 40]`.
fn crossing_rows() -> [Row; 2] {
    [buy(1, 0, 3, 60, 2), sell(2, 0, 2, 30, 2)]
}

#[test]
fn the_crossing_book_verifies_at_the_price_its_own_box_forces() {
    // The positive control. Without it the six refusals below could all be
    // refusals of a fixture that never verified in the first place.
    let bytes = verify(2, &[60, 40], 2, &crossing_rows()).expect("the crossing book verifies");
    assert_eq!(bytes.len(), verified_candidate_len(2).expect("width"));
}

#[test]
fn a_certificate_that_omits_a_live_order_is_refused_as_order_omitted() {
    // The batch holds two live orders; the candidate enumerates one. Omission
    // is how a solver would evade the marginal conjunct on the order it left
    // out, so the completeness conjunct is what makes rationing checkable.
    assert_eq!(
        verify(2, &[60, 40], 2, &crossing_rows()[..1]),
        Err(RuntimeVerifyErrorV2::OrderOmitted),
    );
}

#[test]
fn prices_that_do_not_sum_to_the_scale_never_become_a_candidate() {
    // The simplex is refused at the Candidate record, before any row streams:
    // a price vector off the simplex is not a bad clearing, it is not a
    // clearing at all.
    let mut candidate = vec![0_u8; candidate_len(2).expect("candidate width")];
    assert_eq!(
        CandidateV2::encode_into(
            CandidateHeaderV2 {
                outcome_count: 2,
                page_count: 1,
                candidate_coordinate: 1,
                price_scale: SCALE,
                candidate_id: CANDIDATE,
                product_id: PRODUCT,
                batch_id: BATCH,
                live_order_count: 2,
            },
            &[60, 41],
            &mut candidate,
        ),
        Err(RuntimeWidthErrorV2::InvalidSimplex),
    );
}

#[test]
fn a_residual_on_a_priced_outcome_is_refused_as_priced_residual() {
    // Two buyers at outcomes 0 and 1 of a THREE-outcome market together pay a
    // whole set, so the batch mints two hundred claims at every outcome and
    // hands none of outcome 2 to anybody. Complementary slackness admits that
    // residual only at price zero; this certificate prices outcome 2 at 20.
    let rows = [buy(1, 0, 2, 60, 2), buy(2, 1, 2, 50, 2)];
    assert_eq!(
        verify(3, &[40, 40, 20], 2, &rows),
        Err(RuntimeVerifyErrorV2::PricedResidual),
    );
    // The same fills with the residual outcome at zero verify.
    verify(3, &[50, 50, 0], 2, &rows).expect("the strand at a zero price verifies");
}

#[test]
fn a_price_vector_inside_the_box_but_off_its_minimum_is_refused_as_non_minimal() {
    // Two buyers, each willing to pay a whole set, each filled to the brim:
    // the box is the entire simplex and every vector on it certifies these
    // fills. Decision 0032 makes the lexicographic minimum the ONE admissible
    // vector, so the clearing is a function of the book and not of the solver.
    let rows = [buy(1, 0, 2, 100, 2), buy(2, 1, 2, 100, 2)];
    verify(2, &[0, 100], 2, &rows).expect("the lexicographic minimum verifies");
    assert_eq!(
        verify(2, &[50, 50], 2, &rows),
        Err(RuntimeVerifyErrorV2::NonMinimalPriceVector),
    );
}

#[test]
fn an_order_left_short_strictly_inside_its_limit_is_refused_as_rationed_inside_limit() {
    // The buyer signed a cap of 80 and the clearing charges 60, so it is
    // strictly inside its limit; a candidate that nonetheless leaves it two
    // lots short of its maximum of three is rationing someone who should have
    // been filled.
    let rows = [buy(1, 0, 3, 80, 2), sell(2, 0, 2, 30, 2)];
    assert_eq!(
        verify(2, &[60, 40], 2, &rows),
        Err(RuntimeVerifyErrorV2::RationedInsideLimit),
    );
}

#[test]
fn a_seller_paid_below_their_floor_is_refused_as_credit_limit() {
    // `CreditLimit` is `QuoteLimit`'s twin and it accuses the other party.
    // Until the joint arm the order record had no floor at all, so a net
    // seller signed a maximum fill and accepted whatever price the winning
    // candidate chose, down to zero. This is the first fixture that says the
    // floor binds: a seller with a floor of 70 filled at a price of 60.
    let rows = [buy(1, 0, 3, 60, 2), sell(2, 0, 2, 70, 2)];
    assert_eq!(
        verify(2, &[60, 40], 2, &rows),
        Err(RuntimeVerifyErrorV2::CreditLimit),
    );
}

#[test]
fn a_buyer_charged_above_their_cap_is_refused_as_quote_limit() {
    // The control for the twin above: the same book, the same price, the
    // accusation pointing the other way.
    let rows = [buy(1, 0, 2, 50, 2), sell(2, 0, 2, 30, 2)];
    assert_eq!(
        verify(2, &[60, 40], 2, &rows),
        Err(RuntimeVerifyErrorV2::QuoteLimit),
    );
}

#[test]
fn an_interval_order_is_refused_as_shape_not_interval_until_a_later_cohort() {
    // Cohort-18 admits single-outcome orders only. The layout carries the
    // interval so the record does not move when the interval conjunct lands;
    // the verifier refuses `lo < hi` by name until then.
    let mut interval = terms(buy(1, 0, 3, 60, 2));
    interval.outcome_hi = 1;
    assert_eq!(
        verify_with_terms(2, &[60, 40], 2, &crossing_rows(), 0, interval),
        Err(RuntimeVerifyErrorV2::ShapeNotInterval),
    );
}

#[test]
fn a_row_that_disagrees_with_its_authenticated_shape_is_refused_as_shape_not_interval() {
    // The accelerator cannot reach the order record, so it re-derives the row
    // from the authenticated terms. A candidate whose row claims the buyer
    // moves claims at outcome 1 while the terms say outcome 0 is refused
    // before the row can touch the inventory.
    let mut moved = terms(buy(1, 0, 3, 60, 2));
    moved.outcome_lo = 1;
    moved.outcome_hi = 1;
    assert_eq!(
        verify_with_terms(2, &[60, 40], 2, &crossing_rows(), 0, moved),
        Err(RuntimeVerifyErrorV2::ShapeNotInterval),
    );
}

/// [`verify`], with one row's authenticated terms replaced.
///
/// The shape conjuncts are the only ones that can disagree between the ROW and
/// the TERMS, so they need a driver that can separate the two.
fn verify_with_terms(
    width: u32,
    prices: &[u64],
    live_order_count: u32,
    rows: &[Row],
    substituted_index: usize,
    substituted: AuthenticatedOrderTermsV2,
) -> Result<Vec<u8>, RuntimeVerifyErrorV2> {
    let mut candidate = vec![0_u8; candidate_len(width).expect("candidate width")];
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            outcome_count: width,
            page_count: 1,
            candidate_coordinate: 1,
            price_scale: SCALE,
            candidate_id: CANDIDATE,
            product_id: PRODUCT,
            batch_id: BATCH,
            live_order_count,
        },
        prices,
        &mut candidate,
    )
    .expect("candidate encodes");
    let row_bytes: Vec<Vec<u8>> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            execution_bytes(width, *row, 1, u32::try_from(index).expect("row index"))
        })
        .collect();
    let row_refs: Vec<&[u8]> = row_bytes.iter().map(Vec::as_slice).collect();
    let mut page = vec![
        0_u8;
        page_len(width, u32::try_from(rows.len()).expect("row count"))
            .expect("page width")
    ];
    PageV2::encode_into(
        PageHeaderV2 {
            outcome_count: width,
            page_coordinate: 1,
            page_count: 1,
            revision: PAGE_REVISION,
            candidate_id: CANDIDATE,
        },
        &row_refs,
        &mut page,
    )
    .expect("page encodes");

    let cursor_len = runtime_verifier_len_v2(width).expect("verifier width");
    let verified_len = verified_candidate_len(width).expect("certificate width");
    let zero_verified = vec![0_u8; verified_len];
    let mut cursor = vec![0_u8; cursor_len];
    let mut certificate: Option<Vec<u8>> = None;

    for (index, row) in rows.iter().enumerate() {
        let row_index = u32::try_from(index).expect("row index");
        let authenticated = if index == substituted_index {
            substituted
        } else {
            terms(*row)
        };
        let mut cursor_scratch = vec![0_u8; cursor_len];
        let mut cursor_output = vec![0xa5_u8; cursor_len];
        let mut verified_scratch = vec![0_u8; verified_len];
        let mut verified_output = zero_verified.clone();
        let summary = evaluate_runtime_consider_row_v2(
            RuntimeConsiderRowViewV2 {
                candidate: &candidate,
                page: &page,
                cursor_before: &cursor,
                verified_before: &zero_verified,
                authenticated_order: authenticated,
                expected_page_index: 0,
                expected_row_index: row_index,
                expected_page_revision: PAGE_REVISION,
                expected_revision: u64::from(row_index),
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
        if summary.complete {
            certificate = Some(verified_output);
        }
    }
    Ok(certificate.expect("the last row completes the candidate"))
}
