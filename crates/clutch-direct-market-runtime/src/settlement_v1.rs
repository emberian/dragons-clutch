// SPDX-License-Identifier: AGPL-3.0-or-later

//! Atomic Direct Reservation, PositionV3, GEN1, and terminal transitions.

use clutch_batch::{PartialPolicy, Side};
use clutch_batch_policy_identity::revenue_policy_v1::RevenuePolicyV1;
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, project_general_replay_transition_v1,
    GeneralPositionReplayPrestateV1, GeneralReplayTransitionKindV1,
    GeneralReplayTransitionPlanV1, Id32,
};
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{
    PositionAccountV3, PositionPurposeV3, PositionV3Fields, PositionV3Sha256Backend,
    ReplayV3HashBackend,
};

use crate::reservation_v1::{
    prepare_direct_reservation_admission_v1, AuthenticatedDirectReservationAdmissionV1,
    DirectReservationPhaseV1, DirectReservationV1,
};
use crate::fee_v1::DirectFeeTerminalV1;
use crate::selection_v1::{
    build_direct_selection_v1, AuthenticatedDirectSelectionFreezeV1,
    DirectSelectionPhaseV1, DirectSelectionV1,
};
use crate::{
    require_live, DirectHashBackendV1, DirectMarketErrorV1, DirectPrincipalRefundV1,
    DirectRentOwnerV1,
    DirectRetirementSourceV1, DirectRetirementTransferV1, DirectRootPhaseV1,
    DirectRootReplayPostV1, DirectTerminalReasonV1,
};

const ADMISSION_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/admission-transition/v1\0";
const ADMISSION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/admission-receipt/v1\0";
const RESERVATION_CANCEL_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/reservation-cancel-transition/v1\0";
const RESERVATION_CANCEL_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/reservation-cancel-receipt/v1\0";
const ECONOMIC_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/economic-transition/v1\0";
const ECONOMIC_TERMINAL_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/economic-terminal-receipt/v1\0";

/// Backend required by the Direct/Position/Replay atomic composers.
pub trait DirectSettlementHashBackendV1:
    DirectHashBackendV1 + PositionV3Sha256Backend + ReplayV3HashBackend
{
}

impl<T> DirectSettlementHashBackendV1 for T where
    T: DirectHashBackendV1 + PositionV3Sha256Backend + ReplayV3HashBackend
{
}

/// Fixed admission coordinates supplied by the strict action-2 decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationOrderInputV1 {
    /// Fresh `0xb4/1` Reservation PDA.
    pub reservation_account: [u8; 32],
    /// Canonical RelationV2 order identity.
    pub order_id: [u8; 32],
    /// Buy or sell side.
    pub side: Side,
    /// Scalar native-Egg outcome.
    pub outcome: u8,
    /// Maximum order units.
    pub quantity: u64,
    /// Smallest accepted nonzero fill.
    pub minimum_fill: u64,
    /// Partial or all-or-none policy.
    pub partial_policy: PartialPolicy,
    /// Last eligible Direct generation.
    pub expiry_epoch: u64,
    /// Exact price-unit limit per Egg.
    pub limit_price_units_per_egg: u128,
    /// Persisted Reservation rent ownership.
    pub rent: DirectRentOwnerV1,
}

/// Complete action-2 pure plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationAdmissionWithReplayPlanV1 {
    /// Root and permanent Direct replay successor.
    pub state: DirectRootReplayPostV1,
    /// Fresh exact Reservation.
    pub reservation: DirectReservationV1,
    /// Position successor.
    pub position_poststate: PositionSettlementPoststateV3,
    /// GEN1 successor under the fresh family-80 action-2 role.
    pub replay_transition: GeneralReplayTransitionPlanV1,
    /// Noncircular transition commitment retained by GEN1.
    pub transition_id: [u8; 32],
    /// Direct action receipt retained by permanent replay.
    pub admission_receipt_id: [u8; 32],
}

/// Prepare action 2 across Reservation, PositionV3, GEN1, root, and replay.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_reservation_admission_with_replay_v1<
    A: AuthenticatedDirectReservationAdmissionV1 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    authority: &A,
    state: DirectRootReplayPostV1,
    position_replay: GeneralPositionReplayPrestateV1,
    existing_peer: Option<DirectReservationV1>,
    consumed_sequence: u64,
    observed_slot: u64,
    order: DirectReservationOrderInputV1,
    backend: &B,
) -> Result<DirectReservationAdmissionWithReplayPlanV1, DirectMarketErrorV1> {
    state.replay.validate_against(state.root)?;
    let admission = prepare_direct_reservation_admission_v1(
        authority,
        state.root,
        position_replay.position(),
        existing_peer,
        order.reservation_account,
        order.order_id,
        order.side,
        order.outcome,
        order.quantity,
        order.minimum_fill,
        order.partial_policy,
        order.expiry_epoch,
        order.limit_price_units_per_egg,
        order.rent,
        backend,
    )?;
    require_position_replay_binding(admission.reservation, position_replay)?;
    let reservation_id = admission.reservation.semantic_id(backend)?;
    let position_post_id = position_semantic_id(&admission.position_poststate, backend)?;
    let root_pre_id = state.root.semantic_id(backend)?;
    let transition_id = DirectHashBackendV1::sha256_parts(backend, &[
        ADMISSION_TRANSITION_DOMAIN_V1,
        &state.root.binding().market_instance_id,
        &state.root.binding().generation.to_le_bytes(),
        &root_pre_id,
        &reservation_id,
        &position_replay.position().semantic_id,
        &position_post_id,
        &position_replay.replay_semantic_id().bytes(),
        &consumed_sequence.to_le_bytes(),
        &observed_slot.to_le_bytes(),
    ]);
    require_live(transition_id)?;
    let kind = admission_kind(order.side);
    let replay_transition = project_general_replay_transition_v1(
        position_replay,
        admission.position_poststate,
        kind,
        Id32::new(transition_id)?,
        Id32::new(reservation_id)?,
        backend,
    )?;
    let admission_receipt_id = DirectHashBackendV1::sha256_parts(backend, &[
        ADMISSION_RECEIPT_DOMAIN_V1,
        &transition_id,
        &reservation_id,
        &replay_transition.position_prestate_semantic_id().bytes(),
        &replay_transition.position_poststate_semantic_id().bytes(),
        &replay_transition.replay_prestate_semantic_id().bytes(),
        &replay_transition.replay_poststate_semantic_id().bytes(),
        &replay_transition.delta_id().bytes(),
    ]);
    require_live(admission_receipt_id)?;
    let state = state.admit_reservation(
        consumed_sequence,
        observed_slot,
        admission.reservation.account(),
        reservation_id,
        admission_receipt_id,
        backend,
    )?;
    Ok(DirectReservationAdmissionWithReplayPlanV1 {
        state,
        reservation: admission.reservation,
        position_poststate: admission.position_poststate,
        replay_transition,
        transition_id,
        admission_receipt_id,
    })
}

/// Default-deny action-3 close authentication boundary.
pub trait AuthenticatedDirectReservationCancelV1 {
    /// Authenticate controller authority, exact writable root/replay,
    /// Reservation/Position/GEN1 accounts, observed Reservation balance, payer
    /// refund account, and Realm neutral sink.
    fn authenticate_cancel(
        &self,
        _state: DirectRootReplayPostV1,
        _reservation: DirectReservationV1,
        _position_replay: GeneralPositionReplayPrestateV1,
        _observed_reservation_lamports: u64,
        _consumed_sequence: u64,
        _observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing cancel authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectReservationCancelAuthorityV1;

impl AuthenticatedDirectReservationCancelV1 for NoDirectReservationCancelAuthorityV1 {}

/// One Reservation/Position/GEN1 terminal endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEndpointTerminalPlanV1 {
    /// Exact Reservation semantic ID before terminalization.
    pub reservation_pre_id: [u8; 32],
    /// Terminal Reservation archive; action 3 deletes it immediately.
    pub reservation_post: DirectReservationV1,
    /// Exact terminal Reservation semantic ID.
    pub reservation_post_id: [u8; 32],
    /// Exact Position poststate.
    pub position_poststate: PositionSettlementPoststateV3,
    /// Exact GEN1 transition.
    pub replay_transition: GeneralReplayTransitionPlanV1,
}

/// Complete action-3 close plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReservationCancelPlanV1 {
    /// Root and permanent Direct replay successor.
    pub state: DirectRootReplayPostV1,
    /// Terminal semantic endpoint before account deletion.
    pub endpoint: DirectEndpointTerminalPlanV1,
    /// Exact one-source principal/surplus transfer vector.
    pub retirement: DirectRetirementTransferV1,
    /// Identity of the transfer vector.
    pub retirement_transfer_id: [u8; 32],
    /// Sole action-3 retirement receipt.
    pub retirement_receipt_id: [u8; 32],
}

/// Cancel one active Reservation and close its account atomically.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_reservation_cancel_v1<
    A: AuthenticatedDirectReservationCancelV1 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    authority: &A,
    state: DirectRootReplayPostV1,
    reservation: DirectReservationV1,
    position_replay: GeneralPositionReplayPrestateV1,
    observed_reservation_lamports: u64,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectReservationCancelPlanV1, DirectMarketErrorV1> {
    state.replay.validate_against(state.root)?;
    reservation.validate()?;
    require_position_replay_binding(reservation, position_replay)?;
    if state.root.phase() != DirectRootPhaseV1::Open
        || reservation.phase() != DirectReservationPhaseV1::Active
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    authority.authenticate_cancel(
        state,
        reservation,
        position_replay,
        observed_reservation_lamports,
        consumed_sequence,
        observed_slot,
    )?;
    let reservation_pre_id = reservation.semantic_id(backend)?;
    let retirement = one_source_retirement(
        reservation,
        observed_reservation_lamports,
        state.root.binding().neutral_lamport_sink,
    )?;
    let retirement_transfer_id = retirement.semantic_id(backend)?;
    let transition_id = DirectHashBackendV1::sha256_parts(backend, &[
        RESERVATION_CANCEL_TRANSITION_DOMAIN_V1,
        &state.root.binding().market_instance_id,
        &state.root.binding().generation.to_le_bytes(),
        &reservation_pre_id,
        &position_replay.position().semantic_id,
        &position_replay.replay_semantic_id().bytes(),
        &retirement_transfer_id,
        &consumed_sequence.to_le_bytes(),
        &observed_slot.to_le_bytes(),
    ]);
    require_live(transition_id)?;
    let endpoint = project_terminal_endpoint(
        reservation,
        position_replay,
        EndpointEffectV1::Release,
        DirectReservationPhaseV1::Cancelled,
        transition_id,
        reservation_pre_id,
        cancel_kind(reservation.side()),
        backend,
    )?;
    let retirement_receipt_id = DirectHashBackendV1::sha256_parts(backend, &[
        RESERVATION_CANCEL_RECEIPT_DOMAIN_V1,
        &transition_id,
        &reservation_pre_id,
        &endpoint.reservation_post_id,
        &endpoint.replay_transition.position_prestate_semantic_id().bytes(),
        &endpoint.replay_transition.position_poststate_semantic_id().bytes(),
        &endpoint.replay_transition.replay_prestate_semantic_id().bytes(),
        &endpoint.replay_transition.replay_poststate_semantic_id().bytes(),
        &endpoint.replay_transition.delta_id().bytes(),
        &retirement_transfer_id,
    ]);
    require_live(retirement_receipt_id)?;
    let state = state.cancel_reservation(
        consumed_sequence,
        observed_slot,
        reservation.account(),
        reservation_pre_id,
        retirement_receipt_id,
        backend,
    )?;
    Ok(DirectReservationCancelPlanV1 {
        state,
        endpoint,
        retirement,
        retirement_transfer_id,
        retirement_receipt_id,
    })
}

/// Exact endpoint supplied to action 9..12.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEndpointPrestateV1 {
    /// Exact active Reservation.
    pub reservation: DirectReservationV1,
    /// Exact writable PositionV3 and GEN1 prestate.
    pub position_replay: GeneralPositionReplayPrestateV1,
}

/// Exact writable treasury Position/GEN1 prestate, present only when the
/// authenticated terminal fee split credits nonzero treasury atoms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeeTreasuryPrestateV1 {
    /// Canonical General Position and Replay prestate for the revenue owner.
    pub position_replay: GeneralPositionReplayPrestateV1,
}

/// Exact treasury Position/GEN1 successor for action 9.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeeTreasuryPlanV1 {
    /// Exact Position cash successor.
    pub position_poststate: PositionSettlementPoststateV3,
    /// Exact purpose Replay successor under the action-9 treasury role.
    pub replay_transition: GeneralReplayTransitionPlanV1,
}

/// Default-deny economic terminal authentication boundary.
pub trait AuthenticatedDirectEconomicTerminalV1 {
    /// Authenticate the exact writable root/replay/Selection and complete
    /// Reservation/Position/GEN1 account set. Counts are derived from Selection,
    /// never from a payload field.
    fn authenticate_terminal(
        &self,
        _state: DirectRootReplayPostV1,
        _selection: DirectSelectionV1,
        _ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        _fee_terminal: Option<DirectFeeTerminalV1>,
        _treasury: Option<DirectFeeTreasuryPrestateV1>,
        _reason: DirectTerminalReasonV1,
        _consumed_sequence: u64,
        _observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing economic terminal authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectEconomicTerminalAuthorityV1;

impl AuthenticatedDirectEconomicTerminalV1 for NoDirectEconomicTerminalAuthorityV1 {}

/// Atomic action 9..12 poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectEconomicTerminalPlanV1 {
    /// Terminal root and permanent Direct replay.
    pub state: DirectRootReplayPostV1,
    /// Terminal Selection archive.
    pub selection: DirectSelectionV1,
    /// Canonical Selection-order endpoint prefix.
    pub endpoints: [Option<DirectEndpointTerminalPlanV1>; 2],
    /// Exact assessed fee and recipient split, present only for action 9.
    pub fee_terminal: Option<DirectFeeTerminalV1>,
    /// Exact nonzero treasury credit; absent for zero-fee and lapse terminals.
    pub treasury: Option<DirectFeeTreasuryPlanV1>,
    /// Exact active endpoint count derived from Selection.
    pub endpoint_count: u8,
    /// Noncircular common transition committed by both Reservations and GEN1.
    pub transition_id: [u8; 32],
    /// Sole permanent economic terminal receipt.
    pub economic_terminal_receipt_id: [u8; 32],
}

/// Settle or lapse the complete Selection-owned Reservation prefix.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_economic_terminal_v1<
    A: AuthenticatedDirectEconomicTerminalV1 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    authority: &A,
    state: DirectRootReplayPostV1,
    selection: DirectSelectionV1,
    endpoints: [Option<DirectEndpointPrestateV1>; 2],
    revenue_policy: Option<&RevenuePolicyV1>,
    treasury: Option<DirectFeeTreasuryPrestateV1>,
    reason: DirectTerminalReasonV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectEconomicTerminalPlanV1, DirectMarketErrorV1> {
    prepare_direct_economic_terminal_from_projection_v1(
        authority,
        state,
        state.root,
        selection,
        endpoints,
        revenue_policy,
        treasury,
        reason,
        consumed_sequence,
        observed_slot,
        backend,
    )
}

/// Atomically create the canonical Selection and lapse an open root which
/// missed the submission-close freeze boundary. Reservation and endpoint
/// counts remain derived from the root-owned exact live prefix.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_missed_freeze_terminal_v1<
    F: AuthenticatedDirectSelectionFreezeV1 + ?Sized,
    A: AuthenticatedDirectEconomicTerminalV1 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    freeze_authority: &F,
    terminal_authority: &A,
    state: DirectRootReplayPostV1,
    selection_account: [u8; 32],
    selection_rent: DirectRentOwnerV1,
    reservations: [Option<DirectReservationV1>; 2],
    domain: clutch_batch::relation_v2::EconomicDomainV2,
    price: clutch_batch::relation_v2::PricePreconditionV2,
    endpoints: [Option<DirectEndpointPrestateV1>; 2],
    revenue_policy: Option<&RevenuePolicyV1>,
    treasury: Option<DirectFeeTreasuryPrestateV1>,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectEconomicTerminalPlanV1, DirectMarketErrorV1> {
    state.replay.validate_against(state.root)?;
    if observed_slot < state.root.schedule().submission_closes_slot {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let (selection, selection_validation_root) = build_direct_selection_v1(
        freeze_authority,
        state.root,
        selection_account,
        selection_rent,
        reservations,
        domain,
        price,
        backend,
    )?;
    prepare_direct_economic_terminal_from_projection_v1(
        terminal_authority,
        state,
        selection_validation_root,
        selection,
        endpoints,
        revenue_policy,
        treasury,
        DirectTerminalReasonV1::MissedFreezeLapse,
        consumed_sequence,
        observed_slot,
        backend,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_direct_economic_terminal_from_projection_v1<
    A: AuthenticatedDirectEconomicTerminalV1 + ?Sized,
    B: DirectSettlementHashBackendV1,
>(
    authority: &A,
    state: DirectRootReplayPostV1,
    selection_validation_root: crate::DirectMarketRootV1,
    selection: DirectSelectionV1,
    endpoints: [Option<DirectEndpointPrestateV1>; 2],
    revenue_policy: Option<&RevenuePolicyV1>,
    treasury: Option<DirectFeeTreasuryPrestateV1>,
    reason: DirectTerminalReasonV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectEconomicTerminalPlanV1, DirectMarketErrorV1> {
    state.replay.validate_against(state.root)?;
    selection.validate_against(selection_validation_root)?;
    require_terminal_phase(selection, reason)?;
    let ordered = canonical_terminal_endpoints(selection, endpoints, backend)?;
    let (fee_terminal, treasury_prestate) = prepare_terminal_fee(
        state.root,
        selection,
        &ordered,
        revenue_policy,
        treasury,
        reason,
    )?;
    authority.authenticate_terminal(
        state,
        selection,
        &ordered,
        fee_terminal,
        treasury,
        reason,
        consumed_sequence,
        observed_slot,
    )?;
    let endpoint_count = selection.reservation_count();
    let selection_pre_id = selection.semantic_id(selection_validation_root, backend)?;
    let mut reservation_pre_ids = [[0u8; 32]; 2];
    let mut position_pre_ids = [[0u8; 32]; 2];
    let mut replay_pre_ids = [[0u8; 32]; 2];
    let mut index = 0usize;
    while index < usize::from(endpoint_count) {
        let endpoint = ordered[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
        reservation_pre_ids[index] = endpoint.reservation.semantic_id(backend)?;
        position_pre_ids[index] = endpoint.position_replay.position().semantic_id;
        replay_pre_ids[index] = endpoint.position_replay.replay_semantic_id().bytes();
        index += 1;
    }
    let (treasury_position_pre_id, treasury_replay_pre_id) = match treasury_prestate {
        Some(value) => (
            value.position_replay.position().semantic_id,
            value.position_replay.replay_semantic_id().bytes(),
        ),
        None => ([0; 32], [0; 32]),
    };
    let fee_transcript = fee_terminal.map_or([0u8; 41], DirectFeeTerminalV1::canonical_transcript);
    let selected_transcript = selection
        .selected_pair()
        .map_or([0u8; 253], |pair| pair.canonical_transcript());
    let transition_id = DirectHashBackendV1::sha256_parts(backend, &[
        ECONOMIC_TRANSITION_DOMAIN_V1,
        &state.root.binding().market_instance_id,
        &state.root.binding().generation.to_le_bytes(),
        &state.root.binding().direct_fee_shape_id,
        &[reason.byte()],
        &selection_pre_id,
        &[endpoint_count],
        &reservation_pre_ids[0],
        &reservation_pre_ids[1],
        &position_pre_ids[0],
        &position_pre_ids[1],
        &replay_pre_ids[0],
        &replay_pre_ids[1],
        &treasury_position_pre_id,
        &treasury_replay_pre_id,
        &fee_transcript,
        &selected_transcript,
        &consumed_sequence.to_le_bytes(),
        &observed_slot.to_le_bytes(),
    ]);
    require_live(transition_id)?;

    let root_post_projection = terminal_root_projection(state.root, selection.account(), reason);
    let selection_post = selection.terminalize(root_post_projection, transition_id)?;
    let selection_post_id = selection_post.semantic_id(root_post_projection, backend)?;
    let mut projected = [None; 2];
    let mut previous_alias_plan = None;
    index = 0;
    while index < usize::from(endpoint_count) {
        let endpoint = ordered[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
        let effective_replay = if index != 0 {
            let first_input = ordered[0].ok_or(DirectMarketErrorV1::InvalidCount)?;
            if endpoint.position_replay.position().account
                == first_input.position_replay.position().account
            {
                if endpoint.position_replay != first_input.position_replay {
                    return Err(DirectMarketErrorV1::MismatchedBinding);
                }
                successor_position_replay(
                    first_input.position_replay,
                    previous_alias_plan.ok_or(DirectMarketErrorV1::InvalidCount)?,
                    backend,
                )?
            } else {
                endpoint.position_replay
            }
        } else {
            endpoint.position_replay
        };
        let effect = match reason {
            DirectTerminalReasonV1::Settled => EndpointEffectV1::Settle {
                pair: selection.selected_pair().ok_or(DirectMarketErrorV1::WrongPhase)?,
                fee: fee_terminal.ok_or(DirectMarketErrorV1::MismatchedBinding)?,
            },
            DirectTerminalReasonV1::MissedFreezeLapse
            | DirectTerminalReasonV1::EmptyLapse
            | DirectTerminalReasonV1::NoCandidate
            | DirectTerminalReasonV1::UnselectedLapse
            | DirectTerminalReasonV1::SelectedLapse => EndpointEffectV1::Release,
        };
        let plan = project_terminal_endpoint(
            endpoint.reservation,
            effective_replay,
            effect,
            match reason {
                DirectTerminalReasonV1::Settled => DirectReservationPhaseV1::Settled,
                _ => DirectReservationPhaseV1::Lapsed,
            },
            transition_id,
            reservation_pre_ids[index],
            terminal_kind(reason, endpoint.reservation.side()),
            backend,
        )?;
        if index == 0 {
            previous_alias_plan = Some(plan);
        }
        projected[index] = Some(plan);
        index += 1;
    }
    let treasury_plan = match (treasury_prestate, fee_terminal) {
        (Some(prestate), Some(fee)) => Some(project_treasury_fee_credit(
            state.root,
            prestate,
            fee,
            &ordered,
            &projected,
            transition_id,
            backend,
        )?),
        (None, Some(fee)) if fee.treasury_atoms == 0 => None,
        (None, None) => None,
        _ => return Err(DirectMarketErrorV1::MismatchedBinding),
    };
    let economic_terminal_receipt_id = terminal_receipt_id(
        state,
        selection_pre_id,
        selection_post_id,
        endpoint_count,
        &projected,
        fee_terminal,
        treasury_plan,
        transition_id,
        reason,
        backend,
    )?;
    let state = state.terminalize(
        consumed_sequence,
        observed_slot,
        reason,
        selection.account(),
        economic_terminal_receipt_id,
        backend,
    )?;
    if state.root != root_post_projection {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    Ok(DirectEconomicTerminalPlanV1 {
        state,
        selection: selection_post,
        endpoints: projected,
        fee_terminal,
        treasury: treasury_plan,
        endpoint_count,
        transition_id,
        economic_terminal_receipt_id,
    })
}

#[derive(Clone, Copy, Debug)]
enum EndpointEffectV1 {
    Release,
    Settle {
        pair: clutch_batch::direct_pair_v1::SelectedDirectPairV1,
        fee: DirectFeeTerminalV1,
    },
}

fn project_terminal_endpoint<B: DirectSettlementHashBackendV1>(
    reservation: DirectReservationV1,
    position_replay: GeneralPositionReplayPrestateV1,
    effect: EndpointEffectV1,
    phase: DirectReservationPhaseV1,
    transition_id: [u8; 32],
    evidence_id: [u8; 32],
    kind: GeneralReplayTransitionKindV1,
    backend: &B,
) -> Result<DirectEndpointTerminalPlanV1, DirectMarketErrorV1> {
    reservation.validate()?;
    require_position_replay_binding(reservation, position_replay)?;
    let reservation_pre_id = reservation.semantic_id(backend)?;
    if reservation_pre_id != evidence_id || reservation.phase() != DirectReservationPhaseV1::Active {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let position_poststate = position_effect(reservation, position_replay.position(), effect)?;
    let charged_fee_atoms = match (reservation.side(), effect) {
        (Side::Buy, EndpointEffectV1::Settle { fee, .. }) => fee.charged_fee_atoms,
        _ => 0,
    };
    let reservation_post =
        reservation.terminalize(phase, transition_id, charged_fee_atoms)?;
    let reservation_post_id = reservation_post.semantic_id(backend)?;
    let replay_transition = project_general_replay_transition_v1(
        position_replay,
        position_poststate,
        kind,
        Id32::new(transition_id)?,
        Id32::new(evidence_id)?,
        backend,
    )?;
    Ok(DirectEndpointTerminalPlanV1 {
        reservation_pre_id,
        reservation_post,
        reservation_post_id,
        position_poststate,
        replay_transition,
    })
}

fn position_effect(
    reservation: DirectReservationV1,
    position: AuthenticatedPositionV3,
    effect: EndpointEffectV1,
) -> Result<PositionSettlementPoststateV3, DirectMarketErrorV1> {
    position
        .validate_writable()
        .map_err(|_| DirectMarketErrorV1::InvalidPosition)?;
    let fields = position.semantic.fields();
    let mut cash_atoms = fields.cash_atoms;
    let mut reserved_cash_atoms = fields.reserved_cash_atoms;
    let mut native_eggs = fields.native_eggs;
    match (reservation.side(), effect) {
        (Side::Buy, EndpointEffectV1::Release) => {
            reserved_cash_atoms = reserved_cash_atoms
                .checked_sub(reservation.reserved_cash_atoms())
                .ok_or(DirectMarketErrorV1::InvalidPosition)?;
        }
        (Side::Sell, EndpointEffectV1::Release) => {
            let outcome = usize::from(reservation.outcome());
            native_eggs[outcome] = native_eggs[outcome]
                .checked_add(reservation.reserved_eggs())
                .ok_or(DirectMarketErrorV1::Arithmetic)?;
        }
        (Side::Buy, EndpointEffectV1::Settle { pair, fee }) => {
            require_selected_reservation(reservation, pair)?;
            cash_atoms = cash_atoms
                .checked_sub(pair.consideration_cash_atoms())
                .and_then(|value| value.checked_sub(fee.charged_fee_atoms))
                .ok_or(DirectMarketErrorV1::InvalidPosition)?;
            cash_atoms = cash_atoms
                .checked_add(fee.buyer_rebate_atoms)
                .ok_or(DirectMarketErrorV1::Arithmetic)?;
            reserved_cash_atoms = reserved_cash_atoms
                .checked_sub(reservation.reserved_cash_atoms())
                .ok_or(DirectMarketErrorV1::InvalidPosition)?;
            let outcome = usize::from(pair.outcome());
            native_eggs[outcome] = native_eggs[outcome]
                .checked_add(pair.quantity())
                .ok_or(DirectMarketErrorV1::Arithmetic)?;
        }
        (Side::Sell, EndpointEffectV1::Settle { pair, fee }) => {
            require_selected_reservation(reservation, pair)?;
            cash_atoms = cash_atoms
                .checked_add(pair.consideration_cash_atoms())
                .and_then(|value| value.checked_add(fee.seller_rebate_atoms))
                .ok_or(DirectMarketErrorV1::Arithmetic)?;
            let unfilled = reservation
                .reserved_eggs()
                .checked_sub(pair.quantity())
                .ok_or(DirectMarketErrorV1::MismatchedBinding)?;
            let outcome = usize::from(pair.outcome());
            native_eggs[outcome] = native_eggs[outcome]
                .checked_add(unfilled)
                .ok_or(DirectMarketErrorV1::Arithmetic)?;
        }
    }
    let outstanding_reservations = fields
        .outstanding_reservations
        .checked_sub(1)
        .ok_or(DirectMarketErrorV1::InvalidPosition)?;
    let semantic = PositionAccountV3::new(PositionV3Fields {
        cash_atoms,
        reserved_cash_atoms,
        native_eggs,
        outstanding_reservations,
        ..fields
    })
    .map_err(|_| DirectMarketErrorV1::InvalidPosition)?;
    Ok(PositionSettlementPoststateV3 {
        account: position.account,
        general_market_runtime: position.general_market_runtime,
        prestate_semantic_id: position.semantic_id,
        semantic,
    })
}

fn prepare_terminal_fee(
    root: crate::DirectMarketRootV1,
    selection: DirectSelectionV1,
    ordered: &[Option<DirectEndpointPrestateV1>; 2],
    revenue_policy: Option<&RevenuePolicyV1>,
    treasury: Option<DirectFeeTreasuryPrestateV1>,
    reason: DirectTerminalReasonV1,
) -> Result<
    (Option<DirectFeeTerminalV1>, Option<DirectFeeTreasuryPrestateV1>),
    DirectMarketErrorV1,
> {
    if reason != DirectTerminalReasonV1::Settled {
        if revenue_policy.is_some() || treasury.is_some() {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        return Ok((None, None));
    }
    let pair = selection
        .selected_pair()
        .ok_or(DirectMarketErrorV1::WrongPhase)?;
    let revenue = revenue_policy.ok_or(DirectMarketErrorV1::MismatchedBinding)?;
    if selection.reservation_count() != 2 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let first = ordered[0].ok_or(DirectMarketErrorV1::InvalidCount)?;
    let second = ordered[1].ok_or(DirectMarketErrorV1::InvalidCount)?;
    let (buyer, seller) = match (first.reservation.side(), second.reservation.side()) {
        (Side::Buy, Side::Sell) => (first, second),
        (Side::Sell, Side::Buy) => (second, first),
        _ => return Err(DirectMarketErrorV1::MismatchedBinding),
    };
    let binding = root.binding();
    let fee_policy = binding.fee_policy();
    let expected_maximum = fee_policy.maximum_buyer_fee_atoms(
        buyer.reservation.quantity(),
        binding.outcome_count,
        binding.price_scale,
    )?;
    if buyer.reservation.maximum_fee_atoms() != expected_maximum
        || seller.reservation.maximum_fee_atoms() != 0
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let fee = fee_policy.assess_terminal_buyer(
        pair.quantity(),
        pair.outcome(),
        pair.outcome_count(),
        pair.price_scale(),
        selection.price(),
        buyer.position_replay.position().account,
        seller.position_replay.position().account,
        buyer.reservation.maximum_fee_atoms(),
        revenue,
    )?;
    match (fee.treasury_atoms, treasury) {
        (0, None) => Ok((Some(fee), None)),
        // A fee-bearing policy fixes the treasury account suffix before the
        // selected integer price is evaluated. Authenticate that exact owner
        // even when this particular exact quote assesses zero atoms, but do
        // not manufacture a zero-value Position/Replay mutation.
        (0, Some(prestate)) => {
            require_treasury_position_binding(root, prestate)?;
            Ok((Some(fee), None))
        }
        (_, Some(prestate)) => {
            require_treasury_position_binding(root, prestate)?;
            Ok((Some(fee), Some(prestate)))
        }
        (_, None) => Err(DirectMarketErrorV1::MismatchedBinding),
    }
}

fn require_treasury_position_binding(
    root: crate::DirectMarketRootV1,
    treasury: DirectFeeTreasuryPrestateV1,
) -> Result<(), DirectMarketErrorV1> {
    let binding = root.binding();
    let position = treasury.position_replay.position();
    position
        .validate_writable()
        .map_err(|_| DirectMarketErrorV1::InvalidPosition)?;
    let fields = position.semantic.fields();
    if fields.purpose != PositionPurposeV3::General
        || fields.market_instance_id.bytes() != binding.market_instance_id
        || fields.realm_id.bytes() != binding.realm_id
        || fields.collateral_policy_id.bytes() != binding.collateral_policy_id
        || fields.collateral_release_id.bytes() != binding.collateral_release_id
        || fields.owner.bytes() != binding.fee_treasury_owner
        || fields.purpose_binding_id.bytes() != binding.general_market_runtime
        || fields.replay_account.bytes() != treasury.position_replay.replay_account().bytes()
        || fields.outcome_count != binding.outcome_count
        || position.general_market_runtime != binding.general_market_runtime
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    Ok(())
}

fn project_treasury_fee_credit<B: DirectSettlementHashBackendV1>(
    root: crate::DirectMarketRootV1,
    prestate: DirectFeeTreasuryPrestateV1,
    fee: DirectFeeTerminalV1,
    endpoints: &[Option<DirectEndpointPrestateV1>; 2],
    endpoint_plans: &[Option<DirectEndpointTerminalPlanV1>; 2],
    transition_id: [u8; 32],
    backend: &B,
) -> Result<DirectFeeTreasuryPlanV1, DirectMarketErrorV1> {
    if fee.treasury_atoms == 0 {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    require_treasury_position_binding(root, prestate)?;
    let mut effective = prestate.position_replay;
    let mut index = 0usize;
    while index < 2 {
        if let (Some(endpoint), Some(plan)) = (endpoints[index], endpoint_plans[index]) {
            if endpoint.position_replay.position().account
                == prestate.position_replay.position().account
            {
                if endpoint.position_replay != prestate.position_replay {
                    return Err(DirectMarketErrorV1::MismatchedBinding);
                }
                effective = successor_position_replay(prestate.position_replay, plan, backend)?;
            }
        }
        index += 1;
    }
    let position = effective.position();
    let fields = position.semantic.fields();
    let cash_atoms = fields
        .cash_atoms
        .checked_add(fee.treasury_atoms)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let semantic = PositionAccountV3::new(PositionV3Fields {
        cash_atoms,
        ..fields
    })
    .map_err(|_| DirectMarketErrorV1::InvalidPosition)?;
    let position_poststate = PositionSettlementPoststateV3 {
        account: position.account,
        general_market_runtime: position.general_market_runtime,
        prestate_semantic_id: position.semantic_id,
        semantic,
    };
    let replay_transition = project_general_replay_transition_v1(
        effective,
        position_poststate,
        GeneralReplayTransitionKindV1::DirectMarketSettleTreasury,
        Id32::new(transition_id)?,
        Id32::new(root.binding().direct_fee_shape_id)?,
        backend,
    )?;
    Ok(DirectFeeTreasuryPlanV1 {
        position_poststate,
        replay_transition,
    })
}

fn require_position_replay_binding(
    reservation: DirectReservationV1,
    position_replay: GeneralPositionReplayPrestateV1,
) -> Result<(), DirectMarketErrorV1> {
    let position = position_replay.position();
    position
        .validate_writable()
        .map_err(|_| DirectMarketErrorV1::InvalidPosition)?;
    let fields = position.semantic.fields();
    if reservation.position_account != position.account
        || reservation.position_replay_account != position_replay.replay_account().bytes()
        || reservation.position_replay_account != fields.replay_account.bytes()
        || reservation.position_generation != fields.generation
        || reservation.owner != fields.owner.bytes()
        || reservation.market_instance_id != fields.market_instance_id.bytes()
        || reservation.general_market_runtime != position.general_market_runtime
        || reservation.general_market_runtime != fields.purpose_binding_id.bytes()
        || reservation.outcome_count != fields.outcome_count
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    Ok(())
}

fn require_selected_reservation(
    reservation: DirectReservationV1,
    pair: clutch_batch::direct_pair_v1::SelectedDirectPairV1,
) -> Result<(), DirectMarketErrorV1> {
    let expected_order = match reservation.side() {
        Side::Buy => pair.buy_order_id(),
        Side::Sell => pair.sell_order_id(),
    };
    if reservation.order_id() != expected_order
        || reservation.outcome() != pair.outcome()
        || reservation.outcome_count != pair.outcome_count()
        || reservation.price_scale != pair.price_scale()
        || pair.quantity() > reservation.quantity()
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    Ok(())
}

fn successor_position_replay<B: DirectSettlementHashBackendV1>(
    initial: GeneralPositionReplayPrestateV1,
    prior: DirectEndpointTerminalPlanV1,
    backend: &B,
) -> Result<GeneralPositionReplayPrestateV1, DirectMarketErrorV1> {
    let position_semantic_id = prior
        .replay_transition
        .position_poststate_semantic_id()
        .bytes();
    let position = AuthenticatedPositionV3 {
        account: prior.position_poststate.account,
        general_market_runtime: prior.position_poststate.general_market_runtime,
        semantic: prior.position_poststate.semantic,
        semantic_id: position_semantic_id,
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    project_general_position_replay_prestate_v1(
        initial.replay_account(),
        initial.replay_bump(),
        prior.replay_transition.next_sequence(),
        prior.replay_transition.replay_poststate_body(),
        position,
        backend,
    )
    .map_err(DirectMarketErrorV1::GeneralContract)
}

fn canonical_terminal_endpoints<B: DirectHashBackendV1>(
    selection: DirectSelectionV1,
    endpoints: [Option<DirectEndpointPrestateV1>; 2],
    backend: &B,
) -> Result<[Option<DirectEndpointPrestateV1>; 2], DirectMarketErrorV1> {
    let count = selection.reservation_count();
    let supplied_count = match endpoints {
        [None, None] => 0,
        [Some(_), None] | [None, Some(_)] => 1,
        [Some(_), Some(_)] => 2,
    };
    if u8::try_from(supplied_count).map_err(|_| DirectMarketErrorV1::Arithmetic)? != count {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut ordered = [None; 2];
    let mut supplied = 0usize;
    while supplied < 2 {
        if let Some(endpoint) = endpoints[supplied] {
            let reservation_id = endpoint.reservation.semantic_id(backend)?;
            let mut found = None;
            let mut expected = 0usize;
            while expected < usize::from(count) {
                let expected_index =
                    u8::try_from(expected).map_err(|_| DirectMarketErrorV1::Arithmetic)?;
                if endpoint.reservation.account() == selection.reservation_account(expected_index)?
                    && reservation_id == selection.reservation_semantic_id(expected_index)?
                {
                    found = Some(expected);
                    break;
                }
                expected += 1;
            }
            let found = found.ok_or(DirectMarketErrorV1::MismatchedBinding)?;
            if ordered[found].is_some() {
                return Err(DirectMarketErrorV1::IdentityAlias);
            }
            require_position_replay_binding(endpoint.reservation, endpoint.position_replay)?;
            ordered[found] = Some(endpoint);
        }
        supplied += 1;
    }
    Ok(ordered)
}

fn require_terminal_phase(
    selection: DirectSelectionV1,
    reason: DirectTerminalReasonV1,
) -> Result<(), DirectMarketErrorV1> {
    let correct = match reason {
        DirectTerminalReasonV1::MissedFreezeLapse => matches!(
            selection.phase(),
            DirectSelectionPhaseV1::FrozenEmpty | DirectSelectionPhaseV1::SubmissionOpen
        ),
        DirectTerminalReasonV1::EmptyLapse => {
            selection.phase() == DirectSelectionPhaseV1::FrozenEmpty
        }
        DirectTerminalReasonV1::NoCandidate => {
            selection.phase() == DirectSelectionPhaseV1::Verifying
                && selection.candidate_count() == 0
                && selection.verification_cursor() == 0
        }
        DirectTerminalReasonV1::UnselectedLapse => {
            matches!(
                selection.phase(),
                DirectSelectionPhaseV1::SubmissionOpen | DirectSelectionPhaseV1::Verifying
            )
        }
        DirectTerminalReasonV1::SelectedLapse | DirectTerminalReasonV1::Settled => {
            selection.phase() == DirectSelectionPhaseV1::Selected
        }
    };
    if correct { Ok(()) } else { Err(DirectMarketErrorV1::WrongPhase) }
}

fn terminal_root_projection(
    mut root: crate::DirectMarketRootV1,
    selection_account: [u8; 32],
    reason: DirectTerminalReasonV1,
) -> crate::DirectMarketRootV1 {
    root.selection_account = selection_account;
    root.phase = DirectRootPhaseV1::Terminal;
    root.terminal_reason = Some(reason);
    root
}

fn terminal_receipt_id<B: DirectHashBackendV1>(
    state: DirectRootReplayPostV1,
    selection_pre_id: [u8; 32],
    selection_post_id: [u8; 32],
    endpoint_count: u8,
    endpoints: &[Option<DirectEndpointTerminalPlanV1>; 2],
    fee_terminal: Option<DirectFeeTerminalV1>,
    treasury: Option<DirectFeeTreasuryPlanV1>,
    transition_id: [u8; 32],
    reason: DirectTerminalReasonV1,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    let mut reservation_post_ids = [[0u8; 32]; 2];
    let mut position_post_ids = [[0u8; 32]; 2];
    let mut replay_post_ids = [[0u8; 32]; 2];
    let mut delta_ids = [[0u8; 32]; 2];
    let mut index = 0usize;
    while index < usize::from(endpoint_count) {
        let endpoint = endpoints[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
        reservation_post_ids[index] = endpoint.reservation_post_id;
        position_post_ids[index] = endpoint
            .replay_transition
            .position_poststate_semantic_id()
            .bytes();
        replay_post_ids[index] = endpoint
            .replay_transition
            .replay_poststate_semantic_id()
            .bytes();
        delta_ids[index] = endpoint.replay_transition.delta_id().bytes();
        index += 1;
    }
    let fee_transcript =
        fee_terminal.map_or([0u8; 41], DirectFeeTerminalV1::canonical_transcript);
    let (
        treasury_position_account,
        treasury_position_pre_id,
        treasury_position_post_id,
        treasury_replay_account,
        treasury_replay_pre_id,
        treasury_replay_post_id,
        treasury_delta_id,
    ) = match treasury {
        Some(value) => (
            value.position_poststate.account,
            value.position_poststate.prestate_semantic_id,
            value.replay_transition.position_poststate_semantic_id().bytes(),
            value.replay_transition.replay_account().bytes(),
            value.replay_transition.replay_prestate_semantic_id().bytes(),
            value.replay_transition.replay_poststate_semantic_id().bytes(),
            value.replay_transition.delta_id().bytes(),
        ),
        None => ([0; 32], [0; 32], [0; 32], [0; 32], [0; 32], [0; 32], [0; 32]),
    };
    let id = DirectHashBackendV1::sha256_parts(backend, &[
        ECONOMIC_TERMINAL_RECEIPT_DOMAIN_V1,
        &state.root.binding().market_instance_id,
        &state.root.binding().generation.to_le_bytes(),
        &state.root.binding().direct_fee_shape_id,
        &[reason.byte()],
        &transition_id,
        &selection_pre_id,
        &selection_post_id,
        &[endpoint_count],
        &reservation_post_ids[0],
        &reservation_post_ids[1],
        &position_post_ids[0],
        &position_post_ids[1],
        &replay_post_ids[0],
        &replay_post_ids[1],
        &delta_ids[0],
        &delta_ids[1],
        &fee_transcript,
        &treasury_position_account,
        &treasury_position_pre_id,
        &treasury_position_post_id,
        &treasury_replay_account,
        &treasury_replay_pre_id,
        &treasury_replay_post_id,
        &treasury_delta_id,
        &state.replay.action_transcript_id(),
    ]);
    require_live(id)?;
    Ok(id)
}

fn one_source_retirement(
    reservation: DirectReservationV1,
    observed_lamports: u64,
    neutral_sink: [u8; 32],
) -> Result<DirectRetirementTransferV1, DirectMarketErrorV1> {
    let rent = reservation.rent();
    let floor = rent
        .principal_lamports
        .checked_add(rent.donation_floor_lamports)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    if observed_lamports < floor || rent.payer == neutral_sink {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let retirement = DirectRetirementTransferV1 {
        sources: [
            Some(DirectRetirementSourceV1 {
                account: reservation.account(),
                rent,
                observed_lamports,
            }),
            None,
            None,
            None,
            None,
        ],
        source_count: 1,
        refunds: [
            Some(DirectPrincipalRefundV1 {
                recipient: rent.payer,
                lamports: rent.principal_lamports,
            }),
            None,
            None,
            None,
            None,
        ],
        refund_count: 1,
        neutral_lamport_sink: neutral_sink,
        surplus_lamports: observed_lamports
            .checked_sub(rent.principal_lamports)
            .ok_or(DirectMarketErrorV1::Arithmetic)?,
    };
    retirement.validate()?;
    Ok(retirement)
}

fn position_semantic_id<B: PositionV3Sha256Backend>(
    poststate: &PositionSettlementPoststateV3,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    poststate
        .semantic
        .semantic_id(backend)
        .map(|id| id.bytes())
        .map_err(|_| DirectMarketErrorV1::InvalidPosition)
}

const fn admission_kind(side: Side) -> GeneralReplayTransitionKindV1 {
    match side {
        Side::Buy => GeneralReplayTransitionKindV1::DirectMarketAdmitBuyer,
        Side::Sell => GeneralReplayTransitionKindV1::DirectMarketAdmitSeller,
    }
}

const fn cancel_kind(side: Side) -> GeneralReplayTransitionKindV1 {
    match side {
        Side::Buy => GeneralReplayTransitionKindV1::DirectMarketCancelBuyer,
        Side::Sell => GeneralReplayTransitionKindV1::DirectMarketCancelSeller,
    }
}

const fn terminal_kind(
    reason: DirectTerminalReasonV1,
    side: Side,
) -> GeneralReplayTransitionKindV1 {
    match (reason, side) {
        (DirectTerminalReasonV1::Settled, Side::Buy) => {
            GeneralReplayTransitionKindV1::DirectMarketSettleBuyer
        }
        (DirectTerminalReasonV1::Settled, Side::Sell) => {
            GeneralReplayTransitionKindV1::DirectMarketSettleSeller
        }
        (DirectTerminalReasonV1::MissedFreezeLapse, Side::Buy) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseEmptyBuyer
        }
        (DirectTerminalReasonV1::MissedFreezeLapse, Side::Sell) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseEmptySeller
        }
        (DirectTerminalReasonV1::EmptyLapse, Side::Buy) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseEmptyBuyer
        }
        (DirectTerminalReasonV1::EmptyLapse, Side::Sell) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseEmptySeller
        }
        (DirectTerminalReasonV1::UnselectedLapse, Side::Buy) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedBuyer
        }
        (DirectTerminalReasonV1::NoCandidate, Side::Buy) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedBuyer
        }
        (DirectTerminalReasonV1::UnselectedLapse, Side::Sell) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedSeller
        }
        (DirectTerminalReasonV1::NoCandidate, Side::Sell) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedSeller
        }
        (DirectTerminalReasonV1::SelectedLapse, Side::Buy) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseSelectedBuyer
        }
        (DirectTerminalReasonV1::SelectedLapse, Side::Sell) => {
            GeneralReplayTransitionKindV1::DirectMarketLapseSelectedSeller
        }
    }
}
