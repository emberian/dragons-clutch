//! Hostile coverage for streamed runtime-width candidate verification.
//!
//! Every refusal below names the exact `RuntimeVerifyErrorV2` variant, so a
//! change that keeps the verifier refusing for a different reason is a visible
//! failure rather than a silent reinterpretation.

extern crate std;

use super::*;
use crate::general::runtime_width::{
    CandidateHeaderV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, candidate_len, execution_len,
    page_len,
};
use crate::general_codec::{MAX_SELECTION_CRITERIA, SelectionCriterion};
use std::vec;
use std::vec::Vec;

const CANDIDATE: [u8; 32] = [1; 32];
const PRODUCT: [u8; 32] = [2; 32];
const BATCH: [u8; 32] = [3; 32];
const OWNER: [u8; 32] = [4; 32];

fn order(low: u8) -> [u8; 32] {
    let mut id = [0_u8; 32];
    id[0] = low;
    id
}

/// The candidate carries its own clearing prices; `price_scale` is their sum,
/// so every fixture below states the simplex it means rather than a scale it
/// then has to satisfy.
fn candidate(
    width: u32,
    pages: u32,
    coordinate: u32,
    live_order_count: u32,
    prices: &[u64],
) -> std::vec::Vec<u8> {
    let mut output = vec![0; candidate_len(width).expect("candidate width")];
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            outcome_count: width,
            page_count: pages,
            candidate_coordinate: coordinate,
            price_scale: prices.iter().sum(),
            candidate_id: CANDIDATE,
            product_id: PRODUCT,
            batch_id: BATCH,
            live_order_count,
        },
        prices,
        &mut output,
    )
    .expect("candidate");
    output
}

struct RowFixture {
    bytes: std::vec::Vec<u8>,
    order: AuthenticatedOrderTermsV2,
}

/// One order's shape: side, the single outcome it moves claims at, and how
/// many it moves per lot.
type Shape = (OrderSideV2, u32, u64);

/// One order's signed limits: `(max_lots, max_quote_debit_per_lot,
/// min_quote_credit_per_lot)`.
type Limits = (u64, u64, u64);

fn row(
    width: u32,
    page_coordinate: u32,
    row_coordinate: u32,
    order_low: u8,
    lots: u64,
    shape: Shape,
    limits: Limits,
) -> RowFixture {
    let (side, outcome, claims_per_lot) = shape;
    let (max_lots, debit_limit, credit_floor) = limits;
    let order_id = order(order_low);
    let terms = AuthenticatedOrderTermsV2 {
        order_id,
        owner_id: OWNER,
        nonce: u64::from(order_low),
        max_lots,
        max_quote_debit_per_lot: debit_limit,
        min_quote_credit_per_lot: credit_floor,
        side,
        outcome_lo: outcome,
        outcome_hi: outcome,
        claims_per_lot,
    };
    let receive: Vec<u64> = (0..width).map(|index| terms.derived_row(index).0).collect();
    let deliver: Vec<u64> = (0..width).map(|index| terms.derived_row(index).1).collect();
    let mut bytes = vec![0; execution_len(width).expect("execution width")];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: width,
            page_coordinate,
            execution_coordinate: row_coordinate,
            nonce: terms.nonce,
            order_id,
            owner_id: OWNER,
            max_lots: terms.max_lots,
            lots,
        },
        &receive,
        &deliver,
        &mut bytes,
    )
    .expect("row");
    RowFixture {
        bytes,
        order: terms,
    }
}

fn page(
    width: u32,
    coordinate: u32,
    pages: u32,
    revision: u64,
    rows: &[&[u8]],
) -> std::vec::Vec<u8> {
    let mut output =
        vec![0; page_len(width, u32::try_from(rows.len()).expect("rows")).expect("page width")];
    PageV2::encode_into(
        PageHeaderV2 {
            outcome_count: width,
            page_coordinate: coordinate,
            page_count: pages,
            revision,
            candidate_id: CANDIDATE,
        },
        rows,
        &mut output,
    )
    .expect("page");
    output
}

fn apply_row(
    candidate: &[u8],
    page: &[u8],
    before: &[u8],
    verified_before: &[u8],
    order: AuthenticatedOrderTermsV2,
    coordinate: (u32, u32, u64),
) -> (
    RuntimeConsiderRowSummaryV2,
    std::vec::Vec<u8>,
    std::vec::Vec<u8>,
) {
    let (page_index, row_index, revision) = coordinate;
    let width = CandidateV2::decode(candidate)
        .expect("candidate")
        .header()
        .outcome_count;
    let mut cursor_scratch = vec![0; runtime_verifier_len_v2(width).expect("cursor")];
    let mut cursor_output = vec![0; cursor_scratch.len()];
    let verified_len = verified_candidate_len(width).expect("verified");
    let mut verified_scratch = vec![0; verified_len];
    let mut verified_output = vec![0; verified_len];
    if !verified_before.is_empty() {
        verified_output.copy_from_slice(verified_before);
    }
    let summary = evaluate_runtime_consider_row_v2(
        RuntimeConsiderRowViewV2 {
            candidate,
            page,
            cursor_before: before,
            verified_before,
            authenticated_order: order,
            expected_page_index: page_index,
            expected_row_index: row_index,
            expected_page_revision: 11 + u64::from(page_index),
            expected_revision: revision,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
        },
    )
    .expect("row accepts");
    (summary, cursor_output, verified_output)
}

#[test]
fn runtime_width_sixteen_streams_across_pages_without_page_balance() {
    let width = 16;
    // The whole scale on outcome zero: every other outcome carries a residual
    // (`net_i < M`) and a residual may only sit where the price is zero.
    let mut prices = vec![0_u64; 16];
    prices[0] = 16;
    let candidate = candidate(width, 2, 1, 1, &prices);
    let shape = (OrderSideV2::Buy, 0, 1);
    // Five lots is the order's whole maximum, split two and three across the
    // pages, so the marginal conjunct has nothing to say about it.
    let limits = (5, 2, 0);
    let first = row(width, 1, 1, 1, 2, shape, limits);
    let second = row(width, 2, 1, 1, 3, shape, limits);
    let first_page = page(width, 1, 2, 11, &[&first.bytes]);
    let second_page = page(width, 2, 2, 12, &[&second.bytes]);
    let cursor_len = runtime_verifier_len_v2(width).expect("cursor");
    let verified_len = verified_candidate_len(width).expect("verified");
    let zero_cursor = vec![0; cursor_len];
    let zero_verified = vec![0; verified_len];

    let (summary, middle, unchanged_verified) = apply_row(
        &candidate,
        &first_page,
        &zero_cursor,
        &zero_verified,
        first.order,
        (0, 0, 0),
    );
    assert_eq!(
        summary,
        RuntimeConsiderRowSummaryV2 {
            complete: false,
            order_count: 1,
            revision: 1,
        }
    );
    assert_eq!(unchanged_verified, zero_verified);
    let mut inconsistent_source = middle.clone();
    inconsistent_source[272..276].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        RuntimeCandidateVerifierV2::decode(&inconsistent_source),
        Err(RuntimeVerifyErrorV2::InvalidCursor)
    );

    let (summary, terminal, verified) = apply_row(
        &candidate,
        &second_page,
        &middle,
        &zero_verified,
        second.order,
        (1, 0, 1),
    );
    assert!(summary.complete);
    assert_eq!(summary.revision, 2);
    let cursor = RuntimeCandidateVerifierV2::decode(&terminal).expect("terminal cursor");
    assert!(cursor.is_complete());
    assert!(!cursor.header().has_current_order);
    let certificate = VerifiedCandidateV2::decode(&verified).expect("certificate");
    assert_eq!(certificate.header().filled_lots, 5);
    assert_eq!(certificate.header().quote_debit, 5);
    assert_eq!(certificate.claim_output(0).expect("output"), 5);
    assert_eq!(certificate.claim_output(15).expect("tail"), 0);
    assert_eq!(certificate.price(0).expect("price"), 16);
    assert_eq!(certificate.price(15).expect("tail price"), 0);
    assert_eq!(
        runtime_verified_balance_v2(&verified).expect("balance"),
        RuntimeCandidateBalanceV2 {
            complete_set_move: RuntimeCompleteSetMoveV2::Mint,
            complete_set_quantity: 5,
            quote_surplus: 0,
        }
    );
}

#[test]
fn runtime_width_two_fifty_eight_uses_scratch_without_semantic_cap() {
    let width = 258;
    let mut prices = vec![0_u64; 258];
    prices[0] = 258;
    let candidate = candidate(width, 1, 9, 1, &prices);
    let row = row(width, 1, 1, 1, 1, (OrderSideV2::Buy, 0, 1), (1, 1, 0));
    let page = page(width, 1, 1, 11, &[&row.bytes]);
    let vacant_cursor = [];
    let vacant_verified = [];
    let (summary, cursor, verified) = apply_row(
        &candidate,
        &page,
        &vacant_cursor,
        &vacant_verified,
        row.order,
        (0, 0, 0),
    );
    assert!(summary.complete);
    // Seven `u64` tails per outcome since the joint clearing: prices, the two
    // current-order rows, the two claim accumulators, and the price box's
    // floor and ceiling.
    assert_eq!(cursor.len(), RUNTIME_VERIFIER_HEADER_BYTES_V2 + 56 * 258);
    let certificate = VerifiedCandidateV2::decode(&verified).expect("verified");
    assert_eq!(certificate.claim_output(0).expect("head"), 1);
    assert_eq!(certificate.claim_input(257).expect("tail"), 0);
    assert_eq!(certificate.claim_output(257).expect("tail"), 0);
}

/// THE SELLER'S FLOOR, at its boundary and one step past it.
///
/// Two pure sellers, one at each outcome of a width-two batch, each
/// delivering one claim per lot and filling its whole maximum. The pair is
/// what makes the merge exist: a lone seller leaves `net_i` short of `M` at
/// every outcome it does not touch, and a residual may only sit where the
/// price is zero, so a lone seller can only ever be paid nothing. Together
/// they merge two complete sets, and the lexicographic minimum of the box
/// they induce is `[0, 2]` on a scale of two -- so the SECOND seller is the
/// one the clearing pays, at exactly two.
///
/// The floor is stated on that second seller. It had no conjunct at all
/// until 2026-09-04, which is what `MECHANISM_JOINT_CLEARING_2026_09_04.md`
/// found by modelling the limit as a signed quantity.
///
/// THE FLOOR-ZERO AND FLOOR-ONE ROWS ARE THIS TEST'S OWN POSITIVE CONTROL,
/// and they are the same candidate, the same prices and the same fill as the
/// refusing row. Nothing but the signed floor differs, so a `CreditLimit`
/// here cannot be the fixture failing to reach the conjunct -- an absent
/// signal that logs identically to a disconnected instrument is the failure
/// this shape exists to rule out.
#[test]
fn a_fill_below_the_sellers_floor_refuses_by_name_and_at_the_floor_admits() {
    let width = 2;
    let candidate = candidate(width, 1, 1, 2, &[0, 2]);
    let vacant_cursor = [];
    let vacant_verified = [];
    let unpriced = row(width, 1, 1, 1, 2, (OrderSideV2::Sell, 0, 1), (2, 0, 0));
    for floor in [0, 1] {
        let seller = row(width, 1, 2, 2, 2, (OrderSideV2::Sell, 1, 1), (2, 0, floor));
        let page = page(width, 1, 1, 11, &[&unpriced.bytes, &seller.bytes]);
        let (_, cursor, _) = apply_row(
            &candidate,
            &page,
            &vacant_cursor,
            &vacant_verified,
            unpriced.order,
            (0, 0, 0),
        );
        let (summary, _, verified) = apply_row(
            &candidate,
            &page,
            &cursor,
            &vacant_verified,
            seller.order,
            (0, 1, 1),
        );
        assert!(summary.complete, "floor {floor}");
        let certificate = VerifiedCandidateV2::decode(&verified).expect("verified");
        assert_eq!(certificate.header().quote_credit, 2, "floor {floor}");
        assert_eq!(certificate.header().quote_debit, 0, "floor {floor}");
    }

    let underpaid = row(width, 1, 2, 2, 2, (OrderSideV2::Sell, 1, 1), (2, 0, 2));
    let page = page(width, 1, 1, 11, &[&unpriced.bytes, &underpaid.bytes]);
    let (_, opened, _) = apply_row(
        &candidate,
        &page,
        &vacant_cursor,
        &vacant_verified,
        unpriced.order,
        (0, 0, 0),
    );
    let cursor_len = runtime_verifier_len_v2(width).expect("cursor");
    let verified_len = verified_candidate_len(width).expect("verified");
    let mut cursor_scratch = vec![0; cursor_len];
    let mut cursor_output = vec![0x55; cursor_len];
    let mut verified_scratch = vec![0; verified_len];
    let mut verified_output = vec![0xaa; verified_len];
    let result = evaluate_runtime_consider_row_v2(
        RuntimeConsiderRowViewV2 {
            candidate: &candidate,
            page: &page,
            cursor_before: &opened,
            verified_before: &vec![0; verified_len],
            authenticated_order: underpaid.order,
            expected_page_index: 0,
            expected_row_index: 1,
            expected_page_revision: 11,
            expected_revision: 1,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
        },
    );
    assert_eq!(result, Err(RuntimeVerifyErrorV2::CreditLimit));
    assert_eq!(cursor_output, vec![0x55; cursor_len]);
    assert_eq!(verified_output, vec![0xaa; verified_len]);
}

/// Two fragments of one order may not carry two floors.
///
/// The floor joins `require_same_order`'s list for the same reason every
/// other immutable term is on it: a candidate that could relax the floor
/// between two pages of one order would have a floor only on the fragment
/// that happened to be checked first.
#[test]
fn a_second_fragment_may_not_lower_the_floor_its_first_carried() {
    let width = 2;
    let candidate = candidate(width, 2, 1, 1, &[0, 2]);
    let first = row(width, 1, 1, 1, 1, (OrderSideV2::Sell, 1, 1), (2, 0, 1));
    let first_page = page(width, 1, 2, 11, &[&first.bytes]);
    let vacant_cursor = [];
    let vacant_verified = [];
    let (summary, cursor, verified) = apply_row(
        &candidate,
        &first_page,
        &vacant_cursor,
        &vacant_verified,
        first.order,
        (0, 0, 0),
    );
    assert!(!summary.complete);
    assert_eq!(
        RuntimeCandidateVerifierV2::decode(&cursor)
            .expect("cursor")
            .current_order()
            .expect("current order")
            .expect("open order")
            .min_quote_credit_per_lot,
        1
    );

    let second = row(width, 2, 1, 1, 1, (OrderSideV2::Sell, 1, 1), (2, 0, 1));
    let second_page = page(width, 2, 2, 12, &[&second.bytes]);
    let mut relaxed = second.order;
    relaxed.min_quote_credit_per_lot = 0;
    let mut cursor_scratch = vec![0; cursor.len()];
    let mut cursor_output = vec![0x55; cursor.len()];
    let mut verified_scratch = vec![0; verified.len()];
    let mut verified_output = vec![0xaa; verified.len()];
    let result = evaluate_runtime_consider_row_v2(
        RuntimeConsiderRowViewV2 {
            candidate: &candidate,
            page: &second_page,
            cursor_before: &cursor,
            verified_before: &verified,
            authenticated_order: relaxed,
            expected_page_index: 1,
            expected_row_index: 0,
            expected_page_revision: 12,
            expected_revision: 1,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
        },
    );
    assert_eq!(result, Err(RuntimeVerifyErrorV2::OrderSubstitution));
    assert_eq!(cursor_output, vec![0x55; cursor.len()]);
}

#[test]
fn hostile_order_substitution_limit_and_skip_preserve_candidates() {
    let width = 2;
    let candidate = candidate(width, 1, 1, 1, &[2, 0]);
    // A buy whose signed cap is zero, filled at a price of two: the quote
    // conjunct is what refuses it, not its shape or its fill.
    let first = row(width, 1, 1, 1, 1, (OrderSideV2::Buy, 0, 1), (1, 0, 0));
    let page = page(width, 1, 1, 11, &[&first.bytes]);
    let cursor_len = runtime_verifier_len_v2(width).expect("cursor");
    let verified_len = verified_candidate_len(width).expect("verified");
    let zero_cursor = vec![0; cursor_len];
    let zero_verified = vec![0; verified_len];
    let mut cursor_scratch = vec![0; cursor_len];
    let mut cursor_output = vec![0x55; cursor_len];
    let mut verified_scratch = vec![0; verified_len];
    let mut verified_output = vec![0xaa; verified_len];
    let result = evaluate_runtime_consider_row_v2(
        RuntimeConsiderRowViewV2 {
            candidate: &candidate,
            page: &page,
            cursor_before: &zero_cursor,
            verified_before: &zero_verified,
            authenticated_order: first.order,
            expected_page_index: 0,
            expected_row_index: 0,
            expected_page_revision: 11,
            expected_revision: 0,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
        },
    );
    assert_eq!(result, Err(RuntimeVerifyErrorV2::QuoteLimit));
    assert_eq!(cursor_output, vec![0x55; cursor_len]);
    assert_eq!(verified_output, vec![0xaa; verified_len]);

    let mut substituted = first.order;
    substituted.owner_id = [9; 32];
    let result = evaluate_runtime_consider_row_v2(
        RuntimeConsiderRowViewV2 {
            candidate: &candidate,
            page: &page,
            cursor_before: &zero_cursor,
            verified_before: &zero_verified,
            authenticated_order: substituted,
            expected_page_index: 0,
            expected_row_index: 0,
            expected_page_revision: 11,
            expected_revision: 0,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
        },
    );
    assert_eq!(
        result,
        Err(RuntimeVerifyErrorV2::AuthenticatedOrderMismatch)
    );
    assert_eq!(cursor_output, vec![0x55; cursor_len]);
    assert_eq!(verified_output, vec![0xaa; verified_len]);
}

#[test]
fn verifier_emits_exact_order_manifests_across_group_boundaries() {
    let width = 2;
    // Two buys, one at each outcome, each filling its whole maximum. The box
    // they induce is `[0, 2] x [0, 2]` and its lexicographic minimum on the
    // scale-two simplex is `[0, 2]`, which is the vector the candidate carries.
    let candidate = candidate(width, 1, 1, 2, &[0, 2]);
    let first = row(width, 1, 1, 1, 1, (OrderSideV2::Buy, 0, 1), (1, 1, 0));
    let second = row(width, 1, 2, 2, 1, (OrderSideV2::Buy, 1, 1), (1, 1, 0));
    let page = page(width, 1, 1, 11, &[&first.bytes, &second.bytes]);
    let cursor_len = runtime_verifier_len_v2(width).expect("cursor");
    let verified_len = verified_candidate_len(width).expect("verified");
    let zero_cursor = vec![0; cursor_len];
    let zero_verified = vec![0; verified_len];
    let mut cursor_scratch = vec![0; cursor_len];
    let mut cursor_first = vec![0; cursor_len];
    let mut verified_scratch = vec![0; verified_len];
    let mut verified_first = vec![0; verified_len];
    let mut empty_manifest_scratch =
        vec![0; settlement_manifest_len_v2(width, 0).expect("manifest")];
    let mut empty_manifest_output = vec![0xaa; empty_manifest_scratch.len()];
    let summary = evaluate_runtime_consider_row_with_manifest_v2(
        RuntimeConsiderRowViewV2 {
            candidate: &candidate,
            page: &page,
            cursor_before: &zero_cursor,
            verified_before: &zero_verified,
            authenticated_order: first.order,
            expected_page_index: 0,
            expected_row_index: 0,
            expected_page_revision: 11,
            expected_revision: 0,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_first,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_first,
        },
        RuntimeManifestBuffersV2 {
            manifest_scratch: &mut empty_manifest_scratch,
            manifest_output: &mut empty_manifest_output,
        },
    )
    .expect("first row");
    assert!(!summary.complete);
    assert_eq!(
        SettlementManifestV2::decode(&empty_manifest_output)
            .expect("empty manifest")
            .header()
            .order_count,
        0
    );

    let manifest_len = settlement_manifest_len_v2(width, 2).expect("manifest");
    let mut manifest_scratch = vec![0; manifest_len];
    let mut manifest_output = vec![0xbb; manifest_len];
    let mut cursor_terminal = vec![0; cursor_len];
    let mut verified_terminal = vec![0; verified_len];
    let summary = evaluate_runtime_consider_row_with_manifest_v2(
        RuntimeConsiderRowViewV2 {
            candidate: &candidate,
            page: &page,
            cursor_before: &cursor_first,
            verified_before: &zero_verified,
            authenticated_order: second.order,
            expected_page_index: 0,
            expected_row_index: 1,
            expected_page_revision: 11,
            expected_revision: 1,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_terminal,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_terminal,
        },
        RuntimeManifestBuffersV2 {
            manifest_scratch: &mut manifest_scratch,
            manifest_output: &mut manifest_output,
        },
    )
    .expect("terminal row");
    assert!(summary.complete);
    let manifest = SettlementManifestV2::decode(&manifest_output).expect("manifest");
    assert_eq!(manifest.header().order_count, 2);
    assert_eq!(
        manifest.order(0).expect("first").header().order_coordinate,
        1
    );
    assert_eq!(manifest.order(0).expect("first").claim_output(0), Ok(1));
    assert_eq!(
        manifest.order(0).expect("first").header().source_page_index,
        0
    );
    assert_eq!(
        manifest
            .order(0)
            .expect("first")
            .header()
            .source_execution_index,
        0
    );
    assert_eq!(
        manifest.order(1).expect("second").header().order_coordinate,
        2
    );
    assert_eq!(manifest.order(1).expect("second").claim_output(1), Ok(1));
    assert_eq!(
        manifest
            .order(1)
            .expect("second")
            .header()
            .source_page_index,
        0
    );
    assert_eq!(
        manifest
            .order(1)
            .expect("second")
            .header()
            .source_execution_index,
        1
    );
    assert!(VerifiedCandidateV2::decode(&verified_terminal).is_ok());

    let mut undersized_scratch = vec![0; manifest_len - 1];
    let mut undersized_output = vec![0xcc; manifest_len - 1];
    let cursor_sentinel = vec![0xdd; cursor_len];
    let verified_sentinel = vec![0xee; verified_len];
    let mut cursor_output = cursor_sentinel.clone();
    let mut verified_output = verified_sentinel.clone();
    let result = evaluate_runtime_consider_row_with_manifest_v2(
        RuntimeConsiderRowViewV2 {
            candidate: &candidate,
            page: &page,
            cursor_before: &cursor_first,
            verified_before: &zero_verified,
            authenticated_order: second.order,
            expected_page_index: 0,
            expected_row_index: 1,
            expected_page_revision: 11,
            expected_revision: 1,
            max_orders: 10,
        },
        RuntimeConsiderRowBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
        },
        RuntimeManifestBuffersV2 {
            manifest_scratch: &mut undersized_scratch,
            manifest_output: &mut undersized_output,
        },
    );
    assert_eq!(result, Err(RuntimeVerifyErrorV2::InvalidLength));
    assert_eq!(cursor_output, cursor_sentinel);
    assert_eq!(verified_output, verified_sentinel);
    assert_eq!(undersized_output, vec![0xcc; manifest_len - 1]);
}

#[test]
fn best_valid_submitted_candidate_uses_exact_policy() {
    let width = 2;
    let left_candidate = candidate(width, 1, 1, 1, &[2, 0]);
    let mut right_candidate = candidate(width, 1, 2, 1, &[2, 0]);
    right_candidate[32] = 7;
    let row = row(width, 1, 1, 1, 1, (OrderSideV2::Buy, 0, 1), (1, 1, 0));
    let left_page = page(width, 1, 1, 11, &[&row.bytes]);
    let mut right_page = left_page.clone();
    right_page[32] = 7;
    let zero_cursor = vec![0; runtime_verifier_len_v2(width).expect("cursor")];
    let zero_verified = vec![0; verified_candidate_len(width).expect("verified")];
    let (_, _, left) = apply_row(
        &left_candidate,
        &left_page,
        &zero_cursor,
        &zero_verified,
        row.order,
        (0, 0, 0),
    );
    let (_, _, right) = apply_row(
        &right_candidate,
        &right_page,
        &zero_cursor,
        &zero_verified,
        row.order,
        (0, 0, 0),
    );
    let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
    criteria[0] = SelectionCriterion::MinimizeCandidateId;
    let policy = SelectionPolicyV1 {
        policy_id: [8; 32],
        criterion_count: 1,
        criteria,
    };
    assert_eq!(
        runtime_candidate_better_v2(&policy, &left, &right),
        Ok(true)
    );
}

/// The two live authors of the selection comparison law agree exactly.
///
/// C-05's whole vocabulary rests on one predicate: which of two verified
/// candidates is the better valid submitted one. That predicate has TWO
/// implementations in this crate, and both are live.
/// `runtime_candidate_key_better_v2` above is the persisted-key interpreter
/// the admitted accelerator and the operator both call through
/// `consider_verified_candidate_v2`. `crate::general::candidate_better` is the
/// V1-record copy, reached from `crate::general::consider_verified_input` and
/// therefore from `plan.rs`, which is the stateless differential oracle the
/// accelerator is checked against. Each carries its own private
/// `le_numeric_id`, and the tie-break is little-endian -- byte 31 is most
/// significant -- which is exactly the kind of detail one copy drifts on.
///
/// Nothing joined them. An oracle that has quietly stopped agreeing with
/// the thing it is the oracle FOR does not fail; it certifies. This walks
/// every criterion prefix over a corpus that separates each criterion in
/// turn, ties on it, and ties on all three.
#[test]
fn the_oracle_and_the_runtime_agree_on_which_candidate_is_better() {
    let keys = [
        (7_u64, 5_u64, [3_u8; 32]),
        (7, 5, [4; 32]),
        (7, 9, [3; 32]),
        (11, 5, [3; 32]),
        (0, 0, [0; 32]),
        (u64::MAX, u64::MAX, [0xff; 32]),
        // A pair separated only in the MOST significant tie-break byte,
        // and one separated only in the least, so a big-endian copy of
        // `le_numeric_id` would disagree here rather than nowhere.
        (7, 5, {
            let mut id = [3_u8; 32];
            id[31] = 9;
            id
        }),
        (7, 5, {
            let mut id = [3_u8; 32];
            id[0] = 9;
            id
        }),
    ];
    let orders = [
        [SelectionCriterion::MinimizeCandidateId; 3],
        [
            SelectionCriterion::MaximizeFilledLots,
            SelectionCriterion::MinimizeQuoteSurplus,
            SelectionCriterion::MinimizeCandidateId,
        ],
        [
            SelectionCriterion::MinimizeQuoteSurplus,
            SelectionCriterion::MaximizeFilledLots,
            SelectionCriterion::MinimizeCandidateId,
        ],
    ];
    let record = |key: (u64, u64, [u8; 32])| crate::general::VerifiedCandidateV1 {
        candidate_id: key.2,
        product_id: PRODUCT,
        batch_id: BATCH,
        outcome_count: 2,
        page_count: 1,
        filled_lots: key.0,
        quote_surplus: key.1,
        quote_inputs: 0,
        quote_outputs: 0,
        complete_set_move: crate::general::CompleteSetMoveV1::None,
        complete_set_quantity: 0,
        claim_inputs: [0; crate::general_codec::MAX_OUTCOMES],
        claim_outputs: [0; crate::general_codec::MAX_OUTCOMES],
    };
    let mut compared = 0_usize;
    for order in orders {
        for count in 1..=order.len() {
            let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
            criteria[..count].copy_from_slice(&order[..count]);
            let policy = SelectionPolicyV1 {
                policy_id: [8; 32],
                criterion_count: u8::try_from(count).expect("criterion prefix"),
                criteria,
            };
            for left in keys {
                for right in keys {
                    let runtime = runtime_candidate_key_better_v2(
                        &policy,
                        RuntimeCandidateComparisonKeyV2 {
                            filled_lots: left.0,
                            quote_surplus: left.1,
                            candidate_id: left.2,
                        },
                        RuntimeCandidateComparisonKeyV2 {
                            filled_lots: right.0,
                            quote_surplus: right.1,
                            candidate_id: right.2,
                        },
                    )
                    .expect("runtime comparison");
                    let oracle =
                        crate::general::candidate_better(&policy, &record(left), &record(right));
                    assert_eq!(
                        runtime, oracle,
                        "the runtime and the oracle disagree at {count} criteria on \
                         {left:?} against {right:?}",
                    );
                    compared += 1;
                }
            }
        }
    }
    assert_eq!(compared, 3 * 3 * keys.len() * keys.len());
}
