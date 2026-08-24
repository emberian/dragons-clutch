// SPDX-License-Identifier: AGPL-3.0-or-later
//! Atomic shared-Market Recovery close and runtime terminal postwrite.
//!
//! This capability-disabled composer is the sole bridge from the archived
//! reusable interval pair into liveness `CloseSuccess`. It authenticates the
//! full liveness policy/Recovery bodies, mints the fresh Market terminal
//! receipt from private Product and Source postwrites, plans both mutations
//! before the first write, drains only liveness-owned Recovery custody, and
//! then advances `0xa0/v3` without moving any Failure-account lamports.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::failure_market_admission::{
    authenticate_failure_market_root_v2, AuthenticatedFailureMarketRootV2,
};
use crate::instructions::failure_market_interval_v2::{
    authenticate_failure_market_interval_accounts_v2, AuthenticatedFailureMarketIntervalAccountsV2,
    FailureMarketIntervalArchivePostwriteV2,
};
use crate::instructions::failure_market_resolution_v5::AuthenticatedFailureMarketResolutionPostwriteV5;
use crate::instructions::failure_market_runtime::{
    authenticate_failure_market_runtime_root_v1, write_failure_market_runtime_terminal_plan_v2,
    AuthenticatedFailureMarketRuntimeRootV1, AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::instructions::source_terminal_resolution_v5::{
    AuthenticatedSourceResolutionStatisticResultCloseV1, AuthenticatedSourceResolutionTerminalV1,
    PersistedSourceResolutionTerminalPolicyV1,
};
use crate::seeds;
use clutch_failure_policy_runtime::market_recovery_terminal_v2::{
    admit_failure_market_recovery_terminal_v2,
    authenticate_closed_failure_market_recovery_close_v2,
    AuthenticatedFailureMarketRecoveryTerminalV2, FailureMarketRecoveryTerminalFactsV2,
    FailureMarketRecoveryTerminalReceiptV2,
};
use clutch_failure_policy_runtime::market_runtime_v1::{
    plan_close_failure_market_recovery_v2, AuthenticatedFailureMarketRecoveryCloseV2,
    FailureMarketRecoveryCloseFactsV2, FailureMarketRecoveryCloseReceiptV2,
};
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_compartment_account_v1, decode_runtime_policy_account_v1,
    plan_runtime_transition_v1, RuntimeAtomicTransitionV1, RuntimePersistedAccountViewV1,
    RuntimeTransferRoleV1, RuntimeTransitionActionV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::ContentId;
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1,
    FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1, FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
    FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
};
use clutch_solana_layout::registry;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const RECOVERY_CLOSE_POSTWRITE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/sbf/failure-market-recovery-close-postwrite/v2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRecoveryTerminalAuthorityV2 {
    expected: FailureMarketRecoveryTerminalFactsV2,
}

impl AuthenticatedFailureMarketRecoveryTerminalV2 for FailureMarketRecoveryTerminalAuthorityV2 {
    fn authenticate_failure_market_recovery_terminal(
        &self,
        expected: FailureMarketRecoveryTerminalFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FailureMarketRecoveryCloseAuthorityV2 {
    expected: FailureMarketRecoveryCloseFactsV2,
}

impl AuthenticatedFailureMarketRecoveryCloseV2 for FailureMarketRecoveryCloseAuthorityV2 {
    fn authenticate_failure_market_recovery_close(
        &self,
        expected: FailureMarketRecoveryCloseFactsV2,
    ) -> clutch_failure_policy_runtime::Result<()> {
        if expected != self.expected {
            return Err(clutch_failure_policy_runtime::Error::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedFailureMarketRecoveryLivenessTerminalPrestateV2 {
    policy: RuntimeLivenessPolicyV1,
    recovery: RuntimeCompartmentV1,
    recovery_stored_bump: u8,
    recovery_data_id: ContentId,
    recovery_balance: u64,
}

/// Exact postwrite after sole-custody Recovery close and runtime transition.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedFailureMarketRecoveryClosePostwriteV2 {
    id: ContentId,
    terminal: FailureMarketRecoveryTerminalReceiptV2,
    transition: RuntimeAtomicTransitionV1,
    close: FailureMarketRecoveryCloseReceiptV2,
    runtime: AuthenticatedFailureMarketRuntimeRootV1,
    interval: AuthenticatedFailureMarketIntervalAccountsV2,
}

impl AuthenticatedFailureMarketRecoveryClosePostwriteV2 {
    /// Complete physical/semantic postwrite identity.
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    /// Exact pre-close Market terminal receipt.
    pub(crate) const fn terminal(self) -> FailureMarketRecoveryTerminalReceiptV2 {
        self.terminal
    }

    /// Re-executed and physically applied liveness close.
    pub(crate) const fn transition(self) -> RuntimeAtomicTransitionV1 {
        self.transition
    }

    /// Exact Recovery-close receipt retained by `0xa0/v3`.
    pub(crate) const fn close(self) -> FailureMarketRecoveryCloseReceiptV2 {
        self.close
    }

    /// Hostile-reopened `RecoveryClosed` runtime.
    pub(crate) const fn runtime(self) -> AuthenticatedFailureMarketRuntimeRootV1 {
        self.runtime
    }

    /// Unchanged Idle cell and full unsealed history retained for family seal.
    pub(crate) const fn interval(self) -> AuthenticatedFailureMarketIntervalAccountsV2 {
        self.interval
    }
}

/// Close Recovery only after resolution, Source terminalization, interval
/// archive, Product-link release, and exact shared-runtime archive postwrite.
#[allow(clippy::too_many_arguments)]
pub(crate) fn close_failure_market_recovery_v2<'a>(
    program_id: &Pubkey,
    admission_root_account: &AccountInfo<'a>,
    runtime_root_account: &AccountInfo<'a>,
    interval_cell_account: &AccountInfo<'a>,
    interval_history_account: &AccountInfo<'a>,
    liveness_policy_account: &AccountInfo<'a>,
    recovery_account: &AccountInfo<'a>,
    recovery_refund_owner: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    admission: AuthenticatedFailureMarketRootV2,
    archive: FailureMarketIntervalArchivePostwriteV2,
    archive_runtime: AuthenticatedFailureMarketRuntimeSessionPostwriteV1,
    resolution: AuthenticatedFailureMarketResolutionPostwriteV5,
    source_terminal: AuthenticatedSourceResolutionTerminalV1,
    source_result_close: AuthenticatedSourceResolutionStatisticResultCloseV1,
) -> Outcome<AuthenticatedFailureMarketRecoveryClosePostwriteV2> {
    require_distinct(&[
        admission_root_account.clone(),
        runtime_root_account.clone(),
        interval_cell_account.clone(),
        interval_history_account.clone(),
        liveness_policy_account.clone(),
        recovery_account.clone(),
        recovery_refund_owner.clone(),
        neutral_sink.clone(),
    ])?;
    let live_admission =
        authenticate_failure_market_root_v2(program_id, admission_root_account, false)?;
    require(live_admission == admission, ClutchError::MismatchedState)?;
    let live_runtime = authenticate_failure_market_runtime_root_v1(
        program_id,
        admission_root_account,
        runtime_root_account,
        live_admission,
        true,
    )?;
    require(
        live_runtime == archive_runtime.root(),
        ClutchError::MismatchedState,
    )?;
    let archived_accounts = archive.accounts();
    let live_interval = authenticate_failure_market_interval_accounts_v2(
        program_id,
        interval_cell_account,
        interval_history_account,
        live_admission,
        archived_accounts.funding(),
        archived_accounts.quote(),
        false,
        true,
    )?;
    let policy = live_admission.state().binding().facts();
    let source_liveness = source_terminal.liveness();
    let source_terminal_policy = match source_terminal.policy() {
        PersistedSourceResolutionTerminalPolicyV1::NoReopen(value) => value.authenticated(),
        PersistedSourceResolutionTerminalPolicyV1::ReopenRequest(_) => {
            return Err(Refusal::Adapter(ClutchError::MismatchedState));
        }
    };
    require(
        live_interval == archived_accounts
            && live_runtime.state().session_resolution_receipt_id().bytes()
                == resolution.failure_resolution().id().bytes()
            && live_runtime.state().interval_terminal_receipt_id()
                == archive.append().session_terminal_receipt_id()
            && live_runtime.state().session_history_commitment()
                == archive.append().resulting_root()
            && live_runtime.state().completed_session_count()
                == archive.append().completed_session_count()
            && resolution.product_activation().market_instance_id()
                == live_admission.state().binding().facts().market_instance_id
            && resolution.product_activation().generation()
                == live_admission.state().binding().facts().generation
            && resolution
                .product_activation()
                .failure_resolution_receipt_id()
                .bytes()
                == resolution.failure_resolution().id().bytes()
            && source_result_close.source_terminal_id() == source_terminal.id()
            && source_result_close.source_resolution_input_id()
                == source_terminal_policy.source_resolution_input_id()
            && source_result_close.result_account()
                == source_terminal_policy.target_account()
            && source_result_close.lineage_account()
                == source_terminal_policy.lineage_account()
            && source_result_close.lineage_state_before_id()
                == source_terminal_policy.expected_lineage_state_id()
            && source_result_close.lineage_state_after_id()
                != source_result_close.lineage_state_before_id()
            && !source_result_close.close().lineage_after.is_open
            && source_liveness.action == RuntimeTransitionActionV1::CloseSuccess
            && source_liveness.kind == RuntimeCompartmentKindV1::Source
            && source_liveness.close_account
            && source_liveness.state_before.identity.policy_id == policy.liveness_policy_id
            && source_liveness.state_before.identity.lifecycle_id == policy.liveness_lifecycle_id
            && source_liveness.state_before.identity.generation == policy.generation
            && source_liveness.state_before.identity.neutral_sink == policy.neutral_sink
            && source_liveness.account_id != policy.recovery_compartment_account_id
            && source_liveness.account_id.bytes() != runtime_root_account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let expected_terminal = FailureMarketRecoveryTerminalFactsV2 {
        runtime_before: live_runtime.state_commitment(),
        admission_state_id: live_admission
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        failure_policy_binding_id: live_admission.state().binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        receipt_account_id: live_runtime.state().runtime_account_id(),
        interval_cell_state_id: live_interval.cell_state_id(),
        interval_history_state_id: live_interval.history_state_id(),
        interval_history_root: live_interval.history().history_root(),
        completed_session_count: live_interval.history().completed_session_count(),
        completed_work_calls: live_interval.history().completed_work_calls(),
        exact_reward_lamports: live_interval.history().exact_reward_lamports(),
        latest_interval_terminal_receipt_id: live_interval.history().latest_terminal_receipt_id(),
        resolution_activation_receipt_id: resolution.product_activation().id(),
        source_resolution_terminal_receipt_id: ContentId::from_bytes(source_terminal.id().bytes()),
        source_result_close_receipt_id: ContentId::from_bytes(source_result_close.id().bytes()),
        liveness_policy_id: policy.liveness_policy_id,
        liveness_lifecycle_id: policy.liveness_lifecycle_id,
        recovery_compartment_account_id: policy.recovery_compartment_account_id,
        semantic_owner: policy.recovery_receipt_program_id,
        quote_schedule_id: policy.recovery_quote_schedule_id,
    };
    let terminal = admit_failure_market_recovery_terminal_v2(
        &FailureMarketRecoveryTerminalAuthorityV2 {
            expected: expected_terminal,
        },
        live_runtime.state(),
        live_admission.state(),
        live_interval.funding(),
        live_interval.quote(),
        live_interval.cell(),
        live_interval.history(),
        resolution.product_activation().id(),
        ContentId::from_bytes(source_terminal.id().bytes()),
        ContentId::from_bytes(source_result_close.id().bytes()),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let liveness = authenticate_failure_recovery_terminal_prestate_v2(
        program_id,
        liveness_policy_account,
        recovery_account,
        recovery_refund_owner,
        neutral_sink,
        live_admission,
        terminal,
    )?;
    let transition = plan_failure_recovery_close_v2(
        program_id,
        liveness_policy_account,
        recovery_account,
        runtime_root_account,
        liveness,
        terminal,
    )?;
    let closed_join = authenticate_closed_failure_market_recovery_close_v2(transition, terminal)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_close = FailureMarketRecoveryCloseFactsV2 {
        runtime_before: live_runtime.state_commitment(),
        admission_state_id: expected_terminal.admission_state_id,
        failure_policy_binding_id: expected_terminal.failure_policy_binding_id,
        market_instance_id: expected_terminal.market_instance_id,
        generation: expected_terminal.generation,
        runtime_account_id: expected_terminal.receipt_account_id,
        interval_cell_state_id: expected_terminal.interval_cell_state_id,
        interval_history_state_id: expected_terminal.interval_history_state_id,
        interval_history_root: expected_terminal.interval_history_root,
        completed_session_count: expected_terminal.completed_session_count,
        completed_work_calls: expected_terminal.completed_work_calls,
        exact_reward_lamports: expected_terminal.exact_reward_lamports,
        latest_interval_terminal_receipt_id: expected_terminal.latest_interval_terminal_receipt_id,
        resolution_activation_receipt_id: expected_terminal.resolution_activation_receipt_id,
        source_result_close_receipt_id: expected_terminal.source_result_close_receipt_id,
        recovery_terminal_receipt_id: terminal.id(),
        closed_recovery_join_id: closed_join,
    };
    let (runtime_plan, close) = plan_close_failure_market_recovery_v2(
        &FailureMarketRecoveryCloseAuthorityV2 {
            expected: expected_close,
        },
        live_runtime.state(),
        live_admission.state(),
        live_interval.funding(),
        live_interval.quote(),
        live_interval.cell(),
        live_interval.history(),
        terminal,
        closed_join,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    // Both poststates are complete before the first mutation. Any refusal in
    // the runtime writer rolls this liveness close back with the instruction.
    apply_failure_recovery_close_v2(
        recovery_account,
        recovery_refund_owner,
        neutral_sink,
        transition,
    )?;
    let runtime_after = write_failure_market_runtime_terminal_plan_v2(
        program_id,
        admission_root_account,
        runtime_root_account,
        live_admission,
        live_runtime,
        runtime_plan,
    )?;
    require(
        runtime_after.state().recovery_terminal_receipt_id().bytes() == close.id().bytes()
            && runtime_after.state().phase()
                == clutch_failure_policy_runtime::market_runtime_v1::FailureMarketRuntimePhaseV1::RecoveryClosed
            && runtime_root_account.lamports()
                >= runtime_after.state().root_funding().observed_balance_lamports,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            RECOVERY_CLOSE_POSTWRITE_DOMAIN_V2,
            &terminal.id().bytes(),
            &close.id().bytes(),
            &source_result_close.id().bytes(),
            &live_runtime.state_commitment().bytes(),
            &runtime_after.state_commitment().bytes(),
            &closed_join.bytes(),
            &liveness.recovery_data_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedFailureMarketRecoveryClosePostwriteV2 {
        id,
        terminal,
        transition,
        close,
        runtime: runtime_after,
        interval: live_interval,
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_failure_recovery_terminal_prestate_v2(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    payer_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    admission: AuthenticatedFailureMarketRootV2,
    terminal: FailureMarketRecoveryTerminalReceiptV2,
) -> Outcome<AuthenticatedFailureMarketRecoveryLivenessTerminalPrestateV2> {
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
    require_system_recipient(payer_refund)?;
    require_system_recipient(neutral_sink)?;
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
    let recovery = decode_runtime_compartment_account_v1(
        liveness_id(program_id),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id(recovery_account.key),
            owner_program_id: liveness_id(recovery_account.owner),
            lamports: recovery_account.lamports(),
            data: recovery_frame.body,
            writable: true,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    recovery
        .validate_against_policy(policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = admission.state().binding().facts();
    let terminal_facts = terminal.facts();
    require(
        recovery.kind == RuntimeCompartmentKindV1::Recovery
            && recovery.phase == RuntimeCompartmentPhaseV1::Active
            && recovery.identity.owner == liveness_id(program_id)
            && recovery.identity.owner == facts.recovery_receipt_program_id
            && recovery.identity.account_id == liveness_id(recovery_account.key)
            && recovery.identity.account_id == facts.recovery_compartment_account_id
            && recovery.identity.policy_id == policy.policy_id
            && policy.policy_id == facts.liveness_policy_id
            && recovery.identity.lifecycle_id == facts.liveness_lifecycle_id
            && recovery.identity.generation == facts.generation
            && recovery.identity.payer == facts.recovery_refund_owner
            && recovery.identity.payer == liveness_id(payer_refund.key)
            && recovery.identity.neutral_sink == facts.neutral_sink
            && recovery.identity.neutral_sink == liveness_id(neutral_sink.key)
            && recovery.quote_schedule_id == facts.recovery_quote_schedule_id
            && recovery.receipt_program_id == facts.recovery_receipt_program_id
            && terminal_facts.liveness_policy_id == policy.policy_id
            && terminal_facts.liveness_lifecycle_id == recovery.identity.lifecycle_id
            && terminal_facts.recovery_compartment_account_id == recovery.identity.account_id
            && terminal_facts.semantic_owner == recovery.identity.owner
            && terminal_facts.quote_schedule_id == recovery.quote_schedule_id
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
    require(!recovery_data_id.is_zero(), ClutchError::MismatchedState)?;
    Ok(
        AuthenticatedFailureMarketRecoveryLivenessTerminalPrestateV2 {
            policy,
            recovery,
            recovery_stored_bump: recovery_frame.stored_bump,
            recovery_data_id,
            recovery_balance: recovery_account.lamports(),
        },
    )
}

fn plan_failure_recovery_close_v2(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    recovery_account: &AccountInfo<'_>,
    receipt_account: &AccountInfo<'_>,
    authenticated: AuthenticatedFailureMarketRecoveryLivenessTerminalPrestateV2,
    terminal: FailureMarketRecoveryTerminalReceiptV2,
) -> Outcome<RuntimeAtomicTransitionV1> {
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
            && ContentId::from_bytes(solana_sha256_hasher::hashv(&[&recovery_data[..]]).to_bytes())
                == authenticated.recovery_data_id
            && recovery_account.lamports() == authenticated.recovery_balance
            && receipt_account.owner == program_id
            && receipt_account.key.to_bytes() == terminal.facts().receipt_account_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let intent = terminal.runtime_transition_intent();
    let observation = terminal.runtime_receipt_observation();
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
            lamports: recovery_account.lamports(),
            data: recovery_frame.body,
            writable: true,
        },
        intent,
        Some(observation),
        0,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        transition.action == RuntimeTransitionActionV1::CloseSuccess
            && transition.kind == RuntimeCompartmentKindV1::Recovery
            && transition.state_before == authenticated.recovery
            && transition.state_after.phase == RuntimeCompartmentPhaseV1::ClosedSuccess
            && transition.account_balance_before == authenticated.recovery_balance
            && transition.account_balance_after == 0
            && transition.close_account
            && !transition.write_account_data
            && transition.post_account_data.iter().all(|byte| *byte == 0),
        ClutchError::MismatchedState,
    )?;
    Ok(transition)
}

fn apply_failure_recovery_close_v2(
    recovery: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    sink: &AccountInfo<'_>,
    transition: RuntimeAtomicTransitionV1,
) -> Outcome<()> {
    let payer_amount = transfer_amount(
        &transition,
        RuntimeTransferRoleV1::PayerTerminalRefund,
        payer,
    )?;
    let sink_amount = transfer_amount(
        &transition,
        RuntimeTransferRoleV1::NeutralTerminalSink,
        sink,
    )?;
    require(
        transition.close_account
            && !transition.write_account_data
            && transition.account_id == liveness_id(recovery.key)
            && transition.account_balance_before == recovery.lamports()
            && transition.account_balance_after == 0
            && transition.transfers().len()
                == usize::from(payer_amount != 0) + usize::from(sink_amount != 0)
            && payer_amount
                .checked_add(sink_amount)
                .ok_or(ClutchError::Arithmetic)?
                == recovery.lamports(),
        ClutchError::MismatchedState,
    )?;
    let payer_after = payer
        .lamports()
        .checked_add(payer_amount)
        .ok_or(ClutchError::Arithmetic)?;
    let sink_after = sink
        .lamports()
        .checked_add(sink_amount)
        .ok_or(ClutchError::Arithmetic)?;
    {
        let mut recovery_lamports = recovery
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut payer_lamports = payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut sink_lamports = sink
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **recovery_lamports = 0;
        **payer_lamports = payer_after;
        **sink_lamports = sink_after;
    }
    recovery
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    recovery.assign(&SYSTEM_PROGRAM_ID);
    require(
        recovery.owner == &SYSTEM_PROGRAM_ID
            && recovery.data_len() == 0
            && recovery.lamports() == 0
            && payer.lamports() == payer_after
            && sink.lamports() == sink_after,
        ClutchError::MismatchedState,
    )
}

fn transfer_amount(
    transition: &RuntimeAtomicTransitionV1,
    role: RuntimeTransferRoleV1,
    destination: &AccountInfo<'_>,
) -> Outcome<u64> {
    let mut amount = 0u64;
    let mut seen = false;
    for transfer in transition.transfers() {
        if transfer.role == role {
            require(
                !seen && transfer.destination == liveness_id(destination.key),
                ClutchError::MismatchedState,
            )?;
            seen = true;
            amount = transfer.lamports;
        }
    }
    Ok(amount)
}

fn require_system_recipient(account: &AccountInfo<'_>) -> Outcome<()> {
    require(
        account.owner == &SYSTEM_PROGRAM_ID
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == 0,
        ClutchError::MismatchedState,
    )
}

fn liveness_id(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}

#[cfg(test)]
mod adversarial_recovery_close_tests {
    #[test]
    fn close_composer_plans_both_mutations_before_first_write() {
        let source = include_str!("failure_market_recovery_terminal_v2.rs");
        let outer = source
            .split("pub(crate) fn close_failure_market_recovery_v2")
            .nth(1)
            .expect("single close composer");
        let terminal = outer
            .find("admit_failure_market_recovery_terminal_v2")
            .expect("typed Market terminal");
        let liveness = outer
            .find("plan_failure_recovery_close_v2")
            .expect("liveness close plan");
        let runtime = outer
            .find("plan_close_failure_market_recovery_v2")
            .expect("runtime close plan");
        let first_write = outer
            .find("apply_failure_recovery_close_v2")
            .expect("first physical write");
        let second_write = outer
            .find("write_failure_market_runtime_terminal_plan_v2")
            .expect("second physical write");
        assert!(terminal < liveness && liveness < runtime);
        assert!(runtime < first_write && first_write < second_write);
    }

    #[test]
    fn close_refuses_legacy_external_receipts_and_failure_capital_debits() {
        let source = include_str!("failure_market_recovery_terminal_v2.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("FailureRuntimeExternalV2"));
        assert!(!production.contains("FailureRecoveryTerminalReceiptV2"));
        assert!(!production.contains("project_external_recovery_close_v2"));
        let apply_tail = production
            .split("fn apply_failure_recovery_close_v2")
            .nth(1)
            .expect("narrow liveness movement");
        let apply = apply_tail
            .split("fn transfer_amount")
            .next()
            .expect("bounded apply helper");
        assert!(!apply.contains("runtime_root"));
        assert!(!apply.contains("admission_root"));
    }

    #[test]
    fn close_requires_all_protocol_and_destination_roles_distinct() {
        let source = include_str!("failure_market_recovery_terminal_v2.rs");
        let outer = source
            .split("pub(crate) fn close_failure_market_recovery_v2")
            .nth(1)
            .expect("single close composer");
        let aliases = outer
            .split("require_distinct(&[")
            .nth(1)
            .and_then(|value| value.split("])?;").next())
            .expect("explicit alias set");
        for role in [
            "admission_root_account",
            "runtime_root_account",
            "interval_cell_account",
            "interval_history_account",
            "liveness_policy_account",
            "recovery_account",
            "recovery_refund_owner",
            "neutral_sink",
        ] {
            assert!(aliases.contains(role), "missing role {role}");
        }
    }

    #[test]
    fn source_terminal_cannot_substitute_or_alias_the_recovery_compartment() {
        let source = include_str!("failure_market_recovery_terminal_v2.rs");
        let outer = source
            .split("pub(crate) fn close_failure_market_recovery_v2")
            .nth(1)
            .expect("single close composer");
        for predicate in [
            "source_liveness.kind == RuntimeCompartmentKindV1::Source",
            "source_liveness.state_before.identity.policy_id == policy.liveness_policy_id",
            "source_liveness.state_before.identity.lifecycle_id == policy.liveness_lifecycle_id",
            "source_liveness.state_before.identity.generation == policy.generation",
            "source_liveness.account_id != policy.recovery_compartment_account_id",
        ] {
            assert!(outer.contains(predicate), "missing predicate {predicate}");
        }
    }

    #[test]
    fn recovery_terminal_requires_the_physical_source_result_close() {
        let source = include_str!("failure_market_recovery_terminal_v2.rs");
        let outer = source
            .split("pub(crate) fn close_failure_market_recovery_v2")
            .nth(1)
            .expect("single close composer");
        for predicate in [
            "source_result_close.source_terminal_id() == source_terminal.id()",
            "source_result_close.source_resolution_input_id()",
            "source_result_close.result_account()",
            "source_result_close.lineage_account()",
            "source_result_close.lineage_state_before_id()",
            "!source_result_close.close().lineage_after.is_open",
            "source_result_close_receipt_id: ContentId::from_bytes(source_result_close.id().bytes())",
            "&source_result_close.id().bytes()",
        ] {
            assert!(outer.contains(predicate), "missing Source close guard {predicate}");
        }
    }
}
