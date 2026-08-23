//! Capability-false SourceSeries 77/v2 successor actions.
//!
//! These handlers are compiled so their exact account/state joins can be
//! reviewed, but central dispatch remains the sole capability owner. Actions
//! 5 through 12 stay disabled in every checked profile until the complete
//! Source-to-ResolutionV5 chain is admitted.

use super::source_series::require_live_intent;
use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::source_plane_v3::{
    authenticate_evaluation_release_binding, authenticate_head, authenticate_lineage,
    authenticate_occurrence, authenticate_occurrence_window, authenticate_open_page,
    authenticate_persisted_window_evidence_account, authenticate_raw_page, authenticate_route,
    authenticate_route_clock_bucket, authenticate_statistic_key_input,
    authenticate_summary_program_input, authenticate_window_spec_input, authenticate_window_work,
    invoke_statistic_evaluator, runtime_key,
};
use crate::source_plane_v3_actions::{
    apply_source_work_liveness, authenticate_source_work_schedule_artifact, bind_work_execution,
    fold_window_pages, initialize_window_work, persist_evaluation_result, seal_raw_page,
    seal_window,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_adapter::{
    project_runtime_evaluate_statistic, project_runtime_fold_window_page,
    project_runtime_initialize_window_work, project_runtime_seal_raw_page,
    project_runtime_seal_window, IntentPreimageV3, RuntimeCloseProjectionV1,
    RuntimeCreationProjectionV1, RuntimeMutationProjectionV1,
};
use clutch_source_plane_v3_runtime::{
    LineageAccessV1, OccurrenceDispositionV1, SourceWorkKindV1,
};
use solana_account_info::AccountInfo;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const EVALUATOR_REQUEST_MAGIC_V1: [u8; 8] = *b"DCSPEV01";
const EVALUATOR_REQUEST_BYTES_V1: usize = 256;

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
    require(
        runtime_key(accounts[19].key) == schedule.payer(),
        ClutchError::MismatchedState,
    )?;
    require_live_intent(program_id, &accounts[18], intent)?;
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
        &accounts[19],
        &accounts[13],
        &accounts[14],
        &accounts[20],
        &accounts[21],
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
        &accounts[15],
        call_ordinal,
        ceiling,
        accounts[18].key,
        ceiling,
        &accounts[19],
        &accounts[20],
        &accounts[21],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work,
        &accounts[16],
        &accounts[17],
        &accounts[18],
        &accounts[19],
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
    require(
        runtime_key(accounts[16].key) == schedule.payer(),
        ClutchError::MismatchedState,
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
    require(
        runtime_key(accounts[17].key) == schedule.payer(),
        ClutchError::MismatchedState,
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
    require(
        runtime_key(accounts[21].key) == schedule.payer(),
        ClutchError::MismatchedState,
    )?;
    require_live_intent(program_id, &accounts[20], intent)?;
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
        &accounts[21],
        &accounts[15],
        &accounts[16],
        &accounts[22],
        &accounts[23],
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
        &accounts[17],
        call_ordinal,
        ceiling,
        accounts[20].key,
        ceiling,
        &accounts[21],
        &accounts[22],
        &accounts[23],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work_execution,
        &accounts[18],
        &accounts[19],
        &accounts[20],
        &accounts[21],
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
    require(
        runtime_key(accounts[22].key) == schedule.payer(),
        ClutchError::MismatchedState,
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
    let result = persist_evaluation_result(
        program_id,
        route,
        &key,
        evidence,
        evaluation,
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
    let semantic_receipt_id = plan
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let kind = SourceWorkKindV1::EvaluateStatistic;
    let ceiling = schedule.ceiling_for(kind);
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        semantic_receipt_id,
        &accounts[18],
        call_ordinal,
        ceiling,
        accounts[21].key,
        ceiling,
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
