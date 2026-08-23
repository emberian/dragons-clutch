// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical Position V3, fee-terminal, liveness, and Replay closure of one Lease/Pot.

use clutch_fee_runtime_contract::terminal::FeeTerminalOutcomeV1;
use sha2::{Digest, Sha256};

use crate::{
    DealerActionLivenessAuthorizationV1, DealerAssetTransferAmountsV1, DealerFacilityReplayV1,
    DealerFeeTerminalJoinV1, DealerFundedDependenciesV2, DealerLeaseV2, DealerLivenessScheduleV1,
    DealerPhaseV2, DealerPolicyV1, DealerPositionMarketJoinV1, DealerPositionObservationV3,
    DealerPotCustodyTransitionV1, DealerReplayAccountBindingV1, DealerRuntimeActionV1,
    DealerRuntimeLivenessBindingV1, DealerStateV2, DealerTransferPositionV3,
    DealerTransitionIntentV1, DealerTransitionLivenessModeV1, Error, FacilityPositionBindingV2, Id,
    PreparedDealerPositionPotTransferV1, PreparedDealerReplayTransitionV1, Result,
    SettlementPotPhaseV1, SettlementPotV2, MAX_OUTCOMES,
};

/// Exact semantic domain for the ephemeral Pot close transition identity.
pub const DEALER_LEASE_POT_CLOSE_TRANSITION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/dealer-lease-pot-close-transition/v3\0";

/// Physical rent observation for one atomic Lease/Pot deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeasePotCloseRentV3 {
    /// Lease lamports before deletion.
    pub lease_lamports_before: u64,
    /// Pot lamports before deletion.
    pub pot_lamports_before: u64,
    /// Lease lamports after deletion; exactly zero.
    pub lease_lamports_after: u64,
    /// Pot lamports after deletion; exactly zero.
    pub pot_lamports_after: u64,
}

/// Exact immutable transition fact used as the deleted Pot's post-identity.
///
/// This is not a persisted receipt account. Its identity commits the complete
/// deletion preimage, while Replay commits the resulting State and Position
/// postimages. The SBF adapter must perform both account deletions, both rent
/// transfers, the Position write, the State write, and the Replay write in the
/// same transaction before accepting the prepared result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLeasePotCloseTransitionV3 {
    transition_id: Id,
    action: DealerRuntimeActionV1,
    state_account_id: Id,
    lease_account_id: Id,
    pot_account_id: Id,
    pot_pre_content_id: Id,
    position_account_id: Id,
    position_pre_semantic_id: Id,
    fee_terminal_receipt_id: Id,
    liveness_receipt_id: Id,
    refund_recipients: [Id; 2],
    refund_lamports: [u64; 2],
    neutral_sink: Id,
    neutral_sink_lamports: u64,
}

impl DealerLeasePotCloseTransitionV3 {
    /// Canonical transition identity used as the deleted Pot post-identity.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }

    /// Exact terminal Dealer action.
    pub const fn action(self) -> DealerRuntimeActionV1 {
        self.action
    }

    /// Authoritative Dealer State account.
    pub const fn state_account_id(self) -> Id {
        self.state_account_id
    }

    /// Deleted Lease account.
    pub const fn lease_account_id(self) -> Id {
        self.lease_account_id
    }

    /// Deleted Pot account.
    pub const fn pot_account_id(self) -> Id {
        self.pot_account_id
    }

    /// Semantic identity recomputed from the Pot prestate.
    pub const fn pot_pre_content_id(self) -> Id {
        self.pot_pre_content_id
    }

    /// Canonical facility Position account.
    pub const fn position_account_id(self) -> Id {
        self.position_account_id
    }

    /// Canonical facility Position semantic preidentity.
    pub const fn position_pre_semantic_id(self) -> Id {
        self.position_pre_semantic_id
    }

    /// Canonical fee-runtime terminal receipt.
    pub const fn fee_terminal_receipt_id(self) -> Id {
        self.fee_terminal_receipt_id
    }

    /// Canonical external-liveness work receipt.
    pub const fn liveness_receipt_id(self) -> Id {
        self.liveness_receipt_id
    }

    /// Lease/Pot refundable-principal recipients, in that order.
    pub const fn refund_recipients(self) -> [Id; 2] {
        self.refund_recipients
    }

    /// Lease/Pot refundable principals, in that order.
    pub const fn refund_lamports(self) -> [u64; 2] {
        self.refund_lamports
    }

    /// Immutable neutral sink for every donation lamport.
    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    /// Exact combined hostile prefund and later surplus credit.
    pub const fn neutral_sink_lamports(self) -> u64 {
        self.neutral_sink_lamports
    }
}

/// Atomic pure result for Finalize or pre-collection Abort.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerLeasePotCloseV3 {
    state_after: DealerStateV2,
    transfer: PreparedDealerPositionPotTransferV1,
    replay: PreparedDealerReplayTransitionV1,
    close: DealerLeasePotCloseTransitionV3,
}

impl PreparedDealerLeasePotCloseV3 {
    /// Authoritative State postimage after both counted children close.
    pub const fn state_after(self) -> DealerStateV2 {
        self.state_after
    }

    /// Exact Pot-to-facility Position transfer.
    pub const fn transfer(self) -> PreparedDealerPositionPotTransferV1 {
        self.transfer
    }

    /// Same-transaction canonical Replay advance.
    pub const fn replay(self) -> PreparedDealerReplayTransitionV1 {
        self.replay
    }

    /// Exact Lease/Pot deletion and rent-disposition fact.
    pub const fn close(self) -> DealerLeasePotCloseTransitionV3 {
        self.close
    }
}

/// Finalize one completely collected and delivered Pot against canonical external owners.
#[allow(clippy::too_many_arguments)]
pub fn prepare_finalize_lease_pot_v3(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    lease: &DealerLeaseV2,
    pot_account_id: Id,
    pot: &SettlementPotV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    fee_terminal: &DealerFeeTerminalJoinV1,
    market: DealerPositionMarketJoinV1,
    position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    rent: DealerLeasePotCloseRentV3,
) -> Result<PreparedDealerLeasePotCloseV3> {
    let post_net_sold = pot.validate_transition(policy, state, lease)?;
    if pot.phase != SettlementPotPhaseV1::Finalizing
        || fee_terminal.outcome != FeeTerminalOutcomeV1::Settled
    {
        return Err(Error::InvalidPhase);
    }
    prepare_lease_pot_close_common_v3(
        DealerRuntimeActionV1::FinalizeSettlement,
        policy,
        binding,
        state,
        state_account_id,
        dependency,
        lease,
        pot_account_id,
        pot,
        schedule,
        runtime,
        authorization,
        fee_terminal,
        market,
        position,
        replay,
        replay_binding,
        post_net_sold,
        rent,
    )
}

/// Abort only before the first user input, restoring the exact Begin deposit.
#[allow(clippy::too_many_arguments)]
pub fn prepare_abort_lease_pot_v3(
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    lease: &DealerLeaseV2,
    pot_account_id: Id,
    pot: &SettlementPotV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    fee_terminal: &DealerFeeTerminalJoinV1,
    market: DealerPositionMarketJoinV1,
    position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    current_slot: u64,
    rent: DealerLeasePotCloseRentV3,
) -> Result<PreparedDealerLeasePotCloseV3> {
    if pot.phase != SettlementPotPhaseV1::Collecting
        || pot.collect_cursor != 0
        || pot.deliver_cursor != 0
        || pot.collected_user_cash_atoms != 0
        || pot.collected_user_eggs != [0; MAX_OUTCOMES]
        || pot.delivered_user_cash_atoms != 0
        || pot.delivered_user_eggs != [0; MAX_OUTCOMES]
        || current_slot < lease.collect_deadline_slot
        || fee_terminal.outcome != FeeTerminalOutcomeV1::Aborted
    {
        return Err(Error::InvalidPhase);
    }
    prepare_lease_pot_close_common_v3(
        DealerRuntimeActionV1::AbortBeforeCollection,
        policy,
        binding,
        state,
        state_account_id,
        dependency,
        lease,
        pot_account_id,
        pot,
        schedule,
        runtime,
        authorization,
        fee_terminal,
        market,
        position,
        replay,
        replay_binding,
        state.net_sold,
        rent,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_lease_pot_close_common_v3(
    action: DealerRuntimeActionV1,
    policy: &DealerPolicyV1,
    binding: &FacilityPositionBindingV2,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    lease: &DealerLeaseV2,
    pot_account_id: Id,
    pot: &SettlementPotV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    authorization: &DealerActionLivenessAuthorizationV1,
    fee_terminal: &DealerFeeTerminalJoinV1,
    market: DealerPositionMarketJoinV1,
    position: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    post_net_sold: [i64; MAX_OUTCOMES],
    rent: DealerLeasePotCloseRentV3,
) -> Result<PreparedDealerLeasePotCloseV3> {
    state.validate_against_policy(policy)?;
    lease.validate()?;
    pot.validate_against_lease(lease)?;
    dependency.validate()?;
    authorization.validate_against(schedule, runtime)?;
    position.validate_current(state, binding, policy)?;
    replay.validate()?;
    state_account_id.validate_live()?;
    pot_account_id.validate_live()?;
    let dependency_id = dependency.dependency_id()?;
    let schedule_id = schedule.schedule_id()?.untyped();
    let runtime_digest = runtime.binding_digest()?;
    if !matches!(
        action,
        DealerRuntimeActionV1::FinalizeSettlement | DealerRuntimeActionV1::AbortBeforeCollection
    ) || !matches!(
        state.phase,
        DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly
    ) || state_account_id != binding.dealer_state_account_id
        || state_account_id != lease.dealer_state_account_id
        || pot_account_id != lease.settlement_pot_id
        || state.active_lease_id != lease.lease_account_id
        || state.children.funded_dependencies != 1
        || state.children.epoch_bindings != 1
        || state.children.leases != 1
        || state.children.settlement_pots != 1
        || state.funded_dependencies_id != dependency_id
        || lease.funded_dependencies_id != dependency_id
        || lease.runtime_liveness_binding_digest != runtime_digest
        || pot.runtime_liveness_binding_digest != runtime_digest
        || lease.dealer_liveness_schedule_id != schedule_id
        || pot.dealer_liveness_schedule_id != schedule_id
        || authorization.action != action
        || authorization.owner != state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
        || fee_terminal.selected_fee_record_account_id != lease.selected_fee_record_account_id
        || fee_terminal.selected_fee_record_semantic_id != lease.selected_fee_record_semantic_id
        || fee_terminal.settlement_candidate_id != lease.settlement_candidate_id
        || fee_terminal.fee_revenue_policy_id != lease.fee_revenue_policy_id
        || fee_terminal.available_liveness_lamports() != 0
        || fee_terminal.available_hoard_atoms() != 0
        || fee_terminal.available_fee_funding_atoms() != 0
        || replay.facility_position_account_id() != state.facility_position_account_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_binding_id() != state.facility_position_binding_id
        || replay.position_generation() != state.generation
    {
        return Err(Error::MismatchedBinding);
    }

    let position_body = position.projection.position();
    let custody = pot.derived_custody()?;
    let expected_custody = match action {
        DealerRuntimeActionV1::FinalizeSettlement => {
            if custody.cash_atoms != pot.dealer_net_cash_in_atoms
                || custody.eggs != pot.facility_buy_eggs
            {
                return Err(Error::ConservationFailure);
            }
            DealerAssetTransferAmountsV1 {
                cash_atoms: pot.dealer_net_cash_in_atoms,
                source_reserved_cash_atoms: 0,
                destination_reserved_cash_atoms: 0,
                native_eggs: pot.facility_buy_eggs,
            }
        }
        DealerRuntimeActionV1::AbortBeforeCollection => {
            if custody.cash_atoms != pot.dealer_net_cash_out_atoms
                || custody.eggs != pot.facility_sell_eggs
            {
                return Err(Error::ConservationFailure);
            }
            DealerAssetTransferAmountsV1 {
                cash_atoms: pot.dealer_net_cash_out_atoms,
                source_reserved_cash_atoms: 0,
                destination_reserved_cash_atoms: 0,
                native_eggs: pot.facility_sell_eggs,
            }
        }
        _ => return Err(Error::MismatchedBinding),
    };

    let pot_pre_content_id = pot.pot_content_id()?;
    let close = prepare_close_transition_v3(
        action,
        policy,
        state,
        state_account_id,
        lease,
        pot_account_id,
        pot,
        pot_pre_content_id,
        position.account_id,
        position.semantic_id,
        expected_custody,
        authorization.receipt_semantic_id,
        fee_terminal.terminal_receipt_id,
        replay,
        rent,
    )?;
    let transfer = crate::prepare_dealer_position_pot_transfer_v1(
        action,
        market,
        DealerTransferPositionV3::Facility {
            account_id: position.account_id,
            position: position.projection,
        },
        DealerPotCustodyTransitionV1 {
            pot_account_id,
            pot_pre_semantic_id: pot_pre_content_id,
            pot_post_semantic_id: close.transition_id(),
            cash_pre_atoms: custody.cash_atoms,
            cash_post_atoms: 0,
            eggs_pre: custody.eggs,
            eggs_post: [0; MAX_OUTCOMES],
            outcome_count: pot.outcome_count,
        },
        expected_custody,
    )?;
    let bundle = transfer.bundle();
    let position_post = transfer.position_post();
    if bundle.destination_pre_semantic_id != position.semantic_id
        || position_post.generation() != pot.post_generation
        || position_post.replay_account() != position_body.replay_account()
        || (action == DealerRuntimeActionV1::FinalizeSettlement
            && bundle.destination_post_semantic_id != pot.facility_position_post_id)
    {
        return Err(Error::ConservationFailure);
    }

    let mut state_after = *state;
    state_after.facility_position_id = bundle.destination_post_semantic_id;
    state_after.generation = pot.post_generation;
    state_after.net_sold = post_net_sold;
    state_after.active_lease_id = Id::ZERO;
    state_after.children.leases = 0;
    state_after.children.settlement_pots = 0;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(2)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.validate_against_policy(policy)?;

    let prepared_replay = replay.prepare_transition(
        replay_binding,
        DealerTransitionIntentV1 {
            replay_account_id: replay.replay_account_id(),
            replay_pre_id: replay.replay_id()?,
            state_pre_content_id: state.state_content_id()?,
            state_post_content_id: state_after.state_content_id()?,
            position_pre_semantic_id: bundle.destination_pre_semantic_id,
            position_post_semantic_id: bundle.destination_post_semantic_id,
            liveness_receipt_semantic_id: authorization.receipt_semantic_id,
            fee_evidence_id: fee_terminal.terminal_receipt_id,
            asset_transfer_bundle_id: bundle.bundle_id()?,
            position_generation_before: state.generation,
            position_generation_after: pot.post_generation,
            expected_ordinal: replay.next_transition_ordinal(),
            action,
            liveness_mode: DealerTransitionLivenessModeV1::ExternalReceipt,
        },
    )?;
    Ok(PreparedDealerLeasePotCloseV3 {
        state_after,
        transfer,
        replay: prepared_replay,
        close,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_close_transition_v3(
    action: DealerRuntimeActionV1,
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    lease: &DealerLeaseV2,
    pot_account_id: Id,
    pot: &SettlementPotV2,
    pot_pre_content_id: Id,
    position_account_id: Id,
    position_pre_semantic_id: Id,
    amounts: DealerAssetTransferAmountsV1,
    liveness_receipt_id: Id,
    fee_terminal_receipt_id: Id,
    replay: &DealerFacilityReplayV1,
    rent: DealerLeasePotCloseRentV3,
) -> Result<DealerLeasePotCloseTransitionV3> {
    for identity in [
        state_account_id,
        lease.lease_account_id,
        pot_account_id,
        pot_pre_content_id,
        position_account_id,
        position_pre_semantic_id,
        liveness_receipt_id,
        fee_terminal_receipt_id,
    ] {
        identity.validate_live()?;
    }
    let lease_protected = lease
        .rent
        .refundable_principal
        .checked_add(lease.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    let pot_protected = pot
        .rent
        .refundable_principal
        .checked_add(pot.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    if rent.lease_lamports_after != 0
        || rent.pot_lamports_after != 0
        || rent.lease_lamports_before < lease_protected
        || rent.pot_lamports_before < pot_protected
        || pot.rent.neutral_sink != lease.rent.neutral_sink
        || pot.rent.neutral_sink != policy.neutral_sink
    {
        return Err(Error::ConservationFailure);
    }
    if lease.settlement_pot_id != pot_account_id {
        return Err(Error::MismatchedBinding);
    }
    let mut hasher = Sha256::new();
    hasher.update(DEALER_LEASE_POT_CLOSE_TRANSITION_DOMAIN_V3);
    hasher.update([crate::replay::action_byte(action)]);
    for identity in [
        policy.policy_id()?,
        state.facility_id,
        state_account_id,
        state.state_content_id()?,
        lease.lease_account_id,
        lease.lease_id()?,
        pot_account_id,
        pot_pre_content_id,
        lease.settlement_candidate_id,
        position_account_id,
        position_pre_semantic_id,
        liveness_receipt_id,
        fee_terminal_receipt_id,
        replay.replay_account_id(),
        replay.replay_id()?,
    ] {
        hasher.update(identity.bytes());
    }
    hasher.update(state.generation.to_le_bytes());
    hasher.update(lease.post_generation.to_le_bytes());
    hasher.update(replay.next_transition_ordinal().to_le_bytes());
    hasher.update(amounts.cash_atoms.to_le_bytes());
    hasher.update(amounts.source_reserved_cash_atoms.to_le_bytes());
    hasher.update(amounts.destination_reserved_cash_atoms.to_le_bytes());
    let mut outcome = 0usize;
    while outcome < MAX_OUTCOMES {
        hasher.update(amounts.native_eggs[outcome].to_le_bytes());
        outcome += 1;
    }
    hasher.update(rent.lease_lamports_before.to_le_bytes());
    hasher.update(rent.pot_lamports_before.to_le_bytes());
    hasher.update(lease.rent.payer.bytes());
    hasher.update(lease.rent.refundable_principal.to_le_bytes());
    hasher.update(lease.rent.donation_floor.to_le_bytes());
    hasher.update(pot.rent.payer.bytes());
    hasher.update(pot.rent.refundable_principal.to_le_bytes());
    hasher.update(pot.rent.donation_floor.to_le_bytes());
    hasher.update(lease.rent.neutral_sink.bytes());
    let transition_id = Id::from_bytes(hasher.finalize().into());
    transition_id.validate_live()?;
    let neutral_sink_lamports = rent
        .lease_lamports_before
        .checked_sub(lease.rent.refundable_principal)
        .and_then(|value| {
            value.checked_add(
                rent.pot_lamports_before
                    .checked_sub(pot.rent.refundable_principal)?,
            )
        })
        .ok_or(Error::ArithmeticOverflow)?;
    Ok(DealerLeasePotCloseTransitionV3 {
        transition_id,
        action,
        state_account_id,
        lease_account_id: lease.lease_account_id,
        pot_account_id,
        pot_pre_content_id,
        position_account_id,
        position_pre_semantic_id,
        fee_terminal_receipt_id,
        liveness_receipt_id,
        refund_recipients: [lease.rent.payer, pot.rent.payer],
        refund_lamports: [
            lease.rent.refundable_principal,
            pot.rent.refundable_principal,
        ],
        neutral_sink: policy.neutral_sink,
        neutral_sink_lamports,
    })
}
