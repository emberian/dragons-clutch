// SPDX-License-Identifier: AGPL-3.0-or-later
//! Market-scoped successful Recovery terminal receipt and liveness close join.
//!
//! This successor never consumes the occurrence-scoped `ExternalV2` terminal
//! receipt. It derives one typed terminal receipt from the complete archived
//! shared-Market runtime, the canonical Idle interval pair, Product's once-only
//! Resolution V5 activation, and Source's persisted terminal decision. The
//! mutable Market runtime account is the authenticated receipt-bearing account;
//! liveness remains the sole owner of Recovery work/rent capital.

use clutch_liveness::runtime_adapter_v1::{
    RuntimeAtomicTransitionV1, RuntimeReceiptKindV1, RuntimeReceiptObservationV1,
    RuntimeTransferRoleV1, RuntimeTransitionActionV1, RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeBalanceTransitionV1, RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1,
    RuntimeTerminalAuthorizationV1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{ContentId as ProductContentId, MarketInstanceV2Id};
use sha2::{Digest, Sha256};

use crate::market_interval_cell_v2::{
    FailureMarketIntervalCellStateIdV2, FailureMarketIntervalCellV2,
};
use crate::market_interval_history_v2::{
    FailureMarketIntervalFundingReceiptV2, FailureMarketIntervalHistoryRootV2,
    FailureMarketIntervalHistoryStateIdV2, FailureMarketIntervalHistoryV2,
};
use crate::market_policy_v1::{
    FailureMarketAccountIdV1, FailureMarketAdmissionStateIdV1, FailureMarketAdmissionStateV1,
};
use crate::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use crate::market_runtime_v1::{
    validate_terminal_interval_pair, FailureMarketRuntimePhaseV1,
    FailureMarketRuntimeStateCommitmentV1, FailureMarketRuntimeV1,
};
use crate::{Error, FailurePolicyBindingId, Result};

const TERMINAL_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-recovery-terminal-receipt/v2";
const CLOSED_JOIN_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-closed-recovery-join/v2";

macro_rules! terminal_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from digest bytes without claiming authenticity.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Return exact digest bytes.
            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

terminal_id!(
    FailureMarketRecoveryTerminalReceiptIdV2,
    "Typed identity of one archived Market's successful Recovery terminal receipt."
);
terminal_id!(
    FailureMarketClosedRecoveryJoinIdV2,
    "Typed commitment to the exact successful shared-Market Recovery close."
);

/// Complete authenticated pre-close terminal facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryTerminalFactsV2 {
    /// Complete archived shared runtime.
    pub runtime_before: FailureMarketRuntimeStateCommitmentV1,
    /// Immutable admission identity.
    pub admission_state_id: FailureMarketAdmissionStateIdV1,
    /// Shared Failure policy.
    pub failure_policy_binding_id: FailurePolicyBindingId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Persisted receipt-bearing mutable runtime account.
    pub receipt_account_id: FailureMarketAccountIdV1,
    /// Canonical Idle interval cell.
    pub interval_cell_state_id: FailureMarketIntervalCellStateIdV2,
    /// Complete unsealed interval history.
    pub interval_history_state_id: FailureMarketIntervalHistoryStateIdV2,
    /// Append-only history root.
    pub interval_history_root: FailureMarketIntervalHistoryRootV2,
    /// Exact completed session count.
    pub completed_session_count: u64,
    /// Exact aggregate paid calls.
    pub completed_work_calls: u64,
    /// Exact aggregate keeper rewards.
    pub exact_reward_lamports: u64,
    /// Latest folded subordinate terminal receipt.
    pub latest_interval_terminal_receipt_id: ProductContentId,
    /// Product's once-only Resolution V5 activation receipt.
    pub resolution_activation_receipt_id: ProductContentId,
    /// Source's exact persisted terminal/no-reopen composition.
    pub source_resolution_terminal_receipt_id: ProductContentId,
    /// Source's exact physical StatisticResult/lineage close postwrite.
    pub source_result_close_receipt_id: ProductContentId,
    /// Immutable liveness policy.
    pub liveness_policy_id: LivenessId,
    /// Market-scoped liveness lifecycle.
    pub liveness_lifecycle_id: LivenessId,
    /// Sole Recovery custody account.
    pub recovery_compartment_account_id: LivenessId,
    /// Program semantic owner and required receipt account owner.
    pub semantic_owner: LivenessId,
    /// Exact Recovery quote schedule.
    pub quote_schedule_id: LivenessId,
}

/// Private authority over final Product, Source, interval, and runtime state.
pub trait AuthenticatedFailureMarketRecoveryTerminalV2 {
    /// Authenticate every expected fact without accepting a caller DTO.
    fn authenticate_failure_market_recovery_terminal(
        &self,
        _expected: FailureMarketRecoveryTerminalFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field semantic receipt accepted by liveness `CloseSuccess`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketRecoveryTerminalReceiptV2 {
    id: FailureMarketRecoveryTerminalReceiptIdV2,
    facts: FailureMarketRecoveryTerminalFactsV2,
}

impl FailureMarketRecoveryTerminalReceiptV2 {
    /// Exact typed terminal receipt identity.
    pub const fn id(self) -> FailureMarketRecoveryTerminalReceiptIdV2 {
        self.id
    }

    /// Complete authenticated pre-close facts.
    pub const fn facts(self) -> FailureMarketRecoveryTerminalFactsV2 {
        self.facts
    }

    /// Construct the sole admissible successful Recovery close intent.
    pub fn runtime_transition_intent(self) -> RuntimeTransitionIntentV1 {
        RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::CloseSuccess,
            kind: RuntimeCompartmentKindV1::Recovery,
            policy_id: self.facts.liveness_policy_id,
            lifecycle_id: self.facts.liveness_lifecycle_id,
            account_id: self.facts.recovery_compartment_account_id,
            semantic_owner: self.facts.semantic_owner,
            quote_schedule_id: self.facts.quote_schedule_id,
            receipt_id: LivenessId::from_bytes(self.id.bytes()),
            keeper: LivenessId::ZERO,
            generation: self.facts.generation,
            call_ordinal: 0,
            call_ceiling_lamports: 0,
            keeper_payment_lamports: 0,
            flags: 0,
        }
    }

    /// Project exact receipt facts for the liveness adapter.
    pub fn runtime_receipt_observation(self) -> RuntimeReceiptObservationV1 {
        RuntimeReceiptObservationV1 {
            receipt_account_id: LivenessId::from_bytes(self.facts.receipt_account_id.bytes()),
            receipt_account_owner_program_id: self.facts.semantic_owner,
            receipt_id: LivenessId::from_bytes(self.id.bytes()),
            receipt_kind: RuntimeReceiptKindV1::TerminalSuccess,
            compartment_kind: RuntimeCompartmentKindV1::Recovery,
            semantic_owner: self.facts.semantic_owner,
            lifecycle_id: self.facts.liveness_lifecycle_id,
            quote_schedule_id: self.facts.quote_schedule_id,
            generation: self.facts.generation,
            call_ordinal: 0,
            call_ceiling_lamports: 0,
        }
    }
}

/// Mint the only successful terminal receipt for an archived shared Market.
#[allow(clippy::too_many_arguments)]
pub fn admit_failure_market_recovery_terminal_v2<
    A: AuthenticatedFailureMarketRecoveryTerminalV2 + ?Sized,
>(
    authority: &A,
    runtime: FailureMarketRuntimeV1,
    admission: FailureMarketAdmissionStateV1,
    interval_funding: FailureMarketIntervalFundingReceiptV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    cell: FailureMarketIntervalCellV2,
    history: FailureMarketIntervalHistoryV2,
    resolution_activation_receipt_id: ProductContentId,
    source_resolution_terminal_receipt_id: ProductContentId,
    source_result_close_receipt_id: ProductContentId,
) -> Result<FailureMarketRecoveryTerminalReceiptV2> {
    runtime.validate_against_admission(admission)?;
    if runtime.phase() != FailureMarketRuntimePhaseV1::IntervalArchived {
        return Err(Error::WrongPhase);
    }
    let (interval_cell_state_id, interval_history_state_id) = validate_terminal_interval_pair(
        runtime,
        admission,
        interval_funding,
        quote,
        cell,
        history,
    )?;
    require_live(resolution_activation_receipt_id.bytes())?;
    require_live(source_resolution_terminal_receipt_id.bytes())?;
    require_live(source_result_close_receipt_id.bytes())?;
    if resolution_activation_receipt_id == source_resolution_terminal_receipt_id
        || resolution_activation_receipt_id == source_result_close_receipt_id
        || source_resolution_terminal_receipt_id == source_result_close_receipt_id
    {
        return Err(Error::BindingMismatch);
    }
    let policy = admission.binding().facts();
    let facts = FailureMarketRecoveryTerminalFactsV2 {
        runtime_before: runtime.commitment()?,
        admission_state_id: admission.id()?,
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        generation: policy.generation,
        receipt_account_id: runtime.runtime_account_id(),
        interval_cell_state_id,
        interval_history_state_id,
        interval_history_root: history.history_root(),
        completed_session_count: history.completed_session_count(),
        completed_work_calls: history.completed_work_calls(),
        exact_reward_lamports: history.exact_reward_lamports(),
        latest_interval_terminal_receipt_id: history.latest_terminal_receipt_id(),
        resolution_activation_receipt_id,
        source_resolution_terminal_receipt_id,
        source_result_close_receipt_id,
        liveness_policy_id: policy.liveness_policy_id,
        liveness_lifecycle_id: policy.liveness_lifecycle_id,
        recovery_compartment_account_id: policy.recovery_compartment_account_id,
        semantic_owner: policy.recovery_receipt_program_id,
        quote_schedule_id: policy.recovery_quote_schedule_id,
    };
    authority.authenticate_failure_market_recovery_terminal(facts)?;
    let mut hasher = Sha256::new();
    hasher.update(TERMINAL_DOMAIN_V2);
    hash_terminal_facts(&mut hasher, facts);
    let id = FailureMarketRecoveryTerminalReceiptIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    Ok(FailureMarketRecoveryTerminalReceiptV2 { id, facts })
}

/// Re-execute and bind the exact liveness `CloseSuccess` poststate.
pub fn authenticate_closed_failure_market_recovery_close_v2(
    close: RuntimeAtomicTransitionV1,
    terminal: FailureMarketRecoveryTerminalReceiptV2,
) -> Result<FailureMarketClosedRecoveryJoinIdV2> {
    let intent = terminal.runtime_transition_intent();
    if close.action != RuntimeTransitionActionV1::CloseSuccess
        || close.kind != RuntimeCompartmentKindV1::Recovery
        || close.account_id != intent.account_id
        || close.account_balance_after != 0
        || close.state_before.phase != RuntimeCompartmentPhaseV1::Active
        || close.state_after.phase != RuntimeCompartmentPhaseV1::ClosedSuccess
        || close.state_before.identity.policy_id != intent.policy_id
        || close.state_before.identity.lifecycle_id != intent.lifecycle_id
        || close.state_before.identity.account_id != intent.account_id
        || close.state_before.identity.owner != intent.semantic_owner
        || close.state_before.identity.generation != intent.generation
        || close.state_before.quote_schedule_id != intent.quote_schedule_id
        || close.state_before.receipt_program_id != terminal.facts.semantic_owner
        || close.state_before.terminal_receipt_id != LivenessId::ZERO
        || close.state_after.terminal_receipt_id != intent.receipt_id
        || !close.close_account
        || close.write_account_data
        || close.post_account_data.iter().any(|byte| *byte != 0)
    {
        return Err(Error::BindingMismatch);
    }
    let authorization = RuntimeTerminalAuthorizationV1 {
        kind: RuntimeCompartmentKindV1::Recovery,
        account: intent.account_id,
        owner: intent.semantic_owner,
        generation: intent.generation,
        terminal_receipt_id: intent.receipt_id,
    };
    let balances = RuntimeBalanceTransitionV1 {
        account_balance_before: close.account_balance_before,
        account_balance_after: close.account_balance_after,
    };
    let (expected_after, movement) = close
        .state_before
        .close_success(authorization, balances)
        .map_err(|_| Error::BindingMismatch)?;
    if expected_after != close.state_after
        || !movement.success
        || movement.kind != RuntimeCompartmentKindV1::Recovery
        || movement.account != close.account_id
        || movement.terminal_receipt_id != intent.receipt_id
        || !terminal_transfers_match(&close, movement)
    {
        return Err(Error::BindingMismatch);
    }

    let mut before = [0; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    let mut after = [0; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    close
        .state_before
        .encode(&mut before)
        .map_err(|_| Error::BindingMismatch)?;
    close
        .state_after
        .encode(&mut after)
        .map_err(|_| Error::BindingMismatch)?;
    let mut hasher = Sha256::new();
    hasher.update(CLOSED_JOIN_DOMAIN_V2);
    hasher.update(terminal.id.bytes());
    hasher.update(close.account_id.bytes());
    hasher.update(close.account_balance_before.to_le_bytes());
    hasher.update(close.account_balance_after.to_le_bytes());
    hasher.update(before);
    hasher.update(after);
    hasher.update([u8::from(close.write_account_data)]);
    hasher.update([u8::from(close.close_account)]);
    for transfer in close.transfers() {
        hasher.update(transfer.destination.bytes());
        hasher.update(transfer.lamports.to_le_bytes());
        hasher.update([transfer_role_byte(transfer.role)]);
    }
    let id = FailureMarketClosedRecoveryJoinIdV2::from_bytes(hasher.finalize().into());
    require_live(id.bytes())?;
    Ok(id)
}

fn hash_terminal_facts(hasher: &mut Sha256, facts: FailureMarketRecoveryTerminalFactsV2) {
    hasher.update(facts.runtime_before.bytes());
    hasher.update(facts.admission_state_id.bytes());
    hasher.update(facts.failure_policy_binding_id.bytes());
    hasher.update(facts.market_instance_id.bytes());
    hasher.update(facts.generation.to_le_bytes());
    hasher.update(facts.receipt_account_id.bytes());
    hasher.update(facts.interval_cell_state_id.bytes());
    hasher.update(facts.interval_history_state_id.bytes());
    hasher.update(facts.interval_history_root.bytes());
    hasher.update(facts.completed_session_count.to_le_bytes());
    hasher.update(facts.completed_work_calls.to_le_bytes());
    hasher.update(facts.exact_reward_lamports.to_le_bytes());
    hasher.update(facts.latest_interval_terminal_receipt_id.bytes());
    hasher.update(facts.resolution_activation_receipt_id.bytes());
    hasher.update(facts.source_resolution_terminal_receipt_id.bytes());
    hasher.update(facts.source_result_close_receipt_id.bytes());
    hasher.update(facts.liveness_policy_id.bytes());
    hasher.update(facts.liveness_lifecycle_id.bytes());
    hasher.update(facts.recovery_compartment_account_id.bytes());
    hasher.update(facts.semantic_owner.bytes());
    hasher.update(facts.quote_schedule_id.bytes());
}

fn terminal_transfers_match(
    close: &RuntimeAtomicTransitionV1,
    movement: clutch_liveness::runtime_v1::RuntimeTerminalMovementV1,
) -> bool {
    let transfers = close.transfers();
    let mut expected_count = 0usize;
    if movement.payer_refund_lamports != 0 {
        if transfers.get(expected_count).map(|transfer| {
            transfer.destination == movement.payer
                && transfer.lamports == movement.payer_refund_lamports
                && transfer.role == RuntimeTransferRoleV1::PayerTerminalRefund
        }) != Some(true)
        {
            return false;
        }
        expected_count += 1;
    }
    if movement.neutral_lamports != 0 {
        if transfers.get(expected_count).map(|transfer| {
            transfer.destination == movement.neutral_sink
                && transfer.lamports == movement.neutral_lamports
                && transfer.role == RuntimeTransferRoleV1::NeutralTerminalSink
        }) != Some(true)
        {
            return false;
        }
        expected_count += 1;
    }
    transfers.len() == expected_count
}

fn transfer_role_byte(role: RuntimeTransferRoleV1) -> u8 {
    match role {
        RuntimeTransferRoleV1::KeeperPayment => 1,
        RuntimeTransferRoleV1::PayerWorkRefund => 2,
        RuntimeTransferRoleV1::PayerTerminalRefund => 3,
        RuntimeTransferRoleV1::NeutralTerminalSink => 4,
    }
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod adversarial_terminal_tests {
    use super::*;

    #[test]
    fn terminal_identity_commits_source_and_product_terminal_owners_separately() {
        let mut facts = FailureMarketRecoveryTerminalFactsV2 {
            runtime_before: FailureMarketRuntimeStateCommitmentV1::from_bytes([1; 32]),
            admission_state_id: FailureMarketAdmissionStateIdV1::from_bytes([2; 32]),
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes([3; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([4; 32]),
            generation: 5,
            receipt_account_id: FailureMarketAccountIdV1::from_bytes([6; 32]),
            interval_cell_state_id: FailureMarketIntervalCellStateIdV2::from_bytes([7; 32]),
            interval_history_state_id: FailureMarketIntervalHistoryStateIdV2::from_bytes([8; 32]),
            interval_history_root: FailureMarketIntervalHistoryRootV2::from_bytes([9; 32]),
            completed_session_count: 10,
            completed_work_calls: 11,
            exact_reward_lamports: 12,
            latest_interval_terminal_receipt_id: ProductContentId::from_bytes([13; 32]),
            resolution_activation_receipt_id: ProductContentId::from_bytes([14; 32]),
            source_resolution_terminal_receipt_id: ProductContentId::from_bytes([15; 32]),
            source_result_close_receipt_id: ProductContentId::from_bytes([16; 32]),
            liveness_policy_id: LivenessId::from_bytes([17; 32]),
            liveness_lifecycle_id: LivenessId::from_bytes([18; 32]),
            recovery_compartment_account_id: LivenessId::from_bytes([19; 32]),
            semantic_owner: LivenessId::from_bytes([20; 32]),
            quote_schedule_id: LivenessId::from_bytes([21; 32]),
        };
        let mut first = Sha256::new();
        hash_terminal_facts(&mut first, facts);
        facts.source_resolution_terminal_receipt_id = ProductContentId::from_bytes([21; 32]);
        let mut source_splice = Sha256::new();
        hash_terminal_facts(&mut source_splice, facts);
        facts.source_resolution_terminal_receipt_id = ProductContentId::from_bytes([15; 32]);
        facts.resolution_activation_receipt_id = ProductContentId::from_bytes([22; 32]);
        let mut product_splice = Sha256::new();
        hash_terminal_facts(&mut product_splice, facts);
        assert_ne!(first.finalize(), source_splice.finalize());
        let mut original = Sha256::new();
        let mut original_facts = facts;
        original_facts.resolution_activation_receipt_id = ProductContentId::from_bytes([14; 32]);
        hash_terminal_facts(&mut original, original_facts);
        assert_ne!(original.finalize(), product_splice.finalize());
        let mut physical_splice = facts;
        physical_splice.source_result_close_receipt_id = ProductContentId::from_bytes([23; 32]);
        let mut physical = Sha256::new();
        hash_terminal_facts(&mut physical, physical_splice);
        let mut unspliced = Sha256::new();
        hash_terminal_facts(&mut unspliced, facts);
        assert_ne!(unspliced.finalize(), physical.finalize());
    }
}
