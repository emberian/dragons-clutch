// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical Position V3 and Replay V3 admission of one Dealer Lease/Pot pair.

use sha2::{Digest, Sha256};

use crate::{
    DealerActionLivenessAuthorizationV1, DealerAssetTransferAmountsV1, DealerAssetTransferBundleV1,
    DealerEpochBindingV2, DealerFacilityGenesisV1, DealerFacilityReplayV1,
    DealerFundedDependenciesV2, DealerLeaseSelectionEvidenceV3, DealerLeaseV2,
    DealerLivenessScheduleV1, DealerPhaseV2, DealerPolicyV1, DealerPositionMarketJoinV1,
    DealerPotCustodyTransitionV1, DealerReplayAccountBindingV1, DealerRuntimeActionV1,
    DealerRuntimeLivenessBindingV1, DealerSelectedFeeRecordBindingV1, DealerStateV2,
    DealerTransferPositionV3, DealerTransitionIntentV1, DealerTransitionLivenessModeV1, Error,
    FacilityPositionBindingV2, Id, PreparedDealerPositionPotTransferV1,
    PreparedDealerReplayTransitionV1, Result, SettlementPotV2, MAX_OUTCOMES,
};

/// Exact domain for the pre-creation identity of a newly admitted Pot account.
pub const DEALER_POT_CREATION_INTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-pot-creation-intent/v1\0";

/// Atomic result of selected Lease/Pot admission over canonical shared owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerBeginLeaseV3 {
    /// State after admitting the Lease and Pot and updating current Position identity.
    pub state_after: DealerStateV2,
    /// Epoch child after binding the exact Lease account and candidate.
    pub epoch_after: DealerEpochBindingV2,
    /// Exact Facility Position-to-new-Pot transfer.
    pub transfer: PreparedDealerPositionPotTransferV1,
    /// Same-generation Replay advance binding State, transfer, fee, and liveness.
    pub replay: PreparedDealerReplayTransitionV1,
    /// Canonical pre-creation Pot intent, never a persisted balance owner.
    pub pot_creation_intent_id: Id,
}

/// Derive the unique pre-creation identity for one exact Pot postimage.
pub fn dealer_pot_creation_intent_id_v1(
    pot_account_id: Id,
    lease: &DealerLeaseV2,
    pot: &SettlementPotV2,
) -> Result<Id> {
    pot_account_id.validate_live()?;
    lease.validate()?;
    pot.validate_against_lease(lease)?;
    if pot_account_id != lease.settlement_pot_id {
        return Err(Error::MismatchedBinding);
    }
    let mut hasher = Sha256::new();
    hasher.update(DEALER_POT_CREATION_INTENT_DOMAIN_V1);
    hasher.update(pot_account_id.bytes());
    hasher.update(lease.lease_id()?.bytes());
    hasher.update(pot.pot_content_id()?.bytes());
    let value = Id::from_bytes(hasher.finalize().into());
    value.validate_live()?;
    Ok(value)
}

/// Admit one authenticated selected Lease/Pot and move the exact Begin deposit.
#[allow(clippy::too_many_arguments)]
pub fn prepare_begin_lease_pot_v3(
    genesis: &DealerFacilityGenesisV1,
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    epoch: &DealerEpochBindingV2,
    selection: &DealerLeaseSelectionEvidenceV3,
    lease: &DealerLeaseV2,
    pot_account_id: Id,
    pot: &SettlementPotV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    select_begin: &DealerActionLivenessAuthorizationV1,
    selected_fee: &DealerSelectedFeeRecordBindingV1,
    market: DealerPositionMarketJoinV1,
    facility_position: DealerTransferPositionV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    current_slot: u64,
) -> Result<PreparedDealerBeginLeaseV3> {
    state.validate_against_policy(policy)?;
    let binding_id = binding.binding_id_for(genesis, policy)?;
    dependency.validate_bindings_v3(genesis, binding, policy, schedule, runtime)?;
    select_begin.validate_against(schedule, runtime)?;
    selected_fee.validate()?;
    epoch.validate()?;
    lease.validate()?;
    pot.validate_against_lease(lease)?;
    selection.validate_lease_pot(lease, pot, epoch, policy)?;
    replay.validate()?;
    let facility_before = match facility_position {
        DealerTransferPositionV3::Facility {
            account_id,
            position,
        } => {
            let body = position.position();
            if account_id != state.facility_position_account_id
                || Id::from_bytes(body.purpose_binding_id().bytes()) != binding_id
                || Id::from_bytes(body.replay_account().bytes()) != state.facility_replay_account_id
                || body.generation() != state.generation
            {
                return Err(Error::MismatchedBinding);
            }
            body
        }
        _ => return Err(Error::MismatchedBinding),
    };
    if !matches!(
        state.phase,
        DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly
    ) || state_account_id != binding.dealer_state_account_id
        || state.facility_position_binding_id != binding_id
        || state.children.funded_dependencies != 1
        || state.children.epoch_bindings != 1
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || !state.active_lease_id.is_zero()
        || state.active_epoch_id != epoch.epoch_id
        || state.active_epoch_binding_account_id != epoch.epoch_binding_account_id
        || epoch.counted_generation != state.generation
        || lease.pre_generation != state.generation
        || lease.created_slot != current_slot
        || current_slot >= lease.collect_deadline_slot
        || select_begin.action != DealerRuntimeActionV1::SelectLeaseAndBegin
        || select_begin.owner != state_account_id
        || select_begin.lifecycle_id != state.facility_id
        || select_begin.facility_generation != state.generation
        || replay.facility_position_account_id() != state.facility_position_account_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_binding_id() != binding_id
        || replay.position_generation() != state.generation
        || market.market_instance_v2_id != policy.market_instance_v2_id
        || market.realm_id != policy.realm_id
        || market.outcome_count != policy.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let pot_creation_intent_id = dealer_pot_creation_intent_id_v1(pot_account_id, lease, pot)?;
    let custody_after = pot.derived_custody()?;
    if custody_after.cash_atoms != pot.dealer_net_cash_out_atoms
        || custody_after.eggs != pot.facility_sell_eggs
    {
        return Err(Error::ConservationFailure);
    }
    let transfer = crate::prepare_dealer_position_pot_transfer_v1(
        DealerRuntimeActionV1::SelectLeaseAndBegin,
        market,
        facility_position,
        DealerPotCustodyTransitionV1 {
            pot_account_id,
            pot_pre_semantic_id: pot_creation_intent_id,
            pot_post_semantic_id: pot.pot_content_id()?,
            cash_pre_atoms: 0,
            cash_post_atoms: custody_after.cash_atoms,
            eggs_pre: [0; MAX_OUTCOMES],
            eggs_post: custody_after.eggs,
            outcome_count: policy.outcome_count,
        },
        DealerAssetTransferAmountsV1 {
            cash_atoms: pot.dealer_net_cash_out_atoms,
            source_reserved_cash_atoms: 0,
            destination_reserved_cash_atoms: 0,
            native_eggs: pot.facility_sell_eggs,
        },
    )?;
    let bundle: DealerAssetTransferBundleV1 = transfer.bundle();
    if bundle.source_pre_semantic_id != state.facility_position_id
        || bundle.source_post_semantic_id != lease.facility_position_leased_id
        || pot.facility_position_pre_id != state.facility_position_id
        || pot.facility_position_leased_id != bundle.source_post_semantic_id
        || transfer.position_post().generation() != facility_before.generation()
        || transfer.position_post().replay_account() != facility_before.replay_account()
    {
        return Err(Error::ConservationFailure);
    }
    let mut state_after = *state;
    state_after.facility_position_id = bundle.source_post_semantic_id;
    state_after.active_lease_id = lease.lease_account_id;
    state_after.children.leases = 1;
    state_after.children.settlement_pots = 1;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(2)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.validate_against_policy(policy)?;
    lease.validate_bindings(
        policy,
        &state_after,
        dependency,
        schedule,
        runtime,
        select_begin,
        selected_fee,
    )?;
    pot.validate_transition(policy, &state_after, lease)?;
    let epoch_after = crate::mark_epoch_leased_v2(&state_after, epoch, lease)?;
    let selected_fee_digest = selected_fee.binding_digest()?;
    let prepared_replay = replay.prepare_transition(
        replay_binding,
        DealerTransitionIntentV1 {
            replay_account_id: replay.replay_account_id(),
            replay_pre_id: replay.replay_id()?,
            state_pre_content_id: state.state_content_id()?,
            state_post_content_id: state_after.state_content_id()?,
            position_pre_semantic_id: bundle.source_pre_semantic_id,
            position_post_semantic_id: bundle.source_post_semantic_id,
            liveness_receipt_semantic_id: select_begin.receipt_semantic_id,
            fee_evidence_id: selected_fee_digest,
            asset_transfer_bundle_id: bundle.bundle_id()?,
            position_generation_before: state.generation,
            position_generation_after: state.generation,
            expected_ordinal: replay.next_transition_ordinal(),
            action: DealerRuntimeActionV1::SelectLeaseAndBegin,
            liveness_mode: DealerTransitionLivenessModeV1::ExternalReceipt,
        },
    )?;
    Ok(PreparedDealerBeginLeaseV3 {
        state_after,
        epoch_after,
        transfer,
        replay: prepared_replay,
        pot_creation_intent_id,
    })
}
