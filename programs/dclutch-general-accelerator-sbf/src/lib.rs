#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Readonly stateless admitted-AOT accelerator for General clearing.
//!
//! The program receives the canonical admitted frame from Trading, rebuilds
//! the complete authenticated input bank from Trading-owned scratch pages,
//! evaluates one General successor transition, and returns exactly one typed
//! candidate chunk. It never writes an account, invokes a child, or owns any
//! protocol state; common Trading remains the sole effect and commit authority.

extern crate alloc;
extern crate std;

use alloc::{vec, vec::Vec};

use dclutch_capability_program_contract::hot_v3::HotExecutionEnvelopeV3;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    admitted_v3::{
        ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3, ADMITTED_INSTRUCTIONS_ACCOUNT_V3,
        ADMITTED_RUNTIME_ACCOUNTS_START_V3, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3,
    },
    v2::{
        ACCELERATOR_ACK_HEADER_BYTES_V2, AcceleratorAckV2, AcceleratorRequestV2,
        AuthenticatedScratchPageV2, RequestTransportV2,
    },
};
use dclutch_general_adapter_contract::{
    account_rules_v3::general_account_profile_fixed_count_v3,
    admitted_accelerator_v3::authenticate_frozen_selection_v3,
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_candidate_bank_len_v3,
        general_hot_environment_from_bank_v3, general_hot_scalar_count_v3,
        project_general_hot_candidate_in_place_v3,
        project_general_initialize_candidate_in_place_v3,
        project_general_selection_candidate_in_place_v3,
    },
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    runtime_selection::{
        RUNTIME_SELECTION_CURSOR_BYTES_V2, consider_verified_candidate_v2, freeze_selection_v2,
    },
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementBuffersV2, RuntimeSettlementViewV2,
        evaluate_runtime_settlement_v2, initialize_runtime_settlement_v2,
        runtime_settlement_effect_len_v2,
    },
    runtime_width::{VerifiedCandidateV2, settlement_cursor_len},
    state_artifacts_v3::{
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GeneralReadonlyEvidenceKindV3,
        general_readonly_evidence_count_v3, general_readonly_evidence_v3,
    },
};
use dclutch_general_codec::{
    Action, SelectionPolicyV1,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_general_config_contract::v3::GeneralConfigV3;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::sysvar;

/// Stable physical refusal from the General accelerator boundary.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAcceleratorSbfErrorV3 {
    /// Accelerator request transport or register geometry differed.
    InvalidRequest = 0,
    /// The fixed admitted frame or readonly runtime frame differed.
    InvalidFrame = 1,
    /// The current top-level Trading instruction could not be authenticated.
    InvalidTopLevelInstruction = 2,
    /// A Trading-owned scratch page or whole-bank digest differed.
    InvalidScratchBank = 3,
    /// The exact acknowledgement could not be encoded.
    InvalidAcknowledgement = 4,
}

impl From<GeneralAcceleratorSbfErrorV3> for ProgramError {
    fn from(value: GeneralAcceleratorSbfErrorV3) -> Self {
        Self::Custom(value as u32)
    }
}

/// Semantic refusal after the complete physical frame has authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAcceleratorSemanticErrorV3 {
    /// Action-selected state or evidence was absent, extraneous, or malformed.
    State,
    /// The pure General transition refused its authenticated inputs.
    Transition,
    /// Candidate-bank projection refused.
    Candidate,
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Evaluate one admitted chunk and publish a canonical V2 acknowledgement.
///
/// Physical authentication errors return a program error with no return data.
/// A well-formed request whose General semantics refuse returns the canonical
/// refused acknowledgement, allowing Trading to distinguish transport failure
/// from a failure-atomic semantic refusal.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = AcceleratorRequestV2::decode(instruction_data)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?;
    validate_request_geometry(request)?;
    let (family_request, controller) = authenticate_top_level(accounts)?;
    let fixed_count = usize::from(
        general_account_profile_fixed_count_v3(controller.action)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidFrame)?,
    );
    validate_frame(accounts, request, fixed_count)?;
    let mut candidate = assemble_input_bank(accounts, request, fixed_count)?;
    let request_digest = content(instruction_data)?;
    let evaluation = evaluate_candidate(
        controller,
        &family_request,
        request.tail_count(),
        runtime_accounts(accounts, fixed_count)?,
        &mut candidate,
    );
    let bank_digest = content(&candidate)?;
    let ack = match evaluation {
        Ok(()) => {
            let start = usize::try_from(request.chunk_offset())
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
            let remaining = candidate
                .len()
                .checked_sub(start)
                .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
            let payload_len = remaining
                .min(dclutch_execution_strategy_contract::v2::ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2);
            let payload = candidate
                .get(
                    start
                        ..start
                            .checked_add(payload_len)
                            .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?,
                )
                .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
            AcceleratorAckV2::accepted(request, request_digest, bank_digest, payload)
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?
        }
        Err(_) => AcceleratorAckV2::refused(request, request_digest),
    };
    let ack_len = ACCELERATOR_ACK_HEADER_BYTES_V2
        .checked_add(ack.payload().len())
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
    let mut output = vec![0_u8; ack_len];
    ack.encode_into(&mut output)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement)?;
    set_return_data(&output);
    Ok(())
}

fn validate_request_geometry(request: AcceleratorRequestV2<'_>) -> ProgramResult {
    let tail_count = request.tail_count();
    if request.transport() != RequestTransportV2::ScratchPages
        || tail_count == 0
        || request.scalar_count()
            != general_hot_scalar_count_v3(tail_count)
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?
        || request.identity_count() != GENERAL_HOT_COMMON_IDENTITIES_V3
        || usize::try_from(request.total_bank_bytes())
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?
            != general_hot_candidate_bank_len_v3(tail_count)
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidRequest)?
        || !request.inline_bank().is_empty()
    {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into());
    }
    Ok(())
}

fn validate_frame(
    accounts: &[AccountInfo<'_>],
    request: AcceleratorRequestV2<'_>,
    fixed_count: usize,
) -> ProgramResult {
    let pages = usize::try_from(request.chunk_count())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidFrame)?;
    let expected = ADMITTED_RUNTIME_ACCOUNTS_START_V3
        .checked_add(fixed_count)
        .and_then(|value| value.checked_add(pages))
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidFrame)?;
    if accounts.len() != expected {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidFrame.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.is_writable
            || account.is_signer != (index == ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3)
        {
            return Err(GeneralAcceleratorSbfErrorV3::InvalidFrame.into());
        }
    }
    let trading = account(accounts, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)?;
    if !trading.executable {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidFrame.into());
    }
    Ok(())
}

fn authenticate_top_level(
    accounts: &[AccountInfo<'_>],
) -> Result<([u8; CONTROLLER_REQUEST_BYTES_V2], ControllerRequestV2), ProgramError> {
    let instructions = account(accounts, ADMITTED_INSTRUCTIONS_ACCOUNT_V3)?;
    if instructions.key != &solana_instructions_sysvar::ID
        || instructions.owner != &sysvar::ID
        || instructions.is_writable
        || instructions.is_signer
    {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction.into());
    }
    let index = load_current_index_checked(instructions)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction)?;
    let instruction = load_instruction_at_checked(usize::from(index), instructions)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction)?;
    let trading = account(accounts, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)?;
    if instruction.program_id != *trading.key {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction.into());
    }
    let (_, family) = HotExecutionEnvelopeV3::split_instruction(&instruction.data)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction)?;
    if family.len() != CONTROLLER_REQUEST_BYTES_V2 {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction.into());
    }
    let request = ControllerRequestV2::decode(family)
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction)?;
    let family_copy = family
        .try_into()
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidTopLevelInstruction)?;
    Ok((family_copy, request))
}

fn assemble_input_bank(
    accounts: &[AccountInfo<'_>],
    request: AcceleratorRequestV2<'_>,
    fixed_count: usize,
) -> Result<Vec<u8>, ProgramError> {
    let page_count = usize::try_from(request.chunk_count())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    let page_start = ADMITTED_RUNTIME_ACCOUNTS_START_V3
        .checked_add(fixed_count)
        .ok_or(GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    let trading = account(accounts, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)?;
    let trading_id = ContentId::new(trading.key.to_bytes())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    let bank_len = usize::try_from(request.total_bank_bytes())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
    let mut output = vec![0_u8; bank_len];
    let mut cursor = 0_usize;
    for page_index in 0..page_count {
        let page_account = account(
            accounts,
            page_start
                .checked_add(page_index)
                .ok_or(GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?,
        )?;
        if page_account.owner != trading.key
            || page_account.is_signer
            || page_account.is_writable
            || page_account.executable
        {
            return Err(GeneralAcceleratorSbfErrorV3::InvalidScratchBank.into());
        }
        let data = page_account
            .try_borrow_data()
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
        let page = AuthenticatedScratchPageV2::decode(&data)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
        page.validate_request_input(trading_id, request)
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
        if usize::try_from(page.chunk_index())
            .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?
            != page_index
            || usize::try_from(page.chunk_offset())
                .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?
                != cursor
        {
            return Err(GeneralAcceleratorSbfErrorV3::InvalidScratchBank.into());
        }
        let end = cursor
            .checked_add(page.payload().len())
            .ok_or(GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?;
        output
            .get_mut(cursor..end)
            .ok_or(GeneralAcceleratorSbfErrorV3::InvalidScratchBank)?
            .copy_from_slice(page.payload());
        cursor = end;
    }
    if cursor != output.len() || content(&output)? != request.input_bank_digest() {
        return Err(GeneralAcceleratorSbfErrorV3::InvalidScratchBank.into());
    }
    Ok(output)
}

fn runtime_accounts<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    fixed_count: usize,
) -> Result<&'a [AccountInfo<'info>], ProgramError> {
    accounts
        .get(
            ADMITTED_RUNTIME_ACCOUNTS_START_V3
                ..ADMITTED_RUNTIME_ACCOUNTS_START_V3
                    .checked_add(fixed_count)
                    .ok_or(GeneralAcceleratorSbfErrorV3::InvalidFrame)?,
        )
        .ok_or_else(|| GeneralAcceleratorSbfErrorV3::InvalidFrame.into())
}

fn evaluate_candidate(
    request: ControllerRequestV2,
    family_request: &[u8],
    outcome_count: u32,
    runtime: &[AccountInfo<'_>],
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    if family_request
        != request
            .to_bytes()
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?
    {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let environment = general_hot_environment_from_bank_v3(candidate, outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    match request.action {
        Action::Consider | Action::Freeze => {
            evaluate_selection(request, runtime, outcome_count, candidate)
        }
        Action::InitializeSettlement => {
            evaluate_initialize(request, runtime, outcome_count, environment, candidate)
        }
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            evaluate_settlement(request, runtime, outcome_count, environment, candidate)
        }
    }
}

fn evaluate_selection(
    request: ControllerRequestV2,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let before = if primary.is_empty() {
        &vacant[..]
    } else {
        let state = GeneralLocalStateV3::decode(&primary)
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
        if state.header().kind != GeneralLocalStateKindV3::Selection {
            return Err(GeneralAcceleratorSemanticErrorV3::State);
        }
        state.body()
    };
    let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut output = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    match request.action {
        Action::Consider => {
            let policy_coordinate = evidence_coordinate(
                request.action,
                GeneralReadonlyEvidenceKindV3::SelectionPolicy,
            )?;
            let verified_coordinate = evidence_coordinate(
                request.action,
                GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
            )?;
            let policy_data = data(runtime, policy_coordinate)?;
            let verified_data = data(runtime, verified_coordinate)?;
            let policy = SelectionPolicyV1::decode(&policy_data)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
            let config_data = data(runtime, 1)?;
            let config = GeneralConfigV3::decode(&config_data)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
            let verified = VerifiedCandidateV2::decode(&verified_data)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
            if policy.policy_id != config.selection_policy_id()
                || request.candidate_id != Some(verified.header().candidate_id)
                || request.page_index != verified.header().candidate_coordinate
                || verified.header().outcome_count != outcome_count
            {
                return Err(GeneralAcceleratorSemanticErrorV3::State);
            }
            consider_verified_candidate_v2(
                policy,
                before,
                &verified_data,
                request.expected_revision,
                &mut scratch,
                &mut output,
            )
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?;
        }
        Action::Freeze => {
            freeze_selection_v2(before, request.expected_revision, &mut scratch, &mut output)
                .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?
        }
        _ => return Err(GeneralAcceleratorSemanticErrorV3::State),
    }
    project_general_selection_candidate_in_place_v3(
        request.action,
        &output,
        outcome_count,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_initialize(
    request: ControllerRequestV2,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    if !data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?.is_empty() {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let frozen = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::FrozenSelection,
        )?,
    )?;
    let verifier = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
        )?,
    )?;
    let verified = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        )?,
    )?;
    let config = GeneralConfigV3::decode(&data(runtime, 1)?)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    authenticate_frozen_selection_v3(
        config.selection_policy_id(),
        request.candidate_id,
        outcome_count,
        &frozen,
        &verified,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let cursor_bytes = settlement_cursor_len(outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let inventory_bytes = usize::try_from(outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?
        .checked_mul(8)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    let mut inventory = vec![0_u8; inventory_bytes];
    let mut cursor_scratch = vec![0_u8; cursor_bytes];
    let mut cursor_output = vec![0_u8; cursor_bytes];
    initialize_runtime_settlement_v2(
        &verifier,
        &verified,
        request.expected_revision,
        &mut inventory,
        &mut cursor_scratch,
        &mut cursor_output,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?;
    project_general_initialize_candidate_in_place_v3(
        &cursor_output,
        outcome_count,
        environment,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evaluate_settlement(
    request: ControllerRequestV2,
    runtime: &[AccountInfo<'_>],
    outcome_count: u32,
    environment: dclutch_general_adapter_contract::hot_candidate_v3::GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<(), GeneralAcceleratorSemanticErrorV3> {
    let primary = data(runtime, GENERAL_PRIMARY_STATE_ACCOUNT_V3)?;
    let state = GeneralLocalStateV3::decode(&primary)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if state.header().kind != GeneralLocalStateKindV3::Settlement {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let verified = data(
        runtime,
        evidence_coordinate(
            request.action,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        )?,
    )?;
    let verified_value = VerifiedCandidateV2::decode(&verified)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    if request.candidate_id != Some(verified_value.header().candidate_id)
        || verified_value.header().outcome_count != outcome_count
    {
        return Err(GeneralAcceleratorSemanticErrorV3::State);
    }
    let action = match request.action {
        Action::Collect => RuntimeSettlementActionV2::Collect,
        Action::Materialize => RuntimeSettlementActionV2::Materialize,
        Action::Distribute => RuntimeSettlementActionV2::Distribute,
        Action::Close => RuntimeSettlementActionV2::Close,
        _ => return Err(GeneralAcceleratorSemanticErrorV3::State),
    };
    let manifest_data = match action {
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute => Some(data(
            runtime,
            evidence_coordinate(
                request.action,
                GeneralReadonlyEvidenceKindV3::SettlementManifest,
            )?,
        )?),
        RuntimeSettlementActionV2::Materialize | RuntimeSettlementActionV2::Close => None,
    };
    let config = GeneralConfigV3::decode(&data(runtime, 1)?)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let cursor_bytes = state.body().len();
    let effect_bytes = runtime_settlement_effect_len_v2(outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    let inventory_bytes = usize::try_from(outcome_count)
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?
        .checked_mul(8)
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    let mut cursor_scratch = vec![0_u8; cursor_bytes];
    let mut cursor_output = vec![0_u8; cursor_bytes];
    let mut inventory_scratch = vec![0_u8; inventory_bytes];
    let mut effect_scratch = vec![0_u8; effect_bytes];
    let mut effect_output = vec![0_u8; effect_bytes];
    evaluate_runtime_settlement_v2(
        RuntimeSettlementViewV2 {
            action,
            cursor_before: state.body(),
            verified: &verified,
            manifest: manifest_data.as_deref(),
            manifest_order_index: u32::from(request.execution_index),
            expected_revision: request.expected_revision,
            surplus_beneficiary: if action == RuntimeSettlementActionV2::Close {
                Some(config.quote_surplus_beneficiary())
            } else {
                None
            },
        },
        RuntimeSettlementBuffersV2 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            inventory_scratch: &mut inventory_scratch,
            effect_scratch: &mut effect_scratch,
            effect_output: &mut effect_output,
        },
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Transition)?;
    project_general_hot_candidate_in_place_v3(
        &effect_output,
        &cursor_output,
        outcome_count,
        environment,
        candidate,
    )
    .map_err(|_| GeneralAcceleratorSemanticErrorV3::Candidate)
}

fn evidence_coordinate(
    action: Action,
    kind: GeneralReadonlyEvidenceKindV3,
) -> Result<u16, GeneralAcceleratorSemanticErrorV3> {
    let mut index = 0_u16;
    while index < general_readonly_evidence_count_v3(action) {
        let evidence = general_readonly_evidence_v3(action, index)
            .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
        if evidence.kind == kind {
            return Ok(evidence.coordinate);
        }
        index = index
            .checked_add(1)
            .ok_or(GeneralAcceleratorSemanticErrorV3::State)?;
    }
    Err(GeneralAcceleratorSemanticErrorV3::State)
}

fn data<'a>(
    runtime: &'a [AccountInfo<'_>],
    coordinate: u16,
) -> Result<core::cell::Ref<'a, [u8]>, GeneralAcceleratorSemanticErrorV3> {
    let borrowed = runtime
        .get(usize::from(coordinate))
        .ok_or(GeneralAcceleratorSemanticErrorV3::State)?
        .try_borrow_data()
        .map_err(|_| GeneralAcceleratorSemanticErrorV3::State)?;
    Ok(core::cell::Ref::map(borrowed, |value| &**value))
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| GeneralAcceleratorSbfErrorV3::InvalidFrame.into())
}

fn content(bytes: &[u8]) -> Result<ContentId, ProgramError> {
    ContentId::new(hash(bytes).to_bytes())
        .map_err(|_| GeneralAcceleratorSbfErrorV3::InvalidAcknowledgement.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero test identity")
    }

    fn request(outcome_count: u32, transport: RequestTransportV2) -> AcceleratorRequestV2<'static> {
        AcceleratorRequestV2::new(
            transport,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            outcome_count,
            general_hot_scalar_count_v3(outcome_count).expect("scalar count"),
            GENERAL_HOT_COMMON_IDENTITIES_V3,
            0,
            match transport {
                RequestTransportV2::Inline => {
                    let bytes =
                        general_hot_candidate_bank_len_v3(outcome_count).expect("bank bytes");
                    alloc::boxed::Box::leak(vec![0; bytes].into_boxed_slice())
                }
                RequestTransportV2::ScratchPages => &[],
            },
        )
        .expect("request")
    }

    #[test]
    fn scratch_geometry_accepts_product_widths_one_and_258() {
        for outcome_count in [1_u32, 258] {
            let request = request(outcome_count, RequestTransportV2::ScratchPages);
            validate_request_geometry(request).expect("runtime-width scratch transport");
            assert_eq!(
                usize::try_from(request.total_bank_bytes()).expect("bank bytes"),
                general_hot_candidate_bank_len_v3(outcome_count).expect("General bank")
            );
            assert!(request.chunk_count() > 1);
        }
    }

    #[test]
    fn inline_and_zero_width_requests_refuse() {
        assert_eq!(
            validate_request_geometry(request(1, RequestTransportV2::Inline)),
            Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into())
        );
        let zero = AcceleratorRequestV2::new(
            RequestTransportV2::ScratchPages,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            0,
            87,
            GENERAL_HOT_COMMON_IDENTITIES_V3,
            0,
            &[],
        )
        .expect("transport permits syntactic zero");
        assert_eq!(
            validate_request_geometry(zero),
            Err(GeneralAcceleratorSbfErrorV3::InvalidRequest.into())
        );
    }
}
