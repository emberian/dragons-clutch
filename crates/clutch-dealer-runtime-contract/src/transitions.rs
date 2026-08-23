// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    add, mul, DealerActionLivenessAuthorizationV1, DealerFacilityGenesisV1,
    DealerFacilityPositionPhaseV1, DealerFacilityPositionV1, DealerFundedBudgetDependenciesV1,
    DealerFundedDependenciesV2, DealerLivenessScheduleV1, DealerLpFundingFactsV1,
    DealerPhaseV1, DealerPhaseV2, DealerPolicyV1, DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1,
    DealerStateV1, DealerStateV2, Error, FacilityPositionBindingV1, Id, Result,
    SponsorCapitalDispositionV1, MAX_OUTCOMES,
};

/// Pure result of the atomic Funding-to-Trading semantic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActivationTransitionV1 {
    /// Canonical post-activation root. Facility Position assets are unchanged.
    pub state_after: DealerStateV1,
}

/// Validate full present funding and activate one exact facility.
///
/// This function selects no fee rates, liveness prices, call counts, or
/// compartment implementations. It consumes exact immutable dependency IDs,
/// adapter-observed initial balances, and a complete sealed LP page-set fold.
/// The future adapter remains responsible for authenticating account owners,
/// PDA addresses, token/Hoard custody, Clock, and atomic persistence.
#[allow(clippy::too_many_arguments)]
pub fn activate_dealer_v1(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    liveness_authorization: &DealerActionLivenessAuthorizationV1,
    dependencies: &DealerFundedBudgetDependenciesV1,
    lp_funding: DealerLpFundingFactsV1,
    current_slot: u64,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV1,
) -> Result<DealerActivationTransitionV1> {
    dependencies.validate_for_activation(genesis, binding, policy, schedule, runtime_liveness)?;
    liveness_authorization.validate_against(schedule, runtime_liveness)?;
    lp_funding.validate_against_state(state)?;
    state.validate_against_policy(policy)?;
    position.validate_live_against(binding, policy)?;
    validate_facility_root_join(genesis, binding, policy, position, state)?;
    if state.phase != DealerPhaseV1::Funding
        || state.sponsor_capital_disposition != SponsorCapitalDispositionV1::Refundable
        || liveness_authorization.action != DealerRuntimeActionV1::Activate
        || liveness_authorization.owner != binding.dealer_state_account_id
        || liveness_authorization.lifecycle_id != state.facility_id
        || liveness_authorization.facility_generation != state.generation
        || state.generation != binding.initial_position_generation
        || state.generation != dependencies.counted_generation
        || state.total_shares < policy.minimum_lp_shares
        || state.total_shares > policy.maximum_lp_shares
        || state.children.fee_budgets != 0
        || state.children.liveness_budgets != 0
        || current_slot < policy.trading_open_slot
        || current_slot >= policy.trading_close_slot
        || position.phase != DealerFacilityPositionPhaseV1::Idle
        || position.generation != state.generation
    {
        return Err(Error::InvalidPhase);
    }

    let expected_cash = add(
        state.sponsor_capital_atoms,
        mul(policy.capital_unit_cash_atoms, state.total_shares)?,
    )?;
    let mut expected_eggs = [0u64; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < usize::from(policy.outcome_count) {
        expected_eggs[index] = mul(policy.capital_unit_eggs[index], state.total_shares)?;
        index += 1;
    }
    if position.cash_atoms != expected_cash || position.eggs != expected_eggs {
        return Err(Error::ConservationFailure);
    }

    let mut state_after = *state;
    state_after.phase = DealerPhaseV1::Trading;
    state_after.sponsor_capital_disposition = SponsorCapitalDispositionV1::Donated;
    state_after.validate_against_policy(policy)?;
    Ok(DealerActivationTransitionV1 { state_after })
}

/// Pure result of one Trading-to-UnwindOnly semantic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerUnwindTransitionV1 {
    /// Canonical post-transition root. No asset balance changes.
    pub state_after: DealerStateV1,
}

/// Enter UnwindOnly under an authenticated sponsor halt.
#[allow(clippy::too_many_arguments)]
pub fn sponsor_halt_dealer_v1(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependencies: &DealerFundedBudgetDependenciesV1,
    authenticated_sponsor: Id,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV1,
) -> Result<DealerUnwindTransitionV1> {
    if authenticated_sponsor != state.sponsor {
        return Err(Error::MismatchedBinding);
    }
    enter_unwind_common(
        genesis,
        binding,
        policy,
        schedule,
        runtime_liveness,
        dependencies,
        position,
        state,
    )
}

/// Enter UnwindOnly permissionlessly after the exact queued-share quorum.
#[allow(clippy::too_many_arguments)]
pub fn enter_unwind_by_queue_v1(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependencies: &DealerFundedBudgetDependenciesV1,
    liveness_authorization: &DealerActionLivenessAuthorizationV1,
    lp_funding: DealerLpFundingFactsV1,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV1,
) -> Result<DealerUnwindTransitionV1> {
    lp_funding.validate_against_state(state)?;
    liveness_authorization.validate_against(schedule, runtime_liveness)?;
    if liveness_authorization.action != DealerRuntimeActionV1::EnterUnwind {
        return Err(Error::MismatchedBinding);
    }
    if liveness_authorization.facility_generation != state.generation {
        return Err(Error::MismatchedBinding);
    }
    if !policy.shutdown_queue_threshold_met(state.queued_shares, state.total_shares)? {
        return Err(Error::InvalidPhase);
    }
    enter_unwind_common(
        genesis,
        binding,
        policy,
        schedule,
        runtime_liveness,
        dependencies,
        position,
        state,
    )
}

/// Enter UnwindOnly permissionlessly at or after the immutable close slot.
#[allow(clippy::too_many_arguments)]
pub fn timed_close_dealer_v1(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependencies: &DealerFundedBudgetDependenciesV1,
    liveness_authorization: &DealerActionLivenessAuthorizationV1,
    current_slot: u64,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV1,
) -> Result<DealerUnwindTransitionV1> {
    if current_slot < policy.trading_close_slot {
        return Err(Error::InvalidSchedule);
    }
    liveness_authorization.validate_against(schedule, runtime_liveness)?;
    if liveness_authorization.action != DealerRuntimeActionV1::TimedClose {
        return Err(Error::MismatchedBinding);
    }
    if liveness_authorization.facility_generation != state.generation {
        return Err(Error::MismatchedBinding);
    }
    enter_unwind_common(
        genesis,
        binding,
        policy,
        schedule,
        runtime_liveness,
        dependencies,
        position,
        state,
    )
}

#[allow(clippy::too_many_arguments)]
fn enter_unwind_common(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependencies: &DealerFundedBudgetDependenciesV1,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV1,
) -> Result<DealerUnwindTransitionV1> {
    dependencies.validate_bindings(genesis, binding, policy, schedule, runtime_liveness)?;
    state.validate_policy_bindings(policy)?;
    position.validate_live_against(binding, policy)?;
    validate_facility_root_join(genesis, binding, policy, position, state)?;
    let expected_position_phase = if state.children.leases == 0 {
        DealerFacilityPositionPhaseV1::Idle
    } else {
        DealerFacilityPositionPhaseV1::Leased
    };
    if state.phase != DealerPhaseV1::Trading
        || state.sponsor_capital_disposition != SponsorCapitalDispositionV1::Donated
        || position.phase != expected_position_phase
        || position.generation != state.generation
    {
        return Err(Error::InvalidPhase);
    }
    let mut state_after = *state;
    state_after.phase = DealerPhaseV1::UnwindOnly;
    state_after.validate_against_policy(policy)?;
    Ok(DealerUnwindTransitionV1 { state_after })
}

fn validate_facility_root_join(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV1,
) -> Result<()> {
    let facility_id = genesis.facility_id_for_policy(policy)?;
    binding.binding_id_for(genesis, policy)?;
    if state.policy_id != genesis.policy_id
        || state.facility_id != facility_id.untyped()
        || state.facility_id != binding.facility_id
        || state.facility_position_id != position.position_id()?
        || state.facility_position_account_id != binding.facility_position_account_id
        || state.facility_replay_account_id != binding.facility_replay_account_id
        || state.sponsor != genesis.sponsor
        || state.sponsor_refund_recipient != genesis.sponsor_refund_recipient
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

/// Pure result of an authoritative V2 Funding-to-Trading transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActivationTransitionV2 {
    /// Post-activation V2 root; no asset balance is mirrored here.
    pub state_after: DealerStateV2,
}

/// Activate a V2 facility only while its one counted funded dependency is live.
#[allow(clippy::too_many_arguments)]
pub fn activate_dealer_v2(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    dependency: &DealerFundedDependenciesV2,
    dependency_account_id: Id,
    lp_funding: DealerLpFundingFactsV1,
    current_slot: u64,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV2,
) -> Result<DealerActivationTransitionV2> {
    dependency.validate_bindings(genesis, binding, policy, schedule, runtime_liveness)?;
    authorization.validate_against(schedule, runtime_liveness)?;
    lp_funding.validate_against_state_v2(state)?;
    state.validate_against_policy(policy)?;
    position.validate_live_against(binding, policy)?;
    validate_facility_root_join_v2(genesis, binding, policy, position, state)?;
    if state.phase != DealerPhaseV2::Funding
        || state.sponsor_capital_disposition != SponsorCapitalDispositionV1::Refundable
        || state.children.funded_dependencies != 1
        || state.funded_dependencies_id != dependency.dependency_id()?
        || state.funded_dependencies_account_id != dependency_account_id
        || authorization.action != DealerRuntimeActionV1::Activate
        || authorization.owner != binding.dealer_state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
        || state.generation != binding.initial_position_generation
        || state.generation != dependency.bindings.counted_generation
        || state.total_shares < policy.minimum_lp_shares
        || state.total_shares > policy.maximum_lp_shares
        || current_slot < policy.trading_open_slot
        || current_slot >= policy.trading_close_slot
        || position.phase != DealerFacilityPositionPhaseV1::Idle
        || position.generation != state.generation
    {
        return Err(Error::InvalidPhase);
    }
    let expected_cash = add(
        state.sponsor_capital_atoms,
        mul(policy.capital_unit_cash_atoms, state.total_shares)?,
    )?;
    let mut expected_eggs = [0; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < usize::from(policy.outcome_count) {
        expected_eggs[index] = mul(policy.capital_unit_eggs[index], state.total_shares)?;
        index += 1;
    }
    if position.cash_atoms != expected_cash || position.eggs != expected_eggs {
        return Err(Error::ConservationFailure);
    }
    let mut state_after = *state;
    state_after.phase = DealerPhaseV2::Trading;
    state_after.sponsor_capital_disposition = SponsorCapitalDispositionV1::Donated;
    state_after.validate_against_policy(policy)?;
    Ok(DealerActivationTransitionV2 { state_after })
}

/// Pure result of one authoritative V2 Trading-to-UnwindOnly transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerUnwindTransitionV2 {
    /// Canonical post-transition root.
    pub state_after: DealerStateV2,
}

/// Enter V2 UnwindOnly under an authenticated sponsor halt.
#[allow(clippy::too_many_arguments)]
pub fn sponsor_halt_dealer_v2(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependency: &DealerFundedDependenciesV2,
    authenticated_sponsor: Id,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV2,
) -> Result<DealerUnwindTransitionV2> {
    if authenticated_sponsor != state.sponsor {
        return Err(Error::MismatchedBinding);
    }
    enter_unwind_common_v2(
        genesis,
        binding,
        policy,
        schedule,
        runtime_liveness,
        dependency,
        position,
        state,
    )
}

/// Enter V2 UnwindOnly after exact queued-share quorum.
#[allow(clippy::too_many_arguments)]
pub fn enter_unwind_by_queue_v2(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependency: &DealerFundedDependenciesV2,
    authorization: &DealerActionLivenessAuthorizationV1,
    lp_funding: DealerLpFundingFactsV1,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV2,
) -> Result<DealerUnwindTransitionV2> {
    lp_funding.validate_against_state_v2(state)?;
    authorization.validate_against(schedule, runtime_liveness)?;
    if authorization.action != DealerRuntimeActionV1::EnterUnwind
        || authorization.facility_generation != state.generation
        || !policy.shutdown_queue_threshold_met(state.queued_shares, state.total_shares)?
    {
        return Err(Error::InvalidPhase);
    }
    enter_unwind_common_v2(
        genesis,
        binding,
        policy,
        schedule,
        runtime_liveness,
        dependency,
        position,
        state,
    )
}

/// Enter V2 UnwindOnly at or after the immutable close slot.
#[allow(clippy::too_many_arguments)]
pub fn timed_close_dealer_v2(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependency: &DealerFundedDependenciesV2,
    authorization: &DealerActionLivenessAuthorizationV1,
    current_slot: u64,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV2,
) -> Result<DealerUnwindTransitionV2> {
    authorization.validate_against(schedule, runtime_liveness)?;
    if current_slot < policy.trading_close_slot
        || authorization.action != DealerRuntimeActionV1::TimedClose
        || authorization.facility_generation != state.generation
    {
        return Err(Error::InvalidSchedule);
    }
    enter_unwind_common_v2(
        genesis,
        binding,
        policy,
        schedule,
        runtime_liveness,
        dependency,
        position,
        state,
    )
}

#[allow(clippy::too_many_arguments)]
fn enter_unwind_common_v2(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime_liveness: &DealerRuntimeLivenessBindingV1,
    dependency: &DealerFundedDependenciesV2,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV2,
) -> Result<DealerUnwindTransitionV2> {
    dependency.validate_bindings(genesis, binding, policy, schedule, runtime_liveness)?;
    state.validate_policy_bindings(policy)?;
    position.validate_live_against(binding, policy)?;
    validate_facility_root_join_v2(genesis, binding, policy, position, state)?;
    let position_phase = if state.children.leases == 0 {
        DealerFacilityPositionPhaseV1::Idle
    } else {
        DealerFacilityPositionPhaseV1::Leased
    };
    if state.phase != DealerPhaseV2::Trading
        || state.sponsor_capital_disposition != SponsorCapitalDispositionV1::Donated
        || state.children.funded_dependencies != 1
        || state.funded_dependencies_id != dependency.dependency_id()?
        || position.phase != position_phase
        || position.generation != state.generation
    {
        return Err(Error::InvalidPhase);
    }
    let mut state_after = *state;
    state_after.phase = DealerPhaseV2::UnwindOnly;
    state_after.validate_against_policy(policy)?;
    Ok(DealerUnwindTransitionV2 { state_after })
}

fn validate_facility_root_join_v2(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV2,
) -> Result<()> {
    let facility_id = genesis.facility_id_for_policy(policy)?;
    let binding_id = binding.binding_id_for(genesis, policy)?;
    if state.policy_id != genesis.policy_id
        || state.facility_id != facility_id.untyped()
        || state.facility_id != binding.facility_id
        || state.facility_position_binding_id != binding_id.untyped()
        || state.facility_position_id != position.position_id()?
        || state.facility_position_account_id != binding.facility_position_account_id
        || state.facility_replay_account_id != binding.facility_replay_account_id
        || state.sponsor != genesis.sponsor
        || state.sponsor_refund_recipient != genesis.sponsor_refund_recipient
        || dependency_authority_mismatch(state, binding)
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

fn dependency_authority_mismatch(
    state: &DealerStateV2,
    binding: &FacilityPositionBindingV1,
) -> bool {
    state.funded_dependencies_account_id == binding.dealer_state_account_id
        || state.funded_dependencies_account_id == binding.facility_position_account_id
        || state.funded_dependencies_account_id == binding.facility_replay_account_id
}

/// Complete the root lifecycle only after every V2 child, including funded dependencies, closed.
pub fn close_dealer_state_v2(
    state: &DealerStateV2,
    policy: &DealerPolicyV1,
) -> Result<DealerStateV2> {
    state.validate_against_policy(policy)?;
    if state.phase != DealerPhaseV2::Retiring
        || state.children != crate::DealerChildCountsV2::default()
        || !state.funded_dependencies_account_id.is_zero()
    {
        return Err(Error::InvalidChildGraph);
    }
    let mut state_after = *state;
    state_after.phase = DealerPhaseV2::Closed;
    state_after.validate_against_policy(policy)?;
    Ok(state_after)
}
