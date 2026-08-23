// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bounded V2 LP funding mutations over canonical Position V3 custody.
//!
//! Each page commits the exact aggregate and owner-order prefix before it. Only
//! the unsealed tail may mutate, so one instruction never rewrites an
//! unbounded page chain. Sealing a full tail and appending its successor makes
//! that prefix permanent. Queue intent is deliberately absent: mutable exit
//! intent belongs to the owner-scoped ticket successor, not immutable LP
//! ownership bytes.

use sha2::{Digest, Sha256};

use crate::{
    DealerActionLivenessAuthorizationV1, DealerAssetEndpointKindV1,
    DealerEmptyAssetTransferBundleV1, DealerFacilityReplayV1,
    DealerFundedDependenciesV2,
    DealerLivenessScheduleV1, DealerPhaseV2, DealerPolicyV1,
    DealerReplayAccountBindingV1, DealerRuntimeActionV1,
    DealerRuntimeLivenessBindingV1, DealerStateV2, DealerTransitionIntentV1,
    DealerTransitionLivenessModeV1, DeletableRentOwnerV1, Error, FixedCodec, Id,
    LpEntryV2, LpPageV2, PreparedDealerPositionPairTransferV1,
    PreparedDealerReplayTransitionV1, Result, DEALER_LP_PAGE_SET_INIT_DOMAIN_V2,
    LP_ENTRIES_PER_PAGE, NO_NEXT_LP_PAGE,
};

/// Atomic first-page creation result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerFirstLpPageV2 {
    /// New empty page.
    pub page: LpPageV2,
    /// Authoritative State after counting the page.
    pub state_after: DealerStateV2,
    /// Exact Replay advance binding the State mutation and liveness receipt.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Atomic full-tail seal and successor creation result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerNextLpPageV2 {
    /// Former tail after permanent sealing and next-ordinal commitment.
    pub previous_page_after: LpPageV2,
    /// New empty mutable tail.
    pub page: LpPageV2,
    /// Authoritative State after counting the page.
    pub state_after: DealerStateV2,
    /// Exact Replay advance binding both page writes and liveness receipt.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Atomic LP contribution result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerLpContributionV2 {
    /// Tail page after inserting or increasing the authenticated owner.
    pub page_after: LpPageV2,
    /// Authoritative State after the exact share and Position update.
    pub state_after: DealerStateV2,
    /// Position-to-Position transfer prepared by the canonical Position V3 owner.
    pub transfer: PreparedDealerPositionPairTransferV1,
    /// Exact caller-funded Replay advance.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Atomic pre-activation LP withdrawal result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerLpWithdrawalV2 {
    /// Tail page after decreasing or removing the authenticated owner.
    pub page_after: LpPageV2,
    /// Authoritative State after the exact share and Position update.
    pub state_after: DealerStateV2,
    /// Position-to-Position transfer prepared by the canonical Position V3 owner.
    pub transfer: PreparedDealerPositionPairTransferV1,
    /// Exact caller-funded Replay advance.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Create the first empty LP page under one exact funded liveness receipt.
#[allow(clippy::too_many_arguments)]
pub fn prepare_first_lp_page_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page_account_id: Id,
    page_rent: DeletableRentOwnerV1,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerFirstLpPageV2> {
    validate_create_context(
        policy,
        state,
        state_account_id,
        page_account_id,
        dependency,
        schedule,
        runtime,
        authorization,
        replay,
    )?;
    if state.children.lp_pages != 0
        || !state.lp_page_head_id.is_zero()
        || !state.lp_page_set_root.is_zero()
        || !state.last_lp_owner.is_zero()
    {
        return Err(Error::InvalidChildGraph);
    }
    let page = LpPageV2 {
        policy_id: policy.policy_id()?,
        facility_id: state.facility_id,
        facility_position_binding_id: state.facility_position_binding_id,
        dealer_state_account_id: state_account_id,
        page_set_prefix_root: initial_page_prefix(policy, state, state_account_id)?,
        prefix_last_owner: Id::ZERO,
        prefix_live_positions: 0,
        prefix_total_shares: 0,
        prefix_reserved: 0,
        counted_generation: state.generation,
        page_ordinal: 0,
        next_page_ordinal: NO_NEXT_LP_PAGE,
        entry_count: 0,
        sealed: false,
        revision: 0,
        entries: [LpEntryV2::EMPTY; LP_ENTRIES_PER_PAGE],
        rent: page_rent,
    };
    page.validate_against(policy, state, state_account_id)?;
    let mut state_after = *state;
    state_after.lp_page_head_id = page_account_id;
    state_after.lp_page_set_root = page.page_content_id()?;
    state_after.children.lp_pages = 1;
    state_after.child_sequence = next(state.child_sequence)?;
    state_after.validate_against_policy(policy)?;
    let replay = prepare_funding_replay_v2(
        state,
        &state_after,
        replay,
        replay_binding,
        DealerRuntimeActionV1::CreateLpPage,
        authorization.receipt_semantic_id,
        DealerTransitionLivenessModeV1::ExternalReceipt,
        DealerEmptyAssetTransferBundleV1 {
            action: DealerRuntimeActionV1::CreateLpPage,
        }
        .bundle_id()?,
        state.facility_position_id,
        state.facility_position_id,
    )?;
    Ok(PreparedDealerFirstLpPageV2 {
        page,
        state_after,
        replay,
    })
}

/// Seal a full tail and create its empty successor under one liveness receipt.
#[allow(clippy::too_many_arguments)]
pub fn prepare_next_lp_page_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    previous_page_account_id: Id,
    previous_page: &LpPageV2,
    page_account_id: Id,
    page_rent: DeletableRentOwnerV1,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerNextLpPageV2> {
    validate_create_context(
        policy,
        state,
        state_account_id,
        page_account_id,
        dependency,
        schedule,
        runtime,
        authorization,
        replay,
    )?;
    validate_tail(policy, state, state_account_id, previous_page)?;
    previous_page_account_id.validate_live()?;
    if previous_page_account_id == page_account_id
        || previous_page.entry_count as usize != LP_ENTRIES_PER_PAGE
        || previous_page.sealed
        || state.children.lp_pages >= policy.maximum_lp_pages
    {
        return Err(Error::InvalidLpPage);
    }
    let mut previous_page_after = *previous_page;
    previous_page_after.sealed = true;
    previous_page_after.next_page_ordinal = previous_page
        .page_ordinal
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    previous_page_after.revision = next(previous_page.revision)?;
    previous_page_after.validate_against(policy, state, state_account_id)?;
    let prefix_root = previous_page_after.page_content_id()?;
    let page = LpPageV2 {
        policy_id: previous_page.policy_id,
        facility_id: previous_page.facility_id,
        facility_position_binding_id: previous_page.facility_position_binding_id,
        dealer_state_account_id: previous_page.dealer_state_account_id,
        page_set_prefix_root: prefix_root,
        prefix_last_owner: state.last_lp_owner,
        prefix_live_positions: state.children.live_lp_positions,
        prefix_total_shares: state.total_shares,
        prefix_reserved: 0,
        counted_generation: state.generation,
        page_ordinal: previous_page_after.next_page_ordinal,
        next_page_ordinal: NO_NEXT_LP_PAGE,
        entry_count: 0,
        sealed: false,
        revision: 0,
        entries: [LpEntryV2::EMPTY; LP_ENTRIES_PER_PAGE],
        rent: page_rent,
    };
    page.validate_against(policy, state, state_account_id)?;
    let mut state_after = *state;
    state_after.lp_page_set_root = page.page_content_id()?;
    state_after.children.lp_pages = state_after
        .children
        .lp_pages
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.child_sequence = next(state.child_sequence)?;
    state_after.validate_against_policy(policy)?;
    let replay = prepare_funding_replay_v2(
        state,
        &state_after,
        replay,
        replay_binding,
        DealerRuntimeActionV1::CreateLpPage,
        authorization.receipt_semantic_id,
        DealerTransitionLivenessModeV1::ExternalReceipt,
        DealerEmptyAssetTransferBundleV1 {
            action: DealerRuntimeActionV1::CreateLpPage,
        }
        .bundle_id()?,
        state.facility_position_id,
        state.facility_position_id,
    )?;
    Ok(PreparedDealerNextLpPageV2 {
        previous_page_after,
        page,
        state_after,
        replay,
    })
}

/// Apply one exact LP contribution to the mutable tail.
#[allow(clippy::too_many_arguments)]
pub fn prepare_lp_contribution_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page: &LpPageV2,
    lp_owner: Id,
    share_delta: u64,
    transfer: PreparedDealerPositionPairTransferV1,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerLpContributionV2> {
    validate_tail(policy, state, state_account_id, page)?;
    lp_owner.validate_live()?;
    if state.phase != DealerPhaseV2::Funding || share_delta == 0 || page.sealed {
        return Err(Error::InvalidPhase);
    }
    let bundle = transfer.bundle();
    bundle.validate()?;
    if bundle.action != DealerRuntimeActionV1::Contribute
        || bundle.source_kind != DealerAssetEndpointKindV1::GeneralPosition
        || bundle.destination_kind != DealerAssetEndpointKindV1::FacilityPosition
        || bundle.destination_account_id != state.facility_position_account_id
        || bundle.destination_pre_semantic_id != state.facility_position_id
        || bundle.amounts.cash_atoms
            != policy
                .capital_unit_cash_atoms
                .checked_mul(share_delta)
                .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::MismatchedBinding);
    }
    let mut page_after = *page;
    let (slot, existing) = owner_slot(page, lp_owner)?;
    if existing {
        page_after.entries[slot].shares = page_after.entries[slot]
            .shares
            .checked_add(share_delta)
            .ok_or(Error::ArithmeticOverflow)?;
    } else {
        if usize::from(page.entry_count) == LP_ENTRIES_PER_PAGE {
            return Err(Error::InvalidLpPage);
        }
        let mut cursor = usize::from(page.entry_count);
        while cursor > slot {
            page_after.entries[cursor] = page_after.entries[cursor - 1];
            cursor -= 1;
        }
        page_after.entries[slot] = LpEntryV2 {
            owner: lp_owner,
            shares: share_delta,
        };
        page_after.entry_count = page_after
            .entry_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    page_after.revision = next(page.revision)?;
    page_after.validate()?;
    let mut state_after = *state;
    state_after.facility_position_id = bundle.destination_post_semantic_id;
    state_after.total_shares = state_after
        .total_shares
        .checked_add(share_delta)
        .ok_or(Error::ArithmeticOverflow)?;
    if !existing {
        state_after.children.live_lp_positions = state_after
            .children
            .live_lp_positions
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if state_after.total_shares > policy.maximum_lp_shares {
        return Err(Error::InvalidParameter);
    }
    state_after.last_lp_owner = tail_last_owner(&page_after);
    state_after.lp_page_set_root = page_after.page_content_id()?;
    state_after.child_sequence = next(state.child_sequence)?;
    require_tail_totals(&page_after, &state_after)?;
    state_after.validate_against_policy(policy)?;
    let replay = prepare_funding_replay_v2(
        state,
        &state_after,
        replay,
        replay_binding,
        DealerRuntimeActionV1::Contribute,
        Id::ZERO,
        DealerTransitionLivenessModeV1::CallerFunded,
        bundle.bundle_id()?,
        bundle.destination_pre_semantic_id,
        bundle.destination_post_semantic_id,
    )?;
    Ok(PreparedDealerLpContributionV2 {
        page_after,
        state_after,
        transfer,
        replay,
    })
}

/// Apply one exact LP withdrawal from the mutable tail before activation.
#[allow(clippy::too_many_arguments)]
pub fn prepare_lp_withdrawal_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page: &LpPageV2,
    lp_owner: Id,
    share_delta: u64,
    transfer: PreparedDealerPositionPairTransferV1,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerLpWithdrawalV2> {
    validate_tail(policy, state, state_account_id, page)?;
    lp_owner.validate_live()?;
    if !matches!(state.phase, DealerPhaseV2::Funding | DealerPhaseV2::Cancelled)
        || share_delta == 0
        || page.sealed
    {
        return Err(Error::InvalidPhase);
    }
    let bundle = transfer.bundle();
    bundle.validate()?;
    if bundle.action != DealerRuntimeActionV1::WithdrawFunding
        || bundle.source_kind != DealerAssetEndpointKindV1::FacilityPosition
        || bundle.destination_kind != DealerAssetEndpointKindV1::GeneralPosition
        || bundle.source_account_id != state.facility_position_account_id
        || bundle.source_pre_semantic_id != state.facility_position_id
        || bundle.amounts.cash_atoms
            != policy
                .capital_unit_cash_atoms
                .checked_mul(share_delta)
                .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::MismatchedBinding);
    }
    let (slot, existing) = owner_slot(page, lp_owner)?;
    if !existing || share_delta > page.entries[slot].shares {
        return Err(Error::InvalidParameter);
    }
    let mut page_after = *page;
    let removed = share_delta == page.entries[slot].shares;
    if removed {
        let count = usize::from(page.entry_count);
        let mut cursor = slot;
        while cursor + 1 < count {
            page_after.entries[cursor] = page_after.entries[cursor + 1];
            cursor += 1;
        }
        page_after.entries[count - 1] = LpEntryV2::EMPTY;
        page_after.entry_count -= 1;
    } else {
        page_after.entries[slot].shares -= share_delta;
    }
    page_after.revision = next(page.revision)?;
    page_after.validate()?;
    let mut state_after = *state;
    state_after.facility_position_id = bundle.source_post_semantic_id;
    state_after.total_shares = state_after
        .total_shares
        .checked_sub(share_delta)
        .ok_or(Error::ArithmeticOverflow)?;
    if removed {
        state_after.children.live_lp_positions = state_after
            .children
            .live_lp_positions
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    state_after.last_lp_owner = tail_last_owner(&page_after);
    state_after.lp_page_set_root = page_after.page_content_id()?;
    state_after.child_sequence = next(state.child_sequence)?;
    require_tail_totals(&page_after, &state_after)?;
    state_after.validate_against_policy(policy)?;
    let replay = prepare_funding_replay_v2(
        state,
        &state_after,
        replay,
        replay_binding,
        DealerRuntimeActionV1::WithdrawFunding,
        Id::ZERO,
        DealerTransitionLivenessModeV1::CallerFunded,
        bundle.bundle_id()?,
        bundle.source_pre_semantic_id,
        bundle.source_post_semantic_id,
    )?;
    Ok(PreparedDealerLpWithdrawalV2 {
        page_after,
        state_after,
        transfer,
        replay,
    })
}

fn validate_create_context(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    replay: &DealerFacilityReplayV1,
) -> Result<()> {
    state.validate_against_policy(policy)?;
    dependency.validate()?;
    authorization.validate_against(schedule, runtime)?;
    state_account_id.validate_live()?;
    page_account_id.validate_live()?;
    replay.validate()?;
    if state.phase != DealerPhaseV2::Funding
        || authorization.action != DealerRuntimeActionV1::CreateLpPage
        || authorization.owner != state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
        || state.funded_dependencies_id != dependency.dependency_id()?
        || state.facility_position_binding_id != dependency.facility_position_binding_id
        || dependency.bindings.policy_id != state.policy_id
        || dependency.bindings.facility_id != state.facility_id
        || dependency.bindings.asset_vault_authority_account_id != state_account_id
        || dependency.bindings.liveness_schedule_id != schedule.schedule_id()?.untyped()
        || dependency.bindings.runtime_liveness_policy_id != runtime.runtime_policy_id
        || dependency.bindings.runtime_liveness_binding_digest != runtime.binding_digest()?
        || replay.facility_position_account_id() != state.facility_position_account_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_binding_id() != state.facility_position_binding_id
        || replay.position_generation() != state.generation
        || page_account_id == state_account_id
        || page_account_id == state.facility_position_account_id
        || page_account_id == state.facility_replay_account_id
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

fn validate_tail(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page: &LpPageV2,
) -> Result<()> {
    page.validate_against(policy, state, state_account_id)?;
    let (live, total) = page.aggregate_totals()?;
    if page.page_content_id()? != state.lp_page_set_root
        || page.next_page_ordinal != NO_NEXT_LP_PAGE
        || page.counted_generation != state.generation
        || live != state.children.live_lp_positions
        || total != state.total_shares
        || tail_last_owner(page) != state.last_lp_owner
    {
        return Err(Error::InvalidChildGraph);
    }
    Ok(())
}

fn require_tail_totals(page: &LpPageV2, state: &DealerStateV2) -> Result<()> {
    let (live, total) = page.aggregate_totals()?;
    if live != state.children.live_lp_positions || total != state.total_shares {
        return Err(Error::InvalidChildGraph);
    }
    Ok(())
}

fn owner_slot(page: &LpPageV2, owner: Id) -> Result<(usize, bool)> {
    if !page.prefix_last_owner.is_zero() && owner <= page.prefix_last_owner {
        return Err(Error::InvalidLpPage);
    }
    let count = usize::from(page.entry_count);
    let mut index = 0usize;
    while index < count {
        if page.entries[index].owner == owner {
            return Ok((index, true));
        }
        if page.entries[index].owner > owner {
            return Ok((index, false));
        }
        index += 1;
    }
    Ok((count, false))
}

fn tail_last_owner(page: &LpPageV2) -> Id {
    if page.entry_count == 0 {
        page.prefix_last_owner
    } else {
        page.entries[usize::from(page.entry_count) - 1].owner
    }
}

fn initial_page_prefix(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
) -> Result<Id> {
    let mut hasher = Sha256::new();
    hasher.update(DEALER_LP_PAGE_SET_INIT_DOMAIN_V2);
    for identity in [
        policy.policy_id()?,
        state.facility_id,
        state.facility_position_binding_id,
        state_account_id,
    ] {
        identity.validate_live()?;
        hasher.update(identity.bytes());
    }
    hasher.update(state.generation.to_le_bytes());
    let id = Id::from_bytes(hasher.finalize().into());
    id.validate_live()?;
    Ok(id)
}

/// Exact lamport observation for one empty funding-page close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEmptyLpPageCloseRentV2 {
    /// Closed page lamports before deletion.
    pub page_lamports_before: u64,
    /// Closed page lamports after deletion; exactly zero.
    pub page_lamports_after: u64,
    /// Sole refundable-principal recipient.
    pub payer: Id,
    /// Canonical donation/surplus recipient.
    pub neutral_sink: Id,
    /// Exact principal credit.
    pub payer_refund_lamports: u64,
    /// Exact donation-floor and later-surplus credit.
    pub neutral_sink_lamports: u64,
}

/// Permissionless reverse-tail close result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerEmptyLpPageCloseV2 {
    /// Previous page after becoming the mutable tail, or `None` for page zero.
    pub previous_page_after: Option<LpPageV2>,
    /// Authoritative State after the exact page-count decrement.
    pub state_after: DealerStateV2,
}

/// Close one empty tail in reverse order without stranding earlier LP refunds.
#[allow(clippy::too_many_arguments)]
pub fn close_empty_lp_tail_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    page_account_id: Id,
    page: &LpPageV2,
    previous_page_account_id: Id,
    previous_page: Option<&LpPageV2>,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    rent: DealerEmptyLpPageCloseRentV2,
) -> Result<PreparedDealerEmptyLpPageCloseV2> {
    validate_tail(policy, state, state_account_id, page)?;
    authorization.validate_against(schedule, runtime)?;
    page_account_id.validate_live()?;
    let protected = page
        .rent
        .refundable_principal
        .checked_add(page.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    if !matches!(state.phase, DealerPhaseV2::Funding | DealerPhaseV2::Cancelled)
        || page.entry_count != 0
        || page.sealed
        || page.page_ordinal.checked_add(1) != Some(state.children.lp_pages)
        || authorization.action != DealerRuntimeActionV1::Retire
        || authorization.owner != state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
        || rent.page_lamports_after != 0
        || rent.page_lamports_before < protected
        || rent.payer != page.rent.payer
        || rent.neutral_sink != page.rent.neutral_sink
        || rent.payer_refund_lamports != page.rent.refundable_principal
        || rent.neutral_sink_lamports
            != rent
                .page_lamports_before
                .checked_sub(page.rent.refundable_principal)
                .ok_or(Error::ConservationFailure)?
    {
        return Err(Error::MismatchedBinding);
    }
    let mut state_after = *state;
    state_after.children.lp_pages = state_after
        .children
        .lp_pages
        .checked_sub(1)
        .ok_or(Error::InvalidChildGraph)?;
    state_after.child_sequence = next(state.child_sequence)?;
    let previous_page_after = if page.page_ordinal == 0 {
        if previous_page.is_some()
            || !previous_page_account_id.is_zero()
            || page_account_id != state.lp_page_head_id
            || state.children.live_lp_positions != 0
            || state.total_shares != 0
        {
            return Err(Error::InvalidChildGraph);
        }
        state_after.lp_page_head_id = Id::ZERO;
        state_after.lp_page_set_root = Id::ZERO;
        state_after.last_lp_owner = Id::ZERO;
        None
    } else {
        let previous = previous_page.ok_or(Error::InvalidChildGraph)?;
        previous_page_account_id.validate_live()?;
        previous.validate_against(policy, state, state_account_id)?;
        let (previous_live, previous_total) = previous.aggregate_totals()?;
        if previous_page_account_id == page_account_id
            || previous.page_ordinal.checked_add(1) != Some(page.page_ordinal)
            || previous.next_page_ordinal != page.page_ordinal
            || !previous.sealed
            || usize::from(previous.entry_count) != LP_ENTRIES_PER_PAGE
            || previous.page_content_id()? != page.page_set_prefix_root
            || previous_live != page.prefix_live_positions
            || previous_total != page.prefix_total_shares
        {
            return Err(Error::InvalidChildGraph);
        }
        let mut after = *previous;
        after.next_page_ordinal = NO_NEXT_LP_PAGE;
        after.sealed = false;
        after.revision = next(previous.revision)?;
        after.validate_against(policy, state, state_account_id)?;
        state_after.lp_page_set_root = after.page_content_id()?;
        state_after.last_lp_owner = tail_last_owner(&after);
        Some(after)
    };
    state_after.validate_against_policy(policy)?;
    Ok(PreparedDealerEmptyLpPageCloseV2 {
        previous_page_after,
        state_after,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_funding_replay_v2(
    state: &DealerStateV2,
    state_after: &DealerStateV2,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    action: DealerRuntimeActionV1,
    liveness_receipt_semantic_id: Id,
    liveness_mode: DealerTransitionLivenessModeV1,
    asset_transfer_bundle_id: Id,
    position_pre_semantic_id: Id,
    position_post_semantic_id: Id,
) -> Result<PreparedDealerReplayTransitionV1> {
    if replay.facility_position_account_id() != state.facility_position_account_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_binding_id() != state.facility_position_binding_id
        || replay.position_generation() != state.generation
    {
        return Err(Error::MismatchedBinding);
    }
    replay.prepare_transition(
        replay_binding,
        DealerTransitionIntentV1 {
            replay_account_id: replay.replay_account_id(),
            replay_pre_id: replay.replay_id()?,
            state_pre_content_id: state.state_content_id()?,
            state_post_content_id: state_after.state_content_id()?,
            position_pre_semantic_id,
            position_post_semantic_id,
            liveness_receipt_semantic_id,
            fee_receipt_semantic_id: Id::ZERO,
            asset_transfer_bundle_id,
            position_generation_before: replay.position_generation(),
            position_generation_after: replay.position_generation(),
            expected_ordinal: replay.next_transition_ordinal(),
            action,
            liveness_mode,
        },
    )
}

fn next(value: u64) -> Result<u64> {
    value.checked_add(1).ok_or(Error::ArithmeticOverflow)
}
