//! Structural Verify lifecycle forecast shared by the native and host adapters.

use super::{general_hot_scalar_count_v3, scalar};
use crate::general::{
    artifacts_v3::GeneralDecodedRequestV3,
    candidate_v1::{
        GeneralCandidateErrorV1, GeneralCandidateV1, candidate_verify_terminal_step_v1,
    },
    collection_v1::{GeneralBatchV2, GeneralCollectionErrorV1},
    local_state_v3::{GeneralLocalStateErrorV3, GeneralLocalStateKindV3, GeneralLocalStateV3},
    state_artifacts_v3::{GENERAL_PRIMARY_STATE_ACCOUNT_V3, general_readonly_evidence_v3},
};
use crate::general_codec::Action;
use dclutch_vm::account_profile::AccountObservationV1;

/// Located refusal while forecasting a Verify lifecycle guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralVerifyPreplanErrorV3 {
    /// The selected request is not VerifyCandidateRow.
    Action,
    /// The Product width or scalar bank is not exact.
    Capacity,
    /// A declared evidence coordinate is missing.
    Coordinate,
    /// The observed Batch envelope does not authenticate.
    BatchEnvelope(GeneralLocalStateErrorV3),
    /// The observed Candidate submission envelope does not authenticate.
    SubmissionEnvelope(GeneralLocalStateErrorV3),
    /// The closed Batch body does not decode.
    Batch(GeneralCollectionErrorV1),
    /// The Candidate submission body does not decode.
    Submission(GeneralCandidateErrorV1),
    /// The request names another submitted Candidate.
    RequestCandidate,
    /// The canonical candidate/page selector refused its structural joins.
    Forecast(GeneralCandidateErrorV1),
}

/// Seed only the terminal-row lifecycle guard from authenticated structural
/// evidence. The evaluator still proves order terms, cursor progress and all
/// economic predicates, and must reproduce this forecast before any commit.
/// Every refusal leaves the complete caller scalar bank unchanged.
pub fn seed_general_verify_preplan_terminal_v3(
    request: GeneralDecodedRequestV3,
    outcome_count: u32,
    trading_program: [u8; 32],
    observations: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
) -> Result<(), GeneralVerifyPreplanErrorV3> {
    use GeneralVerifyPreplanErrorV3 as Error;
    if request.action != Action::VerifyCandidateRow {
        return Err(Error::Action);
    }
    let expected_width = general_hot_scalar_count_v3(request.action, outcome_count)
        .ok()
        .and_then(|width| usize::try_from(width).ok())
        .ok_or(Error::Capacity)?;
    if outcome_count == 0 || scalars.len() != expected_width {
        return Err(Error::Capacity);
    }
    let evidence = |index| {
        let coordinate = general_readonly_evidence_v3(request.action, index)
            .map_err(|_| Error::Coordinate)?
            .coordinate;
        observations
            .get(usize::from(coordinate))
            .copied()
            .ok_or(Error::Coordinate)
    };
    let batch_account = evidence(0)?;
    let submission_account = observations
        .get(usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3))
        .copied()
        .ok_or(Error::Coordinate)?;
    let batch_state = GeneralLocalStateV3::decode_owned(
        batch_account.data(),
        batch_account.owner(),
        trading_program,
        GeneralLocalStateKindV3::Batch,
        outcome_count,
    )
    .map_err(Error::BatchEnvelope)?;
    let submission_state = GeneralLocalStateV3::decode_owned(
        submission_account.data(),
        submission_account.owner(),
        trading_program,
        GeneralLocalStateKindV3::Candidate,
        outcome_count,
    )
    .map_err(Error::SubmissionEnvelope)?;
    let batch = GeneralBatchV2::decode(batch_state.body()).map_err(Error::Batch)?;
    let submission =
        GeneralCandidateV1::decode(submission_state.body()).map_err(Error::Submission)?;
    if request.candidate_id != Some(submission.opening().candidate_id) {
        return Err(Error::RequestCandidate);
    }
    let terminal = candidate_verify_terminal_step_v1(
        batch,
        submission,
        evidence(1)?.data(),
        evidence(2)?.data(),
        request.page_index,
        u32::from(request.execution_index),
    )
    .map_err(Error::Forecast)?;
    let destination = scalars
        .get_mut(usize::try_from(scalar::VERIFY_TERMINAL).map_err(|_| Error::Capacity)?)
        .ok_or(Error::Capacity)?;
    *destination = u64::from(terminal);
    Ok(())
}

#[cfg(test)]
pub(super) fn assert_terminal_seed_controls(
    view: &crate::general::candidate_v1::CandidateVerifyRowViewV1<'_>,
) {
    use crate::general::{
        artifacts_v3::GeneralRequestWireV3,
        collection_v1::general_batch_len_v2,
        local_state_v3::{
            GeneralLocalStateHeaderV3, encode_general_local_state_v3_atomic,
            general_local_state_len_v3,
        },
    };
    use std::{vec, vec::Vec};
    let count = view.submission.opening().outcome_count;
    let trading = [0xa1; 32];
    let batch_coordinate = usize::from(
        general_readonly_evidence_v3(Action::VerifyCandidateRow, 0)
            .expect("Batch coordinate")
            .coordinate,
    );
    let candidate_coordinate = usize::from(
        general_readonly_evidence_v3(Action::VerifyCandidateRow, 1)
            .expect("image coordinate")
            .coordinate,
    );
    let page_coordinate = usize::from(
        general_readonly_evidence_v3(Action::VerifyCandidateRow, 2)
            .expect("Page coordinate")
            .coordinate,
    );
    let submission_coordinate = usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3);
    let width = page_coordinate.max(submission_coordinate) + 1;
    let keys = (0..width)
        .map(|index| [u8::try_from(index).expect("bounded coordinates"); 32])
        .collect::<Vec<_>>();
    let mut bodies = vec![Vec::new(); width];
    let mut batch_body = vec![0; general_batch_len_v2(count).expect("Batch width")];
    view.batch.encode_into(&mut batch_body).expect("Batch body");
    for (coordinate, kind, body) in [
        (batch_coordinate, GeneralLocalStateKindV3::Batch, batch_body),
        (
            submission_coordinate,
            GeneralLocalStateKindV3::Candidate,
            view.submission.to_bytes().to_vec(),
        ),
    ] {
        let mut bytes = vec![0; general_local_state_len_v3(kind, count).expect("envelope width")];
        encode_general_local_state_v3_atomic(
            GeneralLocalStateHeaderV3 {
                kind,
                bump: 9,
                rent_principal: 1,
                beneficiary: [0xa2; 32],
            },
            &body,
            &mut vec![0; bytes.len()],
            &mut bytes,
        )
        .expect("observed envelope");
        bodies[coordinate] = bytes;
    }
    bodies[candidate_coordinate] = view.candidate.to_vec();
    bodies[page_coordinate] = view.page.to_vec();
    let request = GeneralDecodedRequestV3 {
        wire: GeneralRequestWireV3::V3,
        action: Action::VerifyCandidateRow,
        expected_revision: view.expected_revision,
        candidate_id: Some(view.submission.opening().candidate_id),
        page_index: view.expected_page_index,
        execution_index: u8::try_from(view.expected_row_index).expect("row index"),
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
        result_state_bump: 0,
    };
    let owners = vec![trading; width];
    let bank_width =
        usize::try_from(general_hot_scalar_count_v3(request.action, count).expect("bank width"))
            .expect("host width");
    let run = |request, bodies: &[Vec<u8>], owners: &[[u8; 32]], scalars: &mut [u64]| {
        let observations = keys
            .iter()
            .zip(owners)
            .zip(bodies)
            .map(|((key, owner), body)| {
                AccountObservationV1::new(key, owner, 1, body, false, false, false)
            })
            .collect::<Vec<_>>();
        seed_general_verify_preplan_terminal_v3(request, count, trading, &observations, scalars)
    };
    let mut bank = vec![0x55; bank_width];
    run(request, &bodies, &owners, &mut bank).expect("canonical structural seed");
    let mut expected = vec![0x55; bank_width];
    expected[usize::try_from(scalar::VERIFY_TERMINAL).expect("guard coordinate")] = 1;
    assert_eq!(bank, expected, "only the terminal guard is seeded");
    use GeneralVerifyPreplanErrorV3 as Error;
    for mutation in 0..8 {
        let mut request = request;
        let mut bodies = bodies.clone();
        let mut owners = owners.clone();
        let mut bank = vec![0x55; bank_width];
        let refusal = match mutation {
            0 => {
                owners[batch_coordinate] = [0xfe; 32];
                Error::BatchEnvelope(GeneralLocalStateErrorV3::InvalidOwner)
            }
            1 => {
                owners[submission_coordinate] = [0xfe; 32];
                Error::SubmissionEnvelope(GeneralLocalStateErrorV3::InvalidOwner)
            }
            2 => {
                bodies[batch_coordinate].pop();
                Error::BatchEnvelope(GeneralLocalStateErrorV3::InvalidLength)
            }
            3 => {
                request.candidate_id = Some([0xee; 32]);
                Error::RequestCandidate
            }
            4 => {
                request.execution_index = u8::MAX;
                Error::Forecast(GeneralCandidateErrorV1::Substitution)
            }
            5 => {
                bodies[page_coordinate][0] ^= 1;
                Error::Forecast(GeneralCandidateErrorV1::Substitution)
            }
            6 => {
                request.action = Action::PlaceOrder;
                Error::Action
            }
            7 => {
                bank.pop();
                Error::Capacity
            }
            _ => unreachable!(),
        };
        let before = bank.clone();
        assert_eq!(
            run(request, &bodies, &owners, &mut bank),
            Err(refusal),
            "mutation {mutation}"
        );
        assert_eq!(
            bank, before,
            "refusal preserves every scalar at mutation {mutation}"
        );
    }
}
