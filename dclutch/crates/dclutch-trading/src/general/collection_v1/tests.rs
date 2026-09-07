//! Hostile coverage for General batch collection.
//!
//! Every refusal below is stated against the exact variant, not against "an
//! error", so a later change that keeps the record refusing for a different
//! reason is a visible test failure rather than a silent reinterpretation.

use std::vec;
use std::vec::Vec;

use crate::general_config::root::{GeneralRootV2, RootError};

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
    order_bytes_with_floor(
        batch_id,
        owner,
        nonce,
        max_lots,
        max_quote_debit_per_lot,
        0,
        receive,
        deliver,
    )
}

/// One order carrying a SELLER'S FLOOR, which `order_bytes` leaves at zero.
///
/// Written as a sibling rather than a ninth parameter on `order_bytes` so that
/// every existing caller keeps saying exactly what it said: a zero floor is the
/// record's reserved-zero bytes and its old `order_id`, and a test that had to
/// pass `0` to keep its meaning would be evidence that the field was not
/// additive after all.
#[allow(clippy::too_many_arguments)]
fn order_bytes_with_floor(
    batch_id: [u8; 32],
    owner: u8,
    nonce: u64,
    max_lots: u64,
    max_quote_debit_per_lot: u64,
    min_quote_credit_per_lot: u64,
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
            min_quote_credit_per_lot,
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
        crate::general::runtime_width::ExecutionHeaderV2 {
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
    header: crate::general::runtime_width::ExecutionHeaderV2,
    receive: &[u64],
    deliver: &[u64],
) -> Vec<u8> {
    let mut bytes = vec![
        0_u8;
        crate::general::runtime_width::execution_len(header.outcome_count)
            .expect("row width")
    ];
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
fn batch_occurrence_terms_are_canonical_and_exclude_runtime_slots_only() {
    let original = opening();
    let terms = GeneralBatchOccurrenceTermsV1::new(original).expect("occurrence terms");
    let bytes = terms.to_bytes();
    let decoded = GeneralBatchOccurrenceTermsV1::decode(&bytes).expect("canonical terms");
    assert_eq!(decoded, terms);
    assert_eq!(decoded.occurrence_id(), terms.occurrence_id());
    assert_eq!(decoded.opening().collection_close_slot, 0);
    assert_eq!(decoded.opening().settlement_close_slot, 0);

    assert_eq!(
        GeneralBatchOccurrenceTermsV1::decode(&bytes[..bytes.len() - 1]),
        Err(GeneralCollectionErrorV1::InvalidLength)
    );
    let mut reserved = bytes;
    reserved[GeneralBatchOccurrenceTermsLayoutV1::RESERVED_B] = 1;
    assert_eq!(
        GeneralBatchOccurrenceTermsV1::decode(&reserved),
        Err(GeneralCollectionErrorV1::InvalidHeader)
    );

    let mut later_window = original;
    later_window.collection_close_slot += 100;
    later_window.settlement_close_slot += 100;
    assert_eq!(
        GeneralBatchOccurrenceTermsV1::new(later_window)
            .expect("later runtime window")
            .occurrence_id(),
        terms.occurrence_id()
    );

    for mut substituted in [
        GeneralBatchOpeningV1 {
            outcome_count: original.outcome_count + 1,
            ..original
        },
        GeneralBatchOpeningV1 {
            sequence: original.sequence + 1,
            ..original
        },
        GeneralBatchOpeningV1 {
            generation: original.generation + 1,
            ..original
        },
        GeneralBatchOpeningV1 {
            market: id(90),
            ..original
        },
        GeneralBatchOpeningV1 {
            product_id: id(91),
            ..original
        },
        GeneralBatchOpeningV1 {
            config_id: id(92),
            ..original
        },
        GeneralBatchOpeningV1 {
            price_scale: original.price_scale + 1,
            ..original
        },
        GeneralBatchOpeningV1 {
            max_orders: original.max_orders + 1,
            ..original
        },
    ] {
        substituted.collection_close_slot = original.collection_close_slot;
        substituted.settlement_close_slot = original.settlement_close_slot;
        assert_ne!(
            GeneralBatchOccurrenceTermsV1::new(substituted)
                .expect("substituted stable term")
                .occurrence_id(),
            terms.occurrence_id()
        );
    }
}

#[test]
fn the_same_batch_occurrence_cannot_reopen_with_a_substituted_runtime_window() {
    let mut root = active_root();
    let first_revision = root.revision();
    let first =
        GeneralBatchV1::open(&mut root, opening(), first_revision, 10).expect("first occurrence");
    let root_after_first = root;
    let mut replay = opening();
    replay.collection_close_slot += 100;
    replay.settlement_close_slot += 100;
    assert_eq!(
        GeneralBatchOccurrenceTermsV1::new(replay)
            .expect("same occurrence terms")
            .occurrence_id(),
        first.batch_id()
    );
    assert_eq!(
        GeneralBatchV1::open(&mut root, replay, first_revision, 20),
        Err(GeneralCollectionErrorV1::Substitution)
    );
    assert_eq!(root, root_after_first);
}

#[test]
fn atomic_physical_admission_matches_funded_semantics_and_refuses_a_closed_window() {
    let mut funded_root = active_root();
    let mut funded_batch = open_batch(&mut funded_root);
    let bytes = simple_order(funded_batch.batch_id(), 9, 1);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let mut signed_bytes = vec![0; general_signed_order_terms_len_v1(WIDTH).expect("signed width")];
    order
        .encode_signed_terms_into(&mut signed_bytes)
        .expect("signed immutable terms");
    let signed = GeneralSignedOrderTermsV1::decode(&signed_bytes).expect("signed terms");
    assert_eq!(signed.order_id(), order.order_id());

    assert_eq!(
        GeneralSignedOrderTermsV1::decode(&signed_bytes[..signed_bytes.len() - 1]),
        Err(GeneralCollectionErrorV1::InvalidLength)
    );
    // Byte 24 was the reserved window and refused nonzero; since 2026-09-04 it
    // is the low byte of the seller's floor, so it decodes and takes the
    // identity with it. The signed image is what the maker signs, so a floor
    // that could be attached to it without moving `order_id` would be a term
    // the maker never agreed to.
    let mut floored = signed_bytes.clone();
    floored[GeneralOrderLayoutV1::MIN_QUOTE_CREDIT_PER_LOT] = 1;
    let floored_terms = GeneralSignedOrderTermsV1::decode(&floored).expect("floored signed terms");
    assert_eq!(floored_terms.header().min_quote_credit_per_lot, 1);
    assert_ne!(floored_terms.order_id(), order.order_id());
    let mut substituted_row = signed_bytes.clone();
    let last = substituted_row.len() - 1;
    substituted_row[last] ^= 1;
    let substituted =
        GeneralSignedOrderTermsV1::decode(&substituted_row).expect("canonical substituted row");
    assert_ne!(substituted.order_id(), order.order_id());

    let mut physical_batch = funded_batch;
    let funded = funded_batch
        .admit(order, funding(9, 100, &[0, 20, 0]), 10)
        .expect("funded semantic admission");
    let physical = physical_batch
        .admit_signed_for_atomic_physical_escrow(signed, 10)
        .expect("physical executor admission");
    assert_eq!(physical, funded);
    assert_eq!(physical_batch, funded_batch);

    let mut stale_root = active_root();
    let mut stale_batch = open_batch(&mut stale_root);
    let stale_before = stale_batch;
    assert_eq!(
        stale_batch.admit_signed_for_atomic_physical_escrow(signed, COLLECTION_CLOSE),
        Err(GeneralCollectionErrorV1::OutsideWindow)
    );
    assert_eq!(stale_batch, stale_before);
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
        min_quote_credit_per_lot: 0,
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

/// THE SELLER'S FLOOR IS INSIDE THE ORDER'S OWN IDENTITY, and a floorless order
/// is the bytes it always was.
///
/// Two statements, and the second is what makes the first additive rather than
/// a wire break. (a) The eight bytes at `MIN_QUOTE_CREDIT_PER_LOT` were
/// `require_zero(24, 8)` from the day this record was written, so an order with
/// no floor writes the same zeros the reserved window held and keeps its
/// `order_id` -- every order signed before 2026-09-04 authenticates unchanged.
/// (b) An order that DOES carry a floor has a different `order_id`, because the
/// identity digest covers the whole header, so the floor cannot be attached to
/// or stripped from a record without the row that names it ceasing to match.
/// That is the whole authentication story for this field: it needs no register
/// and no profile operation, only the digest the row already binds.
#[test]
fn the_sellers_floor_rides_the_order_identity_and_a_floorless_order_is_unmoved() {
    let mut root = active_root();
    let batch = open_batch(&mut root);
    let identity = batch.batch_id();

    let floorless = order_bytes(identity, 9, 1, 10, 5, &[1, 0, 0], &[0, 2, 0]);
    assert_eq!(
        floorless
            .get(
                GeneralOrderLayoutV1::MIN_QUOTE_CREDIT_PER_LOT
                    ..GeneralOrderLayoutV1::MIN_QUOTE_CREDIT_PER_LOT + 8
            )
            .expect("floor window"),
        &[0; 8],
        "a zero floor must write the bytes the reserved window held",
    );
    let without = GeneralOrderV1::decode(&floorless).expect("floorless order");
    assert_eq!(without.header().min_quote_credit_per_lot, 0);
    assert_eq!(without.terms().min_quote_credit_per_lot, 0);

    let floored = order_bytes_with_floor(identity, 9, 1, 10, 5, 3, &[1, 0, 0], &[0, 2, 0]);
    let with = GeneralOrderV1::decode(&floored).expect("floored order");
    assert_eq!(with.header().min_quote_credit_per_lot, 3);
    assert_eq!(with.terms().min_quote_credit_per_lot, 3);
    assert_ne!(
        with.order_id(),
        without.order_id(),
        "the floor is not covered by the identity the row binds",
    );

    // And the signed image the maker actually signs carries it too: the signed
    // terms ARE the record's header plus its rows, so a floor stated to the
    // chain and a floor stated to the maker cannot differ.
    let mut signed =
        vec![0_u8; general_signed_order_terms_len_v1(WIDTH).expect("signed terms width")];
    with.encode_signed_terms_into(&mut signed)
        .expect("signed terms");
    let terms = GeneralSignedOrderTermsV1::decode(&signed).expect("signed terms decode");
    assert_eq!(terms.header().min_quote_credit_per_lot, 3);
    assert_eq!(terms.order_id(), with.order_id());
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
        crate::general::runtime_width::ExecutionHeaderV2 {
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

    let base = crate::general::runtime_width::ExecutionHeaderV2 {
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
        |mut header: crate::general::runtime_width::ExecutionHeaderV2| {
            header.max_lots = 1_000;
            header
        },
        // The row attributes the order to another owner.
        |mut header: crate::general::runtime_width::ExecutionHeaderV2| {
            header.owner_id = id(8);
            header
        },
        // The row replays another nonce's authorization.
        |mut header: crate::general::runtime_width::ExecutionHeaderV2| {
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
    let mut buffer =
        vec![0_u8; crate::general::runtime_width::execution_len(WIDTH).expect("row width")];
    let mut zero_fill = base;
    zero_fill.lots = 0;
    assert_eq!(
        ExecutionV2::encode_into(zero_fill, &[1, 0, 0], &[0, 2, 0], &mut buffer),
        Err(crate::general::runtime_width::RuntimeWidthErrorV2::ZeroCoordinate)
    );
    let mut overfilled = base;
    overfilled.lots = 11;
    assert_eq!(
        ExecutionV2::encode_into(overfilled, &[1, 0, 0], &[0, 2, 0], &mut buffer),
        Err(crate::general::runtime_width::RuntimeWidthErrorV2::InvalidCursor)
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

// ---------------------------------------------------------------------------
// The order wire: fixed-offset mutable state, interleaved rows, masked digest
// ---------------------------------------------------------------------------

/// Pin the wire the order-action EffectPrograms write by.
///
/// The mutable escrow window sits at the FIXED offsets `GeneralOrderLayoutV1`
/// names, and every per-outcome row lies at `192 + 16 * i` with the receive
/// and deliver quantities at fixed intra-row offsets. If any of these move,
/// the fixed-offset artifact writes silently write the wrong field, so this
/// test states the coordinates as bytes rather than trusting the codec to
/// agree with itself.
#[test]
fn the_order_wire_is_fixed_state_then_interleaved_rows() {
    let receive = [11_u64, 0, 13];
    let deliver = [0_u64, 22, 23];
    let bytes = order_bytes(id(9), 4, 5, 6, 7, &receive, &deliver);
    assert_eq!(bytes.len(), 192 + 16 * WIDTH as usize);
    assert_eq!(
        bytes[GeneralOrderLayoutV1::STATE_PHASE],
        GeneralOrderPhaseV1::Placed.tag()
    );
    assert_eq!(GeneralOrderLayoutV1::STATE_PHASE, 160);
    assert_eq!(GeneralOrderLayoutV1::STATE_ADMITTED_SLOT, 168);
    assert_eq!(GeneralOrderLayoutV1::STATE_RELEASED_SLOT, 176);
    assert_eq!(bytes[168..176], 10_u64.to_le_bytes());
    assert_eq!(bytes[176..184], 0_u64.to_le_bytes());
    assert!(bytes[161..168].iter().all(|byte| *byte == 0));
    assert!(bytes[184..192].iter().all(|byte| *byte == 0));
    assert!(bytes[24..32].iter().all(|byte| *byte == 0));
    for outcome in 0..WIDTH as usize {
        let row = 192 + 16 * outcome;
        assert_eq!(bytes[row..row + 8], receive[outcome].to_le_bytes());
        assert_eq!(bytes[row + 8..row + 16], deliver[outcome].to_le_bytes());
    }
}

/// The identity masks exactly the 32-byte mutable window and nothing more.
///
/// The successor half proves the mask covers the window: a lifecycle write
/// leaves the identity fixed. The two flip halves prove it covers NOTHING
/// more: the last header byte before the window and the first row byte after
/// it both move the digest, so a mask one byte too wide on either side is a
/// red test rather than a silently forgeable coordinate.
#[test]
fn the_order_identity_masks_exactly_the_mutable_window() {
    let bytes = order_bytes(id(9), 4, 5, 6, 7, &[1, 0, 0], &[0, 2, 0]);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let identity = general_order_identity_v1(&bytes).expect("identity");
    assert_eq!(order.order_id(), identity);

    let mut released = bytes.clone();
    order
        .encode_successor_state_into(
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Released,
                admitted_slot: 10,
                released_slot: SETTLEMENT_CLOSE,
            },
            &mut released,
        )
        .expect("successor");
    assert_ne!(released, bytes);
    assert_eq!(
        general_order_identity_v1(&released).expect("identity"),
        identity
    );

    let mut header_flip = bytes.clone();
    header_flip[GENERAL_ORDER_STATE_OFFSET_V1 - 1] ^= 1;
    assert_ne!(
        general_order_identity_v1(&header_flip).expect("identity"),
        identity
    );

    let mut row_flip = bytes;
    row_flip[GENERAL_ORDER_ROW_BASE_V1] ^= 1;
    assert_ne!(
        general_order_identity_v1(&row_flip).expect("identity"),
        identity
    );
}

/// The range behind the nonce is a FIELD now, and still not a free byte.
///
/// Before the wire repair bytes 24..32 were never checked, so two encodings of
/// one signed order could carry two identities; the repair made them reserved
/// zero and refused any other content. On 2026-09-04 they became
/// `MIN_QUOTE_CREDIT_PER_LOT`, and the property this test was written for is
/// unchanged and stronger: a content-addressed record may not have a free byte,
/// so a byte that changes must change the identity. What moved is only the
/// verdict -- these bytes now DECODE instead of refusing, and the record they
/// decode to is a different order.
#[test]
fn the_range_behind_the_nonce_is_the_floor_and_moves_the_identity() {
    let bytes = order_bytes(id(9), 4, 5, 6, 7, &[1, 0, 0], &[0, 2, 0]);
    let mut floored = bytes.clone();
    floored[GeneralOrderLayoutV1::MIN_QUOTE_CREDIT_PER_LOT] = 1;
    let without = GeneralOrderV1::decode(&bytes).expect("floorless order");
    let with = GeneralOrderV1::decode(&floored).expect("floored order");
    assert_eq!(without.header().min_quote_credit_per_lot, 0);
    assert_eq!(with.header().min_quote_credit_per_lot, 1);
    assert_ne!(with.order_id(), without.order_id());
}

#[test]
fn physical_residual_release_uses_the_orders_pinned_window_without_a_batch_projection() {
    let bytes = order_bytes(id(9), 4, 5, 6, 7, &[1, 0, 0], &[0, 2, 0]);
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    assert_eq!(
        authenticate_order_residual_release_v1(order, SETTLEMENT_CLOSE - 1),
        Err(GeneralCollectionErrorV1::OutsideWindow)
    );
    let residual = authenticate_order_residual_release_v1(order, SETTLEMENT_CLOSE)
        .expect("permissionless residual");
    assert_eq!(residual.order_id, order.order_id());
    assert_eq!(residual.owner_id, order.header().owner_id);
    assert_eq!(residual.quote_atoms, 0);
    assert_eq!(residual.direction, EscrowDirectionV1::Residual);

    let mut cancelled = vec![0; bytes.len()];
    order
        .encode_successor_state_into(
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Cancelled,
                admitted_slot: order.state().admitted_slot,
                released_slot: SETTLEMENT_CLOSE,
            },
            &mut cancelled,
        )
        .expect("cancelled successor");
    assert_eq!(
        authenticate_order_residual_release_v1(
            GeneralOrderV1::decode(&cancelled).expect("cancelled order"),
            SETTLEMENT_CLOSE,
        ),
        Err(GeneralCollectionErrorV1::InvalidOrderPhase)
    );
}
