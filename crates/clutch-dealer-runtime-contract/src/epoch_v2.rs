// SPDX-License-Identifier: AGPL-3.0-or-later

//! Counted Dealer Epoch binding with funded bind/lapse/retirement paths.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    CountedDealerChildV2, DealerActionLivenessAuthorizationV1, DealerChildKindV2,
    DealerFundedDependenciesV2, DealerLeaseV2, DealerLivenessScheduleV1, DealerPhaseV2,
    DealerPolicyV1, DealerPositionObservationV3, DealerRuntimeActionV1,
    DealerRuntimeLivenessBindingV1, DealerStateV2, DeletableRentOwnerV1, Error,
    FacilityPositionBindingV2, FixedCodec, Id, Result, DEALER_EPOCH_BINDING_CONTENT_DOMAIN_V2,
    DELETABLE_RENT_OWNER_BYTES, DealerEmptyAssetTransferBundleV1, DealerFacilityReplayV1,
    DealerReplayAccountBindingV1, DealerTransitionIntentV1, DealerTransitionLivenessModeV1,
    PreparedDealerReplayTransitionV1,
};
use clutch_retirement::PositionLifecycleV3;

/// Local semantic magic for the V2 Epoch binding.
pub const DEALER_EPOCH_BINDING_MAGIC_V2: [u8; 8] = *b"DCDEPOV2";
/// Exact local semantic version.
pub const DEALER_EPOCH_BINDING_VERSION_V2: u16 = 2;
/// Exact canonical body bytes.
pub const DEALER_EPOCH_BINDING_BYTES_V2: usize =
    HEADER_BYTES + (20 * 32) + (3 * 8) + 8 + DELETABLE_RENT_OWNER_BYTES;

/// Exhaustive Epoch-binding phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerEpochBindingPhaseV2 {
    /// Bound to one upstream Epoch; no Lease has been admitted.
    Bound = 1,
    /// One exact Lease/candidate was admitted and must retire this binding.
    Leased = 2,
}

impl DealerEpochBindingPhaseV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Bound),
            2 => Ok(Self::Leased),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Counted binding from a Dealer generation to one authenticated General Epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEpochBindingV2 {
    /// Exact Dealer policy.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Immutable purpose binding for the shared canonical Position V3.
    pub facility_position_binding_id: Id,
    /// Authoritative DealerState account.
    pub dealer_state_account_id: Id,
    /// Physical counted binding account.
    pub epoch_binding_account_id: Id,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Physical authenticated General Epoch account.
    pub epoch_account_id: Id,
    /// Semantic General Epoch identity used by settlement/fee joins.
    pub epoch_id: Id,
    /// Exact RelationV2 identity.
    pub relation_v2_id: Id,
    /// Canonical EconomicDomainV2 identity.
    pub economic_domain_id: Id,
    /// Quantized price-measure policy identity.
    pub price_measure_policy_id: Id,
    /// Counted funded-dependency semantic identity.
    pub funded_dependencies_id: Id,
    /// External liveness runtime policy.
    pub runtime_liveness_policy_id: Id,
    /// Seven-account external liveness binding digest.
    pub runtime_liveness_binding_digest: Id,
    /// Fine-grained Dealer liveness schedule.
    pub dealer_liveness_schedule_id: Id,
    /// Exact successful BindEpoch receipt account.
    pub bind_receipt_account_id: Id,
    /// Exact BindEpoch receipt semantic identity.
    pub bind_receipt_semantic_id: Id,
    /// Program admitted to own that receipt.
    pub bind_receipt_program_id: Id,
    /// Active Lease account, zero while Bound.
    pub active_lease_account_id: Id,
    /// Final SettlementCandidateId, zero while Bound.
    pub settlement_candidate_id: Id,
    /// Exact Dealer generation consumed by this Epoch.
    pub counted_generation: u64,
    /// Slot at which binding occurred.
    pub bound_slot: u64,
    /// First slot at which an unused binding may lapse.
    pub lapse_after_slot: u64,
    /// Exhaustive phase.
    pub phase: DealerEpochBindingPhaseV2,
    /// Exact rent owner for the counted child.
    pub rent: DeletableRentOwnerV1,
}

impl DealerEpochBindingV2 {
    /// Validate exact identity, phase, schedule, and rent shape.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
            self.epoch_binding_account_id,
            self.market_instance_v2_id,
            self.epoch_account_id,
            self.epoch_id,
            self.relation_v2_id,
            self.economic_domain_id,
            self.price_measure_policy_id,
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.bind_receipt_account_id,
            self.bind_receipt_semantic_id,
            self.bind_receipt_program_id,
        ] {
            identity.validate_live()?;
        }
        if self.counted_generation == 0
            || self.bound_slot == 0
            || self.bound_slot >= self.lapse_after_slot
            || self.epoch_account_id == self.epoch_binding_account_id
            || self.dealer_state_account_id == self.epoch_binding_account_id
        {
            return Err(Error::InvalidParameter);
        }
        match self.phase {
            DealerEpochBindingPhaseV2::Bound => {
                if !self.active_lease_account_id.is_zero()
                    || !self.settlement_candidate_id.is_zero()
                {
                    return Err(Error::InvalidPhase);
                }
            }
            DealerEpochBindingPhaseV2::Leased => {
                self.active_lease_account_id.validate_live()?;
                self.settlement_candidate_id.validate_live()?;
            }
        }
        self.rent.validate()
    }

    /// Counted root edge.
    pub const fn counted_child(&self) -> CountedDealerChildV2 {
        CountedDealerChildV2 {
            facility_id: self.facility_id,
            facility_position_binding_id: self.facility_position_binding_id,
            kind: DealerChildKindV2::EpochBinding,
            counted_generation: self.counted_generation,
        }
    }

    /// Exact semantic identity of the current binding body.
    pub fn binding_id(&self) -> Result<Id> {
        self.content_id(DEALER_EPOCH_BINDING_CONTENT_DOMAIN_V2)
    }

    /// Require exact policy, State, dependency and funded BindEpoch receipt joins.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_bound(
        &self,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        state_account_id: Id,
        dependency: &DealerFundedDependenciesV2,
        schedule: &DealerLivenessScheduleV1,
        runtime: &DealerRuntimeLivenessBindingV1,
        bind: &DealerActionLivenessAuthorizationV1,
    ) -> Result<()> {
        self.validate()?;
        state.validate_against_policy(policy)?;
        dependency.validate()?;
        schedule.validate_for_facility_runtime()?;
        runtime.validate()?;
        bind.validate_against(schedule, runtime)?;
        if self.phase != DealerEpochBindingPhaseV2::Bound
            || !matches!(state.phase, DealerPhaseV2::Trading | DealerPhaseV2::UnwindOnly)
            || self.policy_id != policy.policy_id()?
            || self.facility_id != state.facility_id
            || self.facility_position_binding_id != state.facility_position_binding_id
            || self.dealer_state_account_id != state_account_id
            || self.epoch_binding_account_id != state.active_epoch_binding_account_id
            || self.epoch_id != state.active_epoch_id
            || self.market_instance_v2_id != policy.market_instance_v2_id
            || self.relation_v2_id != policy.relation_v2_id
            || self.price_measure_policy_id != policy.price_measure_policy_id
            || self.funded_dependencies_id != state.funded_dependencies_id
            || self.funded_dependencies_id != dependency.dependency_id()?
            || self.runtime_liveness_policy_id != runtime.runtime_policy_id
            || self.runtime_liveness_policy_id
                != dependency.bindings.runtime_liveness_policy_id
            || self.runtime_liveness_binding_digest != runtime.binding_digest()?
            || self.runtime_liveness_binding_digest
                != dependency.bindings.runtime_liveness_binding_digest
            || self.dealer_liveness_schedule_id != schedule.schedule_id()?.untyped()
            || self.dealer_liveness_schedule_id != dependency.bindings.liveness_schedule_id
            || self.counted_generation != state.generation
            || state.children.epoch_bindings != 1
            || bind.action != DealerRuntimeActionV1::BindEpoch
            || bind.owner != state_account_id
            || bind.lifecycle_id != self.facility_id
            || bind.facility_generation != self.counted_generation
            || self.bind_receipt_account_id != bind.receipt_account_id
            || self.bind_receipt_semantic_id != bind.receipt_semantic_id
            || self.bind_receipt_program_id != bind.receipt_program_id
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }
}

impl FixedCodec for DealerEpochBindingV2 {
    const ENCODED_LEN: usize = DEALER_EPOCH_BINDING_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&DEALER_EPOCH_BINDING_MAGIC_V2, DEALER_EPOCH_BINDING_VERSION_V2);
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
            self.epoch_binding_account_id,
            self.market_instance_v2_id,
            self.epoch_account_id,
            self.epoch_id,
            self.relation_v2_id,
            self.economic_domain_id,
            self.price_measure_policy_id,
            self.funded_dependencies_id,
            self.runtime_liveness_policy_id,
            self.runtime_liveness_binding_digest,
            self.dealer_liveness_schedule_id,
            self.bind_receipt_account_id,
            self.bind_receipt_semantic_id,
            self.bind_receipt_program_id,
            self.active_lease_account_id,
            self.settlement_candidate_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.counted_generation);
        writer.u64(self.bound_slot);
        writer.u64(self.lapse_after_slot);
        writer.u8(self.phase as u8);
        writer.reserved(7);
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&DEALER_EPOCH_BINDING_MAGIC_V2, DEALER_EPOCH_BINDING_VERSION_V2)?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            facility_position_binding_id: reader.id(),
            dealer_state_account_id: reader.id(),
            epoch_binding_account_id: reader.id(),
            market_instance_v2_id: reader.id(),
            epoch_account_id: reader.id(),
            epoch_id: reader.id(),
            relation_v2_id: reader.id(),
            economic_domain_id: reader.id(),
            price_measure_policy_id: reader.id(),
            funded_dependencies_id: reader.id(),
            runtime_liveness_policy_id: reader.id(),
            runtime_liveness_binding_digest: reader.id(),
            dealer_liveness_schedule_id: reader.id(),
            bind_receipt_account_id: reader.id(),
            bind_receipt_semantic_id: reader.id(),
            bind_receipt_program_id: reader.id(),
            active_lease_account_id: reader.id(),
            settlement_candidate_id: reader.id(),
            counted_generation: reader.u64(),
            bound_slot: reader.u64(),
            lapse_after_slot: reader.u64(),
            phase: DealerEpochBindingPhaseV2::decode(reader.u8())?,
            rent: {
                reader.reserved(7)?;
                DeletableRentOwnerV1::decode_body(&mut reader)
            },
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Admit one authenticated Epoch binding as the sole active Epoch child.
#[allow(clippy::too_many_arguments)]
pub fn bind_epoch_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    bind: &DealerActionLivenessAuthorizationV1,
    epoch: &DealerEpochBindingV2,
) -> Result<DealerStateV2> {
    state.validate_against_policy(policy)?;
    if state.children.epoch_bindings != 0
        || !state.active_epoch_id.is_zero()
        || !state.active_epoch_binding_account_id.is_zero()
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || epoch.counted_generation != state.generation
    {
        return Err(Error::InvalidChildGraph);
    }
    let mut next = *state;
    next.active_epoch_id = epoch.epoch_id;
    next.active_epoch_binding_account_id = epoch.epoch_binding_account_id;
    next.children.epoch_bindings = 1;
    next.child_sequence = next
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    epoch.validate_bound(policy, &next, state_account_id, dependency, schedule, runtime, bind)?;
    Ok(next)
}

/// Atomic State/Replay result of one funded Epoch admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerEpochBindV3 {
    /// State after counting the exact Epoch binding.
    pub state_after: DealerStateV2,
    /// Replay advance binding State and funded receipt.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Bind an Epoch and the exact funded receipt into one Replay transition.
#[allow(clippy::too_many_arguments)]
pub fn prepare_bind_epoch_v3(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    bind: &DealerActionLivenessAuthorizationV1,
    epoch: &DealerEpochBindingV2,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerEpochBindV3> {
    let state_after = bind_epoch_v2(
        policy,
        state,
        state_account_id,
        dependency,
        schedule,
        runtime,
        bind,
        epoch,
    )?;
    validate_live_replay(state, replay)?;
    let empty = DealerEmptyAssetTransferBundleV1 {
        action: DealerRuntimeActionV1::BindEpoch,
    };
    let prepared = replay.prepare_transition(
        replay_binding,
        DealerTransitionIntentV1 {
            replay_account_id: replay.replay_account_id(),
            replay_pre_id: replay.replay_id()?,
            state_pre_content_id: state.state_content_id()?,
            state_post_content_id: state_after.state_content_id()?,
            position_pre_semantic_id: state.facility_position_id,
            position_post_semantic_id: state.facility_position_id,
            liveness_receipt_semantic_id: bind.receipt_semantic_id,
            fee_receipt_semantic_id: Id::ZERO,
            asset_transfer_bundle_id: empty.bundle_id()?,
            position_generation_before: state.generation,
            position_generation_after: state.generation,
            expected_ordinal: replay.next_transition_ordinal(),
            action: DealerRuntimeActionV1::BindEpoch,
            liveness_mode: DealerTransitionLivenessModeV1::ExternalReceipt,
        },
    )?;
    Ok(PreparedDealerEpochBindV3 {
        state_after,
        replay: prepared,
    })
}

/// Bind the exact admitted Lease and candidate into the mutable Epoch child.
pub fn mark_epoch_leased_v2(
    state: &DealerStateV2,
    epoch: &DealerEpochBindingV2,
    lease: &DealerLeaseV2,
) -> Result<DealerEpochBindingV2> {
    state.validate()?;
    epoch.validate()?;
    lease.validate()?;
    if epoch.phase != DealerEpochBindingPhaseV2::Bound
        || epoch.policy_id != lease.policy_id
        || epoch.facility_id != lease.facility_id
        || epoch.facility_position_binding_id != lease.facility_position_binding_id
        || epoch.dealer_state_account_id != lease.dealer_state_account_id
        || epoch.epoch_binding_account_id != state.active_epoch_binding_account_id
        || epoch.epoch_id != state.active_epoch_id
        || epoch.epoch_id != lease.epoch_id
        || epoch.epoch_binding_account_id != lease.epoch_binding_account_id
        || state.active_lease_id != lease.lease_account_id
        || lease.pre_generation != epoch.counted_generation
    {
        return Err(Error::MismatchedBinding);
    }
    let mut next = *epoch;
    next.phase = DealerEpochBindingPhaseV2::Leased;
    next.active_lease_account_id = lease.lease_account_id;
    next.settlement_candidate_id = lease.settlement_candidate_id;
    next.validate()?;
    Ok(next)
}

/// Exact rent close observation for one Epoch-binding account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEpochCloseRentV2 {
    /// Exact payer credited by the close.
    pub payer: Id,
    /// Exact neutral sink credited by the close.
    pub neutral_sink: Id,
    /// Lamports observed before close.
    pub lamports_before: u64,
    /// Lamports observed after close; exactly zero.
    pub lamports_after: u64,
    /// Refund recipient balance before close.
    pub payer_before: u64,
    /// Refund recipient balance after close.
    pub payer_after: u64,
    /// Neutral-sink balance before close.
    pub sink_before: u64,
    /// Neutral-sink balance after close.
    pub sink_after: u64,
}

fn validate_epoch_close_rent(
    epoch: &DealerEpochBindingV2,
    observation: DealerEpochCloseRentV2,
) -> Result<()> {
    observation.payer.validate_live()?;
    observation.neutral_sink.validate_live()?;
    let protected = epoch
        .rent
        .refundable_principal
        .checked_add(epoch.rent.donation_floor)
        .ok_or(Error::ArithmeticOverflow)?;
    let donation = observation
        .lamports_before
        .checked_sub(epoch.rent.refundable_principal)
        .ok_or(Error::ConservationFailure)?;
    if observation.payer != epoch.rent.payer
        || observation.neutral_sink != epoch.rent.neutral_sink
        || observation.lamports_after != 0
        || observation.lamports_before < protected
        || observation.payer_after
            != observation
                .payer_before
                .checked_add(epoch.rent.refundable_principal)
                .ok_or(Error::ArithmeticOverflow)?
        || observation.sink_after
            != observation
                .sink_before
                .checked_add(donation)
                .ok_or(Error::ArithmeticOverflow)?
    {
        return Err(Error::ConservationFailure);
    }
    Ok(())
}

/// Legacy internal projection retained only while the stable-Replay successor is integrated.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lapse_epoch_v2(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    binding: &FacilityPositionBindingV2,
    epoch: &DealerEpochBindingV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    lapse: &DealerActionLivenessAuthorizationV1,
    position_before: &DealerPositionObservationV3,
    position_after: &DealerPositionObservationV3,
    current_slot: u64,
    rent: DealerEpochCloseRentV2,
) -> Result<DealerStateV2> {
    state.validate_against_policy(policy)?;
    epoch.validate()?;
    lapse.validate_against(schedule, runtime)?;
    let binding_id = binding.binding_id()?;
    position_before.validate_current(state, binding, policy)?;
    position_after.validate_against(binding, binding_id, policy)?;
    let before = position_before.projection.position();
    let after = position_after.projection.position();
    if epoch.phase != DealerEpochBindingPhaseV2::Bound
        || epoch.epoch_binding_account_id != state.active_epoch_binding_account_id
        || epoch.epoch_id != state.active_epoch_id
        || state.children.epoch_bindings != 1
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state_account_id != epoch.dealer_state_account_id
        || current_slot < epoch.lapse_after_slot
        || lapse.action != DealerRuntimeActionV1::LapseEpoch
        || lapse.owner != state_account_id
        || lapse.lifecycle_id != state.facility_id
        || lapse.facility_generation != state.generation
        || after.generation() != state.generation.checked_add(1).ok_or(Error::ArithmeticOverflow)?
        || after.lifecycle() != PositionLifecycleV3::Open
        || before.cash_atoms() != after.cash_atoms()
        || before.reserved_cash_atoms() != after.reserved_cash_atoms()
        || before.native_eggs() != after.native_eggs()
        || before.outstanding_reservations() != after.outstanding_reservations()
        || Id::from_bytes(after.replay_account().bytes())
            == state.facility_replay_account_id
    {
        return Err(Error::MismatchedBinding);
    }
    validate_epoch_close_rent(epoch, rent)?;
    let mut next = *state;
    next.active_epoch_id = Id::ZERO;
    next.active_epoch_binding_account_id = Id::ZERO;
    next.children.epoch_bindings = 0;
    next.facility_position_id = position_after.semantic_id;
    next.facility_replay_account_id = Id::from_bytes(after.replay_account().bytes());
    next.generation = after.generation();
    next.child_sequence = next
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    next.validate_against_policy(policy)?;
    Ok(next)
}

/// Atomic successor for Epoch lapse over one stable ReplayV3 account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerEpochLapseV3 {
    /// State after closing the binding and consuming one generation.
    pub state_after: DealerStateV2,
    /// Replay after the same generation advance.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Lapse an unused Epoch without rotating or lowering the canonical Replay identity.
#[allow(clippy::too_many_arguments)]
pub fn prepare_lapse_epoch_v3(
    policy: &DealerPolicyV1,
    state: &DealerStateV2,
    state_account_id: Id,
    binding: &FacilityPositionBindingV2,
    epoch: &DealerEpochBindingV2,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    lapse: &DealerActionLivenessAuthorizationV1,
    position_before: &DealerPositionObservationV3,
    position_after: &DealerPositionObservationV3,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
    current_slot: u64,
    rent: DealerEpochCloseRentV2,
) -> Result<PreparedDealerEpochLapseV3> {
    state.validate_against_policy(policy)?;
    epoch.validate()?;
    lapse.validate_against(schedule, runtime)?;
    let binding_id = binding.binding_id()?;
    position_before.validate_current(state, binding, policy)?;
    position_after.validate_against(binding, binding_id, policy)?;
    validate_live_replay(state, replay)?;
    let before = position_before.projection.position();
    let after = position_after.projection.position();
    let next_generation = state
        .generation
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if epoch.phase != DealerEpochBindingPhaseV2::Bound
        || epoch.epoch_binding_account_id != state.active_epoch_binding_account_id
        || epoch.epoch_id != state.active_epoch_id
        || state.children.epoch_bindings != 1
        || state.children.leases != 0
        || state.children.settlement_pots != 0
        || state_account_id != epoch.dealer_state_account_id
        || current_slot < epoch.lapse_after_slot
        || lapse.action != DealerRuntimeActionV1::LapseEpoch
        || lapse.owner != state_account_id
        || lapse.lifecycle_id != state.facility_id
        || lapse.facility_generation != state.generation
        || after.generation() != next_generation
        || after.lifecycle() != PositionLifecycleV3::Open
        || before.cash_atoms() != after.cash_atoms()
        || before.reserved_cash_atoms() != after.reserved_cash_atoms()
        || before.native_eggs() != after.native_eggs()
        || before.outstanding_reservations() != after.outstanding_reservations()
        || Id::from_bytes(after.replay_account().bytes()) != state.facility_replay_account_id
    {
        return Err(Error::MismatchedBinding);
    }
    validate_epoch_close_rent(epoch, rent)?;
    let mut state_after = *state;
    state_after.active_epoch_id = Id::ZERO;
    state_after.active_epoch_binding_account_id = Id::ZERO;
    state_after.children.epoch_bindings = 0;
    state_after.facility_position_id = position_after.semantic_id;
    state_after.generation = next_generation;
    state_after.child_sequence = state_after
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    state_after.validate_against_policy(policy)?;
    let empty = DealerEmptyAssetTransferBundleV1 {
        action: DealerRuntimeActionV1::LapseEpoch,
    };
    let prepared = replay.prepare_transition(
        replay_binding,
        DealerTransitionIntentV1 {
            replay_account_id: replay.replay_account_id(),
            replay_pre_id: replay.replay_id()?,
            state_pre_content_id: state.state_content_id()?,
            state_post_content_id: state_after.state_content_id()?,
            position_pre_semantic_id: position_before.semantic_id,
            position_post_semantic_id: position_after.semantic_id,
            liveness_receipt_semantic_id: lapse.receipt_semantic_id,
            fee_receipt_semantic_id: Id::ZERO,
            asset_transfer_bundle_id: empty.bundle_id()?,
            position_generation_before: state.generation,
            position_generation_after: next_generation,
            expected_ordinal: replay.next_transition_ordinal(),
            action: DealerRuntimeActionV1::LapseEpoch,
            liveness_mode: DealerTransitionLivenessModeV1::ExternalReceipt,
        },
    )?;
    Ok(PreparedDealerEpochLapseV3 {
        state_after,
        replay: prepared,
    })
}

fn validate_live_replay(state: &DealerStateV2, replay: &DealerFacilityReplayV1) -> Result<()> {
    replay.validate()?;
    if replay.facility_position_account_id() != state.facility_position_account_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_binding_id() != state.facility_position_binding_id
        || replay.position_generation() != state.generation
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(())
}

/// Close a leased Epoch after the Lease/Pot pair has atomically terminated.
pub fn retire_epoch_after_lease_v2(
    policy: &DealerPolicyV1,
    state_after_lease: &DealerStateV2,
    epoch: &DealerEpochBindingV2,
    lease: &DealerLeaseV2,
    rent: DealerEpochCloseRentV2,
) -> Result<DealerStateV2> {
    state_after_lease.validate_against_policy(policy)?;
    epoch.validate()?;
    lease.validate()?;
    if epoch.phase != DealerEpochBindingPhaseV2::Leased
        || epoch.epoch_binding_account_id != state_after_lease.active_epoch_binding_account_id
        || epoch.epoch_id != state_after_lease.active_epoch_id
        || epoch.active_lease_account_id != lease.lease_account_id
        || epoch.settlement_candidate_id != lease.settlement_candidate_id
        || state_after_lease.children.epoch_bindings != 1
        || state_after_lease.children.leases != 0
        || state_after_lease.children.settlement_pots != 0
        || !state_after_lease.active_lease_id.is_zero()
        || state_after_lease.generation != lease.post_generation
    {
        return Err(Error::InvalidChildGraph);
    }
    validate_epoch_close_rent(epoch, rent)?;
    let mut next = *state_after_lease;
    next.active_epoch_id = Id::ZERO;
    next.active_epoch_binding_account_id = Id::ZERO;
    next.children.epoch_bindings = 0;
    next.child_sequence = next
        .child_sequence
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    next.validate_against_policy(policy)?;
    Ok(next)
}

const _: () = assert!(DEALER_EPOCH_BINDING_BYTES_V2 == 764);
const _: () = assert!(DEALER_EPOCH_BINDING_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
