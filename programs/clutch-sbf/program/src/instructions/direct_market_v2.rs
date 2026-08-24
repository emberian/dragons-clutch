//! Current Direct `80/1` account authentication and writeback plane.
//!
//! Current actions accept only the fresh b1/v2 root. The unchanged b2/b3/b4
//! physical frames are interpreted only after that root has authenticated, so
//! their historical arithmetic shape cannot become a persisted V1 authority.
//! Action 1 and action 13 refuse before account inspection until their sole
//! Product FundingV4 and Product RootV2/0xba-v2 writers are available.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, MAX_PERMITTED_DATA_INCREASE, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_direct_market_runtime::codec_v1::{
    DIRECT_ACTION_REPLAY_BODY_BYTES_V1, DIRECT_RESERVATION_BODY_BYTES_V1,
    DIRECT_SELECTION_BODY_BYTES_V1,
};
use clutch_direct_market_runtime::codec_v2::{
    authenticate_direct_root_transition_body_v2,
    decode_direct_action_replay_body_for_transition_v2,
    decode_direct_reservation_body_for_transition_v2,
    decode_direct_selection_body_for_transition_v2,
    encode_direct_action_replay_body_into_transition_v2,
    encode_direct_reservation_body_into_transition_v2,
    encode_direct_selection_body_into_transition_v2,
    write_direct_root_transition_body_v2, AuthenticatedDirectRootTransitionV2,
    DIRECT_MARKET_ROOT_BODY_BYTES_V2 as RUNTIME_ROOT_BODY_BYTES_V2,
};
use clutch_direct_market_runtime::lifecycle_v2::{
    prepare_direct_reservation_admission_v2, prepare_direct_reservation_cancel_v2,
    begin_direct_candidate_verification_v2, bind_direct_candidate_work_batch_v2,
    finalize_direct_selection_v2, prepare_direct_candidate_work_batch_v2,
    prepare_direct_economic_terminal_v2, prepare_direct_missed_freeze_terminal_v2,
    prepare_direct_selection_freeze_v2,
    submit_direct_candidate_v2,
    verify_next_direct_candidate_v2, AuthenticatedDirectEconomicTerminalV2,
    AuthenticatedDirectReservationAdmissionV2, AuthenticatedDirectReservationCancelV2,
    AuthenticatedDirectSelectionFreezeV2, DirectRootReplayTransitionV2,
};
use clutch_direct_market_runtime::current_v2::DirectCurrentGeneralAuthorityV2;
use clutch_direct_market_runtime::fee_v2::DirectFeePolicyV2;
use clutch_direct_market_runtime::liveness_v1::DirectCandidateWorkBatchV1;
use clutch_direct_market_runtime::reservation_v1::DirectReservationV1;
use clutch_direct_market_runtime::selection_v1::{DirectSelectionPhaseV1, DirectSelectionV1};
use clutch_direct_market_runtime::settlement_v1::{
    DirectEndpointPrestateV1, DirectFeeTreasuryPrestateV1,
    DirectReservationOrderInputV1,
};
use clutch_direct_market_runtime::{
    DirectActionReplayV1, DirectHashBackendV1, DirectMarketActionV1,
    DirectMarketErrorV1, DirectRentOwnerV1, DirectRootPhaseV1, DirectTerminalReasonV1,
};
use clutch_batch::direct_pair_v1::DirectEconomicBookV1;
use clutch_batch::relation_v2::{
    EconomicDomainV2, PricePreconditionV2, ECONOMIC_RELATION_VERSION_V2,
    EMPTY_ECONOMIC_ORDER_V2,
};
use clutch_batch::{PartialPolicy, Side};
use clutch_batch::relation_v1::FrozenPolicyV1;
use clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2;
use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
};
use clutch_general_v2_contract::GeneralReplayTransitionPlanV1;
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimePersistedAccountViewV1, RuntimeReceiptKindV1,
    RuntimeReceiptObservationV1, RuntimeTransferRoleV1, RuntimeTransitionActionV1,
    RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentV1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_price_measure::PriceVectorV3;
use clutch_product_series::{
    CompiledProductSeriesBundleV6, ContentId, MarketGenesisProfileV2,
    MarketInstancePreimageV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
};
use clutch_retirement::{PositionPurposeV3, PositionV3Sha256Backend, ReplayV3HashBackend};
use clutch_solana_layout::direct_market_v1::{
    DirectAdmitOrderPayloadV1, DirectSubmitCandidatePayloadV1,
};
use clutch_solana_layout::direct_market_v2::{
    DirectMarketRootAccountV2, DIRECT_MARKET_ROOT_BODY_BYTES_V2,
};
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    DIRECT_ACTION_REPLAY_ACCOUNT_TAG, DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
    DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2, DIRECT_RESERVATION_ACCOUNT_BYTES,
    DIRECT_RESERVATION_ACCOUNT_TAG, DIRECT_RESERVATION_ACCOUNT_VERSION,
    DIRECT_SELECTION_ACCOUNT_BYTES, DIRECT_SELECTION_ACCOUNT_TAG,
    DIRECT_SELECTION_ACCOUNT_VERSION,
};
use clutch_solana_layout::{account_len, PriceGridAccount};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::collateral_position_v3::authenticate_general_market_v4_with_data_ids;
use super::general_v2_position_replay::authenticate_current_general_position_replay_v4;
use super::product_artifact::authenticate_product_artifact_v1;

const DIRECT_MARKET_V2_MAX_ACCOUNTS: usize = 30;
const DIRECT_MARKET_V2_MAX_PAYLOAD_BYTES: usize = 80;
const DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2: usize = 4;
const DIRECT_ADMIT_ORDER_FIXED_ACCOUNTS_V2: usize = 19;
const DIRECT_CANCEL_ORDER_ACCOUNTS_V2: usize = 16;
const DIRECT_FREEZE_BOOK_FIXED_ACCOUNTS_V2: usize = 12;

const DIRECT_PRICE_AUTHENTICATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/direct/price-authentication/v2\0";

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V2 == RUNTIME_ROOT_BODY_BYTES_V2);
const _: () = assert!(DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2 == 2_502);
const _: () = assert!(DIRECT_SELECTION_ACCOUNT_BYTES == 1_629);
const _: () = assert!(DIRECT_ACTION_REPLAY_ACCOUNT_BYTES == 394);
const _: () = assert!(DIRECT_RESERVATION_ACCOUNT_BYTES == 473);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectMarketRootV2>() <= 2_560);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectActionReplayV2>() <= 512);
const _: () = assert!(core::mem::size_of::<AuthenticatedDirectSelectionV2>() <= 192);

/// Allocation-free SHA-256 boundary for current Direct account adapters.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DirectRuntimeSha256V2;

impl DirectHashBackendV1 for DirectRuntimeSha256V2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for DirectRuntimeSha256V2 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for DirectRuntimeSha256V2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, parts)
    }
}

/// Current family dispatcher. Unsupported actions refuse before reading any
/// account, so no historical b1/v1 width can select a fallback route.
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        DirectMarketAction::AdmitOrder => {
            process_direct_admit_order_v2(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::CancelOrder => {
            process_direct_cancel_order_v2(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::SubmitCandidate => {
            require(
                accounts.len() <= DIRECT_MARKET_V2_MAX_ACCOUNTS,
                ClutchError::AccountCount,
            )?;
            require(
                payload.len() <= DIRECT_MARKET_V2_MAX_PAYLOAD_BYTES,
                ClutchError::WrongDataLength,
            )?;
            process_direct_submit_candidate_v2(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::BeginVerification | DirectMarketAction::VerifyCandidate => {
            process_direct_candidate_verification_v2(
                program_id,
                accounts,
                sequence,
                action,
                payload,
            )
        }
        DirectMarketAction::FinalizeSelection => {
            require(
                accounts.len() <= DIRECT_MARKET_V2_MAX_ACCOUNTS,
                ClutchError::AccountCount,
            )?;
            process_direct_finalize_selection_v2(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::FreezeBook => {
            process_direct_freeze_book_v2(program_id, accounts, sequence, payload)
        }
        DirectMarketAction::LapseEmpty
        | DirectMarketAction::LapseUnselected
        | DirectMarketAction::LapseSelected => {
            process_direct_lapse_terminal_v2(program_id, accounts, sequence, action, payload)
        }
        DirectMarketAction::InitializeMarket
        | DirectMarketAction::SettlePair
        | DirectMarketAction::RetireTerminal => {
            Err(Refusal::Adapter(ClutchError::UnsupportedInstruction))
        }
    }
}

/// Execute action 2 across current b1/v2, fresh b4, and one General V4
/// Position/Replay pair. The optional peer is derived only from the root's live
/// Reservation count; the payload carries no peer selector or funding amount.
#[inline(never)]
fn process_direct_admit_order_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        accounts.len() >= DIRECT_ADMIT_ORDER_FIXED_ACCOUNTS_V2,
        ClutchError::AccountCount,
    )?;
    let request = DirectAdmitOrderPayloadV1::decode(payload)?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let peer_count = usize::from(root.transition().live_reservations());
    require(peer_count <= 1, ClutchError::MismatchedState)?;
    let expected_count = DIRECT_ADMIT_ORDER_FIXED_ACCOUNTS_V2
        .checked_add(peer_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    require_distinct(accounts)?;
    let direct_replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[13])?;
    let rent_parameters = read_rent(&accounts[14])?;
    let observed_slot = read_clock_slot(&accounts[15])?;
    authenticate_direct_order_limit_v2(
        program_id,
        &root,
        &accounts[16],
        &accounts[17],
        &accounts[18],
        request.limit_price_units_per_egg,
    )?;
    let bound = authenticate_direct_general_market_v4(
        program_id,
        &root,
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        &accounts[12],
        &accounts[17],
    )?;
    let position_replay = authenticate_current_general_position_replay_v4(
        program_id,
        bound,
        &accounts[10],
        &accounts[11],
        &accounts[4],
        &accounts[5],
        accounts[3].key.to_bytes(),
    )?;
    let existing_peer = if peer_count == 0 {
        None
    } else {
        let peer = authenticate_direct_reservation_readonly_v2(
            program_id,
            &accounts[19],
            &root,
        )?;
        require(
            peer.account().to_bytes()
                == root
                    .transition()
                    .reservation_account(0)
                    .map_err(map_direct_error_v2)?
                && peer.semantic_id()
                    == root
                        .transition()
                        .reservation_semantic_id(0)
                        .map_err(map_direct_error_v2)?,
            ClutchError::MismatchedState,
        )?;
        Some(peer.value())
    };
    let (reservation_pda, reservation_bump) = seeds::direct_reservation_v1_pda(
        program_id,
        &root.account(),
        &request.order_id,
    );
    let donation_floor_lamports = authenticate_fresh_direct_pda_v2(
        &accounts[2],
        (reservation_pda, reservation_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_RESERVATION_ACCOUNT_BYTES)?;
    let reservation_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    reservation_rent.validate().map_err(map_direct_error_v2)?;
    let order = DirectReservationOrderInputV1 {
        reservation_account: accounts[2].key.to_bytes(),
        order_id: request.order_id,
        side: request.side,
        outcome: request.outcome,
        quantity: request.quantity,
        minimum_fill: request.minimum_fill,
        partial_policy: request.partial_policy,
        expiry_epoch: request.expiry_epoch,
        limit_price_units_per_egg: request.limit_price_units_per_egg,
        rent: reservation_rent,
    };
    let root_bump = root.bump();
    let replay_bump = direct_replay.bump();
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        direct_replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let authority = DirectReservationAdmissionAuthoritySbfV2 {
        root_semantic_id: state.root().root_semantic_id(),
        replay: state.replay(),
        position: position_replay.position,
        existing_peer,
        order,
        sequence,
        slot: observed_slot,
    };
    let plan = Box::new(
        prepare_direct_reservation_admission_v2(
            &authority,
            &mut state,
            position_replay.replay,
            existing_peer,
            sequence,
            observed_slot,
            order,
            &DirectRuntimeSha256V2,
        )
        .map_err(map_direct_error_v2)?,
    );

    let root_bytes = accounts[0].key.to_bytes();
    let bump_seed = [reservation_bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_DIRECT_RESERVATION_V1,
        &root_bytes,
        &request.order_id,
        &bump_seed,
    ];
    create_current_direct_account_v2(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[13],
        &rent_parameters,
        DIRECT_RESERVATION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    write_position_post_v2(&accounts[4], &plan.position_poststate)?;
    write_general_replay_post_v2(&accounts[5], &plan.replay_transition)?;
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_fresh_direct_reservation_v2(
        &accounts[2],
        reservation_bump,
        plan.reservation,
        state.root(),
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectReservationAdmissionAuthoritySbfV2 {
    root_semantic_id: [u8; 32],
    replay: DirectActionReplayV1,
    position: AuthenticatedPositionV3,
    existing_peer: Option<DirectReservationV1>,
    order: DirectReservationOrderInputV1,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectReservationAdmissionV2 for DirectReservationAdmissionAuthoritySbfV2 {
    fn authenticate_admission_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        position_replay: clutch_general_v2_contract::GeneralPositionReplayPrestateV1,
        existing_peer: Option<DirectReservationV1>,
        consumed_sequence: u64,
        observed_slot: u64,
        order: DirectReservationOrderInputV1,
    ) -> Result<(), DirectMarketErrorV1> {
        if state.root().root_semantic_id() == self.root_semantic_id
            && state.replay() == self.replay
            && position_replay.position() == self.position
            && existing_peer == self.existing_peer
            && order == self.order
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Execute action 3 and retire exactly one active b4. Principal returns only
/// to the persisted payer; hostile prefund and surplus go only to b1/v2's
/// Realm-authenticated neutral sink.
#[inline(never)]
fn process_direct_cancel_order_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, DIRECT_CANCEL_ORDER_ACCOUNTS_V2)?;
    require_distinct(accounts)?;
    require(payload.is_empty(), ClutchError::WrongDataLength)?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let direct_replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    let reservation = authenticate_direct_reservation_writable_v2(
        program_id,
        &accounts[2],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require(
        accounts[14].is_writable
            && !accounts[14].is_signer
            && !accounts[14].executable
            && accounts[14].key.to_bytes() == root.transition().neutral_lamport_sink()
            && accounts[3].key.to_bytes() == reservation.value().owner()
            && accounts[3].key.to_bytes() == reservation.value().rent().payer,
        ClutchError::MismatchedState,
    )?;
    let observed_slot = read_clock_slot(&accounts[15])?;
    let bound = authenticate_direct_general_market_v4(
        program_id,
        &root,
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        &accounts[12],
        &accounts[13],
    )?;
    let position_replay = authenticate_current_general_position_replay_v4(
        program_id,
        bound,
        &accounts[10],
        &accounts[11],
        &accounts[4],
        &accounts[5],
        accounts[3].key.to_bytes(),
    )?;
    let root_bump = root.bump();
    let replay_bump = direct_replay.bump();
    let reservation_lamports = reservation.observed_lamports;
    let reservation_value = reservation.value();
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        direct_replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let authority = DirectReservationCancelAuthoritySbfV2 {
        root_semantic_id: state.root().root_semantic_id(),
        replay: state.replay(),
        reservation: reservation_value,
        position_replay: position_replay.replay,
        observed_lamports: reservation_lamports,
        sequence,
        slot: observed_slot,
    };
    let plan = Box::new(
        prepare_direct_reservation_cancel_v2(
            &authority,
            &mut state,
            reservation_value,
            position_replay.replay,
            reservation_lamports,
            sequence,
            observed_slot,
            &DirectRuntimeSha256V2,
        )
        .map_err(map_direct_error_v2)?,
    );
    require(
        plan.retirement.source_count == 1
            && plan.retirement.refund_count == 1
            && plan.retirement.neutral_lamport_sink == accounts[14].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let refund = plan.retirement.refunds[0]
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source = plan.retirement.sources[0]
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        source.account == accounts[2].key.to_bytes()
            && source.observed_lamports == accounts[2].lamports()
            && refund.recipient == accounts[3].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    credit_lamports_v2(&accounts[3], refund.lamports)?;
    credit_lamports_v2(&accounts[14], plan.retirement.surplus_lamports)?;
    write_position_post_v2(&accounts[4], &plan.endpoint.position_poststate)?;
    write_general_replay_post_v2(&accounts[5], &plan.endpoint.replay_transition)?;
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    close_direct_program_account_v2(&accounts[2], source.observed_lamports)
}

#[derive(Clone, Copy, Debug)]
struct DirectReservationCancelAuthoritySbfV2 {
    root_semantic_id: [u8; 32],
    replay: DirectActionReplayV1,
    reservation: DirectReservationV1,
    position_replay: clutch_general_v2_contract::GeneralPositionReplayPrestateV1,
    observed_lamports: u64,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectReservationCancelV2 for DirectReservationCancelAuthoritySbfV2 {
    fn authenticate_cancel_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        reservation: DirectReservationV1,
        position_replay: clutch_general_v2_contract::GeneralPositionReplayPrestateV1,
        observed_reservation_lamports: u64,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state.root().root_semantic_id() == self.root_semantic_id
            && state.replay() == self.replay
            && reservation == self.reservation
            && position_replay == self.position_replay
            && observed_reservation_lamports == self.observed_lamports
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Execute action 4 over the exhaustive root-derived active Reservation set.
/// The payload carries no count, price, work amount, or liveness ordinal.
#[inline(never)]
fn process_direct_freeze_book_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        accounts.len() >= DIRECT_FREEZE_BOOK_FIXED_ACCOUNTS_V2,
        ClutchError::AccountCount,
    )?;
    require(payload.is_empty(), ClutchError::WrongDataLength)?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let reservation_count = usize::from(root.transition().live_reservations());
    require(reservation_count <= 2, ClutchError::MismatchedState)?;
    let liveness_start = DIRECT_FREEZE_BOOK_FIXED_ACCOUNTS_V2
        .checked_add(reservation_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_count = liveness_start
        .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    require_distinct(&accounts[..liveness_start])?;
    require_direct_freeze_liveness_aliases_v2(accounts, liveness_start)?;

    let direct_replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[4])?;
    let rent_parameters = read_rent(&accounts[5])?;
    let observed_slot = read_clock_slot(&accounts[6])?;
    let (selection_pda, selection_bump) =
        seeds::direct_selection_v1_pda(program_id, &root.account());
    let donation_floor_lamports = authenticate_fresh_direct_pda_v2(
        &accounts[2],
        (selection_pda, selection_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let selection_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    selection_rent.validate().map_err(map_direct_error_v2)?;

    let mut authenticated: [Option<AuthenticatedDirectReservationV2>; 2] = [None; 2];
    let mut index = 0usize;
    while index < reservation_count {
        authenticated[index] = Some(authenticate_direct_reservation_readonly_v2(
            program_id,
            &accounts[DIRECT_FREEZE_BOOK_FIXED_ACCOUNTS_V2 + index],
            &root,
        )?);
        index += 1;
    }
    if reservation_count == 2 {
        let left = authenticated[0]
            .take()
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        let right = authenticated[1]
            .take()
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        authenticated = if right.value().order_id() < left.value().order_id() {
            [Some(right), Some(left)]
        } else {
            [Some(left), Some(right)]
        };
    }
    let mut reservations = [None; 2];
    index = 0;
    while index < reservation_count {
        reservations[index] = Some(
            authenticated[index]
                .as_ref()
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?
                .value(),
        );
        index += 1;
    }
    let price = authenticate_direct_price_precondition_v2(
        program_id,
        &root,
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        reservations,
    )?;
    let root_bump = root.bump();
    let replay_bump = direct_replay.bump();
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        direct_replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let authority = DirectSelectionFreezeAuthoritySbfV2 {
        root_semantic_id: state.root().root_semantic_id(),
        replay: state.replay(),
        selection_account: accounts[2].key.to_bytes(),
        rent: selection_rent,
        reservations,
        price: &price,
        sequence,
        slot: observed_slot,
    };
    let plan = Box::new(
        prepare_direct_selection_freeze_v2(
            &authority,
            &mut state,
            sequence,
            observed_slot,
            accounts[2].key.to_bytes(),
            selection_rent,
            reservations,
            price.domain(),
            price.price(),
            &DirectRuntimeSha256V2,
        )
        .map_err(map_direct_error_v2)?,
    );
    apply_direct_candidate_work_v2(
        program_id,
        &accounts[liveness_start..],
        &accounts[1],
        &mut state,
        &plan.selection,
        DirectMarketActionV1::FreezeBook,
    )?;

    let root_bytes = accounts[0].key.to_bytes();
    let bump_seed = [selection_bump];
    let signer_seeds: [&[u8]; 3] = [
        seeds::SEED_DIRECT_SELECTION_V1,
        &root_bytes,
        &bump_seed,
    ];
    create_current_direct_account_v2(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[4],
        &rent_parameters,
        DIRECT_SELECTION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_fresh_direct_selection_v2(
        &accounts[2],
        selection_bump,
        plan.selection,
        state.root(),
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectSelectionFreezeAuthoritySbfV2<'a> {
    root_semantic_id: [u8; 32],
    replay: DirectActionReplayV1,
    selection_account: [u8; 32],
    rent: DirectRentOwnerV1,
    reservations: [Option<DirectReservationV1>; 2],
    price: &'a AuthenticatedDirectPricePreconditionV2,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectSelectionFreezeV2 for DirectSelectionFreezeAuthoritySbfV2<'_> {
    fn authenticate_freeze_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        selection_account: [u8; 32],
        rent: DirectRentOwnerV1,
        reservations: &[Option<DirectReservationV1>; 2],
        domain: &EconomicDomainV2,
        price: &PricePreconditionV2,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state.root().root_semantic_id() == self.root_semantic_id
            && state.replay() == self.replay
            && selection_account == self.selection_account
            && rent == self.rent
            && reservations == &self.reservations
            && domain == &self.price.domain()
            && price == &self.price.price()
            && self.price.authentication_id() != [0; 32]
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Private exact Product/PriceGrid authority retained only for action 4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDirectPricePreconditionV2 {
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    authentication_id: [u8; 32],
}

impl AuthenticatedDirectPricePreconditionV2 {
    const fn domain(self) -> EconomicDomainV2 { self.domain }
    const fn price(self) -> PricePreconditionV2 { self.price }
    const fn authentication_id(self) -> [u8; 32] { self.authentication_id }
}

/// Execute action 8. A nonempty Selection uses the compact finalization frame;
/// the explicit no-candidate terminal is delegated to its full current General
/// endpoint handler and cannot fall through to historical b1/v1 code.
#[inline(never)]
fn process_direct_finalize_selection_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(accounts.len() >= 3, ClutchError::AccountCount)?;
    let root_probe = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let selection_probe = authenticate_direct_selection_writable_v2(
        program_id,
        &accounts[2],
        &root_probe,
    )?;
    if selection_probe.value().candidate_count() == 0 {
        return process_direct_no_candidate_terminal_v2(
            program_id,
            accounts,
            sequence,
            payload,
        );
    }
    process_direct_nonempty_selection_finalization_v2(
        program_id,
        accounts,
        sequence,
        payload,
    )
}

/// Select the best valid submitted candidate and refund every retained bond.
/// Refund owners are derived from b2 and must be supplied once in canonical
/// key order before the four-account Candidate-liveness suffix.
#[inline(never)]
fn process_direct_nonempty_selection_finalization_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        accounts.len() >= 8 && accounts.len() <= 11,
        ClutchError::AccountCount,
    )?;
    require(payload.is_empty(), ClutchError::WrongDataLength)?;
    require_distinct(&accounts[..4])?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v2(
        program_id,
        &accounts[2],
        &root,
    )?;
    let observed_slot = read_clock_slot(&accounts[3])?;
    let root_bump = root.bump();
    let replay_bump = replay.bump();
    let selection_bump = selection.bump();
    let selection_balance_before = selection.observed_lamports();
    let mut selection_value = selection.into_value();
    let bond_principal_before = root
        .transition()
        .outstanding_candidate_bond_lamports(*selection_value)
        .map_err(map_direct_error_v2)?;
    let selection_rent = selection_value.rent();
    let accounted_balance_before = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_before))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        selection_balance_before >= accounted_balance_before,
        ClutchError::MismatchedState,
    )?;
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let effects = finalize_direct_selection_v2(
        &mut state,
        &mut selection_value,
        sequence,
        observed_slot,
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)?;
    require(
        effects.candidate_bond_movement.is_none(),
        ClutchError::MismatchedState,
    )?;
    let refunds = effects
        .candidate_bond_refunds
        .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        refunds.total_lamports == bond_principal_before,
        ClutchError::MismatchedState,
    )?;
    let refund_count = usize::from(refunds.refund_count);
    let liveness_start = 4usize
        .checked_add(refund_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_count = liveness_start
        .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    let mut index = 0usize;
    while index < refund_count {
        let refund = refunds.refunds[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        let account = &accounts[4 + index];
        require(
            account.is_writable
                && !account.is_signer
                && !account.executable
                && account.key.to_bytes() == refund.recipient,
            ClutchError::MismatchedState,
        )?;
        let mut fixed = 0usize;
        while fixed < 4 {
            require(account.key != accounts[fixed].key, ClutchError::AccountAlias)?;
            fixed += 1;
        }
        if index != 0 {
            require(
                accounts[3 + index].key.to_bytes() < account.key.to_bytes(),
                ClutchError::AccountAlias,
            )?;
        }
        index += 1;
    }
    require_direct_candidate_liveness_aliases_v2(accounts, liveness_start, 4)?;

    debit_lamports_v2(&accounts[2], refunds.total_lamports)?;
    index = 0;
    while index < refund_count {
        let refund = refunds.refunds[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        credit_lamports_v2(&accounts[4 + index], refund.lamports)?;
        index += 1;
    }
    let selection_balance_after = selection_balance_before
        .checked_sub(refunds.total_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        accounts[2].lamports() == selection_balance_after,
        ClutchError::MismatchedState,
    )?;
    let bond_principal_after = state
        .root()
        .outstanding_candidate_bond_lamports(*selection_value)
        .map_err(map_direct_error_v2)?;
    let accounted_balance_after = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_after))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        bond_principal_after == 0 && accounts[2].lamports() >= accounted_balance_after,
        ClutchError::MismatchedState,
    )?;

    apply_direct_candidate_work_v2(
        program_id,
        &accounts[liveness_start..],
        &accounts[1],
        &mut state,
        &selection_value,
        DirectMarketActionV1::FinalizeSelection,
    )?;
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_direct_selection_v2(
        &accounts[2],
        selection_bump,
        *selection_value,
        state.root(),
    )
}

/// Route actions 10..12 without accepting a caller reason. Action 10 may
/// construct the terminal b2 directly from an Open root after submission
/// close; otherwise every branch consumes the already-authenticated b2.
#[inline(never)]
fn process_direct_lapse_terminal_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        DirectMarketAction::LapseEmpty => {
            require(!accounts.is_empty(), ClutchError::AccountCount)?;
            let phase = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?
                .transition()
                .phase();
            if phase == DirectRootPhaseV1::Open {
                process_direct_missed_freeze_lapse_v2(
                    program_id,
                    accounts,
                    sequence,
                    payload,
                )
            } else {
                process_direct_fee_free_selection_terminal_v2(
                    program_id,
                    accounts,
                    sequence,
                    payload,
                    DirectTerminalReasonV1::EmptyLapse,
                    DirectMarketActionV1::LapseEmpty,
                    false,
                )
            }
        }
        DirectMarketAction::LapseUnselected => process_direct_fee_free_selection_terminal_v2(
            program_id,
            accounts,
            sequence,
            payload,
            DirectTerminalReasonV1::UnselectedLapse,
            DirectMarketActionV1::LapseUnselected,
            false,
        ),
        DirectMarketAction::LapseSelected => process_direct_fee_free_selection_terminal_v2(
            program_id,
            accounts,
            sequence,
            payload,
            DirectTerminalReasonV1::SelectedLapse,
            DirectMarketActionV1::LapseSelected,
            false,
        ),
        _ => Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    }
}

/// Execute action 10 from an Open b1/v2 after submission close. The exact
/// fresh b2, complete current b4/Position/Replay prefix, canonical Product
/// price graph, and liveness work are one rollback domain.
#[inline(never)]
fn process_direct_missed_freeze_lapse_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    const FIXED: usize = 19;
    require(accounts.len() >= FIXED, ClutchError::AccountCount)?;
    require(payload.is_empty(), ClutchError::WrongDataLength)?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    require(
        root.transition().phase() == DirectRootPhaseV1::Open,
        ClutchError::MismatchedState,
    )?;
    let endpoint_count = usize::from(root.transition().live_reservations());
    require(endpoint_count <= 2, ClutchError::MismatchedState)?;
    let endpoint_end = endpoint_count
        .checked_mul(3)
        .and_then(|value| value.checked_add(FIXED))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_count = endpoint_end
        .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    require_distinct(&accounts[..FIXED])?;
    require_direct_endpoint_alias_contract_v2(accounts, FIXED, endpoint_count)?;
    require_direct_freeze_liveness_aliases_v2(accounts, endpoint_end)?;
    let replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[4])?;
    let rent_parameters = read_rent(&accounts[5])?;
    let observed_slot = read_clock_slot(&accounts[6])?;
    let (selection_pda, selection_bump) =
        seeds::direct_selection_v1_pda(program_id, &root.account());
    let donation_floor_lamports = authenticate_fresh_direct_pda_v2(
        &accounts[2],
        (selection_pda, selection_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let selection_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    selection_rent.validate().map_err(map_direct_error_v2)?;
    let bound = authenticate_direct_general_market_v4(
        program_id,
        &root,
        &accounts[12],
        &accounts[13],
        &accounts[14],
        &accounts[15],
        &accounts[16],
        &accounts[17],
        &accounts[18],
        &accounts[10],
    )?;

    let mut authenticated: [Option<AuthenticatedDirectReservationV2>; 2] = [None; 2];
    let mut endpoints = [None; 2];
    let mut reservations = [None; 2];
    let mut reservation_accounts = [[0u8; 32]; 2];
    let mut reservation_semantic_ids = [[0u8; 32]; 2];
    let mut index = 0usize;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v2(FIXED, index)?;
        let reservation = authenticate_direct_reservation_writable_v2(
            program_id,
            &accounts[first],
            &root,
        )?;
        if index != 0 {
            let previous = authenticated[index - 1]
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            require(
                previous.value().order_id() < reservation.value().order_id(),
                ClutchError::MismatchedState,
            )?;
        }
        let position_replay = authenticate_current_general_position_replay_v4(
            program_id,
            bound,
            &accounts[16],
            &accounts[17],
            &accounts[first + 1],
            &accounts[first + 2],
            reservation.value().owner(),
        )?;
        authenticated[index] = Some(reservation);
        endpoints[index] = Some(DirectEndpointPrestateV1 {
            reservation: reservation.value(),
            position_replay: position_replay.replay,
        });
        reservations[index] = Some(reservation.value());
        reservation_accounts[index] = reservation.account().to_bytes();
        reservation_semantic_ids[index] = reservation.semantic_id();
        index += 1;
    }
    let price = authenticate_direct_price_precondition_v2(
        program_id,
        &root,
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        reservations,
    )?;
    let root_bump = root.bump();
    let replay_bump = replay.bump();
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let freeze_authority = DirectSelectionFreezeAuthoritySbfV2 {
        root_semantic_id: state.root().root_semantic_id(),
        replay: state.replay(),
        selection_account: accounts[2].key.to_bytes(),
        rent: selection_rent,
        reservations,
        price: &price,
        sequence,
        slot: observed_slot,
    };
    let terminal_authority = DirectMissedFreezeTerminalAuthoritySbfV2 {
        root_semantic_id: state.root().root_semantic_id(),
        replay_semantic_id: state
            .root()
            .action_replay_semantic_id(state.replay(), &DirectRuntimeSha256V2)
            .map_err(map_direct_error_v2)?,
        selection_account: accounts[2].key.to_bytes(),
        selection_rent,
        reservation_accounts,
        reservation_semantic_ids,
        reservation_count: u8::try_from(endpoint_count)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
        price: &price,
        endpoints: &endpoints,
        fee_policy: state.root().fee_policy(),
        realm: state.root().realm_id(),
        sequence,
        slot: observed_slot,
    };
    let plan = Box::new(
        prepare_direct_missed_freeze_terminal_v2(
            &freeze_authority,
            &terminal_authority,
            &mut state,
            accounts[2].key.to_bytes(),
            selection_rent,
            reservations,
            price.domain(),
            price.price(),
            endpoints,
            sequence,
            observed_slot,
            &DirectRuntimeSha256V2,
        )
        .map_err(map_direct_error_v2)?,
    );
    apply_direct_candidate_work_v2(
        program_id,
        &accounts[endpoint_end..],
        &accounts[1],
        &mut state,
        &plan.selection,
        DirectMarketActionV1::LapseEmpty,
    )?;

    let root_bytes = accounts[0].key.to_bytes();
    let bump_seed = [selection_bump];
    let signer_seeds: [&[u8]; 3] = [
        seeds::SEED_DIRECT_SELECTION_V1,
        &root_bytes,
        &bump_seed,
    ];
    create_current_direct_account_v2(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[4],
        &rent_parameters,
        DIRECT_SELECTION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    index = 0;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v2(FIXED, index)?;
        let endpoint = plan.endpoints[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        let reservation = authenticated[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        write_position_post_v2(&accounts[first + 1], &endpoint.position_poststate)?;
        write_general_replay_post_v2(&accounts[first + 2], &endpoint.replay_transition)?;
        write_direct_reservation_v2(
            &accounts[first],
            reservation.bump(),
            endpoint.reservation_post,
            state.root(),
        )?;
        index += 1;
    }
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_fresh_direct_selection_v2(
        &accounts[2],
        selection_bump,
        plan.selection,
        state.root(),
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectMissedFreezeTerminalAuthoritySbfV2<'a> {
    root_semantic_id: [u8; 32],
    replay_semantic_id: [u8; 32],
    selection_account: [u8; 32],
    selection_rent: DirectRentOwnerV1,
    reservation_accounts: [[u8; 32]; 2],
    reservation_semantic_ids: [[u8; 32]; 2],
    reservation_count: u8,
    price: &'a AuthenticatedDirectPricePreconditionV2,
    endpoints: &'a [Option<DirectEndpointPrestateV1>; 2],
    fee_policy: DirectFeePolicyV2,
    realm: [u8; 32],
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectEconomicTerminalV2 for DirectMissedFreezeTerminalAuthoritySbfV2<'_> {
    fn authenticate_terminal_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        selection: DirectSelectionV1,
        ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        fee_policy: DirectFeePolicyV2,
        realm: [u8; 32],
        batch_policy: Option<&FrozenPolicyV1>,
        revenue_policy: Option<&RevenuePolicyV2>,
        fee_terminal: Option<clutch_direct_market_runtime::fee_v1::DirectFeeTerminalV1>,
        treasury: Option<DirectFeeTreasuryPrestateV1>,
        reason: DirectTerminalReasonV1,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state.root().root_semantic_id() != self.root_semantic_id
            || state
                .root()
                .action_replay_semantic_id(state.replay(), &DirectRuntimeSha256V2)?
                != self.replay_semantic_id
            || selection.account() != self.selection_account
            || selection.rent() != self.selection_rent
            || selection.reservation_count() != self.reservation_count
            || *selection.domain() != self.price.domain()
            || *selection.price() != self.price.price()
            || selection.candidate_count() != 0
            || selection.verification_cursor() != 0
            || selection.selected_pair().is_some()
            || selection.terminal_receipt_id() != [0; 32]
            || ordered_endpoints != self.endpoints
            || fee_policy != self.fee_policy
            || realm != self.realm
            || batch_policy.is_some()
            || revenue_policy.is_some()
            || fee_terminal.is_some()
            || treasury.is_some()
            || reason != DirectTerminalReasonV1::MissedFreezeLapse
            || consumed_sequence != self.sequence
            || observed_slot != self.slot
        {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let expected_phase = if self.reservation_count == 2 {
            DirectSelectionPhaseV1::SubmissionOpen
        } else {
            DirectSelectionPhaseV1::FrozenEmpty
        };
        if selection.phase() != expected_phase {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let mut index = 0usize;
        while index < 2 {
            if index < usize::from(self.reservation_count) {
                let bounded = u8::try_from(index)
                    .map_err(|_| DirectMarketErrorV1::Arithmetic)?;
                if selection.reservation_account(bounded)? != self.reservation_accounts[index]
                    || selection.reservation_semantic_id(bounded)?
                        != self.reservation_semantic_ids[index]
                {
                    return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
                }
            } else if self.reservation_accounts[index] != [0; 32]
                || self.reservation_semantic_ids[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
            }
            index += 1;
        }
        Ok(())
    }
}

/// The no-candidate branch is implemented below with the exact General V4
/// endpoint graph. Keeping this named seam prevents account-count probing from
/// selecting any historical economic-terminal handler.
#[inline(never)]
fn process_direct_no_candidate_terminal_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    process_direct_fee_free_selection_terminal_v2(
        program_id,
        accounts,
        sequence,
        payload,
        DirectTerminalReasonV1::NoCandidate,
        DirectMarketActionV1::FinalizeSelection,
        true,
    )
}

/// Execute one existing-selection fee-free terminal. The reason and liveness
/// role come only from the checked dispatcher; no payload enum selects them.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn process_direct_fee_free_selection_terminal_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
    reason: DirectTerminalReasonV1,
    runtime_action: DirectMarketActionV1,
    require_no_candidate: bool,
) -> Outcome<()> {
    require(accounts.len() >= 16, ClutchError::AccountCount)?;
    require(payload.is_empty(), ClutchError::WrongDataLength)?;
    require_distinct(&accounts[..12])?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v2(
        program_id,
        &accounts[2],
        &root,
    )?;
    if require_no_candidate {
        require(
            selection.value().candidate_count() == 0,
            ClutchError::MismatchedState,
        )?;
    }
    let endpoint_count = usize::from(selection.value().reservation_count());
    let endpoint_end = endpoint_count
        .checked_mul(3)
        .and_then(|value| value.checked_add(12))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let minimum_count = endpoint_end
        .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let maximum_count = minimum_count
        .checked_add(3)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        accounts.len() >= minimum_count && accounts.len() <= maximum_count,
        ClutchError::AccountCount,
    )?;
    require_direct_endpoint_alias_contract_v2(accounts, 12, endpoint_count)?;
    let observed_slot = read_clock_slot(&accounts[11])?;
    let bound = authenticate_direct_general_market_v4(
        program_id,
        &root,
        &accounts[3],
        &accounts[4],
        &accounts[5],
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
    )?;

    let mut authenticated_reservations = [None; 2];
    let mut endpoints = [None; 2];
    let mut index = 0usize;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v2(12, index)?;
        let reservation = authenticate_direct_reservation_writable_v2(
            program_id,
            &accounts[first],
            &root,
        )?;
        let selection_index = u8::try_from(index)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            selection
                .value()
                .reservation_account(selection_index)
                .map_err(map_direct_error_v2)?
                == reservation.account().to_bytes()
                && selection
                    .value()
                    .reservation_semantic_id(selection_index)
                    .map_err(map_direct_error_v2)?
                    == reservation.semantic_id(),
            ClutchError::MismatchedState,
        )?;
        let position_replay = authenticate_current_general_position_replay_v4(
            program_id,
            bound,
            &accounts[7],
            &accounts[8],
            &accounts[first + 1],
            &accounts[first + 2],
            reservation.value().owner(),
        )?;
        endpoints[index] = Some(DirectEndpointPrestateV1 {
            reservation: reservation.value(),
            position_replay: position_replay.replay,
        });
        authenticated_reservations[index] = Some(reservation);
        index += 1;
    }

    let root_bump = root.bump();
    let replay_bump = replay.bump();
    let selection_bump = selection.bump();
    let selection_balance_before = selection.observed_lamports();
    let selection_value = selection.into_value();
    let bond_principal_before = root
        .transition()
        .outstanding_candidate_bond_lamports(*selection_value)
        .map_err(map_direct_error_v2)?;
    let selection_rent = selection_value.rent();
    let accounted_balance_before = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_before))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        selection_balance_before >= accounted_balance_before,
        ClutchError::MismatchedState,
    )?;
    let realm = root.transition().realm_id();
    let fee_policy = root.transition().fee_policy();
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let authority = DirectEconomicTerminalAuthoritySbfV2 {
        root_semantic_id: state.root().root_semantic_id(),
        replay: state.replay(),
        selection: &selection_value,
        endpoints: &endpoints,
        fee_policy,
        realm,
        reason,
        sequence,
        slot: observed_slot,
    };
    let plan = Box::new(
        prepare_direct_economic_terminal_v2(
            &authority,
            &mut state,
            *selection_value,
            endpoints,
            realm,
            None,
            None,
            None,
            reason,
            sequence,
            observed_slot,
            &DirectRuntimeSha256V2,
        )
        .map_err(map_direct_error_v2)?,
    );
    let refund_count = plan
        .candidate_bond_refunds
        .map_or(0usize, |refunds| usize::from(refunds.refund_count));
    let refund_end = endpoint_end
        .checked_add(refund_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_count = refund_end
        .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    if let Some(refunds) = plan.candidate_bond_refunds {
        require(
            refunds.total_lamports == bond_principal_before,
            ClutchError::MismatchedState,
        )?;
        index = 0;
        while index < refund_count {
            let refund = refunds.refunds[index]
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            let account = &accounts[endpoint_end + index];
            require(
                account.is_writable
                    && !account.is_signer
                    && !account.executable
                    && account.key.to_bytes() == refund.recipient,
                ClutchError::MismatchedState,
            )?;
            let mut prior = 0usize;
            while prior < endpoint_end {
                require(account.key != accounts[prior].key, ClutchError::AccountAlias)?;
                prior += 1;
            }
            if index != 0 {
                require(
                    accounts[endpoint_end + index - 1].key.to_bytes()
                        < account.key.to_bytes(),
                    ClutchError::AccountAlias,
                )?;
            }
            index += 1;
        }
        debit_lamports_v2(&accounts[2], refunds.total_lamports)?;
        index = 0;
        while index < refund_count {
            let refund = refunds.refunds[index]
                .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
            credit_lamports_v2(&accounts[endpoint_end + index], refund.lamports)?;
            index += 1;
        }
        require(
            accounts[2].lamports()
                == selection_balance_before
                    .checked_sub(refunds.total_lamports)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
            ClutchError::MismatchedState,
        )?;
    } else {
        require(bond_principal_before == 0, ClutchError::MismatchedState)?;
    }

    require_direct_candidate_liveness_aliases_v2(accounts, refund_end, endpoint_end)?;
    apply_direct_candidate_work_v2(
        program_id,
        &accounts[refund_end..],
        &accounts[1],
        &mut state,
        &plan.selection,
        runtime_action,
    )?;

    index = 0;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v2(12, index)?;
        let endpoint = plan.endpoints[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        write_position_post_v2(&accounts[first + 1], &endpoint.position_poststate)?;
        write_general_replay_post_v2(&accounts[first + 2], &endpoint.replay_transition)?;
        let reservation = authenticated_reservations[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        write_direct_reservation_v2(
            &accounts[first],
            reservation.bump(),
            endpoint.reservation_post,
            state.root(),
        )?;
        index += 1;
    }
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_direct_selection_v2(
        &accounts[2],
        selection_bump,
        plan.selection,
        state.root(),
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectEconomicTerminalAuthoritySbfV2<'a> {
    root_semantic_id: [u8; 32],
    replay: DirectActionReplayV1,
    selection: &'a DirectSelectionV1,
    endpoints: &'a [Option<DirectEndpointPrestateV1>; 2],
    fee_policy: DirectFeePolicyV2,
    realm: [u8; 32],
    reason: DirectTerminalReasonV1,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectEconomicTerminalV2 for DirectEconomicTerminalAuthoritySbfV2<'_> {
    fn authenticate_terminal_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        selection: DirectSelectionV1,
        ordered_endpoints: &[Option<DirectEndpointPrestateV1>; 2],
        fee_policy: DirectFeePolicyV2,
        realm: [u8; 32],
        batch_policy: Option<&FrozenPolicyV1>,
        revenue_policy: Option<&RevenuePolicyV2>,
        fee_terminal: Option<clutch_direct_market_runtime::fee_v1::DirectFeeTerminalV1>,
        treasury: Option<DirectFeeTreasuryPrestateV1>,
        reason: DirectTerminalReasonV1,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state.root().root_semantic_id() == self.root_semantic_id
            && state.replay() == self.replay
            && selection == *self.selection
            && ordered_endpoints == self.endpoints
            && fee_policy == self.fee_policy
            && realm == self.realm
            && batch_policy.is_none()
            && revenue_policy.is_none()
            && fee_terminal.is_none()
            && treasury.is_none()
            && reason == self.reason
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

const _: () = assert!(
    core::mem::size_of::<DirectEconomicTerminalAuthoritySbfV2<'static>>() <= 512
);

/// Authenticate the current BundleV6, native basis, price policy, Genesis,
/// and canonical grid before b2 may retain a price vector.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_direct_price_precondition_v2(
    program_id: &Pubkey,
    root: &AuthenticatedDirectMarketRootV2,
    bundle_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    price_grid_account: &AccountInfo<'_>,
    reservations: [Option<DirectReservationV1>; 2],
) -> Outcome<AuthenticatedDirectPricePreconditionV2> {
    let transition = root.transition();
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id,
        bundle_account,
        ContentId::from_bytes(transition.compiler_bundle_v6_id()),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        basis_account,
        bundle.value().native_claim_basis_id.content_id(),
    )?;
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_policy_account,
        bundle.value().price_measure_policy_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle.value().market_genesis_profile_id.content_id(),
    )?;
    require(
        bundle.value().price_measure_policy_id.content_id().bytes()
                == transition.price_policy_id()
            && genesis.value().realm_id.bytes() == transition.realm_id()
            && genesis.value().profile_id.bytes() == transition.collateral_profile_id()
            && genesis.value().relation_policy_id.bytes() == transition.relation_policy_id()
            && genesis.value().fee_policy_id.bytes()
                == transition.fee_policy().revenue_policy_v2_digest
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == transition.price_policy_id()
            && basis.value().outcome_count == transition.outcome_count(),
        ClutchError::MismatchedState,
    )?;
    require(
        price_grid_account.owner == program_id
            && !price_grid_account.is_signer
            && !price_grid_account.executable
            && !price_grid_account.is_writable
            && price_grid_account.data_len() == account_len::PRICE_GRID,
        ClutchError::MismatchedState,
    )?;
    let grid_data = price_grid_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let grid = PriceGridAccount::decode(&grid_data)?;
    expect_pda(
        price_grid_account.key,
        seeds::grid_pda(program_id, &grid.realm.0, &grid.grid.0),
        Some(grid.stored_bump),
    )?;
    require(
        grid.realm.0 == transition.realm_id()
            && grid.grid.0 == genesis.value().price_grid_id.bytes()
            && grid.price_scale == transition.price_scale(),
        ClutchError::MismatchedState,
    )?;

    let mut encoded_limits = [[0u8; 16]; 2];
    let mut book = DirectEconomicBookV1 {
        orders: [EMPTY_ECONOMIC_ORDER_V2; 2],
        len: 0,
    };
    let mut index = 0usize;
    while index < reservations.len() {
        if let Some(reservation) = reservations[index] {
            transition
                .child_reservation_semantic_id(reservation, &DirectRuntimeSha256V2)
                .map_err(map_direct_error_v2)?;
            let limit = reservation.limit_price_units_per_egg();
            grid.tick_of(
                u64::try_from(limit)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            )?;
            encoded_limits[index] = limit.to_le_bytes();
            book.orders[index] = reservation.economic_order().map_err(map_direct_error_v2)?;
            book.len = book
                .len
                .checked_add(1)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        index += 1;
    }
    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: transition.market_instance_id(),
        epoch_semantics_digest: transition.direct_epoch_semantics_id(),
        relation_policy_digest: transition.relation_policy_id(),
        price_policy_digest: transition.price_policy_id(),
        epoch_index: transition.direct_window_index().map_err(map_direct_error_v2)?,
        outcome_count: transition.outcome_count(),
        price_scale: transition.price_scale(),
    };
    let price = clutch_direct_market_runtime::selection_v1::canonical_direct_price_precondition_v1(
        &domain,
        &book,
    )
    .map_err(map_direct_error_v2)?;
    let active = usize::from(transition.outcome_count());
    index = 0;
    while index < price.prices.len() {
        if index < active {
            grid.tick_of(price.prices[index])?;
        } else {
            require(price.prices[index] == 0, ClutchError::NonCanonical)?;
        }
        index += 1;
    }
    let price_vector = PriceVectorV3 {
        basis_degree: basis.value().basis_degree,
        native_outcome_count: transition.outcome_count(),
        price_scale: grid.price_scale,
        prices: price.prices,
    };
    price_policy
        .value()
        .validate_candidate_price_contract(basis.value(), &price_vector, grid.price_scale)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let grid_data_id = solana_sha256_hasher::hashv(&[&grid_data[..]]).to_bytes();
    drop(grid_data);
    let authentication_id = solana_sha256_hasher::hashv(&[
        DIRECT_PRICE_AUTHENTICATION_DOMAIN_V2,
        &transition.root_semantic_id(),
        bundle_account.key.as_ref(),
        basis_account.key.as_ref(),
        price_policy_account.key.as_ref(),
        genesis_account.key.as_ref(),
        price_grid_account.key.as_ref(),
        &grid_data_id,
        &encoded_limits[0],
        &encoded_limits[1],
        &price.semantic_price_digest,
    ])
    .to_bytes();
    require_live_id_v2(authentication_id)?;
    Ok(AuthenticatedDirectPricePreconditionV2 {
        domain,
        price,
        authentication_id,
    })
}

/// Authenticate action 2's exact current BundleV6/Genesis/Grid price limit.
/// The Bundle identity comes only from b1/v2; no payload policy identity is an
/// authority coordinate.
#[inline(never)]
fn authenticate_direct_order_limit_v2(
    program_id: &Pubkey,
    root: &AuthenticatedDirectMarketRootV2,
    bundle_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    price_grid_account: &AccountInfo<'_>,
    limit: u128,
) -> Outcome<()> {
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV6>(
        program_id,
        bundle_account,
        ContentId::from_bytes(root.transition().compiler_bundle_v6_id()),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle.value().market_genesis_profile_id.content_id(),
    )?;
    require(
        bundle.value().price_measure_policy_id.content_id().bytes()
            == root.transition().price_policy_id()
            && genesis.value().realm_id.bytes() == root.transition().realm_id()
            && genesis.value().profile_id.bytes() == root.transition().collateral_profile_id()
            && genesis.value().relation_policy_id.bytes()
                == root.transition().relation_policy_id()
            && genesis.value().fee_policy_id.bytes()
                == root.transition().fee_policy().revenue_policy_v2_digest
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == root.transition().price_policy_id(),
        ClutchError::MismatchedState,
    )?;
    require(
        price_grid_account.owner == program_id
            && !price_grid_account.is_signer
            && !price_grid_account.executable
            && !price_grid_account.is_writable
            && price_grid_account.data_len() == account_len::PRICE_GRID,
        ClutchError::MismatchedState,
    )?;
    let grid = PriceGridAccount::decode(&price_grid_account.data.borrow())?;
    expect_pda(
        price_grid_account.key,
        seeds::grid_pda(program_id, &grid.realm.0, &grid.grid.0),
        Some(grid.stored_bump),
    )?;
    let limit = u64::try_from(limit)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    grid.tick_of(limit)?;
    require(
        grid.realm.0 == root.transition().realm_id()
            && grid.grid.0 == genesis.value().price_grid_id.bytes()
            && grid.price_scale == root.transition().price_scale(),
        ClutchError::MismatchedState,
    )
}

/// Authenticate the exact current General V4/Runtime and collateral graph
/// retained by b1/v2. The complete V4 account-data ID makes all Product and
/// Revenue coordinates transitively immutable; a domain-separated current
/// General authority ID then proves they are the same coordinates persisted by
/// this Direct root.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_direct_general_market_v4(
    program_id: &Pubkey,
    root: &AuthenticatedDirectMarketRootV2,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    collateral_policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
) -> Outcome<BoundCollateralProfileV2> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        collateral_policy_account,
        token_program,
    )?;
    let authenticated_market = authenticate_general_market_v4_with_data_ids(
        program_id,
        market_binding_account,
        market_runtime_account,
    )?;
    let market_binding = authenticated_market.binding();
    let market_runtime = authenticated_market.runtime();
    let market = market_binding.base().base();
    let market_instance = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        market.market_instance_v2_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        market_instance.value().market_genesis_profile_id.content_id(),
    )?;
    let current = market_binding.authority();
    let (treasury_replay, _) = seeds::purpose_replay_v3_pda(
        program_id,
        &current.treasury_position_account().bytes(),
        PositionPurposeV3::General,
        &market_runtime_account.key.to_bytes(),
    );
    let direct_general = DirectCurrentGeneralAuthorityV2 {
        general_market_binding_account: market_binding_account.key.to_bytes(),
        general_market_binding_v4_data_id: authenticated_market.binding_data_id().bytes(),
        general_market_runtime_account: market_runtime_account.key.to_bytes(),
        general_market_runtime_data_id: authenticated_market.runtime_data_id().bytes(),
        revenue_policy_record_account: current.revenue_policy_record_account().bytes(),
        revenue_policy_record_v2_id: current.revenue_policy_record_v2_id().bytes(),
        revenue_policy_v2_digest: current.revenue_policy_v2_digest().bytes(),
        treasury_owner: current.treasury_owner().bytes(),
        treasury_position_derivation_policy_v2_id: current
            .treasury_position_derivation_policy_v2_id()
            .bytes(),
        treasury_position_account: current.treasury_position_account().bytes(),
        treasury_replay_account: treasury_replay.to_bytes(),
        treasury_service_ledger_account: current.treasury_service_ledger_account().bytes(),
    };
    let direct_general_id = direct_general
        .semantic_id(&DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)?;
    let binding_rent = market_binding.rent();
    let binding_floor = binding_rent
        .refundable_principal
        .checked_add(binding_rent.donation_floor)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let runtime_floor = market_runtime
        .rent
        .refundable_principal
        .checked_add(market_runtime.rent.donation_floor)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let release_id = realm
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        direct_general_id == root.transition().current_general_authority_id()
            && market_binding_account.key.to_bytes()
                == root.transition().general_market_binding_account()
            && market_runtime_account.key.to_bytes()
                == root.transition().general_market_runtime_account()
            && market.market_instance_v2_id.bytes() == root.transition().market_instance_id()
            && market.outcome_count == root.transition().outcome_count()
            && market.relation_policy_id.bytes() == root.transition().relation_policy_id()
            && market.price_measure_policy_v1_id.bytes() == root.transition().price_policy_id()
            && market.neutral_sink.bytes() == root.transition().neutral_lamport_sink()
            && market.price_scale == root.transition().price_scale()
            && market_binding.base().batch_policy_id().bytes()
                == root.transition().fee_policy().batch_policy_id
            && current.product_generation() == root.transition().generation()
            && current.revenue_policy_v2_digest().bytes()
                == root.transition().fee_policy().revenue_policy_v2_digest
            && current.revenue_policy_record_v2_id().bytes()
                == root.transition().fee_policy().revenue_policy_record_v2_id
            && current.treasury_owner().bytes()
                == root.transition().fee_policy().treasury_owner
            && current.treasury_position_derivation_policy_v2_id().bytes()
                == root
                    .transition()
                    .fee_policy()
                    .treasury_position_derivation_policy_v2_id
            && genesis.value().realm_id.bytes() == root.transition().realm_id()
            && genesis.value().profile_id.bytes() == root.transition().collateral_profile_id()
            && genesis.value().relation_policy_id.bytes()
                == root.transition().relation_policy_id()
            && genesis.value().fee_policy_id.bytes()
                == root.transition().fee_policy().revenue_policy_v2_digest
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == root.transition().price_policy_id()
            && realm.policy_id().bytes() == root.transition().collateral_policy_id()
            && release_id.bytes() == root.transition().collateral_release_id()
            && market_runtime.market_instance_v2_id == market.market_instance_v2_id
            && market_binding_account.lamports() >= binding_floor
            && market_runtime_account.lamports() >= runtime_floor
            && market_instance
                .value()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == root.transition().market_instance_id(),
        ClutchError::MismatchedState,
    )?;
    let market_bytes = root.transition().market_instance_id();
    refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market_bytes),
            realm: CollateralId::from_bytes(root.transition().realm_id()),
            profile: CollateralId::from_bytes(root.transition().collateral_profile_id()),
            collateral_cap_atoms: market_instance.value().collateral_cap,
            hoard_authority: CollateralId::from_bytes(
                seeds::hoard_authority_v2_pda(program_id, &market_bytes).0.to_bytes(),
            ),
            hoard_token_account: CollateralId::from_bytes(
                seeds::hoard_token_v2_pda(program_id, &market_bytes).0.to_bytes(),
            ),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn require_direct_endpoint_alias_contract_v2(
    accounts: &[AccountInfo<'_>],
    fixed_count: usize,
    endpoint_count: usize,
) -> Outcome<()> {
    let mut index = 0usize;
    while index < endpoint_count {
        let first = direct_endpoint_first_from_v2(fixed_count, index)?;
        let mut fixed = 0usize;
        while fixed < fixed_count {
            require(
                accounts[first].key != accounts[fixed].key
                    && accounts[first + 1].key != accounts[fixed].key
                    && accounts[first + 2].key != accounts[fixed].key,
                ClutchError::AccountAlias,
            )?;
            fixed += 1;
        }
        require(
            accounts[first].key != accounts[first + 1].key
                && accounts[first].key != accounts[first + 2].key
                && accounts[first + 1].key != accounts[first + 2].key,
            ClutchError::AccountAlias,
        )?;
        index += 1;
    }
    if endpoint_count == 2 {
        let left = fixed_count;
        let right = fixed_count
            .checked_add(3)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
        let positions_alias = accounts[left + 1].key == accounts[right + 1].key;
        let replays_alias = accounts[left + 2].key == accounts[right + 2].key;
        require(positions_alias == replays_alias, ClutchError::AccountAlias)?;
        require(
            accounts[left].key != accounts[right + 1].key
                && accounts[left].key != accounts[right + 2].key
                && accounts[right].key != accounts[left + 1].key
                && accounts[right].key != accounts[left + 2].key
                && accounts[left + 1].key != accounts[right + 2].key
                && accounts[left + 2].key != accounts[right + 1].key,
            ClutchError::AccountAlias,
        )?;
    }
    Ok(())
}

fn direct_endpoint_first_from_v2(base: usize, index: usize) -> Outcome<usize> {
    index
        .checked_mul(3)
        .and_then(|offset| offset.checked_add(base))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))
}

/// Execute actions 6 and 7 with the exact four-account Candidate suffix.
#[inline(never)]
fn process_direct_candidate_verification_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, 8)?;
    require(payload.is_empty(), ClutchError::WrongDataLength)?;
    require_distinct(&accounts[..4])?;
    require_direct_candidate_liveness_aliases_v2(accounts, 4, 4)?;
    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v2(
        program_id,
        &accounts[2],
        &root,
    )?;
    let observed_slot = read_clock_slot(&accounts[3])?;
    let root_bump = root.bump();
    let replay_bump = replay.bump();
    let selection_bump = selection.bump();
    let selection_balance = selection.observed_lamports();
    let mut selection_value = selection.into_value();
    let accounted_selection_balance = {
        let rent = selection_value.rent();
        let bond = root
            .transition()
            .outstanding_candidate_bond_lamports(*selection_value)
            .map_err(map_direct_error_v2)?;
        rent.principal_lamports
            .checked_add(rent.donation_floor_lamports)
            .and_then(|value| value.checked_add(bond))
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
    };
    require(
        selection_balance >= accounted_selection_balance,
        ClutchError::MismatchedState,
    )?;
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let effects = match action {
        DirectMarketAction::BeginVerification => begin_direct_candidate_verification_v2(
            &mut state,
            &mut selection_value,
            sequence,
            observed_slot,
            &DirectRuntimeSha256V2,
        ),
        DirectMarketAction::VerifyCandidate => verify_next_direct_candidate_v2(
            &mut state,
            &mut selection_value,
            sequence,
            observed_slot,
            &DirectRuntimeSha256V2,
        ),
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    }
    .map_err(map_direct_error_v2)?;
    require(
        effects.candidate_bond_movement.is_none()
            && effects.candidate_bond_refunds.is_none()
            && accounts[2].lamports() == selection_balance,
        ClutchError::MismatchedState,
    )?;
    let runtime_action = match action {
        DirectMarketAction::BeginVerification => DirectMarketActionV1::BeginVerification,
        DirectMarketAction::VerifyCandidate => DirectMarketActionV1::VerifyCandidate,
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    };
    apply_direct_candidate_work_v2(
        program_id,
        &accounts[4..],
        &accounts[1],
        &mut state,
        &selection_value,
        runtime_action,
    )?;
    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_direct_selection_v2(
        &accounts[2],
        selection_bump,
        *selection_value,
        state.root(),
    )
}

/// Execute action 5 against exact current root/replay/Selection state.
///
/// Accounts are b1/v2 root W, b3 replay W, b2 Selection W, Clock RO,
/// submitter signer W, System program, and an optional exact evicted bond
/// refund owner W. No Product or fee authority is supplied by the caller.
#[inline(never)]
fn process_direct_submit_candidate_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        accounts.len() == 6 || accounts.len() == 7,
        ClutchError::AccountCount,
    )?;
    require_distinct(&accounts[..4])?;
    require_signer(&accounts[4])?;
    require(accounts[4].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[5])?;
    let mut fixed = 0usize;
    while fixed < 4 {
        require(
            accounts[4].key != accounts[fixed].key
                && accounts[5].key != accounts[fixed].key,
            ClutchError::AccountAlias,
        )?;
        fixed += 1;
    }

    let root = authenticate_direct_market_root_writable_v2(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v2(
        program_id,
        &accounts[1],
        &root,
    )?;
    let selection = authenticate_direct_selection_writable_v2(
        program_id,
        &accounts[2],
        &root,
    )?;
    let observed_slot = read_clock_slot(&accounts[3])?;
    let candidate = DirectSubmitCandidatePayloadV1::decode(payload)?.candidate;
    let root_bump = root.bump();
    let replay_bump = replay.bump();
    let selection_bump = selection.bump();
    let selection_balance_before = selection.observed_lamports();
    let mut selection_value = selection.into_value();
    let bond_principal_before = root
        .transition()
        .outstanding_candidate_bond_lamports(*selection_value)
        .map_err(map_direct_error_v2)?;
    let selection_rent = selection_value.rent();
    let accounted_balance_before = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_before))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        selection_balance_before >= accounted_balance_before,
        ClutchError::MismatchedState,
    )?;
    let mut state = DirectRootReplayTransitionV2::authenticate(
        root.into_transition(),
        replay.value(),
    )
    .map_err(map_direct_error_v2)?;
    let effects = submit_direct_candidate_v2(
        &mut state,
        &mut selection_value,
        sequence,
        observed_slot,
        candidate,
        accounts[4].key.to_bytes(),
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)?;

    let expected_selection_balance = match effects.candidate_bond_movement {
        Some(movement) => {
            let expected_count = if movement.evicted_refund_lamports == 0 { 6 } else { 7 };
            require_count(accounts, expected_count)?;
            require(
                movement.incoming_payer == accounts[4].key.to_bytes()
                    && movement.principal_before_lamports == bond_principal_before
                    && movement.principal_after_lamports
                        == state
                            .root()
                            .outstanding_candidate_bond_lamports(*selection_value)
                            .map_err(map_direct_error_v2)?,
                ClutchError::MismatchedState,
            )?;
            if movement.evicted_refund_lamports != 0 {
                require(
                    accounts[6].is_writable
                        && !accounts[6].executable
                        && accounts[6].key.to_bytes() == movement.evicted_refund_recipient,
                    ClutchError::MismatchedState,
                )?;
                let mut index = 0usize;
                while index < 6 {
                    if index != 4 {
                        require(
                            accounts[6].key != accounts[index].key,
                            ClutchError::AccountAlias,
                        )?;
                    }
                    index += 1;
                }
            }
            transfer_signer_lamports_v2(
                &accounts[4],
                &accounts[2],
                &accounts[5],
                movement.incoming_lamports,
            )?;
            if movement.evicted_refund_lamports != 0 {
                debit_lamports_v2(&accounts[2], movement.evicted_refund_lamports)?;
                credit_lamports_v2(&accounts[6], movement.evicted_refund_lamports)?;
            }
            selection_balance_before
                .checked_add(movement.incoming_lamports)
                .and_then(|value| value.checked_sub(movement.evicted_refund_lamports))
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
        }
        None => {
            require_count(accounts, 6)?;
            selection_balance_before
        }
    };
    require(
        accounts[2].lamports() == expected_selection_balance,
        ClutchError::MismatchedState,
    )?;
    let bond_principal_after = state
        .root()
        .outstanding_candidate_bond_lamports(*selection_value)
        .map_err(map_direct_error_v2)?;
    let accounted_balance_after = selection_rent
        .principal_lamports
        .checked_add(selection_rent.donation_floor_lamports)
        .and_then(|value| value.checked_add(bond_principal_after))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        accounts[2].lamports() >= accounted_balance_after,
        ClutchError::MismatchedState,
    )?;

    write_direct_market_root_v2(&accounts[0], root_bump, state.root())?;
    write_direct_action_replay_v2(
        &accounts[1],
        replay_bump,
        state.replay(),
        state.root(),
    )?;
    write_direct_selection_v2(
        &accounts[2],
        selection_bump,
        *selection_value,
        state.root(),
    )
}

/// Stream one exact current Direct work batch through the shared Candidate.
/// No caller ordinal, role, work amount, or receipt identity is accepted.
#[inline(never)]
fn apply_direct_candidate_work_v2(
    program_id: &Pubkey,
    liveness_accounts: &[AccountInfo<'_>],
    receipt_account: &AccountInfo<'_>,
    state: &mut DirectRootReplayTransitionV2,
    selection: &DirectSelectionV1,
    action: DirectMarketActionV1,
) -> Outcome<()> {
    require_count(
        liveness_accounts,
        DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2,
    )?;
    let policy_account = &liveness_accounts[0];
    let candidate_account = &liveness_accounts[1];
    let keeper = &liveness_accounts[2];
    let payer = &liveness_accounts[3];
    let root = state.root();
    let candidate_binding = root.candidate_liveness();
    require(
        policy_account.key.to_bytes() == candidate_binding.policy_account
            && policy_account.owner == program_id
            && !policy_account.is_writable
            && !policy_account.is_signer
            && !policy_account.executable
            && policy_account.data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1
            && candidate_account.key.to_bytes() == candidate_binding.candidate_account
            && candidate_account.owner == program_id
            && candidate_account.is_writable
            && !candidate_account.is_signer
            && !candidate_account.executable
            && candidate_account.data_len() == RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
            && keeper.is_writable
            && keeper.is_signer
            && !keeper.executable
            && payer.is_writable
            && (payer.key == keeper.key || !payer.is_signer)
            && !payer.executable
            && receipt_account.key.to_bytes() == root.action_replay_account()
            && receipt_account.owner == program_id,
        ClutchError::MismatchedState,
    )?;
    require(
        policy_account.key != candidate_account.key
            && policy_account.key != keeper.key
            && policy_account.key != payer.key
            && policy_account.key != receipt_account.key
            && candidate_account.key != keeper.key
            && candidate_account.key != payer.key
            && candidate_account.key != receipt_account.key
            && keeper.key != receipt_account.key
            && payer.key != receipt_account.key,
        ClutchError::AccountAlias,
    )?;

    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_data_id = solana_sha256_hasher::hashv(&[&policy_data[..]]).to_bytes();
    require(
        policy_data_id == candidate_binding.policy_data_id,
        ClutchError::MismatchedState,
    )?;
    let mut candidate_data = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    {
        let data = candidate_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        candidate_data.copy_from_slice(&data);
    }
    let candidate_pre_data_id =
        solana_sha256_hasher::hashv(&[&candidate_data[..]]).to_bytes();
    let (candidate_completed_calls, candidate_last_receipt_id) = {
        let candidate_state = RuntimeCompartmentV1::decode(&candidate_data)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            candidate_state.kind == RuntimeCompartmentKindV1::Candidate
                && candidate_state.identity.policy_id.bytes()
                    == root.candidate_liveness_policy_id()
                && candidate_state.identity.lifecycle_id.bytes()
                    == candidate_binding.global_lifecycle_id
                && candidate_state.identity.account_id.bytes()
                    == candidate_binding.candidate_account
                && candidate_state.identity.owner.bytes()
                    == candidate_binding.candidate_semantic_owner
                && candidate_state.identity.payer.bytes() == payer.key.to_bytes()
                && candidate_state.identity.neutral_sink.bytes()
                    == root.neutral_lamport_sink()
                && candidate_state.identity.generation == candidate_binding.candidate_generation
                && candidate_state.quote_schedule_id.bytes()
                    == candidate_binding.candidate_quote_schedule_id
                && candidate_state.receipt_program_id.bytes()
                    == candidate_binding.candidate_receipt_program_id
                && candidate_state.receipt_program_id.bytes() == program_id.to_bytes()
                && (state.replay().candidate_liveness_completed_calls() != 0
                    || candidate_pre_data_id == candidate_binding.candidate_data_id),
            ClutchError::MismatchedState,
        )?;
        (
            candidate_state.completed_calls,
            candidate_state.last_work_receipt_id.bytes(),
        )
    };
    let batch = prepare_direct_candidate_work_batch_v2(
        state,
        Some(selection),
        action,
        candidate_completed_calls,
        candidate_last_receipt_id,
        candidate_pre_data_id,
        keeper.key.to_bytes(),
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)?;
    apply_direct_candidate_batch_v2(
        program_id,
        policy_account,
        candidate_account,
        keeper,
        payer,
        receipt_account,
        &policy_data,
        &mut candidate_data,
        candidate_binding,
        root.candidate_liveness_policy_id(),
        batch,
    )?;
    bind_direct_candidate_work_batch_v2(state, batch, &DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)
}

/// Apply the already-derived bounded work batch and all exact lamport flows.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn apply_direct_candidate_batch_v2(
    program_id: &Pubkey,
    policy_account: &AccountInfo<'_>,
    candidate_account: &AccountInfo<'_>,
    keeper: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    receipt_account: &AccountInfo<'_>,
    policy_data: &[u8],
    candidate_data: &mut [u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1],
    candidate_binding: clutch_direct_market_runtime::liveness_v1::DirectCandidateLivenessBindingV1,
    candidate_liveness_policy_id: [u8; 32],
    batch: DirectCandidateWorkBatchV1,
) -> Outcome<()> {
    let expected_program = LivenessId::from_bytes(program_id.to_bytes());
    let expected_policy_account = LivenessId::from_bytes(policy_account.key.to_bytes());
    let mut account_balance = candidate_account.lamports();
    let mut keeper_total = 0u64;
    let mut payer_total = 0u64;
    let receipt_count = batch.receipt_count();
    let mut index = 0u8;
    while index < receipt_count {
        let receipt = batch
            .receipt(index, candidate_binding, &DirectRuntimeSha256V2)
            .map_err(map_direct_error_v2)?;
        let account_balance_after = account_balance
            .checked_sub(receipt.call_ceiling_lamports())
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        let intent = RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Candidate,
            policy_id: LivenessId::from_bytes(candidate_liveness_policy_id),
            lifecycle_id: LivenessId::from_bytes(candidate_binding.global_lifecycle_id),
            account_id: LivenessId::from_bytes(candidate_binding.candidate_account),
            semantic_owner: LivenessId::from_bytes(candidate_binding.candidate_semantic_owner),
            quote_schedule_id: LivenessId::from_bytes(
                candidate_binding.candidate_quote_schedule_id,
            ),
            receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
            keeper: LivenessId::from_bytes(keeper.key.to_bytes()),
            generation: candidate_binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
            keeper_payment_lamports: receipt.keeper_payment_lamports(),
            flags: 0,
        };
        let observation = RuntimeReceiptObservationV1 {
            receipt_account_id: LivenessId::from_bytes(receipt_account.key.to_bytes()),
            receipt_account_owner_program_id: expected_program,
            receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
            receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
            compartment_kind: RuntimeCompartmentKindV1::Candidate,
            semantic_owner: LivenessId::from_bytes(candidate_binding.candidate_semantic_owner),
            lifecycle_id: LivenessId::from_bytes(candidate_binding.global_lifecycle_id),
            quote_schedule_id: LivenessId::from_bytes(
                candidate_binding.candidate_quote_schedule_id,
            ),
            generation: candidate_binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
        };
        let transition = plan_runtime_transition_v1(
            expected_program,
            expected_policy_account,
            RuntimePersistedAccountViewV1 {
                account_id: expected_policy_account,
                owner_program_id: LivenessId::from_bytes(policy_account.owner.to_bytes()),
                lamports: policy_account.lamports(),
                data: policy_data,
                writable: policy_account.is_writable,
            },
            RuntimePersistedAccountViewV1 {
                account_id: LivenessId::from_bytes(candidate_account.key.to_bytes()),
                owner_program_id: LivenessId::from_bytes(candidate_account.owner.to_bytes()),
                lamports: account_balance,
                data: candidate_data,
                writable: candidate_account.is_writable,
            },
            intent,
            Some(observation),
            account_balance_after,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            transition.write_account_data
                && !transition.close_account
                && transition.account_balance_before == account_balance
                && transition.account_balance_after == account_balance_after,
            ClutchError::MismatchedState,
        )?;
        for movement in transition.transfers() {
            match movement.role {
                RuntimeTransferRoleV1::KeeperPayment => {
                    require(
                        movement.destination == LivenessId::from_bytes(keeper.key.to_bytes()),
                        ClutchError::MismatchedState,
                    )?;
                    keeper_total = keeper_total
                        .checked_add(movement.lamports)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
                }
                RuntimeTransferRoleV1::PayerWorkRefund => {
                    require(
                        movement.destination == LivenessId::from_bytes(payer.key.to_bytes()),
                        ClutchError::MismatchedState,
                    )?;
                    payer_total = payer_total
                        .checked_add(movement.lamports)
                        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
                }
                _ => return Err(Refusal::Adapter(ClutchError::MismatchedState)),
            }
        }
        candidate_data.copy_from_slice(&transition.post_account_data);
        account_balance = account_balance_after;
        index = index
            .checked_add(1)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    require(
        keeper_total == batch.total_keeper_payment_lamports()
            && payer_total == batch.total_payer_refund_lamports()
            && candidate_account
                .lamports()
                .checked_sub(account_balance)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
                == batch.total_call_ceiling_lamports(),
        ClutchError::MismatchedState,
    )?;
    let coalesced_recipient = keeper.key == payer.key;
    let keeper_after = keeper
        .lamports()
        .checked_add(keeper_total)
        .and_then(|balance| {
            if coalesced_recipient {
                balance.checked_add(payer_total)
            } else {
                Some(balance)
            }
        })
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let payer_after = if coalesced_recipient {
        keeper_after
    } else {
        payer
            .lamports()
            .checked_add(payer_total)
            .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?
    };
    {
        let mut data = candidate_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data.copy_from_slice(candidate_data);
    }
    {
        let mut candidate_lamports = candidate_account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **candidate_lamports = account_balance;
    }
    if coalesced_recipient {
        let mut recipient_lamports = keeper
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **recipient_lamports = keeper_after;
    } else {
        let mut keeper_lamports = keeper
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut payer_lamports = payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **keeper_lamports = keeper_after;
        **payer_lamports = payer_after;
    }
    Ok(())
}

fn require_direct_candidate_liveness_aliases_v2(
    accounts: &[AccountInfo<'_>],
    liveness_start: usize,
    recipient_start: usize,
) -> Outcome<()> {
    require(
        recipient_start <= liveness_start
            && accounts.len()
                == liveness_start
                    .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::AccountCount,
    )?;
    let policy = &accounts[liveness_start];
    let candidate = &accounts[liveness_start + 1];
    let keeper = &accounts[liveness_start + 2];
    let payer = &accounts[liveness_start + 3];
    let mut index = 0usize;
    while index < liveness_start {
        require(
            policy.key != accounts[index].key
                && candidate.key != accounts[index].key
                && (keeper.key != accounts[index].key || index >= recipient_start)
                && (payer.key != accounts[index].key || index >= recipient_start),
            ClutchError::AccountAlias,
        )?;
        index += 1;
    }
    Ok(())
}

/// Action 4 allows the liveness keeper and payer to coalesce only with the
/// immutable Selection-creation payer at index 3. Policy and Candidate never
/// alias the semantic prefix, and no Reservation can become a recipient.
fn require_direct_freeze_liveness_aliases_v2(
    accounts: &[AccountInfo<'_>],
    liveness_start: usize,
) -> Outcome<()> {
    require(
        accounts.len()
            == liveness_start
                .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::AccountCount,
    )?;
    let policy = &accounts[liveness_start];
    let candidate = &accounts[liveness_start + 1];
    let keeper = &accounts[liveness_start + 2];
    let payer = &accounts[liveness_start + 3];
    let creation_payer = accounts[3].key;
    let mut index = 0usize;
    while index < liveness_start {
        require(
            policy.key != accounts[index].key
                && candidate.key != accounts[index].key
                && (keeper.key != accounts[index].key
                    || accounts[index].key == creation_payer)
                && (payer.key != accounts[index].key
                    || accounts[index].key == creation_payer),
            ClutchError::AccountAlias,
        )?;
        index += 1;
    }
    Ok(())
}

#[derive(Debug)]
struct AuthenticatedDirectMarketRootV2 {
    account: Pubkey,
    transition: AuthenticatedDirectRootTransitionV2,
    bump: u8,
    data_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectMarketRootV2 {
    const fn account(&self) -> Pubkey { self.account }
    const fn bump(&self) -> u8 { self.bump }
    const fn data_id(&self) -> [u8; 32] { self.data_id }
    const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    const fn transition(&self) -> &AuthenticatedDirectRootTransitionV2 { &self.transition }
    fn into_transition(self) -> AuthenticatedDirectRootTransitionV2 { self.transition }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDirectActionReplayV2 {
    value: DirectActionReplayV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectActionReplayV2 {
    const fn value(self) -> DirectActionReplayV1 { self.value }
    const fn bump(self) -> u8 { self.bump }
}

#[derive(Debug)]
struct AuthenticatedDirectSelectionV2 {
    value: Box<DirectSelectionV1>,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectSelectionV2 {
    fn value(&self) -> &DirectSelectionV1 { &self.value }
    const fn bump(&self) -> u8 { self.bump }
    const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    fn into_value(self) -> Box<DirectSelectionV1> { self.value }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDirectReservationV2 {
    account: Pubkey,
    value: DirectReservationV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectReservationV2 {
    const fn account(self) -> Pubkey { self.account }
    const fn value(self) -> DirectReservationV1 { self.value }
    const fn bump(self) -> u8 { self.bump }
    const fn semantic_id(self) -> [u8; 32] { self.semantic_id }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectAccountAccessV2 {
    ReadOnly,
    Writable,
}

impl DirectAccountAccessV2 {
    const fn writable(self) -> bool { matches!(self, Self::Writable) }
}

#[inline(never)]
fn authenticate_direct_market_root_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedDirectMarketRootV2> {
    require_program_state_v2(
        program_id,
        account,
        DirectAccountAccessV2::Writable,
        DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = DirectMarketRootAccountV2::decode(&data)?;
    let transition = authenticate_direct_root_transition_body_v2(
        frame.semantic_body(),
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)?;
    let (expected, bump) = seeds::direct_market_root_v2_pda(
        program_id,
        &transition.market_instance_id(),
        transition.generation(),
    );
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        transition.direct_root_account() == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    let rent = transition.root_rent();
    require_rent_coverage_v2(
        rent.principal_lamports,
        rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    require_live_id_v2(data_id)?;
    drop(data);
    Ok(AuthenticatedDirectMarketRootV2 {
        account: *account.key,
        transition,
        bump,
        data_id,
        observed_lamports,
    })
}

#[inline(never)]
fn authenticate_direct_action_replay_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV2,
) -> Outcome<AuthenticatedDirectActionReplayV2> {
    require_program_state_v2(
        program_id,
        account,
        DirectAccountAccessV2::Writable,
        DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (bump, body) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
        DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
        DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
    )?;
    let value = decode_direct_action_replay_body_for_transition_v2(body, root.transition())
        .map_err(map_direct_error_v2)?;
    let (expected, expected_bump) =
        seeds::direct_action_replay_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, expected_bump), Some(bump))?;
    require(
        account.key.to_bytes() == root.transition().action_replay_account(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    let rent = value.rent();
    require_rent_coverage_v2(
        rent.principal_lamports,
        rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    let semantic_id = root
        .transition()
        .action_replay_semantic_id(value, &DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)?;
    require_live_id_v2(data_id)?;
    drop(data);
    Ok(AuthenticatedDirectActionReplayV2 {
        value,
        bump,
        data_id,
        semantic_id,
        observed_lamports,
    })
}

#[inline(never)]
fn authenticate_direct_selection_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV2,
) -> Outcome<AuthenticatedDirectSelectionV2> {
    require_program_state_v2(
        program_id,
        account,
        DirectAccountAccessV2::Writable,
        DIRECT_SELECTION_ACCOUNT_BYTES,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (bump, body) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_SELECTION_ACCOUNT_TAG,
        DIRECT_SELECTION_ACCOUNT_VERSION,
        DIRECT_SELECTION_BODY_BYTES_V1,
    )?;
    let value = Box::new(
        decode_direct_selection_body_for_transition_v2(body, root.transition())
            .map_err(map_direct_error_v2)?,
    );
    let (expected, expected_bump) =
        seeds::direct_selection_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, expected_bump), Some(bump))?;
    require(
        value.account() == account.key.to_bytes()
            && root.transition().selection_account() == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    let rent = value.rent();
    require_rent_coverage_v2(
        rent.principal_lamports,
        rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    let semantic_id = root
        .transition()
        .selection_semantic_id(*value, &DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)?;
    require_live_id_v2(data_id)?;
    drop(data);
    Ok(AuthenticatedDirectSelectionV2 {
        value,
        bump,
        data_id,
        semantic_id,
        observed_lamports,
    })
}

#[inline(never)]
fn authenticate_direct_reservation_writable_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV2,
) -> Outcome<AuthenticatedDirectReservationV2> {
    authenticate_direct_reservation_with_access_v2(
        program_id,
        account,
        root,
        DirectAccountAccessV2::Writable,
    )
}

#[inline(never)]
fn authenticate_direct_reservation_readonly_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV2,
) -> Outcome<AuthenticatedDirectReservationV2> {
    authenticate_direct_reservation_with_access_v2(
        program_id,
        account,
        root,
        DirectAccountAccessV2::ReadOnly,
    )
}

#[inline(never)]
fn authenticate_direct_reservation_with_access_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: &AuthenticatedDirectMarketRootV2,
    access: DirectAccountAccessV2,
) -> Outcome<AuthenticatedDirectReservationV2> {
    require_program_state_v2(
        program_id,
        account,
        access,
        DIRECT_RESERVATION_ACCOUNT_BYTES,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (bump, body) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_RESERVATION_ACCOUNT_TAG,
        DIRECT_RESERVATION_ACCOUNT_VERSION,
        DIRECT_RESERVATION_BODY_BYTES_V1,
    )?;
    let value = decode_direct_reservation_body_for_transition_v2(body, root.transition())
        .map_err(map_direct_error_v2)?;
    let (expected, expected_bump) = seeds::direct_reservation_v1_pda(
        program_id,
        &root.account(),
        &value.order_id,
    );
    expect_pda(account.key, (expected, expected_bump), Some(bump))?;
    require(
        value.reservation_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v2(
        value.rent.principal_lamports,
        value.rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    let semantic_id = root
        .transition()
        .child_reservation_semantic_id(value, &DirectRuntimeSha256V2)
        .map_err(map_direct_error_v2)?;
    require_live_id_v2(data_id)?;
    drop(data);
    Ok(AuthenticatedDirectReservationV2 {
        account: *account.key,
        value,
        bump,
        data_id,
        semantic_id,
        observed_lamports,
    })
}

fn decode_borrowed_child_frame_v2<'a>(
    input: &'a [u8],
    expected_tag: u8,
    expected_version: u8,
    expected_body_len: usize,
) -> Outcome<(u8, &'a [u8])> {
    let expected_len = expected_body_len
        .checked_add(4)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(input.len() == expected_len, ClutchError::WrongDataLength)?;
    require(
        input[0] == expected_tag
            && input[1] == expected_version
            && input[3] == 0
            && input[4..].iter().any(|byte| *byte != 0),
        ClutchError::MismatchedState,
    )?;
    Ok((input[2], &input[4..]))
}

fn write_direct_market_root_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let frame = DirectMarketRootAccountV2::decode(&data)?;
    require(frame.bump() == bump, ClutchError::MismatchedState)?;
    write_direct_root_transition_body_v2(
        transition,
        &mut data[4..],
        &DirectRuntimeSha256V2,
    )
    .map_err(map_direct_error_v2)
}

fn write_direct_action_replay_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectActionReplayV1,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (observed_bump, _) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
        DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
        DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
    )?;
    require(observed_bump == bump, ClutchError::MismatchedState)?;
    encode_direct_action_replay_body_into_transition_v2(value, transition, &mut data[4..])
        .map_err(map_direct_error_v2)
}

fn write_direct_selection_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectSelectionV1,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (observed_bump, _) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_SELECTION_ACCOUNT_TAG,
        DIRECT_SELECTION_ACCOUNT_VERSION,
        DIRECT_SELECTION_BODY_BYTES_V1,
    )?;
    require(observed_bump == bump, ClutchError::MismatchedState)?;
    encode_direct_selection_body_into_transition_v2(value, transition, &mut data[4..])
        .map_err(map_direct_error_v2)
}

fn write_fresh_direct_selection_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectSelectionV1,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    require(
        account.is_writable
            && !account.executable
            && account.data_len() == DIRECT_SELECTION_ACCOUNT_BYTES,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data[0] = DIRECT_SELECTION_ACCOUNT_TAG;
    data[1] = DIRECT_SELECTION_ACCOUNT_VERSION;
    data[2] = bump;
    data[3] = 0;
    encode_direct_selection_body_into_transition_v2(value, transition, &mut data[4..])
        .map_err(map_direct_error_v2)
}

fn write_direct_reservation_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectReservationV1,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (observed_bump, _) = decode_borrowed_child_frame_v2(
        &data,
        DIRECT_RESERVATION_ACCOUNT_TAG,
        DIRECT_RESERVATION_ACCOUNT_VERSION,
        DIRECT_RESERVATION_BODY_BYTES_V1,
    )?;
    require(observed_bump == bump, ClutchError::MismatchedState)?;
    encode_direct_reservation_body_into_transition_v2(value, transition, &mut data[4..])
        .map_err(map_direct_error_v2)
}

fn write_fresh_direct_reservation_v2(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectReservationV1,
    transition: &AuthenticatedDirectRootTransitionV2,
) -> Outcome<()> {
    require(
        account.is_writable
            && !account.executable
            && account.data_len() == DIRECT_RESERVATION_ACCOUNT_BYTES,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data[0] = DIRECT_RESERVATION_ACCOUNT_TAG;
    data[1] = DIRECT_RESERVATION_ACCOUNT_VERSION;
    data[2] = bump;
    data[3] = 0;
    encode_direct_reservation_body_into_transition_v2(value, transition, &mut data[4..])
        .map_err(map_direct_error_v2)
}

#[inline(never)]
fn write_position_post_v2(
    account: &AccountInfo<'_>,
    post: &PositionSettlementPoststateV3,
) -> Outcome<()> {
    let body = post
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        account.is_writable && !account.executable && account.data_len() == body.len(),
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(&body);
    Ok(())
}

#[inline(never)]
fn write_general_replay_post_v2(
    account: &AccountInfo<'_>,
    post: &GeneralReplayTransitionPlanV1,
) -> Outcome<()> {
    let body = post.replay_poststate_body();
    require(
        account.is_writable && !account.executable && account.data_len() == body.len(),
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(body);
    Ok(())
}

fn require_program_state_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    access: DirectAccountAccessV2,
    expected_len: usize,
) -> Outcome<()> {
    require(
        !account.is_signer
            && !account.executable
            && account.owner == program_id
            && account.is_writable == access.writable()
            && account.data_len() == expected_len,
        ClutchError::MismatchedState,
    )
}

fn require_rent_coverage_v2(principal: u64, donation_floor: u64, observed: u64) -> Outcome<()> {
    let floor = principal
        .checked_add(donation_floor)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(principal != 0 && observed >= floor, ClutchError::MismatchedState)
}

fn require_live_id_v2(id: [u8; 32]) -> Outcome<()> {
    require(id != [0; 32], ClutchError::MismatchedState)
}

fn authenticate_fresh_direct_pda_v2(
    account: &AccountInfo<'_>,
    expected: (Pubkey, u8),
) -> Outcome<u64> {
    expect_pda(account.key, expected, None)?;
    require(
        !account.is_signer
            && account.is_writable
            && !account.executable
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.data_len() == 0,
        ClutchError::AlreadyInitialized,
    )?;
    Ok(account.lamports())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn create_current_direct_account_v2<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    principal_lamports: u64,
    donation_floor_lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require_system_program(system_program)?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE
            && principal_lamports == rent.minimum_balance(space)?
            && target.lamports() == donation_floor_lamports,
        ClutchError::MismatchedState,
    )?;
    let expected_balance = donation_floor_lamports
        .checked_add(principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal_lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, false)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, false)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner == program_id
            && target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )
}

fn close_direct_program_account_v2(
    account: &AccountInfo<'_>,
    observed_lamports: u64,
) -> Outcome<()> {
    require(
        account.is_writable
            && account.owner.to_bytes() != SYSTEM_PROGRAM_ID
            && account.lamports() == observed_lamports,
        ClutchError::MismatchedState,
    )?;
    {
        let mut lamports = account
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **lamports = 0;
    }
    account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    account.assign(&SYSTEM_PROGRAM_ID);
    require(
        account.lamports() == 0
            && account.data_len() == 0
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )
}

fn transfer_signer_lamports_v2<'a>(
    payer: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    amount: u64,
) -> Outcome<()> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(destination.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(amount),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), destination.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))
}

fn credit_lamports_v2(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_add(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn debit_lamports_v2(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = lamports
        .checked_sub(amount)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(())
}

fn map_direct_error_v2(error: DirectMarketErrorV1) -> Refusal {
    let adapter = match error {
        DirectMarketErrorV1::Arithmetic => ClutchError::Arithmetic,
        DirectMarketErrorV1::Replay => ClutchError::Replay,
        DirectMarketErrorV1::WrongPhase => ClutchError::NotActive,
        DirectMarketErrorV1::UnauthenticatedAuthority => ClutchError::AuthorizationUnavailable,
        _ => ClutchError::MismatchedState,
    };
    Refusal::Adapter(adapter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_action_group_has_exact_frame_bounds() {
        assert_eq!(DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2, 2_502);
        assert_eq!(DIRECT_SELECTION_ACCOUNT_BYTES, 1_629);
        assert_eq!(DIRECT_ACTION_REPLAY_ACCOUNT_BYTES, 394);
        assert_eq!(DIRECT_RESERVATION_ACCOUNT_BYTES, 473);
        assert!(7 <= DIRECT_MARKET_V2_MAX_ACCOUNTS);
        assert_eq!(4 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2, 8);
        assert_eq!(12 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2, 16);
        assert_eq!(12 + (2 * 3) + 3 + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2, 25);
        assert!(25 <= DIRECT_MARKET_V2_MAX_ACCOUNTS);
    }

    #[test]
    fn reservation_action_frames_are_structurally_hostile() {
        // Action 2 derives the sole optional peer from b1/v2's live count;
        // action 3 has no caller count or refund selector.
        for live_reservations in 0usize..=1 {
            let action_2_accounts = DIRECT_ADMIT_ORDER_FIXED_ACCOUNTS_V2
                .checked_add(live_reservations)
                .expect("bounded frame");
            assert!(matches!(action_2_accounts, 19 | 20));
        }
        assert_eq!(DIRECT_CANCEL_ORDER_ACCOUNTS_V2, 16);
        assert!(20 <= DIRECT_MARKET_V2_MAX_ACCOUNTS);
    }

    #[test]
    fn freeze_book_frame_has_no_caller_count_or_recipient_suffix() {
        for live_reservations in 0usize..=2 {
            let liveness_start = DIRECT_FREEZE_BOOK_FIXED_ACCOUNTS_V2
                .checked_add(live_reservations)
                .expect("bounded root count");
            let total = liveness_start
                .checked_add(DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2)
                .expect("bounded liveness suffix");
            assert!(matches!(total, 16 | 17 | 18));
        }
        assert!(18 <= DIRECT_MARKET_V2_MAX_ACCOUNTS);
    }

    #[test]
    fn lapse_frames_are_derived_only_from_authenticated_counts() {
        for endpoints in 0usize..=2 {
            let missed_freeze = 19 + (3 * endpoints)
                + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2;
            assert!(matches!(missed_freeze, 23 | 26 | 29));
            for refund_owners in 0usize..=3 {
                let existing_selection = 12 + (3 * endpoints) + refund_owners
                    + DIRECT_CANDIDATE_LIVENESS_ACCOUNT_COUNT_V2;
                assert!(existing_selection <= 25);
            }
        }
        assert!(29 <= DIRECT_MARKET_V2_MAX_ACCOUNTS);
    }

    #[test]
    fn child_frame_refuses_v1_alias_padding_and_wrong_width() {
        let mut replay = [1u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES];
        replay[0] = DIRECT_ACTION_REPLAY_ACCOUNT_TAG;
        replay[1] = DIRECT_ACTION_REPLAY_ACCOUNT_VERSION;
        replay[2] = 7;
        replay[3] = 0;
        assert!(decode_borrowed_child_frame_v2(
            &replay,
            DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
            DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
            DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
        )
        .is_ok());
        replay[3] = 1;
        assert!(decode_borrowed_child_frame_v2(
            &replay,
            DIRECT_ACTION_REPLAY_ACCOUNT_TAG,
            DIRECT_ACTION_REPLAY_ACCOUNT_VERSION,
            DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
        )
        .is_err());
    }

    #[test]
    fn no_candidate_endpoint_indices_are_fixed_and_checked() {
        assert!(matches!(direct_endpoint_first_from_v2(12, 0), Ok(12)));
        assert!(matches!(direct_endpoint_first_from_v2(12, 1), Ok(15)));
        assert!(direct_endpoint_first_from_v2(usize::MAX, 1).is_err());
        assert!(direct_endpoint_first_from_v2(12, usize::MAX).is_err());
    }
}
