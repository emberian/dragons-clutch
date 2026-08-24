// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reusable Market-scoped interval session cell.
//!
//! Product owns the exact 592-byte structural-work body. Failure owns the
//! exclusive session pin, bounded paid-transition transcript, terminal latch,
//! and canonical reset. The cell retains its original Rent principal through
//! every session and is physically closed only at exhaustive Market terminal.

use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    advance_quantized_interval_consensus_work_v1, begin_quantized_interval_consensus_v1,
    restore_verified_quantized_interval_payout_v1,
    AuthenticatedQuantizedIntervalConsensusHistoryV1, ContentId as ProductContentId, FixedCodec,
    MarketInstanceV2Id, QuantizedIntervalConsensusCertificateV1Id,
    QuantizedIntervalConsensusContextV1, QuantizedIntervalConsensusProgressV1,
    QuantizedIntervalConsensusWorkV1, QuantizedIntervalConsensusWorkV1Id,
    VerifiedQuantizedIntervalPayoutV1, QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1,
};
use clutch_source_plane_v3::ContentId as SourceContentId;
use clutch_source_plane_v3_runtime::{
    FailurePolicySourceHandoffV1, RuntimeKey as SourceRuntimeKey, SourceFailureKindV1,
    SourcePolicyHandoffJoinV1,
    SuccessfulEvaluationHandoffV1,
};
use sha2::{Digest, Sha256};

use crate::market_interval_history_v2::{
    FailureMarketIntervalFundingReceiptIdV2, FailureMarketIntervalFundingReceiptV2,
    FailureMarketIntervalHistoryAppendReceiptV2, FailureMarketIntervalHistoryRootV2,
    FailureMarketIntervalHistoryV2, FailureMarketIntervalTerminalDispositionV2,
    FailureMarketIntervalTerminalFactsV2,
};
use crate::market_policy_v1::{FailureMarketAccountIdV1, FailureMarketAdmissionStateV1};
use crate::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use crate::{Error, FailurePolicyBindingId, Result};

const CELL_MAGIC_V2: [u8; 8] = *b"DCFICEL2";
const CELL_VERSION_V2: u16 = 2;
const CELL_STATE_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-cell-state/v2";
const CELL_ACTIVATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-activation/v2";
const CELL_WORK_AUTHORIZATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-work-authorization/v2";
const CELL_ADVANCE_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-cell-advance/v2";
const CELL_RESOLUTION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-resolution/v2";
const CELL_EXHAUSTION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-exhaustion/v2";
const CELL_SOURCE_FAILURE_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-source-failure/v2";
const CELL_RESET_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-cell-reset/v2";
const HEADER_BYTES_V2: usize = 16;
const AMOUNT_COUNT_V2: usize = 7;
const ID_COUNT_V2: usize = 13;
const ID_BYTES_V2: usize = 32;

/// Exact semantic-owner body inside the 1,088-byte `0xab/v2` account.
pub const FAILURE_MARKET_INTERVAL_CELL_BYTES_V2: usize = 1_084;

macro_rules! cell_id {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Construct from exact digest bytes without claiming authority.
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

cell_id!(
    FailureMarketIntervalCellStateIdV2,
    "Typed commitment to one complete reusable-cell postimage."
);
cell_id!(
    FailureMarketIntervalCellActivationReceiptIdV2,
    "Typed receipt for one exact Idle-to-Active session pin."
);
cell_id!(
    FailureMarketIntervalCellResetReceiptIdV2,
    "Typed receipt for one terminal append and canonical Idle reset."
);
cell_id!(
    FailureMarketIntervalCellWorkAuthorizationIdV2,
    "Typed exact liveness work authorization for one priced cell advance."
);
cell_id!(
    FailureMarketIntervalCellAdvanceReceiptIdV2,
    "Typed receipt joining one Product work transition to exact liveness payment."
);
cell_id!(
    FailureMarketIntervalCellResolutionReceiptIdV2,
    "Typed private receipt for one exhaustive Product interval resolution."
);
cell_id!(
    FailureMarketIntervalCellExhaustionReceiptIdV2,
    "Typed private receipt for one deterministic finite-budget exhaustion."
);
cell_id!(
    FailureMarketIntervalCellSourceFailureReceiptIdV2,
    "Typed private receipt for one exact zero-payout Source failure attempt."
);

/// Reusable-cell lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketIntervalCellPhaseV2 {
    /// No subordinate session is pinned.
    Idle = 1,
    /// One subordinate session exclusively owns the cell.
    Active = 2,
    /// The session is terminal and must be appended before reset.
    Resolved = 3,
}

impl FailureMarketIntervalCellPhaseV2 {
    fn byte(self) -> u8 {
        match self {
            Self::Idle => 1,
            Self::Active => 2,
            Self::Resolved => 3,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Idle),
            2 => Ok(Self::Active),
            3 => Ok(Self::Resolved),
            _ => Err(Error::InvalidEnum),
        }
    }
}

/// Closed terminal classification. `None` is valid only before terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketIntervalCellDispositionV2 {
    /// Cell is not terminal.
    None = 0,
    /// Exhaustive Product evaluation produced one exact resolution.
    Resolved = 1,
    /// The finite authenticated progress budget was exhausted.
    Exhausted = 2,
    /// Mature Source absence consumed this attempt without Product work.
    SourceAbsent = 3,
    /// Stable evaluator refusal consumed this attempt without Product work.
    SourceRefused = 4,
}

impl FailureMarketIntervalCellDispositionV2 {
    fn byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Resolved => 1,
            Self::Exhausted => 2,
            Self::SourceAbsent => 3,
            Self::SourceRefused => 4,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Resolved),
            2 => Ok(Self::Exhausted),
            3 => Ok(Self::SourceAbsent),
            4 => Ok(Self::SourceRefused),
            _ => Err(Error::InvalidEnum),
        }
    }
}

/// Complete Failure-owned reusable session cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellV2 {
    phase: FailureMarketIntervalCellPhaseV2,
    disposition: FailureMarketIntervalCellDispositionV2,
    attempt_index: u8,
    generation: u64,
    work_rent_principal_lamports: u64,
    completed_session_count: u64,
    transition_nonce: u64,
    accepted_progress_units: u64,
    completed_work_calls: u64,
    exact_reward_lamports: u64,
    failure_policy_binding_id: FailurePolicyBindingId,
    market_instance_id: MarketInstanceV2Id,
    funding_receipt_id: FailureMarketIntervalFundingReceiptIdV2,
    history_account: FailureMarketAccountIdV1,
    rent_refund_owner: FailureMarketAccountIdV1,
    neutral_sink: FailureMarketAccountIdV1,
    session_binding_id: SourceContentId,
    source_handoff_id: SourceContentId,
    session_schedule_id: SourceContentId,
    quote_admission_receipt_id: SourceContentId,
    last_transition_receipt_id: SourceContentId,
    last_liveness_work_receipt_id: SourceContentId,
    terminal_receipt_id: SourceContentId,
    product_work_body: [u8; QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1],
}

impl FailureMarketIntervalCellV2 {
    /// Current reusable-cell phase.
    pub const fn phase(self) -> FailureMarketIntervalCellPhaseV2 {
        self.phase
    }

    /// Terminal classification, or `None` before terminal.
    pub const fn disposition(self) -> FailureMarketIntervalCellDispositionV2 {
        self.disposition
    }

    /// Exact shared Failure policy.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Full-width economic Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Shared Failure/liveness generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Product-authenticated reusable-cell/history capitalization.
    pub const fn funding_receipt_id(self) -> FailureMarketIntervalFundingReceiptIdV2 {
        self.funding_receipt_id
    }

    /// Canonical append-only history account paired with this cell.
    pub const fn history_account(self) -> FailureMarketAccountIdV1 {
        self.history_account
    }

    /// Number of prior terminal sessions already folded into history.
    pub const fn completed_session_count(self) -> u64 {
        self.completed_session_count
    }

    /// Monotone paid-transition count inside the currently pinned session.
    pub const fn transition_nonce(self) -> u64 {
        self.transition_nonce
    }

    /// Exact accepted progress already priced against the active attempt row.
    pub const fn accepted_progress_units(self) -> u64 {
        self.accepted_progress_units
    }

    /// Exact count of Recovery-funded work calls in this session.
    pub const fn completed_work_calls(self) -> u64 {
        self.completed_work_calls
    }

    /// Exact cumulative keeper rewards paid in this session.
    pub const fn exact_reward_lamports(self) -> u64 {
        self.exact_reward_lamports
    }

    /// Immutable principal-refund owner fixed by Product capitalization.
    pub const fn rent_refund_owner(self) -> FailureMarketAccountIdV1 {
        self.rent_refund_owner
    }

    /// Current active attempt row, or zero while Idle.
    pub const fn attempt_index(self) -> u8 {
        self.attempt_index
    }

    /// Exact current session binding, or zero while Idle.
    pub const fn session_binding_id(self) -> SourceContentId {
        self.session_binding_id
    }

    /// Exact successful Source handoff pinned by the active session.
    pub const fn source_handoff_id(self) -> SourceContentId {
        self.source_handoff_id
    }

    /// Exact Source work schedule pinned by the active session.
    pub const fn session_schedule_id(self) -> SourceContentId {
        self.session_schedule_id
    }

    /// Current structural Product work, absent while Idle or for a direct
    /// zero-payout Source-failure terminal.
    pub fn product_work(self) -> Result<Option<QuantizedIntervalConsensusWorkV1>> {
        if self.phase == FailureMarketIntervalCellPhaseV2::Idle
            || matches!(
                self.disposition,
                FailureMarketIntervalCellDispositionV2::SourceAbsent
                    | FailureMarketIntervalCellDispositionV2::SourceRefused
            )
        {
            Ok(None)
        } else {
            Ok(Some(QuantizedIntervalConsensusWorkV1::decode(
                &self.product_work_body,
            )?))
        }
    }

    /// Typed content identity of every semantic byte.
    pub fn id(self) -> Result<FailureMarketIntervalCellStateIdV2> {
        let mut body = [0; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2];
        self.encode_into(&mut body)?;
        let mut hasher = Sha256::new();
        hasher.update(CELL_STATE_DOMAIN_V2);
        hasher.update(body);
        Ok(FailureMarketIntervalCellStateIdV2::from_bytes(
            hasher.finalize().into(),
        ))
    }

    /// Encode the exact 1,084-byte semantic body.
    pub fn encode_into(
        self,
        output: &mut [u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
    ) -> Result<()> {
        self.validate()?;
        output.fill(0);
        output[..8].copy_from_slice(&CELL_MAGIC_V2);
        output[8..10].copy_from_slice(&CELL_VERSION_V2.to_le_bytes());
        output[10] = self.phase.byte();
        output[11] = self.disposition.byte();
        output[12] = self.attempt_index;
        let mut cursor = HEADER_BYTES_V2;
        for amount in [
            self.generation,
            self.work_rent_principal_lamports,
            self.completed_session_count,
            self.transition_nonce,
            self.accepted_progress_units,
            self.completed_work_calls,
            self.exact_reward_lamports,
        ] {
            put_u64(output, &mut cursor, amount)?;
        }
        for id in [
            self.failure_policy_binding_id.bytes(),
            self.market_instance_id.bytes(),
            self.funding_receipt_id.bytes(),
            self.history_account.bytes(),
            self.rent_refund_owner.bytes(),
            self.neutral_sink.bytes(),
            self.session_binding_id.bytes(),
            self.source_handoff_id.bytes(),
            self.session_schedule_id.bytes(),
            self.quote_admission_receipt_id.bytes(),
            self.last_transition_receipt_id.bytes(),
            self.last_liveness_work_receipt_id.bytes(),
            self.terminal_receipt_id.bytes(),
        ] {
            put_id(output, &mut cursor, id)?;
        }
        let end = cursor
            .checked_add(QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1)
            .ok_or(Error::WrongLength)?;
        output
            .get_mut(cursor..end)
            .ok_or(Error::WrongLength)?
            .copy_from_slice(&self.product_work_body);
        cursor = end;
        if output[cursor..].iter().any(|byte| *byte != 0) {
            return Err(Error::WrongLength);
        }
        Ok(())
    }

    /// Hostile-decode and fully validate the semantic owner's canonical body.
    /// Cross-account admission, funding, history, and quote authority remains
    /// a separate mandatory join for execution.
    pub fn decode_canonical(
        input: &[u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
    ) -> Result<Self> {
        if input[..8] != CELL_MAGIC_V2 {
            return Err(Error::BadMagic);
        }
        if input[8..10] != CELL_VERSION_V2.to_le_bytes() {
            return Err(Error::BadVersion);
        }
        if input[13..HEADER_BYTES_V2].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        let phase = FailureMarketIntervalCellPhaseV2::decode(input[10])?;
        let disposition = FailureMarketIntervalCellDispositionV2::decode(input[11])?;
        let attempt_index = input[12];
        let mut cursor = HEADER_BYTES_V2;
        let generation = take_u64(input, &mut cursor)?;
        let work_rent_principal_lamports = take_u64(input, &mut cursor)?;
        let completed_session_count = take_u64(input, &mut cursor)?;
        let transition_nonce = take_u64(input, &mut cursor)?;
        let accepted_progress_units = take_u64(input, &mut cursor)?;
        let completed_work_calls = take_u64(input, &mut cursor)?;
        let exact_reward_lamports = take_u64(input, &mut cursor)?;
        let failure_policy_binding_id =
            FailurePolicyBindingId::from_bytes(take_id(input, &mut cursor)?);
        let market_instance_id = MarketInstanceV2Id::from_bytes(take_id(input, &mut cursor)?);
        let funding_receipt_id =
            FailureMarketIntervalFundingReceiptIdV2::from_bytes(take_id(input, &mut cursor)?);
        let history_account = FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?);
        let rent_refund_owner = FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?);
        let neutral_sink = FailureMarketAccountIdV1::from_bytes(take_id(input, &mut cursor)?);
        let session_binding_id = SourceContentId::from_bytes(take_id(input, &mut cursor)?);
        let source_handoff_id = SourceContentId::from_bytes(take_id(input, &mut cursor)?);
        let session_schedule_id = SourceContentId::from_bytes(take_id(input, &mut cursor)?);
        let quote_admission_receipt_id = SourceContentId::from_bytes(take_id(input, &mut cursor)?);
        let last_transition_receipt_id = SourceContentId::from_bytes(take_id(input, &mut cursor)?);
        let last_liveness_work_receipt_id =
            SourceContentId::from_bytes(take_id(input, &mut cursor)?);
        let terminal_receipt_id = SourceContentId::from_bytes(take_id(input, &mut cursor)?);
        let mut product_work_body = [0; QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1];
        let end = cursor
            .checked_add(QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1)
            .ok_or(Error::WrongLength)?;
        product_work_body.copy_from_slice(input.get(cursor..end).ok_or(Error::WrongLength)?);
        cursor = end;
        if input[cursor..].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            phase,
            disposition,
            attempt_index,
            generation,
            work_rent_principal_lamports,
            completed_session_count,
            transition_nonce,
            accepted_progress_units,
            completed_work_calls,
            exact_reward_lamports,
            failure_policy_binding_id,
            market_instance_id,
            funding_receipt_id,
            history_account,
            rent_refund_owner,
            neutral_sink,
            session_binding_id,
            source_handoff_id,
            session_schedule_id,
            quote_admission_receipt_id,
            last_transition_receipt_id,
            last_liveness_work_receipt_id,
            terminal_receipt_id,
            product_work_body,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode against exact authenticated admission, capitalization,
    /// history, and shared quote receipts.
    pub fn decode_for_admission(
        input: &[u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
        admission: FailureMarketAdmissionStateV1,
        funding: FailureMarketIntervalFundingReceiptV2,
        history: FailureMarketIntervalHistoryV2,
        quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    ) -> Result<Self> {
        let value = Self::decode_canonical(input)?;
        value.validate_against(admission, funding, history, quote)?;
        Ok(value)
    }

    /// Stale-checked commit of one cell plan.
    pub fn commit_plan(&mut self, plan: FailureMarketIntervalCellPlanV2) -> Result<()> {
        self.validate()?;
        if *self != plan.before {
            return Err(Error::StalePlan);
        }
        plan.after.validate()?;
        *self = plan.after;
        Ok(())
    }

    fn validate(self) -> Result<()> {
        for id in [
            self.failure_policy_binding_id.bytes(),
            self.market_instance_id.bytes(),
            self.funding_receipt_id.bytes(),
            self.history_account.bytes(),
            self.rent_refund_owner.bytes(),
            self.neutral_sink.bytes(),
            self.quote_admission_receipt_id.bytes(),
        ] {
            require_live(id)?;
        }
        if self.generation == 0
            || self.work_rent_principal_lamports == 0
            || self.history_account == self.rent_refund_owner
            || self.history_account == self.neutral_sink
            || self.rent_refund_owner == self.neutral_sink
        {
            return Err(Error::BindingMismatch);
        }
        let session_ids = [
            self.session_binding_id,
            self.source_handoff_id,
            self.session_schedule_id,
        ];
        let no_work = self.product_work_body.iter().all(|byte| *byte == 0);
        match self.phase {
            FailureMarketIntervalCellPhaseV2::Idle => {
                if self.disposition != FailureMarketIntervalCellDispositionV2::None
                    || self.attempt_index != 0
                    || session_ids.iter().any(|id| !id.is_zero())
                    || !self.last_transition_receipt_id.is_zero()
                    || !self.last_liveness_work_receipt_id.is_zero()
                    || !self.terminal_receipt_id.is_zero()
                    || self.transition_nonce != 0
                    || self.accepted_progress_units != 0
                    || self.completed_work_calls != 0
                    || self.exact_reward_lamports != 0
                    || !no_work
                {
                    return Err(Error::WrongPhase);
                }
            }
            FailureMarketIntervalCellPhaseV2::Active
            | FailureMarketIntervalCellPhaseV2::Resolved => {
                if session_ids.iter().any(|id| id.is_zero()) {
                    return Err(Error::WrongPhase);
                }
                let expected_attempt = u8::try_from(self.completed_session_count)
                    .map_err(|_| Error::BindingMismatch)?;
                if self.attempt_index != expected_attempt {
                    return Err(Error::BindingMismatch);
                }
                let direct_source_failure = self.phase == FailureMarketIntervalCellPhaseV2::Resolved
                    && matches!(
                        self.disposition,
                        FailureMarketIntervalCellDispositionV2::SourceAbsent
                            | FailureMarketIntervalCellDispositionV2::SourceRefused
                    );
                if direct_source_failure {
                    if !no_work
                        || self.transition_nonce != 0
                        || self.accepted_progress_units != 0
                        || self.completed_work_calls != 0
                        || self.exact_reward_lamports != 0
                        || !self.last_transition_receipt_id.is_zero()
                        || !self.last_liveness_work_receipt_id.is_zero()
                        || self.terminal_receipt_id.is_zero()
                    {
                        return Err(Error::BindingMismatch);
                    }
                    return Ok(());
                }
                if no_work {
                    return Err(Error::WrongPhase);
                }
                let work = QuantizedIntervalConsensusWorkV1::decode(&self.product_work_body)?;
                if work.market_instance_id() != self.market_instance_id
                    || work.checked_coordinates() != self.accepted_progress_units
                    || self.transition_nonce != self.completed_work_calls
                {
                    return Err(Error::BindingMismatch);
                }
                let advanced = self.transition_nonce != 0;
                let complete_advance = !self.last_transition_receipt_id.is_zero()
                    && !self.last_liveness_work_receipt_id.is_zero()
                    && self.accepted_progress_units != 0
                    && self.completed_work_calls != 0
                    && self.exact_reward_lamports != 0;
                let any_advance = !self.last_transition_receipt_id.is_zero()
                    || !self.last_liveness_work_receipt_id.is_zero()
                    || self.accepted_progress_units != 0
                    || self.completed_work_calls != 0
                    || self.exact_reward_lamports != 0;
                if (advanced && !complete_advance) || (!advanced && any_advance) {
                    return Err(Error::BindingMismatch);
                }
                if self.phase == FailureMarketIntervalCellPhaseV2::Active
                    && (self.disposition != FailureMarketIntervalCellDispositionV2::None
                        || !self.terminal_receipt_id.is_zero())
                {
                    return Err(Error::WrongPhase);
                }
                if self.phase == FailureMarketIntervalCellPhaseV2::Resolved
                    && (self.disposition == FailureMarketIntervalCellDispositionV2::None
                        || self.terminal_receipt_id.is_zero())
                {
                    return Err(Error::WrongPhase);
                }
                if self.phase == FailureMarketIntervalCellPhaseV2::Resolved
                    && ((self.disposition == FailureMarketIntervalCellDispositionV2::Resolved
                        && !work.is_complete())
                        || (self.disposition != FailureMarketIntervalCellDispositionV2::Resolved
                            && work.is_complete()))
                {
                    return Err(Error::BindingMismatch);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_against(
        self,
        admission: FailureMarketAdmissionStateV1,
        funding: FailureMarketIntervalFundingReceiptV2,
        history: FailureMarketIntervalHistoryV2,
        quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    ) -> Result<()> {
        self.validate()?;
        let policy = admission.binding().facts();
        let funding_facts = funding.facts();
        let quote_facts = quote.facts();
        let aggregate_calls = history
            .completed_work_calls()
            .checked_add(self.completed_work_calls)
            .ok_or(Error::BindingMismatch)?;
        let aggregate_rewards = history
            .exact_reward_lamports()
            .checked_add(self.exact_reward_lamports)
            .ok_or(Error::BindingMismatch)?;
        if self.failure_policy_binding_id != admission.binding().id()
            || self.market_instance_id != policy.market_instance_id
            || self.generation != policy.generation
            || self.funding_receipt_id != funding.id()
            || self.history_account != funding_facts.history_account
            || self.rent_refund_owner != funding_facts.rent_refund_owner
            || self.neutral_sink != funding_facts.neutral_sink
            || self.work_rent_principal_lamports != funding_facts.work_rent_principal_lamports
            || history.failure_policy_binding_id() != self.failure_policy_binding_id
            || history.market_instance_id() != self.market_instance_id
            || history.generation() != self.generation
            || history.funding_receipt_id() != self.funding_receipt_id
            || history.history_account() != self.history_account
            || history.completed_session_count() != self.completed_session_count
            || self.quote_admission_receipt_id.bytes() != quote.id().bytes()
            || history.quote_admission_receipt_id().bytes()
                != self.quote_admission_receipt_id.bytes()
            || quote_facts.failure_policy_binding_id != self.failure_policy_binding_id
            || aggregate_calls > u64::from(quote_facts.maximum_calls)
            || aggregate_rewards > quote_facts.work_principal_lamports
            || (self.phase != FailureMarketIntervalCellPhaseV2::Idle
                && usize::from(self.attempt_index) >= usize::from(quote.schedule().attempt_count))
        {
            return Err(Error::BindingMismatch);
        }
        if self.phase != FailureMarketIntervalCellPhaseV2::Idle
            && self.accepted_progress_units
                > quote.schedule().attempts[usize::from(self.attempt_index)].max_progress_units
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Initialize the canonical Idle cell from the same private capitalization as
/// its paired append-only history.
pub fn initialize_failure_market_interval_cell_v2(
    admission: FailureMarketAdmissionStateV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    history: FailureMarketIntervalHistoryV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
) -> Result<FailureMarketIntervalCellV2> {
    let policy = admission.binding().facts();
    let facts = funding.facts();
    let value = FailureMarketIntervalCellV2 {
        phase: FailureMarketIntervalCellPhaseV2::Idle,
        disposition: FailureMarketIntervalCellDispositionV2::None,
        attempt_index: 0,
        generation: policy.generation,
        work_rent_principal_lamports: facts.work_rent_principal_lamports,
        completed_session_count: 0,
        transition_nonce: 0,
        accepted_progress_units: 0,
        completed_work_calls: 0,
        exact_reward_lamports: 0,
        failure_policy_binding_id: admission.binding().id(),
        market_instance_id: policy.market_instance_id,
        funding_receipt_id: funding.id(),
        history_account: facts.history_account,
        rent_refund_owner: facts.rent_refund_owner,
        neutral_sink: facts.neutral_sink,
        session_binding_id: SourceContentId::ZERO,
        source_handoff_id: SourceContentId::ZERO,
        session_schedule_id: SourceContentId::ZERO,
        quote_admission_receipt_id: SourceContentId::from_bytes(quote.id().bytes()),
        last_transition_receipt_id: SourceContentId::ZERO,
        last_liveness_work_receipt_id: SourceContentId::ZERO,
        terminal_receipt_id: SourceContentId::ZERO,
        product_work_body: [0; QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1],
    };
    value.validate_against(admission, funding, history, quote)?;
    Ok(value)
}

/// Complete expected Idle-to-Active join. This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellActivationFactsV2 {
    /// Exact Idle cell prestate.
    pub cell_before: FailureMarketIntervalCellStateIdV2,
    /// Exact paired history root, possibly zero before the first session.
    pub history_root: FailureMarketIntervalHistoryRootV2,
    /// Exact one-based history count before this new attempt.
    pub completed_session_count: u64,
    /// Product/Series link pin for this subordinate session.
    pub session_binding_id: SourceContentId,
    /// Authenticated Source successful-evaluation handoff.
    pub source_handoff_id: SourceContentId,
    /// Exact link-scoped Source repair generation; intentionally distinct
    /// from the shared Failure/liveness generation.
    pub source_repair_generation: u64,
    /// Per-Series absolute attempt/window schedule.
    pub session_schedule_id: SourceContentId,
    /// Derived zero-based Market recovery attempt row.
    pub attempt_index: u8,
    /// Exact initial Product structural-work identity.
    pub product_work_id: clutch_product_series::QuantizedIntervalConsensusWorkV1Id,
}

/// Private Product/Source/link authority for one session activation.
pub trait AuthenticatedFailureMarketIntervalCellActivationV2 {
    /// Authenticate the Product link, Source account handoff, per-Series
    /// schedule row, central capability profile, and exact initial work.
    fn authenticate_failure_market_interval_cell_activation(
        &self,
        _expected: FailureMarketIntervalCellActivationFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private activation receipt retained by the shared runtime and Product link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellActivationReceiptV2 {
    id: FailureMarketIntervalCellActivationReceiptIdV2,
    facts: FailureMarketIntervalCellActivationFactsV2,
    cell_after: FailureMarketIntervalCellStateIdV2,
}

impl FailureMarketIntervalCellActivationReceiptV2 {
    /// Exact activation identity.
    pub const fn id(self) -> FailureMarketIntervalCellActivationReceiptIdV2 {
        self.id
    }

    /// Complete authenticated activation facts.
    pub const fn facts(self) -> FailureMarketIntervalCellActivationFactsV2 {
        self.facts
    }

    /// Exact Active cell poststate.
    pub const fn cell_after(self) -> FailureMarketIntervalCellStateIdV2 {
        self.cell_after
    }
}

/// One stale-checked reusable-cell state transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellPlanV2 {
    before: FailureMarketIntervalCellV2,
    after: FailureMarketIntervalCellV2,
}

impl FailureMarketIntervalCellPlanV2 {
    /// Complete resulting cell poststate.
    pub const fn resulting_cell(self) -> FailureMarketIntervalCellV2 {
        self.after
    }
}

/// Begin one exact Product interval scan in the canonical Idle cell.
#[allow(clippy::too_many_arguments)]
pub fn plan_activate_failure_market_interval_cell_v2<
    A: AuthenticatedFailureMarketIntervalCellActivationV2 + ?Sized,
>(
    authority: &A,
    cell: FailureMarketIntervalCellV2,
    admission: FailureMarketAdmissionStateV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    history: FailureMarketIntervalHistoryV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    session_binding_id: SourceContentId,
    session_schedule_id: SourceContentId,
    source_success: SuccessfulEvaluationHandoffV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
) -> Result<(
    FailureMarketIntervalCellPlanV2,
    FailureMarketIntervalCellActivationReceiptV2,
)> {
    cell.validate_against(admission, funding, history, quote)?;
    if cell.phase != FailureMarketIntervalCellPhaseV2::Idle
        || history.family_terminal_receipt_id().bytes() != [0; 32]
    {
        return Err(Error::WrongPhase);
    }
    require_live(session_binding_id.bytes())?;
    require_live(session_schedule_id.bytes())?;
    let quote_facts = quote.facts();
    let attempt_index =
        u8::try_from(cell.completed_session_count).map_err(|_| Error::BindingMismatch)?;
    if usize::from(attempt_index) >= usize::from(quote.schedule().attempt_count)
        || source_success.failure_policy_binding_id().bytes()
            != cell.failure_policy_binding_id.bytes()
        || source_success.occurrence().market_instance_id().bytes()
            != cell.market_instance_id.bytes()
        || quote_facts.failure_policy_binding_id != cell.failure_policy_binding_id
    {
        return Err(Error::BindingMismatch);
    }
    let product_session = begin_quantized_interval_consensus_v1(context)?;
    let product_work = *product_session.work();
    let policy = admission.binding().facts();
    if product_work.market_instance_id() != cell.market_instance_id
        || product_work.product_template_id() != policy.product_template_id
        || product_work.market_genesis_profile_id() != policy.market_genesis_profile_id
        || product_work.native_claim_basis_id() != policy.native_claim_basis_id
        || product_work.price_measure_policy_id() != policy.price_measure_policy_id
        || product_work.capability_profile_id().bytes() != policy.capability_profile_id.bytes()
        || product_work.interval_profile_id() != policy.interval_consensus_profile_id
        || product_work.source_interval_id().bytes()
            != source_success.statistic_result_id()?.bytes()
        || product_work.source_occurrence_id().bytes()
            != source_success.occurrence().occurrence_record_id().bytes()
        || product_work.checked_coordinates() != 0
    {
        return Err(Error::BindingMismatch);
    }
    let product_work_id = product_work.id()?;
    let cell_before = cell.id()?;
    let facts = FailureMarketIntervalCellActivationFactsV2 {
        cell_before,
        history_root: history.history_root(),
        completed_session_count: cell.completed_session_count,
        session_binding_id,
        source_handoff_id: source_success.id(),
        source_repair_generation: source_success.occurrence().repair_generation(),
        session_schedule_id,
        attempt_index,
        product_work_id,
    };
    authority.authenticate_failure_market_interval_cell_activation(facts)?;
    let mut after = cell;
    after.phase = FailureMarketIntervalCellPhaseV2::Active;
    after.attempt_index = attempt_index;
    after.session_binding_id = session_binding_id;
    after.source_handoff_id = source_success.id();
    after.session_schedule_id = session_schedule_id;
    product_work.encode_into(&mut after.product_work_body)?;
    after.validate_against(admission, funding, history, quote)?;
    let cell_after = after.id()?;
    let mut hasher = Sha256::new();
    hasher.update(CELL_ACTIVATION_DOMAIN_V2);
    hasher.update(cell_before.bytes());
    hasher.update(cell_after.bytes());
    hasher.update(history.id()?.bytes());
    hasher.update(quote.id().bytes());
    hasher.update(session_binding_id.bytes());
    hasher.update(source_success.id().bytes());
    hasher.update(
        source_success
            .occurrence()
            .repair_generation()
            .to_le_bytes(),
    );
    hasher.update(session_schedule_id.bytes());
    hasher.update([attempt_index]);
    hasher.update(product_work_id.bytes());
    let receipt = FailureMarketIntervalCellActivationReceiptV2 {
        id: FailureMarketIntervalCellActivationReceiptIdV2::from_bytes(hasher.finalize().into()),
        facts,
        cell_after,
    };
    require_live(receipt.id.bytes())?;
    Ok((
        FailureMarketIntervalCellPlanV2 {
            before: cell,
            after,
        },
        receipt,
    ))
}

/// Complete expected Product/liveness join for one bounded paid advance.
/// This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellAdvanceFactsV2 {
    /// Exact Active cell prestate.
    pub cell_before: FailureMarketIntervalCellStateIdV2,
    /// Exact Active cell poststate.
    pub cell_after: FailureMarketIntervalCellStateIdV2,
    /// Exact append-only history prestate left unchanged by this call.
    pub history_state: crate::market_interval_history_v2::FailureMarketIntervalHistoryStateIdV2,
    /// Product structural-work preimage identity.
    pub product_work_before: QuantizedIntervalConsensusWorkV1Id,
    /// Product structural-work postimage identity.
    pub product_work_after: QuantizedIntervalConsensusWorkV1Id,
    /// Exact Product-reported bounded progress.
    pub processed_coordinates: u16,
    /// Current Market quote attempt row.
    pub attempt_index: u8,
    /// Progress in the current attempt before this call.
    pub accepted_progress_before: u64,
    /// Progress in the current attempt after this call.
    pub accepted_progress_after: u64,
    /// One-based shared liveness call ordinal across all archived sessions.
    pub call_ordinal: u32,
    /// Sole work/reward recipient authenticated by the adapter.
    pub reward_recipient: LivenessId,
    /// Exact reward and liveness debit ceiling; these are deliberately equal.
    pub exact_reward_lamports: u64,
    /// Family receipt identity passed into the liveness runtime.
    pub work_authorization_id: FailureMarketIntervalCellWorkAuthorizationIdV2,
    /// Exact receipt identity persisted by the liveness runtime; this is the
    /// explicit typed projection of `work_authorization_id`, not a second
    /// liveness-owned receipt truth.
    pub runtime_work_receipt_id: LivenessId,
}

/// Private liveness adapter authority for one exact priced work transition.
pub trait AuthenticatedFailureMarketIntervalCellAdvanceV2 {
    /// Authenticate the Recovery compartment prestate, intent, receipt account,
    /// keeper, exact debit/payment, and atomic Product/Failure postwrites.
    fn authenticate_failure_market_interval_cell_advance(
        &self,
        _expected: FailureMarketIntervalCellAdvanceFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private-field joined receipt for one Product/liveness advance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellAdvanceReceiptV2 {
    id: FailureMarketIntervalCellAdvanceReceiptIdV2,
    facts: FailureMarketIntervalCellAdvanceFactsV2,
}

impl FailureMarketIntervalCellAdvanceReceiptV2 {
    /// Complete joined advance identity.
    pub const fn id(self) -> FailureMarketIntervalCellAdvanceReceiptIdV2 {
        self.id
    }

    /// Complete exact joined facts.
    pub const fn facts(self) -> FailureMarketIntervalCellAdvanceFactsV2 {
        self.facts
    }
}

/// One exact priced Product structural-work transition.
#[derive(Clone, Copy, Debug)]
pub struct FailureMarketIntervalCellAdvancePlanV2 {
    cell_plan: FailureMarketIntervalCellPlanV2,
    next_work: QuantizedIntervalConsensusWorkV1,
    progress: QuantizedIntervalConsensusProgressV1,
    receipt: FailureMarketIntervalCellAdvanceReceiptV2,
}

impl FailureMarketIntervalCellAdvancePlanV2 {
    /// Complete resulting reusable cell.
    pub const fn resulting_cell(&self) -> FailureMarketIntervalCellV2 {
        self.cell_plan.after
    }

    /// Exact Product work postimage for the same atomic batch.
    pub const fn next_work(&self) -> QuantizedIntervalConsensusWorkV1 {
        self.next_work
    }

    /// Exact Product-reported progress.
    pub const fn progress(&self) -> QuantizedIntervalConsensusProgressV1 {
        self.progress
    }

    /// Private joined Product/liveness receipt.
    pub const fn receipt(&self) -> FailureMarketIntervalCellAdvanceReceiptV2 {
        self.receipt
    }

    /// Stale-checked reusable-cell plan.
    pub const fn cell_plan(&self) -> FailureMarketIntervalCellPlanV2 {
        self.cell_plan
    }
}

/// Advance one active session by the exact Product progress and Market-owned
/// quote. No caller-supplied ceiling or alternate payout destination exists.
#[allow(clippy::too_many_arguments)]
pub fn plan_advance_failure_market_interval_cell_v2<
    A: AuthenticatedFailureMarketIntervalCellAdvanceV2 + ?Sized,
>(
    authority: &A,
    cell: FailureMarketIntervalCellV2,
    admission: FailureMarketAdmissionStateV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    history: FailureMarketIntervalHistoryV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    source_success: SuccessfulEvaluationHandoffV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
    requested_coordinates: u16,
    reward_recipient: LivenessId,
) -> Result<FailureMarketIntervalCellAdvancePlanV2> {
    cell.validate_against(admission, funding, history, quote)?;
    if cell.phase != FailureMarketIntervalCellPhaseV2::Active
        || source_success.id() != cell.source_handoff_id
        || source_success.failure_policy_binding_id().bytes()
            != cell.failure_policy_binding_id.bytes()
        || reward_recipient.is_zero()
    {
        return Err(Error::WrongPhase);
    }
    let current_work = cell.product_work()?.ok_or(Error::WrongPhase)?;
    let product_work_before = current_work.id()?;
    let (next_work, progress) = advance_quantized_interval_consensus_work_v1(
        &current_work,
        context,
        requested_coordinates,
    )?;
    let product_work_after = next_work.id()?;
    let accepted_progress_before = cell.accepted_progress_units;
    let accepted_progress_after = accepted_progress_before
        .checked_add(u64::from(progress.processed_coordinates))
        .ok_or(Error::BindingMismatch)?;
    let exact_reward_lamports = quote.schedule().exact_progress_reward_lamports(
        cell.attempt_index,
        accepted_progress_before,
        accepted_progress_after,
    )?;
    let next_session_calls = cell
        .completed_work_calls
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let aggregate_calls = history
        .completed_work_calls()
        .checked_add(next_session_calls)
        .ok_or(Error::BindingMismatch)?;
    let aggregate_rewards = history
        .exact_reward_lamports()
        .checked_add(cell.exact_reward_lamports)
        .and_then(|value| value.checked_add(exact_reward_lamports))
        .ok_or(Error::BindingMismatch)?;
    let quote_facts = quote.facts();
    if aggregate_calls > u64::from(quote_facts.maximum_calls)
        || aggregate_rewards > quote_facts.work_principal_lamports
    {
        return Err(Error::BindingMismatch);
    }
    let call_ordinal = u32::try_from(aggregate_calls).map_err(|_| Error::BindingMismatch)?;
    let next_nonce = cell
        .transition_nonce
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    let cell_before = cell.id()?;
    let history_state = history.id()?;
    let mut authorization_hasher = Sha256::new();
    authorization_hasher.update(CELL_WORK_AUTHORIZATION_DOMAIN_V2);
    authorization_hasher.update(cell.failure_policy_binding_id.bytes());
    authorization_hasher.update(cell.market_instance_id.bytes());
    authorization_hasher.update(cell.generation.to_le_bytes());
    authorization_hasher.update(cell_before.bytes());
    authorization_hasher.update(history_state.bytes());
    authorization_hasher.update(product_work_before.bytes());
    authorization_hasher.update(product_work_after.bytes());
    authorization_hasher.update(cell.transition_nonce.to_le_bytes());
    authorization_hasher.update(next_nonce.to_le_bytes());
    authorization_hasher.update(progress.processed_coordinates.to_le_bytes());
    authorization_hasher.update([cell.attempt_index]);
    authorization_hasher.update(accepted_progress_before.to_le_bytes());
    authorization_hasher.update(accepted_progress_after.to_le_bytes());
    authorization_hasher.update(call_ordinal.to_le_bytes());
    authorization_hasher.update(reward_recipient.bytes());
    authorization_hasher.update(exact_reward_lamports.to_le_bytes());
    let work_authorization_id = FailureMarketIntervalCellWorkAuthorizationIdV2::from_bytes(
        authorization_hasher.finalize().into(),
    );
    require_live(work_authorization_id.bytes())?;
    let runtime_work_receipt_id = LivenessId::from_bytes(work_authorization_id.bytes());
    let mut receipt_hasher = Sha256::new();
    receipt_hasher.update(CELL_ADVANCE_DOMAIN_V2);
    receipt_hasher.update(cell_before.bytes());
    receipt_hasher.update(history_state.bytes());
    receipt_hasher.update(product_work_before.bytes());
    receipt_hasher.update(product_work_after.bytes());
    receipt_hasher.update(progress.processed_coordinates.to_le_bytes());
    receipt_hasher.update([cell.attempt_index]);
    receipt_hasher.update(accepted_progress_before.to_le_bytes());
    receipt_hasher.update(accepted_progress_after.to_le_bytes());
    receipt_hasher.update(call_ordinal.to_le_bytes());
    receipt_hasher.update(reward_recipient.bytes());
    receipt_hasher.update(exact_reward_lamports.to_le_bytes());
    receipt_hasher.update(work_authorization_id.bytes());
    let receipt_id =
        FailureMarketIntervalCellAdvanceReceiptIdV2::from_bytes(receipt_hasher.finalize().into());
    require_live(receipt_id.bytes())?;
    let mut after = cell;
    after.transition_nonce = next_nonce;
    after.accepted_progress_units = accepted_progress_after;
    after.completed_work_calls = next_session_calls;
    after.exact_reward_lamports = cell
        .exact_reward_lamports
        .checked_add(exact_reward_lamports)
        .ok_or(Error::BindingMismatch)?;
    after.last_transition_receipt_id = SourceContentId::from_bytes(receipt_id.bytes());
    after.last_liveness_work_receipt_id =
        SourceContentId::from_bytes(runtime_work_receipt_id.bytes());
    next_work.encode_into(&mut after.product_work_body)?;
    after.validate_against(admission, funding, history, quote)?;
    let cell_after = after.id()?;
    let facts = FailureMarketIntervalCellAdvanceFactsV2 {
        cell_before,
        cell_after,
        history_state,
        product_work_before,
        product_work_after,
        processed_coordinates: progress.processed_coordinates,
        attempt_index: cell.attempt_index,
        accepted_progress_before,
        accepted_progress_after,
        call_ordinal,
        reward_recipient,
        exact_reward_lamports,
        work_authorization_id,
        runtime_work_receipt_id,
    };
    authority.authenticate_failure_market_interval_cell_advance(facts)?;
    let receipt = FailureMarketIntervalCellAdvanceReceiptV2 {
        id: receipt_id,
        facts,
    };
    Ok(FailureMarketIntervalCellAdvancePlanV2 {
        cell_plan: FailureMarketIntervalCellPlanV2 {
            before: cell,
            after,
        },
        next_work,
        progress,
        receipt,
    })
}

/// Exact private resolution facts consumed by the Product/Resolution writer.
/// This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellResolutionFactsV2 {
    /// Exact complete Active cell prestate.
    pub cell_before: FailureMarketIntervalCellStateIdV2,
    /// Exact terminal Resolved cell poststate.
    pub cell_after: FailureMarketIntervalCellStateIdV2,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Exclusive Product/Series link pin.
    pub session_binding_id: SourceContentId,
    /// Exact Source successful-evaluation handoff.
    pub source_handoff_id: SourceContentId,
    /// Complete terminal Product structural-work identity.
    pub terminal_work_id: QuantizedIntervalConsensusWorkV1Id,
    /// Exact exhaustive Product certificate.
    pub product_certificate_id: QuantizedIntervalConsensusCertificateV1Id,
    /// Last exact family work receipt persisted by the liveness runtime, or
    /// zero for a zero-work terminal.
    pub last_runtime_work_receipt_id: LivenessId,
    /// Final bounded call count in this session.
    pub completed_work_calls: u64,
    /// Final exact keeper rewards in this session.
    pub exact_reward_lamports: u64,
}

/// Private account/runtime authority for exhaustive resolution restoration.
pub trait AuthenticatedFailureMarketIntervalCellResolutionV2:
    AuthenticatedQuantizedIntervalConsensusHistoryV1
{
    /// Authenticate the exact 0xab transition chain, current Source handoff,
    /// Product link/root, and once-only Resolution V5 writer prestate.
    fn authenticate_failure_market_interval_cell_resolution(
        &self,
        _expected: FailureMarketIntervalCellResolutionFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private exhaustive resolution capability and durable cell receipt.
#[derive(Clone, Copy, Debug)]
pub struct FailureMarketIntervalCellResolutionReceiptV2 {
    id: FailureMarketIntervalCellResolutionReceiptIdV2,
    failure_policy_binding_id: FailurePolicyBindingId,
    facts: FailureMarketIntervalCellResolutionFactsV2,
    verified_payout: VerifiedQuantizedIntervalPayoutV1,
}

impl FailureMarketIntervalCellResolutionReceiptV2 {
    /// Complete private resolution identity.
    pub const fn id(self) -> FailureMarketIntervalCellResolutionReceiptIdV2 {
        self.id
    }

    /// Exact shared Failure policy.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Complete authenticated resolution facts.
    pub const fn facts(self) -> FailureMarketIntervalCellResolutionFactsV2 {
        self.facts
    }

    /// Private Product exhaustive-payout capability. The Product/Resolution
    /// writer must consume this in the same atomic batch as the cell postwrite.
    pub const fn verified_payout(self) -> VerifiedQuantizedIntervalPayoutV1 {
        self.verified_payout
    }
}

/// One exhaustive Product resolution and exact terminal cell postwrite.
#[derive(Clone, Copy, Debug)]
pub struct FailureMarketIntervalCellResolutionPlanV2 {
    cell_plan: FailureMarketIntervalCellPlanV2,
    receipt: FailureMarketIntervalCellResolutionReceiptV2,
}

impl FailureMarketIntervalCellResolutionPlanV2 {
    /// Complete terminal reusable-cell poststate.
    pub const fn resulting_cell(&self) -> FailureMarketIntervalCellV2 {
        self.cell_plan.after
    }

    /// Private Product/Failure resolution receipt.
    pub const fn receipt(&self) -> FailureMarketIntervalCellResolutionReceiptV2 {
        self.receipt
    }

    /// Stale-checked cell plan.
    pub const fn cell_plan(&self) -> FailureMarketIntervalCellPlanV2 {
        self.cell_plan
    }
}

/// Complete Source-owned failure attempt admitted without Product work.
///
/// This projection is not authority. The adapter must authenticate the
/// persisted Source handoff and its exact physical terminal postwrite before
/// it may accept these facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellSourceFailureFactsV2 {
    /// Exact Idle cell prestate.
    pub cell_before: FailureMarketIntervalCellStateIdV2,
    /// Exact paired append-only history prestate.
    pub history_before: crate::market_interval_history_v2::FailureMarketIntervalHistoryStateIdV2,
    /// Exact prior append-only root.
    pub history_root: FailureMarketIntervalHistoryRootV2,
    /// Zero-based finite attempt row consumed by this failure.
    pub attempt_index: u8,
    /// Exact Product post-pin transcript retained through archive.
    pub session_binding_id: SourceContentId,
    /// Canonical per-Series schedule projection for this attempt.
    pub session_schedule_id: SourceContentId,
    /// Source-owned absence versus stable-refusal classification.
    pub source_kind: SourceFailureKindV1,
    /// Exact Source semantic handoff identity.
    pub source_handoff_id: SourceContentId,
    /// Full release/account/result-or-absence/work authentication join.
    pub source_join_id: SourceContentId,
    /// Product/Series occurrence identity which selected this Source attempt.
    pub source_occurrence_id: SourceContentId,
    /// Physical occurrence account retained by the Source join.
    pub source_occurrence_account: SourceRuntimeKey,
    /// Physical predictable StatisticResult slot, created or never created.
    pub result_or_absence_account: SourceRuntimeKey,
    /// Authenticated absence or refused-result account fact.
    pub source_fact_authentication_id: SourceContentId,
    /// Exact Source FailureHandoff work-receipt authentication.
    pub source_work_receipt_authentication_id: SourceContentId,
    /// Exact repair generation selected by the Series occurrence.
    pub source_repair_generation: u64,
    /// Exact evaluated Window identity.
    pub window_id: SourceContentId,
    /// Exact StatisticKey identity.
    pub statistic_key_id: SourceContentId,
    /// Evidence identity, zero only for mature absence.
    pub window_evidence_id: SourceContentId,
    /// StatisticResult identity, zero only for mature absence.
    pub statistic_result_id: SourceContentId,
    /// Stable refusal code, zero only for mature absence.
    pub refusal_code: u32,
    /// Source-owned physical terminal/tombstone postwrite for this attempt.
    pub source_terminal_postwrite_id: SourceContentId,
}

/// Private Source/Product/link authority for one direct failure attempt.
pub trait AuthenticatedFailureMarketIntervalCellSourceFailureV2 {
    /// Authenticate exact current Source state, finite schedule row, Product
    /// link pin, and Source terminal postwrite.
    fn authenticate_failure_market_interval_cell_source_failure(
        &self,
        _expected: FailureMarketIntervalCellSourceFailureFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private receipt for one zero-payout Source failure terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellSourceFailureReceiptV2 {
    id: FailureMarketIntervalCellSourceFailureReceiptIdV2,
    facts: FailureMarketIntervalCellSourceFailureFactsV2,
    cell_after: FailureMarketIntervalCellStateIdV2,
}

impl FailureMarketIntervalCellSourceFailureReceiptV2 {
    /// Exact terminal receipt identity.
    pub const fn id(self) -> FailureMarketIntervalCellSourceFailureReceiptIdV2 {
        self.id
    }

    /// Complete authenticated Source failure facts.
    pub const fn facts(self) -> FailureMarketIntervalCellSourceFailureFactsV2 {
        self.facts
    }

    /// Exact SourceAbsent or SourceRefused cell poststate.
    pub const fn cell_after(self) -> FailureMarketIntervalCellStateIdV2 {
        self.cell_after
    }
}

/// Consume one exact finite Source attempt as an evidence-only zero-payout
/// terminal. No Product work, liveness call, reward, or resolution is minted.
#[allow(clippy::too_many_arguments)]
pub fn plan_refuse_failure_market_interval_cell_v2<
    A: AuthenticatedFailureMarketIntervalCellSourceFailureV2 + ?Sized,
>(
    authority: &A,
    cell: FailureMarketIntervalCellV2,
    admission: FailureMarketAdmissionStateV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    history: FailureMarketIntervalHistoryV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    session_binding_id: SourceContentId,
    session_schedule_id: SourceContentId,
    source_failure: FailurePolicySourceHandoffV1,
    source_join: SourcePolicyHandoffJoinV1,
    source_terminal_postwrite_id: SourceContentId,
) -> Result<(
    FailureMarketIntervalCellPlanV2,
    FailureMarketIntervalCellSourceFailureReceiptV2,
)> {
    cell.validate_against(admission, funding, history, quote)?;
    if cell.phase != FailureMarketIntervalCellPhaseV2::Idle
        || history.family_terminal_receipt_id().bytes() != [0; 32]
    {
        return Err(Error::WrongPhase);
    }
    for id in [
        session_binding_id,
        session_schedule_id,
        source_failure.id(),
        source_join.id(),
        source_terminal_postwrite_id,
    ] {
        require_live(id.bytes())?;
    }
    let occurrence = source_failure.occurrence();
    let attempt_index =
        u8::try_from(cell.completed_session_count).map_err(|_| Error::BindingMismatch)?;
    if usize::from(attempt_index) >= usize::from(quote.schedule().attempt_count)
        || source_failure.failure_policy_binding_id().bytes()
            != cell.failure_policy_binding_id.bytes()
        || occurrence.market_instance_id().bytes() != cell.market_instance_id.bytes()
        || source_join.handoff_id() != source_failure.id()
        || source_join.failure_policy_binding_id() != source_failure.failure_policy_binding_id()
        || source_join.occurrence_account() != occurrence.occurrence_account()
        || source_join.result_or_absence_account().is_zero()
        || source_join.source_fact_authentication_id() != source_failure.source_fact_receipt_id()
        || source_join.clock() != source_failure.clock()
        || source_join.clock_policy_id() != occurrence.clock_policy_id()
        || source_join.source_spec_id() != occurrence.source_spec_id()
        || source_join.window_id() != occurrence.window_id()
        || source_join.statistic_key_id() != occurrence.statistic_key_id()
        || source_join.generation() == 0
        || source_join.work_receipt_authentication_id().is_zero()
        || source_terminal_postwrite_id == source_failure.id()
        || source_terminal_postwrite_id == source_join.id()
    {
        return Err(Error::BindingMismatch);
    }
    match source_failure.kind() {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
            if !source_failure.window_evidence_id().is_zero()
                || !source_failure.statistic_result_id().is_zero()
                || source_failure.refusal_code() != 0
            {
                return Err(Error::BindingMismatch);
            }
        }
        SourceFailureKindV1::SourceEvaluationRefused => {
            if source_failure.window_evidence_id().is_zero()
                || source_failure.statistic_result_id().is_zero()
                || source_failure.refusal_code() == 0
            {
                return Err(Error::BindingMismatch);
            }
        }
    }
    let cell_before = cell.id()?;
    let facts = FailureMarketIntervalCellSourceFailureFactsV2 {
        cell_before,
        history_before: history.id()?,
        history_root: history.history_root(),
        attempt_index,
        session_binding_id,
        session_schedule_id,
        source_kind: source_failure.kind(),
        source_handoff_id: source_failure.id(),
        source_join_id: source_join.id(),
        source_occurrence_id: occurrence.occurrence_record_id(),
        source_occurrence_account: occurrence.occurrence_account(),
        result_or_absence_account: source_join.result_or_absence_account(),
        source_fact_authentication_id: source_join.source_fact_authentication_id(),
        source_work_receipt_authentication_id: source_join.work_receipt_authentication_id(),
        source_repair_generation: occurrence.repair_generation(),
        window_id: occurrence.window_id(),
        statistic_key_id: occurrence.statistic_key_id(),
        window_evidence_id: source_failure.window_evidence_id(),
        statistic_result_id: source_failure.statistic_result_id(),
        refusal_code: source_failure.refusal_code(),
        source_terminal_postwrite_id,
    };
    authority.authenticate_failure_market_interval_cell_source_failure(facts)?;
    let mut receipt_hasher = Sha256::new();
    receipt_hasher.update(CELL_SOURCE_FAILURE_DOMAIN_V2);
    receipt_hasher.update(cell.failure_policy_binding_id.bytes());
    receipt_hasher.update(cell.market_instance_id.bytes());
    receipt_hasher.update(cell.generation.to_le_bytes());
    receipt_hasher.update(cell_before.bytes());
    receipt_hasher.update(facts.history_before.bytes());
    receipt_hasher.update(facts.history_root.bytes());
    receipt_hasher.update([source_failure_kind_byte(facts.source_kind)]);
    receipt_hasher.update([attempt_index]);
    receipt_hasher.update(session_binding_id.bytes());
    receipt_hasher.update(session_schedule_id.bytes());
    receipt_hasher.update(facts.source_handoff_id.bytes());
    receipt_hasher.update(facts.source_join_id.bytes());
    receipt_hasher.update(facts.source_occurrence_id.bytes());
    receipt_hasher.update(facts.source_occurrence_account.bytes());
    receipt_hasher.update(facts.result_or_absence_account.bytes());
    receipt_hasher.update(facts.source_fact_authentication_id.bytes());
    receipt_hasher.update(facts.source_work_receipt_authentication_id.bytes());
    receipt_hasher.update(facts.source_repair_generation.to_le_bytes());
    receipt_hasher.update(facts.window_id.bytes());
    receipt_hasher.update(facts.statistic_key_id.bytes());
    receipt_hasher.update(facts.window_evidence_id.bytes());
    receipt_hasher.update(facts.statistic_result_id.bytes());
    receipt_hasher.update(facts.refusal_code.to_le_bytes());
    receipt_hasher.update(source_terminal_postwrite_id.bytes());
    let id = FailureMarketIntervalCellSourceFailureReceiptIdV2::from_bytes(
        receipt_hasher.finalize().into(),
    );
    require_live(id.bytes())?;
    let mut after = cell;
    after.phase = FailureMarketIntervalCellPhaseV2::Resolved;
    after.disposition = match source_failure.kind() {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
            FailureMarketIntervalCellDispositionV2::SourceAbsent
        }
        SourceFailureKindV1::SourceEvaluationRefused => {
            FailureMarketIntervalCellDispositionV2::SourceRefused
        }
    };
    after.attempt_index = attempt_index;
    after.session_binding_id = session_binding_id;
    after.source_handoff_id = source_failure.id();
    after.session_schedule_id = session_schedule_id;
    after.terminal_receipt_id = SourceContentId::from_bytes(id.bytes());
    after.validate_against(admission, funding, history, quote)?;
    let cell_after = after.id()?;
    Ok((
        FailureMarketIntervalCellPlanV2 {
            before: cell,
            after,
        },
        FailureMarketIntervalCellSourceFailureReceiptV2 {
            id,
            facts,
            cell_after,
        },
    ))
}

const fn source_failure_kind_byte(kind: SourceFailureKindV1) -> u8 {
    match kind {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => 1,
        SourceFailureKindV1::SourceEvaluationRefused => 2,
    }
}

/// Restore Product's private exhaustive payout only from the exact persisted
/// transition chain, then latch one once-only Failure resolution receipt.
#[allow(clippy::too_many_arguments)]
pub fn plan_resolve_failure_market_interval_cell_v2<
    A: AuthenticatedFailureMarketIntervalCellResolutionV2 + ?Sized,
>(
    authority: &A,
    cell: FailureMarketIntervalCellV2,
    admission: FailureMarketAdmissionStateV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    history: FailureMarketIntervalHistoryV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    source_success: SuccessfulEvaluationHandoffV1,
    context: QuantizedIntervalConsensusContextV1<'_>,
) -> Result<FailureMarketIntervalCellResolutionPlanV2> {
    cell.validate_against(admission, funding, history, quote)?;
    if cell.phase != FailureMarketIntervalCellPhaseV2::Active
        || source_success.id() != cell.source_handoff_id
        || source_success.failure_policy_binding_id().bytes()
            != cell.failure_policy_binding_id.bytes()
    {
        return Err(Error::WrongPhase);
    }
    let work = cell.product_work()?.ok_or(Error::WrongPhase)?;
    if !work.is_complete()
        || work.source_interval_id().bytes() != source_success.statistic_result_id()?.bytes()
        || work.source_occurrence_id().bytes()
            != source_success.occurrence().occurrence_record_id().bytes()
    {
        return Err(Error::BindingMismatch);
    }
    let verified_payout = restore_verified_quantized_interval_payout_v1(authority, &work, context)?;
    let terminal_work_id = work.id()?;
    let product_certificate_id = verified_payout.certificate().id()?;
    let cell_before = cell.id()?;
    let last_runtime_work_receipt_id =
        LivenessId::from_bytes(cell.last_liveness_work_receipt_id.bytes());
    let mut receipt_hasher = Sha256::new();
    receipt_hasher.update(CELL_RESOLUTION_DOMAIN_V2);
    receipt_hasher.update(cell.failure_policy_binding_id.bytes());
    receipt_hasher.update(cell.market_instance_id.bytes());
    receipt_hasher.update(cell.generation.to_le_bytes());
    receipt_hasher.update(cell_before.bytes());
    receipt_hasher.update(history.id()?.bytes());
    receipt_hasher.update(cell.session_binding_id.bytes());
    receipt_hasher.update(source_success.id().bytes());
    receipt_hasher.update(terminal_work_id.bytes());
    receipt_hasher.update(product_certificate_id.bytes());
    receipt_hasher.update(last_runtime_work_receipt_id.bytes());
    receipt_hasher.update(cell.completed_work_calls.to_le_bytes());
    receipt_hasher.update(cell.exact_reward_lamports.to_le_bytes());
    let id = FailureMarketIntervalCellResolutionReceiptIdV2::from_bytes(
        receipt_hasher.finalize().into(),
    );
    require_live(id.bytes())?;
    let mut after = cell;
    after.phase = FailureMarketIntervalCellPhaseV2::Resolved;
    after.disposition = FailureMarketIntervalCellDispositionV2::Resolved;
    after.terminal_receipt_id = SourceContentId::from_bytes(id.bytes());
    after.validate_against(admission, funding, history, quote)?;
    let cell_after = after.id()?;
    let facts = FailureMarketIntervalCellResolutionFactsV2 {
        cell_before,
        cell_after,
        market_instance_id: cell.market_instance_id,
        generation: cell.generation,
        session_binding_id: cell.session_binding_id,
        source_handoff_id: source_success.id(),
        terminal_work_id,
        product_certificate_id,
        last_runtime_work_receipt_id,
        completed_work_calls: cell.completed_work_calls,
        exact_reward_lamports: cell.exact_reward_lamports,
    };
    authority.authenticate_failure_market_interval_cell_resolution(facts)?;
    Ok(FailureMarketIntervalCellResolutionPlanV2 {
        cell_plan: FailureMarketIntervalCellPlanV2 {
            before: cell,
            after,
        },
        receipt: FailureMarketIntervalCellResolutionReceiptV2 {
            id,
            failure_policy_binding_id: cell.failure_policy_binding_id,
            facts,
            verified_payout,
        },
    })
}

/// Canonical first exhaustion boundary reached by an incomplete session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureMarketIntervalExhaustionReasonV2 {
    /// The exact attempt-row progress allocation was consumed.
    AttemptProgress = 1,
    /// The shared Market liveness call bound was consumed.
    MarketCalls = 2,
    /// The exact shared work principal was consumed.
    MarketPrincipal = 3,
}

impl FailureMarketIntervalExhaustionReasonV2 {
    fn byte(self) -> u8 {
        match self {
            Self::AttemptProgress => 1,
            Self::MarketCalls => 2,
            Self::MarketPrincipal => 3,
        }
    }
}

/// Exact deterministic exhaustion facts. This projection is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellExhaustionFactsV2 {
    /// Exact incomplete Active cell prestate.
    pub cell_before: FailureMarketIntervalCellStateIdV2,
    /// Exact terminal Exhausted cell poststate.
    pub cell_after: FailureMarketIntervalCellStateIdV2,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/liveness generation.
    pub generation: u64,
    /// Exact subordinate session binding.
    pub session_binding_id: SourceContentId,
    /// Exact incomplete Product work.
    pub terminal_work_id: QuantizedIntervalConsensusWorkV1Id,
    /// Canonical first reached exhaustion boundary.
    pub reason: FailureMarketIntervalExhaustionReasonV2,
    /// Exact attempt progress consumed.
    pub accepted_progress_units: u64,
    /// Exact shared completed-call total, including archived sessions.
    pub aggregate_work_calls: u64,
    /// Exact shared keeper-reward total, including archived sessions.
    pub aggregate_reward_lamports: u64,
}

/// Private runtime authority for the exact exhaustion terminal postwrite.
pub trait AuthenticatedFailureMarketIntervalCellExhaustionV2 {
    /// Authenticate the persisted cell/history bodies and liveness compartment
    /// without allowing a discretionary resolver or alternate disposition.
    fn authenticate_failure_market_interval_cell_exhaustion(
        &self,
        _expected: FailureMarketIntervalCellExhaustionFactsV2,
    ) -> Result<()> {
        Err(Error::BindingMismatch)
    }
}

/// Private deterministic exhaustion receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellExhaustionReceiptV2 {
    id: FailureMarketIntervalCellExhaustionReceiptIdV2,
    failure_policy_binding_id: FailurePolicyBindingId,
    facts: FailureMarketIntervalCellExhaustionFactsV2,
}

impl FailureMarketIntervalCellExhaustionReceiptV2 {
    /// Complete exhaustion identity.
    pub const fn id(self) -> FailureMarketIntervalCellExhaustionReceiptIdV2 {
        self.id
    }

    /// Exact shared Failure policy.
    pub const fn failure_policy_binding_id(self) -> FailurePolicyBindingId {
        self.failure_policy_binding_id
    }

    /// Complete authenticated exhaustion facts.
    pub const fn facts(self) -> FailureMarketIntervalCellExhaustionFactsV2 {
        self.facts
    }
}

/// One deterministic exhausted terminal and exact cell postwrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellExhaustionPlanV2 {
    cell_plan: FailureMarketIntervalCellPlanV2,
    receipt: FailureMarketIntervalCellExhaustionReceiptV2,
}

impl FailureMarketIntervalCellExhaustionPlanV2 {
    /// Complete terminal reusable-cell poststate.
    pub const fn resulting_cell(self) -> FailureMarketIntervalCellV2 {
        self.cell_plan.after
    }

    /// Private deterministic exhaustion receipt.
    pub const fn receipt(self) -> FailureMarketIntervalCellExhaustionReceiptV2 {
        self.receipt
    }

    /// Stale-checked cell plan.
    pub const fn cell_plan(self) -> FailureMarketIntervalCellPlanV2 {
        self.cell_plan
    }
}

/// Latch the canonical first finite exhaustion boundary for incomplete work.
pub fn plan_exhaust_failure_market_interval_cell_v2<
    A: AuthenticatedFailureMarketIntervalCellExhaustionV2 + ?Sized,
>(
    authority: &A,
    cell: FailureMarketIntervalCellV2,
    admission: FailureMarketAdmissionStateV1,
    funding: FailureMarketIntervalFundingReceiptV2,
    history: FailureMarketIntervalHistoryV2,
    quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
) -> Result<FailureMarketIntervalCellExhaustionPlanV2> {
    cell.validate_against(admission, funding, history, quote)?;
    if cell.phase != FailureMarketIntervalCellPhaseV2::Active {
        return Err(Error::WrongPhase);
    }
    let work = cell.product_work()?.ok_or(Error::WrongPhase)?;
    if work.is_complete() {
        return Err(Error::WrongPhase);
    }
    let schedule = quote.schedule();
    let attempt = schedule.attempts[usize::from(cell.attempt_index)];
    let aggregate_work_calls = history
        .completed_work_calls()
        .checked_add(cell.completed_work_calls)
        .ok_or(Error::BindingMismatch)?;
    let aggregate_reward_lamports = history
        .exact_reward_lamports()
        .checked_add(cell.exact_reward_lamports)
        .ok_or(Error::BindingMismatch)?;
    let reason = canonical_exhaustion_reason(
        cell.accepted_progress_units,
        attempt.max_progress_units,
        aggregate_work_calls,
        u64::from(schedule.maximum_calls),
        aggregate_reward_lamports,
        quote.facts().work_principal_lamports,
    )?;
    let terminal_work_id = work.id()?;
    let cell_before = cell.id()?;
    let mut receipt_hasher = Sha256::new();
    receipt_hasher.update(CELL_EXHAUSTION_DOMAIN_V2);
    receipt_hasher.update(cell.failure_policy_binding_id.bytes());
    receipt_hasher.update(cell.market_instance_id.bytes());
    receipt_hasher.update(cell.generation.to_le_bytes());
    receipt_hasher.update(cell_before.bytes());
    receipt_hasher.update(history.id()?.bytes());
    receipt_hasher.update(cell.session_binding_id.bytes());
    receipt_hasher.update(terminal_work_id.bytes());
    receipt_hasher.update([reason.byte()]);
    receipt_hasher.update(cell.accepted_progress_units.to_le_bytes());
    receipt_hasher.update(aggregate_work_calls.to_le_bytes());
    receipt_hasher.update(aggregate_reward_lamports.to_le_bytes());
    let id = FailureMarketIntervalCellExhaustionReceiptIdV2::from_bytes(
        receipt_hasher.finalize().into(),
    );
    require_live(id.bytes())?;
    let mut after = cell;
    after.phase = FailureMarketIntervalCellPhaseV2::Resolved;
    after.disposition = FailureMarketIntervalCellDispositionV2::Exhausted;
    after.terminal_receipt_id = SourceContentId::from_bytes(id.bytes());
    after.validate_against(admission, funding, history, quote)?;
    let cell_after = after.id()?;
    let facts = FailureMarketIntervalCellExhaustionFactsV2 {
        cell_before,
        cell_after,
        market_instance_id: cell.market_instance_id,
        generation: cell.generation,
        session_binding_id: cell.session_binding_id,
        terminal_work_id,
        reason,
        accepted_progress_units: cell.accepted_progress_units,
        aggregate_work_calls,
        aggregate_reward_lamports,
    };
    authority.authenticate_failure_market_interval_cell_exhaustion(facts)?;
    Ok(FailureMarketIntervalCellExhaustionPlanV2 {
        cell_plan: FailureMarketIntervalCellPlanV2 {
            before: cell,
            after,
        },
        receipt: FailureMarketIntervalCellExhaustionReceiptV2 {
            id,
            failure_policy_binding_id: cell.failure_policy_binding_id,
            facts,
        },
    })
}

fn canonical_exhaustion_reason(
    accepted_progress_units: u64,
    maximum_attempt_progress_units: u64,
    aggregate_work_calls: u64,
    maximum_work_calls: u64,
    aggregate_reward_lamports: u64,
    work_principal_lamports: u64,
) -> Result<FailureMarketIntervalExhaustionReasonV2> {
    if accepted_progress_units == maximum_attempt_progress_units {
        Ok(FailureMarketIntervalExhaustionReasonV2::AttemptProgress)
    } else if aggregate_work_calls == maximum_work_calls {
        Ok(FailureMarketIntervalExhaustionReasonV2::MarketCalls)
    } else if aggregate_reward_lamports == work_principal_lamports {
        Ok(FailureMarketIntervalExhaustionReasonV2::MarketPrincipal)
    } else {
        Err(Error::WrongPhase)
    }
}

/// Project the exact terminal facts which the history owner must authenticate
/// and append before this cell may reset. The terminal receipt and both cell
/// postimages are derived from private state rather than caller DTOs.
pub fn project_failure_market_interval_terminal_history_facts_v2(
    cell: FailureMarketIntervalCellV2,
    history: FailureMarketIntervalHistoryV2,
) -> Result<FailureMarketIntervalTerminalFactsV2> {
    cell.validate()?;
    history.validate_internal()?;
    if cell.phase != FailureMarketIntervalCellPhaseV2::Resolved
        || history.failure_policy_binding_id() != cell.failure_policy_binding_id
        || history.market_instance_id() != cell.market_instance_id
        || history.generation() != cell.generation
        || history.funding_receipt_id() != cell.funding_receipt_id
        || history.history_account() != cell.history_account
        || history.completed_session_count() != cell.completed_session_count
    {
        return Err(Error::BindingMismatch);
    }
    let disposition = match cell.disposition {
        FailureMarketIntervalCellDispositionV2::Resolved => {
            FailureMarketIntervalTerminalDispositionV2::Resolved
        }
        FailureMarketIntervalCellDispositionV2::Exhausted => {
            FailureMarketIntervalTerminalDispositionV2::Exhausted
        }
        FailureMarketIntervalCellDispositionV2::SourceAbsent => {
            FailureMarketIntervalTerminalDispositionV2::SourceAbsent
        }
        FailureMarketIntervalCellDispositionV2::SourceRefused => {
            FailureMarketIntervalTerminalDispositionV2::SourceRefused
        }
        FailureMarketIntervalCellDispositionV2::None => return Err(Error::WrongPhase),
    };
    let idle = project_idle_failure_market_interval_cell_v2(cell)?;
    let last_liveness_work_receipt_id = if cell.last_liveness_work_receipt_id.is_zero() {
        ProductContentId::ZERO
    } else {
        ProductContentId::from_bytes(cell.last_liveness_work_receipt_id.bytes())
    };
    Ok(FailureMarketIntervalTerminalFactsV2 {
        history_before: history.id()?,
        session_binding_id: ProductContentId::from_bytes(cell.session_binding_id.bytes()),
        session_terminal_receipt_id: ProductContentId::from_bytes(cell.terminal_receipt_id.bytes()),
        terminal_state_commitment: ProductContentId::from_bytes(cell.id()?.bytes()),
        idle_state_commitment: ProductContentId::from_bytes(idle.id()?.bytes()),
        last_liveness_work_receipt_id,
        disposition,
        completed_work_calls: u32::try_from(cell.completed_work_calls)
            .map_err(|_| Error::BindingMismatch)?,
        exact_reward_lamports: cell.exact_reward_lamports,
    })
}

/// Private reset receipt proving history append and canonical Idle poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketIntervalCellResetReceiptV2 {
    id: FailureMarketIntervalCellResetReceiptIdV2,
    terminal_cell: FailureMarketIntervalCellStateIdV2,
    idle_cell: FailureMarketIntervalCellStateIdV2,
    append_receipt_id:
        crate::market_interval_history_v2::FailureMarketIntervalHistoryAppendReceiptIdV2,
}

impl FailureMarketIntervalCellResetReceiptV2 {
    /// Complete reset identity.
    pub const fn id(self) -> FailureMarketIntervalCellResetReceiptIdV2 {
        self.id
    }

    /// Exact terminal prestate folded into history.
    pub const fn terminal_cell(self) -> FailureMarketIntervalCellStateIdV2 {
        self.terminal_cell
    }

    /// Exact canonical Idle poststate.
    pub const fn idle_cell(self) -> FailureMarketIntervalCellStateIdV2 {
        self.idle_cell
    }

    /// Exact append receipt consumed by this reset.
    pub const fn append_receipt_id(
        self,
    ) -> crate::market_interval_history_v2::FailureMarketIntervalHistoryAppendReceiptIdV2 {
        self.append_receipt_id
    }
}

/// Project the only canonical Idle poststate. This is not permission to write;
/// [`plan_reset_failure_market_interval_cell_v2`] consumes the private append
/// receipt before minting a reset plan.
pub fn project_idle_failure_market_interval_cell_v2(
    cell: FailureMarketIntervalCellV2,
) -> Result<FailureMarketIntervalCellV2> {
    cell.validate()?;
    if cell.phase != FailureMarketIntervalCellPhaseV2::Resolved {
        return Err(Error::WrongPhase);
    }
    let mut idle = cell;
    idle.phase = FailureMarketIntervalCellPhaseV2::Idle;
    idle.disposition = FailureMarketIntervalCellDispositionV2::None;
    idle.attempt_index = 0;
    idle.completed_session_count = cell
        .completed_session_count
        .checked_add(1)
        .ok_or(Error::BindingMismatch)?;
    idle.transition_nonce = 0;
    idle.accepted_progress_units = 0;
    idle.completed_work_calls = 0;
    idle.exact_reward_lamports = 0;
    idle.session_binding_id = SourceContentId::ZERO;
    idle.source_handoff_id = SourceContentId::ZERO;
    idle.session_schedule_id = SourceContentId::ZERO;
    idle.last_transition_receipt_id = SourceContentId::ZERO;
    idle.last_liveness_work_receipt_id = SourceContentId::ZERO;
    idle.terminal_receipt_id = SourceContentId::ZERO;
    idle.product_work_body.fill(0);
    idle.validate()?;
    Ok(idle)
}

/// Consume the exact append receipt and reset the reusable cell atomically.
pub fn plan_reset_failure_market_interval_cell_v2(
    cell: FailureMarketIntervalCellV2,
    append: FailureMarketIntervalHistoryAppendReceiptV2,
) -> Result<(
    FailureMarketIntervalCellPlanV2,
    FailureMarketIntervalCellResetReceiptV2,
)> {
    let terminal_cell = cell.id()?;
    let idle = project_idle_failure_market_interval_cell_v2(cell)?;
    let idle_cell = idle.id()?;
    if append.failure_policy_binding_id() != cell.failure_policy_binding_id
        || append.market_instance_id() != cell.market_instance_id
        || append.generation() != cell.generation
        || append.funding_receipt_id() != cell.funding_receipt_id
        || append.history_account() != cell.history_account
        || append.session_binding_id() != cell.session_binding_id
        || append.session_terminal_receipt_id() != cell.terminal_receipt_id
        || append.terminal_state_commitment().bytes() != terminal_cell.bytes()
        || append.idle_state_commitment().bytes() != idle_cell.bytes()
        || append.completed_session_count() != idle.completed_session_count
    {
        return Err(Error::BindingMismatch);
    }
    let mut hasher = Sha256::new();
    hasher.update(CELL_RESET_DOMAIN_V2);
    hasher.update(terminal_cell.bytes());
    hasher.update(idle_cell.bytes());
    hasher.update(append.id().bytes());
    hasher.update(append.history_before().bytes());
    hasher.update(append.history_after().bytes());
    let receipt = FailureMarketIntervalCellResetReceiptV2 {
        id: FailureMarketIntervalCellResetReceiptIdV2::from_bytes(hasher.finalize().into()),
        terminal_cell,
        idle_cell,
        append_receipt_id: append.id(),
    };
    require_live(receipt.id.bytes())?;
    Ok((
        FailureMarketIntervalCellPlanV2 {
            before: cell,
            after: idle,
        },
        receipt,
    ))
}

fn put_id(
    output: &mut [u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
    cursor: &mut usize,
    value: [u8; ID_BYTES_V2],
) -> Result<()> {
    let end = cursor.checked_add(ID_BYTES_V2).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value);
    *cursor = end;
    Ok(())
}

fn take_id(
    input: &[u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
    cursor: &mut usize,
) -> Result<[u8; ID_BYTES_V2]> {
    let end = cursor.checked_add(ID_BYTES_V2).ok_or(Error::WrongLength)?;
    let value = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(value)
}

fn put_u64(
    output: &mut [u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
    cursor: &mut usize,
    value: u64,
) -> Result<()> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::WrongLength)?
        .copy_from_slice(&value.to_le_bytes());
    *cursor = end;
    Ok(())
}

fn take_u64(
    input: &[u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
    cursor: &mut usize,
) -> Result<u64> {
    let end = cursor.checked_add(8).ok_or(Error::WrongLength)?;
    let bytes = input
        .get(*cursor..end)
        .ok_or(Error::WrongLength)?
        .try_into()
        .map_err(|_| Error::WrongLength)?;
    *cursor = end;
    Ok(u64::from_le_bytes(bytes))
}

fn require_live(bytes: [u8; 32]) -> Result<()> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

const _: () = assert!(
    HEADER_BYTES_V2
        + AMOUNT_COUNT_V2 * 8
        + ID_COUNT_V2 * ID_BYTES_V2
        + QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1
        <= FAILURE_MARKET_INTERVAL_CELL_BYTES_V2
);

#[cfg(test)]
mod tests {
    use super::*;

    fn idle_cell() -> FailureMarketIntervalCellV2 {
        FailureMarketIntervalCellV2 {
            phase: FailureMarketIntervalCellPhaseV2::Idle,
            disposition: FailureMarketIntervalCellDispositionV2::None,
            attempt_index: 0,
            generation: 4,
            work_rent_principal_lamports: 100,
            completed_session_count: 0,
            transition_nonce: 0,
            accepted_progress_units: 0,
            completed_work_calls: 0,
            exact_reward_lamports: 0,
            failure_policy_binding_id: FailurePolicyBindingId::from_bytes([1; 32]),
            market_instance_id: MarketInstanceV2Id::from_bytes([2; 32]),
            funding_receipt_id: FailureMarketIntervalFundingReceiptIdV2::from_bytes([3; 32]),
            history_account: FailureMarketAccountIdV1::from_bytes([4; 32]),
            rent_refund_owner: FailureMarketAccountIdV1::from_bytes([5; 32]),
            neutral_sink: FailureMarketAccountIdV1::from_bytes([6; 32]),
            session_binding_id: SourceContentId::ZERO,
            source_handoff_id: SourceContentId::ZERO,
            session_schedule_id: SourceContentId::ZERO,
            quote_admission_receipt_id: SourceContentId::from_bytes([7; 32]),
            last_transition_receipt_id: SourceContentId::ZERO,
            last_liveness_work_receipt_id: SourceContentId::ZERO,
            terminal_receipt_id: SourceContentId::ZERO,
            product_work_body: [0; QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1],
        }
    }

    #[test]
    fn idle_codec_refuses_hidden_session_bytes_and_clears_reserved_tail() {
        let idle = idle_cell();
        let mut body = [0; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2];
        idle.encode_into(&mut body).unwrap();
        assert_ne!(idle.id().unwrap().bytes(), [0; 32]);

        let mut hidden = idle;
        hidden.session_binding_id = SourceContentId::from_bytes([8; 32]);
        assert_eq!(hidden.encode_into(&mut body), Err(Error::WrongPhase));
        assert_eq!(body[FAILURE_MARKET_INTERVAL_CELL_BYTES_V2 - 1], 0);
        assert_eq!(
            FailureMarketIntervalCellPhaseV2::decode(0),
            Err(Error::InvalidEnum)
        );
    }
    #[test]
    fn exhaustion_trigger_is_exact_ordered_and_refuses_near_misses() {
        assert_eq!(
            canonical_exhaustion_reason(5, 5, 7, 7, 11, 11),
            Ok(FailureMarketIntervalExhaustionReasonV2::AttemptProgress)
        );
        assert_eq!(
            canonical_exhaustion_reason(4, 5, 7, 7, 11, 11),
            Ok(FailureMarketIntervalExhaustionReasonV2::MarketCalls)
        );
        assert_eq!(
            canonical_exhaustion_reason(4, 5, 6, 7, 11, 11),
            Ok(FailureMarketIntervalExhaustionReasonV2::MarketPrincipal)
        );
        assert_eq!(
            canonical_exhaustion_reason(4, 5, 6, 7, 10, 11),
            Err(Error::WrongPhase)
        );
    }

    #[test]
    fn direct_source_failure_is_zero_work_and_canonically_distinct() {
        let mut refused = idle_cell();
        refused.phase = FailureMarketIntervalCellPhaseV2::Resolved;
        refused.disposition = FailureMarketIntervalCellDispositionV2::SourceRefused;
        refused.session_binding_id = SourceContentId::from_bytes([8; 32]);
        refused.source_handoff_id = SourceContentId::from_bytes([9; 32]);
        refused.session_schedule_id = SourceContentId::from_bytes([10; 32]);
        refused.terminal_receipt_id = SourceContentId::from_bytes([11; 32]);
        assert_eq!(refused.validate(), Ok(()));
        assert_eq!(refused.product_work(), Ok(None));

        let mut fabricated_work = refused;
        fabricated_work.product_work_body[0] = 1;
        assert_eq!(fabricated_work.validate(), Err(Error::BindingMismatch));

        let mut fabricated_reward = refused;
        fabricated_reward.completed_work_calls = 1;
        fabricated_reward.exact_reward_lamports = 1;
        fabricated_reward.last_liveness_work_receipt_id = SourceContentId::from_bytes([12; 32]);
        assert_eq!(fabricated_reward.validate(), Err(Error::BindingMismatch));

        let mut collapsed_disposition = refused;
        collapsed_disposition.disposition = FailureMarketIntervalCellDispositionV2::Exhausted;
        assert_eq!(collapsed_disposition.validate(), Err(Error::WrongPhase));
        assert_ne!(
            FailureMarketIntervalCellDispositionV2::SourceRefused.byte(),
            FailureMarketIntervalCellDispositionV2::Exhausted.byte()
        );
        let mut absent = refused;
        absent.disposition = FailureMarketIntervalCellDispositionV2::SourceAbsent;
        assert_eq!(absent.validate(), Ok(()));
        assert_ne!(absent.id().unwrap(), refused.id().unwrap());
    }
}
