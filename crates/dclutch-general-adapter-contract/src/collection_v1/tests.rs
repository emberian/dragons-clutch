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
        &mut bytes,
    )
    .expect("order bytes");
    bytes
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
        GeneralOrderV1::encode_into(header, &[0, 0, 0], &[0, 0, 0], &mut bytes),
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
    noncanonical[192] = 1;
    assert_eq!(
        GeneralBatchV1::decode(&noncanonical),
        Err(GeneralCollectionErrorV1::InvalidHeader)
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

    let execution = ExecutionHeaderV2 {
        outcome_count: WIDTH,
        page_coordinate: 1,
        execution_coordinate: 1,
        nonce: 1,
        order_id: order.order_id(),
        owner_id: id(9),
        max_lots: 10,
        lots: 4,
    };
    let terms =
        authenticate_order_execution_v1(batch, order, execution).expect("terms authenticate");
    // The terms are exactly the record's projection, and the record's digest is
    // the identity the row named -- not a caller assertion.
    assert_eq!(terms, order.terms());
    assert_eq!(terms.order_id, order.order_id());
    assert_eq!(terms.max_lots, 10);
    assert_eq!(terms.max_quote_debit_per_lot, 5);
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
    let execution = ExecutionHeaderV2 {
        outcome_count: WIDTH,
        page_coordinate: 1,
        execution_coordinate: 1,
        nonce: 2,
        order_id: modest.order_id(),
        owner_id: id(9),
        max_lots: 10,
        lots: 4,
    };
    assert_eq!(
        authenticate_order_execution_v1(batch, generous, execution),
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

    let base = ExecutionHeaderV2 {
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
        |mut header: ExecutionHeaderV2| {
            header.max_lots = 1_000;
            header
        },
        // The row fills more than the maker's whole order.
        |mut header: ExecutionHeaderV2| {
            header.lots = 11;
            header
        },
        // A zero fill occupies a coordinate and moves nothing.
        |mut header: ExecutionHeaderV2| {
            header.lots = 0;
            header
        },
        // The row attributes the order to another owner.
        |mut header: ExecutionHeaderV2| {
            header.owner_id = id(8);
            header
        },
        // The row replays another nonce's authorization.
        |mut header: ExecutionHeaderV2| {
            header.nonce = 2;
            header
        },
    ] {
        assert_eq!(
            authenticate_order_execution_v1(batch, order, mutate(base)),
            Err(GeneralCollectionErrorV1::Substitution)
        );
    }
}

#[test]
fn hostile_an_execution_row_against_an_open_batch_is_refused() {
    let mut root = active_root();
    let mut batch = open_batch(&mut root);
    let bytes = simple_order(batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let execution = ExecutionHeaderV2 {
        outcome_count: WIDTH,
        page_coordinate: 1,
        execution_coordinate: 1,
        nonce: 1,
        order_id: order.order_id(),
        owner_id: id(9),
        max_lots: 10,
        lots: 4,
    };
    assert_eq!(
        authenticate_order_execution_v1(batch, order, execution),
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
