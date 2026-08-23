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
    authenticate_head, authenticate_lineage, authenticate_open_page, authenticate_route,
    runtime_key,
};
use crate::source_plane_v3_actions::{
    apply_source_work_liveness, authenticate_source_work_schedule_artifact, bind_work_execution,
    seal_raw_page,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_adapter::{
    project_runtime_seal_raw_page, IntentPreimageV3, RuntimeCloseProjectionV1,
    RuntimeCreationProjectionV1, RuntimeMutationProjectionV1,
};
use clutch_source_plane_v3_runtime::{LineageAccessV1, SourceWorkKindV1};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

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
