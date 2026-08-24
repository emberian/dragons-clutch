// SPDX-License-Identifier: AGPL-3.0-or-later

//! Current Direct lifecycle wrappers over the frozen transition arithmetic.
//!
//! The V1 transition core remains the sole owner of phase, counter, replay,
//! ranking, and exact Reservation arithmetic.  Its root projection is private,
//! total, and one-way: callers authenticate current b1/v2 state, the wrapper
//! checks every projected input supplied to the V1 core, and the successor is
//! reconstructed and revalidated as b1/v2 before it can leave this module.
//! No V1 Product or General authority is exposed or persisted by this API.

use clutch_batch::direct_pair_v1::DirectEconomicCandidateV1;
use clutch_batch::relation_v1::FrozenPolicyV1;
use clutch_batch::relation_v2::{EconomicDomainV2, PricePreconditionV2};
use clutch_batch::{PartialPolicy, Side};
use clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2;
use clutch_general_v2_contract::GeneralPositionReplayPrestateV1;
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};

use crate::current_v2::{
    direct_foundation_root_semantic_id_v2, DirectMarketBindingV2, DirectMarketRootV2,
    DirectRootReplayPostV2,
};
use crate::fee_v1::DirectFeeTerminalV1;
use crate::fee_v2::DirectFeePolicyV2;
use crate::liveness_v1::{
    bind_direct_candidate_work_batch_v1, prepare_direct_candidate_work_batch_v1,
    DirectCandidateWorkBatchV1,
};
use crate::reservation_v1::{
    AuthenticatedDirectReservationAdmissionV1, DirectReservationV1,
};
use crate::selection_v1::{
    begin_direct_candidate_verification_v1, finalize_direct_selection_v1,
    prepare_direct_selection_freeze_v1, submit_direct_candidate_v1,
    verify_next_direct_candidate_v1, AuthenticatedDirectSelectionFreezeV1,
    DirectCandidateBondMovementV1, DirectCandidateBondRefundPlanV1, DirectSelectionV1,
};
use crate::settlement_v1::{
    prepare_direct_economic_terminal_from_current_projection_v2,
    prepare_direct_missed_freeze_terminal_v1,
    prepare_direct_reservation_admission_with_replay_v1,
    prepare_direct_reservation_cancel_v1, AuthenticatedDirectReservationCancelV1,
    AuthenticatedDirectEconomicTerminalV1, DirectEndpointPrestateV1,
    DirectEndpointTerminalPlanV1, DirectFeeTreasuryPlanV1,
    DirectFeeTreasuryPrestateV1, DirectReservationOrderInputV1,
    DirectSettlementHashBackendV1,
};
use crate::{
    DirectActionReplayV1, DirectHashBackendV1, DirectMarketActionV1,
    DirectMarketErrorV1, DirectRentOwnerV1, DirectReplayPhaseV1,
    DirectRetirementTransferV1, DirectRootReplayPostV1, DirectScheduleV1,
    DirectTerminalReasonV1,
};

const FOUNDATION_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/direct/foundation-receipt/v2\0";
const ACTION_TRANSCRIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/direct/action-transcript/v2\0";
const TERMINAL_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/direct/terminal-receipt/v2\0";
const TERMINAL_LIVENESS_SEAL_DOMAIN_V2: &[u8] =
    b"dragons-clutch/direct/terminal-liveness-seal/v2\0";

/// Default-deny current action-1 authority.
pub trait AuthenticatedDirectFoundationV2 {
    /// Authenticate the absent b1/v2 and b3 accounts, exact current Product
    /// family admission, General V4/Revenue authority, 0xba/v2 allocation,
    /// rent funding, schedule policy, and observed Clock slot.
    fn authenticate_foundation_v2(
        &self,
        _binding: &DirectMarketBindingV2,
        _schedule: DirectScheduleV1,
        _root_rent: DirectRentOwnerV1,
        _action_replay_rent: DirectRentOwnerV1,
        _observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing current action-1 authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectFoundationAuthorityV2;

impl AuthenticatedDirectFoundationV2 for NoDirectFoundationAuthorityV2 {}

/// Compact current action-1 receipt. The 2.5KiB root and permanent replay are
/// streamed into caller-provided bodies and never returned by value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFoundationReceiptV2 {
    /// Sole current Direct foundation receipt.
    pub admission_receipt_id: [u8; 32],
    /// Exact semantic identity of the streamed fresh b1/v2 root.
    pub root_semantic_id: [u8; 32],
    /// Exact semantic identity of the streamed permanent b3 replay.
    pub replay_semantic_id: [u8; 32],
    /// Product family successor authenticated during the same outer call.
    pub product_family_poststate_id: [u8; 32],
    /// Product family admission receipt authenticated during the same outer call.
    pub product_family_admission_receipt_id: [u8; 32],
    /// Exact 0xba/v2 Candidate allocation receipt.
    pub candidate_liveness_allocation_receipt_id: [u8; 32],
}

/// Prepare fresh current action 1 directly into caller-owned semantic bodies.
///
/// The buffers exclude their Solana tag/version/bump headers. They must be
/// committed atomically with the Product family and `0xba/v2` successors
/// authenticated by `authority`.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_foundation_into_v2<
    A: AuthenticatedDirectFoundationV2 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A,
    binding: &DirectMarketBindingV2,
    schedule: DirectScheduleV1,
    root_rent: DirectRentOwnerV1,
    action_replay_rent: DirectRentOwnerV1,
    observed_slot: u64,
    root_body_out: &mut [u8],
    replay_body_out: &mut [u8; crate::codec_v1::DIRECT_ACTION_REPLAY_BODY_BYTES_V1],
    backend: &B,
) -> Result<DirectFoundationReceiptV2, DirectMarketErrorV1> {
    binding.validate()?;
    schedule.validate()?;
    root_rent.validate()?;
    action_replay_rent.validate()?;
    if schedule != DirectScheduleV1::canonical_from_foundation_slot(observed_slot)?
        || binding.expected_direct_epoch_semantics_id(schedule, backend)?
            != binding.direct_epoch_semantics_id
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    authority.authenticate_foundation_v2(
        binding,
        schedule,
        root_rent,
        action_replay_rent,
        observed_slot,
    )?;
    let binding_id = binding.semantic_id(backend)?;
    let root_id = direct_foundation_root_semantic_id_v2(
        binding,
        schedule,
        root_rent,
        backend,
    )?;
    let admission_receipt_id = backend.sha256_parts(&[
        FOUNDATION_RECEIPT_DOMAIN_V2,
        &binding_id,
        &root_id,
        &binding.product.product_family_prestate_id,
        &binding.product.product_family_poststate_id,
        &binding.product.product_family_admission_receipt_id,
        &binding.product.family_admission_sequence.to_le_bytes(),
        &binding.product.product_direct_global_liveness_binding_id,
        &binding.product.product_direct_global_liveness_activation_id,
        &binding.product.activated_product_market_binding_id,
        &binding.product.direct_work_quote_id,
        &binding.candidate_liveness.allocation_receipt_id,
        &binding.general.general_market_binding_v4_data_id,
        &binding.general.general_market_runtime_data_id,
        &binding.general.revenue_policy_record_v2_id,
        &binding.general.treasury_position_derivation_policy_v2_id,
        &observed_slot.to_le_bytes(),
        &root_rent.payer,
        &root_rent.principal_lamports.to_le_bytes(),
        &root_rent.donation_floor_lamports.to_le_bytes(),
        &action_replay_rent.payer,
        &action_replay_rent.principal_lamports.to_le_bytes(),
        &action_replay_rent.donation_floor_lamports.to_le_bytes(),
    ]);
    crate::require_live(admission_receipt_id)?;
    let initial_transcript = backend.sha256_parts(&[
        ACTION_TRANSCRIPT_DOMAIN_V2,
        &binding.market_instance_id,
        &binding.direct_root_account,
        &root_id,
        &0u64.to_le_bytes(),
        &[DirectMarketActionV1::InitializeMarket.byte()],
        &[0; 32],
        &[0; 32],
        &observed_slot.to_le_bytes(),
        &admission_receipt_id,
        &[0; 32],
    ]);
    crate::require_live(initial_transcript)?;
    let replay = DirectActionReplayV1 {
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        direct_epoch_semantics_id: binding.direct_epoch_semantics_id,
        direct_root_account: binding.direct_root_account,
        replay_account: binding.action_replay_account,
        rent: action_replay_rent,
        phase: DirectReplayPhaseV1::Active,
        next_action_sequence: 1,
        action_transcript_id: initial_transcript,
        foundation_receipt_id: admission_receipt_id,
        economic_terminal_receipt_id: [0; 32],
        family_terminal_receipt_id: [0; 32],
        candidate_liveness_completed_calls: 0,
        candidate_liveness_last_receipt_id: [0; 32],
        candidate_liveness_batch_receipt_id: [0; 32],
        candidate_liveness_pending: false,
    };
    let projected_root = crate::DirectMarketRootV1 {
        binding: binding.transition_projection(backend)?,
        schedule,
        root_rent,
        phase: crate::DirectRootPhaseV1::Open,
        terminal_reason: None,
        admitted_reservations: 0,
        live_reservations: 0,
        retired_reservations: 0,
        reservation_accounts: [[0; 32]; 2],
        reservation_semantic_ids: [[0; 32]; 2],
        selection_account: [0; 32],
    };
    projected_root.validate()?;
    replay.validate_against(projected_root)?;
    let replay_semantic_id = replay.semantic_id(projected_root, backend)?;
    crate::codec_v2::encode_direct_market_foundation_body_v2(
        binding,
        schedule,
        root_rent,
        root_body_out,
    )?;
    *replay_body_out = crate::codec_v1::encode_direct_action_replay_body_v1(
        replay,
        projected_root,
    )?;
    Ok(DirectFoundationReceiptV2 {
        admission_receipt_id,
        root_semantic_id: root_id,
        replay_semantic_id,
        product_family_poststate_id: binding.product.product_family_poststate_id,
        product_family_admission_receipt_id:
            binding.product.product_family_admission_receipt_id,
        candidate_liveness_allocation_receipt_id:
            binding.candidate_liveness.allocation_receipt_id,
    })
}

/// Derive one exact Candidate work batch from current b1/v2 state.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_candidate_work_batch_v2<B: DirectHashBackendV1>(
    state: &DirectRootReplayPostV2,
    selection: Option<&DirectSelectionV1>,
    action: DirectMarketActionV1,
    candidate_completed_calls: u32,
    candidate_last_receipt_id: [u8; 32],
    candidate_pre_data_id: [u8; 32],
    keeper: [u8; 32],
    backend: &B,
) -> Result<DirectCandidateWorkBatchV1, DirectMarketErrorV1> {
    prepare_direct_candidate_work_batch_v1(
        state.transition_projection(backend)?,
        selection,
        action,
        candidate_completed_calls,
        candidate_last_receipt_id,
        candidate_pre_data_id,
        keeper,
        backend,
    )
}

/// Bind one hostile-reopened Candidate batch and return only current state.
pub fn bind_direct_candidate_work_batch_v2<B: DirectHashBackendV1>(
    state: &mut DirectRootReplayPostV2,
    batch: DirectCandidateWorkBatchV1,
    backend: &B,
) -> Result<(), DirectMarketErrorV1> {
    let projection = state.transition_projection(backend)?;
    let replay = bind_direct_candidate_work_batch_v1(&projection, batch, backend)?;
    replay.validate_against(projection.root)?;
    state.replay = replay;
    state.validate(backend)
}

/// Default-deny current action-2 account authority.
pub trait AuthenticatedDirectReservationAdmissionV2 {
    /// Authenticate current root/replay, Position/Replay, optional peer,
    /// fresh Reservation account, funding, action sequence, and clock slot.
    fn authenticate_admission_v2(
        &self,
        _state: &DirectRootReplayPostV2,
        _position_replay: GeneralPositionReplayPrestateV1,
        _existing_peer: Option<DirectReservationV1>,
        _consumed_sequence: u64,
        _observed_slot: u64,
        _order: DirectReservationOrderInputV1,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing current action-2 authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectReservationAdmissionAuthorityV2;

impl AuthenticatedDirectReservationAdmissionV2 for NoDirectReservationAdmissionAuthorityV2 {}

/// Complete current action-2 plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectReservationAdmissionPlanV2 {
    /// Fresh exact Reservation.
    pub reservation: DirectReservationV1,
    /// Position successor.
    pub position_poststate: PositionSettlementPoststateV3,
    /// GEN1 successor.
    pub replay_transition: clutch_general_v2_contract::GeneralReplayTransitionPlanV1,
    /// Noncircular transition commitment retained by GEN1.
    pub transition_id: [u8; 32],
    /// Direct action receipt retained by permanent replay.
    pub admission_receipt_id: [u8; 32],
}

/// Prepare current action 2 without exposing the private V1 projection.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_reservation_admission_v2<
    A: AuthenticatedDirectReservationAdmissionV2 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    authority: &A,
    state: &mut DirectRootReplayPostV2,
    position_replay: GeneralPositionReplayPrestateV1,
    existing_peer: Option<DirectReservationV1>,
    consumed_sequence: u64,
    observed_slot: u64,
    order: DirectReservationOrderInputV1,
    backend: &B,
) -> Result<DirectReservationAdmissionPlanV2, DirectMarketErrorV1> {
    authority.authenticate_admission_v2(
        state,
        position_replay,
        existing_peer,
        consumed_sequence,
        observed_slot,
        order,
    )?;
    let projection = state.transition_projection(backend)?;
    let projection_authority = ProjectedAdmissionAuthorityV2 {
        root: projection.root,
        position: position_replay.position(),
        existing_peer,
        order,
    };
    let plan = prepare_direct_reservation_admission_with_replay_v1(
        &projection_authority,
        projection,
        position_replay,
        existing_peer,
        consumed_sequence,
        observed_slot,
        order,
        backend,
    )?;
    state.accept_transition_projection_in_place(projection, plan.state, backend)?;
    Ok(DirectReservationAdmissionPlanV2 {
        reservation: plan.reservation,
        position_poststate: plan.position_poststate,
        replay_transition: plan.replay_transition,
        transition_id: plan.transition_id,
        admission_receipt_id: plan.admission_receipt_id,
    })
}

#[derive(Clone, Copy, Debug)]
struct ProjectedAdmissionAuthorityV2 {
    root: crate::DirectMarketRootV1,
    position: AuthenticatedPositionV3,
    existing_peer: Option<DirectReservationV1>,
    order: DirectReservationOrderInputV1,
}

impl AuthenticatedDirectReservationAdmissionV1 for ProjectedAdmissionAuthorityV2 {
    fn authenticate_admission(
        &self,
        root: crate::DirectMarketRootV1,
        position: AuthenticatedPositionV3,
        existing_peer: Option<DirectReservationV1>,
        reservation_account: [u8; 32],
        order_id: [u8; 32],
        side: Side,
        outcome: u8,
        quantity: u64,
        minimum_fill: u64,
        partial_policy: PartialPolicy,
        expiry_epoch: u64,
        limit_price_units_per_egg: u128,
        rent: DirectRentOwnerV1,
    ) -> Result<(), DirectMarketErrorV1> {
        let expected = self.order;
        if root == self.root
            && position == self.position
            && existing_peer == self.existing_peer
            && reservation_account == expected.reservation_account
            && order_id == expected.order_id
            && side == expected.side
            && outcome == expected.outcome
            && quantity == expected.quantity
            && minimum_fill == expected.minimum_fill
            && partial_policy == expected.partial_policy
            && expiry_epoch == expected.expiry_epoch
            && limit_price_units_per_egg == expected.limit_price_units_per_egg
            && rent == expected.rent
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Default-deny current action-3 account authority.
pub trait AuthenticatedDirectReservationCancelV2 {
    /// Authenticate current root/replay and the exact atomic close plane.
    fn authenticate_cancel_v2(
        &self,
        _state: &DirectRootReplayPostV2,
        _reservation: DirectReservationV1,
        _position_replay: GeneralPositionReplayPrestateV1,
        _observed_reservation_lamports: u64,
        _consumed_sequence: u64,
        _observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing current action-3 authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectReservationCancelAuthorityV2;

impl AuthenticatedDirectReservationCancelV2 for NoDirectReservationCancelAuthorityV2 {}

/// Complete current action-3 plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectReservationCancelPlanV2 {
    /// Terminal Reservation/Position/GEN1 endpoint.
    pub endpoint: DirectEndpointTerminalPlanV1,
    /// Exact one-source principal/surplus transfer vector.
    pub retirement: DirectRetirementTransferV1,
    /// Identity of the transfer vector.
    pub retirement_transfer_id: [u8; 32],
    /// Sole action-3 retirement receipt.
    pub retirement_receipt_id: [u8; 32],
}

/// Prepare current action 3 without exposing the private V1 projection.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_reservation_cancel_v2<
    A: AuthenticatedDirectReservationCancelV2 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    authority: &A,
    state: &mut DirectRootReplayPostV2,
    reservation: DirectReservationV1,
    position_replay: GeneralPositionReplayPrestateV1,
    observed_reservation_lamports: u64,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectReservationCancelPlanV2, DirectMarketErrorV1> {
    authority.authenticate_cancel_v2(
        state,
        reservation,
        position_replay,
        observed_reservation_lamports,
        consumed_sequence,
        observed_slot,
    )?;
    let projection = state.transition_projection(backend)?;
    let projection_authority = ProjectedCancelAuthorityV2 {
        state: projection,
        reservation,
        position_replay,
        observed_reservation_lamports,
        consumed_sequence,
        observed_slot,
    };
    let plan = prepare_direct_reservation_cancel_v1(
        &projection_authority,
        projection,
        reservation,
        position_replay,
        observed_reservation_lamports,
        consumed_sequence,
        observed_slot,
        backend,
    )?;
    state.accept_transition_projection_in_place(projection, plan.state, backend)?;
    Ok(DirectReservationCancelPlanV2 {
        endpoint: plan.endpoint,
        retirement: plan.retirement,
        retirement_transfer_id: plan.retirement_transfer_id,
        retirement_receipt_id: plan.retirement_receipt_id,
    })
}

#[derive(Clone, Copy, Debug)]
struct ProjectedCancelAuthorityV2 {
    state: DirectRootReplayPostV1,
    reservation: DirectReservationV1,
    position_replay: GeneralPositionReplayPrestateV1,
    observed_reservation_lamports: u64,
    consumed_sequence: u64,
    observed_slot: u64,
}

impl AuthenticatedDirectReservationCancelV1 for ProjectedCancelAuthorityV2 {
    fn authenticate_cancel(
        &self,
        state: DirectRootReplayPostV1,
        reservation: DirectReservationV1,
        position_replay: GeneralPositionReplayPrestateV1,
        observed_reservation_lamports: u64,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state == self.state
            && reservation == self.reservation
            && position_replay == self.position_replay
            && observed_reservation_lamports == self.observed_reservation_lamports
            && consumed_sequence == self.consumed_sequence
            && observed_slot == self.observed_slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Default-deny current action-4 complete-book authority.
pub trait AuthenticatedDirectSelectionFreezeV2 {
    /// Authenticate current root/replay, exact live Reservation prefix, fresh
    /// Selection, immutable RelationV2 domain/price, rent, sequence, and slot.
    #[allow(clippy::too_many_arguments)]
    fn authenticate_freeze_v2(
        &self,
        _state: &DirectRootReplayPostV2,
        _selection_account: [u8; 32],
        _rent: DirectRentOwnerV1,
        _reservations: &[Option<DirectReservationV1>; 2],
        _domain: &EconomicDomainV2,
        _price: &PricePreconditionV2,
        _consumed_sequence: u64,
        _observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing current action-4 authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectSelectionFreezeAuthorityV2;

impl AuthenticatedDirectSelectionFreezeV2 for NoDirectSelectionFreezeAuthorityV2 {}

/// Current action 4..8 Selection plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectSelectionPlanV2 {
    /// Selection successor.
    pub selection: DirectSelectionV1,
    /// Exact retained-candidate principal movement for action 5.
    pub candidate_bond_movement: Option<DirectCandidateBondMovementV1>,
    /// Complete candidate principal refund vector for action 8.
    pub candidate_bond_refunds: Option<DirectCandidateBondRefundPlanV1>,
}

/// Prepare current action 4 and freeze the complete live Reservation prefix.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_selection_freeze_v2<
    A: AuthenticatedDirectSelectionFreezeV2 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A,
    state: &mut DirectRootReplayPostV2,
    consumed_sequence: u64,
    observed_slot: u64,
    selection_account: [u8; 32],
    rent: DirectRentOwnerV1,
    reservations: [Option<DirectReservationV1>; 2],
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    backend: &B,
) -> Result<DirectSelectionPlanV2, DirectMarketErrorV1> {
    authority.authenticate_freeze_v2(
        state,
        selection_account,
        rent,
        &reservations,
        &domain,
        &price,
        consumed_sequence,
        observed_slot,
    )?;
    let projection = state.transition_projection(backend)?;
    let mut reservation_semantic_ids = [[0; 32]; 2];
    let mut index = 0usize;
    while index < usize::from(projection.root.live_reservations()) {
        reservation_semantic_ids[index] = reservations[index]
            .ok_or(DirectMarketErrorV1::InvalidCount)?
            .semantic_id(backend)?;
        index += 1;
    }
    let projection_authority = ProjectedFreezeAuthorityV2 {
        root: projection.root,
        selection_account,
        rent,
        reservations,
        reservation_semantic_ids,
        domain,
        price,
    };
    let plan = prepare_direct_selection_freeze_v1(
        &projection_authority,
        projection,
        consumed_sequence,
        observed_slot,
        selection_account,
        rent,
        reservations,
        domain,
        price,
        backend,
    )?;
    convert_selection_plan_v2(state, projection, plan, backend)
}

#[derive(Clone, Copy, Debug)]
struct ProjectedFreezeAuthorityV2 {
    root: crate::DirectMarketRootV1,
    selection_account: [u8; 32],
    rent: DirectRentOwnerV1,
    reservations: [Option<DirectReservationV1>; 2],
    reservation_semantic_ids: [[u8; 32]; 2],
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
}

impl AuthenticatedDirectSelectionFreezeV1 for ProjectedFreezeAuthorityV2 {
    fn authenticate_freeze(
        &self,
        root: crate::DirectMarketRootV1,
        selection_account: [u8; 32],
        rent: DirectRentOwnerV1,
        reservations: &[Option<DirectReservationV1>; 2],
        reservation_semantic_ids: &[[u8; 32]; 2],
        domain: &EconomicDomainV2,
        price: &PricePreconditionV2,
    ) -> Result<(), DirectMarketErrorV1> {
        if root == self.root
            && selection_account == self.selection_account
            && rent == self.rent
            && reservations == &self.reservations
            && reservation_semantic_ids == &self.reservation_semantic_ids
            && domain == &self.domain
            && price == &self.price
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Prepare current action 5 and retain only the canonical best-three prefix.
pub fn submit_direct_candidate_v2<B: DirectHashBackendV1>(
    state: &mut DirectRootReplayPostV2,
    selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    candidate: DirectEconomicCandidateV1,
    submitter: [u8; 32],
    backend: &B,
) -> Result<DirectSelectionPlanV2, DirectMarketErrorV1> {
    let projection = state.transition_projection(backend)?;
    let plan = submit_direct_candidate_v1(
        projection,
        selection,
        consumed_sequence,
        observed_slot,
        candidate,
        submitter,
        backend,
    )?;
    convert_selection_plan_v2(state, projection, plan, backend)
}

/// Prepare current action 6's exhaustive verification traversal.
pub fn begin_direct_candidate_verification_v2<B: DirectHashBackendV1>(
    state: &mut DirectRootReplayPostV2,
    selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectSelectionPlanV2, DirectMarketErrorV1> {
    let projection = state.transition_projection(backend)?;
    let plan = begin_direct_candidate_verification_v1(
        projection,
        selection,
        consumed_sequence,
        observed_slot,
        backend,
    )?;
    convert_selection_plan_v2(state, projection, plan, backend)
}

/// Prepare current action 7 for exactly the next retained candidate.
pub fn verify_next_direct_candidate_v2<B: DirectHashBackendV1>(
    state: &mut DirectRootReplayPostV2,
    selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectSelectionPlanV2, DirectMarketErrorV1> {
    let projection = state.transition_projection(backend)?;
    let plan = verify_next_direct_candidate_v1(
        projection,
        selection,
        consumed_sequence,
        observed_slot,
        backend,
    )?;
    convert_selection_plan_v2(state, projection, plan, backend)
}

/// Prepare current action 8 and select the best valid submitted candidate.
pub fn finalize_direct_selection_v2<B: DirectHashBackendV1>(
    state: &mut DirectRootReplayPostV2,
    selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectSelectionPlanV2, DirectMarketErrorV1> {
    let projection = state.transition_projection(backend)?;
    let plan = finalize_direct_selection_v1(
        projection,
        selection,
        consumed_sequence,
        observed_slot,
        backend,
    )?;
    convert_selection_plan_v2(state, projection, plan, backend)
}

fn convert_selection_plan_v2<B: DirectHashBackendV1>(
    state: &mut DirectRootReplayPostV2,
    projection: DirectRootReplayPostV1,
    plan: crate::selection_v1::DirectSelectionFreezePlanV1,
    backend: &B,
) -> Result<DirectSelectionPlanV2, DirectMarketErrorV1> {
    state.accept_transition_projection_in_place(projection, plan.state, backend)?;
    Ok(DirectSelectionPlanV2 {
        selection: plan.selection,
        candidate_bond_movement: plan.candidate_bond_movement,
        candidate_bond_refunds: plan.candidate_bond_refunds,
    })
}

/// Default-deny current action-9..12 account authority.
pub trait AuthenticatedDirectEconomicTerminalV2 {
    /// Authenticate the exact current root/replay, Selection, canonical
    /// Reservation/Position/Replay prefix, optional RevenuePolicyV2 treasury
    /// plane, terminal reason, action sequence, and clock slot.
    #[allow(clippy::too_many_arguments)]
    fn authenticate_terminal_v2(
        &self,
        _state: &DirectRootReplayPostV2,
        _selection: DirectSelectionV1,
        _ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        _fee_policy: DirectFeePolicyV2,
        _realm: [u8; 32],
        _batch_policy: Option<&FrozenPolicyV1>,
        _revenue_policy: Option<&RevenuePolicyV2>,
        _fee_terminal: Option<DirectFeeTerminalV1>,
        _treasury: Option<DirectFeeTreasuryPrestateV1>,
        _reason: DirectTerminalReasonV1,
        _consumed_sequence: u64,
        _observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing current action-9..12 authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectEconomicTerminalAuthorityV2;

impl AuthenticatedDirectEconomicTerminalV2 for NoDirectEconomicTerminalAuthorityV2 {}

/// Atomic current action-9..12 poststate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectEconomicTerminalPlanV2 {
    /// Terminal Selection archive.
    pub selection: DirectSelectionV1,
    /// Canonical Selection-order endpoint prefix.
    pub endpoints: [Option<DirectEndpointTerminalPlanV1>; 2],
    /// Exact assessed fee and split, present only for action 9.
    pub fee_terminal: Option<DirectFeeTerminalV1>,
    /// Exact nonzero current treasury credit, present only when needed.
    pub treasury: Option<DirectFeeTreasuryPlanV1>,
    /// Exact active endpoint count derived from Selection.
    pub endpoint_count: u8,
    /// Noncircular common transition commitment.
    pub transition_id: [u8; 32],
    /// Sole permanent economic terminal receipt.
    pub economic_terminal_receipt_id: [u8; 32],
    /// Complete retained-candidate principal refunds, when still pending.
    pub candidate_bond_refunds: Option<DirectCandidateBondRefundPlanV1>,
}

/// Prepare current action 9..12 under RevenuePolicyV2-native authority.
///
/// Action 9 supplies both immutable policy preimages and the canonical
/// treasury Position/Replay prestate. Lapse actions must supply none of them.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_economic_terminal_v2<
    A: AuthenticatedDirectEconomicTerminalV2 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    authority: &A,
    state: &mut DirectRootReplayPostV2,
    selection: DirectSelectionV1,
    endpoints: [Option<DirectEndpointPrestateV1>; 2],
    realm: [u8; 32],
    batch_policy: Option<&FrozenPolicyV1>,
    revenue_policy: Option<&RevenuePolicyV2>,
    treasury: Option<DirectFeeTreasuryPrestateV1>,
    reason: DirectTerminalReasonV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectEconomicTerminalPlanV2, DirectMarketErrorV1> {
    let fee_policy = state.root.binding().fee_policy();
    let projection = state.transition_projection(backend)?;
    let projection_authority = ProjectedEconomicAuthorityV2 {
        authority,
        current_state: state,
        projected_state: projection,
        fee_policy,
        realm,
        batch_policy,
        revenue_policy,
        treasury,
        reason,
        consumed_sequence,
        observed_slot,
    };
    let plan = prepare_direct_economic_terminal_from_current_projection_v2(
        &projection_authority,
        projection,
        selection,
        endpoints,
        fee_policy,
        realm,
        batch_policy,
        revenue_policy,
        treasury,
        reason,
        consumed_sequence,
        observed_slot,
        backend,
    )?;
    drop(projection_authority);
    convert_economic_plan_v2(state, projection, plan, backend)
}

/// Prepare the action-10 missed-freeze lapse from a still-open current root.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_missed_freeze_terminal_v2<
    F: AuthenticatedDirectSelectionFreezeV2 + ?Sized,
    A: AuthenticatedDirectEconomicTerminalV2 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    freeze_authority: &F,
    terminal_authority: &A,
    state: &mut DirectRootReplayPostV2,
    selection_account: [u8; 32],
    selection_rent: DirectRentOwnerV1,
    reservations: [Option<DirectReservationV1>; 2],
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    endpoints: [Option<DirectEndpointPrestateV1>; 2],
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectEconomicTerminalPlanV2, DirectMarketErrorV1> {
    freeze_authority.authenticate_freeze_v2(
        state,
        selection_account,
        selection_rent,
        &reservations,
        &domain,
        &price,
        consumed_sequence,
        observed_slot,
    )?;
    let fee_policy = state.root.binding().fee_policy();
    let projection = state.transition_projection(backend)?;
    let mut reservation_semantic_ids = [[0; 32]; 2];
    let mut index = 0usize;
    while index < usize::from(projection.root.live_reservations()) {
        reservation_semantic_ids[index] = reservations[index]
            .ok_or(DirectMarketErrorV1::InvalidCount)?
            .semantic_id(backend)?;
        index += 1;
    }
    let projected_freeze = ProjectedFreezeAuthorityV2 {
        root: projection.root,
        selection_account,
        rent: selection_rent,
        reservations,
        reservation_semantic_ids,
        domain,
        price,
    };
    let realm = state.root.binding().realm_id;
    let projected_terminal = ProjectedEconomicAuthorityV2 {
        authority: terminal_authority,
        current_state: state,
        projected_state: projection,
        fee_policy,
        realm,
        batch_policy: None,
        revenue_policy: None,
        treasury: None,
        reason: DirectTerminalReasonV1::MissedFreezeLapse,
        consumed_sequence,
        observed_slot,
    };
    let plan = prepare_direct_missed_freeze_terminal_v1(
        &projected_freeze,
        &projected_terminal,
        projection,
        selection_account,
        selection_rent,
        reservations,
        domain,
        price,
        endpoints,
        None,
        None,
        consumed_sequence,
        observed_slot,
        backend,
    )?;
    drop(projected_terminal);
    convert_economic_plan_v2(state, projection, plan, backend)
}

#[derive(Debug)]
struct ProjectedEconomicAuthorityV2<'a, A: ?Sized> {
    authority: &'a A,
    current_state: &'a DirectRootReplayPostV2,
    projected_state: DirectRootReplayPostV1,
    fee_policy: DirectFeePolicyV2,
    realm: [u8; 32],
    batch_policy: Option<&'a FrozenPolicyV1>,
    revenue_policy: Option<&'a RevenuePolicyV2>,
    treasury: Option<DirectFeeTreasuryPrestateV1>,
    reason: DirectTerminalReasonV1,
    consumed_sequence: u64,
    observed_slot: u64,
}

impl<A: AuthenticatedDirectEconomicTerminalV2 + ?Sized>
    AuthenticatedDirectEconomicTerminalV1 for ProjectedEconomicAuthorityV2<'_, A>
{
    fn authenticate_terminal(
        &self,
        state: DirectRootReplayPostV1,
        selection: DirectSelectionV1,
        ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        fee_terminal: Option<DirectFeeTerminalV1>,
        treasury: Option<DirectFeeTreasuryPrestateV1>,
        reason: DirectTerminalReasonV1,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state != self.projected_state
            || treasury != self.treasury
            || reason != self.reason
            || consumed_sequence != self.consumed_sequence
            || observed_slot != self.observed_slot
        {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        self.authority.authenticate_terminal_v2(
            self.current_state,
            selection,
            ordered_endpoints,
            self.fee_policy,
            self.realm,
            self.batch_policy,
            self.revenue_policy,
            fee_terminal,
            treasury,
            reason,
            consumed_sequence,
            observed_slot,
        )
    }
}

fn convert_economic_plan_v2<B: DirectHashBackendV1>(
    state: &mut DirectRootReplayPostV2,
    projection: DirectRootReplayPostV1,
    plan: crate::settlement_v1::DirectEconomicTerminalPlanV1,
    backend: &B,
) -> Result<DirectEconomicTerminalPlanV2, DirectMarketErrorV1> {
    state.accept_transition_projection_in_place(projection, plan.state, backend)?;
    Ok(DirectEconomicTerminalPlanV2 {
        selection: plan.selection,
        endpoints: plan.endpoints,
        fee_terminal: plan.fee_terminal,
        treasury: plan.treasury,
        endpoint_count: plan.endpoint_count,
        transition_id: plan.transition_id,
        economic_terminal_receipt_id: plan.economic_terminal_receipt_id,
        candidate_bond_refunds: plan.candidate_bond_refunds,
    })
}

/// Default-deny current action-13 Product/archive authority.
pub trait AuthenticatedDirectTerminalV2 {
    /// Authenticate the complete current Product RootV2/LinkV2/family
    /// prestate, finalized Resolution, b1/v2/b2/b3/b4 deletion set, transfer
    /// vector, and exact Product family successor identities.
    #[allow(clippy::too_many_arguments)]
    fn authenticate_terminal_v2(
        &self,
        _state: &DirectRootReplayPostV2,
        _root_semantic_id: [u8; 32],
        _replay_semantic_id: [u8; 32],
        _selection: &DirectSelectionV1,
        _reservations: &[Option<DirectReservationV1>; 2],
        _final_resolution: crate::DirectFinalResolutionV1,
        _retirement: &DirectRetirementTransferV1,
        _retirement_transfer_id: [u8; 32],
        _product_family_prestate_id: [u8; 32],
        _product_family_poststate_id: [u8; 32],
        _consumed_sequence: u64,
        _observed_slot: u64,
        _family_terminal_sequence: u32,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing current action-13 authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectTerminalAuthorityV2;

impl AuthenticatedDirectTerminalV2 for NoDirectTerminalAuthorityV2 {}

/// Provisional current action-13 transition awaiting the exact eighth
/// Candidate work receipt. It is never accepted by Product or persisted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectFamilyTerminalPreparationV2 {
    root_semantic_id: [u8; 32],
    replay_pre_semantic_id: [u8; 32],
    retirement_transfer_id: [u8; 32],
    final_resolution: crate::DirectFinalResolutionV1,
    replay_post: DirectActionReplayV1,
    provisional_terminal_receipt_id: [u8; 32],
    product_family_prestate_id: [u8; 32],
    product_family_poststate_id: [u8; 32],
    family_terminal_sequence: u32,
}

impl DirectFamilyTerminalPreparationV2 {
    /// Current provisional state used only by the exact Candidate work binder.
    pub fn prepared_state(&self, root: DirectMarketRootV2) -> DirectRootReplayPostV2 {
        DirectRootReplayPostV2 {
            root,
            replay: self.replay_post,
        }
    }

    /// Provisional replay used only as input to the final Candidate batch.
    pub const fn prepared_replay(&self) -> &DirectActionReplayV1 {
        &self.replay_post
    }
}

/// Final sealed current Direct terminal facts consumed by Product and 0xba/v2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectFamilyTerminalPlanV2 {
    /// Exact b1/v2 root identity before deletion.
    pub root_semantic_id: [u8; 32],
    /// Permanent b3 identity before action 13.
    pub replay_pre_semantic_id: [u8; 32],
    /// Canonical complete source/refund/surplus vector.
    pub retirement: DirectRetirementTransferV1,
    /// Identity of the complete transfer vector.
    pub retirement_transfer_id: [u8; 32],
    /// Exact finalized current Resolution joined only at family retirement.
    pub final_resolution: crate::DirectFinalResolutionV1,
    /// Terminal replay after the exact eighth Candidate work batch.
    pub replay_post: DirectActionReplayV1,
    /// Sole current terminal receipt consumed by Product.
    pub terminal_receipt_id: [u8; 32],
    /// Authenticated Product family prestate identity.
    pub product_family_prestate_id: [u8; 32],
    /// Exact Product family successor identity.
    pub product_family_poststate_id: [u8; 32],
    /// Zero-based terminal occurrence coordinate.
    pub family_terminal_sequence: u32,
}

/// Prepare current action 13 before its final Candidate work batch.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_family_terminal_v2<
    A: AuthenticatedDirectTerminalV2 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A,
    state: &DirectRootReplayPostV2,
    selection: &DirectSelectionV1,
    reservations: &[Option<DirectReservationV1>; 2],
    final_resolution: crate::DirectFinalResolutionV1,
    retirement: &DirectRetirementTransferV1,
    product_family_prestate_id: [u8; 32],
    product_family_poststate_id: [u8; 32],
    consumed_sequence: u64,
    observed_slot: u64,
    family_terminal_sequence: u32,
    backend: &B,
) -> Result<DirectFamilyTerminalPreparationV2, DirectMarketErrorV1> {
    state.validate(backend)?;
    let projection = state.transition_projection(backend)?;
    state.replay.require_action(projection.root, consumed_sequence)?;
    selection.validate_against(projection.root)?;
    retirement.validate()?;
    for id in [
        final_resolution.account,
        final_resolution.semantic_id,
        final_resolution.data_id,
        product_family_prestate_id,
        product_family_poststate_id,
    ] {
        crate::require_live(id)?;
    }
    if state.root.phase() != crate::DirectRootPhaseV1::Terminal
        || final_resolution.account != state.root.binding().resolution_account
        || product_family_prestate_id == product_family_poststate_id
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let expected_sources = usize::from(state.root.live_reservations())
        .checked_add(3)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let ordered_reservations = crate::canonical_terminal_reservation_archives(
        &projection.root,
        selection,
        reservations,
        backend,
    )?;
    if retirement.neutral_lamport_sink != state.root.binding().neutral_lamport_sink
        || usize::from(retirement.source_count) != expected_sources
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    crate::require_terminal_retirement_source_v1(
        retirement,
        state.root.binding().direct_root_account,
        state.root.root_rent(),
    )?;
    crate::require_terminal_retirement_source_v1(
        retirement,
        state.root.binding().action_replay_account,
        state.replay.rent(),
    )?;
    crate::require_terminal_retirement_source_v1(
        retirement,
        state.root.selection_account(),
        selection.rent(),
    )?;
    let mut reservation_ids = [[0; 32]; 2];
    let mut index = 0usize;
    while index < usize::from(state.root.live_reservations()) {
        let reservation = ordered_reservations[index]
            .ok_or(DirectMarketErrorV1::InvalidCount)?;
        crate::require_terminal_retirement_source_v1(
            retirement,
            reservation.account(),
            reservation.rent(),
        )?;
        reservation_ids[index] = reservation.semantic_id(backend)?;
        index += 1;
    }
    let root_semantic_id = state.root.semantic_id(backend)?;
    let replay_pre_semantic_id = state.replay.semantic_id(projection.root, backend)?;
    let selection_semantic_id = selection.semantic_id(projection.root, backend)?;
    let retirement_transfer_id = retirement.semantic_id(backend)?;
    authority.authenticate_terminal_v2(
        state,
        root_semantic_id,
        replay_pre_semantic_id,
        selection,
        &ordered_reservations,
        final_resolution,
        retirement,
        retirement_transfer_id,
        product_family_prestate_id,
        product_family_poststate_id,
        consumed_sequence,
        observed_slot,
        family_terminal_sequence,
    )?;
    let replay_with_action = state.replay.advance(
        root_semantic_id,
        root_semantic_id,
        DirectMarketActionV1::RetireTerminal,
        observed_slot,
        retirement_transfer_id,
        backend,
    )?;
    let provisional_terminal_receipt_id = backend.sha256_parts(&[
        TERMINAL_RECEIPT_DOMAIN_V2,
        &product_family_prestate_id,
        &product_family_poststate_id,
        &state.root.binding().market_instance_id,
        &state.root.binding().generation.to_le_bytes(),
        &state.root.binding().direct_root_account,
        &state.root.binding().action_replay_account,
        &root_semantic_id,
        &state.root.binding().product.product_market_binding_id,
        &state.root.binding().product.series_link_v2_id,
        &state.root.binding().product.compiler_bundle_v6_id,
        &state.root.binding().product.attachment_plan_v5_id,
        &state.root.binding().product.product_direct_global_liveness_binding_id,
        &state.root.binding().product.product_direct_global_liveness_activation_id,
        &state.root.binding().product.direct_work_quote_id,
        &final_resolution.account,
        &final_resolution.semantic_id,
        &final_resolution.data_id,
        &selection_semantic_id,
        &reservation_ids[0],
        &reservation_ids[1],
        &replay_pre_semantic_id,
        &replay_with_action.action_transcript_id(),
        &state.replay.economic_terminal_receipt_id(),
        &retirement_transfer_id,
        &consumed_sequence.to_le_bytes(),
        &observed_slot.to_le_bytes(),
        &family_terminal_sequence.to_le_bytes(),
    ]);
    crate::require_live(provisional_terminal_receipt_id)?;
    let mut replay_post = replay_with_action;
    replay_post.phase = DirectReplayPhaseV1::Terminal;
    replay_post.family_terminal_receipt_id = provisional_terminal_receipt_id;
    replay_post.validate_against(projection.root)?;
    Ok(DirectFamilyTerminalPreparationV2 {
        root_semantic_id,
        replay_pre_semantic_id,
        retirement_transfer_id,
        final_resolution,
        replay_post,
        provisional_terminal_receipt_id,
        product_family_prestate_id,
        product_family_poststate_id,
        family_terminal_sequence,
    })
}

/// Seal action 13 only after the exact eighth Candidate work receipt is bound.
pub fn seal_direct_family_terminal_liveness_v2<B: DirectHashBackendV1>(
    preparation: DirectFamilyTerminalPreparationV2,
    root: &DirectMarketRootV2,
    retirement: &DirectRetirementTransferV1,
    final_resolution: crate::DirectFinalResolutionV1,
    bound_replay: DirectActionReplayV1,
    backend: &B,
) -> Result<DirectFamilyTerminalPlanV2, DirectMarketErrorV1> {
    let projected_root = root.transition_projection(backend)?;
    let provisional = preparation.replay_post;
    if root.semantic_id(backend)? != preparation.root_semantic_id
        || retirement.semantic_id(backend)? != preparation.retirement_transfer_id
        || final_resolution != preparation.final_resolution
        || !provisional.candidate_liveness_pending()
        || bound_replay.candidate_liveness_pending()
        || provisional.market_instance_id != bound_replay.market_instance_id
        || provisional.generation != bound_replay.generation
        || provisional.direct_epoch_semantics_id
            != bound_replay.direct_epoch_semantics_id
        || provisional.direct_root_account != bound_replay.direct_root_account
        || provisional.replay_account != bound_replay.replay_account
        || provisional.rent() != bound_replay.rent()
        || provisional.phase() != bound_replay.phase()
        || provisional.next_action_sequence() != bound_replay.next_action_sequence()
        || provisional.foundation_receipt_id() != bound_replay.foundation_receipt_id()
        || provisional.economic_terminal_receipt_id()
            != bound_replay.economic_terminal_receipt_id()
        || provisional.family_terminal_receipt_id()
            != bound_replay.family_terminal_receipt_id()
        || provisional.family_terminal_receipt_id()
            != preparation.provisional_terminal_receipt_id
        || provisional.candidate_liveness_completed_calls() != 7
        || bound_replay.candidate_liveness_completed_calls() != 8
        || bound_replay.candidate_liveness_last_receipt_id() == [0; 32]
        || bound_replay.candidate_liveness_batch_receipt_id() == [0; 32]
    {
        return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
    }
    let expected_transcript = backend.sha256_parts(&[
        crate::REPLAY_LIVENESS_BATCH_DOMAIN_V1,
        &provisional.market_instance_id,
        &provisional.direct_root_account,
        &provisional.action_transcript_id(),
        &bound_replay.candidate_liveness_completed_calls().to_le_bytes(),
        &bound_replay.candidate_liveness_last_receipt_id(),
        &bound_replay.candidate_liveness_batch_receipt_id(),
    ]);
    if bound_replay.action_transcript_id() != expected_transcript {
        return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
    }
    bound_replay.validate_against(projected_root)?;
    let terminal_receipt_id = backend.sha256_parts(&[
        TERMINAL_LIVENESS_SEAL_DOMAIN_V2,
        &preparation.provisional_terminal_receipt_id,
        &preparation.product_family_prestate_id,
        &preparation.product_family_poststate_id,
        &bound_replay.action_transcript_id(),
        &bound_replay.candidate_liveness_completed_calls().to_le_bytes(),
        &bound_replay.candidate_liveness_last_receipt_id(),
        &bound_replay.candidate_liveness_batch_receipt_id(),
    ]);
    crate::require_live(terminal_receipt_id)?;
    let mut replay_post = bound_replay;
    replay_post.family_terminal_receipt_id = terminal_receipt_id;
    replay_post.validate_against(projected_root)?;
    Ok(DirectFamilyTerminalPlanV2 {
        root_semantic_id: preparation.root_semantic_id,
        replay_pre_semantic_id: preparation.replay_pre_semantic_id,
        retirement: *retirement,
        retirement_transfer_id: preparation.retirement_transfer_id,
        final_resolution,
        replay_post,
        terminal_receipt_id,
        product_family_prestate_id: preparation.product_family_prestate_id,
        product_family_poststate_id: preparation.product_family_poststate_id,
        family_terminal_sequence: preparation.family_terminal_sequence,
    })
}

const _: () = assert!(core::mem::size_of::<DirectFoundationReceiptV2>() <= 224);
const _: () = assert!(core::mem::size_of::<DirectFamilyTerminalPreparationV2>() <= 768);
const _: () = assert!(core::mem::size_of::<DirectFamilyTerminalPlanV2>() <= 1_024);
