//! Disabled current Direct `80/1` account authentication and writeback plane.
//!
//! The dispatcher recognizes this module only behind the exact central
//! capability check, and every `80/1/1..=13` capability remains false. It owns
//! the hostile Solana boundary for the fresh `0xb1..=0xb4/v1` family while
//! economic state and transition identities stay exclusively in
//! `clutch-direct-market-runtime`.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, MAX_PERMITTED_DATA_INCREASE,
};
use clutch_direct_market_runtime::codec_v1::{
    decode_direct_action_replay_body_v1, decode_direct_market_root_body_v1,
    decode_direct_reservation_body_v1, decode_direct_selection_body_v1,
    encode_direct_action_replay_body_v1, encode_direct_market_root_body_v1,
    encode_direct_reservation_body_v1, encode_direct_selection_body_v1,
    DIRECT_ACTION_REPLAY_BODY_BYTES_V1 as RUNTIME_REPLAY_BODY_BYTES,
    DIRECT_MARKET_ROOT_BODY_BYTES_V1 as RUNTIME_ROOT_BODY_BYTES,
    DIRECT_RESERVATION_BODY_BYTES_V1 as RUNTIME_RESERVATION_BODY_BYTES,
    DIRECT_SELECTION_BODY_BYTES_V1 as RUNTIME_SELECTION_BODY_BYTES,
};
use clutch_direct_market_runtime::reservation_v1::DirectReservationV1;
use clutch_direct_market_runtime::reservation_v1::AuthenticatedDirectReservationAdmissionV1;
use clutch_direct_market_runtime::settlement_v1::AuthenticatedDirectReservationCancelV1;
use clutch_direct_market_runtime::settlement_v1::AuthenticatedDirectEconomicTerminalV1;
use clutch_direct_market_runtime::selection_v1::DirectSelectionV1;
use clutch_direct_market_runtime::selection_v1::{
    begin_direct_candidate_verification_v1, canonical_direct_price_precondition_v1,
    finalize_direct_selection_v1,
    prepare_direct_selection_freeze_v1, submit_direct_candidate_v1,
    verify_next_direct_candidate_v1, AuthenticatedDirectSelectionFreezeV1,
    DirectSelectionPhaseV1,
};
use clutch_direct_market_runtime::{
    build_direct_retirement_transfer_v1, direct_epoch_semantics_id_v1,
    direct_schedule_policy_id_v1,
    prepare_direct_family_terminal_v1,
    prepare_direct_foundation_v1, AuthenticatedDirectFoundationV1,
    AuthenticatedDirectTerminalV1, DirectActionReplayV1, DirectHashBackendV1,
    DirectFinalResolutionV1, DirectMarketBindingV1, DirectMarketErrorV1, DirectMarketRootV1,
    DirectRentOwnerV1,
    DirectRetirementSourceV1, DirectRetirementTransferV1, DirectRootPhaseV1,
    DirectRootReplayPostV1, DirectScheduleV1, DirectTerminalReasonV1,
};
use clutch_direct_market_runtime::fee_v1::{DirectFeePolicyV1, DirectFeeTerminalV1};
use clutch_direct_market_runtime::liveness_v1::{
    bind_direct_candidate_work_batch_v1, prepare_direct_candidate_work_batch_v1,
    AuthenticatedDirectCandidateLivenessV1, DirectCandidateLivenessBindingV1,
    DirectCandidateWorkScheduleV1, DIRECT_CANDIDATE_RESERVED_CALLS_V1,
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
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy, BATCH_POLICY_BYTES,
};
use clutch_batch_policy_identity::revenue_policy_v1::{
    decode_revenue_policy, revenue_policy_digest, RevenuePolicyV1, REVENUE_POLICY_BYTES,
};
use clutch_direct_market_runtime::settlement_v1::{
    prepare_direct_reservation_admission_with_replay_v1, prepare_direct_reservation_cancel_v1,
    prepare_direct_economic_terminal_v1, prepare_direct_missed_freeze_terminal_v1,
    DirectEndpointPrestateV1, DirectFeeTreasuryPrestateV1,
    DirectReservationOrderInputV1,
};
use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
};
use clutch_general_v2_contract::GeneralReplayTransitionPlanV1;
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{PositionV3Sha256Backend, ReplayV3HashBackend};
use clutch_batch::relation_v2::{
    EconomicDomainV2, PricePreconditionV2, ECONOMIC_RELATION_VERSION_V2,
};
use clutch_batch::direct_pair_v1::DirectEconomicBookV1;
use clutch_batch::relation_v2::EMPTY_ECONOMIC_ORDER_V2;
use clutch_price_measure::PriceVectorV3;
use clutch_product_series::{
    CompiledProductSeriesBundleV5, ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2,
    MarketFamilyV1, MarketInstanceV2Id, NativeClaimBasisV1, PriceMeasurePolicyV1,
    SeriesPlanV5Id,
};
use clutch_solana_layout::direct_market_v1::{
    DirectActionReplayAccountV1, DirectMarketRootAccountV1, DirectReservationAccountV1,
    DirectSelectionAccountV1, DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
    DIRECT_MARKET_ROOT_BODY_BYTES_V1, DIRECT_RESERVATION_BODY_BYTES_V1,
    DIRECT_SELECTION_BODY_BYTES_V1,
    decode_direct_empty_payload_v1,
    DirectAdmitOrderPayloadV1,
    DirectSubmitCandidatePayloadV1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use clutch_solana_layout::registry::DirectMarketAction;
use clutch_solana_layout::revenue::{RevenuePolicyRecordV1, REVENUE_POLICY_RECORD_BYTES};
use clutch_solana_layout::{account_len, PriceGridAccount};
use clutch_solana_layout::registry::{
    DIRECT_ACTION_REPLAY_ACCOUNT_BYTES, DIRECT_MARKET_ROOT_ACCOUNT_BYTES,
    DIRECT_RESERVATION_ACCOUNT_BYTES, DIRECT_SELECTION_ACCOUNT_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::product_artifact::authenticate_product_artifact_v1;
use super::artifact::read_clock_slot;
use super::collateral_position_v3::authenticate_general_market_v3;
use super::general_v2_position_replay::authenticate_current_general_position_replay_v3;
use super::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    AuthenticatedMarketLifecycleRootV1,
};

const DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/account-authentication/v1\0";
const DIRECT_PRICE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/price-authentication/v1\0";
const DIRECT_MARKET_V1_MAX_ACCOUNTS: usize = 30;
const DIRECT_MARKET_V1_MAX_PAYLOAD_BYTES: usize = 80;
const DIRECT_RETIRE_TERMINAL_MAX_ACCOUNTS: usize = 16;
const DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1: usize = 4;

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V1 == RUNTIME_ROOT_BODY_BYTES);
const _: () = assert!(DIRECT_SELECTION_BODY_BYTES_V1 == RUNTIME_SELECTION_BODY_BYTES);
const _: () = assert!(DIRECT_ACTION_REPLAY_BODY_BYTES_V1 == RUNTIME_REPLAY_BODY_BYTES);
const _: () = assert!(DIRECT_RESERVATION_BODY_BYTES_V1 == RUNTIME_RESERVATION_BODY_BYTES);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectMarketRootV1>() <= 160);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectActionReplayV1>() <= 160);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectSelectionV1>() <= 160);
// Hostile Product/Direct semantic owners are retained in their boxed decode
// buffers or authenticated prestates. Private adapter capabilities borrow
// them, so no capability can silently cross the 4 KiB SBF frame boundary.
const _: () = assert!(core::mem::size_of::<DirectFoundationAuthoritySbfV1<'static>>() <= 512);
const _: () = assert!(core::mem::size_of::<DirectReservationAdmissionAuthoritySbfV1<'static>>() <= 512);
const _: () = assert!(core::mem::size_of::<DirectReservationCancelAuthoritySbfV1<'static>>() <= 512);
const _: () = assert!(core::mem::size_of::<DirectSelectionFreezeAuthoritySbfV1<'static>>() <= 512);
const _: () = assert!(core::mem::size_of::<DirectEconomicTerminalAuthoritySbfV1<'static>>() <= 512);
const _: () = assert!(core::mem::size_of::<DirectMissedFreezeTerminalAuthoritySbfV1<'static>>() <= 512);
const _: () = assert!(core::mem::size_of::<DirectFamilyTerminalAuthoritySbfV1<'static>>() <= 512);

/// Runtime SHA-256 implementation for all current Direct semantic identities.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DirectRuntimeSha256V1;

impl DirectHashBackendV1 for DirectRuntimeSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for DirectRuntimeSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for DirectRuntimeSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Stream one exact Direct receipt batch through the shared Candidate owner.
///
/// The four accounts are immutable policy, writable Candidate, writable
/// keeper signer, and writable immutable payer/refund owner. Child receipts
/// are recomputed from b3 one at a time; neither caller counts nor caller work
/// amounts enter the transition. This helper mutates only after every child,
/// aggregate transfer, and final b3 binding has been checked.
#[inline(never)]
fn apply_direct_candidate_work_v1(
    program_id: &Pubkey,
    liveness_accounts: &[AccountInfo<'_>],
    receipt_account: &AccountInfo<'_>,
    prepared: &DirectRootReplayPostV1,
    selection: &DirectSelectionV1,
    action: clutch_direct_market_runtime::DirectMarketActionV1,
) -> Outcome<DirectActionReplayV1> {
    require_count(
        liveness_accounts,
        DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1,
    )?;
    let policy_account = &liveness_accounts[0];
    let candidate_account = &liveness_accounts[1];
    let keeper = &liveness_accounts[2];
    let payer = &liveness_accounts[3];
    let binding = prepared.root.binding();
    let candidate_binding = binding.candidate_liveness;
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
            && receipt_account.key.to_bytes() == binding.action_replay_account
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
                    == binding.candidate_liveness_policy_id
                && candidate_state.identity.lifecycle_id.bytes()
                    == candidate_binding.global_lifecycle_id
                && candidate_state.identity.account_id.bytes()
                    == candidate_binding.candidate_account
                && candidate_state.identity.owner.bytes()
                    == candidate_binding.candidate_semantic_owner
                && candidate_state.identity.payer.bytes() == payer.key.to_bytes()
                && candidate_state.identity.neutral_sink.bytes() == binding.neutral_lamport_sink
                && candidate_state.identity.generation == candidate_binding.candidate_generation
                && candidate_state.quote_schedule_id.bytes()
                    == candidate_binding.candidate_quote_schedule_id
                && candidate_state.receipt_program_id.bytes()
                    == candidate_binding.candidate_receipt_program_id
                && candidate_state.receipt_program_id.bytes() == program_id.to_bytes()
                && (prepared.replay.candidate_liveness_completed_calls() != 0
                    || candidate_pre_data_id == candidate_binding.candidate_data_id),
            ClutchError::MismatchedState,
        )?;
        (
            candidate_state.completed_calls,
            candidate_state.last_work_receipt_id.bytes(),
        )
    };
    let batch = prepare_direct_candidate_work_batch_v1(
        *prepared,
        Some(selection),
        action,
        candidate_completed_calls,
        candidate_last_receipt_id,
        candidate_pre_data_id,
        keeper.key.to_bytes(),
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;

    let expected_program = LivenessId::from_bytes(program_id.to_bytes());
    let expected_policy_account = LivenessId::from_bytes(policy_account.key.to_bytes());
    let mut account_balance = candidate_account.lamports();
    let mut keeper_total = 0u64;
    let mut payer_total = 0u64;
    let receipt_count = batch.receipt_count();
    let mut index = 0u8;
    while index < receipt_count {
        let receipt = batch
            .receipt(index, candidate_binding, &DirectRuntimeSha256V1)
            .map_err(map_direct_error_v1)?;
        let account_balance_after = account_balance
            .checked_sub(receipt.call_ceiling_lamports())
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        let intent = RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Candidate,
            policy_id: LivenessId::from_bytes(binding.candidate_liveness_policy_id),
            lifecycle_id: LivenessId::from_bytes(candidate_binding.global_lifecycle_id),
            account_id: LivenessId::from_bytes(candidate_binding.candidate_account),
            semantic_owner: LivenessId::from_bytes(
                candidate_binding.candidate_semantic_owner,
            ),
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
            semantic_owner: LivenessId::from_bytes(
                candidate_binding.candidate_semantic_owner,
            ),
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
                data: &policy_data,
                writable: policy_account.is_writable,
            },
            RuntimePersistedAccountViewV1 {
                account_id: LivenessId::from_bytes(candidate_account.key.to_bytes()),
                owner_program_id: LivenessId::from_bytes(candidate_account.owner.to_bytes()),
                lamports: account_balance,
                data: &candidate_data,
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
    let bound_replay = bind_direct_candidate_work_batch_v1(
        prepared,
        batch,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
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
    drop(policy_data);
    {
        let mut data = candidate_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data.copy_from_slice(&candidate_data);
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
    Ok(bound_replay)
}

/// Disabled family-internal dispatcher for the complete current `80/1`
/// lifecycle. Shared program dispatch does not call this function until the
/// capability profile admits the whole family as one release unit.
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    require(
        accounts.len() <= DIRECT_MARKET_V1_MAX_ACCOUNTS,
        ClutchError::AccountCount,
    )?;
    require(
        payload.len() <= DIRECT_MARKET_V1_MAX_PAYLOAD_BYTES,
        ClutchError::WrongDataLength,
    )?;
    match action {
        DirectMarketAction::InitializeMarket => {
            process_direct_initialize_market_v1(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::AdmitOrder => {
            process_direct_admit_order_v1(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::CancelOrder => {
            process_direct_cancel_order_v1(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::FreezeBook => {
            process_direct_freeze_book_v1(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::SubmitCandidate
        | DirectMarketAction::BeginVerification
        | DirectMarketAction::VerifyCandidate
        | DirectMarketAction::FinalizeSelection => process_direct_selection_lifecycle_v1(
            program_id,
            accounts,
            sequence,
            action,
            payload,
        ),
        DirectMarketAction::SettlePair
        | DirectMarketAction::LapseEmpty
        | DirectMarketAction::LapseUnselected
        | DirectMarketAction::LapseSelected => process_direct_economic_terminal_v1(
            program_id,
            accounts,
            sequence,
            action,
            payload,
        ),
        DirectMarketAction::RetireTerminal => {
            process_direct_retire_terminal_v1(program_id, accounts, sequence, payload)
        }
    }
}

/// Exact authenticated `0xb1/1` Direct root prestate.
#[derive(Debug)]
pub(crate) struct AuthenticatedDirectMarketRootV1 {
    account: Pubkey,
    value: Box<DirectMarketRootV1>,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectMarketRootV1 {
    pub(crate) fn account(&self) -> Pubkey { self.account }
    pub(crate) fn value(&self) -> DirectMarketRootV1 { *self.value }
    pub(crate) fn bump(&self) -> u8 { self.bump }
    pub(crate) fn data_id(&self) -> [u8; 32] { self.data_id }
    pub(crate) fn semantic_id(&self) -> [u8; 32] { self.semantic_id }
    pub(crate) fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) fn value_ref(&self) -> &DirectMarketRootV1 { &self.value }
}

/// Exact authenticated permanent `0xb3/1` Direct replay/receipt prestate.
#[derive(Debug)]
pub(crate) struct AuthenticatedDirectActionReplayV1 {
    account: Pubkey,
    value: Box<DirectActionReplayV1>,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectActionReplayV1 {
    pub(crate) fn account(&self) -> Pubkey { self.account }
    pub(crate) fn value(&self) -> DirectActionReplayV1 { *self.value }
    pub(crate) fn bump(&self) -> u8 { self.bump }
    pub(crate) fn data_id(&self) -> [u8; 32] { self.data_id }
    pub(crate) fn semantic_id(&self) -> [u8; 32] { self.semantic_id }
    pub(crate) fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) fn value_ref(&self) -> &DirectActionReplayV1 { &self.value }
}

/// Exact authenticated `0xb2/1` Selection prestate.
#[derive(Debug)]
pub(crate) struct AuthenticatedDirectSelectionV1 {
    account: Pubkey,
    value: Box<DirectSelectionV1>,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectSelectionV1 {
    pub(crate) fn account(&self) -> Pubkey { self.account }
    pub(crate) fn value(&self) -> DirectSelectionV1 { *self.value }
    pub(crate) fn bump(&self) -> u8 { self.bump }
    pub(crate) fn data_id(&self) -> [u8; 32] { self.data_id }
    pub(crate) fn semantic_id(&self) -> [u8; 32] { self.semantic_id }
    pub(crate) fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) fn value_ref(&self) -> &DirectSelectionV1 { &self.value }
}

/// Exact authenticated `0xb4/1` funded Reservation prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectReservationV1 {
    account: Pubkey,
    value: DirectReservationV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectReservationV1 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> DirectReservationV1 { self.value }
    pub(crate) const fn bump(self) -> u8 { self.bump }
    pub(crate) const fn data_id(self) -> [u8; 32] { self.data_id }
    pub(crate) const fn semantic_id(self) -> [u8; 32] { self.semantic_id }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
    pub(crate) const fn value_ref(&self) -> &DirectReservationV1 { &self.value }
}

/// Private Product/PriceGrid-authenticated action-4 input. Construction owns
/// the complete immutable graph join; callers receive only the exact Relation
/// domain and price precondition persisted by b2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectPricePreconditionV1 {
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    authentication_id: [u8; 32],
}

impl AuthenticatedDirectPricePreconditionV1 {
    pub(crate) const fn domain(self) -> EconomicDomainV2 { self.domain }
    pub(crate) const fn price(self) -> PricePreconditionV2 { self.price }
    pub(crate) const fn authentication_id(self) -> [u8; 32] { self.authentication_id }
}

/// Exact hostile-authenticated fee owners consumed by Direct foundation and
/// settlement. The batch and revenue preimages remain owned by their existing
/// policy crates; this capability introduces no caller-shaped fee DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDirectFeePolicySbfV1 {
    revenue: RevenuePolicyV1,
    direct: DirectFeePolicyV1,
}

#[inline(never)]
fn authenticate_direct_fee_policy_v1(
    program_id: &Pubkey,
    batch_preimage_account: &AccountInfo<'_>,
    revenue_record_account: &AccountInfo<'_>,
    revenue_preimage_account: &AccountInfo<'_>,
    expected_realm: [u8; 32],
    expected_batch_policy_id: [u8; 32],
    expected_revenue_policy_id: [u8; 32],
) -> Outcome<AuthenticatedDirectFeePolicySbfV1> {
    // The two raw policy preimages are content-addressed facts, not account
    // owners. Their respective persisted General/Realm owners below pin the
    // rederived digests; accepting an alternate account address containing
    // identical canonical bytes cannot change the selected policy.
    require(
        !batch_preimage_account.is_writable
            && !batch_preimage_account.is_signer
            && !batch_preimage_account.executable
            && batch_preimage_account.data_len() == BATCH_POLICY_BYTES,
        ClutchError::MismatchedState,
    )?;
    require_program_state_v1(
        program_id,
        revenue_record_account,
        DirectAccountAccessV1::ReadOnly,
        REVENUE_POLICY_RECORD_BYTES,
    )?;
    require(
        !revenue_preimage_account.is_writable
            && !revenue_preimage_account.is_signer
            && !revenue_preimage_account.executable
            && revenue_preimage_account.data_len() == REVENUE_POLICY_BYTES,
        ClutchError::MismatchedState,
    )?;
    let batch = decode_batch_policy(
        &batch_preimage_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let batch_id = batch_policy_digest(&batch)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue = decode_revenue_policy(
        &revenue_preimage_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue_id = revenue_policy_digest(&revenue)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue_record = RevenuePolicyRecordV1::decode(
        &revenue_record_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    expect_pda(
        revenue_record_account.key,
        seeds::revenue_policy_pda(program_id, &revenue_record.realm.bytes()),
        Some(revenue_record.stored_bump),
    )?;
    require(
        batch_id.0 == expected_batch_policy_id
            && revenue_id.0 == expected_revenue_policy_id
            && revenue_record.realm.bytes() == expected_realm
            && revenue_record.policy_digest.bytes() == revenue_id.0
            && revenue_record.treasury.bytes() == revenue.treasury,
        ClutchError::MismatchedState,
    )?;
    let direct = DirectFeePolicyV1::from_policies(&batch, &revenue)
        .map_err(map_direct_error_v1)?;
    Ok(AuthenticatedDirectFeePolicySbfV1 {
        revenue,
        direct,
    })
}

/// Authenticate the current Product bundle, native basis, price policy,
/// Genesis V2, and immutable venue grid before b2 may own a price vector.
/// Every active component must be an exact grid tick; every inactive component
/// must be zero; Product independently checks width, scale, and simplex sum.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn authenticate_direct_price_precondition_v1(
    program_id: &Pubkey,
    root: &AuthenticatedDirectMarketRootV1,
    bundle_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    price_grid_account: &AccountInfo<'_>,
    reservations: [Option<DirectReservationV1>; 2],
) -> Outcome<AuthenticatedDirectPricePreconditionV1> {
    let binding = root.value().binding;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        bundle_account,
        ContentId::from_bytes(binding.compiler_bundle_v5_id),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        basis_account,
        bundle.value().native_claim_basis_id.content_id(),
    )?;
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_policy_account,
        bundle.value().price_measure_policy_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle.value().market_genesis_profile_id.content_id(),
    )?;
    require(
        bundle.value().price_measure_policy_id.content_id().bytes() == binding.price_policy_id
            && bundle.value().series_plan_id.bytes() == binding.founder_series_plan_id
            && genesis.value().realm_id.bytes() == binding.realm_id
            && genesis.value().profile_id.bytes() == binding.collateral_profile_id
            && genesis.value().relation_policy_id.bytes() == binding.relation_policy_id
            && genesis.value().fee_policy_id.bytes() == binding.revenue_policy_id
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == binding.price_policy_id
            && basis.value().outcome_count == binding.outcome_count,
        ClutchError::MismatchedState,
    )?;

    require(
        price_grid_account.owner == program_id
            && !price_grid_account.is_signer
            && !price_grid_account.executable
            && !price_grid_account.is_writable
            && price_grid_account.data_len() == account_len::PRICE_GRID,
        ClutchError::MismatchedState,
    )?;
    let grid_data = price_grid_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let grid = PriceGridAccount::decode(&grid_data)?;
    expect_pda(
        price_grid_account.key,
        seeds::grid_pda(program_id, &grid.realm.0, &grid.grid.0),
        Some(grid.stored_bump),
    )?;
    require(
        grid.realm.0 == binding.realm_id
            && grid.grid.0 == genesis.value().price_grid_id.bytes()
            && grid.price_scale == binding.price_scale,
        ClutchError::MismatchedState,
    )?;
    let mut index = 0usize;
    let mut encoded_limits = [[0u8; 16]; 2];
    let mut book = DirectEconomicBookV1 {
        orders: [EMPTY_ECONOMIC_ORDER_V2; 2],
        len: 0,
    };
    while index < reservations.len() {
        if let Some(reservation) = reservations[index] {
            reservation
                .validate_against_root(root.value())
                .map_err(map_direct_error_v1)?;
            let limit = reservation.limit_price_units_per_egg();
            let grid_limit = u64::try_from(limit)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            grid.tick_of(grid_limit)?;
            encoded_limits[index] = limit.to_le_bytes();
            book.orders[index] = reservation.economic_order().map_err(map_direct_error_v1)?;
            book.len = book
                .len
                .checked_add(1)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        index += 1;
    }
    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: binding.market_instance_id,
        epoch_semantics_digest: binding.direct_epoch_semantics_id,
        relation_policy_digest: binding.relation_policy_id,
        price_policy_digest: binding.price_policy_id,
        epoch_index: binding.direct_window_index().map_err(map_direct_error_v1)?,
        outcome_count: binding.outcome_count,
        price_scale: binding.price_scale,
    };
    let price = canonical_direct_price_precondition_v1(&domain, &book)
        .map_err(map_direct_error_v1)?;
    let prices = price.prices;
    let active = usize::from(binding.outcome_count);
    index = 0;
    while index < prices.len() {
        if index < active {
            grid.tick_of(prices[index])?;
        } else {
            require(prices[index] == 0, ClutchError::NonCanonical)?;
        }
        index += 1;
    }
    let price_vector = PriceVectorV3 {
        basis_degree: basis.value().basis_degree,
        native_outcome_count: binding.outcome_count,
        price_scale: grid.price_scale,
        prices,
    };
    price_policy
        .value()
        .validate_candidate_price_contract(basis.value(), &price_vector, grid.price_scale)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let grid_data_id = solana_sha256_hasher::hashv(&[&grid_data[..]]).to_bytes();
    drop(grid_data);

    let semantic_price_digest = price.semantic_price_digest;
    let authentication_id = solana_sha256_hasher::hashv(&[
        DIRECT_PRICE_AUTHENTICATION_DOMAIN_V1,
        &root.semantic_id(),
        bundle_account.key.as_ref(),
        basis_account.key.as_ref(),
        price_policy_account.key.as_ref(),
        genesis_account.key.as_ref(),
        price_grid_account.key.as_ref(),
        &grid_data_id,
        &encoded_limits[0],
        &encoded_limits[1],
        &semantic_price_digest,
    ])
    .to_bytes();
    require_live_id_v1(authentication_id)?;
    Ok(AuthenticatedDirectPricePreconditionV1 {
        domain,
        price,
        authentication_id,
    })
}

/// Execute the complete persisted-selection sublifecycle (actions 5..=8).
///
/// The fixed prefix is writable b1 root, writable permanent b3 replay, writable
/// b2 Selection, and read-only Clock. Action 5 appends the writable submitter,
/// System program, and exactly one evicted refund owner only when replacement
/// occurs. Action 8 appends the complete sorted unique refund-owner vector
/// derived from b2. Actions 6..=8 then append immutable liveness policy,
/// writable Candidate, writable keeper signer, and immutable writable payer.
/// No caller index, work amount, work ordinal, or refund membership exists;
/// b2 and b3 fix every suffix coordinate.
pub(crate) fn process_direct_selection_lifecycle_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    if action == DirectMarketAction::FinalizeSelection {
        require(accounts.len() >= 3, ClutchError::AccountCount)?;
        let root_probe = authenticate_direct_market_root_writable_v1(
            program_id,
            &accounts[0],
        )?;
        let selection_probe = authenticate_direct_selection_writable_v1(
            program_id,
            &accounts[2],
            &root_probe,
        )?;
        if selection_probe.value().candidate_count() == 0 {
            return process_direct_economic_terminal_v1(
                program_id,
                accounts,
                sequence,
                action,
                payload,
            );
        }
    }
    require(accounts.len() >= 4, ClutchError::AccountCount)?;
    require_distinct(&accounts[..4])?;
    match action {
        DirectMarketAction::SubmitCandidate => {
            require(
                accounts.len() == 6 || accounts.len() == 7,
                ClutchError::AccountCount,
            )?;
            require_signer(&accounts[4])?;
            require(accounts[4].is_writable, ClutchError::NotWritable)?;
            require_system_program(&accounts[5])?;
            let mut index = 0usize;
            while index < 4 {
                require(
                    accounts[4].key != accounts[index].key
                        && accounts[5].key != accounts[index].key,
                    ClutchError::AccountAlias,
                )?;
                index += 1;
            }
        }
        DirectMarketAction::FinalizeSelection => require(
            accounts.len() >= 8 && accounts.len() <= 11,
            ClutchError::AccountCount,
        )?,
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => {
            require_count(accounts, 8)?
        }
        _ => require_count(accounts, 4)?,
    }
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v1(
        program_id,
        &accounts[2],
        &root,
    )?;
    let observed_slot = read_clock_slot(&accounts[3])?;
    let state = DirectRootReplayPostV1 {
        root: root.value(),
        replay: replay.value(),
    };
    let plan = match action {
        DirectMarketAction::SubmitCandidate => {
            let candidate = DirectSubmitCandidatePayloadV1::decode(payload)?.candidate;
            submit_direct_candidate_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                candidate,
                accounts[4].key.to_bytes(),
                &DirectRuntimeSha256V1,
            )
        }
        DirectMarketAction::BeginVerification => {
            decode_direct_empty_payload_v1(payload)?;
            begin_direct_candidate_verification_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                &DirectRuntimeSha256V1,
            )
        }
        DirectMarketAction::VerifyCandidate => {
            decode_direct_empty_payload_v1(payload)?;
            verify_next_direct_candidate_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                &DirectRuntimeSha256V1,
            )
        }
        DirectMarketAction::FinalizeSelection => {
            decode_direct_empty_payload_v1(payload)?;
            finalize_direct_selection_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                &DirectRuntimeSha256V1,
            )
        }
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    }
    .map_err(map_direct_error_v1)?;

    let bond_principal_before = selection
        .value()
        .outstanding_candidate_bond_lamports(root.value())
        .map_err(map_direct_error_v1)?;
    let selection_rent = selection.value().rent();
    let accounted_balance_before = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_before))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        selection.observed_lamports() >= accounted_balance_before,
        ClutchError::MismatchedState,
    )?;

    let expected_selection_balance = match action {
        DirectMarketAction::SubmitCandidate => {
            let expected_count = if plan
                .candidate_bond_movement
                .map_or(false, |movement| movement.evicted_refund_lamports != 0)
            {
                7
            } else {
                6
            };
            require_count(accounts, expected_count)?;
            match plan.candidate_bond_movement {
                Some(movement) => {
                    require(
                        movement.incoming_payer == accounts[4].key.to_bytes()
                            && movement.principal_before_lamports == bond_principal_before
                            && movement.principal_after_lamports
                                == plan
                                    .selection
                                    .outstanding_candidate_bond_lamports(plan.state.root)
                                    .map_err(map_direct_error_v1)?,
                        ClutchError::MismatchedState,
                    )?;
                    if movement.evicted_refund_lamports != 0 {
                        require(
                            accounts[6].is_writable
                                && !accounts[6].executable
                                && accounts[6].key.to_bytes()
                                    == movement.evicted_refund_recipient,
                            ClutchError::MismatchedState,
                        )?;
                        let mut fixed = 0usize;
                        while fixed < 6 {
                            if fixed != 4 {
                                require(
                                    accounts[6].key != accounts[fixed].key,
                                    ClutchError::AccountAlias,
                                )?;
                            }
                            fixed += 1;
                        }
                    }
                    transfer_signer_lamports_v1(
                        &accounts[4],
                        &accounts[2],
                        &accounts[5],
                        movement.incoming_lamports,
                    )?;
                    if movement.evicted_refund_lamports != 0 {
                        debit_lamports_v1(
                            &accounts[2],
                            movement.evicted_refund_lamports,
                        )?;
                        credit_lamports_v1(
                            &accounts[6],
                            movement.evicted_refund_lamports,
                        )?;
                    }
                    selection
                        .observed_lamports()
                        .checked_add(movement.incoming_lamports)
                        .and_then(|value| {
                            value.checked_sub(movement.evicted_refund_lamports)
                        })
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                }
                None => selection.observed_lamports(),
            }
        }
        DirectMarketAction::FinalizeSelection => {
            let refunds = plan
                .candidate_bond_refunds
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            require(
                refunds.total_lamports == bond_principal_before,
                ClutchError::MismatchedState,
            )?;
            let refund_end = 4usize
                .checked_add(usize::from(refunds.refund_count))
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
            let expected_count = refund_end
                .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
            require_count(accounts, expected_count)?;
            let mut index = 0usize;
            while index < usize::from(refunds.refund_count) {
                let refund = refunds.refunds[index]
                    .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
                let account = &accounts[4 + index];
                require(
                    account.is_writable
                        && !account.executable
                        && account.key.to_bytes() == refund.recipient,
                    ClutchError::MismatchedState,
                )?;
                let mut fixed = 0usize;
                while fixed < 4 {
                    require(account.key != accounts[fixed].key, ClutchError::AccountAlias)?;
                    fixed += 1;
                }
                if index != 0 {
                    require(
                        accounts[3 + index].key.to_bytes() < account.key.to_bytes(),
                        ClutchError::AccountAlias,
                    )?;
                }
                index += 1;
            }
            debit_lamports_v1(&accounts[2], refunds.total_lamports)?;
            index = 0;
            while index < usize::from(refunds.refund_count) {
                let refund = refunds.refunds[index]
                    .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
                credit_lamports_v1(&accounts[4 + index], refund.lamports)?;
                index += 1;
            }
            selection
                .observed_lamports()
                .checked_sub(refunds.total_lamports)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
        }
        _ => selection.observed_lamports(),
    };
    require(
        accounts[2].lamports() == expected_selection_balance,
        ClutchError::MismatchedState,
    )?;
    let bond_principal_after = plan
        .selection
        .outstanding_candidate_bond_lamports(plan.state.root)
        .map_err(map_direct_error_v1)?;
    let accounted_balance_after = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_after))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        accounts[2].lamports() >= accounted_balance_after,
        ClutchError::MismatchedState,
    )?;

    let liveness_start = match action {
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => 4usize,
        DirectMarketAction::FinalizeSelection => {
            let refunds = plan
                .candidate_bond_refunds
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            4usize
                .checked_add(usize::from(refunds.refund_count))
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
        }
        DirectMarketAction::SubmitCandidate => 0usize,
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };
    let bound_replay = match action {
        DirectMarketAction::BeginVerification
        | DirectMarketAction::VerifyCandidate
        | DirectMarketAction::FinalizeSelection => {
            require_direct_candidate_liveness_aliases_v1(
                accounts,
                liveness_start,
                if action == DirectMarketAction::FinalizeSelection {
                    4
                } else {
                    liveness_start
                },
            )?;
            let runtime_action = match action {
                DirectMarketAction::BeginVerification => {
                    clutch_direct_market_runtime::DirectMarketActionV1::BeginVerification
                }
                DirectMarketAction::VerifyCandidate => {
                    clutch_direct_market_runtime::DirectMarketActionV1::VerifyCandidate
                }
                DirectMarketAction::FinalizeSelection => {
                    clutch_direct_market_runtime::DirectMarketActionV1::FinalizeSelection
                }
                _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
            };
            Some(apply_direct_candidate_work_v1(
                program_id,
                &accounts[liveness_start..],
                &accounts[1],
                &plan.state,
                &plan.selection,
                runtime_action,
            )?)
        }
        DirectMarketAction::SubmitCandidate => None,
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };

    // SVM transaction atomicity joins any candidate-bond movement, the shared
    // Candidate work transition, and these three semantic postimages.
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        replay.bump(),
        bound_replay.unwrap_or(plan.state.replay),
        plan.state.root,
    )?;
    write_direct_selection_v1(
        &accounts[2],
        selection.bump(),
        plan.selection,
        plan.state.root,
    )
}

/// Require the canonical liveness suffix to be disjoint from semantic state.
/// Keeper and immutable payer may alias each other and may alias only the
/// already-derived native-lamport recipient suffix beginning at
/// `recipient_start`; they can never alias b1/b2/b3, policy, or Candidate.
fn require_direct_candidate_liveness_aliases_v1(
    accounts: &[AccountInfo<'_>],
    liveness_start: usize,
    recipient_start: usize,
) -> Outcome<()> {
    require(
        recipient_start <= liveness_start
            && accounts.len()
                == liveness_start
                    .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1)
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

/// Creation terminals have one already-authenticated native payer before a
/// semantic endpoint suffix. Only the liveness keeper or immutable liveness
/// payer may coincide with that exact payer role; policy and Candidate remain
/// disjoint from every prior account and no endpoint account may alias.
fn require_direct_candidate_liveness_creation_aliases_v1(
    accounts: &[AccountInfo<'_>],
    liveness_start: usize,
    creation_payer_index: usize,
) -> Outcome<()> {
    require(
        creation_payer_index < liveness_start
            && accounts.len()
                == liveness_start
                    .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1)
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
                && (keeper.key != accounts[index].key || index == creation_payer_index)
                && (payer.key != accounts[index].key || index == creation_payer_index),
            ClutchError::AccountAlias,
        )?;
        index += 1;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct DirectSelectionFreezeAuthoritySbfV1<'a> {
    root: &'a DirectMarketRootV1,
    selection_account: [u8; 32],
    rent: &'a DirectRentOwnerV1,
    reservation_accounts: &'a [[u8; 32]; 2],
    reservation_semantic_ids: &'a [[u8; 32]; 2],
    reservation_count: u8,
    price: &'a AuthenticatedDirectPricePreconditionV1,
}

impl AuthenticatedDirectSelectionFreezeV1 for DirectSelectionFreezeAuthoritySbfV1<'_> {
    fn authenticate_freeze(
        &self,
        root: DirectMarketRootV1,
        selection_account: [u8; 32],
        rent: DirectRentOwnerV1,
        reservations: &[Option<DirectReservationV1>; 2],
        reservation_semantic_ids: &[[u8; 32]; 2],
        domain: &EconomicDomainV2,
        price: &PricePreconditionV2,
    ) -> Result<(), DirectMarketErrorV1> {
        if root != *self.root
            || selection_account != self.selection_account
            || rent != *self.rent
            || *domain != self.price.domain()
            || *price != self.price.price()
            || self.price.authentication_id() == [0; 32]
            || reservation_semantic_ids != self.reservation_semantic_ids
        {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let mut index = 0usize;
        while index < 2 {
            if index < usize::from(self.reservation_count) {
                let reservation = reservations[index]
                    .ok_or(DirectMarketErrorV1::UnauthenticatedAuthority)?;
                if reservation.account() != self.reservation_accounts[index] {
                    return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
                }
            } else if reservations[index].is_some()
                || self.reservation_accounts[index] != [0; 32]
                || self.reservation_semantic_ids[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Execute action 4 with an exhaustive account-derived Reservation prefix.
///
/// Fixed accounts 0..=11 are root, replay, fresh Selection, payer, System,
/// Rent, Clock, BundleV5, NativeClaimBasis, PriceMeasurePolicy, GenesisV2, and
/// PriceGrid. Exactly `root.live_reservations` read-only b4 accounts follow,
/// then immutable liveness policy, writable Candidate, writable keeper signer,
/// and the Candidate's immutable writable payer. No packet count, call amount,
/// work ordinal, or order index is accepted.
pub(crate) fn process_direct_freeze_book_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(accounts.len() >= 12, ClutchError::AccountCount)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    let reservation_count = usize::from(root.value().live_reservations());
    let liveness_start = 12usize
        .checked_add(reservation_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_count = liveness_start
        .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    require_distinct(&accounts[..liveness_start])?;
    let liveness_accounts = &accounts[liveness_start..];
    let mut base_index = 0usize;
    while base_index < liveness_start {
        require(
            liveness_accounts[0].key != accounts[base_index].key
                && liveness_accounts[1].key != accounts[base_index].key
                && (liveness_accounts[2].key != accounts[base_index].key
                    || accounts[base_index].key == accounts[3].key)
                && (liveness_accounts[3].key != accounts[base_index].key
                    || accounts[base_index].key == accounts[3].key),
            ClutchError::AccountAlias,
        )?;
        base_index += 1;
    }
    let replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[4])?;
    let rent_parameters = read_rent(&accounts[5])?;
    let observed_slot = read_clock_slot(&accounts[6])?;
    decode_direct_empty_payload_v1(payload)?;
    let (selection_pda, selection_bump) =
        seeds::direct_selection_v1_pda(program_id, &root.account());
    let (_, donation_floor_lamports) = authenticate_fresh_direct_pda_v1(
        &accounts[2],
        (selection_pda, selection_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let selection_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    selection_rent.validate().map_err(map_direct_error_v1)?;

    let mut authenticated: [Option<AuthenticatedDirectReservationV1>; 2] = [None; 2];
    let mut index = 0usize;
    while index < reservation_count {
        authenticated[index] = Some(authenticate_direct_reservation_readonly_v1(
            program_id,
            &accounts[12 + index],
            &root,
        )?);
        index += 1;
    }
    if reservation_count == 2 {
        let left = authenticated[0]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        let right = authenticated[1]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        if right.value().order_id() < left.value().order_id() {
            authenticated = [Some(right), Some(left)];
        }
    }
    let mut reservations = [None; 2];
    index = 0;
    while index < reservation_count {
        let current = authenticated[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        reservations[index] = Some(current.value());
        index += 1;
    }
    let price = authenticate_direct_price_precondition_v1(
        program_id,
        &root,
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        reservations,
    )?;
    let mut reservation_accounts = [[0u8; 32]; 2];
    let mut reservation_semantic_ids = [[0u8; 32]; 2];
    index = 0;
    while index < reservation_count {
        let current = authenticated[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        reservation_accounts[index] = current.account().to_bytes();
        reservation_semantic_ids[index] = current.semantic_id();
        index += 1;
    }
    let reservation_count_u8 = u8::try_from(reservation_count)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let authority = DirectSelectionFreezeAuthoritySbfV1 {
        root: root.value_ref(),
        selection_account: accounts[2].key.to_bytes(),
        rent: &selection_rent,
        reservation_accounts: &reservation_accounts,
        reservation_semantic_ids: &reservation_semantic_ids,
        reservation_count: reservation_count_u8,
        price: &price,
    };
    let plan = prepare_direct_selection_freeze_v1(
        &authority,
        DirectRootReplayPostV1 {
            root: root.value(),
            replay: replay.value(),
        },
        sequence,
        observed_slot,
        accounts[2].key.to_bytes(),
        selection_rent,
        reservations,
        price.domain(),
        price.price(),
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
    let bound_replay = apply_direct_candidate_work_v1(
        program_id,
        liveness_accounts,
        &accounts[1],
        &plan.state,
        &plan.selection,
        clutch_direct_market_runtime::DirectMarketActionV1::FreezeBook,
    )?;

    let root_bytes = root.account().to_bytes();
    let bump_seed = [selection_bump];
    let signer_seeds: [&[u8]; 3] = [
        seeds::SEED_DIRECT_SELECTION_V1,
        &root_bytes,
        &bump_seed,
    ];
    create_current_direct_account_v1(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[4],
        &rent_parameters,
        DIRECT_SELECTION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        replay.bump(),
        bound_replay,
        plan.state.root,
    )?;
    write_direct_selection_v1(
        &accounts[2],
        selection_bump,
        plan.selection,
        plan.state.root,
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectReservationAdmissionAuthoritySbfV1<'a> {
    root: &'a DirectMarketRootV1,
    position: &'a AuthenticatedPositionV3,
    existing_peer: &'a Option<DirectReservationV1>,
    reservation_account: [u8; 32],
    order_id: [u8; 32],
    side: clutch_batch::Side,
    outcome: u8,
    quantity: u64,
    minimum_fill: u64,
    partial_policy: clutch_batch::PartialPolicy,
    expiry_epoch: u64,
    limit_price_units_per_egg: u128,
    rent: &'a DirectRentOwnerV1,
}

#[derive(Clone, Copy, Debug)]
struct DirectFoundationAuthoritySbfV1<'a> {
    product_root: &'a clutch_product_series::MarketLifecycleRootV1,
    founder_link: &'a clutch_product_series::SeriesMarketLinkV1,
    bundle: &'a CompiledProductSeriesBundleV5,
    fee_policy: &'a DirectFeePolicyV1,
    binding: &'a DirectMarketBindingV1,
    schedule: &'a DirectScheduleV1,
    root_rent: &'a DirectRentOwnerV1,
    replay_rent: &'a DirectRentOwnerV1,
    family_sequence: u32,
    observed_slot: u64,
}

impl AuthenticatedDirectFoundationV1 for DirectFoundationAuthoritySbfV1<'_> {
    fn authenticate_foundation(
        &self,
        product_root: &clutch_product_series::MarketLifecycleRootV1,
        founder_link: &clutch_product_series::SeriesMarketLinkV1,
        compiler_bundle: &CompiledProductSeriesBundleV5,
        fee_policy: DirectFeePolicyV1,
        _candidate_liveness: AuthenticatedDirectCandidateLivenessV1,
        binding: DirectMarketBindingV1,
        schedule: DirectScheduleV1,
        foundation_slot: u64,
        root_rent: DirectRentOwnerV1,
        action_replay_rent: DirectRentOwnerV1,
        family_admission_sequence: u32,
    ) -> Result<(), DirectMarketErrorV1> {
        if product_root == self.product_root
            && founder_link == self.founder_link
            && compiler_bundle == self.bundle
            && fee_policy == *self.fee_policy
            && binding == *self.binding
            && schedule == *self.schedule
            && foundation_slot == self.observed_slot
            && root_rent == *self.root_rent
            && action_replay_rent == *self.replay_rent
            && family_admission_sequence == self.family_sequence
            && self.observed_slot <= schedule.admission_opens_slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Execute action 1 and atomically admit the Product Direct-family child.
///
/// The twenty-one accounts are Product root, founder link, BundleV5, fresh b1,
/// fresh b3, payer, Realm, Profile, collateral policy, token program, General
/// MarketBindingV3, General runtime, MarketInstanceV2, GenesisV2, System,
/// Rent, Clock, PriceGrid, canonical batch-policy preimage, Realm revenue
/// policy record, and canonical revenue-policy preimage. The raw preimages are
/// accepted only after their digests are joined to the immutable General V3
/// and Genesis/Realm owners; no caller-shaped rate or recipient enters b1.
pub(crate) fn process_direct_initialize_market_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, 21)?;
    require_distinct(accounts)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[5])?;
    require(accounts[5].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[14])?;
    let rent_parameters = read_rent(&accounts[15])?;
    let observed_slot = read_clock_slot(&accounts[16])?;
    decode_direct_empty_payload_v1(payload)?;
    let schedule = DirectScheduleV1::canonical_from_foundation_slot(observed_slot)
        .map_err(map_direct_error_v1)?;

    let mut link_output = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    {
        let data = accounts[1]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV1::decode_into(&data, &mut link_output)?;
    }
    let link_binding = link_output.state.binding();
    let mut product_output = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let product_root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[0],
        link_binding.market_instance_id,
        link_binding.generation,
        true,
        &mut product_output,
    )?;
    let founder_link = authenticate_series_market_link_v1(
        program_id,
        &accounts[1],
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        product_root.account(),
        false,
        &mut link_output,
    )?;
    let product_binding = product_root.state().binding();
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        &accounts[2],
        founder_link.state().binding().compiler_output_id,
    )?;
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
    )?;
    let (market_binding, market_runtime) = authenticate_general_market_v3(
        program_id,
        &accounts[10],
        &accounts[11],
    )?;
    let general_market = market_binding.base().base();
    let market_instance = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        &accounts[12],
        general_market.market_instance_v2_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        &accounts[13],
        market_instance.value().market_genesis_profile_id.content_id(),
    )?;
    let grid = authenticate_direct_price_grid_v1(
        program_id,
        &accounts[17],
        genesis.value().price_grid_id,
        product_binding.realm_id,
    )?;
    let authenticated_fee = authenticate_direct_fee_policy_v1(
        program_id,
        &accounts[18],
        &accounts[19],
        &accounts[20],
        product_binding.realm_id.bytes(),
        market_binding.base().batch_policy_id().bytes(),
        genesis.value().fee_policy_id.bytes(),
    )?;
    let release_id = realm
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let product_market_binding_id = product_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let general_family = product_root
        .state()
        .product_families()
        .family(MarketFamilyV1::General);
    require(
        product_binding.market_instance_id == general_market.market_instance_v2_id
            && product_binding.outcome_count == general_market.outcome_count
            && product_binding.realm_id == genesis.value().realm_id
            && product_binding.collateral_profile_id == genesis.value().profile_id
            && product_binding.collateral_policy_id.bytes() == realm.policy_id().bytes()
            && product_binding.collateral_release_id.bytes() == release_id.bytes()
            && product_binding.price_measure_policy_id.bytes()
                == general_market.price_measure_policy_v1_id.bytes()
            && product_binding.native_claim_basis_id.bytes()
                == general_market.native_claim_basis_id.bytes()
            && general_market.relation_policy_id.bytes()
                == genesis.value().relation_policy_id.bytes()
            && general_market.price_scale == grid.price_scale
            && market_runtime.market_instance_v2_id == general_market.market_instance_v2_id
            && accounts[11].key.to_bytes() == general_market.market.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        market_binding.product_market_root_account().bytes() == accounts[0].key.to_bytes()
            && market_binding.product_market_binding_id().bytes()
                == product_market_binding_id.bytes()
            && market_binding.product_generation() == product_binding.generation
            && market_binding.series_market_link_account().bytes()
                == accounts[1].key.to_bytes()
            && market_binding.series_ordinal() == founder_link.state().binding().ordinal
            && market_binding.compiler_bundle_v5_id().bytes() == bundle.semantic_id().bytes()
            && market_binding.attachment_plan_v4_id().bytes()
                == founder_link.state().binding().attachment_plan_id.bytes()
            && market_binding.market_liability_founding_id().bytes()
                == product_binding.market_liability_founding_id.bytes()
            && market_binding.claim_mint_founding_plan_id().bytes()
                == product_binding.claim_mint_founding_plan_id.bytes()
            && market_binding.claim_issuance_binding_id().bytes()
                == product_binding.claim_issuance_binding_id.bytes()
            && market_binding.general_founding_capability_id().bytes()
                == product_binding.general_founding_capability_id.bytes()
            && general_market.series_plan_v5_id.bytes()
                == founder_link.state().binding().series_plan_id.bytes()
            && general_family.counts().live == 1
            && product_root
                .state()
                .product_families()
                .binding()
                .family_root_id(MarketFamilyV1::General)
                .bytes()
                == accounts[10].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let (root_pda, root_bump) = seeds::direct_market_root_v1_pda(
        program_id,
        &product_binding.market_instance_id.bytes(),
        product_binding.generation,
    );
    let (replay_pda, replay_bump) = seeds::direct_action_replay_v1_pda(program_id, &root_pda);
    let (_, root_donation) = authenticate_fresh_direct_pda_v1(
        &accounts[3],
        (root_pda, root_bump),
    )?;
    let (_, replay_donation) = authenticate_fresh_direct_pda_v1(
        &accounts[4],
        (replay_pda, replay_bump),
    )?;
    require(
        product_root
            .state()
            .product_families()
            .binding()
            .family_root_id(MarketFamilyV1::Direct)
            .bytes() == accounts[3].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let root_rent = DirectRentOwnerV1 {
        payer: accounts[5].key.to_bytes(),
        principal_lamports: rent_parameters.minimum_balance(DIRECT_MARKET_ROOT_ACCOUNT_BYTES)?,
        donation_floor_lamports: root_donation,
    };
    let replay_rent = DirectRentOwnerV1 {
        payer: accounts[5].key.to_bytes(),
        principal_lamports: rent_parameters.minimum_balance(DIRECT_ACTION_REPLAY_ACCOUNT_BYTES)?,
        donation_floor_lamports: replay_donation,
    };
    let family_sequence = product_root
        .state()
        .product_families()
        .family(MarketFamilyV1::Direct)
        .counts()
        .admitted;
    let product_family_prestate_id = product_root
        .state()
        .product_families()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let fee_policy = authenticated_fee.direct;
    let mut direct_binding = DirectMarketBindingV1 {
        market_instance_id: product_binding.market_instance_id.bytes(),
        generation: product_binding.generation,
        outcome_count: product_binding.outcome_count,
        realm_id: product_binding.realm_id.bytes(),
        collateral_profile_id: product_binding.collateral_profile_id.bytes(),
        collateral_policy_id: product_binding.collateral_policy_id.bytes(),
        collateral_release_id: product_binding.collateral_release_id.bytes(),
        resolution_account: product_binding.resolution_account_id.bytes(),
        direct_epoch_semantics_id: [0; 32],
        revenue_policy_id: fee_policy.revenue_policy_id,
        batch_policy_id: fee_policy.batch_policy_id,
        direct_fee_shape_id: fee_policy
            .semantic_id(&DirectRuntimeSha256V1)
            .map_err(map_direct_error_v1)?,
        fee_treasury_owner: fee_policy.treasury_owner,
        fee_dispersion_bps: fee_policy.dispersion_bps,
        fee_floor_range_bps: fee_policy.floor_range_bps,
        fee_maker_rebate_num: fee_policy.maker_rebate_num,
        fee_treasury_num: fee_policy.treasury_num,
        fee_split_den: fee_policy.split_den,
        candidate_lifecycle_policy_id: genesis.value().candidate_lifecycle_policy_id.bytes(),
        candidate_liveness_policy_id: genesis.value().candidate_liveness_policy_id.bytes(),
        // Product has not yet landed the global seven-account capitalization
        // writer/allocation receipt. These padding facts cannot reach the pure
        // foundation because this staged adapter passes `None` below.
        candidate_liveness: DirectCandidateLivenessBindingV1 {
            policy_account: [0; 32],
            policy_data_id: [0; 32],
            global_lifecycle_id: [0; 32],
            global_bundle_binding_id: [0; 32],
            global_capitalization_receipt_id: [0; 32],
            global_bundle_commitment_id: [0; 32],
            candidate_account: [0; 32],
            candidate_data_id: [0; 32],
            candidate_semantic_owner: [0; 32],
            candidate_quote_schedule_id: [0; 32],
            candidate_receipt_program_id: [0; 32],
            candidate_generation: 0,
            first_call_ordinal: 0,
            reserved_calls: DIRECT_CANDIDATE_RESERVED_CALLS_V1,
            reserved_work_lamports: 8,
            allocation_receipt_id: [0; 32],
            work_schedule: DirectCandidateWorkScheduleV1 {
                freeze_book_lamports: 1,
                begin_verification_lamports: 1,
                verify_candidate_lamports: 1,
                finalize_selection_lamports: 1,
                economic_terminal_lamports: 1,
                retire_terminal_lamports: 1,
                retained_candidate_bond_lamports: 1,
            },
            work_schedule_id: [0; 32],
        },
        direct_schedule_policy_id: [0; 32],
        product_root_account: accounts[0].key.to_bytes(),
        product_market_binding_id: product_market_binding_id.bytes(),
        product_family_prestate_id,
        general_product_preauthorization_id: market_binding
            .product_preauthorization_id()
            .bytes(),
        family_admission_sequence: family_sequence,
        founder_series_link_account: accounts[1].key.to_bytes(),
        founder_series_link_binding_id: founder_link
            .state()
            .binding()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
        compiler_bundle_v5_id: bundle.semantic_id().bytes(),
        founder_series_plan_id: founder_link.state().binding().series_plan_id.bytes(),
        founder_series_ordinal: founder_link.state().binding().ordinal,
        direct_root_account: accounts[3].key.to_bytes(),
        action_replay_account: accounts[4].key.to_bytes(),
        general_market_binding: accounts[10].key.to_bytes(),
        general_market_runtime: accounts[11].key.to_bytes(),
        neutral_lamport_sink: founder_link.state().binding().neutral_lamport_sink.bytes(),
        relation_policy_id: general_market.relation_policy_id.bytes(),
        price_policy_id: general_market.price_measure_policy_v1_id.bytes(),
        price_scale: grid.price_scale,
    };
    direct_binding.direct_schedule_policy_id = direct_schedule_policy_id_v1(
        direct_binding,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
    direct_binding.direct_epoch_semantics_id = direct_epoch_semantics_id_v1(
        direct_binding,
        schedule,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
    require(
        direct_binding.neutral_lamport_sink == general_market.neutral_sink.bytes()
            && accounts[5].key.to_bytes() != direct_binding.neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    let authority = DirectFoundationAuthoritySbfV1 {
        product_root: product_root.state(),
        founder_link: founder_link.state(),
        bundle: bundle.value(),
        fee_policy: &fee_policy,
        binding: &direct_binding,
        schedule: &schedule,
        root_rent: &root_rent,
        replay_rent: &replay_rent,
        family_sequence,
        observed_slot,
    };
    let plan = prepare_direct_foundation_v1(
        &authority,
        product_root.state(),
        founder_link.state(),
        bundle.value(),
        fee_policy,
        None,
        direct_binding,
        schedule,
        observed_slot,
        root_rent,
        replay_rent,
        family_sequence,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
    let product_post = product_root
        .state()
        .admit_product_family_child(
            &plan.product_authority,
            MarketFamilyV1::Direct,
            family_sequence,
            plan.admission_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let market_bytes = direct_binding.market_instance_id;
    let generation_bytes = direct_binding.generation.to_le_bytes();
    let root_bump_seed = [root_bump];
    let root_seeds: [&[u8]; 4] = [
        seeds::SEED_DIRECT_MARKET_ROOT_V1,
        &market_bytes,
        &generation_bytes,
        &root_bump_seed,
    ];
    create_current_direct_account_v1(
        program_id, &accounts[5], &accounts[3], &accounts[14], &rent_parameters,
        DIRECT_MARKET_ROOT_ACCOUNT_BYTES, root_rent.principal_lamports,
        root_donation, &root_seeds,
    )?;
    let root_account_bytes = accounts[3].key.to_bytes();
    let replay_bump_seed = [replay_bump];
    let replay_seeds: [&[u8]; 3] = [
        seeds::SEED_DIRECT_ACTION_REPLAY_V1,
        &root_account_bytes,
        &replay_bump_seed,
    ];
    create_current_direct_account_v1(
        program_id, &accounts[5], &accounts[4], &accounts[14], &rent_parameters,
        DIRECT_ACTION_REPLAY_ACCOUNT_BYTES, replay_rent.principal_lamports,
        replay_donation, &replay_seeds,
    )?;
    write_direct_market_root_v1(&accounts[3], root_bump, plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[4], replay_bump, plan.state.replay, plan.state.root,
    )?;
    write_product_root_post_v1(&accounts[0], product_root, &product_post)
}

impl AuthenticatedDirectReservationAdmissionV1 for DirectReservationAdmissionAuthoritySbfV1<'_> {
    fn authenticate_admission(
        &self,
        root: DirectMarketRootV1,
        position: AuthenticatedPositionV3,
        existing_peer: Option<DirectReservationV1>,
        reservation_account: [u8; 32],
        order_id: [u8; 32],
        side: clutch_batch::Side,
        outcome: u8,
        quantity: u64,
        minimum_fill: u64,
        partial_policy: clutch_batch::PartialPolicy,
        expiry_epoch: u64,
        limit_price_units_per_egg: u128,
        rent: DirectRentOwnerV1,
    ) -> Result<(), DirectMarketErrorV1> {
        if root == *self.root
            && position == *self.position
            && existing_peer == *self.existing_peer
            && reservation_account == self.reservation_account
            && order_id == self.order_id
            && side == self.side
            && outcome == self.outcome
            && quantity == self.quantity
            && minimum_fill == self.minimum_fill
            && partial_policy == self.partial_policy
            && expiry_epoch == self.expiry_epoch
            && limit_price_units_per_egg == self.limit_price_units_per_egg
            && rent == *self.rent
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Execute action 2 across b1/b3, a fresh b4, PositionV3, and GEN1.
///
/// The fixed nineteen-account prefix is root, Direct replay, fresh
/// Reservation, owner/payer, Position, GEN1, Realm, Profile, collateral
/// policy, token program, General MarketBindingV3, General runtime,
/// MarketInstanceV2 artifact, System, Rent, Clock, BundleV5, GenesisV2, and
/// PriceGrid. When the root owns one live Reservation, its exact read-only b4
/// peer is the sole suffix account; the root count, never payload data, fixes
/// whether that suffix exists. All funding is derived from Position state.
pub(crate) fn process_direct_admit_order_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(accounts.len() >= 19, ClutchError::AccountCount)?;
    let request = DirectAdmitOrderPayloadV1::decode(payload)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    let peer_count = usize::from(root.value().live_reservations());
    require(peer_count <= 1, ClutchError::MismatchedState)?;
    let expected_count = 19usize
        .checked_add(peer_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    require_distinct(accounts)?;
    let direct_replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[13])?;
    let rent_parameters = read_rent(&accounts[14])?;
    let observed_slot = read_clock_slot(&accounts[15])?;
    authenticate_direct_order_limit_v1(
        program_id,
        &root,
        &accounts[16],
        &accounts[17],
        &accounts[18],
        request.limit_price_units_per_egg,
    )?;

    let bound = authenticate_direct_general_market_v1(
        program_id,
        &root,
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        &accounts[12],
        &accounts[17],
    )?;
    let position_replay = authenticate_current_general_position_replay_v3(
        program_id,
        bound,
        &accounts[10],
        &accounts[11],
        &accounts[4],
        &accounts[5],
        accounts[3].key.to_bytes(),
    )?;
    let existing_peer = if peer_count == 0 {
        None
    } else {
        let peer = authenticate_direct_reservation_readonly_v1(
            program_id,
            &accounts[19],
            &root,
        )?;
        require(
            peer.account().to_bytes()
                == root.value().reservation_account(0).map_err(map_direct_error_v1)?
                && peer.semantic_id()
                    == root.value().reservation_semantic_id(0).map_err(map_direct_error_v1)?,
            ClutchError::MismatchedState,
        )?;
        Some(peer.value())
    };
    let (reservation_pda, reservation_bump) = seeds::direct_reservation_v1_pda(
        program_id,
        &root.account(),
        &request.order_id,
    );
    let (_, donation_floor_lamports) = authenticate_fresh_direct_pda_v1(
        &accounts[2],
        (reservation_pda, reservation_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_RESERVATION_ACCOUNT_BYTES)?;
    let reservation_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    let authority = DirectReservationAdmissionAuthoritySbfV1 {
        root: root.value_ref(),
        position: &position_replay.position,
        existing_peer: &existing_peer,
        reservation_account: accounts[2].key.to_bytes(),
        order_id: request.order_id,
        side: request.side,
        outcome: request.outcome,
        quantity: request.quantity,
        minimum_fill: request.minimum_fill,
        partial_policy: request.partial_policy,
        expiry_epoch: request.expiry_epoch,
        limit_price_units_per_egg: request.limit_price_units_per_egg,
        rent: &reservation_rent,
    };
    let plan = prepare_direct_reservation_admission_with_replay_v1(
        &authority,
        DirectRootReplayPostV1 {
            root: root.value(),
            replay: direct_replay.value(),
        },
        position_replay.replay,
        existing_peer,
        sequence,
        observed_slot,
        DirectReservationOrderInputV1 {
            reservation_account: accounts[2].key.to_bytes(),
            order_id: request.order_id,
            side: request.side,
            outcome: request.outcome,
            quantity: request.quantity,
            minimum_fill: request.minimum_fill,
            partial_policy: request.partial_policy,
            expiry_epoch: request.expiry_epoch,
            limit_price_units_per_egg: request.limit_price_units_per_egg,
            rent: reservation_rent,
        },
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;

    let root_bytes = root.account().to_bytes();
    let bump_seed = [reservation_bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_DIRECT_RESERVATION_V1,
        &root_bytes,
        &request.order_id,
        &bump_seed,
    ];
    create_current_direct_account_v1(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[13],
        &rent_parameters,
        DIRECT_RESERVATION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    write_position_post_v1(&accounts[4], &plan.position_poststate)?;
    write_general_replay_post_v1(&accounts[5], &plan.replay_transition)?;
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        direct_replay.bump(),
        plan.state.replay,
        plan.state.root,
    )?;
    write_direct_reservation_v1(
        &accounts[2],
        reservation_bump,
        plan.reservation,
        plan.state.root,
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectReservationCancelAuthoritySbfV1<'a> {
    state: &'a DirectRootReplayPostV1,
    reservation: &'a DirectReservationV1,
    position_replay: &'a clutch_general_v2_contract::GeneralPositionReplayPrestateV1,
    observed_lamports: u64,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectReservationCancelV1 for DirectReservationCancelAuthoritySbfV1<'_> {
    fn authenticate_cancel(
        &self,
        state: DirectRootReplayPostV1,
        reservation: DirectReservationV1,
        position_replay: clutch_general_v2_contract::GeneralPositionReplayPrestateV1,
        observed_reservation_lamports: u64,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state == *self.state
            && reservation == *self.reservation
            && position_replay == *self.position_replay
            && observed_reservation_lamports == self.observed_lamports
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Execute action 3 and immediately retire exactly one active b4 archive.
///
/// The sixteen accounts are root, Direct replay, Reservation, owner/payer,
/// Position, GEN1, Realm, Profile, collateral policy, token program, General
/// MarketBindingV3, General runtime, MarketInstanceV2, GenesisV2, neutral
/// lamport sink, and Clock. Principal returns only to the persisted payer;
/// every other lamport goes only to the root's Realm-authenticated sink.
pub(crate) fn process_direct_cancel_order_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, 16)?;
    require_distinct(accounts)?;
    decode_direct_empty_payload_v1(payload)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    let direct_replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        &root,
    )?;
    let reservation = authenticate_direct_reservation_writable_v1(
        program_id,
        &accounts[2],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require(
        accounts[14].is_writable
            && !accounts[14].is_signer
            && accounts[14].key.to_bytes() == root.value().binding.neutral_lamport_sink
            && accounts[3].key.to_bytes() == reservation.value().owner()
            && accounts[3].key.to_bytes() == reservation.value().rent().payer,
        ClutchError::MismatchedState,
    )?;
    let observed_slot = read_clock_slot(&accounts[15])?;
    let bound = authenticate_direct_general_market_v1(
        program_id,
        &root,
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        &accounts[12],
        &accounts[13],
    )?;
    let position_replay = authenticate_current_general_position_replay_v3(
        program_id,
        bound,
        &accounts[10],
        &accounts[11],
        &accounts[4],
        &accounts[5],
        accounts[3].key.to_bytes(),
    )?;
    let state = DirectRootReplayPostV1 {
        root: root.value(),
        replay: direct_replay.value(),
    };
    let authority = DirectReservationCancelAuthoritySbfV1 {
        state: &state,
        reservation: reservation.value_ref(),
        position_replay: &position_replay.replay,
        observed_lamports: reservation.observed_lamports(),
        sequence,
        slot: observed_slot,
    };
    let plan = prepare_direct_reservation_cancel_v1(
        &authority,
        state,
        reservation.value(),
        position_replay.replay,
        reservation.observed_lamports(),
        sequence,
        observed_slot,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
    require(
        plan.retirement.source_count == 1
            && plan.retirement.refund_count == 1
            && plan.retirement.neutral_lamport_sink == accounts[14].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let refund = plan.retirement.refunds[0]
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source = plan.retirement.sources[0]
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        source.account == accounts[2].key.to_bytes()
            && source.observed_lamports == accounts[2].lamports()
            && refund.recipient == accounts[3].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    credit_lamports_v1(&accounts[3], refund.lamports)?;
    credit_lamports_v1(&accounts[14], plan.retirement.surplus_lamports)?;
    write_position_post_v1(&accounts[4], &plan.endpoint.position_poststate)?;
    write_general_replay_post_v1(&accounts[5], &plan.endpoint.replay_transition)?;
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        direct_replay.bump(),
        plan.state.replay,
        plan.state.root,
    )?;
    close_direct_program_account_v1(&accounts[2], source.observed_lamports)
}

#[derive(Clone, Copy, Debug)]
struct DirectEconomicTerminalAuthoritySbfV1<'a> {
    state: &'a DirectRootReplayPostV1,
    selection: &'a DirectSelectionV1,
    endpoints: &'a [Option<DirectEndpointPrestateV1>; 2],
    treasury: Option<DirectFeeTreasuryPrestateV1>,
    reason: DirectTerminalReasonV1,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectEconomicTerminalV1 for DirectEconomicTerminalAuthoritySbfV1<'_> {
    fn authenticate_terminal(
        &self,
        state: DirectRootReplayPostV1,
        selection: DirectSelectionV1,
        ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        fee_terminal: Option<DirectFeeTerminalV1>,
        treasury: Option<DirectFeeTreasuryPrestateV1>,
        reason: DirectTerminalReasonV1,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state == *self.state
            && selection == *self.selection
            && ordered_endpoints == self.endpoints
            && fee_terminal.is_some() == (reason == DirectTerminalReasonV1::Settled)
            && treasury == self.treasury
            && reason == self.reason
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DirectMissedFreezeTerminalAuthoritySbfV1<'a> {
    state: &'a DirectRootReplayPostV1,
    selection_account: [u8; 32],
    selection_rent: DirectRentOwnerV1,
    reservation_accounts: [[u8; 32]; 2],
    reservation_semantic_ids: [[u8; 32]; 2],
    reservation_count: u8,
    price: &'a AuthenticatedDirectPricePreconditionV1,
    endpoints: &'a [Option<DirectEndpointPrestateV1>; 2],
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectEconomicTerminalV1 for DirectMissedFreezeTerminalAuthoritySbfV1<'_> {
    fn authenticate_terminal(
        &self,
        state: DirectRootReplayPostV1,
        selection: DirectSelectionV1,
        ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        fee_terminal: Option<DirectFeeTerminalV1>,
        treasury: Option<DirectFeeTreasuryPrestateV1>,
        reason: DirectTerminalReasonV1,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state != *self.state
            || selection.account() != self.selection_account
            || selection.rent() != self.selection_rent
            || selection.reservation_count() != self.reservation_count
            || *selection.domain() != self.price.domain()
            || *selection.price() != self.price.price()
            || selection.candidate_count() != 0
            || selection.verification_cursor() != 0
            || selection.selected_pair().is_some()
            || selection.terminal_receipt_id() != [0; 32]
            || ordered_endpoints != self.endpoints
            || fee_terminal.is_some()
            || treasury.is_some()
            || reason != DirectTerminalReasonV1::MissedFreezeLapse
            || consumed_sequence != self.sequence
            || observed_slot != self.slot
        {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let expected_phase = if self.reservation_count == 2 {
            DirectSelectionPhaseV1::SubmissionOpen
        } else {
            DirectSelectionPhaseV1::FrozenEmpty
        };
        if selection.phase() != expected_phase {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let mut index = 0usize;
        while index < 2 {
            if index < usize::from(self.reservation_count) {
                let bounded = u8::try_from(index)
                    .map_err(|_| DirectMarketErrorV1::Arithmetic)?;
                if selection.reservation_account(bounded)? != self.reservation_accounts[index]
                    || selection.reservation_semantic_id(bounded)?
                        != self.reservation_semantic_ids[index]
                {
                    return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
                }
            } else if self.reservation_accounts[index] != [0; 32]
                || self.reservation_semantic_ids[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Execute action 10 directly from an open root after submission close.
///
/// Fixed accounts 0..=18 are b1, b3, fresh b2, payer, System, Rent, Clock,
/// BundleV5, NativeClaimBasis, PriceMeasurePolicy, GenesisV2, PriceGrid,
/// Realm, Profile, collateral policy, token program, General MarketBindingV3,
/// General runtime, and MarketInstanceV2. Exactly the root-owned
/// live count of `(b4, PositionV3, GEN1)` triples follows in canonical order,
/// then immutable liveness policy, writable Candidate, writable keeper signer,
/// and the Candidate's immutable writable payer.
/// The payload is empty. The exact Product-grid price vector is derived from
/// the authenticated canonical book and cannot be caller-selected or supplied
/// later by selection.
#[inline(never)]
fn process_direct_missed_freeze_lapse_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(accounts.len() >= 19, ClutchError::AccountCount)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    require(root.value().phase() == DirectRootPhaseV1::Open, ClutchError::MismatchedState)?;
    let endpoint_count = usize::from(root.value().live_reservations());
    let endpoint_end = endpoint_count
        .checked_mul(3)
        .and_then(|value| value.checked_add(19))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_count = endpoint_end
        .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    require_distinct(&accounts[..19])?;
    require_direct_endpoint_alias_contract_v1(accounts, 19, endpoint_count)?;
    decode_direct_empty_payload_v1(payload)?;
    let replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[4])?;
    let rent_parameters = read_rent(&accounts[5])?;
    let observed_slot = read_clock_slot(&accounts[6])?;
    let (selection_pda, selection_bump) =
        seeds::direct_selection_v1_pda(program_id, &root.account());
    let (_, donation_floor_lamports) = authenticate_fresh_direct_pda_v1(
        &accounts[2],
        (selection_pda, selection_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let selection_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    selection_rent.validate().map_err(map_direct_error_v1)?;

    let bound = authenticate_direct_general_market_v1(
        program_id,
        &root,
        &accounts[12],
        &accounts[13],
        &accounts[14],
        &accounts[15],
        &accounts[16],
        &accounts[17],
        &accounts[18],
        &accounts[10],
    )?;
    let mut authenticated: [Option<AuthenticatedDirectReservationV1>; 2] = [None; 2];
    let mut endpoints = [None; 2];
    let mut reservations = [None; 2];
    let mut reservation_accounts = [[0u8; 32]; 2];
    let mut reservation_semantic_ids = [[0u8; 32]; 2];
    let mut index = 0usize;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v1(19, index)?;
        let reservation = authenticate_direct_reservation_writable_v1(
            program_id,
            &accounts[first],
            &root,
        )?;
        if index != 0 {
            let previous = authenticated[index - 1]
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            require(
                previous.value().order_id() < reservation.value().order_id(),
                ClutchError::MismatchedState,
            )?;
        }
        let position_replay = authenticate_current_general_position_replay_v3(
            program_id,
            bound,
            &accounts[16],
            &accounts[17],
            &accounts[first + 1],
            &accounts[first + 2],
            reservation.value().owner(),
        )?;
        authenticated[index] = Some(reservation);
        endpoints[index] = Some(DirectEndpointPrestateV1 {
            reservation: reservation.value(),
            position_replay: position_replay.replay,
        });
        reservations[index] = Some(reservation.value());
        reservation_accounts[index] = reservation.account().to_bytes();
        reservation_semantic_ids[index] = reservation.semantic_id();
        index += 1;
    }
    let price = authenticate_direct_price_precondition_v1(
        program_id,
        &root,
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        reservations,
    )?;
    let reservation_count = u8::try_from(endpoint_count)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let freeze_authority = DirectSelectionFreezeAuthoritySbfV1 {
        root: root.value_ref(),
        selection_account: accounts[2].key.to_bytes(),
        rent: &selection_rent,
        reservation_accounts: &reservation_accounts,
        reservation_semantic_ids: &reservation_semantic_ids,
        reservation_count,
        price: &price,
    };
    let state = DirectRootReplayPostV1 {
        root: root.value(),
        replay: replay.value(),
    };
    let terminal_authority = DirectMissedFreezeTerminalAuthoritySbfV1 {
        state: &state,
        selection_account: accounts[2].key.to_bytes(),
        selection_rent,
        reservation_accounts,
        reservation_semantic_ids,
        reservation_count,
        price: &price,
        endpoints: &endpoints,
        sequence,
        slot: observed_slot,
    };
    let plan = prepare_direct_missed_freeze_terminal_v1(
        &freeze_authority,
        &terminal_authority,
        state,
        accounts[2].key.to_bytes(),
        selection_rent,
        reservations,
        price.domain(),
        price.price(),
        endpoints,
        None,
        None,
        sequence,
        observed_slot,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
    require_direct_candidate_liveness_creation_aliases_v1(
        accounts,
        endpoint_end,
        3,
    )?;
    let bound_replay = apply_direct_candidate_work_v1(
        program_id,
        &accounts[endpoint_end..],
        &accounts[1],
        &plan.state,
        &plan.selection,
        clutch_direct_market_runtime::DirectMarketActionV1::LapseEmpty,
    )?;

    let root_bytes = root.account().to_bytes();
    let bump_seed = [selection_bump];
    let signer_seeds: [&[u8]; 3] = [
        seeds::SEED_DIRECT_SELECTION_V1,
        &root_bytes,
        &bump_seed,
    ];
    create_current_direct_account_v1(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[4],
        &rent_parameters,
        DIRECT_SELECTION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    index = 0;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v1(19, index)?;
        let endpoint = plan.endpoints[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        let reservation = authenticated[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        write_position_post_v1(&accounts[first + 1], &endpoint.position_poststate)?;
        write_general_replay_post_v1(&accounts[first + 2], &endpoint.replay_transition)?;
        write_direct_reservation_v1(
            &accounts[first],
            reservation.bump(),
            endpoint.reservation_post,
            plan.state.root,
        )?;
        index += 1;
    }
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        replay.bump(),
        bound_replay,
        plan.state.root,
    )?;
    write_direct_selection_v1(
        &accounts[2],
        selection_bump,
        plan.selection,
        plan.state.root,
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectFamilyTerminalAuthoritySbfV1<'a> {
    product_root: &'a clutch_product_series::MarketLifecycleRootV1,
    founder_link: &'a clutch_product_series::SeriesMarketLinkV1,
    bundle: &'a CompiledProductSeriesBundleV5,
    root: &'a DirectMarketRootV1,
    root_semantic_id: [u8; 32],
    replay: &'a DirectActionReplayV1,
    replay_semantic_id: [u8; 32],
    selection: &'a DirectSelectionV1,
    reservations: &'a [Option<DirectReservationV1>; 2],
    final_resolution: DirectFinalResolutionV1,
    retirement: &'a DirectRetirementTransferV1,
    retirement_transfer_id: [u8; 32],
    sequence: u64,
    slot: u64,
    family_sequence: u32,
}

impl AuthenticatedDirectTerminalV1 for DirectFamilyTerminalAuthoritySbfV1<'_> {
    fn authenticate_terminal(
        &self,
        product_root: &clutch_product_series::MarketLifecycleRootV1,
        founder_link: &clutch_product_series::SeriesMarketLinkV1,
        compiler_bundle: &CompiledProductSeriesBundleV5,
        root: &DirectMarketRootV1,
        root_semantic_id: [u8; 32],
        replay: &DirectActionReplayV1,
        replay_semantic_id: [u8; 32],
        selection: &DirectSelectionV1,
        reservations: &[Option<DirectReservationV1>; 2],
        final_resolution: DirectFinalResolutionV1,
        retirement: &DirectRetirementTransferV1,
        retirement_transfer_id: [u8; 32],
        consumed_sequence: u64,
        observed_slot: u64,
        family_terminal_sequence: u32,
    ) -> Result<(), DirectMarketErrorV1> {
        if product_root == self.product_root
            && founder_link == self.founder_link
            && compiler_bundle == self.bundle
            && root == self.root
            && root_semantic_id == self.root_semantic_id
            && replay == self.replay
            && replay_semantic_id == self.replay_semantic_id
            && selection == self.selection
            && reservations == self.reservations
            && final_resolution == self.final_resolution
            && retirement == self.retirement
            && retirement_transfer_id == self.retirement_transfer_id
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
            && family_terminal_sequence == self.family_sequence
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Execute action 13, derive the terminal receipt from the exact b3 successor,
/// commit it into Product, refund exact persisted principal, sink every
/// surplus lamport, and delete b1, b2, b3, and the complete live b4 archive.
///
/// Fixed accounts 0..=8 are Product root, founder link, BundleV5, b1, b3,
/// b2, ResolutionV5, Clock, and the Realm neutral sink. The exact b2-order
/// live b4 prefix follows, then the sorted unique persisted payer accounts.
/// Both suffix lengths are derived from authenticated state.
#[inline(never)]
pub(crate) fn process_direct_retire_terminal_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(accounts.len() >= 9, ClutchError::AccountCount)?;
    require(
        accounts.len() <= DIRECT_RETIRE_TERMINAL_MAX_ACCOUNTS,
        ClutchError::AccountCount,
    )?;
    decode_direct_empty_payload_v1(payload)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[3])?;
    let replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[4],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v1(
        program_id,
        &accounts[5],
        &root,
    )?;
    let final_resolution = authenticate_direct_resolution_v5(
        program_id,
        &root,
        &accounts[6],
    )?;
    let observed_slot = read_clock_slot(&accounts[7])?;
    require(
        accounts[8].is_writable
            && !accounts[8].is_signer
            && !accounts[8].executable
            && accounts[8].key.to_bytes() == root.value().binding.neutral_lamport_sink,
        ClutchError::MismatchedState,
    )?;
    let reservation_count = usize::from(selection.value().reservation_count());
    require(
        reservation_count == usize::from(root.value().live_reservations()),
        ClutchError::MismatchedState,
    )?;
    let reservation_end = 9usize
        .checked_add(reservation_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(accounts.len() >= reservation_end, ClutchError::AccountCount)?;
    require_distinct(&accounts[..reservation_end])?;

    let mut link_output = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    {
        let data = accounts[1]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV1::decode_into(&data, &mut link_output)?;
    }
    let binding = root.value().binding();
    let link_binding = link_output.state.binding();
    let mut product_output = Box::new(MarketLifecycleRootAccountV1::decode_buffer());
    let product_root = authenticate_market_lifecycle_root_v1(
        program_id,
        &accounts[0],
        MarketInstanceV2Id::from_bytes(binding.market_instance_id),
        binding.generation,
        true,
        &mut product_output,
    )?;
    let founder_link = authenticate_series_market_link_v1(
        program_id,
        &accounts[1],
        SeriesPlanV5Id::from_bytes(binding.founder_series_plan_id),
        binding.founder_series_ordinal,
        MarketInstanceV2Id::from_bytes(binding.market_instance_id),
        binding.generation,
        product_root.account(),
        false,
        &mut link_output,
    )?;
    require(
        link_binding == founder_link.state().binding()
            && accounts[0].key.to_bytes() == binding.product_root_account
            && accounts[1].key.to_bytes() == binding.founder_series_link_account,
        ClutchError::MismatchedState,
    )?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        &accounts[2],
        ContentId::from_bytes(binding.compiler_bundle_v5_id),
    )?;

    let mut authenticated: [Option<AuthenticatedDirectReservationV1>; 2] = [None; 2];
    let mut reservations = [None; 2];
    let mut sources: [Option<DirectRetirementSourceV1>; 5] = [None; 5];
    sources[0] = Some(DirectRetirementSourceV1 {
        account: accounts[3].key.to_bytes(),
        rent: root.value().root_rent(),
        observed_lamports: root.observed_lamports(),
    });
    sources[1] = Some(DirectRetirementSourceV1 {
        account: accounts[4].key.to_bytes(),
        rent: replay.value().rent(),
        observed_lamports: replay.observed_lamports(),
    });
    sources[2] = Some(DirectRetirementSourceV1 {
        account: accounts[5].key.to_bytes(),
        rent: selection.value().rent(),
        observed_lamports: selection.observed_lamports(),
    });
    let mut index = 0usize;
    while index < reservation_count {
        let reservation = authenticate_direct_reservation_writable_v1(
            program_id,
            &accounts[9 + index],
            &root,
        )?;
        let bounded = u8::try_from(index)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            reservation.account().to_bytes()
                == selection.value().reservation_account(bounded).map_err(map_direct_error_v1)?
                && reservation.semantic_id()
                    != selection.value().reservation_semantic_id(bounded)
                        .map_err(map_direct_error_v1)?
                && reservation.value().terminal_receipt_id()
                    == selection.value().terminal_receipt_id(),
            ClutchError::MismatchedState,
        )?;
        authenticated[index] = Some(reservation);
        reservations[index] = Some(reservation.value());
        sources[3 + index] = Some(DirectRetirementSourceV1 {
            account: reservation.account().to_bytes(),
            rent: reservation.value().rent(),
            observed_lamports: reservation.observed_lamports(),
        });
        index += 1;
    }
    let retirement = build_direct_retirement_transfer_v1(
        sources,
        accounts[8].key.to_bytes(),
    )
    .map_err(map_direct_error_v1)?;
    let expected_count = reservation_end
        .checked_add(usize::from(retirement.refund_count))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    index = reservation_end;
    while index < expected_count {
        require(
            accounts[index].is_writable
                && !accounts[index].is_signer
                && !accounts[index].executable,
            ClutchError::NotWritable,
        )?;
        let mut prior = 0usize;
        while prior < reservation_end {
            require(accounts[index].key != accounts[prior].key, ClutchError::AccountAlias)?;
            prior += 1;
        }
        if index != reservation_end {
            require(
                accounts[index - 1].key.to_bytes() < accounts[index].key.to_bytes(),
                ClutchError::AccountAlias,
            )?;
        }
        let refund_index = index
            .checked_sub(reservation_end)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        let refund = retirement.refunds[refund_index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            accounts[index].key.to_bytes() == refund.recipient,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    let retirement_transfer_id = retirement
        .semantic_id(&DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    let family_sequence = product_root
        .state()
        .product_families()
        .family(MarketFamilyV1::Direct)
        .counts()
        .terminal;
    let state = DirectRootReplayPostV1 {
        root: root.value(),
        replay: replay.value(),
    };
    let authority = DirectFamilyTerminalAuthoritySbfV1 {
        product_root: product_root.state(),
        founder_link: founder_link.state(),
        bundle: bundle.value(),
        root: root.value_ref(),
        root_semantic_id: root.semantic_id(),
        replay: replay.value_ref(),
        replay_semantic_id: replay.semantic_id(),
        selection: selection.value_ref(),
        reservations: &reservations,
        final_resolution,
        retirement: &retirement,
        retirement_transfer_id,
        sequence,
        slot: observed_slot,
        family_sequence,
    };
    let plan = prepare_direct_family_terminal_v1(
        &authority,
        product_root.state(),
        founder_link.state(),
        bundle.value(),
        &state,
        selection.value_ref(),
        &reservations,
        final_resolution,
        &retirement,
        sequence,
        observed_slot,
        family_sequence,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;
    let product_post = product_root
        .state()
        .terminalize_product_family_child(
            &plan.product_authority,
            MarketFamilyV1::Direct,
            family_sequence,
            plan.terminal_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    index = 0;
    while index < usize::from(plan.retirement.refund_count) {
        let refund = plan.retirement.refunds[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        credit_lamports_v1(&accounts[reservation_end + index], refund.lamports)?;
        index += 1;
    }
    credit_lamports_v1(&accounts[8], plan.retirement.surplus_lamports)?;
    write_product_root_post_v1(&accounts[0], product_root, &product_post)?;
    index = 0;
    while index < reservation_count {
        let reservation = authenticated[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        close_direct_program_account_v1(
            &accounts[9 + index],
            reservation.observed_lamports(),
        )?;
        index += 1;
    }
    close_direct_program_account_v1(&accounts[5], selection.observed_lamports())?;
    close_direct_program_account_v1(&accounts[4], replay.observed_lamports())?;
    close_direct_program_account_v1(&accounts[3], root.observed_lamports())
}

/// Execute action 8's empty no-trade branch or actions 9..=12 over the
/// complete b2-owned endpoint prefix.
///
/// Fixed accounts 0..=11 are b1, b3, b2, Realm, Profile, collateral policy,
/// token program, General MarketBindingV3, General runtime, MarketInstanceV2,
/// GenesisV2, and Clock. Exactly `selection.reservation_count`
/// triples follow in b2 order: b4, PositionV3, GEN1. No payload count or
/// endpoint index is accepted. A repeated Position/GEN1 pair is admitted only
/// when both exact b4 owners name that same pair; the pure composer then
/// advances the second GEN1 ordinal from the first successor. Action 9 then
/// requires the canonical batch preimage, Realm revenue record, and revenue
/// preimage. When the immutable policy can credit treasury, its exact
/// PositionV3/GEN1 pair is the final suffix and may alias an endpoint only as
/// the complete pair. Lapse actions admit no fee suffix. Actions 8..=12 append
/// the canonical four-account Candidate liveness
/// suffix after every complete bond-refund owner. No caller work count,
/// ordinal, or payment amount enters the transition.
pub(crate) fn process_direct_economic_terminal_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    require(accounts.len() >= 3, ClutchError::AccountCount)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    if action == DirectMarketAction::LapseEmpty
        && root.value().phase() == DirectRootPhaseV1::Open
    {
        return process_direct_missed_freeze_lapse_v1(
            program_id,
            accounts,
            sequence,
            payload,
        );
    }
    require(accounts.len() >= 12, ClutchError::AccountCount)?;
    decode_direct_empty_payload_v1(payload)?;
    let direct_replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v1(
        program_id,
        &accounts[2],
        &root,
    )?;
    let reason = match action {
        DirectMarketAction::FinalizeSelection => DirectTerminalReasonV1::NoCandidate,
        DirectMarketAction::SettlePair => DirectTerminalReasonV1::Settled,
        DirectMarketAction::LapseEmpty => DirectTerminalReasonV1::EmptyLapse,
        DirectMarketAction::LapseUnselected => DirectTerminalReasonV1::UnselectedLapse,
        DirectMarketAction::LapseSelected => DirectTerminalReasonV1::SelectedLapse,
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };
    let endpoint_count = usize::from(selection.value().reservation_count());
    let endpoint_end = endpoint_count
        .checked_mul(3)
        .and_then(|value| value.checked_add(12))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let root_fee_policy = root.value().binding().fee_policy();
    let treasury_meta_required = reason == DirectTerminalReasonV1::Settled
        && root_fee_policy.fee_bearing()
        && root_fee_policy.treasury_num != 0;
    let fee_suffix_count = if reason == DirectTerminalReasonV1::Settled {
        if treasury_meta_required { 5usize } else { 3usize }
    } else {
        0usize
    };
    let base_count = endpoint_end
        .checked_add(fee_suffix_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let liveness_suffix_count = DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1;
    let minimum_count = base_count
        .checked_add(liveness_suffix_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let maximum_count = minimum_count
        .checked_add(3)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        accounts.len() >= minimum_count && accounts.len() <= maximum_count,
        ClutchError::AccountCount,
    )?;
    require_distinct(&accounts[..12])?;
    require_direct_endpoint_alias_contract_v1(accounts, 12, endpoint_count)?;
    require_direct_fee_suffix_alias_contract_v1(
        accounts,
        endpoint_count,
        reason == DirectTerminalReasonV1::Settled,
        treasury_meta_required,
    )?;
    let observed_slot = read_clock_slot(&accounts[11])?;
    let bound = authenticate_direct_general_market_v1(
        program_id,
        &root,
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
    )?;
    let mut endpoints = [None; 2];
    let mut index = 0usize;
    while index < endpoint_count {
        let first = direct_endpoint_first_v1(index)?;
        let reservation = authenticate_direct_reservation_writable_v1(
            program_id,
            &accounts[first],
            &root,
        )?;
        let selection_index = u8::try_from(index)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            selection.value().reservation_account(selection_index)
                .map_err(map_direct_error_v1)? == reservation.account().to_bytes()
                && selection.value().reservation_semantic_id(selection_index)
                    .map_err(map_direct_error_v1)? == reservation.semantic_id(),
            ClutchError::MismatchedState,
        )?;
        let position_replay = authenticate_current_general_position_replay_v3(
            program_id,
            bound,
            &accounts[7],
            &accounts[8],
            &accounts[first + 1],
            &accounts[first + 2],
            reservation.value().owner(),
        )?;
        endpoints[index] = Some(DirectEndpointPrestateV1 {
            reservation: reservation.value(),
            position_replay: position_replay.replay,
        });
        index += 1;
    }
    let (authenticated_fee, treasury_prestate) = if reason == DirectTerminalReasonV1::Settled {
        let authenticated_fee = authenticate_direct_fee_policy_v1(
            program_id,
            &accounts[endpoint_end],
            &accounts[endpoint_end + 1],
            &accounts[endpoint_end + 2],
            root.value().binding.realm_id,
            root.value().binding.batch_policy_id,
            root.value().binding.revenue_policy_id,
        )?;
        require(
            authenticated_fee.direct == root_fee_policy,
            ClutchError::MismatchedState,
        )?;
        let treasury = if treasury_meta_required {
            let position_replay = authenticate_current_general_position_replay_v3(
                program_id,
                bound,
                &accounts[7],
                &accounts[8],
                &accounts[endpoint_end + 3],
                &accounts[endpoint_end + 4],
                root_fee_policy.treasury_owner,
            )?;
            Some(DirectFeeTreasuryPrestateV1 {
                position_replay: position_replay.replay,
            })
        } else {
            None
        };
        (Some(authenticated_fee), treasury)
    } else {
        (None, None)
    };
    let state = DirectRootReplayPostV1 {
        root: root.value(),
        replay: direct_replay.value(),
    };
    let authority = DirectEconomicTerminalAuthoritySbfV1 {
        state: &state,
        selection: selection.value_ref(),
        endpoints: &endpoints,
        treasury: treasury_prestate,
        reason,
        sequence,
        slot: observed_slot,
    };
    let plan = prepare_direct_economic_terminal_v1(
        &authority,
        state,
        selection.value(),
        endpoints,
        authenticated_fee.as_ref().map(|value| &value.revenue),
        treasury_prestate,
        reason,
        sequence,
        observed_slot,
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;

    let bond_principal_before = selection
        .value()
        .outstanding_candidate_bond_lamports(root.value())
        .map_err(map_direct_error_v1)?;
    let selection_rent = selection.value().rent();
    let accounted_selection_balance = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_before))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        selection.observed_lamports() >= accounted_selection_balance,
        ClutchError::MismatchedState,
    )?;
    let refund_count = plan
        .candidate_bond_refunds
        .map_or(0usize, |refunds| usize::from(refunds.refund_count));
    let refund_end = base_count
        .checked_add(refund_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_count = refund_end
        .checked_add(liveness_suffix_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    if let Some(refunds) = plan.candidate_bond_refunds {
        require(
            refunds.total_lamports == bond_principal_before,
            ClutchError::MismatchedState,
        )?;
        index = 0;
        while index < refund_count {
            let refund = refunds.refunds[index]
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            let account = &accounts[base_count + index];
            require(
                account.is_writable
                    && !account.executable
                    && account.key.to_bytes() == refund.recipient,
                ClutchError::MismatchedState,
            )?;
            let mut prior = 0usize;
            while prior < base_count {
                require(account.key != accounts[prior].key, ClutchError::AccountAlias)?;
                prior += 1;
            }
            if index != 0 {
                require(
                    accounts[base_count + index - 1].key.to_bytes()
                        < account.key.to_bytes(),
                    ClutchError::AccountAlias,
                )?;
            }
            index += 1;
        }
    } else {
        require(bond_principal_before == 0, ClutchError::MismatchedState)?;
    }

    if let Some(refunds) = plan.candidate_bond_refunds {
        debit_lamports_v1(&accounts[2], refunds.total_lamports)?;
        index = 0;
        while index < refund_count {
            let refund = refunds.refunds[index]
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            credit_lamports_v1(&accounts[base_count + index], refund.lamports)?;
            index += 1;
        }
        require(
            accounts[2].lamports()
                == selection
                    .observed_lamports()
                    .checked_sub(refunds.total_lamports)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
            ClutchError::MismatchedState,
        )?;
    }

    require_direct_candidate_liveness_aliases_v1(
        accounts,
        refund_end,
        base_count,
    )?;
    let runtime_action = match action {
        DirectMarketAction::FinalizeSelection => {
            clutch_direct_market_runtime::DirectMarketActionV1::FinalizeSelection
        }
        DirectMarketAction::SettlePair => {
            clutch_direct_market_runtime::DirectMarketActionV1::SettlePair
        }
        DirectMarketAction::LapseEmpty => {
            clutch_direct_market_runtime::DirectMarketActionV1::LapseEmpty
        }
        DirectMarketAction::LapseUnselected => {
            clutch_direct_market_runtime::DirectMarketActionV1::LapseUnselected
        }
        DirectMarketAction::LapseSelected => {
            clutch_direct_market_runtime::DirectMarketActionV1::LapseSelected
        }
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };
    let bound_replay = apply_direct_candidate_work_v1(
        program_id,
        &accounts[refund_end..],
        &accounts[1],
        &plan.state,
        &plan.selection,
        runtime_action,
    )?;

    index = 0;
    while index < endpoint_count {
        let first = direct_endpoint_first_v1(index)?;
        let endpoint = plan.endpoints[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        write_position_post_v1(&accounts[first + 1], &endpoint.position_poststate)?;
        write_general_replay_post_v1(&accounts[first + 2], &endpoint.replay_transition)?;
        let reservation_data = authenticate_direct_reservation_writable_v1(
            program_id,
            &accounts[first],
            &root,
        )?;
        write_direct_reservation_v1(
            &accounts[first],
            reservation_data.bump(),
            endpoint.reservation_post,
            plan.state.root,
        )?;
        index += 1;
    }
    if let Some(treasury) = plan.treasury {
        require(treasury_meta_required, ClutchError::MismatchedState)?;
        write_position_post_v1(
            &accounts[endpoint_end + 3],
            &treasury.position_poststate,
        )?;
        write_general_replay_post_v1(
            &accounts[endpoint_end + 4],
            &treasury.replay_transition,
        )?;
    }
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        direct_replay.bump(),
        bound_replay,
        plan.state.root,
    )?;
    write_direct_selection_v1(
        &accounts[2],
        selection.bump(),
        plan.selection,
        plan.state.root,
    )
}

fn require_direct_endpoint_alias_contract_v1(
    accounts: &[AccountInfo<'_>],
    fixed_count: usize,
    endpoint_count: usize,
) -> Outcome<()> {
    let mut index = 0usize;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v1(fixed_count, index)?;
        let mut fixed = 0usize;
        while fixed < fixed_count {
            require(
                accounts[first].key != accounts[fixed].key
                    && accounts[first + 1].key != accounts[fixed].key
                    && accounts[first + 2].key != accounts[fixed].key,
                ClutchError::AccountAlias,
            )?;
            fixed += 1;
        }
        require(
            accounts[first].key != accounts[first + 1].key
                && accounts[first].key != accounts[first + 2].key
                && accounts[first + 1].key != accounts[first + 2].key,
            ClutchError::AccountAlias,
        )?;
        index += 1;
    }
    if endpoint_count == 2 {
        let left = fixed_count;
        let right = fixed_count
            .checked_add(3)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
        let positions_alias = accounts[left + 1].key == accounts[right + 1].key;
        let replays_alias = accounts[left + 2].key == accounts[right + 2].key;
        require(positions_alias == replays_alias, ClutchError::AccountAlias)?;
        require(
            accounts[left].key != accounts[right + 1].key
                && accounts[left].key != accounts[right + 2].key
                && accounts[right].key != accounts[left + 1].key
                && accounts[right].key != accounts[left + 2].key
                && accounts[left + 1].key != accounts[right + 2].key
                && accounts[left + 2].key != accounts[right + 1].key,
            ClutchError::AccountAlias,
        )?;
    }
    Ok(())
}

fn require_direct_fee_suffix_alias_contract_v1(
    accounts: &[AccountInfo<'_>],
    endpoint_count: usize,
    fee_suffix_present: bool,
    treasury_present: bool,
) -> Outcome<()> {
    if !fee_suffix_present {
        return Ok(());
    }
    let suffix = endpoint_count
        .checked_mul(3)
        .and_then(|value| value.checked_add(12))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let policy_end = suffix
        .checked_add(3)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let mut policy = suffix;
    while policy < policy_end {
        let mut prior = 0usize;
        while prior < policy {
            require(
                accounts[policy].key != accounts[prior].key,
                ClutchError::AccountAlias,
            )?;
            prior += 1;
        }
        policy += 1;
    }
    if !treasury_present {
        return Ok(());
    }
    let treasury_position = policy_end;
    let treasury_replay = policy_end
        .checked_add(1)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        accounts[treasury_position].key != accounts[treasury_replay].key,
        ClutchError::AccountAlias,
    )?;

    let mut fixed_or_policy = 0usize;
    while fixed_or_policy < policy_end {
        let is_endpoint_member = fixed_or_policy >= 12 && fixed_or_policy < suffix;
        if !is_endpoint_member {
            require(
                accounts[treasury_position].key != accounts[fixed_or_policy].key
                    && accounts[treasury_replay].key != accounts[fixed_or_policy].key,
                ClutchError::AccountAlias,
            )?;
        }
        fixed_or_policy += 1;
    }

    let mut endpoint = 0usize;
    while endpoint < endpoint_count {
        let first = direct_endpoint_first_v1(endpoint)?;
        let position_alias =
            accounts[treasury_position].key == accounts[first + 1].key;
        let replay_alias = accounts[treasury_replay].key == accounts[first + 2].key;
        require(position_alias == replay_alias, ClutchError::AccountAlias)?;
        require(
            accounts[treasury_position].key != accounts[first].key
                && accounts[treasury_position].key != accounts[first + 2].key
                && accounts[treasury_replay].key != accounts[first].key
                && accounts[treasury_replay].key != accounts[first + 1].key,
            ClutchError::AccountAlias,
        )?;
        endpoint += 1;
    }
    Ok(())
}

fn direct_endpoint_first_v1(index: usize) -> Outcome<usize> {
    direct_endpoint_first_from_v1(12, index)
}

fn direct_endpoint_first_from_v1(base: usize, index: usize) -> Outcome<usize> {
    index
        .checked_mul(3)
        .and_then(|offset| offset.checked_add(base))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))
}

fn authenticate_direct_resolution_v5(
    program_id: &Pubkey,
    root: &AuthenticatedDirectMarketRootV1,
    account: &AccountInfo<'_>,
) -> Outcome<DirectFinalResolutionV1> {
    use clutch_collateral_adapter_v2::{ResolutionStateV5, ResolutionV5, RESOLUTION_V5_BYTES};
    require(
        account.owner == program_id
            && !account.is_signer
            && !account.executable
            && !account.is_writable
            && account.data_len() == RESOLUTION_V5_BYTES
            && account.key.to_bytes() == root.value().binding.resolution_account,
        ClutchError::MismatchedState,
    )?;
    let resolution = ResolutionV5::decode(&account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        account.key,
        seeds::resolution_v5_pda(program_id, &root.value().binding.market_instance_id),
        Some(resolution.stored_bump),
    )?;
    let account_id = CollateralId::from_bytes(account.key.to_bytes());
    let semantic_id = resolution
        .semantic_id(&DirectRuntimeSha256V1)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let data_id = resolution
        .data_id(account_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let minimum_balance = resolution.rent
        .refundable_principal()
        .checked_add(resolution.rent.donation_floor())
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        resolution.state == ResolutionStateV5::Finalized
            && resolution.facts.market_instance_id.bytes()
                == root.value().binding.market_instance_id
            && resolution.facts.generation == root.value().binding.generation
            && resolution.facts.outcome_count == root.value().binding.outcome_count
            && account.lamports() >= minimum_balance,
        ClutchError::MismatchedState,
    )?;
    Ok(DirectFinalResolutionV1 {
        account: account.key.to_bytes(),
        semantic_id: semantic_id.bytes(),
        data_id: data_id.bytes(),
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_direct_general_market_v1(
    program_id: &Pubkey,
    root: &AuthenticatedDirectMarketRootV1,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
) -> Outcome<BoundCollateralProfileV2> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let (market_binding, market_runtime) = authenticate_general_market_v3(
        program_id,
        market_binding_account,
        market_runtime_account,
    )?;
    let market = market_binding.base().base();
    let market_instance = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        market.market_instance_v2_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        market_instance.value().market_genesis_profile_id.content_id(),
    )?;
    let binding = root.value().binding;
    let release_id = realm
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        market.market_instance_v2_id.bytes() == binding.market_instance_id
            && market.outcome_count == binding.outcome_count
            && market.series_plan_v5_id.bytes() == binding.founder_series_plan_id
            && market.relation_policy_id.bytes() == binding.relation_policy_id
            && market.price_measure_policy_v1_id.bytes() == binding.price_policy_id
            && market.neutral_sink.bytes() == binding.neutral_lamport_sink
            && market.price_scale == binding.price_scale
            && genesis.value().realm_id.bytes() == binding.realm_id
            && genesis.value().profile_id.bytes() == binding.collateral_profile_id
            && genesis.value().relation_policy_id.bytes() == binding.relation_policy_id
            && genesis.value().fee_policy_id.bytes() == binding.revenue_policy_id
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == binding.price_policy_id
            && realm.policy_id().bytes() == binding.collateral_policy_id
            && release_id.bytes() == binding.collateral_release_id
            && market_runtime_account.key.to_bytes() == binding.general_market_runtime
            && market_binding_account.key.to_bytes() == binding.general_market_binding
            && market_runtime.market_instance_v2_id == market.market_instance_v2_id
            && market_binding.base().batch_policy_id().bytes() == binding.batch_policy_id
            && market_binding.product_market_root_account().bytes()
                == binding.product_root_account
            && market_binding.product_market_binding_id().bytes()
                == binding.product_market_binding_id
            && market_binding.product_preauthorization_id().bytes()
                == binding.general_product_preauthorization_id
            && market_binding.product_generation() == binding.generation
            && market_binding.series_market_link_account().bytes()
                == binding.founder_series_link_account
            && market_binding.series_ordinal() == binding.founder_series_ordinal
            && market_binding.compiler_bundle_v5_id().bytes()
                == binding.compiler_bundle_v5_id
            && market.series_plan_v5_id.bytes() == binding.founder_series_plan_id
            && market_instance.value().id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes() == binding.market_instance_id,
        ClutchError::MismatchedState,
    )?;
    let market_bytes = binding.market_instance_id;
    refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market_bytes),
            realm: CollateralId::from_bytes(binding.realm_id),
            profile: CollateralId::from_bytes(binding.collateral_profile_id),
            collateral_cap_atoms: market_instance.value().collateral_cap,
            hoard_authority: CollateralId::from_bytes(
                seeds::hoard_authority_v2_pda(program_id, &market_bytes).0.to_bytes(),
            ),
            hoard_token_account: CollateralId::from_bytes(
                seeds::hoard_token_v2_pda(program_id, &market_bytes).0.to_bytes(),
            ),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn authenticate_direct_order_limit_v1(
    program_id: &Pubkey,
    root: &AuthenticatedDirectMarketRootV1,
    bundle_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    price_grid_account: &AccountInfo<'_>,
    limit: u128,
) -> Outcome<()> {
    let binding = root.value().binding;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        bundle_account,
        ContentId::from_bytes(binding.compiler_bundle_v5_id),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle.value().market_genesis_profile_id.content_id(),
    )?;
    require(
        genesis.value().realm_id.bytes() == binding.realm_id
            && genesis.value().profile_id.bytes() == binding.collateral_profile_id
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == binding.price_policy_id,
        ClutchError::MismatchedState,
    )?;
    require(
        price_grid_account.owner == program_id
            && !price_grid_account.is_signer
            && !price_grid_account.executable
            && !price_grid_account.is_writable
            && price_grid_account.data_len() == account_len::PRICE_GRID,
        ClutchError::MismatchedState,
    )?;
    let grid = PriceGridAccount::decode(&price_grid_account.data.borrow())?;
    expect_pda(
        price_grid_account.key,
        seeds::grid_pda(program_id, &grid.realm.0, &grid.grid.0),
        Some(grid.stored_bump),
    )?;
    let limit = u64::try_from(limit)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    grid.tick_of(limit)?;
    require(
        grid.realm.0 == binding.realm_id
            && grid.grid.0 == genesis.value().price_grid_id.bytes()
            && grid.price_scale == binding.price_scale,
        ClutchError::MismatchedState,
    )
}

fn authenticate_direct_price_grid_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_grid_id: ContentId,
    expected_realm_id: ContentId,
) -> Outcome<PriceGridAccount> {
    require(
        account.owner == program_id
            && !account.is_signer
            && !account.executable
            && !account.is_writable
            && account.data_len() == account_len::PRICE_GRID,
        ClutchError::MismatchedState,
    )?;
    let grid = PriceGridAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::grid_pda(program_id, &grid.realm.0, &grid.grid.0),
        Some(grid.stored_bump),
    )?;
    require(
        grid.grid.0 == expected_grid_id.bytes()
            && grid.realm.0 == expected_realm_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(grid)
}

fn write_product_root_post_v1(
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedMarketLifecycleRootV1<'_>,
    successor: &clutch_product_series::MarketLifecycleRootV1,
) -> Outcome<()> {
    require(
        account.is_writable
            && *account.key == authenticated.account()
            && successor.binding() == authenticated.state().binding(),
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV1::encode_parts(
        successor,
        authenticated.rent_principal_lamports(),
        authenticated.value().stored_bump,
        &mut data,
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectAccountAccessV1 { ReadOnly, Writable }

impl DirectAccountAccessV1 {
    const fn writable(self) -> bool { matches!(self, Self::Writable) }
}

#[inline(never)]
fn authenticate_root_with_access_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    access: DirectAccountAccessV1,
) -> Outcome<AuthenticatedDirectMarketRootV1> {
    require_program_state_v1(program_id, account, access, DIRECT_MARKET_ROOT_ACCOUNT_BYTES)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_MARKET_ROOT_ACCOUNT_BYTES>(&data)?;
    let frame = DirectMarketRootAccountV1::decode(&bytes)?;
    let value = decode_direct_market_root_body_v1(frame.semantic_body())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_market_root_v1_pda(
        program_id,
        &value.binding.market_instance_id,
        value.binding.generation,
    );
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.binding.direct_root_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.root_rent.principal_lamports,
        value.root_rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(&DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    require_live_id_v1(data_id)?;
    require_live_id_v1(semantic_id)?;
    Ok(AuthenticatedDirectMarketRootV1 {
        account: *account.key, value: Box::new(value), bump, data_id, semantic_id,
        observed_lamports,
    })
}

pub(crate) fn authenticate_direct_market_root_readonly_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedDirectMarketRootV1> {
    authenticate_root_with_access_v1(program_id, account, DirectAccountAccessV1::ReadOnly)
}

pub(crate) fn authenticate_direct_market_root_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedDirectMarketRootV1> {
    authenticate_root_with_access_v1(program_id, account, DirectAccountAccessV1::Writable)
}

#[inline(never)]
pub(crate) fn authenticate_direct_action_replay_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectActionReplayV1> {
    require_program_state_v1(
        program_id,
        account,
        DirectAccountAccessV1::Writable,
        DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_ACTION_REPLAY_ACCOUNT_BYTES>(&data)?;
    let frame = DirectActionReplayAccountV1::decode(&bytes)?;
    let value = decode_direct_action_replay_body_v1(frame.semantic_body(), root.value())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_action_replay_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.replay_account == account.key.to_bytes()
            && value.direct_root_account == root.account().to_bytes()
            && root.value().binding.action_replay_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.rent.principal_lamports,
        value.rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(root.value(), &DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    Ok(AuthenticatedDirectActionReplayV1 {
        account: *account.key, value: Box::new(value), bump, data_id, semantic_id,
        observed_lamports,
    })
}

#[inline(never)]
fn authenticate_selection_with_access_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV1,
    access: DirectAccountAccessV1,
) -> Outcome<AuthenticatedDirectSelectionV1> {
    require_program_state_v1(program_id, account, access, DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_SELECTION_ACCOUNT_BYTES>(&data)?;
    let frame = DirectSelectionAccountV1::decode(&bytes)?;
    let value = decode_direct_selection_body_v1(frame.semantic_body(), root.value())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_selection_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.account() == account.key.to_bytes()
            && root.value().selection_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.rent().principal_lamports,
        value.rent().donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(root.value(), &DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    Ok(AuthenticatedDirectSelectionV1 {
        account: *account.key, value: Box::new(value), bump, data_id, semantic_id,
        observed_lamports,
    })
}

pub(crate) fn authenticate_direct_selection_readonly_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectSelectionV1> {
    authenticate_selection_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::ReadOnly,
    )
}

pub(crate) fn authenticate_direct_selection_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectSelectionV1> {
    authenticate_selection_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::Writable,
    )
}

#[inline(never)]
fn authenticate_reservation_with_access_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV1,
    access: DirectAccountAccessV1,
) -> Outcome<AuthenticatedDirectReservationV1> {
    require_program_state_v1(program_id, account, access, DIRECT_RESERVATION_ACCOUNT_BYTES)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_RESERVATION_ACCOUNT_BYTES>(&data)?;
    let frame = DirectReservationAccountV1::decode(&bytes)?;
    let value = decode_direct_reservation_body_v1(frame.semantic_body(), root.value())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_reservation_v1_pda(
        program_id,
        &root.account(),
        &value.order_id,
    );
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.reservation_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.rent.principal_lamports,
        value.rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(&DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    Ok(AuthenticatedDirectReservationV1 {
        account: *account.key, value, bump, data_id, semantic_id, observed_lamports,
    })
}

pub(crate) fn authenticate_direct_reservation_readonly_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectReservationV1> {
    authenticate_reservation_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::ReadOnly,
    )
}

pub(crate) fn authenticate_direct_reservation_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectReservationV1> {
    authenticate_reservation_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::Writable,
    )
}

/// Authenticate a fresh System-owned, zero-data current Direct PDA and return
/// its hostile prefund donation floor. The caller still must calculate rent,
/// fund principal without discounting the prefund, allocate, assign, and write.
pub(crate) fn authenticate_fresh_direct_pda_v1(
    account: &AccountInfo<'_>,
    expected: (Pubkey, u8),
) -> Outcome<(u8, u64)> {
    expect_pda(account.key, expected, None)?;
    require(
        !account.is_signer
            && account.is_writable
            && !account.executable
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.data_len() == 0,
        ClutchError::AlreadyInitialized,
    )?;
    Ok((expected.1, account.lamports()))
}

pub(crate) fn write_direct_market_root_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_market_root_body_v1(value).map_err(map_direct_error_v1)?;
    let frame = DirectMarketRootAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_MARKET_ROOT_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

pub(crate) fn write_direct_action_replay_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectActionReplayV1,
    root: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_action_replay_body_v1(value, root).map_err(map_direct_error_v1)?;
    let frame = DirectActionReplayAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

pub(crate) fn write_direct_selection_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectSelectionV1,
    root: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_selection_body_v1(value, root).map_err(map_direct_error_v1)?;
    let frame = DirectSelectionAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_SELECTION_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

pub(crate) fn write_direct_reservation_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectReservationV1,
    root: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_reservation_body_v1(value, root).map_err(map_direct_error_v1)?;
    let frame = DirectReservationAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_RESERVATION_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

fn require_program_state_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    access: DirectAccountAccessV1,
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

fn require_rent_coverage_v1(
    principal: u64,
    donation_floor: u64,
    observed: u64,
) -> Outcome<()> {
    let floor = principal
        .checked_add(donation_floor)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(principal != 0 && observed >= floor, ClutchError::MismatchedState)
}

fn require_live_id_v1(id: [u8; 32]) -> Outcome<()> {
    require(id != [0; 32], ClutchError::MismatchedState)
}

fn exact_array<const N: usize>(input: &[u8]) -> Outcome<[u8; N]> {
    require(input.len() == N, ClutchError::WrongDataLength)?;
    let mut output = [0u8; N];
    output.copy_from_slice(input);
    Ok(output)
}

fn write_exact_v1<const N: usize>(account: &AccountInfo<'_>, value: &[u8; N]) -> Outcome<()> {
    require(
        account.is_writable && !account.executable && account.data_len() == N,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(value);
    Ok(())
}

#[inline(never)]
fn write_position_post_v1(
    account: &AccountInfo<'_>,
    post: &PositionSettlementPoststateV3,
) -> Outcome<()> {
    let body = post
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact_v1(account, &body)
}

#[inline(never)]
fn write_general_replay_post_v1(
    account: &AccountInfo<'_>,
    post: &GeneralReplayTransitionPlanV1,
) -> Outcome<()> {
    let body = post.replay_poststate_body();
    require(
        account.is_writable && account.data_len() == body.len(),
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(body);
    Ok(())
}

fn credit_lamports_v1(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_add(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn debit_lamports_v1(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_sub(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn transfer_signer_lamports_v1<'a>(
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

fn close_direct_program_account_v1(
    account: &AccountInfo<'_>,
    expected_lamports: u64,
) -> Outcome<()> {
    require(
        account.is_writable && account.lamports() == expected_lamports,
        ClutchError::MismatchedState,
    )?;
    {
        let mut lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = 0;
    }
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(
        account.lamports() == 0
            && account.data_len() == 0
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

/// Fund the exact persisted principal in full even when a hostile caller
/// prefunded the PDA, then allocate and assign without changing that donation.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn create_current_direct_account_v1<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    principal_lamports: u64,
    donation_floor_lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require_system_program(system_program)?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE
            && principal_lamports == rent.minimum_balance(space)?
            && target.lamports() == donation_floor_lamports,
        ClutchError::MismatchedState,
    )?;
    let expected_balance = donation_floor_lamports
        .checked_add(principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal_lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, false)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, false)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner == program_id
            && target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )
}

fn map_direct_error_v1(error: DirectMarketErrorV1) -> Refusal {
    let adapter = match error {
        DirectMarketErrorV1::Arithmetic => ClutchError::Arithmetic,
        DirectMarketErrorV1::Replay => ClutchError::Replay,
        DirectMarketErrorV1::WrongPhase => ClutchError::NotActive,
        DirectMarketErrorV1::UnauthenticatedAuthority => ClutchError::AuthorizationUnavailable,
        _ => ClutchError::MismatchedState,
    };
    Refusal::Adapter(adapter)
}

fn direct_account_authentication_id_v1(
    account: [u8; 32],
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V1,
        &account,
        &data_id,
        &semantic_id,
        &observed_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_authentication_transcript_is_role_sensitive() {
        let root = direct_account_authentication_id_v1([1; 32], [2; 32], [3; 32], 4);
        let changed_account = direct_account_authentication_id_v1([9; 32], [2; 32], [3; 32], 4);
        let changed_data = direct_account_authentication_id_v1([1; 32], [9; 32], [3; 32], 4);
        let changed_semantic = direct_account_authentication_id_v1([1; 32], [2; 32], [9; 32], 4);
        let changed_lamports = direct_account_authentication_id_v1([1; 32], [2; 32], [3; 32], 9);
        assert_ne!(root, changed_account);
        assert_ne!(root, changed_data);
        assert_ne!(root, changed_semantic);
        assert_ne!(root, changed_lamports);
    }

    #[test]
    fn current_family_frames_are_statically_bounded() {
        assert_eq!(
            12 + 2 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1,
            18,
        );
        assert!(18 <= DIRECT_MARKET_V1_MAX_ACCOUNTS);
        assert_eq!(4 + 3 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1, 11);
        assert_eq!(12 + 2 * 3 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1, 22);
        assert!(22 <= DIRECT_MARKET_V1_MAX_ACCOUNTS);
        assert_eq!(direct_endpoint_first_from_v1(12, 0).unwrap(), 12);
        assert_eq!(direct_endpoint_first_from_v1(12, 1).unwrap(), 15);
        assert_eq!(direct_endpoint_first_from_v1(19, 0).unwrap(), 19);
        assert_eq!(direct_endpoint_first_from_v1(19, 1).unwrap(), 22);
        assert_eq!(
            19 + 2 * 3 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1,
            29,
        );
        assert_eq!(
            12 + 2 * 3 + 3 + 2 + 3 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V1,
            DIRECT_MARKET_V1_MAX_ACCOUNTS,
        );
        assert_eq!(18 + 3, 21);
        assert!(21 <= DIRECT_MARKET_V1_MAX_ACCOUNTS);
        assert_eq!(9 + 2 + 5, DIRECT_RETIRE_TERMINAL_MAX_ACCOUNTS);
        assert_eq!(
            clutch_solana_layout::direct_market_v1::DIRECT_ADMIT_ORDER_PAYLOAD_BYTES_V1,
            DIRECT_MARKET_V1_MAX_PAYLOAD_BYTES,
        );
    }
}
