// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fail-closed retirement prerequisite for one successor Failure occurrence.
//!
//! The current repository has no semantic owner for the complete liability
//! lifecycle of a Product `MarketInstanceV2`. The legacy Market/SupplyLedger
//! family is keyed by another market model, the Series funding state owns only
//! capitalization and its global ordinal cursor, and General selected-candidate
//! retirement owns only a General epoch. None can authorize closure of this
//! occurrence.
//!
//! This module therefore owns only the exact prerequisite below. It binds every
//! terminal fact which does exist, derives deterministic closed-Recovery and
//! replay joins, and then refuses to mint root-close authority until the missing
//! Product occurrence liability owner exists. It accepts no caller boolean,
//! generic attestation, or substitute terminal authority.

use clutch_evidence_recovery::RecoveryPhase;
use clutch_liveness::runtime_adapter_v1::{
    RuntimeAtomicTransitionV1, RuntimeTransferRoleV1, RuntimeTransitionActionV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeBalanceTransitionV1, RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1,
    RuntimeLivenessErrorV1, RuntimeTerminalAuthorizationV1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
};
use clutch_product_series::{MarketInstanceV2Id, SeriesPlanV5, SeriesPlanV5Id};
use clutch_source_plane_v3_runtime::AuthenticatedSourceReleaseV1;
use sha2::{Digest, Sha256};

use crate::external_v2::{
    FailureRecoveryTerminalDispositionV2, FailureRecoveryTerminalReceiptV2,
    FailureRuntimeExternalV2, FailureRuntimeStateCommitmentV2,
};
use crate::FailurePolicyBindingId;

const REPLAY_EXPECTATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/failure-prefunded-replay-expectation/v1";
const CLOSED_RECOVERY_JOIN_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-closed-recovery-join/v1";
const RETIREMENT_PREREQUISITE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/failure-retirement-prerequisite/v1";
const REPLAY_JOIN_DOMAIN_V1: &[u8] = b"dragons-clutch/failure-retirement-replay-join/v1";

/// Canonical name of the missing semantic owner which prevents authorization.
pub const MISSING_PRODUCT_OCCURRENCE_LIABILITY_OWNER_V1: &str =
    "Product MarketInstanceV2 liability lifecycle runtime";

/// Result alias for the fail-closed retirement prerequisite.
pub type RetirementResultV1<T> = core::result::Result<T, FailureRetirementErrorV1>;

macro_rules! retirement_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from exact digest bytes without claiming authenticity.
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

retirement_id!(
    PrefundedFailureReplayExpectationIdV1,
    "Typed identity of one Series-funded permanent Failure replay expectation."
);
retirement_id!(
    ClosedFailureRecoveryJoinIdV1,
    "Typed commitment to the exact successful liveness Recovery close."
);
retirement_id!(
    FailureRetirementReplayJoinIdV1,
    "Typed generation-bound join to the expected permanent replay account."
);
retirement_id!(
    FailureRetirementPrerequisiteIdV1,
    "Typed identity of every presently available Failure retirement prerequisite."
);
retirement_id!(
    FailureRootCloseAuthorizationIdV1,
    "Typed identity reserved for a complete future Failure root-close authorization."
);

/// Exact refusal from the per-occurrence retirement prerequisite owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureRetirementErrorV1 {
    /// The Failure semantic owner refused the exact runtime or receipt.
    Failure(crate::Error),
    /// Product refused the exact Series identity.
    Product(clutch_product_series::Error),
    /// Liveness refused the supplied successful close transition.
    Liveness(RuntimeLivenessErrorV1),
    /// A required identity was the reserved all-zero value.
    ZeroIdentity,
    /// Independently owned occurrence fields did not form one exact join.
    BindingMismatch,
    /// Failure Recovery was not resolved; dormancy is not retirement.
    RecoveryNotResolved,
    /// The supplied liveness transition was not the exact successful Recovery close.
    RecoveryCloseMismatch,
    /// No authoritative successor Product occurrence liability/terminal owner exists.
    ProductMarketInstanceV2LiabilityTerminalOwnerUnavailable,
}

impl From<crate::Error> for FailureRetirementErrorV1 {
    fn from(value: crate::Error) -> Self {
        Self::Failure(value)
    }
}

impl From<clutch_product_series::Error> for FailureRetirementErrorV1 {
    fn from(value: clutch_product_series::Error) -> Self {
        Self::Product(value)
    }
}

impl From<RuntimeLivenessErrorV1> for FailureRetirementErrorV1 {
    fn from(value: RuntimeLivenessErrorV1) -> Self {
        Self::Liveness(value)
    }
}

/// Adapter-bound expectation for the permanent replay account funded at
/// occurrence activation.
///
/// This is not terminal authority and does not prove that an arbitrary account
/// currently has these properties. A concrete Product/Series adapter must mint
/// it only from its authenticated MarketCore funding receipt and exact physical
/// account observation. The retirement prerequisite rechecks every semantic
/// field against the Failure runtime before retaining it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefundedFailureReplayExpectationV1 {
    id: PrefundedFailureReplayExpectationIdV1,
    account_id: [u8; 32],
    stored_bump: u8,
    binding_id: FailurePolicyBindingId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    market_core_funding_receipt_id: [u8; 32],
    permanent_rent_lamports: u64,
    prior_donation_lamports: u64,
    permanent_rent_funder: [u8; 32],
}

impl PrefundedFailureReplayExpectationV1 {
    /// Bind the exact facts projected by the Product/Series funding adapter.
    ///
    /// No boolean claims are accepted. This constructor describes the future
    /// adapter seam; it does not itself authenticate Solana accounts or derive
    /// a PDA.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_series_funding_adapter(
        account_id: [u8; 32],
        stored_bump: u8,
        binding_id: FailurePolicyBindingId,
        series_plan_id: SeriesPlanV5Id,
        ordinal: u32,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        market_core_funding_receipt_id: [u8; 32],
        permanent_rent_lamports: u64,
        prior_donation_lamports: u64,
        permanent_rent_funder: [u8; 32],
    ) -> RetirementResultV1<Self> {
        if any_zero(&[
            account_id,
            binding_id.bytes(),
            series_plan_id.bytes(),
            market_instance_id.bytes(),
            market_core_funding_receipt_id,
            permanent_rent_funder,
        ]) {
            return Err(FailureRetirementErrorV1::ZeroIdentity);
        }
        if generation == 0 || permanent_rent_lamports == 0 || account_id == permanent_rent_funder {
            return Err(FailureRetirementErrorV1::BindingMismatch);
        }
        let mut hasher = Sha256::new();
        hasher.update(REPLAY_EXPECTATION_DOMAIN_V1);
        hasher.update(account_id);
        hasher.update([stored_bump]);
        hasher.update(binding_id.bytes());
        hasher.update(series_plan_id.bytes());
        hasher.update(ordinal.to_le_bytes());
        hasher.update(market_instance_id.bytes());
        hasher.update(generation.to_le_bytes());
        hasher.update(market_core_funding_receipt_id);
        hasher.update(permanent_rent_lamports.to_le_bytes());
        hasher.update(prior_donation_lamports.to_le_bytes());
        hasher.update(permanent_rent_funder);
        Ok(Self {
            id: PrefundedFailureReplayExpectationIdV1::from_bytes(hasher.finalize().into()),
            account_id,
            stored_bump,
            binding_id,
            series_plan_id,
            ordinal,
            market_instance_id,
            generation,
            market_core_funding_receipt_id,
            permanent_rent_lamports,
            prior_donation_lamports,
            permanent_rent_funder,
        })
    }

    /// Complete deterministic expectation identity.
    pub const fn id(self) -> PrefundedFailureReplayExpectationIdV1 {
        self.id
    }

    /// Expected permanent replay account.
    pub const fn account_id(self) -> [u8; 32] {
        self.account_id
    }

    /// Canonical bump persisted in the permanent replay body.
    pub const fn stored_bump(self) -> u8 {
        self.stored_bump
    }

    /// Exact immutable Failure binding persisted in the replay body.
    pub const fn binding_id(self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact SeriesPlanV5 identity.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact occurrence ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Exact full-width Product occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Nonzero one-shot activation generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Product/Series MarketCore funding receipt which paid permanent rent.
    pub const fn market_core_funding_receipt_id(self) -> [u8; 32] {
        self.market_core_funding_receipt_id
    }

    /// Exact permanent rent principal paid at activation.
    pub const fn permanent_rent_lamports(self) -> u64 {
        self.permanent_rent_lamports
    }

    /// Lamports already present before the admitted rent debit.
    pub const fn prior_donation_lamports(self) -> u64 {
        self.prior_donation_lamports
    }

    /// Exact historical funder of permanent replay rent.
    pub const fn permanent_rent_funder(self) -> [u8; 32] {
        self.permanent_rent_funder
    }
}

/// Every available, exact prerequisite for one resolved Failure root close.
///
/// Private fields prevent consumers from assembling a partial DTO. This is
/// deliberately not root-close authority: [`Self::authorize_root_close`]
/// always refuses until the named Product liability owner exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRetirementPrerequisiteV1 {
    id: FailureRetirementPrerequisiteIdV1,
    binding_id: FailurePolicyBindingId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    semantic_state_id: [u8; 32],
    generation: u64,
    transition_nonce: u64,
    runtime_state_commitment: FailureRuntimeStateCommitmentV2,
    recovery_terminal_receipt_id: [u8; 32],
    closed_recovery_join_id: ClosedFailureRecoveryJoinIdV1,
    source_release_account_id: [u8; 32],
    source_release_manifest_id: [u8; 32],
    source_release_authentication_id: [u8; 32],
    replay_expectation_id: PrefundedFailureReplayExpectationIdV1,
    replay_account_id: [u8; 32],
    replay_join_id: FailureRetirementReplayJoinIdV1,
}

impl FailureRetirementPrerequisiteV1 {
    /// Deterministic identity of the complete available prerequisite.
    pub const fn id(self) -> FailureRetirementPrerequisiteIdV1 {
        self.id
    }

    /// Exact immutable Failure binding.
    pub const fn binding_id(self) -> FailurePolicyBindingId {
        self.binding_id
    }

    /// Exact SeriesPlanV5 identity.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact ordinal inside the finite Series.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Exact full-width economic occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Durable Failure semantic-state identity.
    pub const fn semantic_state_id(self) -> [u8; 32] {
        self.semantic_state_id
    }

    /// Nonzero occurrence generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact resolved Failure transition nonce.
    pub const fn transition_nonce(self) -> u64 {
        self.transition_nonce
    }

    /// Commitment to every canonical resolved Failure runtime byte.
    pub const fn runtime_state_commitment(self) -> FailureRuntimeStateCommitmentV2 {
        self.runtime_state_commitment
    }

    /// Exact resolved Failure receipt consumed by liveness close.
    pub const fn recovery_terminal_receipt_id(self) -> [u8; 32] {
        self.recovery_terminal_receipt_id
    }

    /// Deterministic join to the exact successful Recovery close poststate.
    pub const fn closed_recovery_join_id(self) -> ClosedFailureRecoveryJoinIdV1 {
        self.closed_recovery_join_id
    }

    /// Physical immutable Source release account authenticated by Source.
    pub const fn source_release_account_id(self) -> [u8; 32] {
        self.source_release_account_id
    }

    /// Exact Source release manifest admitted by Failure.
    pub const fn source_release_manifest_id(self) -> [u8; 32] {
        self.source_release_manifest_id
    }

    /// Complete Source owner/PDA/body authentication admitted by Failure.
    pub const fn source_release_authentication_id(self) -> [u8; 32] {
        self.source_release_authentication_id
    }

    /// Series-funded permanent replay expectation.
    pub const fn replay_expectation_id(self) -> PrefundedFailureReplayExpectationIdV1 {
        self.replay_expectation_id
    }

    /// Expected pre-funded permanent replay account.
    pub const fn replay_account_id(self) -> [u8; 32] {
        self.replay_account_id
    }

    /// Deterministic generation-bound pending replay join.
    pub const fn replay_join_id(self) -> FailureRetirementReplayJoinIdV1 {
        self.replay_join_id
    }

    /// Name the exact semantic owner still required for root closure.
    pub const fn missing_owner(self) -> MissingFailureRetirementOwnerV1 {
        MissingFailureRetirementOwnerV1::ProductMarketInstanceV2LiabilityLifecycle
    }

    /// Refuse root-close authorization until the actual successor Product
    /// occurrence liability runtime supplies its private terminal capability.
    ///
    /// This method cannot be bypassed with a boolean or a General/Series
    /// substitute. The return type is private-field and has no public
    /// constructor.
    pub const fn authorize_root_close(self) -> RetirementResultV1<FailureRootCloseAuthorizationV1> {
        Err(FailureRetirementErrorV1::ProductMarketInstanceV2LiabilityTerminalOwnerUnavailable)
    }
}

/// Missing semantic-owner classification carried by the fail-closed boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingFailureRetirementOwnerV1 {
    /// Successor Product `MarketInstanceV2` liability lifecycle and market terminality.
    ProductMarketInstanceV2LiabilityLifecycle,
}

/// Private-field root-close capability reserved for the complete future join.
///
/// No public constructor exists in this generation. In particular, a client,
/// a Series funding cursor, and a General selected candidate cannot mint it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRootCloseAuthorizationV1 {
    id: FailureRootCloseAuthorizationIdV1,
    prerequisite_id: FailureRetirementPrerequisiteIdV1,
    product_occurrence_terminal_account_id: [u8; 32],
    product_occurrence_terminal_owner_program_id: [u8; 32],
    product_occurrence_terminal_receipt_id: [u8; 32],
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    replay_join_id: FailureRetirementReplayJoinIdV1,
}

impl FailureRootCloseAuthorizationV1 {
    /// Complete future authorization identity.
    pub const fn id(self) -> FailureRootCloseAuthorizationIdV1 {
        self.id
    }

    /// Exact prerequisite consumed by the future owner join.
    pub const fn prerequisite_id(self) -> FailureRetirementPrerequisiteIdV1 {
        self.prerequisite_id
    }

    /// Physical account of the missing Product occurrence-liability owner.
    pub const fn product_occurrence_terminal_account_id(self) -> [u8; 32] {
        self.product_occurrence_terminal_account_id
    }

    /// Runtime program which must own the physical terminal account.
    pub const fn product_occurrence_terminal_owner_program_id(self) -> [u8; 32] {
        self.product_occurrence_terminal_owner_program_id
    }

    /// Receipt from the missing Product occurrence liability owner.
    pub const fn product_occurrence_terminal_receipt_id(self) -> [u8; 32] {
        self.product_occurrence_terminal_receipt_id
    }

    /// Exact full-width Product occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact one-shot occurrence generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Permanent replay join sealed by the future root close.
    pub const fn replay_join_id(self) -> FailureRetirementReplayJoinIdV1 {
        self.replay_join_id
    }
}

/// Bind every terminal fact currently owned by an implemented runtime.
///
/// The liveness close is re-executed from its complete authenticated prestate;
/// its poststate, transfer list, zero balance, close flags, and exact Failure
/// terminal receipt must all match. The resulting prerequisite still cannot
/// authorize deletion because Product occurrence liabilities have no current
/// semantic owner.
#[allow(clippy::too_many_arguments)]
pub fn prepare_failure_retirement_prerequisite_v1(
    runtime: &FailureRuntimeExternalV2,
    recovery_terminal: FailureRecoveryTerminalReceiptV2,
    closed_recovery: RuntimeAtomicTransitionV1,
    series: &SeriesPlanV5,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    source_release: AuthenticatedSourceReleaseV1,
    replay: PrefundedFailureReplayExpectationV1,
) -> RetirementResultV1<FailureRetirementPrerequisiteV1> {
    runtime.check()?;
    series.validate_shape()?;
    if runtime.phase() != RecoveryPhase::Resolved
        || recovery_terminal.disposition() != FailureRecoveryTerminalDispositionV2::Resolved
    {
        return Err(FailureRetirementErrorV1::RecoveryNotResolved);
    }
    let expected_terminal = runtime.recovery_terminal_receipt()?;
    let runtime_state_commitment = runtime.state_commitment()?;
    if recovery_terminal != expected_terminal
        || recovery_terminal.runtime_state_commitment() != runtime_state_commitment
    {
        return Err(FailureRetirementErrorV1::BindingMismatch);
    }

    let binding = runtime.binding();
    let series_plan_id = series.id()?;
    if binding.series_plan_id() != series_plan_id
        || binding.ordinal() != ordinal
        || binding.market_instance_id() != market_instance_id
        || replay.series_plan_id() != series_plan_id
        || replay.ordinal() != ordinal
        || replay.market_instance_id() != market_instance_id
        || replay.generation() != binding.generation()
        || replay.binding_id() != runtime.binding_id()
        || replay.permanent_rent_funder() != runtime.recovery_payer().bytes()
    {
        return Err(FailureRetirementErrorV1::BindingMismatch);
    }

    runtime.authenticate_source_release(source_release)?;
    if source_release.manifest_id() != runtime.source_release_manifest_id()
        || source_release.id() != runtime.source_release_authentication_id()
    {
        return Err(FailureRetirementErrorV1::BindingMismatch);
    }

    let closed_recovery_join_id = authenticate_closed_failure_recovery_close_v1(
        closed_recovery,
        recovery_terminal,
        runtime.recovery_compartment_account_id().bytes(),
    )?;
    let semantic_state_id = runtime.semantic_state_id().bytes();
    let source_release_account_id = source_release.account().bytes();
    let source_release_manifest_id = source_release.manifest_id().bytes();
    let source_release_authentication_id = source_release.id().bytes();
    let replay_account_id = replay.account_id();

    if any_zero(&[
        semantic_state_id,
        source_release_account_id,
        source_release_manifest_id,
        source_release_authentication_id,
        replay_account_id,
    ]) {
        return Err(FailureRetirementErrorV1::ZeroIdentity);
    }

    let replay_join_id = retirement_replay_join_id(
        runtime.binding_id(),
        series_plan_id,
        ordinal,
        market_instance_id,
        binding.generation(),
        replay,
    );
    let recovery_terminal_receipt_id = recovery_terminal.id().bytes();
    let transition_nonce = recovery_terminal.transition_nonce();
    let mut hasher = Sha256::new();
    hasher.update(RETIREMENT_PREREQUISITE_DOMAIN_V1);
    hasher.update(runtime.binding_id().bytes());
    hasher.update(series_plan_id.bytes());
    hasher.update(ordinal.to_le_bytes());
    hasher.update(market_instance_id.bytes());
    hasher.update(semantic_state_id);
    hasher.update(binding.generation().to_le_bytes());
    hasher.update(transition_nonce.to_le_bytes());
    hasher.update(runtime_state_commitment.bytes());
    hasher.update(recovery_terminal_receipt_id);
    hasher.update(closed_recovery_join_id.bytes());
    hasher.update(source_release_account_id);
    hasher.update(source_release_manifest_id);
    hasher.update(source_release_authentication_id);
    hasher.update(replay.id().bytes());
    hasher.update(replay_account_id);
    hasher.update(replay_join_id.bytes());
    Ok(FailureRetirementPrerequisiteV1 {
        id: FailureRetirementPrerequisiteIdV1::from_bytes(hasher.finalize().into()),
        binding_id: runtime.binding_id(),
        series_plan_id,
        ordinal,
        market_instance_id,
        semantic_state_id,
        generation: binding.generation(),
        transition_nonce,
        runtime_state_commitment,
        recovery_terminal_receipt_id,
        closed_recovery_join_id,
        source_release_account_id,
        source_release_manifest_id,
        source_release_authentication_id,
        replay_expectation_id: replay.id(),
        replay_account_id,
        replay_join_id,
    })
}

/// Authenticate the exact successful Recovery close transition and derive its
/// canonical join identity. Adapters may use this to recheck that an atomic
/// a0/a2/a3 writer is applying the same close embedded in a retirement
/// prerequisite.
pub fn authenticate_closed_failure_recovery_close_v1(
    close: RuntimeAtomicTransitionV1,
    terminal: FailureRecoveryTerminalReceiptV2,
    expected_account_id: [u8; 32],
) -> RetirementResultV1<ClosedFailureRecoveryJoinIdV1> {
    let intent = terminal.runtime_transition_intent();
    if close.action != RuntimeTransitionActionV1::CloseSuccess
        || close.kind != RuntimeCompartmentKindV1::Recovery
        || close.account_id.bytes() != expected_account_id
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
        || close.state_before.terminal_receipt_id != clutch_liveness::Id::ZERO
        || close.state_after.terminal_receipt_id != intent.receipt_id
        || !close.close_account
        || close.write_account_data
        || close.post_account_data.iter().any(|byte| *byte != 0)
    {
        return Err(FailureRetirementErrorV1::RecoveryCloseMismatch);
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
    let (expected_after, movement) = close.state_before.close_success(authorization, balances)?;
    if expected_after != close.state_after
        || !movement.success
        || movement.kind != RuntimeCompartmentKindV1::Recovery
        || movement.account != close.account_id
        || movement.terminal_receipt_id != intent.receipt_id
        || !terminal_transfers_match(&close, movement)
    {
        return Err(FailureRetirementErrorV1::RecoveryCloseMismatch);
    }

    let mut before = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    let mut after = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    close.state_before.encode(&mut before)?;
    close.state_after.encode(&mut after)?;
    let mut hasher = Sha256::new();
    hasher.update(CLOSED_RECOVERY_JOIN_DOMAIN_V1);
    hasher.update([2]);
    hasher.update([6]);
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
        hasher.update([transfer_role_code(transfer.role)]);
    }
    Ok(ClosedFailureRecoveryJoinIdV1::from_bytes(
        hasher.finalize().into(),
    ))
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

fn retirement_replay_join_id(
    binding_id: FailurePolicyBindingId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    replay: PrefundedFailureReplayExpectationV1,
) -> FailureRetirementReplayJoinIdV1 {
    let mut hasher = Sha256::new();
    hasher.update(REPLAY_JOIN_DOMAIN_V1);
    hasher.update(binding_id.bytes());
    hasher.update(series_plan_id.bytes());
    hasher.update(ordinal.to_le_bytes());
    hasher.update(market_instance_id.bytes());
    hasher.update(generation.to_le_bytes());
    hasher.update(replay.id().bytes());
    hasher.update(replay.account_id());
    hasher.update([replay.stored_bump()]);
    hasher.update(replay.market_core_funding_receipt_id());
    hasher.update(replay.permanent_rent_lamports().to_le_bytes());
    hasher.update(replay.prior_donation_lamports().to_le_bytes());
    FailureRetirementReplayJoinIdV1::from_bytes(hasher.finalize().into())
}

const fn transfer_role_code(role: RuntimeTransferRoleV1) -> u8 {
    match role {
        RuntimeTransferRoleV1::KeeperPayment => 0,
        RuntimeTransferRoleV1::PayerWorkRefund => 1,
        RuntimeTransferRoleV1::PayerTerminalRefund => 2,
        RuntimeTransferRoleV1::NeutralTerminalSink => 3,
    }
}

fn any_zero(values: &[[u8; 32]]) -> bool {
    values
        .iter()
        .any(|value| value.iter().all(|byte| *byte == 0))
}
