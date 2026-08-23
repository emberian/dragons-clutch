// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reusable Market-scoped interval session cell.
//!
//! Product owns the exact 592-byte structural-work body. Failure owns the
//! exclusive session pin, bounded paid-transition transcript, terminal latch,
//! and canonical reset. The cell retains its original Rent principal through
//! every session and is physically closed only at exhaustive Market terminal.

use clutch_product_series::{
    begin_quantized_interval_consensus_v1, FixedCodec, MarketInstanceV2Id,
    QuantizedIntervalConsensusContextV1, QuantizedIntervalConsensusWorkV1,
    QUANTIZED_INTERVAL_CONSENSUS_WORK_BYTES_V1,
};
use clutch_source_plane_v3::ContentId as SourceContentId;
use clutch_source_plane_v3_runtime::SuccessfulEvaluationHandoffV1;
use sha2::{Digest, Sha256};

use crate::market_interval_history_v2::{
    FailureMarketIntervalFundingReceiptIdV2, FailureMarketIntervalFundingReceiptV2,
    FailureMarketIntervalHistoryAppendReceiptV2, FailureMarketIntervalHistoryRootV2,
    FailureMarketIntervalHistoryV2,
};
use crate::market_policy_v1::{FailureMarketAccountIdV1, FailureMarketAdmissionStateV1};
use crate::market_quote_v1::FailureMarketRecoveryQuoteAdmissionReceiptV1;
use crate::{Error, FailurePolicyBindingId, Result};

const CELL_MAGIC_V2: [u8; 8] = *b"DCFICEL2";
const CELL_VERSION_V2: u16 = 2;
const CELL_STATE_DOMAIN_V2: &[u8] = b"dragons-clutch/failure-market-interval-cell-state/v2";
const CELL_ACTIVATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/failure-market-interval-cell-activation/v2";
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
    /// A separately authenticated source/evaluator owner refused the session.
    Refused = 3,
}

impl FailureMarketIntervalCellDispositionV2 {
    fn byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Resolved => 1,
            Self::Exhausted => 2,
            Self::Refused => 3,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Resolved),
            2 => Ok(Self::Exhausted),
            3 => Ok(Self::Refused),
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

    /// Current active attempt row, or zero while Idle.
    pub const fn attempt_index(self) -> u8 {
        self.attempt_index
    }

    /// Exact current session binding, or zero while Idle.
    pub const fn session_binding_id(self) -> SourceContentId {
        self.session_binding_id
    }

    /// Current structural Product work, absent only while Idle.
    pub fn product_work(self) -> Result<Option<QuantizedIntervalConsensusWorkV1>> {
        if self.phase == FailureMarketIntervalCellPhaseV2::Idle {
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

    /// Hostile-decode against exact authenticated admission, capitalization,
    /// history, and shared quote receipts.
    pub fn decode_for_admission(
        input: &[u8; FAILURE_MARKET_INTERVAL_CELL_BYTES_V2],
        admission: FailureMarketAdmissionStateV1,
        funding: FailureMarketIntervalFundingReceiptV2,
        history: FailureMarketIntervalHistoryV2,
        quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
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
                if session_ids.iter().any(|id| id.is_zero()) || no_work {
                    return Err(Error::WrongPhase);
                }
                let expected_attempt = u8::try_from(self.completed_session_count)
                    .map_err(|_| Error::BindingMismatch)?;
                if self.attempt_index != expected_attempt {
                    return Err(Error::BindingMismatch);
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

    fn validate_against(
        self,
        admission: FailureMarketAdmissionStateV1,
        funding: FailureMarketIntervalFundingReceiptV2,
        history: FailureMarketIntervalHistoryV2,
        quote: FailureMarketRecoveryQuoteAdmissionReceiptV1,
    ) -> Result<()> {
        self.validate()?;
        let policy = admission.binding().facts();
        let funding_facts = funding.facts();
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
        || source_success.occurrence().repair_generation() != cell.generation
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
}
