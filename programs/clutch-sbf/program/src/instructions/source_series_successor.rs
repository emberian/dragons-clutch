//! Current SourceSeries 77/v2 successor actions.
//!
//! These handlers are compiled so their exact account/state joins can be
//! reviewed, while central dispatch remains the sole all-or-none capability
//! owner. No handler or inner receipt independently enables a tuple.

use super::source_series::require_live_intent;
use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::source_plane_v3::{
    authenticate_evaluation_release_binding, authenticate_head, authenticate_lineage,
    authenticate_occurrence, authenticate_occurrence_window, authenticate_open_page,
    authenticate_persisted_window_evidence_account, authenticate_raw_page, authenticate_route,
    authenticate_persisted_result_account, authenticate_result_absence,
    authenticate_route_clock_bucket, authenticate_window_seal_absence,
    authenticate_reopen_generation_request,
    authenticate_statistic_key_input, authenticate_statistic_key_policy_input,
    authenticate_summary_program_input, authenticate_window_spec_input, authenticate_window_work,
    authenticate_work_receipt, invoke_statistic_evaluator, runtime_key,
    primary_maturity_handoff, source_refusal_handoff, successful_evaluation_handoff,
};
use crate::source_plane_v3_actions::{
    apply_postterminal_source_work_from_custody_v1, apply_source_work_liveness,
    authenticate_source_funding_custody_v1,
    authenticate_source_work_schedule_artifact, bind_work_execution,
    close_head_generation, close_open_page_generation, close_statistic_result_generation,
    close_window_work_generation, join_failure_absence_handoff, join_failure_result_handoff,
    authenticate_source_terminal_policy_for_close,
    fold_window_pages, initialize_window_work, join_successful_evaluation_handoff,
    persist_evaluation_result, persist_source_policy_handoff, reopen_runtime_account,
    seal_raw_page, seal_window,
};
use crate::instructions::failure_market_admission::authenticate_failure_market_root_v3;
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_adapter::{
    project_runtime_evaluate_statistic, project_runtime_fold_window_page,
    project_runtime_initialize_window_work, project_runtime_seal_raw_page,
    project_runtime_seal_window, IntentPreimageV3, RuntimeCloseProjectionV1,
    RuntimeCreationProjectionV1, RuntimeMutationProjectionV1,
};
use clutch_source_plane_v3_runtime::{
    account_data_id, AuthenticatedStatisticResultAbsenceV1,
    AuthenticatedStatisticResultAccountV1, FailurePolicySourceHandoffV1, LineageAccessV1,
    OccurrenceDispositionV1, SourceReopenTargetV1, SourceWorkKindV1,
    SuccessfulEvaluationHandoffV1,
};
use clutch_solana_layout::source_series::{
    CloseGenerationIntentV2, EmitFailureHandoffIntentV2, ReopenGenerationIntentV2,
    SourceHandoffKindV2, SourceMutableFamilyV2,
};
use clutch_source_plane_v3::StatisticResultStatusV3;
use solana_account_info::AccountInfo;
use solana_clock::Clock;
use solana_get_sysvar::GetSysvar;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const EVALUATOR_REQUEST_MAGIC_V1: [u8; 8] = *b"DCSPEV01";
const EVALUATOR_REQUEST_BYTES_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceHandoffFactV2 {
    FailureAbsence(
        FailurePolicySourceHandoffV1,
        AuthenticatedStatisticResultAbsenceV1,
    ),
    FailureResult(
        FailurePolicySourceHandoffV1,
        AuthenticatedStatisticResultAccountV1,
    ),
    Successful(
        SuccessfulEvaluationHandoffV1,
        AuthenticatedStatisticResultAccountV1,
    ),
}

/// Execute action 5 as one rollback domain: authenticate both mutable
/// generations, freeze an immutable page, advance SourceHead, close the
/// consumed open generation, validate the exact hostile intent postimage, and
/// debit the release-selected Source liveness compartment exactly once.
pub(super) fn process_seal_raw_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: IntentPreimageV3,
) -> Outcome<()> {
    require(sequence != 0, ClutchError::Replay)?;
    let call_ordinal =
        u32::try_from(sequence).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[18],
    )?;
    require_live_intent(program_id, &accounts[17], intent)?;
    let head_lineage =
        authenticate_lineage(program_id, route, &accounts[9], LineageAccessV1::Mutable)
            .map_err(Refusal::from)?;
    let head =
        authenticate_head(program_id, route, &accounts[8], head_lineage).map_err(Refusal::from)?;
    let open_lineage =
        authenticate_lineage(program_id, route, &accounts[11], LineageAccessV1::Mutable)
            .map_err(Refusal::from)?;
    let open = authenticate_open_page(program_id, route, head, &accounts[10], open_lineage)
        .map_err(Refusal::from)?;
    let head_before = head.head();
    let open_before = open.open();
    let execution = seal_raw_page(
        program_id,
        route,
        head,
        open,
        head_lineage,
        open_lineage,
        &accounts[8],
        &accounts[10],
        &accounts[9],
        &accounts[11],
        &accounts[12],
        custody,
        &accounts[18],
        &accounts[18],
        &accounts[13],
        &accounts[19],
        &accounts[20],
    )?;
    let close = execution.open_close.funding;
    require(
        execution.head.account_data_before_id == head.account_data_id()
            && close.account == open.account()
            && close.generation == open.terminal_generation()
            && close.terminal_receipt_id == execution.semantic.transition_receipt_id
            && execution.page_funding.account == runtime_key(accounts[12].key)
            && execution.page_header.generation == 1,
        ClutchError::MismatchedState,
    )?;
    let plan = project_runtime_seal_raw_page(
        &route.source_plane(),
        &head_before,
        &open_before,
        &execution.semantic.head_after,
        &execution.semantic.sealed_page,
        RuntimeMutationProjectionV1 {
            account_data_before_id: execution.head.account_data_before_id,
            account_data_after_id: execution.head.account_data_after_id,
            generation: head.terminal_generation(),
        },
        RuntimeCloseProjectionV1 {
            account_data_id: open.account_data_id(),
            generation: close.generation,
            principal_recipient: ContentId::from_bytes(close.principal_recipient.bytes()),
            payer_principal_lamports: close.payer_refund_lamports,
            neutral_sink: ContentId::from_bytes(close.neutral_sink.bytes()),
            neutral_surplus_lamports: close.neutral_surplus_lamports,
        },
        RuntimeCreationProjectionV1 {
            account_data_id: execution.page_account_data_id,
            generation: execution.page_header.generation,
            payer: ContentId::from_bytes(execution.page_funding.payer.bytes()),
            rent_principal_lamports: execution.page_funding.payer_debit_lamports,
        },
        execution.semantic.transition_receipt_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    intent
        .validate_for_program(ContentId::from_bytes(program_id.to_bytes()), plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_receipt_id = plan
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let kind = SourceWorkKindV1::SealRawPage;
    let ceiling = schedule.ceiling_for(kind);
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        semantic_receipt_id,
        &accounts[14],
        call_ordinal,
        ceiling,
        accounts[17].key,
        ceiling,
        custody,
        &accounts[18],
        &accounts[19],
        &accounts[20],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work,
        &accounts[15],
        &accounts[16],
        &accounts[17],
        &accounts[18],
    )?;
    Ok(())
}

/// Execute action 6 from the exact Product occurrence and its immutable
/// content-addressed WindowSpec, while keeping capability admission false.
pub(super) fn process_initialize_window_work(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: IntentPreimageV3,
) -> Outcome<()> {
    require(sequence != 0, ClutchError::Replay)?;
    let call_ordinal =
        u32::try_from(sequence).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[16],
    )?;
    require_live_intent(program_id, &accounts[15], intent)?;
    let window_input =
        authenticate_window_spec_input(program_id, route, &accounts[9]).map_err(Refusal::from)?;
    let window = window_input.window();
    let occurrence = authenticate_occurrence_window(
        program_id,
        route,
        &accounts[8],
        OccurrenceDispositionV1::ExactExisting,
        &window,
    )
    .map_err(Refusal::from)?;
    let work = clutch_source_plane_v3::WindowWorkV3::new(&window)
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    let opened = initialize_window_work(
        program_id,
        route,
        &window,
        custody,
        &accounts[16],
        &accounts[10],
        &accounts[11],
        &accounts[17],
        &accounts[18],
    )?;
    let ledger = opened.funding.ledger;
    let plan = project_runtime_initialize_window_work(
        &route.source_plane(),
        &window,
        &work,
        RuntimeCreationProjectionV1 {
            account_data_id: opened.account_data_id,
            generation: opened.header.generation,
            payer: ContentId::from_bytes(ledger.principal_recipient.bytes()),
            rent_principal_lamports: ledger.payer_principal_lamports,
        },
        ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                b"dragons-clutch/source-window-work-authority/v1",
                &occurrence.id().bytes(),
                &window_input.id().bytes(),
            ])
            .to_bytes(),
        ),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    intent
        .validate_for_program(ContentId::from_bytes(program_id.to_bytes()), plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_receipt_id = plan
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let kind = SourceWorkKindV1::TerminalLifecycle;
    let ceiling = schedule.ceiling_for(kind);
    let work_execution = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        semantic_receipt_id,
        &accounts[12],
        call_ordinal,
        ceiling,
        accounts[15].key,
        ceiling,
        custody,
        &accounts[16],
        &accounts[17],
        &accounts[18],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work_execution,
        &accounts[13],
        &accounts[14],
        &accounts[15],
        &accounts[16],
    )?;
    Ok(())
}

/// Execute one bounded action-7 page fold from exact occurrence, WindowSpec,
/// mutable work lineage, and immutable page authorities.
pub(super) fn process_fold_window_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: IntentPreimageV3,
) -> Outcome<()> {
    require(sequence != 0, ClutchError::Replay)?;
    let call_ordinal =
        u32::try_from(sequence).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[17],
    )?;
    require_live_intent(program_id, &accounts[16], intent)?;
    let window_input =
        authenticate_window_spec_input(program_id, route, &accounts[9]).map_err(Refusal::from)?;
    let window = window_input.window();
    let occurrence = authenticate_occurrence_window(
        program_id,
        route,
        &accounts[8],
        OccurrenceDispositionV1::ExactExisting,
        &window,
    )
    .map_err(Refusal::from)?;
    let work_lineage =
        authenticate_lineage(program_id, route, &accounts[11], LineageAccessV1::Mutable)
            .map_err(Refusal::from)?;
    let work = authenticate_window_work(
        program_id,
        route,
        &accounts[10],
        &window,
        work_lineage,
    )
    .map_err(Refusal::from)?;
    let page = authenticate_raw_page(program_id, route, &accounts[12]).map_err(Refusal::from)?;
    let execution = fold_window_pages(
        route,
        &window,
        work,
        &[page],
        &accounts[10],
        &accounts[11],
        work_lineage,
    )?;
    require(
        execution.mutation.account_data_before_id == work.account_data_id(),
        ClutchError::MismatchedState,
    )?;
    let fold_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/source-window-fold-authority/v1",
            &occurrence.id().bytes(),
            &window_input.id().bytes(),
            &page.id().bytes(),
            &execution.semantic.fold_receipt_id.bytes(),
        ])
        .to_bytes(),
    );
    let plan = project_runtime_fold_window_page(
        &route.source_plane(),
        &window,
        &work.work(),
        &page.page(),
        &execution.semantic.work_after,
        RuntimeMutationProjectionV1 {
            account_data_before_id: execution.mutation.account_data_before_id,
            account_data_after_id: execution.mutation.account_data_after_id,
            generation: work.terminal_generation(),
        },
        fold_authentication_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    intent
        .validate_for_program(ContentId::from_bytes(program_id.to_bytes()), plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_receipt_id = plan
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let kind = SourceWorkKindV1::FoldWindowPages;
    let ceiling = schedule.ceiling_for(kind);
    let work_execution = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        semantic_receipt_id,
        &accounts[13],
        call_ordinal,
        ceiling,
        accounts[16].key,
        ceiling,
        custody,
        &accounts[17],
        &accounts[18],
        &accounts[19],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work_execution,
        &accounts[14],
        &accounts[15],
        &accounts[16],
        &accounts[17],
    )?;
    Ok(())
}

/// Execute action 8 from the exact mature Clock/page/work evidence, persist
/// the immutable WindowSeal, and close the consumed work generation once.
pub(super) fn process_seal_window(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: IntentPreimageV3,
) -> Outcome<()> {
    require(sequence != 0, ClutchError::Replay)?;
    let call_ordinal =
        u32::try_from(sequence).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[20],
    )?;
    require_live_intent(program_id, &accounts[19], intent)?;
    let clock = authenticate_route_clock_bucket(route, &accounts[8]).map_err(Refusal::from)?;
    let window_input = authenticate_window_spec_input(program_id, route, &accounts[10])
        .map_err(Refusal::from)?;
    let window = window_input.window();
    let occurrence = authenticate_occurrence_window(
        program_id,
        route,
        &accounts[9],
        OccurrenceDispositionV1::ExactExisting,
        &window,
    )
    .map_err(Refusal::from)?;
    let work_lineage =
        authenticate_lineage(program_id, route, &accounts[12], LineageAccessV1::Mutable)
            .map_err(Refusal::from)?;
    let work = authenticate_window_work(
        program_id,
        route,
        &accounts[11],
        &window,
        work_lineage,
    )
    .map_err(Refusal::from)?;
    let maturity_page =
        authenticate_raw_page(program_id, route, &accounts[13]).map_err(Refusal::from)?;
    let execution = seal_window(
        program_id,
        route,
        &route.source_plane(),
        &route.clock_policy(),
        clock.snapshot(),
        &window,
        work,
        maturity_page,
        work_lineage,
        &accounts[11],
        &accounts[12],
        &accounts[14],
        custody,
        &accounts[20],
        &accounts[20],
        &accounts[15],
        &accounts[21],
        &accounts[22],
    )?;
    let close = execution.work_close.funding;
    require(
        close.account == work.account()
            && close.generation == work.terminal_generation()
            && close.terminal_receipt_id == execution.evidence.id()
            && execution.seal_funding.account == runtime_key(accounts[14].key)
            && execution.seal_header.generation == 1,
        ClutchError::MismatchedState,
    )?;
    let evidence_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/source-window-seal-authority/v1",
            &occurrence.id().bytes(),
            &window_input.id().bytes(),
            &clock.id().bytes(),
            &maturity_page.id().bytes(),
            &execution.evidence.id().bytes(),
        ])
        .to_bytes(),
    );
    let plan = project_runtime_seal_window(
        &route.source_plane(),
        &window,
        &work.work(),
        &maturity_page.page(),
        &execution.evidence.closure(),
        &execution.evidence.seal(),
        RuntimeCloseProjectionV1 {
            account_data_id: work.account_data_id(),
            generation: close.generation,
            principal_recipient: ContentId::from_bytes(close.principal_recipient.bytes()),
            payer_principal_lamports: close.payer_refund_lamports,
            neutral_sink: ContentId::from_bytes(close.neutral_sink.bytes()),
            neutral_surplus_lamports: close.neutral_surplus_lamports,
        },
        RuntimeCreationProjectionV1 {
            account_data_id: execution.seal_account_data_id,
            generation: execution.seal_header.generation,
            payer: ContentId::from_bytes(execution.seal_funding.payer.bytes()),
            rent_principal_lamports: execution.seal_funding.payer_debit_lamports,
        },
        evidence_authentication_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    intent
        .validate_for_program(ContentId::from_bytes(program_id.to_bytes()), plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_receipt_id = plan
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let kind = SourceWorkKindV1::SealWindow;
    let ceiling = schedule.ceiling_for(kind);
    let work_execution = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        semantic_receipt_id,
        &accounts[16],
        call_ordinal,
        ceiling,
        accounts[19].key,
        ceiling,
        custody,
        &accounts[20],
        &accounts[21],
        &accounts[22],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work_execution,
        &accounts[17],
        &accounts[18],
        &accounts[19],
        &accounts[20],
    )?;
    Ok(())
}

fn evaluator_instruction_v1(
    evaluator_program: &Pubkey,
    window_account: &AccountInfo<'_>,
    statistic_key_account: &AccountInfo<'_>,
    summary_account: &AccountInfo<'_>,
    seal_account: &AccountInfo<'_>,
    route_id: ContentId,
    occurrence_id: ContentId,
    window_id: ContentId,
    statistic_key_id: ContentId,
    summary_id: ContentId,
    seal_id: ContentId,
    evidence_id: ContentId,
    clock_slot: u64,
    clock_unix_timestamp: u64,
) -> Instruction {
    let mut data = std::vec![0_u8; EVALUATOR_REQUEST_BYTES_V1];
    data[..8].copy_from_slice(&EVALUATOR_REQUEST_MAGIC_V1);
    data[8..10].copy_from_slice(&1_u16.to_le_bytes());
    data[16..48].copy_from_slice(&route_id.bytes());
    data[48..80].copy_from_slice(&occurrence_id.bytes());
    data[80..112].copy_from_slice(&window_id.bytes());
    data[112..144].copy_from_slice(&statistic_key_id.bytes());
    data[144..176].copy_from_slice(&summary_id.bytes());
    data[176..208].copy_from_slice(&seal_id.bytes());
    data[208..240].copy_from_slice(&evidence_id.bytes());
    data[240..248].copy_from_slice(&clock_slot.to_le_bytes());
    data[248..256].copy_from_slice(&clock_unix_timestamp.to_le_bytes());
    Instruction {
        program_id: *evaluator_program,
        accounts: std::vec![
            AccountMeta::new_readonly(*window_account.key, false),
            AccountMeta::new_readonly(*statistic_key_account.key, false),
            AccountMeta::new_readonly(*summary_account.key, false),
            AccountMeta::new_readonly(*seal_account.key, false),
        ],
        data,
    }
}

/// Execute action 9 from the current release's exact evaluator Program,
/// ProgramData/ELF, SummaryProgram semantics, persisted action-8 seal, and
/// Product occurrence. The CPI request contains no caller-provided semantic
/// coordinates and the result/work/liveness writes share one rollback domain.
pub(super) fn process_evaluate_statistic(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: IntentPreimageV3,
) -> Outcome<()> {
    require(sequence != 0, ClutchError::Replay)?;
    let call_ordinal =
        u32::try_from(sequence).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[22],
    )?;
    require_live_intent(program_id, &accounts[21], intent)?;
    let clock = authenticate_route_clock_bucket(route, &accounts[8]).map_err(Refusal::from)?;
    let window_input = authenticate_window_spec_input(program_id, route, &accounts[10])
        .map_err(Refusal::from)?;
    let summary_input = authenticate_summary_program_input(program_id, route, &accounts[12])
        .map_err(Refusal::from)?;
    let statistic_key_input = authenticate_statistic_key_input(
        program_id,
        route,
        &accounts[11],
        window_input,
        summary_input,
    )
    .map_err(Refusal::from)?;
    let window = window_input.window();
    let summary = summary_input.summary();
    let key = statistic_key_input.key();
    let occurrence = authenticate_occurrence(
        program_id,
        route,
        &accounts[9],
        OccurrenceDispositionV1::ExactExisting,
        &window,
        &key,
    )
    .map_err(Refusal::from)?;
    let evidence = authenticate_persisted_window_evidence_account(
        program_id,
        route,
        &accounts[13],
        clock,
        &window,
    )
    .map_err(Refusal::from)?;
    let binding = authenticate_evaluation_release_binding(
        route,
        summary,
        &accounts[14],
        &accounts[15],
    )
    .map_err(Refusal::from)?;
    let evaluator_instruction = evaluator_instruction_v1(
        accounts[14].key,
        &accounts[10],
        &accounts[11],
        &accounts[12],
        &accounts[13],
        route.route_id(),
        occurrence.id(),
        window.id().map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?,
        key.id().map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?,
        summary
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?,
        evidence
            .seal()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?,
        evidence.id(),
        clock.snapshot().slot,
        clock.snapshot().unix_timestamp,
    );
    let evaluator_accounts = [
        accounts[10].clone(),
        accounts[11].clone(),
        accounts[12].clone(),
        accounts[13].clone(),
        accounts[14].clone(),
    ];
    let evaluation = invoke_statistic_evaluator(
        route,
        binding,
        summary,
        &accounts[14],
        &accounts[15],
        clock.snapshot(),
        &window,
        &key,
        evidence,
        &evaluator_instruction,
        &evaluator_accounts,
    )
    .map_err(Refusal::from)?;
    let result_lineage = authenticate_lineage(
        program_id,
        route,
        &accounts[17],
        LineageAccessV1::Mutable,
    )
    .map_err(Refusal::from)?;
    let result = persist_evaluation_result(
        program_id,
        route,
        &key,
        evidence,
        evaluation,
        result_lineage,
        custody,
        &accounts[22],
        &accounts[16],
        &accounts[17],
        &accounts[23],
        &accounts[24],
    )?;
    let ledger = result.funding.ledger;
    let evaluation_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/source-evaluation-action-authority/v1",
            &occurrence.id().bytes(),
            &window_input.id().bytes(),
            &summary_input.id().bytes(),
            &statistic_key_input.id().bytes(),
            &evidence.id().bytes(),
            &evaluation.id().bytes(),
        ])
        .to_bytes(),
    );
    let plan = project_runtime_evaluate_statistic(
        &route.source_plane(),
        &window,
        &key,
        &summary,
        &evidence.seal(),
        &evaluation.result(),
        RuntimeCreationProjectionV1 {
            account_data_id: result.account_data_id,
            generation: result.header.generation,
            payer: ContentId::from_bytes(ledger.principal_recipient.bytes()),
            rent_principal_lamports: ledger.payer_principal_lamports,
        },
        evaluation_authentication_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    intent
        .validate_for_program(ContentId::from_bytes(program_id.to_bytes()), plan)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let kind = SourceWorkKindV1::EvaluateStatistic;
    let ceiling = schedule.ceiling_for(kind);
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        result.account_data_id,
        &accounts[18],
        call_ordinal,
        ceiling,
        accounts[21].key,
        ceiling,
        custody,
        &accounts[22],
        &accounts[23],
        &accounts[24],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work,
        &accounts[19],
        &accounts[20],
        &accounts[21],
        &accounts[22],
    )?;
    Ok(())
}

/// Persist one exhaustive action-10 Source handoff after joining the exact
/// shared-Market Failure policy, Product occurrence, immutable Source facts,
/// and a newly paid FailureHandoff work receipt.
///
/// This instruction does not classify a relation or write ResolutionV5. The
/// durable private postwrite is consumed later together with the full current
/// ProfileV4/BundleV5 Product route.
pub(super) fn process_emit_source_handoff(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: EmitFailureHandoffIntentV2,
) -> Outcome<()> {
    require(sequence != 0, ClutchError::Replay)?;
    let call_ordinal =
        u32::try_from(sequence).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[21],
    )?;
    let clock = authenticate_route_clock_bucket(route, &accounts[8]).map_err(Refusal::from)?;
    require(
        clock.snapshot().slot < intent.valid_before_slot,
        ClutchError::Replay,
    )?;
    let failure = authenticate_failure_market_root_v3(program_id, &accounts[16], false)?;
    let failure_binding = failure.state().binding();
    let failure_facts = failure_binding.facts();
    let failure_policy_binding_id = ContentId::from_bytes(failure_binding.id().bytes());
    let summary_program_id = ContentId::from_bytes(failure_facts.summary_program_id.bytes());
    require(
        failure_facts.source_release_manifest_id.bytes() == route.release_manifest_id().bytes()
            && failure_facts.source_release_authentication_id.bytes()
                == route.release_authentication_id().bytes()
            && failure_facts.source_release_account_id.bytes() == route.release_account().bytes()
            && failure_facts.source_plane_contract_id.bytes()
                == route.source_plane_contract_id().bytes()
            && failure_facts.source_spec_id.bytes() == route.source_spec_id().bytes()
            && failure_facts.clock_policy_id.bytes() == route.clock_policy_id().bytes(),
        ClutchError::MismatchedState,
    )?;
    let window_input = authenticate_window_spec_input(program_id, route, &accounts[10])
        .map_err(Refusal::from)?;
    let window = window_input.window();
    let statistic_key_input = authenticate_statistic_key_policy_input(
        program_id,
        route,
        &accounts[11],
        window_input,
        summary_program_id,
    )
    .map_err(Refusal::from)?;
    let key = statistic_key_input.key();
    require(
        failure_facts.primary_window_id.bytes() == window.id().map_err(|_| {
            Refusal::Adapter(ClutchError::SourceAdmissionFailed)
        })?.bytes()
            && failure_facts.statistic_key_id.bytes()
                == key
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?
                    .bytes(),
        ClutchError::MismatchedState,
    )?;
    let occurrence = authenticate_occurrence(
        program_id,
        route,
        &accounts[9],
        OccurrenceDispositionV1::ExactExisting,
        &window,
        &key,
    )
    .map_err(Refusal::from)?;
    require(
        occurrence.market_instance_id().bytes() == failure_facts.market_instance_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let result_lineage =
        crate::source_plane_v3::authenticate_lineage(
            program_id,
            route,
            &accounts[14],
            LineageAccessV1::ReadOnly,
        )
        .map_err(Refusal::from)?;
    let (handoff_id, source_fact) = match intent.kind {
        SourceHandoffKindV2::FailureAbsence => {
            let _seal_absence = authenticate_window_seal_absence(
                program_id,
                route,
                &accounts[12],
                &window,
            )
            .map_err(Refusal::from)?;
            let absence = authenticate_result_absence(
                program_id,
                route,
                &accounts[13],
                &key,
                result_lineage,
            )
            .map_err(Refusal::from)?;
            let handoff = primary_maturity_handoff(
                route,
                failure_policy_binding_id,
                occurrence,
                clock,
                &window,
                absence,
            )
            .map_err(Refusal::from)?;
            (handoff.id(), SourceHandoffFactV2::FailureAbsence(handoff, absence))
        }
        SourceHandoffKindV2::FailureResult | SourceHandoffKindV2::SuccessfulEvaluation => {
            let evidence = authenticate_persisted_window_evidence_account(
                program_id,
                route,
                &accounts[12],
                clock,
                &window,
            )
            .map_err(Refusal::from)?;
            let result = authenticate_persisted_result_account(
                program_id,
                route,
                &accounts[13],
                &window,
                &key,
                summary_program_id,
                evidence,
                result_lineage,
            )
            .map_err(Refusal::from)?;
            match intent.kind {
                SourceHandoffKindV2::FailureResult => {
                    require(
                        result.result().status() == StatisticResultStatusV3::Refused,
                        ClutchError::MismatchedState,
                    )?;
                    let handoff = source_refusal_handoff(
                        route,
                        failure_policy_binding_id,
                        occurrence,
                        clock,
                        &window,
                        evidence,
                        result,
                    )
                    .map_err(Refusal::from)?;
                    (handoff.id(), SourceHandoffFactV2::FailureResult(handoff, result))
                }
                SourceHandoffKindV2::SuccessfulEvaluation => {
                    require(
                        result.result().status() == StatisticResultStatusV3::Success,
                        ClutchError::MismatchedState,
                    )?;
                    let handoff = successful_evaluation_handoff(
                        route,
                        failure_policy_binding_id,
                        occurrence,
                        clock,
                        &window,
                        evidence,
                        result,
                    )
                    .map_err(Refusal::from)?;
                    (handoff.id(), SourceHandoffFactV2::Successful(handoff, result))
                }
                SourceHandoffKindV2::FailureAbsence => {
                    return Err(ClutchError::NonCanonical.into());
                }
            }
        }
    };
    require(handoff_id.bytes() == intent.handoff_id, ClutchError::MismatchedState)?;
    let kind = SourceWorkKindV1::FailureHandoff;
    let ceiling = schedule.ceiling_for(kind);
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        handoff_id,
        &accounts[15],
        call_ordinal,
        ceiling,
        accounts[20].key,
        ceiling,
        custody,
        &accounts[21],
        &accounts[22],
        &accounts[23],
    )?;
    let authenticated_work = work.authenticated_receipt();
    require(
        authenticated_work.id().bytes() == intent.source_work_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let joined = match source_fact {
        SourceHandoffFactV2::FailureAbsence(handoff, absence) => {
            join_failure_absence_handoff(route, handoff, absence, authenticated_work)?
        }
        SourceHandoffFactV2::FailureResult(handoff, result) => {
            join_failure_result_handoff(route, handoff, result, authenticated_work)?
        }
        SourceHandoffFactV2::Successful(handoff, result) => {
            join_successful_evaluation_handoff(route, handoff, result, authenticated_work)?
        }
    };
    let persisted = persist_source_policy_handoff(
        program_id,
        route,
        joined,
        custody,
        &accounts[21],
        &accounts[17],
        &accounts[22],
        &accounts[23],
    )?;
    require(
        persisted.authenticated().source_policy_handoff_join_id() == joined.id(),
        ClutchError::MismatchedState,
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work,
        &accounts[18],
        &accounts[19],
        &accounts[20],
        &accounts[21],
    )?;
    Ok(())
}

/// Retire one exact mutable Source generation only after the shared Product
/// writer has minted the route/schedule/generation-bound terminal-success
/// receipt. The payload terminal digest is comparison-only.
pub(super) fn process_close_generation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: CloseGenerationIntentV2,
) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[12],
    )?;
    let clock = Clock::get().map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    require(
        clock.slot < intent.valid_before_slot
            && route.release_manifest_id().bytes() == intent.source_release_manifest_id,
        ClutchError::MismatchedState,
    )?;
    let lineage = authenticate_lineage(program_id, route, &accounts[10], LineageAccessV1::Mutable)
        .map_err(Refusal::from)?;
    let expected_family = match intent.family {
        SourceMutableFamilyV2::SourceHead => {
            clutch_source_plane_v3_runtime::LineageFamilyV1::SourceHead
        }
        SourceMutableFamilyV2::OpenRawPage => {
            clutch_source_plane_v3_runtime::LineageFamilyV1::OpenRawPage
        }
        SourceMutableFamilyV2::WindowWork => {
            clutch_source_plane_v3_runtime::LineageFamilyV1::WindowWork
        }
        SourceMutableFamilyV2::StatisticResult => {
            clutch_source_plane_v3_runtime::LineageFamilyV1::StatisticResult
        }
    };
    require(
        lineage.account_data_id().bytes() == intent.expected_lineage_state_id
            && lineage.lineage().family == expected_family
            && lineage.lineage().is_open
            && lineage.lineage().active_account == runtime_key(accounts[9].key),
        ClutchError::MismatchedState,
    )?;
    let terminal = authenticate_work_receipt(program_id, route, schedule, &accounts[11])
        .map_err(Refusal::from)?;
    require(
        terminal.receipt().semantic_receipt_id().bytes()
            == intent.semantic_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let expected_closed_lineage_state_id = authenticate_source_terminal_policy_for_close(
        program_id,
        route,
        lineage,
        terminal.receipt().semantic_receipt_id(),
        &accounts[8],
    )?;
    let target_generation = lineage.lineage().latest_generation;
    let close = match intent.family {
        SourceMutableFamilyV2::SourceHead => close_head_generation(
            program_id,
            route,
            lineage,
            &accounts[9],
            &accounts[10],
            &accounts[12],
            &accounts[13],
            terminal,
        ),
        SourceMutableFamilyV2::OpenRawPage => close_open_page_generation(
            program_id,
            route,
            lineage,
            &accounts[9],
            &accounts[10],
            &accounts[12],
            &accounts[13],
            terminal,
        ),
        SourceMutableFamilyV2::WindowWork => close_window_work_generation(
            program_id,
            route,
            lineage,
            &accounts[9],
            &accounts[10],
            &accounts[12],
            &accounts[13],
            terminal,
        ),
        SourceMutableFamilyV2::StatisticResult => close_statistic_result_generation(
            program_id,
            route,
            lineage,
            &accounts[9],
            &accounts[10],
            &accounts[12],
            &accounts[13],
            terminal,
        ),
    }?;
    let closed_lineage_bytes = close
        .lineage_after
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let closed_lineage_state_id = account_data_id(
        runtime_key(accounts[10].key),
        &closed_lineage_bytes,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        !close.lineage_after.is_open
            && custody.account() == runtime_key(accounts[12].key)
            && close.funding.generation == target_generation
            && close.funding.terminal_receipt_id.bytes()
                == intent.semantic_terminal_receipt_id
            && close.lineage_after.last_close_receipt_id.bytes()
                == intent.semantic_terminal_receipt_id
            && closed_lineage_state_id == expected_closed_lineage_state_id,
        ClutchError::MismatchedState,
    )?;
    Ok(())
}

/// Reopen one exact closed Source generation from the immutable typed target
/// persisted by the release-selected Product/Failure generation owner. The
/// payload supplies only comparison digests and never supplies body bytes.
/// The new generation is capitalized from the same bounded lifecycle custody
/// and emits one paid TerminalLifecycle work receipt. The keeper signs the
/// call but never becomes the semantic rent payer or refund owner.
pub(super) fn process_reopen_generation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: ReopenGenerationIntentV2,
) -> Outcome<()> {
    require(sequence != 0, ClutchError::Replay)?;
    let route = authenticate_route(
        program_id,
        &accounts[0],
        &accounts[1],
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
    )
    .map_err(Refusal::from)?;
    let schedule = authenticate_source_work_schedule_artifact(program_id, route, &accounts[7])?;
    let custody = authenticate_source_funding_custody_v1(
        program_id, route, schedule, &accounts[13],
    )?;
    let clock = Clock::get().map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    require(
        clock.slot < intent.valid_before_slot
            && route.release_manifest_id().bytes() == intent.source_release_manifest_id,
        ClutchError::MismatchedState,
    )?;
    let lineage = authenticate_lineage(program_id, route, &accounts[10], LineageAccessV1::Mutable)
        .map_err(Refusal::from)?;
    let authorization = authenticate_reopen_generation_request(route, &accounts[8], lineage)
        .map_err(Refusal::from)?;
    let target = authorization.target();
    let requested_family = match intent.family {
        SourceMutableFamilyV2::SourceHead => {
            clutch_source_plane_v3_runtime::SourceReopenFamilyV1::SourceHead
        }
        SourceMutableFamilyV2::OpenRawPage => {
            clutch_source_plane_v3_runtime::SourceReopenFamilyV1::OpenRawPage
        }
        SourceMutableFamilyV2::WindowWork => {
            clutch_source_plane_v3_runtime::SourceReopenFamilyV1::WindowWork
        }
        SourceMutableFamilyV2::StatisticResult => {
            clutch_source_plane_v3_runtime::SourceReopenFamilyV1::StatisticResult
        }
    };
    let recipe = target
        .recipe(route)
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    let recipe_id = recipe
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::WrongPda))?;
    let target_body_id = target
        .body_id()
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    require(
        target.family() == requested_family
            && authorization.expected_lineage_state_id().bytes()
                == intent.expected_lineage_state_id
            && lineage.account_data_id().bytes() == intent.expected_lineage_state_id
            && lineage.lineage().semantic_binding_id == recipe_id
            && recipe_id.bytes() == intent.semantic_binding_id
            && target_body_id.bytes() == intent.target_body_id,
        ClutchError::MismatchedState,
    )?;
    let opened = match target {
        SourceReopenTargetV1::SourceHead(body) => reopen_runtime_account(
            program_id,
            route,
            lineage,
            clutch_source_plane_v3_runtime::LineageFamilyV1::SourceHead,
            recipe_id,
            &recipe,
            body,
            custody,
            &accounts[13],
            &accounts[9],
            &accounts[10],
            &accounts[14],
            &accounts[15],
        ),
        SourceReopenTargetV1::OpenRawPage(body) => reopen_runtime_account(
            program_id,
            route,
            lineage,
            clutch_source_plane_v3_runtime::LineageFamilyV1::OpenRawPage,
            recipe_id,
            &recipe,
            body,
            custody,
            &accounts[13],
            &accounts[9],
            &accounts[10],
            &accounts[14],
            &accounts[15],
        ),
        SourceReopenTargetV1::WindowWork(body) => reopen_runtime_account(
            program_id,
            route,
            lineage,
            clutch_source_plane_v3_runtime::LineageFamilyV1::WindowWork,
            recipe_id,
            &recipe,
            body,
            custody,
            &accounts[13],
            &accounts[9],
            &accounts[10],
            &accounts[14],
            &accounts[15],
        ),
        SourceReopenTargetV1::StatisticResult(body) => reopen_runtime_account(
            program_id,
            route,
            lineage,
            clutch_source_plane_v3_runtime::LineageFamilyV1::StatisticResult,
            recipe_id,
            &recipe,
            body,
            custody,
            &accounts[13],
            &accounts[9],
            &accounts[10],
            &accounts[14],
            &accounts[15],
        ),
    }?;
    let lineage_after_id = opened
        .lineage_after
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    require(
        opened.header.generation
                == lineage
                    .lineage()
                    .latest_generation
                    .checked_add(1)
                    .ok_or(ClutchError::Arithmetic)?
            && opened.lineage_after.is_open
            && opened.lineage_after.active_account == runtime_key(accounts[9].key)
            && opened.lineage_after.last_opened_state_id == opened.account_data_id
            && opened.lineage_after.last_close_receipt_id == ContentId::ZERO
            && authorization.generation_policy_id()
                == lineage.lineage().last_close_receipt_id
            && !lineage_after_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    let semantic_receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/source-reopen-action/v1",
            &route.route_id().bytes(),
            &authorization.id().bytes(),
            &opened.account_data_id.bytes(),
            &lineage_after_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!semantic_receipt_id.is_zero(), ClutchError::MismatchedState)?;
    let kind = SourceWorkKindV1::TerminalLifecycle;
    let ceiling = schedule.ceiling_for(kind);
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        semantic_receipt_id,
        &accounts[11],
        u32::try_from(sequence).map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
        ceiling,
        accounts[12].key,
        ceiling,
        custody,
        &accounts[13],
        &accounts[14],
        &accounts[15],
    )?;
    let postterminal = apply_postterminal_source_work_from_custody_v1(
        program_id,
        route,
        schedule,
        work,
        &accounts[13],
        &accounts[12],
        &accounts[14],
    )?;
    require(!postterminal.id().is_zero(), ClutchError::MismatchedState)?;
    Ok(())
}
