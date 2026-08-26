extern crate std;

use dclutch_general_codec::{
    CandidateV1, ExecutionV1, MAX_EXECUTIONS_PER_PAGE, MAX_OUTCOMES, PAGE_BYTES, PageV1,
    SELECTION_CURSOR_BYTES, SELECTION_POLICY_BYTES, SETTLEMENT_CURSOR_BYTES, SelectionCriterion,
    SelectionCursorV1, SelectionPolicyV1,
};

use crate::GENERAL_CHILD_PLAN_HEADER_BYTES_V2;

use super::*;

fn id(byte: u8) -> [u8; 32] {
    let mut value = [byte; 32];
    value[31] = byte.wrapping_add(1);
    value
}

fn vector(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
    let mut value = [0; MAX_OUTCOMES];
    value[0] = first;
    value[1] = second;
    value
}

fn candidate() -> CandidateV1 {
    CandidateV1 {
        outcome_count: 2,
        candidate_id: id(1),
        product_id: id(2),
        batch_id: id(3),
        page_count: 1,
        price_scale: 2,
        prices: vector(1, 1),
    }
}

fn policy() -> SelectionPolicyV1 {
    let mut criteria =
        [SelectionCriterion::MaximizeFilledLots; dclutch_general_codec::MAX_SELECTION_CRITERIA];
    criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
    criteria[2] = SelectionCriterion::MinimizeCandidateId;
    SelectionPolicyV1 {
        policy_id: id(4),
        criterion_count: 3,
        criteria,
    }
}

fn execution(order: u8, owner: u8, receive: [u64; MAX_OUTCOMES]) -> ExecutionV1 {
    ExecutionV1 {
        order_id: id(order),
        owner_id: id(owner),
        nonce: 1,
        max_lots: 1,
        max_quote_debit_per_lot: 1,
        lots: 1,
        quote_debit: 1,
        quote_credit: 0,
        receive_per_lot: receive,
        deliver_per_lot: [0; MAX_OUTCOMES],
    }
}

fn page() -> [u8; PAGE_BYTES] {
    let mut executions = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
    executions[0] = execution(5, 6, vector(1, 0));
    executions[1] = execution(7, 8, vector(0, 1));
    PageV1 {
        outcome_count: 2,
        candidate_id: candidate().candidate_id,
        page_index: 0,
        page_count: 1,
        execution_count: 2,
        executions,
    }
    .to_bytes()
    .expect("page")
}

fn limits() -> GeneralPlanLimitsV2 {
    GeneralPlanLimitsV2::new(2, 10, 10, 2, policy().policy_id).expect("limits")
}

struct ConsiderFixture {
    candidate: [u8; CANDIDATE_BYTES],
    policy: [u8; SELECTION_POLICY_BYTES],
    page: [u8; PAGE_BYTES],
    verification: [u8; VERIFICATION_CURSOR_BYTES_V1],
    selection: [u8; SELECTION_CURSOR_BYTES],
    certificate: [u8; VERIFIED_CANDIDATE_BYTES_V1],
}

fn consider_fixture() -> ConsiderFixture {
    let candidate = candidate().to_bytes().expect("candidate");
    let policy = policy().to_bytes().expect("policy");
    let page = page();
    let verification_before = [0; VERIFICATION_CURSOR_BYTES_V1];
    let selection_before = [0; SELECTION_CURSOR_BYTES];
    let certificate_before = [0; VERIFIED_CANDIDATE_BYTES_V1];
    let mut verification_scratch = [0; VERIFICATION_CURSOR_BYTES_V1];
    let mut verification = [0xa5; VERIFICATION_CURSOR_BYTES_V1];
    let mut selection_scratch = [0; SELECTION_CURSOR_BYTES];
    let mut selection = [0xa5; SELECTION_CURSOR_BYTES];
    let mut certificate_scratch = [0; VERIFIED_CANDIDATE_BYTES_V1];
    let mut certificate = [0xa5; VERIFIED_CANDIDATE_BYTES_V1];
    let summary = evaluate_consider_v2(
        ConsiderPlanViewV2 {
            candidate: &candidate,
            policy: &policy,
            page: &page,
            verification_before: &verification_before,
            selection_before: &selection_before,
            certificate_before: &certificate_before,
            incumbent_certificate: None,
            expected_revision: 0,
            limits: limits(),
        },
        ConsiderPlanBuffersV2 {
            verification_scratch: &mut verification_scratch,
            verification_output: &mut verification,
            selection_scratch: &mut selection_scratch,
            selection_output: &mut selection,
            certificate_scratch: &mut certificate_scratch,
            certificate_output: &mut certificate,
        },
    )
    .expect("consider");
    assert_eq!(
        summary,
        ConsiderPlanSummaryV2 {
            complete: true,
            order_count: 2,
            selection_considered: true,
        }
    );
    ConsiderFixture {
        candidate,
        policy,
        page,
        verification,
        selection,
        certificate,
    }
}

#[test]
fn verifier_selection_and_certificate_are_distinct_failure_atomic_banks() {
    let fixture = consider_fixture();
    assert_eq!(fixture.verification.len(), 960);
    let verifier = CandidateVerifierV1::decode(&fixture.verification).expect("verifier");
    assert!(verifier.is_complete());
    let selection = SelectionCursorV1::decode(&fixture.selection).expect("selection");
    assert_eq!(selection.best_candidate_id, Some(candidate().candidate_id));
    let certificate = VerifiedCandidateV1::decode(&fixture.certificate).expect("certificate");
    assert_eq!(certificate.candidate_id, candidate().candidate_id);

    let mut hostile_page = fixture.page;
    hostile_page[16] ^= 1;
    let verification_before = [0; VERIFICATION_CURSOR_BYTES_V1];
    let selection_before = [0; SELECTION_CURSOR_BYTES];
    let certificate_before = [0; VERIFIED_CANDIDATE_BYTES_V1];
    let mut verification_scratch = [0; VERIFICATION_CURSOR_BYTES_V1];
    let mut verification_output = [0x33; VERIFICATION_CURSOR_BYTES_V1];
    let mut selection_scratch = [0; SELECTION_CURSOR_BYTES];
    let mut selection_output = [0x44; SELECTION_CURSOR_BYTES];
    let mut certificate_scratch = [0; VERIFIED_CANDIDATE_BYTES_V1];
    let mut certificate_output = [0x55; VERIFIED_CANDIDATE_BYTES_V1];
    let before = (verification_output, selection_output, certificate_output);
    assert!(
        evaluate_consider_v2(
            ConsiderPlanViewV2 {
                candidate: &fixture.candidate,
                policy: &fixture.policy,
                page: &hostile_page,
                verification_before: &verification_before,
                selection_before: &selection_before,
                certificate_before: &certificate_before,
                incumbent_certificate: None,
                expected_revision: 0,
                limits: limits(),
            },
            ConsiderPlanBuffersV2 {
                verification_scratch: &mut verification_scratch,
                verification_output: &mut verification_output,
                selection_scratch: &mut selection_scratch,
                selection_output: &mut selection_output,
                certificate_scratch: &mut certificate_scratch,
                certificate_output: &mut certificate_output,
            },
        )
        .is_err()
    );
    assert_eq!(
        (verification_output, selection_output, certificate_output),
        before
    );
}

#[test]
fn incumbent_certificate_cannot_cross_product_or_batch_coordinates() {
    let fixture = consider_fixture();
    let mut substituted = fixture.certificate;
    substituted[80] ^= 1;
    assert_eq!(
        decode_incumbent(
            &fixture.selection,
            Some(&substituted),
            candidate(),
            policy(),
        ),
        Err(PlanErrorV2::CoordinateMismatch)
    );
}

#[test]
fn freeze_initialize_and_settlement_emit_plans_without_child_authority() {
    let fixture = consider_fixture();
    let selection = SelectionCursorV1::decode(&fixture.selection).expect("selection");
    let mut freeze_scratch = [0; SELECTION_CURSOR_BYTES];
    let mut frozen = [0xa5; SELECTION_CURSOR_BYTES];
    evaluate_freeze_v2(
        &fixture.selection,
        selection.revision,
        &mut freeze_scratch,
        &mut frozen,
    )
    .expect("freeze");

    let mut settlement_scratch = [0; SETTLEMENT_CURSOR_BYTES];
    let mut settlement = [0xa5; SETTLEMENT_CURSOR_BYTES];
    evaluate_initialize_settlement_v2(
        &frozen,
        &fixture.certificate,
        &fixture.candidate,
        0,
        &mut settlement_scratch,
        &mut settlement,
    )
    .expect("initialize");

    let mut cursor_scratch = [0; SETTLEMENT_CURSOR_BYTES];
    let mut cursor_output = [0x11; SETTLEMENT_CURSOR_BYTES];
    let mut first_scratch = [0; GENERAL_CHILD_PLAN_HEADER_BYTES_V2 + 8];
    let mut first_output = [0x22; GENERAL_CHILD_PLAN_HEADER_BYTES_V2 + 8];
    let mut second_scratch = [];
    let mut second_output = [];
    let summary = evaluate_settlement_v2(
        SettlementPlanViewV2 {
            action: Action::Collect,
            cursor_before: &settlement,
            certificate: &fixture.certificate,
            page: Some(&fixture.page),
            context: ExecutionContextV1 {
                market_id: id(9),
                release_set_id: id(10),
            },
            expected_revision: 0,
            surplus_route: None,
        },
        SettlementPlanBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            first_effect_scratch: &mut first_scratch,
            first_effect_output: &mut first_output,
            second_effect_scratch: &mut second_scratch,
            second_effect_output: &mut second_output,
        },
    )
    .expect("collect plan");
    assert_eq!(
        summary.first_effect_bytes,
        u32::try_from(GENERAL_CHILD_PLAN_HEADER_BYTES_V2 + 8).expect("effect width")
    );
    assert_eq!(summary.second_effect_bytes, 0);
    assert_eq!(&first_output[..8], &crate::GENERAL_CHILD_PLAN_MAGIC_V2);

    let outputs_before = (cursor_output, first_output, second_output);
    assert_eq!(
        evaluate_settlement_v2(
            SettlementPlanViewV2 {
                action: Action::Collect,
                cursor_before: &settlement,
                certificate: &fixture.certificate,
                page: Some(&fixture.page),
                context: ExecutionContextV1 {
                    market_id: id(9),
                    release_set_id: id(10),
                },
                expected_revision: 1,
                surplus_route: None,
            },
            SettlementPlanBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                first_effect_scratch: &mut first_scratch,
                first_effect_output: &mut first_output,
                second_effect_scratch: &mut second_scratch,
                second_effect_output: &mut second_output,
            },
        ),
        Err(PlanErrorV2::Transition)
    );
    assert_eq!((cursor_output, first_output, second_output), outputs_before);
}

#[test]
fn effect_capacity_cannot_truncate_or_extend_the_semantic_plan() {
    let fixture = consider_fixture();
    let selection = SelectionCursorV1::decode(&fixture.selection).expect("selection");
    let mut freeze_scratch = [0; SELECTION_CURSOR_BYTES];
    let mut frozen = [0; SELECTION_CURSOR_BYTES];
    evaluate_freeze_v2(
        &fixture.selection,
        selection.revision,
        &mut freeze_scratch,
        &mut frozen,
    )
    .expect("freeze");
    let mut settlement_scratch = [0; SETTLEMENT_CURSOR_BYTES];
    let mut settlement = [0; SETTLEMENT_CURSOR_BYTES];
    evaluate_initialize_settlement_v2(
        &frozen,
        &fixture.certificate,
        &fixture.candidate,
        0,
        &mut settlement_scratch,
        &mut settlement,
    )
    .expect("initialize");

    refuse_collect_capacity::<{ GENERAL_CHILD_PLAN_HEADER_BYTES_V2 + 7 }>(&fixture, &settlement, 0);
    refuse_collect_capacity::<{ GENERAL_CHILD_PLAN_HEADER_BYTES_V2 + 9 }>(&fixture, &settlement, 0);
    refuse_collect_capacity::<{ GENERAL_CHILD_PLAN_HEADER_BYTES_V2 + 8 }>(&fixture, &settlement, 1);
}

fn refuse_collect_capacity<const FIRST: usize>(
    fixture: &ConsiderFixture,
    settlement: &[u8; SETTLEMENT_CURSOR_BYTES],
    second_capacity: usize,
) {
    let mut cursor_scratch = [0; SETTLEMENT_CURSOR_BYTES];
    let mut cursor_output = [0x11; SETTLEMENT_CURSOR_BYTES];
    let mut first_scratch = [0; FIRST];
    let mut first_output = [0x22; FIRST];
    let mut second_scratch_storage = [0; 1];
    let mut second_output_storage = [0x33; 1];
    let cursor_before = cursor_output;
    let first_before = first_output;
    let second_before = second_output_storage;
    let second_scratch = &mut second_scratch_storage[..second_capacity];
    let second_output = &mut second_output_storage[..second_capacity];

    assert_eq!(
        evaluate_settlement_v2(
            SettlementPlanViewV2 {
                action: Action::Collect,
                cursor_before: settlement,
                certificate: &fixture.certificate,
                page: Some(&fixture.page),
                context: ExecutionContextV1 {
                    market_id: id(9),
                    release_set_id: id(10),
                },
                expected_revision: 0,
                surplus_route: None,
            },
            SettlementPlanBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                first_effect_scratch: &mut first_scratch,
                first_effect_output: &mut first_output,
                second_effect_scratch: second_scratch,
                second_effect_output: second_output,
            },
        ),
        Err(PlanErrorV2::EffectCapacity)
    );
    assert_eq!(cursor_output, cursor_before);
    assert_eq!(first_output, first_before);
    assert_eq!(second_output_storage, second_before);
}
