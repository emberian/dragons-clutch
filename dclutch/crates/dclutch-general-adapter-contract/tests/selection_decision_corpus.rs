//! Replay every Lean-decided General selection decision through the adapter.
//!
//! `DClutchSemantics.GeneralV5Assurance` owns the best-valid-submitted rule:
//! `betterBy` and the `Selection.consider` fold that keeps an incumbent no
//! submission betters. The runtime decides the same thing in a composition of
//! three functions -- `runtime_verified_balance_v2` derives the quote surplus,
//! `runtime_candidate_key_better_v2` interprets the policy, and
//! `consider_verified_candidate_v2` is the fold that reads the incumbent's key
//! back out of the persisted cursor. Nothing checked one against the other.
//!
//! What the corpus does NOT cover, because the runtime refuses these a gate
//! earlier than Lean decides anything: `DuplicateCandidate`, `Substitution`
//! and `RevisionMismatch` are the account frame's optimistic-concurrency and
//! comparison-domain obligations. Product, Batch, policy, price scale and
//! outcome width are held fixed per vector so none of them can fire and the
//! only thing under test is the decision.

#![allow(clippy::panic, clippy::indexing_slicing, clippy::unwrap_used)]

use dclutch_general_adapter_contract::runtime_selection::{
    RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2, consider_verified_candidate_v2,
};
use dclutch_general_adapter_contract::runtime_verify::runtime_verified_balance_v2;
use dclutch_general_adapter_contract::runtime_width::{
    VerifiedCandidateHeaderV2, VerifiedCandidateV2, verified_candidate_len,
};
use dclutch_general_codec::{MAX_SELECTION_CRITERIA, SelectionCriterion, SelectionPolicyV1};

#[allow(dead_code, missing_docs)]
mod corpus {
    include!("generated/selection_decision_corpus_v1.rs");
}

use corpus::{GENERAL_SELECTION_VECTORS_V1, GeneralSelectionSubmissionV1};

const PRODUCT: [u8; 32] = [1; 32];
const BATCH: [u8; 32] = [2; 32];
const POLICY: [u8; 32] = [3; 32];
const PRICE_SCALE: u64 = 1;

/// `canonicalCriteria` from `GeneralV5Assurance.lean`, as policy data.
fn canonical_policy() -> SelectionPolicyV1 {
    let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
    criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
    criteria[2] = SelectionCriterion::MinimizeCandidateId;
    SelectionPolicyV1 {
        policy_id: POLICY,
        criterion_count: 3,
        criteria,
    }
}

/// Encode one corpus submission as the certificate selection actually reads.
///
/// Coordinate and verification revision carry no Lean authority: they are the
/// frame's own nonzero coordinates. The coordinate is what makes an exact
/// objective tie reachable at all, because two byte-identical certificates are
/// refused as a `DuplicateCandidate` before any comparison happens.
fn certificate(submission: &GeneralSelectionSubmissionV1, coordinate: u32) -> Vec<u8> {
    let width = u32::try_from(submission.claim_inputs.len()).expect("outcome width");
    let mut bytes = vec![0_u8; verified_candidate_len(width).expect("verified width")];
    VerifiedCandidateV2::encode_into(
        VerifiedCandidateHeaderV2 {
            outcome_count: width,
            page_count: 1,
            candidate_coordinate: coordinate,
            revision: 1,
            candidate_id: submission.candidate_id,
            product_id: PRODUCT,
            batch_id: BATCH,
            filled_lots: submission.filled_lots,
            quote_debit: submission.quote_debit,
            quote_credit: submission.quote_credit,
            price_scale: PRICE_SCALE,
        },
        submission.claim_inputs,
        submission.claim_outputs,
        &mut bytes,
    )
    .expect("verified candidate encode");
    bytes
}

/// The runtime derives the quote surplus Lean's `Objective` carries.
///
/// `Candidate.quoteAfterMaterialization` and `derive_balance` are independent
/// implementations of the same complete-set arithmetic; the mint and merge
/// vectors are the ones that separate them from a passthrough.
#[test]
fn lean_decided_quote_surplus_is_what_the_runtime_derives() {
    for vector in GENERAL_SELECTION_VECTORS_V1 {
        for (index, submission) in vector.submissions.iter().enumerate() {
            let coordinate = u32::try_from(index + 1).expect("coordinate");
            let bytes = certificate(submission, coordinate);
            let balance = runtime_verified_balance_v2(&bytes)
                .unwrap_or_else(|error| panic!("{} submission {index}: {error:?}", vector.name));
            assert_eq!(
                balance.quote_surplus, submission.quote_surplus,
                "{} submission {index} quote surplus",
                vector.name
            );
        }
    }
}

/// The runtime cursor selects the candidate Lean's fold selects.
///
/// The winning COORDINATE is the load-bearing assertion: identity, lots and
/// surplus all coincide on the tie vector, and only the coordinate says
/// whether the incumbent was kept or replaced.
#[test]
fn lean_selection_corpus_replays_through_the_runtime_cursor() {
    let policy = canonical_policy();
    for vector in GENERAL_SELECTION_VECTORS_V1 {
        let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        let mut cursor = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        for (index, submission) in vector.submissions.iter().enumerate() {
            let coordinate = u32::try_from(index + 1).expect("coordinate");
            let bytes = certificate(submission, coordinate);
            let before = cursor;
            consider_verified_candidate_v2(
                policy,
                &before,
                &bytes,
                u64::try_from(index).expect("expected revision"),
                &mut scratch,
                &mut cursor,
            )
            .unwrap_or_else(|error| panic!("{} submission {index}: {error:?}", vector.name));
        }
        let header = RuntimeSelectionCursorV2::decode(&cursor)
            .unwrap_or_else(|error| panic!("{} cursor: {error:?}", vector.name))
            .header();
        assert_eq!(
            header.best_candidate_coordinate, vector.best_coordinate,
            "{} winning coordinate",
            vector.name
        );
        assert_eq!(
            header.best_candidate_id, vector.best_candidate_id,
            "{} winning identity",
            vector.name
        );
        assert_eq!(
            header.best_filled_lots, vector.best_filled_lots,
            "{} winning filled lots",
            vector.name
        );
        assert_eq!(
            header.best_quote_surplus, vector.best_quote_surplus,
            "{} winning quote surplus",
            vector.name
        );
        assert_eq!(
            header.submitted_count,
            u32::try_from(vector.submissions.len()).expect("submitted count"),
            "{} submitted count",
            vector.name
        );
    }
}
