//! Current Direct `80/1` account authentication and writeback plane.
//!
//! Current actions accept only the fresh b1/v2 root. The unchanged b2/b3/b4
//! physical frames are interpreted only after that root has authenticated, so
//! their historical arithmetic shape cannot become a persisted V1 authority.
//! Action 1 and action 13 refuse before account inspection until their sole
//! Product FundingV4 and Product RootV2/0xba-v2 writers are available.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    require_system_program, transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_direct_market_runtime::codec_v1::{
    DIRECT_ACTION_REPLAY_BODY_BYTES_V1, DIRECT_RESERVATION_BODY_BYTES_V1,
    DIRECT_SELECTION_BODY_BYTES_V1,
};
use clutch_direct_market_runtime::codec_v2::{
    authenticate_direct_root_transition_body_v2,
    decode_direct_action_replay_body_for_transition_v2,
    decode_direct_selection_body_for_transition_v2,
    encode_direct_action_replay_body_into_transition_v2,
    encode_direct_selection_body_into_transition_v2,
    write_direct_root_transition_body_v2, AuthenticatedDirectRootTransitionV2,
    DIRECT_MARKET_ROOT_BODY_BYTES_V2 as RUNTIME_ROOT_BODY_BYTES_V2,
};
use clutch_direct_market_runtime::lifecycle_v2::{
    begin_direct_candidate_verification_v2, bind_direct_candidate_work_batch_v2,
    prepare_direct_candidate_work_batch_v2, submit_direct_candidate_v2,
    verify_next_direct_candidate_v2, DirectRootReplayTransitionV2,
};
use clutch_direct_market_runtime::liveness_v1::DirectCandidateWorkBatchV1;
use clutch_direct_market_runtime::selection_v1::DirectSelectionV1;
use clutch_direct_market_runtime::{
    DirectActionReplayV1, DirectHashBackendV1, DirectMarketActionV1,
    DirectMarketErrorV1,
};
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimePersistedAccountViewV1, RuntimeReceiptKindV1,
    RuntimeReceiptObservationV1, RuntimeTransferRoleV1, RuntimeTransitionActionV1,
    RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentV1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_retirement::{PositionV3Sha256Backend, ReplayV3HashBackend};
use clutch_solana_layout::direct_market_v1::DirectSubmitCandidatePayloadV1;
use clutch_solana_layout::direct_market_v2::{
    DirectMarketRootAccountV2, DIRECT_MARKET_ROOT_BODY_BYTES_V2,
};
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    DIRECT_ACTION_REPLAY_ACCOUNT_TAG, DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
    DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2, DIRECT_RESERVATION_ACCOUNT_BYTES,
    DIRECT_SELECTION_ACCOUNT_BYTES, DIRECT_SELECTION_ACCOUNT_TAG,
    DIRECT_SELECTION_ACCOUNT_VERSION,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const DIRECT_MARKET_V2_MAX_ACCOUNTS: usize = 30;
const DIRECT_MARKET_V2_MAX_PAYLOAD_BYTES: usize = 80;
const DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2: usize = 4;

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V2 == RUNTIME_ROOT_BODY_BYTES_V2);
const _: () = assert!(DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2 == 2_502);
const _: () = assert!(DIRECT_SELECTION_ACCOUNT_BYTES == 1_629);
const _: () = assert!(DIRECT_ACTION_REPLAY_ACCOUNT_BYTES == 394);
const _: () = assert!(DIRECT_RESERVATION_ACCOUNT_BYTES == 473);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectMarketRootV2>() <= 2_560);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectActionReplayV2>() <= 512);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectSelectionV2>() <= 192);

/// Allocation-free SHA-256 boundary for current Direct account adapters.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DirectRuntimeSha256V2;

impl DirectHashBackendV1 for DirectRuntimeSha256V2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for DirectRuntimeSha256V2 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for DirectRuntimeSha256V2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, parts)
    }
}

/// Current family dispatcher. Unsupported actions refuse before reading any
/// account, so no historical b1/v1 width can select a fallback route.
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        DirectMarketAction::SubmitCandidate => {
            require(
                accounts.len() <= DIRECT_MARKET_V2_MAX_ACCOUNTS,
                ClutchError::AccountCount,
            )?;
            require(
                payload.len() <= DIRECT_MARKET_V2_MAX_PAYLOAD_BYTES,
                ClutchError::WrongDataLength,
            )?;
            process_direct_submit_candidate_v2(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => {
            process_direct_candidate_verification_v2(
                program_id,
                accounts,
                sequence,
                action,
                payload,
            )
        }
        DirectMarketAction::InitializeMarket
        | DirectMarketAction::AdmitOrder
        | DirectMarketAction::CancelOrder
        | DirectMarketAction::FreezeBook
        | DirectMarketAction::FinalizeSelection
        | DirectMarketAction::SettlePair
        | DirectMarketAction::LapseEmpty
        | DirectMarketAction::LapseUnselected
        | DirectMarketAction::LapseSelected
        | DirectMarketAction::RetireTerminal => {
            Err(Refusal::Adapter(ClutchError::UnsupportedInstruction))
        }
    }
}

/// Execute actions 6 and 7 with the exact four-account Candidate suffix.
#[inline(never)]
fn process_direct_candidate_verification_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, 8)?;
    require(payload.is_empty(), ClutchError::WrongDataLength)?;
    require_distinct(&accounts[..4])?;
    require_direct_candidate_liveness_aliases_v2(accounts, 4, 4)?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v2(
        program_id,
        &accounts[2],
        &root,
    )?;
    let observed_slot = read_clock_slot(&accounts[3])?;
    let root_bump = root.bump();
    let replay_bump = replay.bump();
    let selection_bump = selection.bump();
    let selection_balance = selection.observed_lamports();
    let mut selection_value = selection.into_value();
    let accounted_selection_balance = {
        let rent = selection_value.rent();
        let bond = root
            .transition()
            .outstanding_candidate_bond_lamports(*selection_value)
            .map_err(map_direct_error_v2)?;
        rent.principal_lamports
            .checked_add(rent.donation_floor_lamports)
            .and_then(|value| value.checked_add(bond))
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
    };
    require(
        selection_balance >= accounted_selection_balance,
        ClutchError::MismatchedState,
    )?;
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let effects = match action {
        DirectMarketAction::BeginVerification => begin_direct_candidate_verification_v2(
            &mut state,
            &mut selection_value,
            sequence,
            observed_slot,
            &DirectRuntimeSha256V2,
        ),
        DirectMarketAction::VerifyCandidate => verify_next_direct_candidate_v2(
            &mut state,
            &mut selection_value,
            sequence,
            observed_slot,
            &DirectRuntimeSha256V2,
        ),
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    }
    .map_err(map_direct_error_v2)?;
    require(
        effects.candidate_bond_movement.is_none()
            && effects.candidate_bond_refunds.is_none()
            && accounts[2].lamports() == selection_balance,
        ClutchError::MismatchedState,
    )?;
    let runtime_action = match action {
        DirectMarketAction::BeginVerification => DirectMarketActionV1::BeginVerification,
        DirectMarketAction::VerifyCandidate => DirectMarketActionV1::VerifyCandidate,
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };
    apply_direct_candidate_work_v2(
        program_id,
        &accounts[4..],
        &accounts[1],
        &mut state,
        &selection_value,
        runtime_action,
    )?;
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_direct_selection_v2(
        &accounts[2],
        selection_bump,
        *selection_value,
        state.root(),
    )
}

/// Execute action 5 against exact current root/replay/Selection state.
///
/// Accounts are b1/v2 root W, b3 replay W, b2 Selection W, Clock RO,
/// submitter signer W, System program, and an optional exact evicted bond
/// refund owner W. No Product or fee authority is supplied by the caller.
#[inline(never)]
fn process_direct_submit_candidate_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        accounts.len() == 6 || accounts.len() == 7,
        ClutchError::AccountCount,
    )?;
    require_distinct(&accounts[..4])?;
    require_signer(&accounts[4])?;
    require(accounts[4].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[5])?;
    let mut fixed = 0usize;
    while fixed < 4 {
        require(
            accounts[4].key != accounts[fixed].key
                && accounts[5].key != accounts[fixed].key,
            ClutchError::AccountAlias,
        )?;
        fixed += 1;
    }

    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v2(
        program_id,
        &accounts[2],
        &root,
    )?;
    let observed_slot = read_clock_slot(&accounts[3])?;
    let candidate = DirectSubmitCandidatePayloadV1::decode(payload)?.candidate;
    let root_bump = root.bump();
    let replay_bump = replay.bump();
    let selection_bump = selection.bump();
    let selection_balance_before = selection.observed_lamports();
    let mut selection_value = selection.into_value();
    let bond_principal_before = root
        .transition()
        .outstanding_candidate_bond_lamports(*selection_value)
        .map_err(map_direct_error_v2)?;
    let selection_rent = selection_value.rent();
    let accounted_balance_before = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_before))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        selection_balance_before >= accounted_balance_before,
        ClutchError::MismatchedState,
    )?;
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let effects = submit_direct_candidate_v2(
        &mut state,
        &mut selection_value,
        sequence,
        observed_slot,
        candidate,
        accounts[4].key.to_bytes(),
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)?;

    let expected_selection_balance = match effects.candidate_bond_movement {
        Some(movement) => {
            let expected_count = if movement.evicted_refund_lamports == 0 { 6 } else { 7 };
            require_count(accounts, expected_count)?;
            require(
                movement.incoming_payer == accounts[4].key.to_bytes()
                    && movement.principal_before_lamports == bond_principal_before
                    && movement.principal_after_lamports
                        == state
                            .root()
                            .outstanding_candidate_bond_lamports(*selection_value)
                            .map_err(map_direct_error_v2)?,
                ClutchError::MismatchedState,
            )?;
            if movement.evicted_refund_lamports != 0 {
                require(
                    accounts[6].is_writable
                        && !accounts[6].executable
                        && accounts[6].key.to_bytes() == movement.evicted_refund_recipient,
                    ClutchError::MismatchedState,
                )?;
                let mut index = 0usize;
                while index < 6 {
                    if index != 4 {
                        require(
                            accounts[6].key != accounts[index].key,
                            ClutchError::AccountAlias,
                        )?;
                    }
                    index += 1;
                }
            }
            transfer_signer_lamports_v2(
                &accounts[4],
                &accounts[2],
                &accounts[5],
                movement.incoming_lamports,
            )?;
            if movement.evicted_refund_lamports != 0 {
                debit_lamports_v2(&accounts[2], movement.evicted_refund_lamports)?;
                credit_lamports_v2(&accounts[6], movement.evicted_refund_lamports)?;
            }
            selection_balance_before
                .checked_add(movement.incoming_lamports)
                .and_then(|value| value.checked_sub(movement.evicted_refund_lamports))
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
        }
        None => {
            require_count(accounts, 6)?;
            selection_balance_before
        }
    };
    require(
        accounts[2].lamports() == expected_selection_balance,
        ClutchError::MismatchedState,
    )?;
    let bond_principal_after = state
        .root()
        .outstanding_candidate_bond_lamports(*selection_value)
        .map_err(map_direct_error_v2)?;
    let accounted_balance_after = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_after))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        accounts[2].lamports() >= accounted_balance_after,
        ClutchError::MismatchedState,
    )?;

    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_direct_selection_v2(
        &accounts[2],
        selection_bump,
        *selection_value,
        state.root(),
    )
}

/// Stream one exact current Direct work batch through the shared Candidate.
/// No caller ordinal, role, work amount, or receipt identity is accepted.
#[inline(never)]
fn apply_direct_candidate_work_v2(
    program_id: &Pubkey,
    liveness_accounts: &[AccountInfo<'_>],
    receipt_account: &AccountInfo<'_>,
    state: &mut DirectRootReplayTransitionV2,
    selection: &DirectSelectionV1,
    action: DirectMarketActionV1,
) -> Outcome<()> {
    require_count(
        liveness_accounts,
        DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2,
    )?;
    let policy_account = &liveness_accounts[0];
    let candidate_account = &liveness_accounts[1];
    let keeper = &liveness_accounts[2];
    let payer = &liveness_accounts[3];
    let root = state.root();
    let candidate_binding = root.candidate_liveness();
    require(
        policy_account.key.to_bytes() == candidate_binding.policy_account
            && policy_account.owner == program_id
            && !policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && policy_account.data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1
            && candidate_account.key.to_bytes() == candidate_binding.candidate_account
            && candidate_account.owner == program_id
            && candidate_account.is_writable
            && !candidate_account.is_signer
            && !candidate_account.executable
            && candidate_account.data_len() == RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
            && keeper.is_writable
            && keeper.is_signer
            && !keeper.executable
            && payer.is_writable
            && (payer.key == keeper.key || !payer.is_signer)
            && !payer.executable
            && receipt_account.key.to_bytes() == root.action_replay_account()
            && receipt_account.owner == program_id,
        ClutchError::MismatchedState,
    )?;
    require(
        policy_account.key != candidate_account.key
            && policy_account.key != keeper.key
            && policy_account.key != payer.key
            && policy_account.key != receipt_account.key
            && candidate_account.key != keeper.key
            && candidate_account.key != payer.key
            && candidate_account.key != receipt_account.key
            && keeper.key != receipt_account.key
            && payer.key != receipt_account.key,
        ClutchError::AccountAlias,
    )?;

    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_data_id = solana_sha256_hasher::hashv(&[&policy_data[..]]).to_bytes();
    require(
        policy_data_id == candidate_binding.policy_data_id,
        ClutchError::MismatchedState,
    )?;
    let mut candidate_data = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    {
        let data = candidate_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        candidate_data.copy_from_slice(&data);
    }
    let candidate_pre_data_id =
        solana_sha256_hasher::hashv(&[&candidate_data[..]]).to_bytes();
    let (candidate_completed_calls, candidate_last_receipt_id) = {
        let candidate_state = RuntimeCompartmentV1::decode(&candidate_data)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            candidate_state.kind == RuntimeCompartmentKindV1::Candidate
                && candidate_state.identity.policy_id.bytes()
                    == root.candidate_liveness_policy_id()
                && candidate_state.identity.lifecycle_id.bytes()
                    == candidate_binding.global_lifecycle_id
                && candidate_state.identity.account_id.bytes()
                    == candidate_binding.candidate_account
                && candidate_state.identity.owner.bytes()
                    == candidate_binding.candidate_semantic_owner
                && candidate_state.identity.payer.bytes() == payer.key.to_bytes()
                && candidate_state.identity.neutral_sink.bytes()
                    == root.neutral_lamport_sink()
                && candidate_state.identity.generation == candidate_binding.candidate_generation
                && candidate_state.quote_schedule_id.bytes()
                    == candidate_binding.candidate_quote_schedule_id
                && candidate_state.receipt_program_id.bytes()
                    == candidate_binding.candidate_receipt_program_id
                && candidate_state.receipt_program_id.bytes() == program_id.to_bytes()
                && (state.replay().candidate_liveness_completed_calls() != 0
                    || candidate_pre_data_id == candidate_binding.candidate_data_id),
            ClutchError::MismatchedState,
        )?;
        (
            candidate_state.completed_calls,
            candidate_state.last_work_receipt_id.bytes(),
        )
    };
    let batch = prepare_direct_candidate_work_batch_v2(
        state,
        Some(selection),
        action,
        candidate_completed_calls,
        candidate_last_receipt_id,
        candidate_pre_data_id,
        keeper.key.to_bytes(),
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)?;
    apply_direct_candidate_batch_v2(
        program_id,
        policy_account,
        candidate_account,
        keeper,
        payer,
        receipt_account,
        &policy_data,
        &mut candidate_data,
        candidate_binding,
        root.candidate_liveness_policy_id(),
        batch,
    )?;
    bind_direct_candidate_work_batch_v2(state, batch, &DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)
}

/// Apply the already-derived bounded work batch and all exact lamport flows.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn apply_direct_candidate_batch_v2(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    candidate_account: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    receipt_account: &AccountInfo<'_>,
    policy_data: &[u8],
    candidate_data: &mut [u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1],
    candidate_binding: clutch_direct_market_runtime::liveness_v1::DirectCandidateLivenessBindingV1,
    candidate_liveness_policy_id: [u8; 32],
    batch: DirectCandidateWorkBatchV1,
) -> Outcome<()> {
    let expected_program = LivenessId::from_bytes(program_id.to_bytes());
    let expected_policy_account = LivenessId::from_bytes(policy_account.key.to_bytes());
    let mut account_balance = candidate_account.lamports();
    let mut keeper_total = 0u64;
    let mut payer_total = 0u64;
    let receipt_count = batch.receipt_count();
    let mut index = 0u8;
    while index < receipt_count {
        let receipt = batch
            .receipt(index, candidate_binding, &DirectRuntimeSha256V2)
            .map_err(map_direct_error_v2)?;
        let account_balance_after = account_balance
            .checked_sub(receipt.call_ceiling_lamports())
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        let intent = RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Candidate,
            policy_id: LivenessId::from_bytes(candidate_liveness_policy_id),
            lifecycle_id: LivenessId::from_bytes(candidate_binding.global_lifecycle_id),
            account_id: LivenessId::from_bytes(candidate_binding.candidate_account),
            semantic_owner: LivenessId::from_bytes(candidate_binding.candidate_semantic_owner),
            quote_schedule_id: LivenessId::from_bytes(
                candidate_binding.candidate_quote_schedule_id,
            ),
            receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
            keeper: LivenessId::from_bytes(keeper.key.to_bytes()),
            generation: candidate_binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
            keeper_payment_lamports: receipt.keeper_payment_lamports(),
            flags: 0,
        };
        let observation = RuntimeReceiptObservationV1 {
            receipt_account_id: LivenessId::from_bytes(receipt_account.key.to_bytes()),
            receipt_account_owner_program_id: expected_program,
            receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
            receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
            compartment_kind: RuntimeCompartmentKindV1::Candidate,
            semantic_owner: LivenessId::from_bytes(candidate_binding.candidate_semantic_owner),
            lifecycle_id: LivenessId::from_bytes(candidate_binding.global_lifecycle_id),
            quote_schedule_id: LivenessId::from_bytes(
                candidate_binding.candidate_quote_schedule_id,
            ),
            generation: candidate_binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
        };
        let transition = plan_runtime_transition_v1(
            expected_program,
            expected_policy_account,
            RuntimePersistedAccountViewV1 {
                account_id: expected_policy_account,
                owner_program_id: LivenessId::from_bytes(policy_account.owner.to_bytes()),
                lamports: policy_account.lamports(),
                data: policy_data,
                writable: policy_account.is_writable,
            },
            RuntimePersistedAccountViewV1 {
                account_id: LivenessId::from_bytes(candidate_account.key.to_bytes()),
                owner_program_id: LivenessId::from_bytes(candidate_account.owner.to_bytes()),
                lamports: account_balance,
                data: candidate_data,
                writable: candidate_account.is_writable,
            },
            intent,
            Some(observation),
            account_balance_after,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            transition.write_account_data
                && !transition.close_account
                && transition.account_balance_before == account_balance
                && transition.account_balance_after == account_balance_after,
            ClutchError::MismatchedState,
        )?;
        for movement in transition.transfers() {
            match movement.role {
                RuntimeTransferRoleV1::KeeperPayment => {
                    require(
                        movement.destination == LivenessId::from_bytes(keeper.key.to_bytes()),
                        ClutchError::MismatchedState,
                    )?;
                    keeper_total = keeper_total
                        .checked_add(movement.lamports)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
                }
                RuntimeTransferRoleV1::PayerWorkRefund => {
                    require(
                        movement.destination == LivenessId::from_bytes(payer.key.to_bytes()),
                        ClutchError::MismatchedState,
                    )?;
                    payer_total = payer_total
                        .checked_add(movement.lamports)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
                }
                _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
            }
        }
        candidate_data.copy_from_slice(&transition.post_account_data);
        account_balance = account_balance_after;
        index = index
            .checked_add(1)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    require(
        keeper_total == batch.total_keeper_payment_lamports()
            && payer_total == batch.total_payer_refund_lamports()
            && candidate_account
                .lamports()
                .checked_sub(account_balance)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                == batch.total_call_ceiling_lamports(),
        ClutchError::MismatchedState,
    )?;
    let coalesced_recipient = keeper.key == payer.key;
    let keeper_after = keeper
        .lamports()
        .checked_add(keeper_total)
        .and_then(|balance| {
            if coalesced_recipient {
                balance.checked_add(payer_total)
            } else {
                Some(balance)
            }
        })
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_after = if coalesced_recipient {
        keeper_after
    } else {
        payer
            .lamports()
            .checked_add(payer_total)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
    };
    {
        let mut data = candidate_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data.copy_from_slice(candidate_data);
    }
    {
        let mut candidate_lamports = candidate_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **candidate_lamports = account_balance;
    }
    if coalesced_recipient {
        let mut recipient_lamports = keeper
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **recipient_lamports = keeper_after;
    } else {
        let mut keeper_lamports = keeper
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut payer_lamports = payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **keeper_lamports = keeper_after;
        **payer_lamports = payer_after;
    }
    Ok(())
}

fn require_direct_candidate_liveness_aliases_v2(
    accounts: &[AccountInfo<'_>],
    liveness_start: usize,
    recipient_start: usize,
) -> Outcome<()> {
    require(
        recipient_start <= liveness_start
            && accounts.len()
                == liveness_start
                    .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::AccountCount,
    )?;
    let policy = &accounts[liveness_start];
    let candidate = &accounts[liveness_start + 1];
    let keeper = &accounts[liveness_start + 2];
    let payer = &accounts[liveness_start + 3];
    let mut index = 0usize;
    while index < liveness_start {
        require(
            policy.key != accounts[index].key
                && candidate.key != accounts[index].key
                && (keeper.key != accounts[index].key || index >= recipient_start)
                && (payer.key != accounts[index].key || index >= recipient_start),
            ClutchError::AccountAlias,
        )?;
        index += 1;
    }
    Ok(())
}

#[derive(Debug)]
struct AuthenticatedDirectMarketRootV2 {
    account: Pubkey,
    transition: AuthenticatedDirectRootTransitionV2,
    bump: u8,
    data_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectMarketRootV2 {
    const fn account(&self) -> Pubkey { self.account }
    const fn bump(&self) -> u8 { self.bump }
    const fn data_id(&self) -> [u8; 32] { self.data_id }
    const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    const fn transition(&self) -> &AuthenticatedDirectRootTransitionV2 { &self.transition }
    fn into_transition(self) -> AuthenticatedDirectRootTransitionV2 { self.transition }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDirectActionReplayV2 {
    value: DirectActionReplayV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectActionReplayV2 {
    const fn value(self) -> DirectActionReplayV1 { self.value }
    const fn bump(self) -> u8 { self.bump }
}

#[derive(Debug)]
struct AuthenticatedDirectSelectionV2 {
    value: Box<DirectSelectionV1>,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectSelectionV2 {
    const fn bump(&self) -> u8 { self.bump }
    const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    fn into_value(self) -> Box<DirectSelectionV1> { self.value }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectAccountAccessV2 {
    ReadOnly,
    Writable,
}

impl DirectAccountAccessV2 {
    const fn writable(self) -> bool { matches!(self, Self::Writable) }
}

#[inline(never)]
fn authenticate_direct_market_root_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedDirectMarketRootV2> {
    require_program_state_v2(
        program_id,
        account,
        DirectAccountAccessV2::Writable,
        DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = DirectMarketRootAccountV2::decode(&data)?;
    let transition = authenticate_direct_root_transition_body_v2(
        frame.semantic_body(),
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)?;
    let (expected, bump) = seeds::direct_market_root_v2_pda(
        program_id,
        &transition.market_instance_id(),
        transition.generation(),
    );
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        transition.direct_root_account() == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    let rent = transition.root_rent();
    require_rent_coverage_v2(
        rent.principal_lamports,
        rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    require_live_id_v2(data_id)?;
    drop(data);
    Ok(AuthenticatedDirectMarketRootV2 {
        account: *account.key,
        transition,
        bump,
        data_id,
        observed_lamports,
    })
}

#[inline(never)]
fn authenticate_direct_action_replay_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV2,
) -> Outcome<AuthenticatedDirectActionReplayV2> {
    require_program_state_v2(
        program_id,
        account,
        DirectAccountAccessV2::Writable,
        DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (bump, body) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
        DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
        DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
    )?;
    let value = decode_direct_action_replay_body_for_transition_v2(body, root.transition())
        .map_err(map_direct_error_v2)?;
    let (expected, expected_bump) =
        seeds::direct_action_replay_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, expected_bump), Some(bump))?;
    require(
        account.key.to_bytes() == root.transition().action_replay_account(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    let rent = value.rent();
    require_rent_coverage_v2(
        rent.principal_lamports,
        rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    let semantic_id = root
        .transition()
        .action_replay_semantic_id(value, &DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)?;
    require_live_id_v2(data_id)?;
    drop(data);
    Ok(AuthenticatedDirectActionReplayV2 {
        value,
        bump,
        data_id,
        semantic_id,
        observed_lamports,
    })
}

#[inline(never)]
fn authenticate_direct_selection_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV2,
) -> Outcome<AuthenticatedDirectSelectionV2> {
    require_program_state_v2(
        program_id,
        account,
        DirectAccountAccessV2::Writable,
        DIRECT_SELECTION_ACCOUNT_BYTES,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (bump, body) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_SELECTION_ACCOUNT_TAG,
        DIRECT_SELECTION_ACCOUNT_VERSION,
        DIRECT_SELECTION_BODY_BYTES_V1,
    )?;
    let value = Box::new(
        decode_direct_selection_body_for_transition_v2(body, root.transition())
            .map_err(map_direct_error_v2)?,
    );
    let (expected, expected_bump) =
        seeds::direct_selection_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, expected_bump), Some(bump))?;
    require(
        value.account() == account.key.to_bytes()
            && root.transition().selection_account() == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    let rent = value.rent();
    require_rent_coverage_v2(
        rent.principal_lamports,
        rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    let semantic_id = root
        .transition()
        .selection_semantic_id(*value, &DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)?;
    require_live_id_v2(data_id)?;
    drop(data);
    Ok(AuthenticatedDirectSelectionV2 {
        value,
        bump,
        data_id,
        semantic_id,
        observed_lamports,
    })
}

fn decode_borrowed_child_frame_v2<'a>(
    input: &'a [u8],
    expected_tag: u8,
    expected_version: u8,
    expected_body_len: usize,
) -> Outcome<(u8, &'a [u8])> {
    let expected_len = expected_body_len
        .checked_add(4)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(input.len() == expected_len, ClutchError::WrongDataLength)?;
    require(
        input[0] == expected_tag
            && input[1] == expected_version
            && input[3] == 0
            && input[4..].iter().any(|byte| *byte != 0),
        ClutchError::MismatchedState,
    )?;
    Ok((input[2], &input[4..]))
}

fn write_direct_market_root_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = DirectMarketRootAccountV2::decode(&data)?;
    require(frame.bump() == bump, ClutchError::MismatchedState)?;
    write_direct_root_transition_body_v2(
        transition,
        &mut data[4..],
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)
}

fn write_direct_action_replay_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectActionReplayV1,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (observed_bump, _) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
        DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
        DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
    )?;
    require(observed_bump == bump, ClutchError::MismatchedState)?;
    encode_direct_action_replay_body_into_transition_v2(value, transition, &mut data[4..])
        .map_err(map_direct_error_v2)
}

fn write_direct_selection_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectSelectionV1,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (observed_bump, _) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_SELECTION_ACCOUNT_TAG,
        DIRECT_SELECTION_ACCOUNT_VERSION,
        DIRECT_SELECTION_BODY_BYTES_V1,
    )?;
    require(observed_bump == bump, ClutchError::MismatchedState)?;
    encode_direct_selection_body_into_transition_v2(value, transition, &mut data[4..])
        .map_err(map_direct_error_v2)
}

fn require_program_state_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    access: DirectAccountAccessV2,
    expected_len: usize,
) -> Outcome<()> {
    require(
        !account.is_signer
            && !account.executable
            && account.owner == program_id
            && account.is_writable == access.writable()
            && account.data_len() == expected_len,
        ClutchError::MismatchedState,
    )
}

fn require_rent_coverage_v2(principal: u64, donation_floor: u64, observed: u64) -> Outcome<()> {
    let floor = principal
        .checked_add(donation_floor)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(principal != 0 && observed >= floor, ClutchError::MismatchedState)
}

fn require_live_id_v2(id: [u8; 32]) -> Outcome<()> {
    require(id != [0; 32], ClutchError::MismatchedState)
}

fn transfer_signer_lamports_v2<'a>(
    payer: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    amount: u64,
) -> Outcome<()> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(destination.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(amount),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), destination.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))
}

fn credit_lamports_v2(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_add(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn debit_lamports_v2(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_sub(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn map_direct_error_v2(error: DirectMarketErrorV1) -> Refusal {
    let adapter = match error {
        DirectMarketErrorV1::Arithmetic => ClutchError::Arithmetic,
        DirectMarketErrorV1::Replay => ClutchError::Replay,
        DirectMarketErrorV1::WrongPhase => ClutchError::NotActive,
        DirectMarketErrorV1::UnauthenticatedAuthority => ClutchError::AuthorizationUnavailable,
        _ => ClutchError::MismatchedState,
    };
    Refusal::Adapter(adapter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_action_group_has_exact_frame_bounds() {
        assert_eq!(DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2, 2_502);
        assert_eq!(DIRECT_SELECTION_ACCOUNT_BYTES, 1_629);
        assert_eq!(DIRECT_ACTION_REPLAY_ACCOUNT_BYTES, 394);
        assert_eq!(DIRECT_RESERVATION_ACCOUNT_BYTES, 473);
        assert!(7 <= DIRECT_MARKET_V2_MAX_ACCOUNTS);
        assert_eq!(4 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2, 8);
    }

    #[test]
    fn child_frame_refuses_v1_alias_padding_and_wrong_width() {
        let mut replay = [1u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES];
        replay[0] = DIRECT_ACTION_REPLAY_ACCOUNT_TAG;
        replay[1] = DIRECT_ACTION_REPLAY_ACCOUNT_VERSION;
        replay[2] = 7;
        replay[3] = 0;
        assert!(decode_borrowed_child_frame_v2(
            &replay,
            DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
            DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
            DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
        )
        .is_ok());
        replay[3] = 1;
        assert!(decode_borrowed_child_frame_v2(
            &replay,
            DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
            DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
            DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
        )
        .is_err());
    }
}
