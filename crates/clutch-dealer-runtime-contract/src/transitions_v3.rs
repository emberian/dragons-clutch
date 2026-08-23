// SPDX-License-Identifier: AGPL-3.0-or-later

//! PositionV3-native activation and UnwindOnly transitions.

use crate::{
    lp_funding_v2::prepare_funding_replay_v2, DealerActionLivenessAuthorizationV1,
    DealerEmptyAssetTransferBundleV1, DealerFacilityReplayV1,
    DealerFundedDependenciesV2, DealerLivenessScheduleV1, DealerPhaseV2,
    DealerPolicyV1, DealerPositionObservationV3, DealerReplayAccountBindingV1,
    DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1, DealerStateV2,
    DealerTransitionLivenessModeV1, Error, FacilityPositionBindingV2, FixedCodec, Id,
    LpPageV2, PreparedDealerReplayTransitionV1, Result,
};

/// Atomic Funding-to-Trading result over PositionV3 and the last mutable page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerActivationV3 {
    /// Last page after its ownership/share facts become immutable.
    pub tail_page_after: LpPageV2,
    /// Authoritative State after activation.
    pub state_after: DealerStateV2,
    /// Replay advance binding the page/State writes and funded receipt.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Atomic transition into UnwindOnly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerUnwindV3 {
    /// Authoritative State after the transition.
    pub state_after: DealerStateV2,
    /// Replay advance binding the exact cause and State write.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Activate only after the exact PositionV3 balance equals sponsor plus all LP units.
#[allow(clippy::too_many_arguments)]
pub fn prepare_activate_dealer_v3(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    current_slot: u64,
    tail_page: &LpPageV2,
    position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerActivationV3> {
    validate_v3_plane(
        policy,
        binding,
        state,
        state_account_id,
        dependency,
        schedule,
        runtime,
        position,
        replay,
    )?;
    authorization.validate_against(schedule, runtime)?;
    tail_page.validate_against(policy, state, state_account_id)?;
    let (live, shares) = tail_page.aggregate_totals()?;
    if state.phase != DealerPhaseV2::Funding
        || state.total_shares < policy.minimum_lp_shares
        || state.total_shares > policy.maximum_lp_shares
        || current_slot < policy.trading_open_slot
        || current_slot >= policy.trading_close_slot
        || authorization.action != DealerRuntimeActionV1::Activate
        || authorization.owner != state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
        || tail_page.page_content_id()? != state.lp_page_set_root
        || tail_page.next_page_ordinal != crate::NO_NEXT_LP_PAGE
        || tail_page.sealed
        || tail_page.entry_count == 0
        || live != state.children.live_lp_positions
        || shares != state.total_shares
    {
        return Err(Error::InvalidPhase);
    }
    let canonical = position.projection.position();
    let expected_cash = state
        .sponsor_capital_atoms
        .checked_add(
            policy
                .capital_unit_cash_atoms
                .checked_mul(state.total_shares)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)?;
    let mut expected_eggs = [0u64; crate::MAX_OUTCOMES];
    let mut index = 0usize;
    while index < usize::from(policy.outcome_count) {
        expected_eggs[index] = policy.capital_unit_eggs[index]
            .checked_mul(state.total_shares)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if canonical.cash_atoms() != expected_cash
        || canonical.reserved_cash_atoms() != 0
        || canonical.native_eggs() != expected_eggs
    {
        return Err(Error::ConservationFailure);
    }
    let mut tail_page_after = *tail_page;
    tail_page_after.sealed = true;
    tail_page_after.revision = tail_page_after
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    tail_page_after.validate_against(policy, state, state_account_id)?;
    let mut state_after = *state;
    state_after.lp_page_set_root = tail_page_after.page_content_id()?;
    state_after.phase = DealerPhaseV2::Trading;
    state_after.sponsor_capital_disposition = crate::SponsorCapitalDispositionV1::Donated;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.validate_against_policy(policy)?;
    let replay = prepare_funding_replay_v2(
        state,
        &state_after,
        replay,
        replay_binding,
        DealerRuntimeActionV1::Activate,
        authorization.receipt_semantic_id,
        DealerTransitionLivenessModeV1::ExternalReceipt,
        DealerEmptyAssetTransferBundleV1 {
            action: DealerRuntimeActionV1::Activate,
        }
        .bundle_id()?,
        state.facility_position_id,
        state.facility_position_id,
    )?;
    Ok(PreparedDealerActivationV3 {
        tail_page_after,
        state_after,
        replay,
    })
}

/// Enter UnwindOnly under the exact authenticated sponsor signature.
#[allow(clippy::too_many_arguments)]
pub fn prepare_sponsor_halt_dealer_v3(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authenticated_sponsor: Id,
    position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerUnwindV3> {
    validate_v3_plane(
        policy,
        binding,
        state,
        state_account_id,
        dependency,
        schedule,
        runtime,
        position,
        replay,
    )?;
    if authenticated_sponsor != state.sponsor {
        return Err(Error::MismatchedBinding);
    }
    prepare_unwind(
        policy,
        state,
        replay,
        replay_binding,
        DealerRuntimeActionV1::SponsorHalt,
        Id::ZERO,
        DealerTransitionLivenessModeV1::CallerFunded,
    )
}

/// Enter UnwindOnly at or after the immutable trading close slot.
#[allow(clippy::too_many_arguments)]
pub fn prepare_timed_close_dealer_v3(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    current_slot: u64,
    position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerUnwindV3> {
    validate_v3_plane(
        policy,
        binding,
        state,
        state_account_id,
        dependency,
        schedule,
        runtime,
        position,
        replay,
    )?;
    authorization.validate_against(schedule, runtime)?;
    if current_slot < policy.trading_close_slot
        || authorization.action != DealerRuntimeActionV1::TimedClose
        || authorization.owner != state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
    {
        return Err(Error::InvalidSchedule);
    }
    prepare_unwind(
        policy,
        state,
        replay,
        replay_binding,
        DealerRuntimeActionV1::TimedClose,
        authorization.receipt_semantic_id,
        DealerTransitionLivenessModeV1::ExternalReceipt,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_v3_plane(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
) -> Result<()> {
    state.validate_against_policy(policy)?;
    dependency.validate()?;
    schedule.validate_for_facility_runtime()?;
    runtime.validate()?;
    let binding_id = binding.binding_id()?;
    position.validate_current(state, binding, policy)?;
    replay.validate()?;
    if binding.policy_id != state.policy_id
        || binding.facility_id != state.facility_id
        || binding.dealer_state_account_id != state_account_id
        || binding_id != state.facility_position_binding_id
        || state.funded_dependencies_id != dependency.dependency_id()?
        || dependency.facility_position_binding_id != binding_id
        || dependency.bindings.policy_id != state.policy_id
        || dependency.bindings.facility_id != state.facility_id
        || dependency.bindings.asset_vault_authority_account_id != state_account_id
        || dependency.bindings.liveness_schedule_id != schedule.schedule_id()?.untyped()
        || dependency.bindings.runtime_liveness_policy_id != runtime.runtime_policy_id
        || dependency.bindings.runtime_liveness_binding_digest != runtime.binding_digest()?
        || replay.facility_position_account_id() != state.facility_position_account_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_binding_id() != binding_id
        || replay.position_generation() != state.generation
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

fn prepare_unwind(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    action: DealerRuntimeActionV1,
    liveness_receipt_semantic_id: Id,
    liveness_mode: DealerTransitionLivenessModeV1,
) -> Result<PreparedDealerUnwindV3> {
    if state.phase != DealerPhaseV2::Trading {
        return Err(Error::InvalidPhase);
    }
    let mut state_after = *state;
    state_after.phase = DealerPhaseV2::UnwindOnly;
    state_after.validate_against_policy(policy)?;
    let replay = prepare_funding_replay_v2(
        state,
        &state_after,
        replay,
        replay_binding,
        action,
        liveness_receipt_semantic_id,
        liveness_mode,
        DealerEmptyAssetTransferBundleV1 { action }.bundle_id()?,
        state.facility_position_id,
        state.facility_position_id,
    )?;
    Ok(PreparedDealerUnwindV3 {
        state_after,
        replay,
    })
}
