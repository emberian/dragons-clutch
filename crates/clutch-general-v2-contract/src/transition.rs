// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure poststate owners for the first General V2 lifecycle actions.

use crate::*;

/// Authenticated account identities and allocation facts for action 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitEpochTransitionV1 {
    /// Actual immutable MarketBinding PDA.
    pub market_binding: Id32,
    /// Actual mutable MarketRuntime PDA.
    pub market_runtime: Id32,
    /// New Epoch PDA.
    pub epoch: Id32,
    /// New EconomicDomain PDA.
    pub economic_domain: Id32,
    /// New Window PDA.
    pub window: Id32,
    /// New Budget PDA.
    pub budget: Id32,
    /// Signed root-funding payer.
    pub funding_payer: Id32,
    /// Init payload.
    pub payload: InitEpochPayloadV1,
    /// Inclusive Genesis-derived coordinate minimum.
    pub coordinate_domain_min: u128,
    /// Inclusive Genesis-derived coordinate maximum.
    pub coordinate_domain_max: u128,
    /// Epoch rent compartment.
    pub epoch_rent: DeletableRentOwnerV1,
    /// EconomicDomain rent compartment.
    pub economic_domain_rent: DeletableRentOwnerV1,
    /// Window rent compartment.
    pub window_rent: DeletableRentOwnerV1,
    /// Budget rent compartment.
    pub budget_rent: DeletableRentOwnerV1,
    /// Full prepaid SelectedCandidate rent principal.
    pub selected_candidate_rent_principal: u64,
    /// Epoch PDA bump.
    pub epoch_bump: u8,
    /// EconomicDomain PDA bump.
    pub economic_domain_bump: u8,
    /// Window PDA bump.
    pub window_bump: u8,
    /// Budget PDA bump.
    pub budget_bump: u8,
}

/// Exact action-2 pure poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitEpochPoststateV1 {
    /// MarketRuntime after consuming exactly one index/generation.
    pub market_runtime: MarketRuntimeV3AccountV1,
    /// Newly initialized counted Epoch.
    pub epoch: GeneralEpochV6AccountV1,
    /// Newly initialized immutable EconomicDomain artifact.
    pub economic_domain: EconomicDomainV2AccountV1,
    /// Newly initialized empty Window.
    pub window: CandidateWindowV4AccountV1,
    /// Newly initialized present-funded root Budget.
    pub budget: EpochBudgetV2AccountV1,
}

/// Construct all action-2 poststates and advance the MarketRuntime exactly once.
pub fn init_epoch_poststate_v1<B: Sha256BackendV1>(
    backend: &B,
    binding: MarketBindingV1,
    runtime: MarketRuntimeV3AccountV1,
    request: InitEpochTransitionV1,
) -> Result<InitEpochPoststateV1, CodecError> {
    binding.validate()?;
    runtime.validate()?;
    request.payload.validate()?;
    for id in [
        request.market_binding,
        request.market_runtime,
        request.epoch,
        request.economic_domain,
        request.window,
        request.budget,
        request.funding_payer,
    ] {
        require_live(id)?;
    }
    for rent in [
        request.epoch_rent,
        request.economic_domain_rent,
        request.window_rent,
        request.budget_rent,
    ] {
        rent.validate()?;
        if rent.payer != request.funding_payer {
            return Err(CodecError::MismatchedBinding);
        }
    }
    if binding.market != request.market_runtime
        || runtime.market_binding != request.market_binding
        || runtime.market_instance_v2_id != binding.market_instance_v2_id
        || request.payload.market_instance_v2_id != binding.market_instance_v2_id
        || request.payload.epoch_index != runtime.next_epoch_index
        || request.selected_candidate_rent_principal == 0
    {
        return Err(CodecError::MismatchedBinding);
    }
    let generation = runtime.next_epoch_generation;
    let semantics = EpochSemanticsV1 {
        market_instance_v2_id: binding.market_instance_v2_id,
        epoch_index: request.payload.epoch_index,
        generation,
        freeze_deadline_slot: request.payload.freeze_deadline_slot,
    };
    let epoch_semantics_digest = epoch_semantics_digest_v1(backend, semantics)?;
    let economic_transcript = EconomicDomainV2Transcript {
        relation_version: binding.relation_version,
        market_instance_v2_id: binding.market_instance_v2_id,
        epoch_semantics_digest,
        relation_policy_id: binding.relation_policy_id,
        price_measure_policy_v1_id: binding.price_measure_policy_v1_id,
        native_claim_basis_id: binding.native_claim_basis_id,
        epoch_index: request.payload.epoch_index,
        outcome_count: binding.outcome_count,
        price_scale: binding.price_scale,
        coordinate_domain_min: request.coordinate_domain_min,
        coordinate_domain_max: request.coordinate_domain_max,
    };
    economic_transcript.validate()?;

    let epoch = GeneralEpochV6AccountV1 {
        market_binding: request.market_binding,
        market_runtime: request.market_runtime,
        market_instance_v2_id: binding.market_instance_v2_id,
        economic_domain: request.economic_domain,
        window: request.window,
        budget: request.budget,
        order_set: Id32::ZERO,
        epoch_index: request.payload.epoch_index,
        generation,
        freeze_deadline_slot: request.payload.freeze_deadline_slot,
        frozen_slot: 0,
        candidate_bundle_count: 0,
        work_count: 0,
        selected_candidate_count: 0,
        rent: request.epoch_rent,
        phase: GeneralEpochPhaseV1::Open,
        stored_bump: request.epoch_bump,
        flags: 0,
    };
    let economic_domain = EconomicDomainV2AccountV1 {
        epoch: request.epoch,
        transcript: economic_transcript,
        rent: request.economic_domain_rent,
        stored_bump: request.economic_domain_bump,
        flags: 0,
    };
    let window = CandidateWindowV4AccountV1 {
        epoch: request.epoch,
        market: request.market_runtime,
        relation_policy_id: binding.relation_policy_id,
        admission_policy_id: binding.admission_policy_id,
        score_policy_id: binding.score_policy_id,
        freeze_deadline_slot: request.payload.freeze_deadline_slot,
        frozen_slot: 0,
        reveal_opens_slot: 0,
        submission_closes_slot: 0,
        verification_closes_slot: 0,
        finalized_slot: 0,
        admission_head: Id32::ZERO,
        best_candidate_node: Id32::ZERO,
        best_settlement_candidate_id: Id32::ZERO,
        selected_candidate_artifact: Id32::ZERO,
        best_rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
        admitted_count: 0,
        revealed_count: 0,
        verdict_count: 0,
        valid_verdict_count: 0,
        expired_commitment_count: 0,
        expired_unverified_count: 0,
        live_node_count: 0,
        closed_node_count: 0,
        best_ordinal: 0,
        epoch_generation: generation,
        rent: request.window_rent,
        rank_key_len: u8::try_from(SCORE_V2_Q_ACTIVE_RANK_BYTES)
            .map_err(|_| CodecError::InvalidCount)?,
        stored_bump: request.window_bump,
        flags: 0,
    };
    let budget = EpochBudgetV2AccountV1 {
        epoch: request.epoch,
        market: request.market_runtime,
        admission_policy_id: binding.admission_policy_id,
        funding_payer: request.funding_payer,
        epoch_generation: generation,
        freeze_initial: binding.freeze_reward,
        freeze_remaining: binding.freeze_reward,
        finalize_initial: binding.finalize_reward,
        finalize_remaining: binding.finalize_reward,
        solver_initial: binding.solver_prize,
        solver_remaining: binding.solver_prize,
        root_close_initial: binding.root_close_reward,
        root_close_remaining: binding.root_close_reward,
        selected_rent_initial: request.selected_candidate_rent_principal,
        selected_rent_remaining: request.selected_candidate_rent_principal,
        rent: request.budget_rent,
        freeze_paid: 0,
        finalize_paid: 0,
        solver_state: 0,
        selected_rent_state: 0,
        stored_bump: request.budget_bump,
        flags: 0,
    };
    let market_runtime = MarketRuntimeV3AccountV1 {
        next_epoch_index: runtime
            .next_epoch_index
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?,
        next_epoch_generation: runtime
            .next_epoch_generation
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?,
        created_epoch_count: runtime
            .created_epoch_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?,
        ..runtime
    };
    for result in [
        epoch.validate(),
        economic_domain.validate(),
        window.validate(),
        budget.validate(),
        market_runtime.validate(),
    ] {
        result?;
    }
    Ok(InitEpochPoststateV1 {
        market_runtime,
        epoch,
        economic_domain,
        window,
        budget,
    })
}

/// Authenticated action-6 inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezeEpochTransitionV1<'a> {
    /// Actual Epoch PDA.
    pub epoch_id: Id32,
    /// Actual MarketBinding PDA.
    pub market_binding_id: Id32,
    /// Actual MarketRuntime PDA.
    pub market_runtime_id: Id32,
    /// Current Clock slot.
    pub current_slot: u64,
    /// Strict action-6 payload.
    pub payload: FreezeEpochPayloadV1,
    /// Prestate Epoch.
    pub epoch: &'a GeneralEpochV6AccountV1,
    /// Immutable EconomicDomain artifact.
    pub economic_domain: &'a EconomicDomainV2AccountV1,
    /// Prestate Window.
    pub window: &'a CandidateWindowV4AccountV1,
    /// Prestate Budget.
    pub budget: &'a EpochBudgetV2AccountV1,
    /// Immutable Market binding.
    pub binding: &'a MarketBindingV1,
}

/// Exact action-6 pure poststate and funded reward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreezeEpochPoststateV1 {
    /// Frozen Epoch.
    pub epoch: GeneralEpochV6AccountV1,
    /// Window with exact `F/R/S/V` boundaries.
    pub window: CandidateWindowV4AccountV1,
    /// Budget after consuming only the freeze reward.
    pub budget: EpochBudgetV2AccountV1,
    /// Exact keeper reward authorized by the transition.
    pub keeper_reward: u64,
}

/// Freeze the canonical empty book and stamp all checked boundaries.
pub fn freeze_epoch_poststate_v1<B: Sha256BackendV1>(
    backend: &B,
    request: FreezeEpochTransitionV1<'_>,
) -> Result<FreezeEpochPoststateV1, CodecError> {
    request.epoch.validate()?;
    request.economic_domain.validate()?;
    request.window.validate()?;
    request.budget.validate()?;
    request.binding.validate()?;
    if request.epoch.phase != GeneralEpochPhaseV1::Open
        || request.current_slot < request.epoch.freeze_deadline_slot
        || request.current_slot == 0
        || request.epoch.market_binding != request.market_binding_id
        || request.epoch.market_runtime != request.market_runtime_id
        || request.binding.market != request.market_runtime_id
        || request.economic_domain.epoch != request.epoch_id
        || request.window.epoch != request.epoch_id
        || request.window.market != request.market_runtime_id
        || request.budget.epoch != request.epoch_id
        || request.budget.market != request.market_runtime_id
        || request.economic_domain.transcript.market_instance_v2_id
            != request.epoch.market_instance_v2_id
        || request.epoch.market_instance_v2_id != request.binding.market_instance_v2_id
        || request.economic_domain.transcript.relation_policy_id
            != request.binding.relation_policy_id
        || request
            .economic_domain
            .transcript
            .price_measure_policy_v1_id
            != request.binding.price_measure_policy_v1_id
        || request.economic_domain.transcript.native_claim_basis_id
            != request.binding.native_claim_basis_id
        || request.economic_domain.transcript.outcome_count != request.binding.outcome_count
        || request.economic_domain.transcript.price_scale != request.binding.price_scale
        || request.economic_domain.transcript.epoch_index != request.epoch.epoch_index
        || request.window.epoch_generation != request.epoch.generation
        || request.budget.epoch_generation != request.epoch.generation
        || request.window.freeze_deadline_slot != request.epoch.freeze_deadline_slot
        || request.window.relation_policy_id != request.binding.relation_policy_id
        || request.window.admission_policy_id != request.binding.admission_policy_id
        || request.window.score_policy_id != request.binding.score_policy_id
        || request.budget.admission_policy_id != request.binding.admission_policy_id
        || request.budget.freeze_paid != 0
        || request.budget.freeze_remaining != request.binding.freeze_reward
    {
        return Err(CodecError::MismatchedBinding);
    }
    let expected_semantics = request.epoch.semantics_digest(backend)?;
    if request.payload.epoch_semantics_id != expected_semantics
        || request.economic_domain.transcript.epoch_semantics_digest != expected_semantics
    {
        return Err(CodecError::MismatchedBinding);
    }
    let economic_digest = economic_domain_digest_v2(backend, request.economic_domain.transcript)?;
    let order_set = empty_order_set_digest_v1(backend, economic_digest)?;
    let reveal_opens_slot = request
        .current_slot
        .checked_add(request.binding.commit_span_slots)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let submission_closes_slot = reveal_opens_slot
        .checked_add(request.binding.reveal_span_slots)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let verification_closes_slot = submission_closes_slot
        .checked_add(request.binding.verification_span_slots)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let epoch = GeneralEpochV6AccountV1 {
        order_set,
        frozen_slot: request.current_slot,
        phase: GeneralEpochPhaseV1::Frozen,
        ..*request.epoch
    };
    let window = CandidateWindowV4AccountV1 {
        frozen_slot: request.current_slot,
        reveal_opens_slot,
        submission_closes_slot,
        verification_closes_slot,
        ..*request.window
    };
    let budget = EpochBudgetV2AccountV1 {
        freeze_remaining: 0,
        freeze_paid: 1,
        ..*request.budget
    };
    epoch.validate()?;
    window.validate()?;
    budget.validate()?;
    Ok(FreezeEpochPoststateV1 {
        epoch,
        window,
        budget,
        keeper_reward: request.binding.freeze_reward,
    })
}

/// Authenticated action-7 inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginCandidateTransitionV1<'a> {
    /// Actual Epoch PDA.
    pub epoch_id: Id32,
    /// Actual MarketRuntime PDA.
    pub market_runtime_id: Id32,
    /// Newly derived ordinal-owned node PDA.
    pub node_id: Id32,
    /// Signed funding payer.
    pub payer: Id32,
    /// Signed reveal authority.
    pub submitter: Id32,
    /// Refund destination.
    pub refund_destination: Id32,
    /// Immutable solver-prize destination.
    pub solver_destination: Id32,
    /// Current Clock slot.
    pub current_slot: u64,
    /// Strict action-7 payload.
    pub payload: BeginCandidatePayloadV1,
    /// New node rent compartment.
    pub node_rent: DeletableRentOwnerV1,
    /// New node PDA bump.
    pub node_bump: u8,
    /// Prestate Epoch.
    pub epoch: &'a GeneralEpochV6AccountV1,
    /// Prestate Window.
    pub window: &'a CandidateWindowV4AccountV1,
    /// Immutable Market binding.
    pub binding: &'a MarketBindingV1,
}

/// Exact action-7 pure poststate and commitment debit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginCandidatePoststateV1 {
    /// Epoch after incrementing its authoritative node count.
    pub epoch: GeneralEpochV6AccountV1,
    /// Window after assigning the one-based ordinal and reverse head.
    pub window: CandidateWindowV4AccountV1,
    /// Newly committed AdmissionNode.
    pub node: AdmissionNodeV3AccountV1,
    /// Exact payer debit: Node principal, bond, and cleanup reward only.
    pub commit_payer_funding: u64,
}

/// Admit one commitment and atomically update both count owners and the head.
pub fn begin_candidate_poststate_v1(
    request: BeginCandidateTransitionV1<'_>,
) -> Result<BeginCandidatePoststateV1, CodecError> {
    request.epoch.validate()?;
    request.window.validate()?;
    request.binding.validate()?;
    request.node_rent.validate()?;
    for id in [
        request.epoch_id,
        request.market_runtime_id,
        request.node_id,
        request.payer,
        request.submitter,
        request.refund_destination,
        request.solver_destination,
    ] {
        require_live(id)?;
    }
    if request.epoch.phase != GeneralEpochPhaseV1::Frozen
        || request.payload.epoch != request.epoch_id
        || request.epoch.market_runtime != request.market_runtime_id
        || request.binding.market != request.market_runtime_id
        || request.window.epoch != request.epoch_id
        || request.window.market != request.market_runtime_id
        || request.epoch.market_instance_v2_id != request.binding.market_instance_v2_id
        || request.window.relation_policy_id != request.binding.relation_policy_id
        || request.window.admission_policy_id != request.binding.admission_policy_id
        || request.window.score_policy_id != request.binding.score_policy_id
        || request.window.epoch_generation != request.epoch.generation
        || u64::from(request.epoch.candidate_bundle_count) != request.window.live_node_count
        || request.node_rent.payer != request.payer
        || request.current_slot < request.window.frozen_slot
        || request.current_slot >= request.window.reveal_opens_slot
    {
        return Err(CodecError::MismatchedBinding);
    }
    let ordinal = request
        .window
        .admitted_count
        .checked_add(1)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let count = request
        .epoch
        .candidate_bundle_count
        .checked_add(1)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let commit_payer_funding = request
        .node_rent
        .refundable_principal
        .checked_add(request.binding.bond_lamports)
        .and_then(|value| value.checked_add(request.binding.node_cleanup_reward))
        .ok_or(CodecError::ArithmeticOverflow)?;
    let node = AdmissionNodeV3AccountV1 {
        epoch: request.epoch_id,
        market: request.market_runtime_id,
        relation_policy_id: request.binding.relation_policy_id,
        node: request.node_id,
        previous_node: request.window.admission_head,
        admission_policy_id: request.binding.admission_policy_id,
        score_policy_id: request.binding.score_policy_id,
        commitment: request.payload.commitment,
        submitter_authority: request.submitter,
        solver_reward_destination: request.solver_destination,
        payer: request.payer,
        refund_destination: request.refund_destination,
        candidate_bundle_digest: Id32::ZERO,
        settlement_candidate_id: Id32::ZERO,
        base_relation_candidate_id: Id32::ZERO,
        settlement_witness_digest: Id32::ZERO,
        rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
        epoch_generation: request.epoch.generation,
        ordinal,
        committed_slot: request.current_slot,
        window_frozen_slot: request.window.frozen_slot,
        revealed_slot: 0,
        terminal_slot: 0,
        rent: request.node_rent,
        bond_lamports: request.binding.bond_lamports,
        cleanup_reward: request.binding.node_cleanup_reward,
        work_escrow_lamports: 0,
        work_funding_initial: 0,
        rank_key_len: 0,
        candidate_kind: SettlementCandidateKindV1::Direct,
        status: AdmissionNodeStatusV1::Committed,
        stored_bump: request.node_bump,
        flags: 0,
    };
    let epoch = GeneralEpochV6AccountV1 {
        candidate_bundle_count: count,
        ..*request.epoch
    };
    let window = CandidateWindowV4AccountV1 {
        admission_head: request.node_id,
        admitted_count: ordinal,
        live_node_count: request
            .window
            .live_node_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?,
        ..*request.window
    };
    node.validate()?;
    epoch.validate()?;
    window.validate()?;
    Ok(BeginCandidatePoststateV1 {
        epoch,
        window,
        node,
        commit_payer_funding,
    })
}

/// Authenticated action-8 variant-zero inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCandidateFeedTransitionV1<'a> {
    /// Actual Epoch PDA.
    pub epoch_id: Id32,
    /// Actual FeedStage PDA.
    pub feed_id: Id32,
    /// Current Clock slot.
    pub current_slot: u64,
    /// Strict action-8 variant-zero payload.
    pub payload: OpenCandidateFeedPayloadV1,
    /// FeedStage rent compartment.
    pub feed_rent: DeletableRentOwnerV1,
    /// Future ClearWork rent compartment capitalized now.
    pub work_rent: DeletableRentOwnerV1,
    /// FeedStage PDA bump.
    pub feed_bump: u8,
    /// Prestate Epoch.
    pub epoch: &'a GeneralEpochV6AccountV1,
    /// Prestate Window.
    pub window: &'a CandidateWindowV4AccountV1,
    /// Prestate committed node.
    pub node: &'a AdmissionNodeV3AccountV1,
    /// Immutable Market binding.
    pub binding: &'a MarketBindingV1,
    /// Immutable EconomicDomain artifact.
    pub economic_domain: &'a EconomicDomainV2AccountV1,
}

/// Exact action-8 open poststate and disjoint funding plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenCandidateFeedPoststateV1 {
    /// Window after incrementing its revealed count.
    pub window: CandidateWindowV4AccountV1,
    /// Node after opening its commitment and capitalizing Work.
    pub node: AdmissionNodeV3AccountV1,
    /// Newly created empty active-width FeedStage header.
    pub feed_stage: CandidateFeedHeaderV2,
    /// Exact commitment/reveal/lifetime funding decomposition.
    pub funding: CandidateFundingV1,
}

/// Project the sole exact writable byte range for one sequential feed segment.
///
/// The returned range is bounded to the segment's selected active-tail family;
/// adapters need not reproduce CandidateFeed offset or record-width arithmetic.
pub fn candidate_feed_segment_byte_range_v1(
    segment: CandidateFeedSegmentPayloadV1<'_>,
    stage: CandidateFeedHeaderV2,
) -> Result<core::ops::Range<usize>, CodecError> {
    segment.validate()?;
    stage.validate(false)?;
    let offsets = candidate_feed_tail_offsets_v2(stage)?;
    let (expected, limit, family_at) = match segment.kind {
        CandidateFeedWriteKindV1::Prices => (
            u16::from(stage.prices_written),
            u16::from(stage.outcome_count),
            offsets.prices_at(),
        ),
        CandidateFeedWriteKindV1::Fills => (
            u16::from(stage.fills_written),
            u16::from(stage.order_count),
            offsets.fills_at(),
        ),
        CandidateFeedWriteKindV1::QuantizedAtoms => (
            u16::from(stage.atoms_written),
            u16::from(stage.atom_count),
            offsets.atoms_at(),
        ),
        CandidateFeedWriteKindV1::SettlementSlices => {
            (stage.slices_written, stage.slice_count, offsets.slices_at())
        }
    };
    let end = segment.require_cursor(expected)?;
    if end > limit {
        return Err(CodecError::InvalidCount);
    }
    let relative_start = usize::from(segment.cursor)
        .checked_mul(segment.kind.record_bytes())
        .ok_or(CodecError::ArithmeticOverflow)?;
    let write_at = family_at
        .checked_add(relative_start)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let write_end = write_at
        .checked_add(segment.records.len())
        .ok_or(CodecError::ArithmeticOverflow)?;
    let family_bytes = usize::from(limit)
        .checked_mul(segment.kind.record_bytes())
        .ok_or(CodecError::ArithmeticOverflow)?;
    let family_end = family_at
        .checked_add(family_bytes)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if write_end > family_end || family_end > offsets.end() {
        return Err(CodecError::InvalidCount);
    }
    Ok(write_at..write_end)
}

/// Open one exact commitment and construct its empty active-width FeedStage.
pub fn open_candidate_feed_poststate_v1<B: Sha256BackendV1>(
    backend: &B,
    request: OpenCandidateFeedTransitionV1<'_>,
) -> Result<OpenCandidateFeedPoststateV1, CodecError> {
    request.epoch.validate()?;
    request.window.validate()?;
    request.node.validate()?;
    request.binding.validate()?;
    request.economic_domain.validate()?;
    request.payload.validate()?;
    request.feed_rent.validate()?;
    request.work_rent.validate()?;
    if request.epoch.phase != GeneralEpochPhaseV1::Frozen
        || request.node.status != AdmissionNodeStatusV1::Committed
        || request.payload.epoch != request.epoch_id
        || request.payload.node != request.node.node
        || request.node.epoch != request.epoch_id
        || request.window.epoch != request.epoch_id
        || request.economic_domain.epoch != request.epoch_id
        || request.node.market != request.epoch.market_runtime
        || request.binding.market != request.epoch.market_runtime
        || request.node.epoch_generation != request.epoch.generation
        || request.window.epoch_generation != request.epoch.generation
        || request.feed_rent.payer != request.node.payer
        || request.work_rent.payer != request.node.payer
        || request.current_slot < request.window.reveal_opens_slot
        || request.current_slot >= request.window.submission_closes_slot
        || request.node.window_frozen_slot != request.window.frozen_slot
        || request.node.relation_policy_id != request.binding.relation_policy_id
        || request.node.admission_policy_id != request.binding.admission_policy_id
        || request.node.score_policy_id != request.binding.score_policy_id
        || request.payload.outcome_count != request.binding.outcome_count
        || request.payload.basis_degree != request.binding.basis_degree
        || request.payload.price_scale != request.binding.price_scale
        || request.economic_domain.transcript.market_instance_v2_id
            != request.epoch.market_instance_v2_id
        || request.economic_domain.transcript.relation_policy_id
            != request.binding.relation_policy_id
        || request
            .economic_domain
            .transcript
            .price_measure_policy_v1_id
            != request.binding.price_measure_policy_v1_id
        || request.economic_domain.transcript.native_claim_basis_id
            != request.binding.native_claim_basis_id
        || request.economic_domain.transcript.outcome_count != request.binding.outcome_count
        || request.economic_domain.transcript.price_scale != request.binding.price_scale
    {
        return Err(CodecError::MismatchedBinding);
    }
    let kind_bit = match request.payload.candidate_kind {
        SettlementCandidateKindV1::Direct => 1,
        SettlementCandidateKindV1::CoveredDealer => 2,
    };
    if request.binding.candidate_kind_mask & kind_bit == 0 {
        return Err(CodecError::InvalidState);
    }
    let opening = CandidateCommitmentOpeningV1 {
        epoch: request.epoch_id,
        market: request.epoch.market_runtime,
        relation_policy_id: request.binding.relation_policy_id,
        admission_policy_id: request.binding.admission_policy_id,
        score_policy_id: request.binding.score_policy_id,
        frozen_slot: request.window.frozen_slot,
        submitter_authority: request.node.submitter_authority,
        solver_reward_destination: request.node.solver_reward_destination,
        candidate_bundle_digest: request.payload.candidate_bundle_digest,
        secret: request.payload.secret,
    };
    if candidate_commitment_v1(backend, opening)? != request.node.commitment {
        return Err(CodecError::MismatchedBinding);
    }
    let funding = required_candidate_funding_v1(
        *request.binding,
        request.payload.order_count,
        request.payload.slice_count,
        request.node.rent,
        request.feed_rent,
        request.work_rent,
    )?;
    let economic_domain_digest =
        economic_domain_digest_v2(backend, request.economic_domain.transcript)?;
    let node = AdmissionNodeV3AccountV1 {
        candidate_bundle_digest: request.payload.candidate_bundle_digest,
        settlement_candidate_id: request.payload.settlement_candidate_id,
        base_relation_candidate_id: request.payload.base_relation_candidate_id,
        settlement_witness_digest: request.payload.settlement_witness_digest,
        revealed_slot: request.current_slot,
        work_escrow_lamports: funding.work_allocation,
        work_funding_initial: funding.work_allocation,
        candidate_kind: request.payload.candidate_kind,
        status: AdmissionNodeStatusV1::Revealed,
        ..*request.node
    };
    let window = CandidateWindowV4AccountV1 {
        revealed_count: request
            .window
            .revealed_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?,
        ..*request.window
    };
    let feed_stage = CandidateFeedHeaderV2 {
        epoch: request.epoch_id,
        node: request.node.node,
        market: request.epoch.market_runtime,
        order_set: request.epoch.order_set,
        relation_policy_id: request.binding.relation_policy_id,
        economic_domain_digest,
        native_claim_basis_id: request.binding.native_claim_basis_id,
        candidate_price_digest: request.payload.candidate_price_digest,
        price_measure_policy_v1_id: request.binding.price_measure_policy_v1_id,
        settlement_candidate_id: request.payload.settlement_candidate_id,
        base_relation_candidate_id: request.payload.base_relation_candidate_id,
        settlement_witness_digest: request.payload.settlement_witness_digest,
        price_body_digest: request.payload.price_body_digest,
        epoch_generation: request.epoch.generation,
        virtual_split: request.payload.virtual_split,
        virtual_merge: request.payload.virtual_merge,
        honored_aon_mask: request.payload.honored_aon_mask,
        price_scale: request.payload.price_scale,
        common_denominator: request.payload.common_denominator,
        close_reward_lamports: request.binding.feed_close_reward,
        basis_degree: request.payload.basis_degree,
        outcome_count: request.payload.outcome_count,
        order_count: request.payload.order_count,
        atom_count: request.payload.atom_count,
        slice_count: request.payload.slice_count,
        prices_written: 0,
        fills_written: 0,
        atoms_written: 0,
        slices_written: 0,
        candidate_kind: request.payload.candidate_kind,
        price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
        quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
        rent: request.feed_rent,
        stored_bump: request.feed_bump,
        flags: 0,
    };
    node.validate()?;
    window.validate()?;
    feed_stage.validate(false)?;
    require_live(request.feed_id)?;
    Ok(OpenCandidateFeedPoststateV1 {
        window,
        node,
        feed_stage,
        funding,
    })
}

/// Advance exactly one action-8 FeedStage cursor after strict segment decode.
///
/// The returned header is the sole semantic mutation. The adapter copies the
/// borrowed exact records at the active-tail offset supplied by
/// [`candidate_feed_tail_v2`], encodes this header, and re-decodes the complete
/// stage before committing. That final decode owns cross-segment atom/slice
/// canonicality; this function owns sequential cursor arithmetic.
pub fn candidate_feed_segment_poststate_v1(
    segment: CandidateFeedSegmentPayloadV1<'_>,
    stage: CandidateFeedHeaderV2,
    node: AdmissionNodeV3AccountV1,
    window: CandidateWindowV4AccountV1,
    current_slot: u64,
) -> Result<CandidateFeedHeaderV2, CodecError> {
    let _write_range = candidate_feed_segment_byte_range_v1(segment, stage)?;
    stage.validate(false)?;
    node.validate()?;
    window.validate()?;
    if node.status != AdmissionNodeStatusV1::Revealed
        || segment.epoch != stage.epoch
        || segment.node != stage.node
        || node.epoch != stage.epoch
        || node.node != stage.node
        || window.epoch != stage.epoch
        || current_slot < window.reveal_opens_slot
        || current_slot >= window.submission_closes_slot
    {
        return Err(CodecError::MismatchedBinding);
    }
    let mut post = stage;
    match segment.kind {
        CandidateFeedWriteKindV1::Prices => {
            let end = segment.require_cursor(u16::from(stage.prices_written))?;
            if end > u16::from(stage.outcome_count) {
                return Err(CodecError::InvalidCount);
            }
            post.prices_written = u8::try_from(end).map_err(|_| CodecError::InvalidCount)?;
        }
        CandidateFeedWriteKindV1::Fills => {
            let end = segment.require_cursor(u16::from(stage.fills_written))?;
            if end > u16::from(stage.order_count) {
                return Err(CodecError::InvalidCount);
            }
            post.fills_written = u8::try_from(end).map_err(|_| CodecError::InvalidCount)?;
        }
        CandidateFeedWriteKindV1::QuantizedAtoms => {
            let end = segment.require_cursor(u16::from(stage.atoms_written))?;
            if end > u16::from(stage.atom_count) {
                return Err(CodecError::InvalidCount);
            }
            post.atoms_written = u8::try_from(end).map_err(|_| CodecError::InvalidCount)?;
        }
        CandidateFeedWriteKindV1::SettlementSlices => {
            let end = segment.require_cursor(stage.slices_written)?;
            if end > stage.slice_count {
                return Err(CodecError::InvalidCount);
            }
            post.slices_written = end;
        }
    }
    post.validate(false)?;
    Ok(post)
}

/// Validate a complete FeedStage and every General-owned digest before action 9.
///
/// This bounded identity-lab owner deliberately requires zero orders and zero
/// slices. RelationV2 remains the sole owner of the base economic candidate ID.
pub fn seal_empty_book_candidate_v1<B: Sha256BackendV1>(
    backend: &B,
    candidate_feed: Id32,
    stage_bytes: &[u8],
    node: AdmissionNodeV3AccountV1,
    binding: MarketBindingV1,
    economic_domain: EconomicDomainV2AccountV1,
) -> Result<CandidateFeedHeaderV2, CodecError> {
    require_live(candidate_feed)?;
    node.validate()?;
    binding.validate()?;
    economic_domain.validate()?;
    let header = CandidateFeedHeaderV2::decode_account(stage_bytes, false)?;
    let economic_digest = economic_domain_digest_v2(backend, economic_domain.transcript)?;
    if node.status != AdmissionNodeStatusV1::Revealed
        || header.node != node.node
        || header.epoch != node.epoch
        || header.market != node.market
        || header.relation_policy_id != node.relation_policy_id
        || header.candidate_kind != node.candidate_kind
        || header.candidate_kind != SettlementCandidateKindV1::Direct
        || header.settlement_candidate_id != header.base_relation_candidate_id
        || header.settlement_candidate_id != node.settlement_candidate_id
        || header.base_relation_candidate_id != node.base_relation_candidate_id
        || header.settlement_witness_digest != node.settlement_witness_digest
        || binding.market != node.market
        || header.order_count != 0
        || header.slice_count != 0
        || economic_domain.epoch != node.epoch
        || economic_domain.transcript.market_instance_v2_id != binding.market_instance_v2_id
        || economic_domain.transcript.relation_policy_id != binding.relation_policy_id
        || economic_domain.transcript.price_measure_policy_v1_id
            != binding.price_measure_policy_v1_id
        || economic_domain.transcript.native_claim_basis_id != binding.native_claim_basis_id
        || economic_domain.transcript.outcome_count != binding.outcome_count
        || economic_domain.transcript.price_scale != binding.price_scale
        || header.outcome_count != binding.outcome_count
        || header.basis_degree != binding.basis_degree
        || header.price_scale != binding.price_scale
        || header.native_claim_basis_id != binding.native_claim_basis_id
        || header.price_measure_policy_v1_id != binding.price_measure_policy_v1_id
        || header.economic_domain_digest != economic_digest
        || header.order_set != empty_order_set_digest_v1(backend, economic_digest)?
    {
        return Err(CodecError::MismatchedBinding);
    }
    let body = quantized_witness_body_digest_v3(backend, candidate_feed, stage_bytes, false)?;
    let bundle = candidate_bundle_digest_v1(backend, stage_bytes, false)?;
    let witness = empty_settlement_witness_digest_v1(backend, header.base_relation_candidate_id)?;
    let tail = candidate_feed_tail_v2(stage_bytes, header)?;
    let mut prices = [0u64; MAX_OUTCOMES];
    for (index, record) in tail.prices_le().chunks_exact(8).enumerate() {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(record);
        prices[index] = u64::from_le_bytes(bytes);
    }
    let candidate_price = price_semantics_digest_v2(
        backend,
        PriceSemanticsV2 {
            domain: economic_domain.transcript,
            prices,
        },
    )?;
    if body != header.price_body_digest
        || bundle != node.candidate_bundle_digest
        || witness != header.settlement_witness_digest
        || candidate_price != header.candidate_price_digest
    {
        return Err(CodecError::MismatchedBinding);
    }
    // Re-run complete sealed semantics while the account still carries the
    // stage tag. The adapter may now change only its first two bytes.
    header.validate(true)?;
    Ok(header)
}

/// Authenticated action-10 inputs for the bounded empty-book path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitClearWorkTransitionV1<'a> {
    /// Actual Epoch PDA.
    pub epoch_id: Id32,
    /// Actual sealed Feed PDA.
    pub feed_id: Id32,
    /// Newly derived ClearWork PDA.
    pub work_id: Id32,
    /// ClearWork rent compartment funded at reveal.
    pub work_rent: DeletableRentOwnerV1,
    /// ClearWork PDA bump.
    pub work_bump: u8,
    /// Prestate Epoch.
    pub epoch: &'a GeneralEpochV6AccountV1,
    /// Prestate revealed node.
    pub node: &'a AdmissionNodeV3AccountV1,
    /// Authenticated sealed Feed header.
    pub feed: &'a CandidateFeedHeaderV2,
    /// Immutable Market binding.
    pub binding: &'a MarketBindingV1,
}

/// Exact action-10 pure poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitClearWorkPoststateV1 {
    /// Epoch after incrementing its authoritative Work count.
    pub epoch: GeneralEpochV6AccountV1,
    /// Node after moving its entire Work compartment.
    pub node: AdmissionNodeV3AccountV1,
    /// Newly initialized ClearWork header.
    pub work: ClearWorkHeaderV2,
    /// Exact active-width Work allocation bytes.
    pub work_account_bytes: usize,
}

/// Move the reveal-funded Work compartment into one exact-size ClearWork.
pub fn init_clear_work_poststate_v1(
    request: InitClearWorkTransitionV1<'_>,
) -> Result<InitClearWorkPoststateV1, CodecError> {
    request.epoch.validate()?;
    request.node.validate()?;
    request.feed.validate(true)?;
    request.binding.validate()?;
    request.work_rent.validate()?;
    for id in [request.epoch_id, request.feed_id, request.work_id] {
        require_live(id)?;
    }
    if request.epoch.phase != GeneralEpochPhaseV1::Frozen
        || request.node.status != AdmissionNodeStatusV1::Revealed
        || request.node.epoch != request.epoch_id
        || request.feed.epoch != request.epoch_id
        || request.feed.node != request.node.node
        || request.feed.market != request.epoch.market_runtime
        || request.node.market != request.epoch.market_runtime
        || request.feed.epoch_generation != request.epoch.generation
        || request.node.epoch_generation != request.epoch.generation
        || request.feed.order_set != request.epoch.order_set
        || request.binding.market != request.epoch.market_runtime
        || request.feed.relation_policy_id != request.binding.relation_policy_id
        || request.feed.native_claim_basis_id != request.binding.native_claim_basis_id
        || request.feed.price_measure_policy_v1_id != request.binding.price_measure_policy_v1_id
        || request.feed.outcome_count != request.binding.outcome_count
        || request.feed.basis_degree != request.binding.basis_degree
        || request.feed.price_scale != request.binding.price_scale
        || request.work_rent.payer != request.node.payer
        || request.feed.order_count != 0
        || request.feed.slice_count != 0
    {
        return Err(CodecError::MismatchedBinding);
    }
    let reward_reserve = request
        .binding
        .price_check_reward
        .checked_add(request.binding.completion_reward)
        .and_then(|value| value.checked_add(request.binding.work_close_reward))
        .ok_or(CodecError::ArithmeticOverflow)?;
    let work_allocation = request
        .work_rent
        .refundable_principal
        .checked_add(reward_reserve)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if request.node.work_escrow_lamports != work_allocation
        || request.node.work_funding_initial != work_allocation
    {
        return Err(CodecError::MismatchedBinding);
    }
    let work = ClearWorkHeaderV2 {
        epoch: request.epoch_id,
        node: request.node.node,
        market: request.epoch.market_runtime,
        order_set: request.epoch.order_set,
        feed: request.feed_id,
        candidate_bundle_digest: request.node.candidate_bundle_digest,
        settlement_candidate_id: request.feed.settlement_candidate_id,
        base_relation_candidate_id: request.feed.base_relation_candidate_id,
        relation_policy_id: request.feed.relation_policy_id,
        economic_domain_digest: request.feed.economic_domain_digest,
        native_claim_basis_id: request.feed.native_claim_basis_id,
        candidate_price_digest: request.feed.candidate_price_digest,
        price_measure_policy_v1_id: request.feed.price_measure_policy_v1_id,
        score_policy_id: request.node.score_policy_id,
        price_body_digest: request.feed.price_body_digest,
        epoch_generation: request.epoch.generation,
        rent: request.work_rent,
        reward_remaining: reward_reserve,
        reward_earned: 0,
        slice_count: 0,
        slice_cursor: 0,
        outcome_count: request.feed.outcome_count,
        order_count: 0,
        order_cursor: 0,
        phase: 0,
        candidate_kind: request.feed.candidate_kind,
        price_witness_schema: request.feed.price_witness_schema,
        quantized_semantics_version: request.feed.quantized_semantics_version,
        stored_bump: request.work_bump,
        flags: 0,
        sha256: Sha256CheckpointV1 {
            state: SHA256_INITIAL_STATE_V1,
            block: [0; 64],
            block_len: 0,
            total_len: 0,
        },
    };
    let epoch = GeneralEpochV6AccountV1 {
        work_count: request
            .epoch
            .work_count
            .checked_add(1)
            .ok_or(CodecError::ArithmeticOverflow)?,
        ..*request.epoch
    };
    let node = AdmissionNodeV3AccountV1 {
        work_escrow_lamports: 0,
        ..*request.node
    };
    work.validate()?;
    epoch.validate()?;
    node.validate()?;
    Ok(InitClearWorkPoststateV1 {
        epoch,
        node,
        work,
        work_account_bytes: clear_work_account_len(request.feed.outcome_count, 0)?,
    })
}

/// RelationV2/PriceMeasure result projected into the General mutation owner.
///
/// This is deliberately a forgeable pure-data projection, not a verified
/// capability. A live adapter must construct it only from the successful
/// return of the separately reviewed RelationV2 and PriceMeasure owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmptyBookVerificationVerdictV1 {
    /// Economically valid candidate and exact score inputs.
    Valid(ScoreV2QComponentsV1),
    /// Well-formed and authenticated, but economically invalid.
    Refused,
}

/// Authenticated action-14 inputs for the bounded empty-book verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteCandidateVerificationTransitionV1<'a> {
    /// Current Clock slot.
    pub current_slot: u64,
    /// Checked external-verifier projection.
    pub verdict: EmptyBookVerificationVerdictV1,
    /// Prestate Epoch.
    pub epoch: &'a GeneralEpochV6AccountV1,
    /// Prestate Window.
    pub window: &'a CandidateWindowV4AccountV1,
    /// Prestate revealed node.
    pub node: &'a AdmissionNodeV3AccountV1,
    /// Prestate empty-book ClearWork.
    pub work: &'a ClearWorkHeaderV2,
    /// Immutable Market binding.
    pub binding: &'a MarketBindingV1,
}

/// Exact successful action-14 poststate, including checked-invalid refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteCandidateVerificationPoststateV1 {
    /// Window after one exhaustive verdict and possible best-rank replacement.
    pub window: CandidateWindowV4AccountV1,
    /// Terminal valid or refused AdmissionNode.
    pub node: AdmissionNodeV3AccountV1,
    /// Complete Work with only its close reward remaining.
    pub work: ClearWorkHeaderV2,
    /// Exact checked-work reward authorized now.
    pub keeper_reward: u64,
}

/// Terminalize one bounded zero-order/zero-slice verification result.
pub fn complete_candidate_verification_poststate_v1(
    request: CompleteCandidateVerificationTransitionV1<'_>,
) -> Result<CompleteCandidateVerificationPoststateV1, CodecError> {
    request.epoch.validate()?;
    request.window.validate()?;
    request.node.validate()?;
    request.work.validate()?;
    request.binding.validate()?;
    if request.epoch.phase != GeneralEpochPhaseV1::Frozen
        || request.node.status != AdmissionNodeStatusV1::Revealed
        || request.work.phase != 0
        || request.work.reward_earned != 0
        || request.epoch.work_count == 0
        || request.work.order_count != 0
        || request.work.slice_count != 0
        || request.work.order_cursor != 0
        || request.work.slice_cursor != 0
        || request.node.epoch != request.work.epoch
        || request.node.node != request.work.node
        || request.node.market != request.work.market
        || request.node.epoch_generation != request.work.epoch_generation
        || request.epoch.generation != request.work.epoch_generation
        || request.window.epoch != request.work.epoch
        || request.window.epoch_generation != request.work.epoch_generation
        || request.current_slot < request.window.submission_closes_slot
        || request.current_slot >= request.window.verification_closes_slot
        || request.work.candidate_bundle_digest != request.node.candidate_bundle_digest
        || request.work.settlement_candidate_id != request.node.settlement_candidate_id
        || request.work.base_relation_candidate_id != request.node.base_relation_candidate_id
        || request.work.relation_policy_id != request.node.relation_policy_id
        || request.work.score_policy_id != request.node.score_policy_id
        || request.binding.market != request.work.market
        || request.binding.relation_policy_id != request.work.relation_policy_id
        || request.binding.score_policy_id != request.work.score_policy_id
        || request.work.sha256
            != (Sha256CheckpointV1 {
                state: SHA256_INITIAL_STATE_V1,
                block: [0; 64],
                block_len: 0,
                total_len: 0,
            })
    {
        return Err(CodecError::MismatchedBinding);
    }
    let keeper_reward = request
        .binding
        .price_check_reward
        .checked_add(request.binding.completion_reward)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let reward_remaining = request
        .work
        .reward_remaining
        .checked_sub(keeper_reward)
        .ok_or(CodecError::InvalidState)?;
    if reward_remaining != request.binding.work_close_reward {
        return Err(CodecError::MismatchedBinding);
    }
    let reward_earned = request
        .work
        .reward_earned
        .checked_add(keeper_reward)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let mut node = *request.node;
    let mut window = *request.window;
    window.verdict_count = window
        .verdict_count
        .checked_add(1)
        .ok_or(CodecError::ArithmeticOverflow)?;
    match request.verdict {
        EmptyBookVerificationVerdictV1::Valid(score) => {
            if score.settlement_candidate_id != node.settlement_candidate_id {
                return Err(CodecError::MismatchedBinding);
            }
            let rank = encode_score_v2_q_first_admitted_tie_v1(
                score,
                FirstAdmittedTieV1 {
                    ordinal: node.ordinal,
                },
            )?;
            node.rank_key = rank;
            node.rank_key_len =
                u8::try_from(SCORE_V2_Q_ACTIVE_RANK_BYTES).map_err(|_| CodecError::InvalidCount)?;
            node.status = AdmissionNodeStatusV1::VerifiedValid;
            window.valid_verdict_count = window
                .valid_verdict_count
                .checked_add(1)
                .ok_or(CodecError::ArithmeticOverflow)?;
            if window.best_candidate_node.is_zero() || rank > window.best_rank_key {
                window.best_candidate_node = node.node;
                window.best_settlement_candidate_id = node.settlement_candidate_id;
                window.best_rank_key = rank;
                window.best_ordinal = node.ordinal;
            }
        }
        EmptyBookVerificationVerdictV1::Refused => {
            node.rank_key = [0; SCORE_V2_Q_RANK_CAPACITY];
            node.rank_key_len = 0;
            node.status = AdmissionNodeStatusV1::VerifiedRefused;
        }
    }
    node.terminal_slot = request.current_slot;
    let work = ClearWorkHeaderV2 {
        reward_remaining,
        reward_earned,
        phase: 3,
        ..*request.work
    };
    node.validate()?;
    window.validate()?;
    work.validate()?;
    Ok(CompleteCandidateVerificationPoststateV1 {
        window,
        node,
        work,
        keeper_reward,
    })
}

/// Authenticated action-15 inputs after exhaustive terminal candidate counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeSelectionTransitionV1<'a> {
    /// Actual Epoch PDA.
    pub epoch_id: Id32,
    /// Actual Window PDA.
    pub window_id: Id32,
    /// Actual MarketBinding PDA.
    pub market_binding_id: Id32,
    /// Actual best-node Feed PDA retained for settlement.
    pub feed_id: Id32,
    /// Newly derived SelectedCandidate PDA.
    pub selected_candidate_id: Id32,
    /// Current Clock slot.
    pub current_slot: u64,
    /// SelectedCandidate rent with Budget's original payer and full principal.
    pub selected_rent: DeletableRentOwnerV1,
    /// SelectedCandidate PDA bump.
    pub selected_bump: u8,
    /// Prestate Epoch.
    pub epoch: &'a GeneralEpochV6AccountV1,
    /// Prestate Window.
    pub window: &'a CandidateWindowV4AccountV1,
    /// Prestate Budget.
    pub budget: &'a EpochBudgetV2AccountV1,
    /// Authenticated best AdmissionNode.
    pub node: &'a AdmissionNodeV3AccountV1,
    /// Authenticated sealed Feed header.
    pub feed: &'a CandidateFeedHeaderV2,
}

/// Exact action-15 pure poststate and present-funded reward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizeSelectionPoststateV1 {
    /// Epoch after incrementing the counted selected artifact.
    pub epoch: GeneralEpochV6AccountV1,
    /// Window after deleting every working-best field and storing the artifact.
    pub window: CandidateWindowV4AccountV1,
    /// Budget after moving selected rent and paying only finalization reward.
    pub budget: EpochBudgetV2AccountV1,
    /// Newly materialized downstream settlement authority.
    pub selected_candidate: SelectedCandidateV1AccountV1,
    /// Exact finalizer reward authorized by this transition.
    pub finalizer_reward: u64,
}

/// Materialize one selected authority without paying the solver prize.
pub fn finalize_selection_poststate_v1(
    request: FinalizeSelectionTransitionV1<'_>,
) -> Result<FinalizeSelectionPoststateV1, CodecError> {
    request.epoch.validate()?;
    request.window.validate()?;
    request.budget.validate()?;
    request.node.validate()?;
    request.feed.validate(true)?;
    request.selected_rent.validate()?;
    for id in [
        request.epoch_id,
        request.window_id,
        request.market_binding_id,
        request.feed_id,
        request.selected_candidate_id,
    ] {
        require_live(id)?;
    }
    if request.epoch.phase != GeneralEpochPhaseV1::Frozen
        || request.epoch.selected_candidate_count != 0
        || request.epoch.market_binding != request.market_binding_id
        || request.window.finalized_slot != 0
        || request.window.valid_verdict_count == 0
        || request.current_slot < request.window.submission_closes_slot
        || request
            .window
            .revealed_count
            .checked_add(request.window.expired_commitment_count)
            .ok_or(CodecError::ArithmeticOverflow)?
            != request.window.admitted_count
        || request
            .window
            .verdict_count
            .checked_add(request.window.expired_unverified_count)
            .ok_or(CodecError::ArithmeticOverflow)?
            != request.window.revealed_count
        || u64::from(request.epoch.candidate_bundle_count) != request.window.live_node_count
        || request.window.epoch != request.epoch_id
        || request.window.epoch_generation != request.epoch.generation
        || request.budget.epoch != request.epoch_id
        || request.budget.market != request.epoch.market_runtime
        || request.node.market != request.epoch.market_runtime
        || request.budget.epoch_generation != request.epoch.generation
        || request.node.epoch != request.epoch_id
        || request.node.epoch_generation != request.epoch.generation
        || request.feed.epoch != request.epoch_id
        || request.feed.epoch_generation != request.epoch.generation
        || request.window.best_candidate_node != request.node.node
        || request.window.best_settlement_candidate_id != request.node.settlement_candidate_id
        || request.window.best_rank_key != request.node.rank_key
        || request.window.best_ordinal != request.node.ordinal
        || request.node.status != AdmissionNodeStatusV1::VerifiedValid
        || request.feed.node != request.node.node
        || request.feed.market != request.epoch.market_runtime
        || request.node.market != request.epoch.market_runtime
        || request.feed.order_set != request.epoch.order_set
        || request.feed.settlement_candidate_id != request.node.settlement_candidate_id
        || request.feed.base_relation_candidate_id != request.node.base_relation_candidate_id
        || request.feed.settlement_witness_digest != request.node.settlement_witness_digest
        || request.budget.finalize_paid != 0
        || request.budget.finalize_remaining != request.budget.finalize_initial
        || request.budget.selected_rent_state != 0
        || request.budget.selected_rent_remaining != request.budget.selected_rent_initial
        || request.selected_rent.payer != request.budget.funding_payer
        || request.selected_rent.refundable_principal != request.budget.selected_rent_initial
    {
        return Err(CodecError::MismatchedBinding);
    }
    let selected_candidate = SelectedCandidateV1AccountV1 {
        epoch: request.epoch_id,
        market: request.epoch.market_runtime,
        window: request.window_id,
        market_binding: request.market_binding_id,
        source_admission_node: request.node.node,
        selected_feed: request.feed_id,
        order_set: request.epoch.order_set,
        economic_domain_digest: request.feed.economic_domain_digest,
        candidate_bundle_digest: request.node.candidate_bundle_digest,
        settlement_candidate_id: request.node.settlement_candidate_id,
        base_relation_candidate_id: request.node.base_relation_candidate_id,
        settlement_witness_digest: request.node.settlement_witness_digest,
        relation_policy_id: request.node.relation_policy_id,
        price_measure_policy_v1_id: request.feed.price_measure_policy_v1_id,
        native_claim_basis_id: request.feed.native_claim_basis_id,
        candidate_price_digest: request.feed.candidate_price_digest,
        price_body_digest: request.feed.price_body_digest,
        score_policy_id: request.node.score_policy_id,
        solver_reward_destination: request.node.solver_reward_destination,
        rank_key: request.node.rank_key,
        epoch_generation: request.epoch.generation,
        ordinal: request.node.ordinal,
        selected_slot: request.current_slot,
        slice_count: request.feed.slice_count,
        next_slice_index: 0,
        rent: request.selected_rent,
        candidate_kind: request.node.candidate_kind,
        price_witness_schema: request.feed.price_witness_schema,
        quantized_semantics_version: request.feed.quantized_semantics_version,
        rank_key_len: u8::try_from(SCORE_V2_Q_ACTIVE_RANK_BYTES)
            .map_err(|_| CodecError::InvalidCount)?,
        // The empty slice set is already exhaustively materialized. Leaving
        // it open would create a vacuous progress state that no slice action
        // could advance and would make the artifact permanently unretirable.
        entitlement_state: if request.feed.slice_count == 0 { 2 } else { 0 },
        stored_bump: request.selected_bump,
        flags: 0,
    };
    let epoch = GeneralEpochV6AccountV1 {
        selected_candidate_count: 1,
        phase: GeneralEpochPhaseV1::Finalized,
        ..*request.epoch
    };
    let window = CandidateWindowV4AccountV1 {
        finalized_slot: request.current_slot,
        best_candidate_node: Id32::ZERO,
        best_settlement_candidate_id: Id32::ZERO,
        selected_candidate_artifact: request.selected_candidate_id,
        best_rank_key: [0; SCORE_V2_Q_RANK_CAPACITY],
        best_ordinal: 0,
        ..*request.window
    };
    let budget = EpochBudgetV2AccountV1 {
        finalize_remaining: 0,
        selected_rent_remaining: 0,
        finalize_paid: 1,
        selected_rent_state: 1,
        ..*request.budget
    };
    selected_candidate.validate()?;
    epoch.validate()?;
    window.validate()?;
    budget.validate()?;
    Ok(FinalizeSelectionPoststateV1 {
        epoch,
        window,
        budget,
        selected_candidate,
        finalizer_reward: request.budget.finalize_initial,
    })
}

fn require_live(id: Id32) -> Result<(), CodecError> {
    if id.is_zero() {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    struct Sha;

    impl Sha256BackendV1 for Sha {
        fn sha256(&self, parts: &[&[u8]]) -> [u8; ID_BYTES] {
            let mut hash = Sha256::new();
            for part in parts {
                hash.update(part);
            }
            hash.finalize().into()
        }
    }

    fn id(byte: u8) -> Id32 {
        Id32::new([byte; ID_BYTES]).unwrap()
    }

    fn rent(payer: Id32, principal: u64) -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1 {
            payer,
            refundable_principal: principal,
            donation_floor: 3,
        }
    }

    fn binding(runtime_id: Id32) -> MarketBindingV1 {
        MarketBindingV1 {
            market: runtime_id,
            market_genesis_profile_v2_id: id(2),
            market_instance_v2_id: id(3),
            series_plan_v5_id: id(4),
            series_funding_terms_v2_id: id(5),
            relation_policy_id: id(6),
            price_measure_policy_v1_id: id(7),
            native_claim_basis_id: id(8),
            admission_policy_id: id(9),
            score_policy_id: id(10),
            settlement_policy_id: id(11),
            neutral_sink: id(12),
            price_scale: 100,
            commit_span_slots: 10,
            reveal_span_slots: 10,
            verification_span_slots: 20,
            bond_lamports: 1000,
            invalidity_penalty: 100,
            abandonment_penalty: 50,
            node_cleanup_reward: 10,
            price_check_reward: 2,
            order_reward: 3,
            slice_reward: 4,
            completion_reward: 5,
            work_close_reward: 6,
            feed_close_reward: 7,
            freeze_reward: 8,
            finalize_reward: 9,
            solver_prize: 10,
            root_close_reward: 11,
            relation_version: 2,
            outcome_count: 3,
            basis_degree: 2,
            rank_key_len: 88,
            candidate_kind_mask: 1,
            stored_bump: 1,
            flags: 0,
        }
    }

    #[test]
    fn init_freeze_begin_and_open_have_one_exact_poststate() {
        let payer = id(30);
        let binding_id = id(20);
        let runtime_id = id(21);
        let epoch_id = id(22);
        let economic_id = id(23);
        let window_id = id(24);
        let budget_id = id(25);
        let feed_id = id(26);
        let work_id = id(27);
        let node_id = id(28);
        let binding = binding(runtime_id);
        let runtime = MarketRuntimeV3AccountV1 {
            market_binding: binding_id,
            market_instance_v2_id: binding.market_instance_v2_id,
            next_epoch_index: 0,
            next_epoch_generation: 1,
            created_epoch_count: 0,
            retired_epoch_count: 0,
            rent: rent(payer, 100),
            stored_bump: 1,
            flags: 0,
        };
        let initialized = init_epoch_poststate_v1(
            &Sha,
            binding,
            runtime,
            InitEpochTransitionV1 {
                market_binding: binding_id,
                market_runtime: runtime_id,
                epoch: epoch_id,
                economic_domain: economic_id,
                window: window_id,
                budget: budget_id,
                funding_payer: payer,
                payload: InitEpochPayloadV1 {
                    market_instance_v2_id: binding.market_instance_v2_id,
                    epoch_index: 0,
                    freeze_deadline_slot: 100,
                },
                coordinate_domain_min: 10,
                coordinate_domain_max: 1_000,
                epoch_rent: rent(payer, 101),
                economic_domain_rent: rent(payer, 102),
                window_rent: rent(payer, 103),
                budget_rent: rent(payer, 104),
                selected_candidate_rent_principal: 105,
                epoch_bump: 2,
                economic_domain_bump: 3,
                window_bump: 4,
                budget_bump: 5,
            },
        )
        .unwrap();
        assert_eq!(initialized.market_runtime.next_epoch_index, 1);
        assert_eq!(initialized.epoch.market_runtime, runtime_id);
        let semantics = initialized.epoch.semantics_digest(&Sha).unwrap();
        let frozen = freeze_epoch_poststate_v1(
            &Sha,
            FreezeEpochTransitionV1 {
                epoch_id,
                market_binding_id: binding_id,
                market_runtime_id: runtime_id,
                current_slot: 100,
                payload: FreezeEpochPayloadV1 {
                    epoch_semantics_id: semantics,
                },
                epoch: &initialized.epoch,
                economic_domain: &initialized.economic_domain,
                window: &initialized.window,
                budget: &initialized.budget,
                binding: &binding,
            },
        )
        .unwrap();
        assert_eq!(frozen.window.reveal_opens_slot, 110);
        assert_eq!(frozen.window.submission_closes_slot, 120);
        assert_eq!(frozen.window.verification_closes_slot, 140);
        assert_eq!(frozen.keeper_reward, binding.freeze_reward);

        let mut candidate_feed = CandidateFeedHeaderV2 {
            epoch: epoch_id,
            node: node_id,
            market: runtime_id,
            order_set: frozen.epoch.order_set,
            relation_policy_id: binding.relation_policy_id,
            economic_domain_digest: economic_domain_digest_v2(
                &Sha,
                initialized.economic_domain.transcript,
            )
            .unwrap(),
            native_claim_basis_id: binding.native_claim_basis_id,
            candidate_price_digest: id(40),
            price_measure_policy_v1_id: binding.price_measure_policy_v1_id,
            settlement_candidate_id: id(41),
            base_relation_candidate_id: id(41),
            settlement_witness_digest: empty_settlement_witness_digest_v1(&Sha, id(41)).unwrap(),
            price_body_digest: id(42),
            epoch_generation: frozen.epoch.generation,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            price_scale: 100,
            common_denominator: 1,
            close_reward_lamports: binding.feed_close_reward,
            basis_degree: 2,
            outcome_count: 3,
            order_count: 0,
            atom_count: 1,
            slice_count: 0,
            prices_written: 3,
            fills_written: 0,
            atoms_written: 1,
            slices_written: 0,
            candidate_kind: SettlementCandidateKindV1::Direct,
            price_witness_schema: PRICE_MEASURE_WITNESS_SCHEMA_V3,
            quantized_semantics_version: QUANTIZED_PRICE_MEASURE_SEMANTICS_V1,
            rent: rent(payer, 200),
            stored_bump: 7,
            flags: 0,
        };
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[..3].copy_from_slice(&[20, 30, 50]);
        candidate_feed.candidate_price_digest = price_semantics_digest_v2(
            &Sha,
            PriceSemanticsV2 {
                domain: initialized.economic_domain.transcript,
                prices,
            },
        )
        .unwrap();
        const FEED_LEN: usize = CANDIDATE_FEED_HEADER_BYTES + 24 + 24;
        let mut full_stage = [0u8; FEED_LEN];
        candidate_feed
            .encode(&mut full_stage[..CANDIDATE_FEED_HEADER_BYTES], false)
            .unwrap();
        let prices_at = CANDIDATE_FEED_HEADER_BYTES;
        full_stage[prices_at..prices_at + 8].copy_from_slice(&20u64.to_le_bytes());
        full_stage[prices_at + 8..prices_at + 16].copy_from_slice(&30u64.to_le_bytes());
        full_stage[prices_at + 16..prices_at + 24].copy_from_slice(&50u64.to_le_bytes());
        let atoms_at = prices_at + 24;
        full_stage[atoms_at..atoms_at + 16].copy_from_slice(&50u128.to_le_bytes());
        full_stage[atoms_at + 16..atoms_at + 24].copy_from_slice(&1u64.to_le_bytes());
        let body = quantized_witness_body_digest_v3(&Sha, feed_id, &full_stage, false).unwrap();
        candidate_feed.price_body_digest = body;
        candidate_feed
            .encode(&mut full_stage[..CANDIDATE_FEED_HEADER_BYTES], false)
            .unwrap();
        let bundle = candidate_bundle_digest_v1(&Sha, &full_stage, false).unwrap();
        let submitter = id(31);
        let solver = id(32);
        let secret = [33; 32];
        let commitment = candidate_commitment_v1(
            &Sha,
            CandidateCommitmentOpeningV1 {
                epoch: epoch_id,
                market: runtime_id,
                relation_policy_id: binding.relation_policy_id,
                admission_policy_id: binding.admission_policy_id,
                score_policy_id: binding.score_policy_id,
                frozen_slot: 100,
                submitter_authority: submitter,
                solver_reward_destination: solver,
                candidate_bundle_digest: bundle,
                secret,
            },
        )
        .unwrap();
        let begun = begin_candidate_poststate_v1(BeginCandidateTransitionV1 {
            epoch_id,
            market_runtime_id: runtime_id,
            node_id,
            payer,
            submitter,
            refund_destination: id(34),
            solver_destination: solver,
            current_slot: 101,
            payload: BeginCandidatePayloadV1 {
                epoch: epoch_id,
                commitment,
            },
            node_rent: rent(payer, 150),
            node_bump: 6,
            epoch: &frozen.epoch,
            window: &frozen.window,
            binding: &binding,
        })
        .unwrap();
        assert_eq!(begun.node.market, runtime_id);
        assert_eq!(begun.window.admission_head, node_id);

        let opened = open_candidate_feed_poststate_v1(
            &Sha,
            OpenCandidateFeedTransitionV1 {
                epoch_id,
                feed_id,
                current_slot: 110,
                payload: OpenCandidateFeedPayloadV1 {
                    epoch: epoch_id,
                    node: node_id,
                    secret,
                    candidate_bundle_digest: bundle,
                    settlement_candidate_id: candidate_feed.settlement_candidate_id,
                    base_relation_candidate_id: candidate_feed.base_relation_candidate_id,
                    settlement_witness_digest: candidate_feed.settlement_witness_digest,
                    candidate_price_digest: candidate_feed.candidate_price_digest,
                    price_body_digest: body,
                    virtual_split: 0,
                    virtual_merge: 0,
                    honored_aon_mask: 0,
                    price_scale: 100,
                    common_denominator: 1,
                    basis_degree: 2,
                    outcome_count: 3,
                    order_count: 0,
                    atom_count: 1,
                    slice_count: 0,
                    candidate_kind: SettlementCandidateKindV1::Direct,
                },
                feed_rent: candidate_feed.rent,
                work_rent: rent(payer, 201),
                feed_bump: 7,
                epoch: &begun.epoch,
                window: &begun.window,
                node: &begun.node,
                binding: &binding,
                economic_domain: &initialized.economic_domain,
            },
        )
        .unwrap();
        assert_eq!(opened.node.market, runtime_id);
        assert_eq!(opened.feed_stage.market, runtime_id);
        assert_eq!(opened.window.revealed_count, 1);
        assert_eq!(opened.funding.reveal_payer_funding, 421);
        let price_records = [0u8; 16];
        let price_segment = CandidateFeedSegmentPayloadV1 {
            kind: CandidateFeedWriteKindV1::Prices,
            epoch: epoch_id,
            node: node_id,
            cursor: 0,
            count: 2,
            records: &price_records,
        };
        assert_eq!(
            candidate_feed_segment_byte_range_v1(price_segment, opened.feed_stage),
            Ok(CANDIDATE_FEED_HEADER_BYTES..CANDIDATE_FEED_HEADER_BYTES + 16)
        );
        let segment_stage = candidate_feed_segment_poststate_v1(
            price_segment,
            opened.feed_stage,
            opened.node,
            opened.window,
            110,
        )
        .unwrap();
        assert_eq!(segment_stage.prices_written, 2);
        assert_eq!(
            candidate_feed_segment_byte_range_v1(
                CandidateFeedSegmentPayloadV1 {
                    cursor: 1,
                    ..price_segment
                },
                opened.feed_stage,
            ),
            Err(CodecError::MismatchedBinding)
        );

        // The complete stage constructed independently must now seal under the
        // opened node and all four General-owned digest checks.
        assert_eq!(
            seal_empty_book_candidate_v1(
                &Sha,
                feed_id,
                &full_stage,
                opened.node,
                binding,
                initialized.economic_domain,
            ),
            Ok(candidate_feed)
        );
        let wrong_direct_id = id(99);
        let wrong_node = AdmissionNodeV3AccountV1 {
            settlement_candidate_id: wrong_direct_id,
            base_relation_candidate_id: wrong_direct_id,
            settlement_witness_digest: empty_settlement_witness_digest_v1(&Sha, wrong_direct_id)
                .unwrap(),
            ..opened.node
        };
        assert_eq!(wrong_node.validate(), Ok(()));
        assert_eq!(
            seal_empty_book_candidate_v1(
                &Sha,
                feed_id,
                &full_stage,
                wrong_node,
                binding,
                initialized.economic_domain,
            ),
            Err(CodecError::MismatchedBinding)
        );
        let work = init_clear_work_poststate_v1(InitClearWorkTransitionV1 {
            epoch_id,
            feed_id,
            work_id,
            work_rent: rent(payer, 201),
            work_bump: 8,
            epoch: &begun.epoch,
            node: &opened.node,
            feed: &candidate_feed,
            binding: &binding,
        })
        .unwrap();
        assert_eq!(work.epoch.work_count, 1);
        assert_eq!(work.node.work_escrow_lamports, 0);
        assert_eq!(work.work.market, runtime_id);

        let completed = complete_candidate_verification_poststate_v1(
            CompleteCandidateVerificationTransitionV1 {
                current_slot: 120,
                verdict: EmptyBookVerificationVerdictV1::Valid(ScoreV2QComponentsV1 {
                    certified_risk_flow_atoms: 3,
                    cash_equivalent_direct_flow_atoms: 2,
                    virtual_churn_atoms: 1,
                    settlement_candidate_id: work.node.settlement_candidate_id,
                }),
                epoch: &work.epoch,
                window: &opened.window,
                node: &work.node,
                work: &work.work,
                binding: &binding,
            },
        )
        .unwrap();
        assert_eq!(completed.keeper_reward, 7);
        assert_eq!(completed.work.reward_remaining, binding.work_close_reward);
        let refused = complete_candidate_verification_poststate_v1(
            CompleteCandidateVerificationTransitionV1 {
                verdict: EmptyBookVerificationVerdictV1::Refused,
                current_slot: 120,
                epoch: &work.epoch,
                window: &opened.window,
                node: &work.node,
                work: &work.work,
                binding: &binding,
            },
        )
        .unwrap();
        assert_eq!(refused.node.status, AdmissionNodeStatusV1::VerifiedRefused);
        assert_eq!(refused.window.verdict_count, 1);
        assert_eq!(refused.window.valid_verdict_count, 0);
        assert_eq!(
            complete_candidate_verification_poststate_v1(
                CompleteCandidateVerificationTransitionV1 {
                    current_slot: 140,
                    verdict: EmptyBookVerificationVerdictV1::Refused,
                    epoch: &work.epoch,
                    window: &opened.window,
                    node: &work.node,
                    work: &work.work,
                    binding: &binding,
                }
            ),
            Err(CodecError::MismatchedBinding)
        );
        let verified = completed.node;
        assert_eq!(
            verified.rank_key,
            encode_score_v2_q_first_admitted_tie_v1(
                ScoreV2QComponentsV1 {
                    certified_risk_flow_atoms: 3,
                    cash_equivalent_direct_flow_atoms: 2,
                    virtual_churn_atoms: 1,
                    settlement_candidate_id: verified.settlement_candidate_id,
                },
                FirstAdmittedTieV1 {
                    ordinal: verified.ordinal,
                },
            )
            .unwrap()
        );
        let selected_id = id(35);
        let finalized = finalize_selection_poststate_v1(FinalizeSelectionTransitionV1 {
            epoch_id,
            window_id,
            market_binding_id: binding_id,
            feed_id,
            selected_candidate_id: selected_id,
            current_slot: 120,
            selected_rent: rent(payer, 105),
            selected_bump: 9,
            epoch: &work.epoch,
            window: &completed.window,
            budget: &frozen.budget,
            node: &verified,
            feed: &candidate_feed,
        })
        .unwrap();
        assert_eq!(finalized.window.selected_candidate_artifact, selected_id);
        assert!(finalized.window.best_candidate_node.is_zero());
        assert_eq!(finalized.epoch.selected_candidate_count, 1);
        assert_eq!(finalized.budget.solver_remaining, binding.solver_prize);
        assert_eq!(finalized.selected_candidate.market, runtime_id);
        assert_eq!(finalized.selected_candidate.slice_count, 0);
        assert_eq!(finalized.selected_candidate.next_slice_index, 0);
        assert_eq!(finalized.selected_candidate.entitlement_state, 2);

        assert_eq!(
            begin_candidate_poststate_v1(BeginCandidateTransitionV1 {
                market_runtime_id: id(99),
                ..BeginCandidateTransitionV1 {
                    epoch_id,
                    market_runtime_id: runtime_id,
                    node_id: id(98),
                    payer,
                    submitter,
                    refund_destination: id(34),
                    solver_destination: solver,
                    current_slot: 102,
                    payload: BeginCandidatePayloadV1 {
                        epoch: epoch_id,
                        commitment,
                    },
                    node_rent: rent(payer, 150),
                    node_bump: 6,
                    epoch: &frozen.epoch,
                    window: &frozen.window,
                    binding: &binding,
                }
            }),
            Err(CodecError::MismatchedBinding)
        );
    }
}
