//! Hostile coverage for candidate submission and on-chain page verification.
//!
//! The centrepiece is `the_whole_candidate_life_runs_through_the_evaluator`: a
//! real batch, really placed and escrowed orders, a real candidate whose
//! identity is its own digest, every row driven through
//! `evaluate_runtime_consider_row_with_manifest_v2`, and the certificate that
//! falls out fed to the selection cursor that was waiting for a writer.

use std::vec;
use std::vec::Vec;

use crate::general_codec::{MAX_SELECTION_CRITERIA, SelectionCriterion, SelectionPolicyV1};
use crate::general_config::root::GeneralRootV2;

use super::*;
use crate::general::collection_v1::{
    GeneralBatchOpeningV1, GeneralOrderHeaderV1, GeneralOrderPhaseV1, GeneralOrderStateV1,
    MakerFundingV1, general_order_len_v1,
};
use crate::general::runtime_manifest::settlement_manifest_len_v2;
use crate::general::runtime_selection::{
    RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2, consider_verified_candidate_v2,
};
use crate::general::runtime_width::{
    CandidateHeaderV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2, candidate_len,
    execution_len, page_len,
};

const WIDTH: u32 = 3;
const PRICE_SCALE: u64 = 100;
const COLLECTION_CLOSE: u64 = 1_000;
const SETTLEMENT_CLOSE: u64 = 2_000;
const ADMISSION_SLOT: u64 = 10;
const SUBMISSION_SLOT: u64 = 1_100;
const PAGE_REVISION: u64 = 9;
const REWARD_RATE: u64 = 5_000;
const ROW_COUNT: u32 = 2;

/// Advance one submission's work escrow as if `cranks` rows had been verified.
///
/// The record layer is the authority for this, so the helper round-trips
/// through the canonical bytes rather than reaching into private state.
fn spend_verification_cranks(submission: GeneralCandidateV1, cranks: u32) -> GeneralCandidateV1 {
    let mut bytes = submission.to_bytes();
    let spent = u64::from(cranks) * submission.opening().reward_rate_lamports;
    let remaining = submission.state().verification_remaining - spent;
    bytes[192..200].copy_from_slice(&remaining.to_le_bytes());
    GeneralCandidateV1::decode(&bytes).expect("canonical spent submission")
}

/// Submit with this module's canonical work-escrow parameters.
fn submit_at(
    batch: GeneralBatchV1,
    candidate: CandidateV2<'_>,
    funded_lamports: u64,
    slot: u64,
) -> GeneralCandidateResultV1<GeneralCandidateV1> {
    GeneralCandidateV1::submit(
        batch,
        candidate,
        PAGE_REVISION,
        ROW_COUNT,
        REWARD_RATE,
        id(40),
        funded_lamports,
        slot,
    )
}

fn id(low: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value[0] = low;
    value
}

fn opening() -> GeneralBatchOpeningV1 {
    GeneralBatchOpeningV1 {
        outcome_count: WIDTH,
        sequence: 0,
        generation: 7,
        market: id(1),
        product_id: id(3),
        config_id: id(2),
        price_scale: PRICE_SCALE,
        collection_close_slot: COLLECTION_CLOSE,
        settlement_close_slot: SETTLEMENT_CLOSE,
        max_orders: 4,
    }
}

/// One maker's order, placed and escrowed against a live batch.
fn place(
    batch: &mut GeneralBatchV1,
    owner: u8,
    nonce: u64,
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
            batch_id: batch.batch_id(),
            generation: 7,
            max_lots: 10,
            max_quote_debit_per_lot: 5,
            min_quote_credit_per_lot: 0,
            valid_until_slot: SETTLEMENT_CLOSE,
        },
        receive,
        deliver,
        GeneralOrderStateV1 {
            phase: GeneralOrderPhaseV1::Placed,
            admitted_slot: ADMISSION_SLOT,
            released_slot: 0,
        },
        &mut bytes,
    )
    .expect("order record");
    let order = GeneralOrderV1::decode(&bytes).expect("order");
    let claims: Vec<u64> = (0..WIDTH)
        .map(|index| order.claim_reserve(index).expect("reserve"))
        .collect();
    batch
        .admit(
            order,
            MakerFundingV1 {
                owner_id: id(owner),
                available_quote: 1_000,
                available_claims: &claims,
            },
            ADMISSION_SLOT,
        )
        .expect("admit and escrow");
    bytes
}

/// Build a candidate whose declared identity IS its own masked digest.
fn candidate_bytes(batch_id: [u8; 32], page_count: u32, prices: &[u64]) -> Vec<u8> {
    let mut bytes = vec![0_u8; candidate_len(WIDTH).expect("candidate width")];
    let header = CandidateHeaderV2 {
        outcome_count: WIDTH,
        page_count,
        candidate_coordinate: 1,
        price_scale: PRICE_SCALE,
        // A placeholder: the record is encoded once to fix every other byte,
        // then re-encoded with the digest those bytes produce. That is the only
        // way a self-describing record can carry its own identity, and it is
        // exactly what `general_candidate_identity_v1` masks out.
        candidate_id: id(0xff),
        product_id: id(3),
        batch_id,
    };
    CandidateV2::encode_into(header, prices, &mut bytes).expect("draft candidate");
    let identity = general_candidate_identity_v1(&bytes).expect("identity");
    CandidateV2::encode_into(
        CandidateHeaderV2 {
            candidate_id: identity,
            ..header
        },
        prices,
        &mut bytes,
    )
    .expect("addressed candidate");
    bytes
}

fn row_bytes(
    order_bytes: &[u8],
    page_coordinate: u32,
    execution_coordinate: u32,
    lots: u64,
) -> Vec<u8> {
    let order = GeneralOrderV1::decode(order_bytes).expect("order");
    let header = order.header();
    let receive: Vec<u64> = (0..WIDTH)
        .map(|index| order.receive_per_lot(index).expect("receive"))
        .collect();
    let deliver: Vec<u64> = (0..WIDTH)
        .map(|index| order.deliver_per_lot(index).expect("deliver"))
        .collect();
    let mut bytes = vec![0_u8; execution_len(WIDTH).expect("row width")];
    ExecutionV2::encode_into(
        ExecutionHeaderV2 {
            outcome_count: WIDTH,
            page_coordinate,
            execution_coordinate,
            nonce: header.nonce,
            order_id: order.order_id(),
            owner_id: header.owner_id,
            max_lots: header.max_lots,
            lots,
        },
        &receive,
        &deliver,
        &mut bytes,
    )
    .expect("row bytes");
    bytes
}

fn page_bytes(candidate_id: [u8; 32], coordinate: u32, page_count: u32, rows: &[&[u8]]) -> Vec<u8> {
    let count = u32::try_from(rows.len()).expect("row count");
    let mut bytes = vec![0_u8; page_len(WIDTH, count).expect("page width")];
    PageV2::encode_into(
        PageHeaderV2 {
            outcome_count: WIDTH,
            page_coordinate: coordinate,
            page_count,
            revision: PAGE_REVISION,
            candidate_id,
        },
        rows,
        &mut bytes,
    )
    .expect("page bytes");
    bytes
}

/// A batch with two escrowed orders whose portfolios net to a complete set.
struct Fixture {
    batch: GeneralBatchV1,
    orders: Vec<Vec<u8>>,
    candidate: Vec<u8>,
    pages: Vec<Vec<u8>>,
    submission: GeneralCandidateV1,
}

fn fixture() -> Fixture {
    let mut root = GeneralRootV2::active(id(1), id(2), 7).expect("root");
    let revision = root.revision();
    let mut batch =
        GeneralBatchV1::open(&mut root, opening(), revision, ADMISSION_SLOT).expect("open batch");

    // Two orders that trade opposite sides of the same outcome pair, so the
    // candidate's aggregate claim delta is uniform and the balance derives.
    let first = place(&mut batch, 9, 1, &[1, 0, 0], &[0, 1, 0]);
    let second = place(&mut batch, 8, 2, &[0, 1, 0], &[1, 0, 0]);
    let revision = root.revision();
    batch.close(&mut root, revision).expect("close batch");

    let candidate = candidate_bytes(batch.batch_id(), 1, &[40, 60, 0]);
    let candidate_id = CandidateV2::decode(&candidate)
        .expect("candidate")
        .header()
        .candidate_id;

    // Rows must be grouped by the PROTOCOL's identity order, which is
    // little-endian, not `[u8; 32]`'s `Ord`.
    let mut order_bytes = vec![first, second];
    order_bytes.sort_by(|left, right| {
        let left_id = GeneralOrderV1::decode(left).expect("left").order_id();
        let right_id = GeneralOrderV1::decode(right).expect("right").order_id();
        if left_id == right_id {
            core::cmp::Ordering::Equal
        } else if crate::general::runtime_verify::runtime_identity_precedes_v2(&left_id, &right_id) {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Greater
        }
    });

    let rows: Vec<Vec<u8>> = order_bytes
        .iter()
        .enumerate()
        .map(|(index, bytes)| row_bytes(bytes, 1, u32::try_from(index).expect("row") + 1, 4))
        .collect();
    let row_refs: Vec<&[u8]> = rows.iter().map(Vec::as_slice).collect();
    let page = page_bytes(candidate_id, 1, 1, &row_refs);

    let opening_probe = GeneralCandidateOpeningV1 {
        outcome_count: WIDTH,
        page_count: 1,
        page_revision: PAGE_REVISION,
        submitted_slot: SUBMISSION_SLOT,
        candidate_id,
        batch_id: batch.batch_id(),
        solver_id: id(40),
        row_count: ROW_COUNT,
        reward_rate_lamports: REWARD_RATE,
    };
    let submission = GeneralCandidateV1::submit(
        batch,
        CandidateV2::decode(&candidate).expect("candidate"),
        PAGE_REVISION,
        ROW_COUNT,
        REWARD_RATE,
        id(40),
        opening_probe.work_capacity().expect("capacity"),
        SUBMISSION_SLOT,
    )
    .expect("submit");

    Fixture {
        batch,
        orders: order_bytes,
        candidate,
        pages: vec![page],
        submission,
    }
}

/// Drive every row of the fixture's single page and return the certificate.
fn verify_all(fixture: &Fixture) -> (Vec<u8>, GeneralCandidateV1) {
    let cursor_len = candidate_verifier_len_v1(fixture.submission).expect("cursor width");
    let verified_len = candidate_certificate_len_v1(fixture.submission).expect("certificate width");
    let mut cursor = vec![0_u8; cursor_len];
    let mut certificate = vec![0_u8; verified_len];
    let page = PageV2::decode(&fixture.pages[0]).expect("page");
    let mut submission = fixture.submission;

    for row_index in 0..page.row_count() {
        let view = CandidateVerifyRowViewV1 {
            batch: fixture.batch,
            submission,
            candidate: &fixture.candidate,
            page: &fixture.pages[0],
            order: &fixture.orders[usize::try_from(row_index).expect("row")],
            cursor_before: &cursor,
            verified_before: &certificate,
            expected_page_index: 0,
            expected_row_index: row_index,
            expected_revision: u64::from(row_index),
        };
        let manifest_orders = candidate_verify_manifest_orders_v1(&view).expect("manifest sizing");
        let manifest_len =
            settlement_manifest_len_v2(WIDTH, manifest_orders).expect("manifest width");
        let mut manifest_scratch = vec![0_u8; manifest_len];
        let mut manifest_output = vec![0_u8; manifest_len];
        let mut cursor_scratch = vec![0_u8; cursor_len];
        let mut cursor_output = vec![0_u8; cursor_len];
        let mut verified_scratch = vec![0_u8; verified_len];
        let mut verified_output = vec![0_u8; verified_len];
        let summary = verify_candidate_row_v1(
            view,
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
        )
        .expect("row verifies");
        assert_eq!(summary.manifest_order_count, manifest_orders);
        assert_eq!(summary.reward.lamports, REWARD_RATE);
        assert_eq!(summary.reward.compartment, WorkCompartmentV1::Verification);
        submission = summary.submission;
        cursor = cursor_output;
        if summary.complete {
            certificate = verified_output;
            assert_eq!(
                submission.state().status,
                GeneralCandidateStatusV1::Verified,
                "the terminal verification transition records its own certificate"
            );
        }
    }
    (certificate, submission)
}

// ---------------------------------------------------------------------------
// The whole life
// ---------------------------------------------------------------------------

#[test]
fn the_whole_candidate_life_runs_through_the_evaluator() {
    let fixture = fixture();
    assert_eq!(
        fixture.submission.state().status,
        GeneralCandidateStatusV1::Submitted
    );
    let (certificate, mut submission) = verify_all(&fixture);

    // The certificate exists because the protocol produced it from pages the
    // protocol authenticated, against orders whose collateral the protocol is
    // holding. Before this module the same bytes could only be handed in.
    assert_eq!(
        submission.state().status,
        GeneralCandidateStatusV1::Verified
    );
    let verified = VerifiedCandidateV2::decode(&certificate).expect("certificate");
    assert_eq!(
        verified.header().candidate_id,
        fixture.submission.opening().candidate_id
    );
    assert_eq!(verified.header().batch_id, fixture.batch.batch_id());
    assert_eq!(verified.header().filled_lots, 8);
    assert_eq!(
        submission.state().verified_digest,
        dclutch_sha256_adapter::digest(&certificate)
    );

    // And selection -- which has always been able to read a certificate and
    // never had one written for it -- accepts this one.
    let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
    criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
    criteria[2] = SelectionCriterion::MinimizeCandidateId;
    let policy = SelectionPolicyV1 {
        policy_id: id(50),
        criterion_count: 3,
        criteria,
    };
    let mut scratch = vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut cursor = vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let vacant = vec![0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    consider_verified_candidate_v2(policy, &vacant, &certificate, 0, &mut scratch, &mut cursor)
        .expect("selection accepts a certificate the chain produced");
    let selection = RuntimeSelectionCursorV2::decode(&cursor).expect("selection cursor");
    assert_eq!(
        selection.header().best_candidate_id,
        fixture.submission.opening().candidate_id
    );

    let reward = submission.record_considered().expect("considered");
    assert_eq!(reward.lamports, REWARD_RATE);
    assert_eq!(
        submission.state().status,
        GeneralCandidateStatusV1::Considered
    );
}

// ---------------------------------------------------------------------------
// Content addressing
// ---------------------------------------------------------------------------

#[test]
fn a_candidate_must_carry_its_own_digest_as_its_identity() {
    let fixture = fixture();
    let candidate = CandidateV2::decode(&fixture.candidate).expect("candidate");
    authenticate_candidate_identity_v1(candidate).expect("canonical identity");

    // `CandidateV2::decode` treats `candidate_id` as a declared field and
    // checks nothing about it. Before submission bound it, a candidate could
    // name ANY identity -- including one already verified under other prices,
    // which is how a solver would get a certificate for a candidate whose
    // simplex nobody had checked.
    let mut forged = fixture.candidate.clone();
    forged[32..64].copy_from_slice(&id(0x77));
    let forged_candidate = CandidateV2::decode(&forged).expect("still structurally valid");
    assert_eq!(
        authenticate_candidate_identity_v1(forged_candidate),
        Err(GeneralCandidateErrorV1::NonCanonicalIdentity)
    );
    assert_eq!(
        submit_at(
            fixture.batch,
            forged_candidate,
            fixture
                .submission
                .opening()
                .work_capacity()
                .expect("capacity"),
            SUBMISSION_SLOT,
        ),
        Err(GeneralCandidateErrorV1::NonCanonicalIdentity)
    );
}

#[test]
fn hostile_moving_any_priced_byte_moves_the_identity() {
    let fixture = fixture();
    let identity = general_candidate_identity_v1(&fixture.candidate).expect("identity");
    // The mask covers exactly the identity field; every other byte is in the
    // digest, so a re-priced candidate is a different candidate.
    let repriced = candidate_bytes(fixture.batch.batch_id(), 1, &[41, 59, 0]);
    assert_ne!(
        general_candidate_identity_v1(&repriced).expect("identity"),
        identity
    );
    let mut rebatched = fixture.candidate.clone();
    rebatched[96..128].copy_from_slice(&id(0x66));
    assert_ne!(
        general_candidate_identity_v1(&rebatched).expect("identity"),
        identity
    );
}

// ---------------------------------------------------------------------------
// Submission
// ---------------------------------------------------------------------------

#[test]
fn hostile_a_candidate_cannot_be_submitted_against_an_open_batch_or_outside_the_window() {
    let mut root = GeneralRootV2::active(id(1), id(2), 7).expect("root");
    let revision = root.revision();
    let mut batch =
        GeneralBatchV1::open(&mut root, opening(), revision, ADMISSION_SLOT).expect("open batch");
    place(&mut batch, 9, 1, &[1, 0, 0], &[0, 1, 0]);
    let candidate = candidate_bytes(batch.batch_id(), 1, &[40, 60, 0]);
    let decoded = CandidateV2::decode(&candidate).expect("candidate");

    let funded = ROW_COUNT as u64 * REWARD_RATE + 2 * REWARD_RATE;
    // The batch is still collecting, so its order set can still grow.
    assert_eq!(
        submit_at(batch, decoded, funded, SUBMISSION_SLOT),
        Err(GeneralCandidateErrorV1::Collection(
            GeneralCollectionErrorV1::NotClosed
        ))
    );

    let revision = root.revision();
    batch.close(&mut root, revision).expect("close");
    // Before the collection window ends, a solver could submit and then close.
    assert_eq!(
        submit_at(batch, decoded, funded, COLLECTION_CLOSE - 1),
        Err(GeneralCandidateErrorV1::OutsideWindow)
    );
    // After the settlement window there is nothing left to settle.
    assert_eq!(
        submit_at(batch, decoded, funded, SETTLEMENT_CLOSE),
        Err(GeneralCandidateErrorV1::OutsideWindow)
    );
    submit_at(batch, decoded, funded, SUBMISSION_SLOT).expect("inside the window");
}

#[test]
fn a_submission_round_trips_through_a_hostile_decode() {
    let fixture = fixture();
    let bytes = fixture.submission.to_bytes();
    assert_eq!(bytes.len(), GENERAL_CANDIDATE_BYTES_V1);
    assert_eq!(
        GeneralCandidateV1::decode(&bytes).expect("round trip"),
        fixture.submission
    );
    assert_eq!(
        GeneralCandidateV1::decode(&bytes[..GENERAL_CANDIDATE_BYTES_V1 - 1]),
        Err(GeneralCandidateErrorV1::InvalidLength)
    );
    let mut noncanonical = bytes;
    noncanonical[180] = 1;
    assert_eq!(
        GeneralCandidateV1::decode(&noncanonical),
        Err(GeneralCandidateErrorV1::InvalidHeader)
    );
    // A submission whose work escrow holds more than the work it declared.
    let mut overfunded = bytes;
    let past_capacity = fixture
        .submission
        .opening()
        .verification_capacity()
        .expect("capacity")
        + 1;
    overfunded[192..200].copy_from_slice(&past_capacity.to_le_bytes());
    assert_eq!(
        GeneralCandidateV1::decode(&overfunded),
        Err(GeneralCandidateErrorV1::Uncapitalized)
    );
    let mut unknown_status = bytes;
    unknown_status[20] = 9;
    assert_eq!(
        GeneralCandidateV1::decode(&unknown_status),
        Err(GeneralCandidateErrorV1::InvalidStatus)
    );
    // A submission that claims to be verified with no certificate behind it.
    let mut lying = bytes;
    lying[20] = GeneralCandidateStatusV1::Verified.tag();
    assert_eq!(
        GeneralCandidateV1::decode(&lying),
        Err(GeneralCandidateErrorV1::InvalidStatus)
    );
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

#[test]
fn hostile_a_page_from_another_candidate_or_revision_is_refused() {
    let fixture = fixture();
    let cursor_len = candidate_verifier_len_v1(fixture.submission).expect("cursor width");
    let verified_len = candidate_certificate_len_v1(fixture.submission).expect("certificate width");

    let foreign_candidate = candidate_bytes(fixture.batch.batch_id(), 1, &[41, 59, 0]);
    let foreign_id = CandidateV2::decode(&foreign_candidate)
        .expect("foreign")
        .header()
        .candidate_id;
    let row = row_bytes(&fixture.orders[0], 1, 1, 4);
    let foreign_page = page_bytes(foreign_id, 1, 1, &[&row]);

    // A page bound to a different candidate.
    assert_eq!(
        run_one(
            &fixture,
            &fixture.candidate,
            &foreign_page,
            0,
            0,
            0,
            cursor_len,
            verified_len
        ),
        Err(GeneralCandidateErrorV1::Substitution)
    );

    // A page at the submission's candidate but a revision the submission did
    // not pin. Without the pin a solver could publish a second page at the same
    // coordinate and feed whichever one suited the step.
    let mut wrong_revision = fixture.pages[0].clone();
    wrong_revision[24..32].copy_from_slice(&(PAGE_REVISION + 1).to_le_bytes());
    assert_eq!(
        run_one(
            &fixture,
            &fixture.candidate,
            &wrong_revision,
            0,
            0,
            0,
            cursor_len,
            verified_len
        ),
        Err(GeneralCandidateErrorV1::Substitution)
    );

    // The candidate's own page, offered at a step it does not sit at. The page
    // is canonical; what is wrong is which step is consuming it, and the
    // coordinate conjunct is the only thing that says so.
    assert_eq!(
        run_one(
            &fixture,
            &fixture.candidate,
            &fixture.pages[0],
            0,
            1,
            0,
            cursor_len,
            verified_len,
        ),
        Err(GeneralCandidateErrorV1::Substitution)
    );
}

#[test]
fn hostile_a_row_cannot_be_verified_against_a_cancelled_order() {
    let fixture = fixture();
    let cursor_len = candidate_verifier_len_v1(fixture.submission).expect("cursor width");
    let verified_len = candidate_certificate_len_v1(fixture.submission).expect("certificate width");

    // The maker refunded themselves while the batch was still collecting; the
    // candidate was built before that. Its escrow is gone, so no row against it
    // may verify -- and the refusal names the PHASE, because every coordinate
    // in the row still matches the record.
    let order = GeneralOrderV1::decode(&fixture.orders[0]).expect("order");
    let mut cancelled = vec![0_u8; fixture.orders[0].len()];
    order
        .encode_successor_state_into(
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Cancelled,
                admitted_slot: ADMISSION_SLOT,
                released_slot: ADMISSION_SLOT + 1,
            },
            &mut cancelled,
        )
        .expect("cancelled successor");
    assert_eq!(
        run_one_with_order(
            &fixture,
            &fixture.pages[0],
            &cancelled,
            cursor_len,
            verified_len
        ),
        Err(GeneralCandidateErrorV1::Collection(
            GeneralCollectionErrorV1::InvalidOrderPhase
        ))
    );
}

#[test]
fn hostile_a_row_naming_an_order_from_another_batch_is_refused() {
    let fixture = fixture();
    let cursor_len = candidate_verifier_len_v1(fixture.submission).expect("cursor width");
    let verified_len = candidate_certificate_len_v1(fixture.submission).expect("certificate width");

    let mut other_root = GeneralRootV2::active(id(1), id(2), 7).expect("root");
    let revision = other_root.revision();
    let mut other_opening = opening();
    other_opening.sequence = 0;
    other_opening.max_orders = 2;
    let mut other = GeneralBatchV1::open(&mut other_root, other_opening, revision, ADMISSION_SLOT)
        .expect("other batch");
    let foreign_order = place(&mut other, 9, 1, &[1, 0, 0], &[0, 1, 0]);

    assert_eq!(
        run_one_with_order(
            &fixture,
            &fixture.pages[0],
            &foreign_order,
            cursor_len,
            verified_len
        ),
        Err(GeneralCandidateErrorV1::Collection(
            GeneralCollectionErrorV1::Substitution
        ))
    );
}

#[test]
fn hostile_verification_cannot_start_on_an_already_verified_submission() {
    let fixture = fixture();
    let (_, submission) = verify_all(&fixture);
    let cursor_len = candidate_verifier_len_v1(submission).expect("cursor width");
    let verified_len = candidate_certificate_len_v1(submission).expect("certificate width");
    let mut replayed = fixture;
    replayed.submission = submission;
    assert_eq!(
        run_one(
            &replayed,
            &replayed.candidate.clone(),
            &replayed.pages[0].clone(),
            0,
            0,
            0,
            cursor_len,
            verified_len,
        ),
        Err(GeneralCandidateErrorV1::InvalidPhaseTransition)
    );
}

#[test]
fn hostile_the_certificate_recorded_must_be_this_submissions_own() {
    let fixture = fixture();
    let (certificate, verified_submission) = verify_all(&fixture);
    // Recording happens on a submission whose rows were really verified: its
    // escrow already says so, and a fresh one would be refused for exactly
    // that reason.
    let pre_record = spend_verification_cranks(fixture.submission, ROW_COUNT);
    assert_eq!(
        verified_submission.state().verification_remaining,
        pre_record.state().verification_remaining
    );

    // A certificate for a candidate this submission does not name.
    let mut foreign = certificate.clone();
    foreign[32..64].copy_from_slice(&id(0x71));
    let mut submission = pre_record;
    assert_eq!(
        submission.record_verified(fixture.batch, &foreign),
        Err(GeneralCandidateErrorV1::Substitution)
    );
    // A certificate for another batch.
    let mut rebatched = certificate.clone();
    rebatched[96..128].copy_from_slice(&id(0x72));
    assert_eq!(
        submission.record_verified(fixture.batch, &rebatched),
        Err(GeneralCandidateErrorV1::Substitution)
    );
    // The honest one, and then a second recording of it.
    submission
        .record_verified(fixture.batch, &certificate)
        .expect("honest certificate");
    assert_eq!(
        submission.record_verified(fixture.batch, &certificate),
        Err(GeneralCandidateErrorV1::InvalidPhaseTransition)
    );
    // And considering may not run before verification.
    let mut fresh = fixture.submission;
    assert_eq!(
        fresh.record_considered().err(),
        Some(GeneralCandidateErrorV1::InvalidPhaseTransition)
    );
}

#[test]
fn a_certificate_whose_debit_exceeds_the_batch_escrow_is_refused_at_recording() {
    let fixture = fixture();
    let (certificate, _) = verify_all(&fixture);
    let verified = VerifiedCandidateV2::decode(&certificate).expect("certificate");
    // The honest certificate fits inside what the batch is holding.
    let mut submission = spend_verification_cranks(fixture.submission, ROW_COUNT);
    submission
        .record_verified(fixture.batch, &certificate)
        .expect("inside the escrow");

    // A certificate claiming a debit past the whole batch's escrow could not be
    // paid at Collect. Recording refuses it here rather than stranding the
    // settlement at its first short row.
    let mut overdrawn = certificate.clone();
    let past_escrow = fixture.batch.state().committed_quote_reserve + 1;
    overdrawn[136..144].copy_from_slice(&past_escrow.to_le_bytes());
    let mut fresh = spend_verification_cranks(fixture.submission, ROW_COUNT);
    assert!(verified.header().quote_debit < past_escrow);
    assert_eq!(
        fresh.record_verified(fixture.batch, &overdrawn),
        Err(GeneralCandidateErrorV1::Collection(
            GeneralCollectionErrorV1::EscrowShortfall
        ))
    );
}

// ---------------------------------------------------------------------------
// The identity order a builder must respect
// ---------------------------------------------------------------------------

#[test]
fn the_protocol_orders_identities_little_endian_and_a_builder_must_too() {
    use crate::general::runtime_verify::runtime_identity_precedes_v2;

    let mut low = [0_u8; 32];
    low[0] = 2;
    let mut high = [0_u8; 32];
    high[31] = 1;
    // Lexicographically `low` is the greater of the two; numerically, read
    // little-endian as the protocol reads it, it is the lesser. A builder that
    // sorts with the derived `Ord` produces candidates the verifier refuses
    // with `NonCanonicalOrder`, and the disagreement is invisible against
    // fabricated `[low, 0, 0, ...]` identities.
    assert!(low.as_slice() > high.as_slice());
    assert!(runtime_identity_precedes_v2(&low, &high));
    assert!(!runtime_identity_precedes_v2(&high, &low));
    assert!(!runtime_identity_precedes_v2(&low, &low));
}

// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_one(
    fixture: &Fixture,
    candidate: &[u8],
    page: &[u8],
    order_index: usize,
    page_index: u32,
    row_index: u32,
    cursor_len: usize,
    verified_len: usize,
) -> GeneralCandidateResultV1<CandidateVerifyRowSummaryV1> {
    run_inner(
        fixture,
        candidate,
        page,
        &fixture.orders[order_index],
        page_index,
        row_index,
        cursor_len,
        verified_len,
    )
}

fn run_one_with_order(
    fixture: &Fixture,
    page: &[u8],
    order: &[u8],
    cursor_len: usize,
    verified_len: usize,
) -> GeneralCandidateResultV1<CandidateVerifyRowSummaryV1> {
    run_inner(
        fixture,
        &fixture.candidate,
        page,
        order,
        0,
        0,
        cursor_len,
        verified_len,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_inner(
    fixture: &Fixture,
    candidate: &[u8],
    page: &[u8],
    order: &[u8],
    page_index: u32,
    row_index: u32,
    cursor_len: usize,
    verified_len: usize,
) -> GeneralCandidateResultV1<CandidateVerifyRowSummaryV1> {
    let cursor = vec![0_u8; cursor_len];
    let certificate = vec![0_u8; verified_len];
    let manifest_len = settlement_manifest_len_v2(WIDTH, 1).expect("manifest width");
    let mut manifest_scratch = vec![0_u8; manifest_len];
    let mut manifest_output = vec![0_u8; manifest_len];
    let mut cursor_scratch = vec![0_u8; cursor_len];
    let mut cursor_output = vec![0_u8; cursor_len];
    let mut verified_scratch = vec![0_u8; verified_len];
    let mut verified_output = vec![0_u8; verified_len];
    verify_candidate_row_v1(
        CandidateVerifyRowViewV1 {
            batch: fixture.batch,
            submission: fixture.submission,
            candidate,
            page,
            order,
            cursor_before: &cursor,
            verified_before: &certificate,
            expected_page_index: page_index,
            expected_row_index: row_index,
            expected_revision: 0,
        },
        CandidateVerifyRowBuffersV1 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
            manifest_scratch: &mut manifest_scratch,
            manifest_output: &mut manifest_output,
        },
    )
}

// ---------------------------------------------------------------------------
// The funded work escrow
//
// Gen-2's Consider was permissionless and UNPAID, which makes a verb
// permissible rather than live: a valid candidate nobody cranked before the
// selection window closed simply never competed, and a submitter whose
// consideration was censored had no recourse. These pin the thing that
// replaces hoping.
// ---------------------------------------------------------------------------

#[test]
fn every_permissionless_crank_is_paid_out_of_the_candidates_own_escrow() {
    let fixture = fixture();
    let opening = fixture.submission.opening();
    // One reward per row, plus one for the single consideration, plus one to
    // close the candidate out.
    assert_eq!(opening.verification_capacity(), Ok(3 * REWARD_RATE));
    assert_eq!(opening.cleanup_capacity(), REWARD_RATE);
    assert_eq!(opening.work_capacity(), Ok(4 * REWARD_RATE));

    let (_, mut submission) = verify_all(&fixture);
    // Both rows drew, and the consideration is what the remainder is for.
    assert_eq!(submission.state().verification_remaining, REWARD_RATE);
    let reward = submission.record_considered().expect("considered");
    assert_eq!(reward.lamports, REWARD_RATE);
    assert_eq!(reward.compartment, WorkCompartmentV1::Verification);
    assert_eq!(submission.state().verification_remaining, 0);

    // Closing out pays its own crank and returns nothing, because a candidate
    // that ran to completion spent exactly what it was funded for.
    let (cleanup, solver_refund) = submission.close_out().expect("close out");
    assert_eq!(cleanup.lamports, REWARD_RATE);
    assert_eq!(cleanup.compartment, WorkCompartmentV1::Cleanup);
    assert_eq!(solver_refund, 0);
    assert_eq!(
        submission.close_out().err(),
        Some(GeneralCandidateErrorV1::Uncapitalized)
    );
}

#[test]
fn an_abandoned_candidate_refunds_its_unspent_work_to_the_solver() {
    let fixture = fixture();
    // Nobody verified it. The cleanup crank is still paid -- someone did the
    // work of closing the accounts -- and everything else goes back to the
    // solver rather than to whoever happened to call.
    let mut submission = fixture.submission;
    let (cleanup, solver_refund) = submission.close_out().expect("close out");
    assert_eq!(cleanup.lamports, REWARD_RATE);
    assert_eq!(solver_refund, 3 * REWARD_RATE);
}

#[test]
fn hostile_a_submission_must_be_funded_for_exactly_the_work_it_declares() {
    let fixture = fixture();
    let candidate = CandidateV2::decode(&fixture.candidate).expect("candidate");
    let exact = fixture
        .submission
        .opening()
        .work_capacity()
        .expect("capacity");

    // Underfunding buys work nobody is paid for; overfunding leaves lamports
    // with no rule for who gets them, which is the same hole facing the other
    // way. Both are refused with the same name.
    for funded in [0, exact - 1, exact + 1, exact * 2] {
        assert_eq!(
            submit_at(fixture.batch, candidate, funded, SUBMISSION_SLOT).err(),
            Some(GeneralCandidateErrorV1::Uncapitalized),
            "funding {funded} against a capacity of {exact}"
        );
    }
    submit_at(fixture.batch, candidate, exact, SUBMISSION_SLOT).expect("exact funding");
}

#[test]
fn hostile_a_declared_row_count_that_is_not_the_candidates_own_cannot_complete() {
    // The declared row count is what the work escrow is sized against, so a
    // submission that declares more rows than its candidate carries would be
    // buying cranks nobody can perform -- and one that declares fewer would be
    // consuming cranks nobody paid for. The terminal row is required to land
    // exactly on the declaration, so neither completes.
    let mut fixture = fixture();
    let overstated = GeneralCandidateV1::submit(
        fixture.batch,
        CandidateV2::decode(&fixture.candidate).expect("candidate"),
        PAGE_REVISION,
        ROW_COUNT + 1,
        REWARD_RATE,
        id(40),
        (u64::from(ROW_COUNT + 1) + 2) * REWARD_RATE,
        SUBMISSION_SLOT,
    )
    .expect("a longer declaration is fundable");
    fixture.submission = overstated;

    let cursor_len = candidate_verifier_len_v1(overstated).expect("cursor width");
    let verified_len = candidate_certificate_len_v1(overstated).expect("certificate width");
    let mut cursor = vec![0_u8; cursor_len];
    let certificate = vec![0_u8; verified_len];
    let page = PageV2::decode(&fixture.pages[0]).expect("page");
    let mut submission = overstated;
    let mut last = Ok(());

    for row_index in 0..page.row_count() {
        let view = CandidateVerifyRowViewV1 {
            batch: fixture.batch,
            submission,
            candidate: &fixture.candidate,
            page: &fixture.pages[0],
            order: &fixture.orders[usize::try_from(row_index).expect("row")],
            cursor_before: &cursor,
            verified_before: &certificate,
            expected_page_index: 0,
            expected_row_index: row_index,
            expected_revision: u64::from(row_index),
        };
        let manifest_orders = candidate_verify_manifest_orders_v1(&view).expect("manifest sizing");
        let manifest_len =
            settlement_manifest_len_v2(WIDTH, manifest_orders).expect("manifest width");
        let mut manifest_scratch = vec![0_u8; manifest_len];
        let mut manifest_output = vec![0_u8; manifest_len];
        let mut cursor_scratch = vec![0_u8; cursor_len];
        let mut cursor_output = vec![0_u8; cursor_len];
        let mut verified_scratch = vec![0_u8; verified_len];
        let mut verified_output = vec![0_u8; verified_len];
        match verify_candidate_row_v1(
            view,
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
        ) {
            Ok(summary) => {
                submission = summary.submission;
                cursor = cursor_output;
            }
            Err(error) => {
                last = Err(error);
                break;
            }
        }
    }
    // The terminal row is the one that refuses: two rows really exist, and the
    // submission said there were three.
    assert_eq!(last, Err(GeneralCandidateErrorV1::Uncapitalized));
}

#[test]
fn hostile_a_crank_cannot_be_drawn_twice_for_one_row() {
    let fixture = fixture();
    let cursor_len = candidate_verifier_len_v1(fixture.submission).expect("cursor width");
    let verified_len = candidate_certificate_len_v1(fixture.submission).expect("certificate width");

    // A submission whose escrow already says both rows were paid, offered a
    // row-zero step. The capitalization check runs BEFORE the work, against
    // the revision the step claims, so the replay is refused at the escrow
    // rather than at the verifier.
    let mut replayed = fixture;
    replayed.submission = spend_verification_cranks(replayed.submission, ROW_COUNT);
    assert_eq!(
        run_one(
            &replayed,
            &replayed.candidate.clone(),
            &replayed.pages[0].clone(),
            0,
            0,
            0,
            cursor_len,
            verified_len,
        ),
        Err(GeneralCandidateErrorV1::Uncapitalized)
    );
}
