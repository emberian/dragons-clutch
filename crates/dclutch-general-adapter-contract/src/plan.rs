//! Stateless General plan evaluation behind generic Trading authority.
//!
//! This module accepts only already-authenticated readonly semantic views. It
//! owns no account, signer, release, clock, or child-program authority and it
//! performs no CPI. Scratch may change on refusal; caller-owned candidate
//! outputs are copied only after the whole plan accepts. The verifier,
//! selection, certificate, cursor, and child-plan banks remain distinct so a
//! transport may chunk them without imposing a semantic outcome/page cap.

use dclutch_general_codec::{
    Action, CANDIDATE_BYTES, CandidateV1, PAGE_BYTES, PageViewV1, SELECTION_CURSOR_BYTES,
    SELECTION_POLICY_BYTES, SETTLEMENT_CURSOR_BYTES, SelectionCursorV1, SelectionPolicyV1,
};

use crate::{
    AggregateReplayContextV1, CandidateVerifierV1, ChildExecutionError, ConsiderVerifiedInputV1,
    ExecutionContextV1, GeneralChildEffectV1, GeneralChildPlanV2, QuoteSurplusRouteV2,
    RowReplayContextV1, SettlementChildrenV1, SettlementRowInputV1, VERIFICATION_CURSOR_BYTES_V1,
    VERIFIED_CANDIDATE_BYTES_V1, VerifiedCandidateV1, close, collect_execution_row,
    consider_verified_input, distribute_execution_row, freeze_selection, initialize_settlement,
    materialize,
};

/// Stable refusal from stateless General plan evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanErrorV2 {
    /// A readonly or caller-owned bank had another exact width.
    InvalidWidth,
    /// Candidate, policy, page, certificate, or cursor bytes refused.
    InvalidInput,
    /// An immutable config-selected limit or identity differed.
    ConfigMismatch,
    /// A page, candidate, revision, phase, or incumbent coordinate differed.
    CoordinateMismatch,
    /// Candidate verification, selection, or settlement semantics refused.
    Transition,
    /// Physical bank capacities differed from the exact semantic effect plan.
    EffectCapacity,
}

/// Result alias for stateless General plans.
pub type PlanResultV2<T> = core::result::Result<T, PlanErrorV2>;

/// Already-authenticated immutable limits selected by General config.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPlanLimitsV2 {
    outcome_count: u16,
    max_pages_per_candidate: u32,
    max_orders_per_candidate: u32,
    price_scale: u64,
    selection_policy_id: [u8; 32],
}

impl GeneralPlanLimitsV2 {
    /// Construct one exact config projection.
    pub fn new(
        outcome_count: u16,
        max_pages_per_candidate: u32,
        max_orders_per_candidate: u32,
        price_scale: u64,
        selection_policy_id: [u8; 32],
    ) -> PlanResultV2<Self> {
        if outcome_count == 0
            || max_pages_per_candidate == 0
            || max_orders_per_candidate == 0
            || price_scale == 0
            || is_zero(&selection_policy_id)
        {
            return Err(PlanErrorV2::ConfigMismatch);
        }
        Ok(Self {
            outcome_count,
            max_pages_per_candidate,
            max_orders_per_candidate,
            price_scale,
            selection_policy_id,
        })
    }

    fn require_candidate(self, candidate: CandidateV1) -> PlanResultV2<()> {
        if u16::from(candidate.outcome_count) != self.outcome_count
            || candidate.page_count > self.max_pages_per_candidate
            || candidate.price_scale != self.price_scale
        {
            Err(PlanErrorV2::ConfigMismatch)
        } else {
            Ok(())
        }
    }
}

/// Readonly views for one streamed candidate-page evaluation.
pub struct ConsiderPlanViewV2<'a> {
    /// Exact immutable candidate header.
    pub candidate: &'a [u8],
    /// Exact immutable interpreted selection policy.
    pub policy: &'a [u8],
    /// Exact next immutable candidate page.
    pub page: &'a [u8],
    /// Zero initial or canonical persisted verifier state.
    pub verification_before: &'a [u8],
    /// Zero initial or canonical batch selection state.
    pub selection_before: &'a [u8],
    /// All-zero destination for this candidate certificate.
    pub certificate_before: &'a [u8],
    /// Current best certificate when selection is nonempty.
    pub incumbent_certificate: Option<&'a [u8]>,
    /// Exact optimistic verifier revision consumed by this page.
    pub expected_revision: u64,
    /// Config-selected immutable limits.
    pub limits: GeneralPlanLimitsV2,
}

/// Separate scratch/candidate banks for failure-atomic consideration.
pub struct ConsiderPlanBuffersV2<'a> {
    /// Non-authoritative 960-byte verifier scratch.
    pub verification_scratch: &'a mut [u8],
    /// Candidate 960-byte verifier successor; unchanged on refusal.
    pub verification_output: &'a mut [u8],
    /// Non-authoritative selection scratch.
    pub selection_scratch: &'a mut [u8],
    /// Candidate selection successor; unchanged on refusal.
    pub selection_output: &'a mut [u8],
    /// Non-authoritative certificate scratch.
    pub certificate_scratch: &'a mut [u8],
    /// Candidate certificate successor; unchanged on refusal.
    pub certificate_output: &'a mut [u8],
}

/// Small accepted result for one streamed page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsiderPlanSummaryV2 {
    /// Whether this page completed candidate-wide verification.
    pub complete: bool,
    /// Exact globally grouped order count after this page.
    pub order_count: u32,
    /// Whether selection/certificate semantics executed.
    pub selection_considered: bool,
}

/// Evaluate one candidate page into distinct transportable candidate banks.
#[inline(never)]
pub fn evaluate_consider_v2(
    view: ConsiderPlanViewV2<'_>,
    buffers: ConsiderPlanBuffersV2<'_>,
) -> PlanResultV2<ConsiderPlanSummaryV2> {
    require_consider_widths(&view, &buffers)?;
    let candidate = CandidateV1::decode(view.candidate).map_err(|_| PlanErrorV2::InvalidInput)?;
    let policy = SelectionPolicyV1::decode(view.policy).map_err(|_| PlanErrorV2::InvalidInput)?;
    view.limits.require_candidate(candidate)?;
    if policy.policy_id != view.limits.selection_policy_id {
        return Err(PlanErrorV2::ConfigMismatch);
    }
    let page = PageViewV1::decode(view.page).map_err(|_| PlanErrorV2::InvalidInput)?;
    if page.candidate_id() != candidate.candidate_id
        || page.outcome_count() != candidate.outcome_count
        || page.page_count() != candidate.page_count
    {
        return Err(PlanErrorV2::CoordinateMismatch);
    }

    let ConsiderPlanBuffersV2 {
        verification_scratch,
        verification_output,
        selection_scratch,
        selection_output,
        certificate_scratch,
        certificate_output,
    } = buffers;
    let mut verifier = if view.verification_before.iter().all(|byte| *byte == 0) {
        if view.expected_revision != 0 || page.page_index() != 0 {
            return Err(PlanErrorV2::CoordinateMismatch);
        }
        CandidateVerifierV1::begin(candidate)
    } else {
        let value = CandidateVerifierV1::decode(view.verification_before)
            .map_err(|_| PlanErrorV2::InvalidInput)?;
        if value.candidate() != candidate
            || value.next_page() != page.page_index()
            || value.revision() != view.expected_revision
        {
            return Err(PlanErrorV2::CoordinateMismatch);
        }
        value
    };
    verifier
        .ingest_page_at(view.page, view.expected_revision)
        .map_err(|_| PlanErrorV2::Transition)?;
    if verifier.order_count() > view.limits.max_orders_per_candidate {
        return Err(PlanErrorV2::ConfigMismatch);
    }
    verification_scratch.fill(0);
    verifier
        .encode_into(verification_scratch)
        .map_err(|_| PlanErrorV2::Transition)?;
    selection_scratch.copy_from_slice(view.selection_before);
    certificate_scratch.copy_from_slice(view.certificate_before);
    let complete = verifier.is_complete();
    if complete {
        if certificate_scratch.iter().any(|byte| *byte != 0) {
            return Err(PlanErrorV2::CoordinateMismatch);
        }
        let verified = verifier.finish().map_err(|_| PlanErrorV2::Transition)?;
        let incumbent = decode_incumbent(
            selection_scratch,
            view.incumbent_certificate,
            candidate,
            policy,
        )?;
        let selection_revision = selection_revision(selection_scratch)?;
        consider_verified_input(
            selection_scratch,
            certificate_scratch,
            ConsiderVerifiedInputV1 {
                candidate: &candidate,
                policy: &policy,
                verified: &verified,
                incumbent: incumbent.as_ref(),
                expected_revision: selection_revision,
            },
        )
        .map_err(|_| PlanErrorV2::Transition)?;
    } else if certificate_scratch.iter().any(|byte| *byte != 0) {
        return Err(PlanErrorV2::CoordinateMismatch);
    }

    verification_output.copy_from_slice(verification_scratch);
    selection_output.copy_from_slice(selection_scratch);
    certificate_output.copy_from_slice(certificate_scratch);
    Ok(ConsiderPlanSummaryV2 {
        complete,
        order_count: verifier.order_count(),
        selection_considered: complete,
    })
}

/// Evaluate permissionless selection freeze into caller-owned candidate bytes.
#[inline(never)]
pub fn evaluate_freeze_v2(
    selection_before: &[u8],
    expected_revision: u64,
    scratch: &mut [u8],
    output: &mut [u8],
) -> PlanResultV2<()> {
    if selection_before.len() != SELECTION_CURSOR_BYTES
        || scratch.len() != SELECTION_CURSOR_BYTES
        || output.len() != SELECTION_CURSOR_BYTES
    {
        return Err(PlanErrorV2::InvalidWidth);
    }
    scratch.copy_from_slice(selection_before);
    freeze_selection(scratch, expected_revision).map_err(|_| PlanErrorV2::Transition)?;
    output.copy_from_slice(scratch);
    Ok(())
}

/// Evaluate settlement initialization into caller-owned candidate bytes.
#[inline(never)]
pub fn evaluate_initialize_settlement_v2(
    selection: &[u8],
    certificate: &[u8],
    candidate_bytes: &[u8],
    expected_revision: u64,
    scratch: &mut [u8],
    output: &mut [u8],
) -> PlanResultV2<()> {
    if selection.len() != SELECTION_CURSOR_BYTES
        || certificate.len() != VERIFIED_CANDIDATE_BYTES_V1
        || candidate_bytes.len() != CANDIDATE_BYTES
        || scratch.len() != SETTLEMENT_CURSOR_BYTES
        || output.len() != SETTLEMENT_CURSOR_BYTES
    {
        return Err(PlanErrorV2::InvalidWidth);
    }
    let candidate = CandidateV1::decode(candidate_bytes).map_err(|_| PlanErrorV2::InvalidInput)?;
    let verified =
        VerifiedCandidateV1::decode(certificate).map_err(|_| PlanErrorV2::InvalidInput)?;
    require_certificate(candidate, verified)?;
    scratch.fill(0);
    initialize_settlement(scratch, selection, &verified, expected_revision)
        .map_err(|_| PlanErrorV2::Transition)?;
    output.copy_from_slice(scratch);
    Ok(())
}

/// Readonly views for one two-pass settlement continuation.
pub struct SettlementPlanViewV2<'a> {
    /// Collect, Materialize, Distribute, or Close.
    pub action: Action,
    /// Exact settlement cursor prestate.
    pub cursor_before: &'a [u8],
    /// Program-derived selected candidate certificate.
    pub certificate: &'a [u8],
    /// Exact next page for row actions; absent for aggregate actions.
    pub page: Option<&'a [u8]>,
    /// Authenticated Market/release coordinates.
    pub context: ExecutionContextV1,
    /// Exact optimistic settlement revision.
    pub expected_revision: u64,
    /// Immutable terminal surplus route, present only when Close may need it.
    pub surplus_route: Option<QuoteSurplusRouteV2>,
}

/// Cursor and two semantic-effect banks for failure-atomic settlement.
pub struct SettlementPlanBuffersV2<'a> {
    /// Non-authoritative cursor scratch.
    pub cursor_scratch: &'a mut [u8],
    /// Candidate cursor successor; unchanged on refusal.
    pub cursor_output: &'a mut [u8],
    /// Non-authoritative first semantic-effect scratch.
    pub first_effect_scratch: &'a mut [u8],
    /// Candidate first semantic-effect bank; unchanged on refusal.
    pub first_effect_output: &'a mut [u8],
    /// Non-authoritative second semantic-effect scratch.
    pub second_effect_scratch: &'a mut [u8],
    /// Candidate second semantic-effect bank; unchanged on refusal.
    pub second_effect_output: &'a mut [u8],
}

/// Exact settlement-bank lengths for canonical chunk transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementPlanSummaryV2 {
    /// Exact length of the first complete semantic-effect bank.
    pub first_effect_bytes: u32,
    /// Exact length of the second complete semantic-effect bank.
    pub second_effect_bytes: u32,
}

/// Evaluate one settlement continuation without accounts, writes, or CPI.
#[inline(never)]
pub fn evaluate_settlement_v2(
    view: SettlementPlanViewV2<'_>,
    buffers: SettlementPlanBuffersV2<'_>,
) -> PlanResultV2<SettlementPlanSummaryV2> {
    require_settlement_widths(&view, &buffers)?;
    let verified =
        VerifiedCandidateV1::decode(view.certificate).map_err(|_| PlanErrorV2::InvalidInput)?;
    let SettlementPlanBuffersV2 {
        cursor_scratch,
        cursor_output,
        first_effect_scratch,
        first_effect_output,
        second_effect_scratch,
        second_effect_output,
    } = buffers;
    cursor_scratch.copy_from_slice(view.cursor_before);
    let mut measurement = EffectRecorderV2 {
        first: None,
        second: None,
        lengths: [0; 2],
        count: 0,
        surplus_route: view.surplus_route,
        error: None,
    };
    let measured =
        execute_settlement_transition_v2(&view, &verified, cursor_scratch, &mut measurement);
    if let Some(error) = measurement.error {
        return Err(error);
    }
    measured?;
    require_effect_capacities(
        &measurement,
        first_effect_scratch.len(),
        second_effect_scratch.len(),
    )?;

    cursor_scratch.copy_from_slice(view.cursor_before);
    first_effect_scratch.fill(0);
    second_effect_scratch.fill(0);
    let mut recorder = EffectRecorderV2 {
        first: Some(first_effect_scratch),
        second: Some(second_effect_scratch),
        lengths: [0; 2],
        count: 0,
        surplus_route: view.surplus_route,
        error: None,
    };
    let emitted = execute_settlement_transition_v2(&view, &verified, cursor_scratch, &mut recorder);
    if let Some(error) = recorder.error {
        return Err(error);
    }
    emitted?;
    if recorder.count != measurement.count || recorder.lengths != measurement.lengths {
        return Err(PlanErrorV2::Transition);
    }
    let summary = SettlementPlanSummaryV2 {
        first_effect_bytes: recorder.lengths[0],
        second_effect_bytes: recorder.lengths[1],
    };
    cursor_output.copy_from_slice(cursor_scratch);
    first_effect_output.copy_from_slice(recorder.first.as_deref().ok_or(PlanErrorV2::Transition)?);
    second_effect_output
        .copy_from_slice(recorder.second.as_deref().ok_or(PlanErrorV2::Transition)?);
    Ok(summary)
}

fn execute_settlement_transition_v2(
    view: &SettlementPlanViewV2<'_>,
    verified: &VerifiedCandidateV1,
    cursor: &mut [u8],
    children: &mut impl SettlementChildrenV1,
) -> PlanResultV2<()> {
    let result = match view.action {
        Action::Collect => collect_execution_row(
            cursor,
            SettlementRowInputV1 {
                context: view.context,
                verified,
                page_bytes: view.page.ok_or(PlanErrorV2::CoordinateMismatch)?,
                expected_revision: view.expected_revision,
            },
            children,
        ),
        Action::Materialize => materialize(
            cursor,
            view.context,
            verified,
            view.expected_revision,
            children,
        ),
        Action::Distribute => distribute_execution_row(
            cursor,
            SettlementRowInputV1 {
                context: view.context,
                verified,
                page_bytes: view.page.ok_or(PlanErrorV2::CoordinateMismatch)?,
                expected_revision: view.expected_revision,
            },
            children,
        ),
        Action::Close => close(
            cursor,
            view.context,
            verified,
            view.expected_revision,
            children,
        ),
        _ => return Err(PlanErrorV2::CoordinateMismatch),
    };
    result.map_err(|_| PlanErrorV2::Transition)
}

fn require_effect_capacities(
    measurement: &EffectRecorderV2<'_>,
    first_capacity: usize,
    second_capacity: usize,
) -> PlanResultV2<()> {
    let first_required = if measurement.count > 0 {
        usize::try_from(measurement.lengths[0]).map_err(|_| PlanErrorV2::EffectCapacity)?
    } else {
        0
    };
    let second_required = if measurement.count > 1 {
        usize::try_from(measurement.lengths[1]).map_err(|_| PlanErrorV2::EffectCapacity)?
    } else {
        0
    };
    if first_capacity != first_required || second_capacity != second_required {
        Err(PlanErrorV2::EffectCapacity)
    } else {
        Ok(())
    }
}

fn require_consider_widths(
    view: &ConsiderPlanViewV2<'_>,
    buffers: &ConsiderPlanBuffersV2<'_>,
) -> PlanResultV2<()> {
    if view.candidate.len() != CANDIDATE_BYTES
        || view.policy.len() != SELECTION_POLICY_BYTES
        || view.page.len() != PAGE_BYTES
        || view.verification_before.len() != VERIFICATION_CURSOR_BYTES_V1
        || view.selection_before.len() != SELECTION_CURSOR_BYTES
        || view.certificate_before.len() != VERIFIED_CANDIDATE_BYTES_V1
        || buffers.verification_scratch.len() != VERIFICATION_CURSOR_BYTES_V1
        || buffers.verification_output.len() != VERIFICATION_CURSOR_BYTES_V1
        || buffers.selection_scratch.len() != SELECTION_CURSOR_BYTES
        || buffers.selection_output.len() != SELECTION_CURSOR_BYTES
        || buffers.certificate_scratch.len() != VERIFIED_CANDIDATE_BYTES_V1
        || buffers.certificate_output.len() != VERIFIED_CANDIDATE_BYTES_V1
    {
        Err(PlanErrorV2::InvalidWidth)
    } else {
        Ok(())
    }
}

fn decode_incumbent(
    selection_bytes: &[u8],
    incumbent_bytes: Option<&[u8]>,
    candidate: CandidateV1,
    policy: SelectionPolicyV1,
) -> PlanResultV2<Option<VerifiedCandidateV1>> {
    if selection_bytes.iter().all(|byte| *byte == 0) {
        return if incumbent_bytes.is_none() {
            Ok(None)
        } else {
            Err(PlanErrorV2::CoordinateMismatch)
        };
    }
    let selection =
        SelectionCursorV1::decode(selection_bytes).map_err(|_| PlanErrorV2::InvalidInput)?;
    if selection.batch_id != candidate.batch_id || selection.policy_id != policy.policy_id {
        return Err(PlanErrorV2::CoordinateMismatch);
    }
    match (selection.best_candidate_id, incumbent_bytes) {
        (None, None) => Ok(None),
        (Some(expected), Some(bytes)) if bytes.len() == VERIFIED_CANDIDATE_BYTES_V1 => {
            let value =
                VerifiedCandidateV1::decode(bytes).map_err(|_| PlanErrorV2::InvalidInput)?;
            if value.candidate_id == expected {
                Ok(Some(value))
            } else {
                Err(PlanErrorV2::CoordinateMismatch)
            }
        }
        _ => Err(PlanErrorV2::CoordinateMismatch),
    }
}

fn selection_revision(selection_bytes: &[u8]) -> PlanResultV2<u64> {
    if selection_bytes.iter().all(|byte| *byte == 0) {
        Ok(0)
    } else {
        SelectionCursorV1::decode(selection_bytes)
            .map(|value| value.revision)
            .map_err(|_| PlanErrorV2::InvalidInput)
    }
}

fn require_certificate(candidate: CandidateV1, verified: VerifiedCandidateV1) -> PlanResultV2<()> {
    if verified.candidate_id != candidate.candidate_id
        || verified.product_id != candidate.product_id
        || verified.batch_id != candidate.batch_id
        || verified.outcome_count != candidate.outcome_count
        || verified.page_count != candidate.page_count
    {
        Err(PlanErrorV2::CoordinateMismatch)
    } else {
        Ok(())
    }
}

fn require_settlement_widths(
    view: &SettlementPlanViewV2<'_>,
    buffers: &SettlementPlanBuffersV2<'_>,
) -> PlanResultV2<()> {
    let row = matches!(view.action, Action::Collect | Action::Distribute);
    if view.cursor_before.len() != SETTLEMENT_CURSOR_BYTES
        || view.certificate.len() != VERIFIED_CANDIDATE_BYTES_V1
        || row != view.page.is_some()
        || view.page.is_some_and(|page| page.len() != PAGE_BYTES)
        || buffers.cursor_scratch.len() != SETTLEMENT_CURSOR_BYTES
        || buffers.cursor_output.len() != SETTLEMENT_CURSOR_BYTES
        || buffers.first_effect_scratch.len() != buffers.first_effect_output.len()
        || buffers.second_effect_scratch.len() != buffers.second_effect_output.len()
    {
        Err(PlanErrorV2::InvalidWidth)
    } else {
        Ok(())
    }
}

struct EffectRecorderV2<'a> {
    first: Option<&'a mut [u8]>,
    second: Option<&'a mut [u8]>,
    lengths: [u32; 2],
    count: u8,
    surplus_route: Option<QuoteSurplusRouteV2>,
    error: Option<PlanErrorV2>,
}

impl EffectRecorderV2<'_> {
    fn record(
        &mut self,
        plan: core::result::Result<GeneralChildPlanV2<'_>, crate::Error>,
    ) -> core::result::Result<(), ChildExecutionError> {
        let plan = match plan {
            Ok(value) => value,
            Err(_) => return self.refuse(PlanErrorV2::Transition),
        };
        let length = match plan.encoded_len() {
            Ok(value) => value,
            Err(_) => return self.refuse(PlanErrorV2::Transition),
        };
        let target = match self.count {
            0 => self.first.as_deref_mut(),
            1 => self.second.as_deref_mut(),
            _ => return self.refuse(PlanErrorV2::EffectCapacity),
        };
        if let Some(target) = target {
            if length != target.len() {
                return self.refuse(PlanErrorV2::EffectCapacity);
            }
            if plan.encode_into(target).is_err() {
                return self.refuse(PlanErrorV2::Transition);
            }
        }
        self.lengths[usize::from(self.count)] =
            u32::try_from(length).map_err(|_| ChildExecutionError::Refused)?;
        self.count = self.count.saturating_add(1);
        Ok(())
    }

    fn refuse<T>(&mut self, error: PlanErrorV2) -> core::result::Result<T, ChildExecutionError> {
        self.error = Some(error);
        Err(ChildExecutionError::Refused)
    }
}

impl SettlementChildrenV1 for EffectRecorderV2<'_> {
    fn collect_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; dclutch_general_codec::MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record_row(
            GeneralChildEffectV1::CollectClaims,
            context,
            outcome_count,
            quantities,
        )
    }

    fn collect_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record_scalar_row(GeneralChildEffectV1::CollectCollateral, context, quantity)
    }

    fn mint_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record_uniform_aggregate(
            GeneralChildEffectV1::MintCompleteSet,
            context,
            outcome_count,
            quantity,
        )
    }

    fn merge_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record_uniform_aggregate(
            GeneralChildEffectV1::MergeCompleteSet,
            context,
            outcome_count,
            quantity,
        )
    }

    fn distribute_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; dclutch_general_codec::MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record_row(
            GeneralChildEffectV1::DistributeClaims,
            context,
            outcome_count,
            quantities,
        )
    }

    fn distribute_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        self.record_scalar_row(
            GeneralChildEffectV1::DistributeCollateral,
            context,
            quantity,
        )
    }

    fn pay_surplus(
        &mut self,
        context: AggregateReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let route = match self.surplus_route {
            Some(value) => value,
            None => return self.refuse(PlanErrorV2::CoordinateMismatch),
        };
        let tail = quantity.to_le_bytes();
        self.record(GeneralChildPlanV2::new_surplus(context, &tail, route))
    }
}

impl EffectRecorderV2<'_> {
    fn record_row(
        &mut self,
        effect: GeneralChildEffectV1,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; dclutch_general_codec::MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError> {
        let mut tail = [0_u8; 8 * dclutch_general_codec::MAX_OUTCOMES];
        let active = encode_quantities(outcome_count, quantities, &mut tail)?;
        self.record(GeneralChildPlanV2::new_row(
            effect,
            context,
            u32::from(outcome_count),
            active,
        ))
    }

    fn record_scalar_row(
        &mut self,
        effect: GeneralChildEffectV1,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let tail = quantity.to_le_bytes();
        self.record(GeneralChildPlanV2::new_row(effect, context, 1, &tail))
    }

    fn record_uniform_aggregate(
        &mut self,
        effect: GeneralChildEffectV1,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError> {
        let quantities = [quantity; dclutch_general_codec::MAX_OUTCOMES];
        let mut tail = [0_u8; 8 * dclutch_general_codec::MAX_OUTCOMES];
        let active = encode_quantities(outcome_count, &quantities, &mut tail)?;
        self.record(GeneralChildPlanV2::new_aggregate(
            effect,
            context,
            u32::from(outcome_count),
            active,
        ))
    }
}

fn encode_quantities<'a>(
    outcome_count: u8,
    quantities: &[u64; dclutch_general_codec::MAX_OUTCOMES],
    output: &'a mut [u8; 8 * dclutch_general_codec::MAX_OUTCOMES],
) -> core::result::Result<&'a [u8], ChildExecutionError> {
    let count = usize::from(outcome_count);
    if count == 0 || count > quantities.len() {
        return Err(ChildExecutionError::Refused);
    }
    for (index, quantity) in quantities.iter().take(count).enumerate() {
        let offset = index.checked_mul(8).ok_or(ChildExecutionError::Refused)?;
        output
            .get_mut(offset..offset.saturating_add(8))
            .ok_or(ChildExecutionError::Refused)?
            .copy_from_slice(&quantity.to_le_bytes());
    }
    output
        .get(..count.saturating_mul(8))
        .ok_or(ChildExecutionError::Refused)
}

const fn is_zero(value: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < value.len() {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests;
