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
use clutch_batch::relation_v2::{EconomicDomainV2, PricePreconditionV2};
use clutch_batch::{PartialPolicy, Side};
use clutch_general_v2_contract::GeneralPositionReplayPrestateV1;
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};

use crate::current_v2::DirectRootReplayPostV2;
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
    prepare_direct_reservation_admission_with_replay_v1,
    prepare_direct_reservation_cancel_v1, AuthenticatedDirectReservationCancelV1,
    DirectEndpointTerminalPlanV1, DirectReservationOrderInputV1,
    DirectSettlementHashBackendV1,
};
use crate::{
    DirectHashBackendV1, DirectMarketErrorV1, DirectRentOwnerV1,
    DirectRetirementTransferV1, DirectRootReplayPostV1,
};

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
    /// Current b1/v2 root and permanent b3 replay successor.
    pub state: DirectRootReplayPostV2,
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
    state: DirectRootReplayPostV2,
    position_replay: GeneralPositionReplayPrestateV1,
    existing_peer: Option<DirectReservationV1>,
    consumed_sequence: u64,
    observed_slot: u64,
    order: DirectReservationOrderInputV1,
    backend: &B,
) -> Result<DirectReservationAdmissionPlanV2, DirectMarketErrorV1> {
    authority.authenticate_admission_v2(
        &state,
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
    let current_state = state.accept_transition_projection(projection, plan.state, backend)?;
    Ok(DirectReservationAdmissionPlanV2 {
        state: current_state,
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
    /// Current b1/v2 root and permanent replay successor.
    pub state: DirectRootReplayPostV2,
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
    state: DirectRootReplayPostV2,
    reservation: DirectReservationV1,
    position_replay: GeneralPositionReplayPrestateV1,
    observed_reservation_lamports: u64,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectReservationCancelPlanV2, DirectMarketErrorV1> {
    authority.authenticate_cancel_v2(
        &state,
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
    let current_state = state.accept_transition_projection(projection, plan.state, backend)?;
    Ok(DirectReservationCancelPlanV2 {
        state: current_state,
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
    /// Current b1/v2 root and permanent replay successor.
    pub state: DirectRootReplayPostV2,
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
    state: DirectRootReplayPostV2,
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
        &state,
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
    state: DirectRootReplayPostV2,
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
    state: DirectRootReplayPostV2,
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
    state: DirectRootReplayPostV2,
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
    state: DirectRootReplayPostV2,
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
    state: DirectRootReplayPostV2,
    projection: DirectRootReplayPostV1,
    plan: crate::selection_v1::DirectSelectionFreezePlanV1,
    backend: &B,
) -> Result<DirectSelectionPlanV2, DirectMarketErrorV1> {
    let current_state = state.accept_transition_projection(projection, plan.state, backend)?;
    Ok(DirectSelectionPlanV2 {
        state: current_state,
        selection: plan.selection,
        candidate_bond_movement: plan.candidate_bond_movement,
        candidate_bond_refunds: plan.candidate_bond_refunds,
    })
}
