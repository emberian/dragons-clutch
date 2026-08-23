//! SourceSeries 77/v2 SBF entry seam.
//!
//! This module first enforces the frozen payload and account-role contract.
//! Individual runtime actions remain centrally capability-gated; adding this
//! decoder alone never makes a tuple executable.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::source_plane_v3::{authenticate_generation_request, authenticate_route, runtime_key};
use crate::source_plane_v3_actions::{
    apply_source_work_liveness, authenticate_source_work_schedule_artifact, bind_work_execution,
    initialize_head, register_release_from_artifact,
};
use clutch_solana_layout::registry::SourceSeriesAction;
use clutch_solana_layout::source_series::{
    decode_payload_v2, validate_account_metas_v2, ObservedSourceAccountMetaV2,
    SourceSeriesPayloadV2,
};
use clutch_source_plane_v3::ContentId;
use clutch_source_plane_v3_adapter::{project_runtime_initialize_source_head, PdaRecipeV3};
use clutch_source_plane_v3_runtime::{
    initialize_source_head as derive_initial_head, SourceWorkKindV1,
};
use solana_account_info::AccountInfo;
use solana_clock::Clock;
use solana_get_sysvar::GetSysvar;
use solana_pubkey::Pubkey;
use std::vec::Vec;

/// Decode one exact SourceSeries action and enter its bounded implementation.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: SourceSeriesAction,
    payload: &[u8],
) -> Outcome<()> {
    let mut observed = Vec::with_capacity(accounts.len());
    for account in accounts {
        observed.push(ObservedSourceAccountMetaV2 {
            key: account.key.to_bytes(),
            writable: account.is_writable,
            signer: account.is_signer,
        });
    }
    validate_account_metas_v2(action, &observed)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let decoded = decode_payload_v2(action, payload)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    match (action, decoded) {
        (SourceSeriesAction::RegisterRelease, SourceSeriesPayloadV2::RegisterRelease(intent)) => {
            require(sequence == 0, ClutchError::Replay)?;
            register_release_from_artifact(
                program_id,
                ContentId::from_bytes(intent.source_release_manifest_id),
                &accounts[0],
                &accounts[2],
                &accounts[1],
                &accounts[3],
                &accounts[4],
            )?;
            Ok(())
        }
        (SourceSeriesAction::InitializeHead, SourceSeriesPayloadV2::Transition(intent)) => {
            process_initialize_head(program_id, accounts, sequence, intent)
        }
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Execute action 2 against the exact release-selected route and schedule.
/// The generation request is derived under the release-selected external
/// authority; Clutch only authenticates and consumes it.
#[allow(clippy::too_many_arguments)]
fn process_initialize_head(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    intent: clutch_source_plane_v3_adapter::IntentPreimageV3,
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
        runtime_key(accounts[15].key) == schedule.payer(),
        ClutchError::MismatchedState,
    )?;
    let authorization =
        authenticate_generation_request(route, &accounts[8]).map_err(Refusal::from)?;
    let head = derive_initial_head(route, authorization)
        .map_err(|_| Refusal::Adapter(ClutchError::SourceAdmissionFailed))?;
    let clock = Clock::get().map_err(|_| Refusal::Adapter(ClutchError::WrongClockSysvar))?;
    require(
        clock.slot < intent.valid_before_slot()
            && intent.adapter_program_id() == ContentId::from_bytes(program_id.to_bytes())
            && intent.submitter() == ContentId::from_bytes(accounts[14].key.to_bytes()),
        ClutchError::MismatchedState,
    )?;

    // The state and lineage writes remain rollback-safe if any subsequent
    // intent, receipt, or liveness join refuses in this same instruction.
    let opened = initialize_head(
        program_id,
        route,
        authorization,
        &accounts[15],
        &accounts[9],
        &accounts[10],
        &accounts[16],
        &accounts[17],
    )?;
    let recipe = PdaRecipeV3::source_head(
        route.source_plane_contract_id(),
        route.source_spec_id(),
        head.repair_generation,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::WrongPda))?;
    let ledger = opened.funding.ledger;
    let plan = project_runtime_initialize_source_head(
        &route.source_plane(),
        &head,
        recipe
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::WrongPda))?,
        opened.account_data_id,
        opened.header.generation,
        authorization.id(),
        ContentId::from_bytes(ledger.principal_recipient.bytes()),
        ledger.payer_principal_lamports,
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
    let work = bind_work_execution(
        program_id,
        route,
        schedule,
        kind,
        semantic_receipt_id,
        &accounts[11],
        call_ordinal,
        ceiling,
        accounts[14].key,
        ceiling,
        &accounts[15],
        &accounts[16],
        &accounts[17],
    )?;
    apply_source_work_liveness(
        program_id,
        route,
        work,
        &accounts[12],
        &accounts[13],
        &accounts[14],
        &accounts[15],
    )?;
    Ok(())
}
