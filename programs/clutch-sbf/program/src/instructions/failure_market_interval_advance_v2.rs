// SPDX-License-Identifier: AGPL-3.0-or-later
//! Atomic Product progress, Recovery payment, and Failure cell advance.
//!
//! This capability-disabled module is the only Active-to-Active writer. The
//! existing liveness Recovery compartment is the sole work custodian: Failure
//! neither holds nor debits a second reserve. Runtime payment is applied and
//! hostile-reauthenticated before the exact cell postimage is written last;
//! any later refusal rolls the whole SVM instruction back.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::AuthenticatedFailureMarketRootV2;
use crate::instructions::failure_market_interval_v2::{
    write_failure_market_interval_paid_advance_v2, AuthenticatedFailureMarketIntervalAccountsV2,
    AuthenticatedFailureMarketIntervalPaidAdvanceV2,
};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::instructions::product_failure_begin::{
    require_cached_root_and_link, require_exact_successful_source_join_v2,
};
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    AuthenticatedMarketLifecycleRootV1, AuthenticatedSeriesMarketLinkV1,
};
use crate::seeds;
use crate::source_plane_v3_actions::SourcePolicyHandoffJoinV1;
use clutch_failure_policy_runtime::market_interval_cell_v2::{
    plan_advance_failure_market_interval_cell_v2, AuthenticatedFailureMarketIntervalCellAdvanceV2,
    FailureMarketIntervalCellAdvanceFactsV2, FailureMarketIntervalCellAdvanceReceiptV2,
};
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, plan_runtime_transition_v1, RuntimeAtomicTransitionV1,
    RuntimePersistedAccountViewV1, RuntimeReceiptKindV1, RuntimeReceiptObservationV1,
    RuntimeTransferRoleV1, RuntimeTransitionActionV1, RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentV1, RuntimeLivenessPolicyV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    ContentId, MarketLifecyclePhaseV1, QuantizedIntervalConsensusContextV1, SeriesMarketLinkPhaseV1,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FAILURE_ACCOUNT_HEADER_BYTES_V1,
    FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1, FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1, FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use clutch_solana_layout::registry;
use clutch_source_plane_v3_runtime::SuccessfulEvaluationHandoffV1;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const FAILURE_MARKET_PAID_ADVANCE_PREAUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-paid-advance-preauthorization/v2\0";
const FAILURE_MARKET_PAID_ADVANCE_RUNTIME_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-paid-advance-runtime-postwrite/v2\0";
const FAILURE_MARKET_PAID_ADVANCE_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-paid-advance-postwrite/v2\0";

/// Private exact Recovery prestate admitted to the pure Failure plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketPaidAdvancePreauthorizationV2 {
    id: ContentId,
    cell_state_id:
        clutch_failure_policy_runtime::market_interval_cell_v2::FailureMarketIntervalCellStateIdV2,
    history_state_id: clutch_failure_policy_runtime::market_interval_history_v2::FailureMarketIntervalHistoryStateIdV2,
    product_work_before: clutch_product_series::QuantizedIntervalConsensusWorkV1Id,
    attempt_index: u8,
    completed_calls_before: u32,
    maximum_lamports_per_call: u64,
    remaining_work_lamports: u64,
    keeper: LivenessId,
}

impl AuthenticatedFailureMarketIntervalCellAdvanceV2
    for FailureMarketPaidAdvancePreauthorizationV2
{
    fn authenticate_failure_market_interval_cell_advance(
        &self,
        expected: FailureMarketIntervalCellAdvanceFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let expected_ordinal = self
            .completed_calls_before
            .checked_add(1)
            .ok_or(clutch_failure_policy_runtime::Error::BindingMismatch)?;
        if self.id.is_zero()
            || expected.cell_before != self.cell_state_id
            || expected.history_state != self.history_state_id
            || expected.product_work_before != self.product_work_before
            || expected.attempt_index != self.attempt_index
            || expected.call_ordinal != expected_ordinal
            || expected.reward_recipient != self.keeper
            || expected.processed_coordinates == 0
            || expected.accepted_progress_after <= expected.accepted_progress_before
            || expected.exact_reward_lamports == 0
            || expected.exact_reward_lamports > self.maximum_lamports_per_call
            || expected.exact_reward_lamports > self.remaining_work_lamports
            || expected.runtime_work_receipt_id.bytes() != expected.work_authorization_id.bytes()
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Hostile-reauthenticated Recovery compartment payment postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedFailureRecoveryPaidAdvanceV2 {
    id: ContentId,
    intent: RuntimeTransitionIntentV1,
    observation: RuntimeReceiptObservationV1,
    transition: RuntimeAtomicTransitionV1,
    recovery_data_before_id: ContentId,
    recovery_data_after_id: ContentId,
    keeper_balance_before: u64,
    keeper_balance_after: u64,
}

/// Exact intent/receipt facts admitted to the sole Recovery transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureRecoveryTransitionExpectationV2 {
    policy_id: LivenessId,
    lifecycle_id: LivenessId,
    account_id: LivenessId,
    semantic_owner: LivenessId,
    quote_schedule_id: LivenessId,
    receipt_account_id: LivenessId,
    receipt_program_id: LivenessId,
    receipt_id: LivenessId,
    keeper: LivenessId,
    generation: u64,
    call_ordinal: u32,
    exact_reward_lamports: u64,
}

/// Private liveness postwrite authority accepted by the narrow cell writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketPaidAdvanceCellWriteV2 {
    runtime: AuthenticatedFailureRecoveryPaidAdvanceV2,
    advance: FailureMarketIntervalCellAdvanceReceiptV2,
}

impl AuthenticatedFailureMarketIntervalPaidAdvanceV2 for FailureMarketPaidAdvanceCellWriteV2 {
    fn authenticate_failure_market_interval_paid_advance_v2(
        &self,
        expected: FailureMarketIntervalCellAdvanceReceiptV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        let facts = expected.facts();
        if expected != self.advance
            || self.runtime.id.is_zero()
            || self.runtime.intent.receipt_id != facts.runtime_work_receipt_id
            || self.runtime.intent.keeper != facts.reward_recipient
            || self.runtime.intent.call_ordinal != facts.call_ordinal
            || self.runtime.intent.call_ceiling_lamports != facts.exact_reward_lamports
            || self.runtime.intent.keeper_payment_lamports != facts.exact_reward_lamports
            || self.runtime.transition.state_after.last_work_receipt_id
                != facts.runtime_work_receipt_id
        {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Complete exact Recovery-payment plus Active-cell postwrite receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFailureMarketPaidAdvancePostwriteV2 {
    id: ContentId,
    advance: FailureMarketIntervalCellAdvanceReceiptV2,
    runtime_postwrite_id: ContentId,
    cell_authentication_before: ContentId,
    cell_authentication_after: ContentId,
    recovery_data_before_id: ContentId,
    recovery_data_after_id: ContentId,
}

impl AuthenticatedFailureMarketPaidAdvancePostwriteV2 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn advance(self) -> FailureMarketIntervalCellAdvanceReceiptV2 {
        self.advance
    }

    pub(crate) const fn runtime_postwrite_id(self) -> ContentId {
        self.runtime_postwrite_id
    }

    pub(crate) const fn cell_authentication_before(self) -> ContentId {
        self.cell_authentication_before
    }

    pub(crate) const fn cell_authentication_after(self) -> ContentId {
        self.cell_authentication_after
    }

    pub(crate) const fn recovery_data_before_id(self) -> ContentId {
        self.recovery_data_before_id
    }

    pub(crate) const fn recovery_data_after_id(self) -> ContentId {
        self.recovery_data_after_id
    }
}

/// Apply one exact priced Product advance through the sole Recovery custody.
#[allow(clippy::too_many_arguments)]
pub(crate) fn advance_failure_market_interval_paid_v2<'root, 'link>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    cell_account: &AccountInfo<'_>,
    history_account: &AccountInfo<'_>,
    liveness_policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    root_before: AuthenticatedMarketLifecycleRootV1<'_>,
    link_before: AuthenticatedSeriesMarketLinkV1<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval_before: AuthenticatedFailureMarketIntervalAccountsV2,
    source_join: SourcePolicyHandoffJoinV1,
    source_success: SuccessfulEvaluationHandoffV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
    requested_coordinates: u16,
    root_decode: &'root mut MarketLifecycleRootAccountV1,
    link_decode: &'link mut SeriesMarketLinkAccountV1,
) -> Outcome<(
    AuthenticatedFailureMarketPaidAdvancePostwriteV2,
    AuthenticatedFailureMarketIntervalAccountsV2,
)> {
    require_distinct_paid_advance_accounts_v2(
        root_account,
        link_account,
        cell_account,
        history_account,
        liveness_policy_account,
        recovery_account,
        keeper,
        payer_refund,
        admission,
        source_join,
    )?;
    let root_binding = root_before.state().binding();
    let link_binding = link_before.state().binding();
    let live_root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        root_binding.market_instance_id,
        root_binding.generation,
        false,
        root_decode,
    )?;
    let live_link = authenticate_series_market_link_v1(
        program_id,
        link_account,
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        *root_account.key,
        false,
        link_decode,
    )?;
    require_cached_root_and_link(root_before, live_root, link_before, live_link)?;
    let policy = admission.state().binding().facts();
    require(
        !root_account.is_writable
            && !link_account.is_writable
            && cell_account.is_writable
            && !history_account.is_writable
            && *cell_account.key == interval_before.cell_account()
            && *history_account.key == interval_before.history_account()
            && interval_before.admission_root_account() == admission.account()
            && live_root.state().phase() == MarketLifecyclePhaseV1::Active
            && live_root.state().resolution_semantic_id() == ContentId::ZERO
            && live_root.state().resolution_data_id() == ContentId::ZERO
            && live_root.state().resolution_activation_receipt_id() == ContentId::ZERO
            && live_link.state().phase() == SeriesMarketLinkPhaseV1::Active
            && live_link.state().active_failure_sessions() == 1
            && live_link.state().failure_session_transcript_id().bytes()
                == interval_before.cell().session_binding_id().bytes()
            && interval_before.cell().market_instance_id() == root_binding.market_instance_id
            && interval_before.cell().generation() == root_binding.generation
            && interval_before.cell().failure_policy_binding_id().bytes()
                == root_binding.market_failure_policy_binding_id.bytes()
            && admission.state().binding().id().bytes()
                == root_binding.market_failure_policy_binding_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require_exact_successful_source_join_v2(
        source_join,
        source_success,
        link_binding,
        root_binding,
        policy,
    )?;
    let liveness = authenticate_failure_recovery_liveness_prestate_v2(
        program_id,
        liveness_policy_account,
        recovery_account,
        payer_refund,
        admission,
        interval_before,
    )?;
    require_system_recipient(keeper, true)?;
    let current_work = interval_before
        .cell()
        .product_work()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    let preauthorization_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_PAID_ADVANCE_PREAUTHORIZATION_DOMAIN_V2,
            &root_before.authentication_id().bytes(),
            &link_before.authentication_id().bytes(),
            &interval_before.cell_authentication_id().bytes(),
            &interval_before.history_authentication_id().bytes(),
            &liveness.policy.policy_id.bytes(),
            &liveness.recovery.identity.account_id.bytes(),
            &liveness.recovery.last_work_receipt_id.bytes(),
            &liveness.recovery.completed_calls.to_le_bytes(),
            keeper.key.as_ref(),
            &requested_coordinates.to_le_bytes(),
            &source_join.id().bytes(),
            &source_success.id().bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(preauthorization_id)?;
    let preauthorization = FailureMarketPaidAdvancePreauthorizationV2 {
        id: preauthorization_id,
        cell_state_id: interval_before.cell_state_id(),
        history_state_id: interval_before.history_state_id(),
        product_work_before: current_work
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        attempt_index: interval_before.cell().attempt_index(),
        completed_calls_before: liveness.recovery.completed_calls,
        maximum_lamports_per_call: liveness.recovery.maximum_lamports_per_call,
        remaining_work_lamports: liveness.recovery.remaining_work_lamports,
        keeper: liveness_id(keeper.key),
    };
    let advance = plan_advance_failure_market_interval_cell_v2(
        &preauthorization,
        interval_before.cell(),
        admission.state(),
        interval_before.funding(),
        interval_before.history(),
        interval_before.quote(),
        source_success,
        context,
        requested_coordinates,
        liveness_id(keeper.key),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let advance_receipt = advance.receipt();
    let runtime = apply_failure_recovery_paid_advance_v2(
        program_id,
        liveness_policy_account,
        recovery_account,
        keeper,
        payer_refund,
        cell_account,
        liveness,
        advance_receipt,
    )?;
    let cell_authentication_before = interval_before.cell_authentication_id();
    let interval_after = write_failure_market_interval_paid_advance_v2(
        program_id,
        cell_account,
        history_account,
        admission,
        interval_before,
        advance,
        &FailureMarketPaidAdvanceCellWriteV2 {
            runtime,
            advance: advance_receipt,
        },
    )?;
    let cell_authentication_after = interval_after.cell_authentication_id();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_PAID_ADVANCE_POSTWRITE_DOMAIN_V2,
            &preauthorization_id.bytes(),
            &advance_receipt.id().bytes(),
            &runtime.id.bytes(),
            &cell_authentication_before.bytes(),
            &cell_authentication_after.bytes(),
            &runtime.recovery_data_before_id.bytes(),
            &runtime.recovery_data_after_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok((
        AuthenticatedFailureMarketPaidAdvancePostwriteV2 {
            id,
            advance: advance_receipt,
            runtime_postwrite_id: runtime.id,
            cell_authentication_before,
            cell_authentication_after,
            recovery_data_before_id: runtime.recovery_data_before_id,
            recovery_data_after_id: runtime.recovery_data_after_id,
        },
        interval_after,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedFailureRecoveryLivenessPrestateV2 {
    policy: RuntimeLivenessPolicyV1,
    recovery: RuntimeCompartmentV1,
    recovery_stored_bump: u8,
    recovery_data_id: ContentId,
    recovery_balance: u64,
}

fn authenticate_failure_recovery_liveness_prestate_v2(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
) -> Outcome<AuthenticatedFailureRecoveryLivenessPrestateV2> {
    require(
        policy_account.owner == program_id
            && !policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && policy_account.data_len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1
            && recovery_account.owner == program_id
            && recovery_account.is_writable
            && !recovery_account.is_signer
            && !recovery_account.executable
            && recovery_account.data_len() == FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    require_system_recipient(payer_refund, payer_refund.key == keeper.key)?;
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_frame = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let recovery_frame = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let policy = decode_runtime_policy_account_v1(
        liveness_id(program_id),
        liveness_id(policy_account.key),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(policy_account.key),
            owner_program_id: liveness_id(policy_account.owner),
            lamports: policy_account.lamports(),
            data: policy_frame.body,
            writable: false,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recovery = RuntimeCompartmentV1::decode(recovery_frame.body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    recovery
        .validate_against_policy(policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = admission.state().binding().facts();
    let quote = interval.quote().facts();
    require(
        recovery.kind == RuntimeCompartmentKindV1::Recovery
            && recovery.identity.owner == liveness_id(program_id)
            && recovery.identity.account_id == liveness_id(recovery_account.key)
            && recovery.identity.account_id == facts.recovery_compartment_account_id
            && recovery.identity.policy_id == policy.policy_id
            && policy.policy_id == facts.liveness_policy_id
            && recovery.identity.lifecycle_id == facts.liveness_lifecycle_id
            && recovery.identity.generation == facts.generation
            && recovery.identity.payer == facts.recovery_refund_owner
            && recovery.identity.payer == liveness_id(payer_refund.key)
            && recovery.identity.neutral_sink == facts.neutral_sink
            && recovery.quote_schedule_id == facts.recovery_quote_schedule_id
            && recovery.quote_schedule_id.bytes() == quote.quote_schedule_id.bytes()
            && recovery.receipt_program_id == facts.recovery_receipt_program_id
            && recovery.receipt_program_id == liveness_id(program_id)
            && recovery.maximum_calls == quote.maximum_calls
            && recovery.maximum_lamports_per_call == quote.maximum_lamports_per_call
            && recovery.capitalized_work_lamports == quote.work_principal_lamports
            && recovery_account.lamports()
                >= recovery
                    .expected_account_balance_lamports()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &policy.policy_id.bytes()),
        Some(policy_frame.stored_bump),
    )?;
    expect_pda(
        recovery_account.key,
        seeds::failure_external_recovery_pda(
            program_id,
            &recovery.identity.lifecycle_id.bytes(),
            recovery.identity.generation,
        ),
        Some(recovery_frame.stored_bump),
    )?;
    let recovery_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&recovery_data[..]]).to_bytes());
    require_live_content_id(recovery_data_id)?;
    Ok(AuthenticatedFailureRecoveryLivenessPrestateV2 {
        policy,
        recovery,
        recovery_stored_bump: recovery_frame.stored_bump,
        recovery_data_id,
        recovery_balance: recovery_account.lamports(),
    })
}

#[allow(clippy::too_many_arguments)]
fn apply_failure_recovery_paid_advance_v2(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    receipt_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureRecoveryLivenessPrestateV2,
    advance: FailureMarketIntervalCellAdvanceReceiptV2,
) -> Outcome<AuthenticatedFailureRecoveryPaidAdvanceV2> {
    let facts = advance.facts();
    let intent = RuntimeTransitionIntentV1 {
        action: RuntimeTransitionActionV1::SpendWork,
        kind: RuntimeCompartmentKindV1::Recovery,
        policy_id: authenticated.policy.policy_id,
        lifecycle_id: authenticated.recovery.identity.lifecycle_id,
        account_id: authenticated.recovery.identity.account_id,
        semantic_owner: authenticated.recovery.identity.owner,
        quote_schedule_id: authenticated.recovery.quote_schedule_id,
        receipt_id: facts.runtime_work_receipt_id,
        keeper: facts.reward_recipient,
        generation: authenticated.recovery.identity.generation,
        call_ordinal: facts.call_ordinal,
        call_ceiling_lamports: facts.exact_reward_lamports,
        keeper_payment_lamports: facts.exact_reward_lamports,
        flags: 0,
    };
    intent
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observation = RuntimeReceiptObservationV1 {
        receipt_account_id: liveness_id(receipt_account.key),
        receipt_account_owner_program_id: liveness_id(program_id),
        receipt_id: facts.runtime_work_receipt_id,
        receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
        compartment_kind: RuntimeCompartmentKindV1::Recovery,
        semantic_owner: authenticated.recovery.identity.owner,
        lifecycle_id: authenticated.recovery.identity.lifecycle_id,
        quote_schedule_id: authenticated.recovery.quote_schedule_id,
        generation: authenticated.recovery.identity.generation,
        call_ordinal: facts.call_ordinal,
        call_ceiling_lamports: facts.exact_reward_lamports,
    };
    require_exact_recovery_intent_and_observation_v2(
        intent,
        observation,
        FailureRecoveryTransitionExpectationV2 {
            policy_id: authenticated.policy.policy_id,
            lifecycle_id: authenticated.recovery.identity.lifecycle_id,
            account_id: authenticated.recovery.identity.account_id,
            semantic_owner: authenticated.recovery.identity.owner,
            quote_schedule_id: authenticated.recovery.quote_schedule_id,
            receipt_account_id: liveness_id(receipt_account.key),
            receipt_program_id: liveness_id(program_id),
            receipt_id: facts.runtime_work_receipt_id,
            keeper: facts.reward_recipient,
            generation: authenticated.recovery.identity.generation,
            call_ordinal: facts.call_ordinal,
            exact_reward_lamports: facts.exact_reward_lamports,
        },
    )?;
    let balance_after = authenticated
        .recovery_balance
        .checked_sub(facts.exact_reward_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_frame = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let recovery_frame = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    require(
        recovery_frame.stored_bump == authenticated.recovery_stored_bump
            && RuntimeCompartmentV1::decode(recovery_frame.body)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == authenticated.recovery
            && recovery_account.lamports() == authenticated.recovery_balance,
        ClutchError::MismatchedState,
    )?;
    let transition = plan_runtime_transition_v1(
        liveness_id(program_id),
        liveness_id(policy_account.key),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(policy_account.key),
            owner_program_id: liveness_id(policy_account.owner),
            lamports: policy_account.lamports(),
            data: policy_frame.body,
            writable: false,
        },
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(recovery_account.key),
            owner_program_id: liveness_id(recovery_account.owner),
            lamports: authenticated.recovery_balance,
            data: recovery_frame.body,
            writable: true,
        },
        intent,
        Some(observation),
        balance_after,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(recovery_data);
    drop(policy_data);
    require_exact_recovery_transition_v2(
        transition,
        authenticated,
        facts,
        keeper,
        payer_refund,
        balance_after,
    )?;
    let keeper_balance_before = keeper.lamports();
    let keeper_balance_after = keeper_balance_before
        .checked_add(facts.exact_reward_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    {
        let mut recovery_data = recovery_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let live_frame = decode_failure_account_body_v1(
            &recovery_data,
            registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
            registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
            FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
        )?;
        require(
            live_frame.stored_bump == authenticated.recovery_stored_bump
                && ContentId::from_bytes(
                    solana_sha256_hasher::hashv(&[&recovery_data[..]]).to_bytes(),
                ) == authenticated.recovery_data_id,
            ClutchError::MismatchedState,
        )?;
        recovery_data[FAILURE_ACCOUNT_HEADER_BYTES_V1..]
            .copy_from_slice(&transition.post_account_data);
    }
    {
        let mut recovery_lamports = recovery_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut keeper_lamports = keeper
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **recovery_lamports = balance_after;
        **keeper_lamports = keeper_balance_after;
    }
    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_frame = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let recovery_after = RuntimeCompartmentV1::decode(recovery_frame.body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recovery_data_after_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&recovery_data[..]]).to_bytes());
    require(
        recovery_after == transition.state_after
            && recovery_data_after_id != authenticated.recovery_data_id
            && recovery_account.lamports() == balance_after
            && keeper.lamports() == keeper_balance_after,
        ClutchError::MismatchedState,
    )?;
    drop(recovery_data);
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_MARKET_PAID_ADVANCE_RUNTIME_POSTWRITE_DOMAIN_V2,
            &advance.id().bytes(),
            &authenticated.recovery_data_id.bytes(),
            &recovery_data_after_id.bytes(),
            &authenticated.recovery_balance.to_le_bytes(),
            &balance_after.to_le_bytes(),
            keeper.key.as_ref(),
            &keeper_balance_before.to_le_bytes(),
            &keeper_balance_after.to_le_bytes(),
            &facts.runtime_work_receipt_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedFailureRecoveryPaidAdvanceV2 {
        id,
        intent,
        observation,
        transition,
        recovery_data_before_id: authenticated.recovery_data_id,
        recovery_data_after_id,
        keeper_balance_before,
        keeper_balance_after,
    })
}

fn require_exact_recovery_transition_v2(
    transition: RuntimeAtomicTransitionV1,
    authenticated: AuthenticatedFailureRecoveryLivenessPrestateV2,
    facts: FailureMarketIntervalCellAdvanceFactsV2,
    keeper: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    balance_after: u64,
) -> Outcome<()> {
    require(
        transition.action == RuntimeTransitionActionV1::SpendWork
            && transition.kind == RuntimeCompartmentKindV1::Recovery
            && transition.account_id == authenticated.recovery.identity.account_id
            && transition.account_balance_before == authenticated.recovery_balance
            && transition.account_balance_after == balance_after
            && transition.state_before == authenticated.recovery
            && transition.write_account_data
            && !transition.close_account
            && transition.state_after.completed_calls
                == authenticated
                    .recovery
                    .completed_calls
                    .checked_add(1)
                    .ok_or(ClutchError::Arithmetic)?
            && transition.state_after.remaining_calls
                == authenticated
                    .recovery
                    .remaining_calls
                    .checked_sub(1)
                    .ok_or(ClutchError::Arithmetic)?
            && transition.state_after.completed_work_ceiling_lamports
                == authenticated
                    .recovery
                    .completed_work_ceiling_lamports
                    .checked_add(facts.exact_reward_lamports)
                    .ok_or(ClutchError::Arithmetic)?
            && transition.state_after.remaining_work_lamports
                == authenticated
                    .recovery
                    .remaining_work_lamports
                    .checked_sub(facts.exact_reward_lamports)
                    .ok_or(ClutchError::Arithmetic)?
            && transition.state_after.last_work_receipt_id == facts.runtime_work_receipt_id
            && transition.state_after.keeper_paid_lamports
                == authenticated
                    .recovery
                    .keeper_paid_lamports
                    .checked_add(facts.exact_reward_lamports)
                    .ok_or(ClutchError::Arithmetic)?
            && transition.state_after.payer_refunded_work_lamports
                == authenticated.recovery.payer_refunded_work_lamports
            && authenticated.recovery.identity.payer == liveness_id(payer_refund.key)
            && transition.transfers().len() == 1,
        ClutchError::MismatchedState,
    )?;
    let movement = transition.transfers()[0];
    require(
        movement.role == RuntimeTransferRoleV1::KeeperPayment
            && movement.destination == liveness_id(keeper.key)
            && movement.lamports == facts.exact_reward_lamports,
        ClutchError::MismatchedState,
    )
}

fn require_exact_recovery_intent_and_observation_v2(
    intent: RuntimeTransitionIntentV1,
    observation: RuntimeReceiptObservationV1,
    expected: FailureRecoveryTransitionExpectationV2,
) -> Outcome<()> {
    require(
        intent.action == RuntimeTransitionActionV1::SpendWork
            && intent.kind == RuntimeCompartmentKindV1::Recovery
            && intent.policy_id == expected.policy_id
            && intent.lifecycle_id == expected.lifecycle_id
            && intent.account_id == expected.account_id
            && intent.semantic_owner == expected.semantic_owner
            && intent.quote_schedule_id == expected.quote_schedule_id
            && intent.receipt_id == expected.receipt_id
            && intent.keeper == expected.keeper
            && intent.generation == expected.generation
            && intent.call_ordinal == expected.call_ordinal
            && intent.call_ceiling_lamports == expected.exact_reward_lamports
            && intent.keeper_payment_lamports == expected.exact_reward_lamports
            && intent.flags == 0
            && observation.receipt_account_id == expected.receipt_account_id
            && observation.receipt_account_owner_program_id == expected.receipt_program_id
            && observation.receipt_id == expected.receipt_id
            && observation.receipt_kind == RuntimeReceiptKindV1::WorkCompleted
            && observation.compartment_kind == RuntimeCompartmentKindV1::Recovery
            && observation.semantic_owner == expected.semantic_owner
            && observation.lifecycle_id == expected.lifecycle_id
            && observation.quote_schedule_id == expected.quote_schedule_id
            && observation.generation == expected.generation
            && observation.call_ordinal == expected.call_ordinal
            && observation.call_ceiling_lamports == expected.exact_reward_lamports,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn require_distinct_paid_advance_accounts_v2(
    root: &AccountInfo<'_>,
    link: &AccountInfo<'_>,
    cell: &AccountInfo<'_>,
    history: &AccountInfo<'_>,
    policy: &AccountInfo<'_>,
    recovery: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    source_join: SourcePolicyHandoffJoinV1,
) -> Outcome<()> {
    require_distinct_paid_advance_keys_v2(
        [
            *root.key,
            *link.key,
            *cell.key,
            *history.key,
            *policy.key,
            *recovery.key,
            admission.account(),
            Pubkey::new_from_array(source_join.occurrence_account().bytes()),
            Pubkey::new_from_array(source_join.result_or_absence_account().bytes()),
            Pubkey::new_from_array(source_join.work_receipt_account().bytes()),
        ],
        *keeper.key,
        *payer.key,
    )
}

fn require_distinct_paid_advance_keys_v2(
    fixed: [Pubkey; 10],
    keeper: Pubkey,
    payer: Pubkey,
) -> Outcome<()> {
    let mut index = 0_usize;
    while index < fixed.len() {
        let mut other = index + 1;
        while other < fixed.len() {
            require(fixed[index] != fixed[other], ClutchError::AccountAlias)?;
            other += 1;
        }
        require(fixed[index] != keeper, ClutchError::AccountAlias)?;
        require(fixed[index] != payer, ClutchError::AccountAlias)?;
        index += 1;
    }
    Ok(())
}

fn require_system_recipient(account: &AccountInfo<'_>, writable: bool) -> Outcome<()> {
    require(
        account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.is_writable == writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == 0,
        ClutchError::MismatchedState,
    )
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

fn liveness_id(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> LivenessId {
        LivenessId::from_bytes([byte; 32])
    }

    fn transition_expectation() -> FailureRecoveryTransitionExpectationV2 {
        FailureRecoveryTransitionExpectationV2 {
            policy_id: id(1),
            lifecycle_id: id(2),
            account_id: id(3),
            semantic_owner: id(4),
            quote_schedule_id: id(5),
            receipt_account_id: id(6),
            receipt_program_id: id(7),
            receipt_id: id(8),
            keeper: id(9),
            generation: 10,
            call_ordinal: 11,
            exact_reward_lamports: 12,
        }
    }

    fn transition_intent(
        expected: FailureRecoveryTransitionExpectationV2,
    ) -> RuntimeTransitionIntentV1 {
        RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Recovery,
            policy_id: expected.policy_id,
            lifecycle_id: expected.lifecycle_id,
            account_id: expected.account_id,
            semantic_owner: expected.semantic_owner,
            quote_schedule_id: expected.quote_schedule_id,
            receipt_id: expected.receipt_id,
            keeper: expected.keeper,
            generation: expected.generation,
            call_ordinal: expected.call_ordinal,
            call_ceiling_lamports: expected.exact_reward_lamports,
            keeper_payment_lamports: expected.exact_reward_lamports,
            flags: 0,
        }
    }

    fn receipt_observation(
        expected: FailureRecoveryTransitionExpectationV2,
    ) -> RuntimeReceiptObservationV1 {
        RuntimeReceiptObservationV1 {
            receipt_account_id: expected.receipt_account_id,
            receipt_account_owner_program_id: expected.receipt_program_id,
            receipt_id: expected.receipt_id,
            receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
            compartment_kind: RuntimeCompartmentKindV1::Recovery,
            semantic_owner: expected.semantic_owner,
            lifecycle_id: expected.lifecycle_id,
            quote_schedule_id: expected.quote_schedule_id,
            generation: expected.generation,
            call_ordinal: expected.call_ordinal,
            call_ceiling_lamports: expected.exact_reward_lamports,
        }
    }

    #[test]
    fn sole_outer_paid_advance_orders_runtime_before_final_cell_write() {
        let source = include_str!("failure_market_interval_advance_v2.rs");
        let outer = source
            .find("pub(crate) fn advance_failure_market_interval_paid_v2")
            .unwrap();
        let runtime_write = source[outer..]
            .find("let runtime = apply_failure_recovery_paid_advance_v2")
            .unwrap();
        let cell_write = source[outer..]
            .find("let interval_after = write_failure_market_interval_paid_advance_v2")
            .unwrap();
        assert!(runtime_write < cell_write);
        assert_eq!(
            source
                .matches("pub(crate) fn advance_failure_market_interval_paid_v2")
                .count(),
            1
        );
        assert!(!source.contains("write_failure_market_interval_cell_plan_v2"));
    }

    #[test]
    fn recovery_payment_has_no_failure_reserve_or_hoard_path() {
        let source = include_str!("failure_market_interval_advance_v2.rs");
        assert!(source.contains("RuntimeCompartmentKindV1::Recovery"));
        assert!(!source.contains("Hoard"));
        assert!(!source.contains("future_fee"));
        assert!(!source.contains("accepted_progress_reward"));
    }

    #[test]
    fn exact_recovery_intent_refuses_every_authority_substitution() {
        let expected = transition_expectation();
        let intent = transition_intent(expected);
        let observation = receipt_observation(expected);
        assert!(
            require_exact_recovery_intent_and_observation_v2(intent, observation, expected,)
                .is_ok()
        );

        let mut wrong = intent;
        wrong.kind = RuntimeCompartmentKindV1::Source;
        assert!(
            require_exact_recovery_intent_and_observation_v2(wrong, observation, expected,)
                .is_err()
        );
        wrong = intent;
        wrong.receipt_id = id(13);
        assert!(
            require_exact_recovery_intent_and_observation_v2(wrong, observation, expected,)
                .is_err()
        );
        wrong = intent;
        wrong.keeper = id(14);
        assert!(
            require_exact_recovery_intent_and_observation_v2(wrong, observation, expected,)
                .is_err()
        );
        wrong = intent;
        wrong.call_ordinal = 13;
        assert!(
            require_exact_recovery_intent_and_observation_v2(wrong, observation, expected,)
                .is_err()
        );
        wrong = intent;
        wrong.call_ceiling_lamports = 13;
        assert!(
            require_exact_recovery_intent_and_observation_v2(wrong, observation, expected,)
                .is_err()
        );
        wrong = intent;
        wrong.keeper_payment_lamports = 11;
        assert!(
            require_exact_recovery_intent_and_observation_v2(wrong, observation, expected,)
                .is_err()
        );

        let mut wrong_observation = observation;
        wrong_observation.receipt_account_id = id(15);
        assert!(require_exact_recovery_intent_and_observation_v2(
            intent,
            wrong_observation,
            expected,
        )
        .is_err());
        wrong_observation = observation;
        wrong_observation.receipt_account_owner_program_id = id(16);
        assert!(require_exact_recovery_intent_and_observation_v2(
            intent,
            wrong_observation,
            expected,
        )
        .is_err());
        wrong_observation = observation;
        wrong_observation.receipt_kind = RuntimeReceiptKindV1::TerminalSuccess;
        assert!(require_exact_recovery_intent_and_observation_v2(
            intent,
            wrong_observation,
            expected,
        )
        .is_err());
        wrong_observation = observation;
        wrong_observation.generation = 17;
        assert!(require_exact_recovery_intent_and_observation_v2(
            intent,
            wrong_observation,
            expected,
        )
        .is_err());
    }

    #[test]
    fn every_fixed_paid_advance_account_alias_refuses() {
        let canonical = [
            Pubkey::new_from_array([1; 32]),
            Pubkey::new_from_array([2; 32]),
            Pubkey::new_from_array([3; 32]),
            Pubkey::new_from_array([4; 32]),
            Pubkey::new_from_array([5; 32]),
            Pubkey::new_from_array([6; 32]),
            Pubkey::new_from_array([7; 32]),
            Pubkey::new_from_array([8; 32]),
            Pubkey::new_from_array([9; 32]),
            Pubkey::new_from_array([10; 32]),
        ];
        let keeper = Pubkey::new_from_array([11; 32]);
        let payer = Pubkey::new_from_array([12; 32]);
        assert!(require_distinct_paid_advance_keys_v2(canonical, keeper, payer).is_ok());
        assert!(require_distinct_paid_advance_keys_v2(canonical, keeper, keeper).is_ok());

        let mut first = 0_usize;
        while first < canonical.len() {
            let mut second = first + 1;
            while second < canonical.len() {
                let mut aliased = canonical;
                aliased[second] = aliased[first];
                assert!(require_distinct_paid_advance_keys_v2(aliased, keeper, payer).is_err());
                second += 1;
            }
            assert!(
                require_distinct_paid_advance_keys_v2(canonical, canonical[first], payer).is_err()
            );
            assert!(
                require_distinct_paid_advance_keys_v2(canonical, keeper, canonical[first]).is_err()
            );
            first += 1;
        }
    }

    #[test]
    fn stale_prestates_and_final_refusal_are_structurally_atomic() {
        let source = include_str!("failure_market_interval_advance_v2.rs");
        let cell_source = include_str!("failure_market_interval_v2.rs");
        assert!(source.contains("authenticate_market_lifecycle_root_v1"));
        assert!(source.contains("authenticate_series_market_link_v1"));
        assert!(source.contains("== authenticated.recovery_data_id"));
        assert!(cell_source.contains("authenticate_unchanged_account_prestate"));
        assert!(cell_source.contains("authenticate_write_prestate"));
        assert!(source.contains("any later refusal rolls the whole SVM instruction back"));
    }
}
