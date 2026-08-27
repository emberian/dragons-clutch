//! Hostile coverage for General batch collection.
//!
//! Every refusal below is stated against the exact variant, not against "an
//! error", so a later change that keeps the record refusing for a different
//! reason is a visible test failure rather than a silent reinterpretation.

use std::vec;
use std::vec::Vec;

use dclutch_general_config_contract::root::{GeneralRootV2, RootError};

use super::*;

const WIDTH: u32 = 3;
const COLLECTION_CLOSE: u64 = 1_000;
const SETTLEMENT_CLOSE: u64 = 2_000;

fn id(low: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = low;
    value
}

fn active_root() -> GeneralRootV2 {
    GeneralRootV2::active(id(1), id(2), 7).expect("active root")
}

fn opening() -> GeneralBatchOpeningV1 {
    GeneralBatchOpeningV1 {
        outcome_count: WIDTH,
        sequence: 0,
        generation: 7,
        market: id(1),
        product_id: id(3),
        config_id: id(2),
        price_scale: 100,
        collection_close_slot: COLLECTION_CLOSE,
        settlement_close_slot: SETTLEMENT_CLOSE,
        max_orders: 4,
    }
}

/// Open one batch at the root's exact revision and sequence.
fn open_batch(root: &mut GeneralRootV2) -> GeneralBatchV1 {
    let revision = root.revision();
    GeneralBatchV1::open(root, opening(), revision, 10).expect("open batch")
}

fn placed(admitted_slot: u64) -> GeneralOrderStateV1 {
    GeneralOrderStateV1 {
        phase: GeneralOrderPhaseV1::Placed,
        admitted_slot,
        released_slot: 0,
    }
}

fn order_bytes(
    batch_id: [u8; 32],
    owner: u8,
    nonce: u64,
    max_lots: u64,
    max_quote_debit_per_lot: u64,
    receive: &[u64],
    deliver: &[u64],
) -> Vec<u8> {
    let mut bytes = vec![0_u8; general_order_len_v1(WIDTH).expect("order width")];
    GeneralOrderV1::encode_into(
        GeneralOrderHeaderV1 {
            outcome_count: WIDTH,
            nonce,
            owner_id: id(owner),
            market: id(1),
            batch_id,
            generation: 7,
            max_lots,
            max_quote_debit_per_lot,
            valid_until_slot: SETTLEMENT_CLOSE,
        },
        receive,
        deliver,
        placed(10),
        &mut bytes,
    )
    .expect("order bytes");
    bytes
}

/// Build a real Execution row, tails included.
///
/// The tails are not decoration: `authenticate_order_execution_v1` binds them
/// to the order record, so a helper that fabricated a header alone could not
/// express the substitution these tests refuse.
fn execution_bytes(
    order: GeneralOrderV1<'_>,
    lots: u64,
    receive: &[u64],
    deliver: &[u64],
) -> Vec<u8> {
    let header = order.header();
    execution_bytes_from(
        crate::runtime_width::ExecutionHeaderV2 {
            outcome_count: header.outcome_count,
            page_coordinate: 1,
            execution_coordinate: 1,
            nonce: header.nonce,
            order_id: order.order_id(),
            owner_id: header.owner_id,
            max_lots: header.max_lots,
            lots,
        },
        receive,
        deliver,
    )
}

fn execution_bytes_from(
    header: crate::runtime_width::ExecutionHeaderV2,
    receive: &[u64],
    deliver: &[u64],
) -> Vec<u8> {
    let mut bytes =
        vec![0_u8; crate::runtime_width::execution_len(header.outcome_count).expect("row width")];
    ExecutionV2::encode_into(header, receive, deliver, &mut bytes).expect("row bytes");
    bytes
}

fn row(bytes: &[u8]) -> ExecutionV2<'_> {
    ExecutionV2::decode(bytes).expect("row")
}

fn simple_order(batch_id: [u8; 32], owner: u8, nonce: u64) -> Vec<u8> {
    order_bytes(batch_id, owner, nonce, 10, 5, &[1, 0, 0], &[0, 2, 0])
}

fn funding(owner: u8, quote: u64, claims: &[u64]) -> MakerFundingV1<'_> {
    MakerFundingV1 {
        owner_id: id(owner),
        available_quote: quote,
        available_claims: claims,
    }
}

// ---------------------------------------------------------------------------
// The record layer
// ---------------------------------------------------------------------------

#[test]
fn batch_bytes_round_trip_through_a_hostile_decode() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let bytes = batch.to_bytes();
    assert_eq!(bytes.len(), GENERAL_BATCH_BYTES_V1);
    assert_eq!(GeneralBatchV1::decode(&bytes).expect("decode"), batch);
}

#[test]
fn order_bytes_round_trip_and_carry_their_own_identity() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("decode");
    assert_eq!(bytes.len(), general_order_len_v1(WIDTH).expect("width"));
    assert_eq!(order.order_id(), order.terms().order_id);
    assert_eq!(order.receive_per_lot(0).expect("receive"), 1);
    assert_eq!(order.deliver_per_lot(1).expect("deliver"), 2);
    assert_eq!(order.quote_reserve().expect("quote"), 50);
    assert_eq!(order.claim_reserve(1).expect("claim"), 20);
}

#[test]
fn the_batch_identity_is_fixed_at_open_and_admission_does_not_move_it() {
    // This is the property the whole design rests on: a Candidate names a
    // `batch_id` that was computed before its order set existed.
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let bytes = simple_order(identity, 9, 1);
    batch
        .admit(
            GeneralOrderV1::decode(&bytes).expect("order"),
            funding(9, 100, &[0, 20, 0]),
            10,
        )
        .expect("admit");
    assert_eq!(batch.state().order_count, 1);
    assert_eq!(batch.batch_id(), identity);
}

#[test]
fn two_batches_differing_only_in_sequence_have_different_identities() {
    let mut root = active_root();
    let first = open_batch(&mut root).batch_id();
    let revision = root.revision();
    let mut next = opening();
    next.sequence = 1;
    let second = GeneralBatchV1::open(&mut root, next, revision, 10)
        .expect("second batch")
        .batch_id();
    assert_ne!(first, second);
}

#[test]
fn a_degenerate_order_moving_no_claim_is_refused() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let header = GeneralOrderHeaderV1 {
        outcome_count: WIDTH,
        nonce: 1,
        owner_id: id(9),
        market: id(1),
        batch_id: batch.batch_id(),
        generation: 7,
        max_lots: 10,
        max_quote_debit_per_lot: 5,
        valid_until_slot: SETTLEMENT_CLOSE,
    };
    let mut bytes = vec![0_u8; general_order_len_v1(WIDTH).expect("order width")];
    // The encoder hostile-decodes its own candidate, so a record that would not
    // survive `decode` is refused before any caller can hold one.
    assert_eq!(
        GeneralOrderV1::encode_into(header, &[0, 0, 0], &[0, 0, 0], placed(10), &mut bytes),
        Err(GeneralCollectionErrorV1::ZeroIdentity)
    );
    assert_eq!(
        GeneralOrderV1::decode(&bytes),
        Err(GeneralCollectionErrorV1::ZeroIdentity)
    );
}

#[test]
fn a_truncated_or_repadded_record_is_refused() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let bytes = batch.to_bytes();
    assert_eq!(
        GeneralBatchV1::decode(&bytes[..GENERAL_BATCH_BYTES_V1 - 1]),
        Err(GeneralCollectionErrorV1::InvalidLength)
    );
    let mut noncanonical = bytes;
    noncanonical[196] = 1;
    assert_eq!(
        GeneralBatchV1::decode(&noncanonical),
        Err(GeneralCollectionErrorV1::InvalidHeader)
    );
    // 192..196 is `cancelled_count` now, not padding, and it is bounded by the
    // admission count rather than by a zero rule.
    let mut impossible_cancellations = bytes;
    impossible_cancellations[192] = 1;
    assert_eq!(
        GeneralBatchV1::decode(&impossible_cancellations),
        Err(GeneralCollectionErrorV1::BatchFull)
    );
    let mut wrong_phase = bytes;
    wrong_phase[10] = ORDER_PHASE;
    assert_eq!(
        GeneralBatchV1::decode(&wrong_phase),
        Err(GeneralCollectionErrorV1::InvalidHeader)
    );
}

#[test]
fn an_order_record_is_not_accepted_as_a_batch_record() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    assert_eq!(
        GeneralBatchV1::decode(&bytes),
        Err(GeneralCollectionErrorV1::InvalidLength)
    );
}

// ---------------------------------------------------------------------------
// The root join -- open_batch / close_batch finally have non-test callers
// ---------------------------------------------------------------------------

#[test]
fn opening_a_batch_advances_the_root_exactly_once() {
    let mut root = active_root();
    assert_eq!(root.next_batch_sequence(), 0);
    assert_eq!(root.open_batches(), 0);
    let batch = open_batch(&mut root);
    assert_eq!(root.next_batch_sequence(), 1);
    assert_eq!(root.open_batches(), 1);
    assert_eq!(root.revision(), 2);
    assert_eq!(batch.state().opened_root_revision, 1);
}

#[test]
fn closing_a_batch_returns_the_root_to_a_retirable_state() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let revision = root.revision();
    let identity = batch.close(&mut root, revision).expect("close");
    assert_eq!(identity, batch.batch_id());
    assert_eq!(root.open_batches(), 0);
    assert_eq!(batch.state().status, BatchStatusV1::Closed);
    // The root can now retire, which it could not do while the batch was open.
    let revision = root.revision();
    root.begin_retiring(revision).expect("begin retiring");
    let revision = root.revision();
    root.retire(revision).expect("retire");
}

#[test]
fn a_batch_left_open_blocks_root_retirement() {
    // The reason the collection window is a slot bound rather than an authority
    // key: `retire` refuses while any batch is open, so an unbounded open batch
    // is a permanent denial of the retirement path.
    let mut root = active_root();
    let _batch = open_batch(&mut root);
    let revision = root.revision();
    root.begin_retiring(revision).expect("begin retiring");
    let revision = root.revision();
    assert_eq!(root.retire(revision), Err(RootError::OutstandingBatches));
}

// ---------------------------------------------------------------------------
// Hostile: the five cases the family standard requires
// ---------------------------------------------------------------------------

#[test]
fn hostile_an_order_into_a_closed_batch_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");
    let bytes = simple_order(identity, 9, 1);
    assert_eq!(
        batch.admit(
            GeneralOrderV1::decode(&bytes).expect("order"),
            funding(9, 100, &[0, 20, 0]),
            10,
        ),
        Err(GeneralCollectionErrorV1::NotCollecting)
    );
    assert_eq!(batch.state().order_count, 0);
}

#[test]
fn hostile_an_order_after_the_collection_window_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    assert_eq!(
        batch.admit(
            GeneralOrderV1::decode(&bytes).expect("order"),
            funding(9, 100, &[0, 20, 0]),
            COLLECTION_CLOSE,
        ),
        Err(GeneralCollectionErrorV1::OutsideWindow)
    );
}

#[test]
fn hostile_closing_without_an_open_is_refused_by_the_root() {
    // No batch was ever opened, so `open_batches` is zero and the checked
    // subtraction in `close_batch` is what refuses -- the count is the authority,
    // not a caller-supplied boolean.
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let revision = root.revision();
    batch.close(&mut root, revision).expect("first close");

    let mut second = active_root();
    let mut orphan = open_batch(&mut second);
    // Drain the counter behind the orphan's back, then try to close it.
    let revision = second.revision();
    second.close_batch(revision).expect("drain");
    let revision = second.revision();
    assert_eq!(
        orphan.close(&mut second, revision),
        Err(GeneralCollectionErrorV1::Root(
            RootError::OutstandingBatches
        ))
    );
}

#[test]
fn hostile_a_closed_batch_cannot_be_closed_twice() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");
    let revision = root.revision();
    assert_eq!(
        batch.close(&mut root, revision),
        Err(GeneralCollectionErrorV1::NotCollecting)
    );
}

#[test]
fn hostile_sequence_replay_is_refused_by_the_root_guards() {
    let mut root = active_root();
    let revision = root.revision();
    GeneralBatchV1::open(&mut root, opening(), revision, 10).expect("first open");
    // Replaying the same opening -- same sequence 0 -- after the root advanced.
    let revision = root.revision();
    assert_eq!(
        GeneralBatchV1::open(&mut root, opening(), revision, 10),
        Err(GeneralCollectionErrorV1::Substitution)
    );
    // And replaying the stale revision with the correct next sequence.
    let mut next = opening();
    next.sequence = 1;
    assert_eq!(
        GeneralBatchV1::open(&mut root, next, 1, 10),
        Err(GeneralCollectionErrorV1::Root(
            RootError::CoordinateMismatch
        ))
    );
}

#[test]
fn hostile_a_refused_open_does_not_advance_the_root() {
    let mut root = active_root();
    let revision = root.revision();
    let mut wrong = opening();
    wrong.sequence = 9;
    assert_eq!(
        GeneralBatchV1::open(&mut root, wrong, revision, 10),
        Err(GeneralCollectionErrorV1::Substitution)
    );
    assert_eq!(root.revision(), 1);
    assert_eq!(root.next_batch_sequence(), 0);
    assert_eq!(root.open_batches(), 0);
}

#[test]
fn hostile_a_cross_market_batch_is_refused_at_open_and_at_close() {
    let mut root = active_root();
    let revision = root.revision();
    let mut foreign = opening();
    foreign.market = id(0x5a);
    assert_eq!(
        GeneralBatchV1::open(&mut root, foreign, revision, 10),
        Err(GeneralCollectionErrorV1::Substitution)
    );

    // A batch opened against one market cannot be closed against another root.
    let mut batch = open_batch(&mut root);
    let mut other_market = GeneralRootV2::active(id(0x5a), id(2), 7).expect("other root");
    let revision = other_market.revision();
    assert_eq!(
        batch.close(&mut other_market, revision),
        Err(GeneralCollectionErrorV1::Substitution)
    );
}

#[test]
fn hostile_a_cross_market_order_is_refused_at_admission() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    // A well-formed order for a different market's batch: its `batch_id` is the
    // digest of a different opening, so one comparison catches it.
    let mut foreign_root = GeneralRootV2::active(id(0x5a), id(2), 7).expect("foreign root");
    let mut foreign_opening = opening();
    foreign_opening.market = id(0x5a);
    let revision = foreign_root.revision();
    let foreign_batch = GeneralBatchV1::open(&mut foreign_root, foreign_opening, revision, 10)
        .expect("foreign batch");
    let bytes = order_bytes(
        foreign_batch.batch_id(),
        9,
        1,
        10,
        5,
        &[1, 0, 0],
        &[0, 2, 0],
    );
    assert_eq!(
        batch.admit(
            GeneralOrderV1::decode(&bytes).expect("order"),
            funding(9, 100, &[0, 20, 0]),
            10,
        ),
        Err(GeneralCollectionErrorV1::Substitution)
    );
}

#[test]
fn hostile_an_order_naming_a_foreign_generation_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let mut bytes = simple_order(batch.batch_id(), 9, 1);
    // Substitute the generation in place; the record still decodes.
    bytes[128..136].copy_from_slice(&9_u64.to_le_bytes());
    assert_eq!(
        batch.admit(
            GeneralOrderV1::decode(&bytes).expect("order"),
            funding(9, 100, &[0, 20, 0]),
            10,
        ),
        Err(GeneralCollectionErrorV1::Substitution)
    );
}

#[test]
fn hostile_an_unfunded_order_is_refused_on_quote_and_on_claims_separately() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    // Worst case is 10 lots * 5 quote = 50, and 10 lots * 2 claims at outcome 1.
    assert_eq!(
        batch.admit(order, funding(9, 49, &[0, 20, 0]), 10),
        Err(GeneralCollectionErrorV1::Unfunded)
    );
    assert_eq!(
        batch.admit(order, funding(9, 50, &[0, 19, 0]), 10),
        Err(GeneralCollectionErrorV1::Unfunded)
    );
    // Funding presented for another maker is not this maker's funding.
    assert_eq!(
        batch.admit(order, funding(8, 500, &[0, 200, 0]), 10),
        Err(GeneralCollectionErrorV1::Unfunded)
    );
    assert_eq!(batch.state().order_count, 0);
    batch
        .admit(order, funding(9, 50, &[0, 20, 0]), 10)
        .expect("exactly funded");
    assert_eq!(batch.state().committed_quote_reserve, 50);
}

#[test]
fn hostile_an_order_expiring_before_settlement_closes_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let mut bytes = simple_order(batch.batch_id(), 9, 1);
    bytes[152..160].copy_from_slice(&(SETTLEMENT_CLOSE - 1).to_le_bytes());
    assert_eq!(
        batch.admit(
            GeneralOrderV1::decode(&bytes).expect("order"),
            funding(9, 100, &[0, 20, 0]),
            10,
        ),
        Err(GeneralCollectionErrorV1::Expired)
    );
}

#[test]
fn hostile_the_immutable_order_maximum_is_enforced() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let orders: Vec<Vec<u8>> = (0..5).map(|n| simple_order(identity, 9, n)).collect();
    for bytes in orders.iter().take(4) {
        batch
            .admit(
                GeneralOrderV1::decode(bytes).expect("order"),
                funding(9, 1_000, &[0, 200, 0]),
                10,
            )
            .expect("admit");
    }
    assert_eq!(batch.state().order_count, 4);
    assert_eq!(
        batch.admit(
            GeneralOrderV1::decode(&orders[4]).expect("order"),
            funding(9, 1_000, &[0, 200, 0]),
            10,
        ),
        Err(GeneralCollectionErrorV1::BatchFull)
    );
    assert!(batch.close_is_permissionless(10));
}

#[test]
fn a_partly_filled_batch_is_not_permissionlessly_closable_before_its_window() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    assert!(!batch.close_is_permissionless(10));
    assert!(batch.close_is_permissionless(COLLECTION_CLOSE));
}

// ---------------------------------------------------------------------------
// The join to the settlement half
// ---------------------------------------------------------------------------

#[test]
fn a_candidate_naming_the_closed_batch_authenticates() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");
    let candidate = CandidateHeaderV2 {
        outcome_count: WIDTH,
        page_count: 1,
        candidate_coordinate: 1,
        price_scale: 100,
        candidate_id: id(0x11),
        product_id: id(3),
        batch_id: batch.batch_id(),
    };
    authenticate_batch_candidate_v1(batch, candidate).expect("candidate authenticates");
}

#[test]
fn hostile_a_candidate_naming_a_still_open_batch_is_refused() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let candidate = CandidateHeaderV2 {
        outcome_count: WIDTH,
        page_count: 1,
        candidate_coordinate: 1,
        price_scale: 100,
        candidate_id: id(0x11),
        product_id: id(3),
        batch_id: batch.batch_id(),
    };
    assert_eq!(
        authenticate_batch_candidate_v1(batch, candidate),
        Err(GeneralCollectionErrorV1::NotClosed)
    );
}

#[test]
fn hostile_a_candidate_substituting_product_or_scale_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");
    let base = CandidateHeaderV2 {
        outcome_count: WIDTH,
        page_count: 1,
        candidate_coordinate: 1,
        price_scale: 100,
        candidate_id: id(0x11),
        product_id: id(3),
        batch_id: batch.batch_id(),
    };
    for mutate in [
        |mut header: CandidateHeaderV2| {
            header.product_id = id(0x44);
            header
        },
        |mut header: CandidateHeaderV2| {
            header.price_scale = 99;
            header
        },
        |mut header: CandidateHeaderV2| {
            header.outcome_count = WIDTH + 1;
            header
        },
        |mut header: CandidateHeaderV2| {
            header.batch_id = id(0x77);
            header
        },
    ] {
        assert_eq!(
            authenticate_batch_candidate_v1(batch, mutate(base)),
            Err(GeneralCollectionErrorV1::Substitution)
        );
    }
}

#[test]
fn an_execution_row_projects_the_terms_the_verifier_consumes() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let bytes = simple_order(identity, 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    let execution = execution_bytes(order, 4, &[1, 0, 0], &[0, 2, 0]);
    let terms =
        authenticate_order_execution_v1(batch, order, row(&execution)).expect("terms authenticate");
    // The terms are exactly the record's projection, and the record's digest is
    // the identity the row named -- not a caller assertion.
    assert_eq!(terms, order.terms());
    assert_eq!(terms.order_id, order.order_id());
    assert_eq!(terms.max_lots, 10);
    assert_eq!(terms.max_quote_debit_per_lot, 5);
}

#[test]
fn hostile_a_row_cannot_fill_an_order_with_a_portfolio_its_maker_never_signed() {
    // The defect this refuses: `AuthenticatedOrderTermsV2` carries no per-lot
    // coordinate, and the verifier accumulates claim inputs and outputs from
    // the CANDIDATE PAGE's vectors. While only the compact header fields were
    // bound, a candidate author could pay a maker in the outcome they were
    // delivering and take the one they were buying, at the maker's own signed
    // `max_lots` and quote limit, with a digest that matched.
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let bytes = simple_order(identity, 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    // Honest row: exactly the record's vectors.
    let honest = execution_bytes(order, 4, &[1, 0, 0], &[0, 2, 0]);
    authenticate_order_execution_v1(batch, order, row(&honest)).expect("honest row");

    for (receive, deliver) in [
        // The maker's own vectors, swapped.
        (vec![0, 2, 0], vec![1, 0, 0]),
        // The maker delivers an outcome their order never mentioned.
        (vec![1, 0, 0], vec![0, 2, 3]),
        // The maker receives strictly less than they signed for.
        (vec![0, 0, 0], vec![0, 2, 0]),
        // The maker delivers strictly more than they signed for, which is also
        // strictly more than admission escrowed.
        (vec![1, 0, 0], vec![0, 20, 0]),
    ] {
        let hostile = execution_bytes(order, 4, &receive, &deliver);
        assert_eq!(
            authenticate_order_execution_v1(batch, order, row(&hostile)),
            Err(GeneralCollectionErrorV1::Substitution)
        );
    }
}

#[test]
fn hostile_an_execution_row_cannot_import_terms_from_another_order() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let generous = order_bytes(identity, 9, 1, 1_000, 5_000, &[1, 0, 0], &[0, 2, 0]);
    let modest = simple_order(identity, 9, 2);
    let generous = GeneralOrderV1::decode(&generous).expect("generous");
    let modest = GeneralOrderV1::decode(&modest).expect("modest");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    // A row that names the modest order but is handed the generous record.
    let execution = execution_bytes_from(
        crate::runtime_width::ExecutionHeaderV2 {
            outcome_count: WIDTH,
            page_coordinate: 1,
            execution_coordinate: 1,
            nonce: 2,
            order_id: modest.order_id(),
            owner_id: id(9),
            max_lots: 10,
            lots: 4,
        },
        &[1, 0, 0],
        &[0, 2, 0],
    );
    assert_eq!(
        authenticate_order_execution_v1(batch, generous, row(&execution)),
        Err(GeneralCollectionErrorV1::Substitution)
    );
}

#[test]
fn hostile_an_execution_row_overstating_max_lots_or_fill_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let bytes = simple_order(identity, 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    let base = crate::runtime_width::ExecutionHeaderV2 {
        outcome_count: WIDTH,
        page_coordinate: 1,
        execution_coordinate: 1,
        nonce: 1,
        order_id: order.order_id(),
        owner_id: id(9),
        max_lots: 10,
        lots: 4,
    };
    for mutate in [
        // The row claims a larger candidate-wide maximum than the maker signed.
        |mut header: crate::runtime_width::ExecutionHeaderV2| {
            header.max_lots = 1_000;
            header
        },
        // The row attributes the order to another owner.
        |mut header: crate::runtime_width::ExecutionHeaderV2| {
            header.owner_id = id(8);
            header
        },
        // The row replays another nonce's authorization.
        |mut header: crate::runtime_width::ExecutionHeaderV2| {
            header.nonce = 2;
            header
        },
    ] {
        let hostile = execution_bytes_from(mutate(base), &[1, 0, 0], &[0, 2, 0]);
        assert_eq!(
            authenticate_order_execution_v1(batch, order, row(&hostile)),
            Err(GeneralCollectionErrorV1::Substitution)
        );
    }
    // Two mutations are refused one level lower -- the Execution record itself
    // will not encode them -- so this states where each refusal actually lives
    // instead of asserting the same thing at two layers.
    let mut buffer = vec![0_u8; crate::runtime_width::execution_len(WIDTH).expect("row width")];
    let mut zero_fill = base;
    zero_fill.lots = 0;
    assert_eq!(
        ExecutionV2::encode_into(zero_fill, &[1, 0, 0], &[0, 2, 0], &mut buffer),
        Err(crate::runtime_width::RuntimeWidthErrorV2::ZeroCoordinate)
    );
    let mut overfilled = base;
    overfilled.lots = 11;
    assert_eq!(
        ExecutionV2::encode_into(overfilled, &[1, 0, 0], &[0, 2, 0], &mut buffer),
        Err(crate::runtime_width::RuntimeWidthErrorV2::InvalidCursor)
    );
}

#[test]
fn hostile_an_execution_row_against_an_open_batch_is_refused() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let execution = execution_bytes(order, 4, &[1, 0, 0], &[0, 2, 0]);
    assert_eq!(
        authenticate_order_execution_v1(batch, order, row(&execution)),
        Err(GeneralCollectionErrorV1::NotClosed)
    );
}

#[test]
fn the_worst_case_reserve_cannot_overflow_silently() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let bytes = order_bytes(
        batch.batch_id(),
        9,
        1,
        u64::MAX,
        u64::MAX,
        &[1, 0, 0],
        &[0, 2, 0],
    );
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    assert_eq!(
        order.quote_reserve(),
        Err(GeneralCollectionErrorV1::ArithmeticOverflow)
    );
    assert_eq!(
        order.claim_reserve(1),
        Err(GeneralCollectionErrorV1::ArithmeticOverflow)
    );
}

// ---------------------------------------------------------------------------
// Escrow at admission, and the lifecycle that returns it
//
// Decision 0009 §2 recorded the collect-time External debit as a live credit
// regression and left it open. These are the tests of the thing that replaces
// it: admission MOVES the maker's worst case into the order's own escrow, so
// the only balance a settlement can ever be short of is one the protocol is
// already holding.
// ---------------------------------------------------------------------------

#[test]
fn admission_escrows_the_exact_worst_case_and_says_so() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let escrow = batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");

    // max_lots 10 * max_quote_debit_per_lot 5.
    assert_eq!(escrow.direction, EscrowDirectionV1::Deposit);
    assert_eq!(escrow.quote_atoms, 50);
    assert_eq!(escrow.quote_atoms, order.quote_reserve().expect("reserve"));
    assert_eq!(escrow.order_id, order.order_id());
    assert_eq!(escrow.owner_id, id(9));
    assert_eq!(escrow.outcome_count, WIDTH);
    // The batch counter is now the sum of balances actually held.
    assert_eq!(batch.state().committed_quote_reserve, 50);
    assert_eq!(batch.state().order_count, 1);
    assert_eq!(batch.state().cancelled_count, 0);
}

#[test]
fn cancellation_returns_the_whole_escrow_and_only_the_maker_may_ask() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");

    // Another identity cannot cancel a maker's order, and the refusal names
    // that rather than a generic substitution.
    assert_eq!(
        batch.cancel(order, id(8), 11),
        Err(GeneralCollectionErrorV1::NotTheMaker)
    );
    assert_eq!(batch.state().committed_quote_reserve, 50);

    let refund = batch.cancel(order, id(9), 11).expect("cancel");
    assert_eq!(refund.direction, EscrowDirectionV1::Refund);
    assert_eq!(refund.quote_atoms, 50);
    assert_eq!(refund.order_id, order.order_id());
    assert_eq!(batch.state().committed_quote_reserve, 0);
    assert_eq!(batch.state().cancelled_count, 1);
    // The admission coordinate is NOT returned: an envelope that a maker could
    // churn is one they could use to exhaust another maker's opportunity.
    assert_eq!(batch.state().order_count, 1);
}

#[test]
fn the_cancelled_successor_record_keeps_the_identity_a_candidate_names() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");

    let mut successor = vec![0_u8; bytes.len()];
    order
        .encode_successor_state_into(
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Cancelled,
                admitted_slot: 10,
                released_slot: 11,
            },
            &mut successor,
        )
        .expect("cancelled successor");
    let cancelled = GeneralOrderV1::decode(&successor).expect("cancelled");
    // The identity is the digest of the immutable prefix, so writing the
    // lifecycle tail cannot move what a manifest and a settlement row name.
    assert_eq!(cancelled.order_id(), order.order_id());
    assert_eq!(cancelled.state().phase, GeneralOrderPhaseV1::Cancelled);
    assert_eq!(cancelled.header(), order.header());
    assert_eq!(&successor[..160], &bytes[..160]);
}

#[test]
fn hostile_a_cancelled_order_can_never_be_settled_against() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");
    let mut successor = vec![0_u8; bytes.len()];
    order
        .encode_successor_state_into(
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Cancelled,
                admitted_slot: 10,
                released_slot: 11,
            },
            &mut successor,
        )
        .expect("cancelled successor");
    let cancelled = GeneralOrderV1::decode(&successor).expect("cancelled");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    // A candidate built while the batch was still collecting can still name
    // this order. Its escrow has been returned, so the row must refuse -- and
    // it refuses on the PHASE, not on any coordinate, because every coordinate
    // still matches.
    let execution = execution_bytes(cancelled, 4, &[1, 0, 0], &[0, 2, 0]);
    assert_eq!(
        authenticate_order_execution_v1(batch, cancelled, row(&execution)),
        Err(GeneralCollectionErrorV1::InvalidOrderPhase)
    );
}

#[test]
fn hostile_a_second_cancellation_or_a_late_one_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");
    let mut successor = vec![0_u8; bytes.len()];
    order
        .encode_successor_state_into(
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Cancelled,
                admitted_slot: 10,
                released_slot: 11,
            },
            &mut successor,
        )
        .expect("cancelled successor");
    let cancelled = GeneralOrderV1::decode(&successor).expect("cancelled");

    // The double refund: the record has already left the Placed phase.
    assert_eq!(
        batch.cancel(cancelled, id(9), 12),
        Err(GeneralCollectionErrorV1::InvalidOrderPhase)
    );
    // A cancelled record cannot be written a second time either.
    let mut again = vec![0_u8; bytes.len()];
    assert_eq!(
        cancelled.encode_successor_state_into(
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Released,
                admitted_slot: 10,
                released_slot: 12,
            },
            &mut again,
        ),
        Err(GeneralCollectionErrorV1::InvalidOrderPhase)
    );
    // After the collection window the order set is final and a candidate may
    // already be built against it; cancelling then would settle a candidate
    // against collateral that had left.
    assert_eq!(
        batch.cancel(order, id(9), COLLECTION_CLOSE),
        Err(GeneralCollectionErrorV1::OutsideWindow)
    );
}

#[test]
fn hostile_a_cancellation_cannot_cross_batches() {
    let mut root = active_root();
    let mut first = open_batch(&mut root);
    let bytes = simple_order(first.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    first
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");
    let revision = root.revision();
    first.close(&mut root, revision).expect("close");

    let mut second_opening = opening();
    second_opening.sequence = 1;
    let revision = root.revision();
    let mut second =
        GeneralBatchV1::open(&mut root, second_opening, revision, 10).expect("second batch");
    // A refund is a debit against the escrow of the batch that holds it.
    assert_eq!(
        second.cancel(order, id(9), 11),
        Err(GeneralCollectionErrorV1::Substitution)
    );
    assert_eq!(second.state().committed_quote_reserve, 0);
}

#[test]
fn a_release_after_the_settlement_window_returns_whatever_is_left() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    // Before the window ends a candidate may still be settling against it.
    assert_eq!(
        batch.release(order, SETTLEMENT_CLOSE - 1),
        Err(GeneralCollectionErrorV1::OutsideWindow)
    );
    let residual = batch.release(order, SETTLEMENT_CLOSE).expect("release");
    assert_eq!(residual.direction, EscrowDirectionV1::Residual);
    // Deliberately not a computed number: whatever a winning candidate
    // collected already left the escrow, so the remaining balance IS the
    // refund, and quoting a second figure would be a second authority over it.
    assert_eq!(residual.quote_atoms, 0);
    assert_eq!(residual.order_id, order.order_id());
    assert_eq!(residual.owner_id, id(9));
}

#[test]
fn a_candidate_cannot_debit_more_quote_than_the_batch_escrowed() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit");
    let identity = batch.batch_id();
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    let header = |quote_debit: u64| VerifiedCandidateHeaderV2 {
        outcome_count: WIDTH,
        page_count: 1,
        candidate_coordinate: 1,
        revision: 1,
        candidate_id: id(40),
        product_id: id(3),
        batch_id: identity,
        filled_lots: 4,
        quote_debit,
        quote_credit: 0,
        price_scale: 100,
    };
    // Exactly the escrow is fine; one atom past it is a candidate that could
    // not be paid, and it is refused before any settlement account exists
    // rather than stranding at the first short Collect.
    authenticate_batch_verified_candidate_v1(batch, header(50)).expect("inside the escrow");
    assert_eq!(
        authenticate_batch_verified_candidate_v1(batch, header(51)),
        Err(GeneralCollectionErrorV1::EscrowShortfall)
    );
}

#[test]
fn a_cancellation_lowers_the_ceiling_a_candidate_must_fit_inside() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let identity = batch.batch_id();
    let first = simple_order(identity, 9, 1);
    let second = simple_order(identity, 8, 2);
    let first = GeneralOrderV1::decode(&first).expect("first");
    let second = GeneralOrderV1::decode(&second).expect("second");
    batch
        .admit(first, funding(9, 100, &[0, 20, 0]), 10)
        .expect("admit first");
    batch
        .admit(second, funding(8, 100, &[0, 20, 0]), 10)
        .expect("admit second");
    assert_eq!(batch.state().committed_quote_reserve, 100);
    batch.cancel(second, id(8), 11).expect("cancel second");
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");

    let header = |quote_debit: u64| VerifiedCandidateHeaderV2 {
        outcome_count: WIDTH,
        page_count: 1,
        candidate_coordinate: 1,
        revision: 1,
        candidate_id: id(40),
        product_id: id(3),
        batch_id: identity,
        filled_lots: 4,
        quote_debit,
        quote_credit: 0,
        price_scale: 100,
    };
    // The refunded maker's escrow is gone, so the candidate ceiling went with
    // it: a candidate sized against the pre-cancellation batch is refused.
    authenticate_batch_verified_candidate_v1(batch, header(50)).expect("inside the escrow");
    assert_eq!(
        authenticate_batch_verified_candidate_v1(batch, header(100)),
        Err(GeneralCollectionErrorV1::EscrowShortfall)
    );
}
