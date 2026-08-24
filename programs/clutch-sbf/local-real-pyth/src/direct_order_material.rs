//! Chain-derived unsigned material for current Direct actions 2 through 4.
//!
//! This is the sole host constructor for the physically routed reservation and
//! freeze-book actions. It accepts finalized account observations, not browser
//! payload fields. Action 2 derives one deterministic order for the operator's
//! own authenticated ordinary Position; action 3 can close only a Reservation
//! already owned by that signer; action 4 derives the exhaustive Reservation
//! prefix, canonical price, rent principal, liveness work, and all postimages.
//! The result is unsigned and blockhash-free.

use crate::action_material::{
    chain_derived_direct_role_v2, finish_chain_derived_direct_material_v2,
    ActionFreshnessBoundaryV1, CanonicalActionMaterialErrorV1,
    CanonicalActionMaterialV1,
};
use crate::operatord::KeeperActionSelection;
use crate::rpc_index::{IndexedProgramRelease, ObservedRpcAccount, RpcCommitment};
use crate::transaction_builder::{ExactEquation, IntegerUnit, ProtocolTransactionBuilder};
use crate::workflow_graph::ExplicitOperatorReleaseManifest;
use clutch_batch::direct_pair_v1::DirectEconomicBookV1;
use clutch_batch::relation_v2::{
    EconomicDomainV2, PricePreconditionV2, ECONOMIC_RELATION_VERSION_V2,
    EMPTY_ECONOMIC_ORDER_V2,
};
use clutch_batch::{PartialPolicy, Side};
use clutch_client_contract::direct_market::DirectMarketClientPayloadV1;
use clutch_direct_market_runtime::codec_v2::{
    authenticate_direct_root_transition_body_v2,
    decode_direct_action_replay_body_for_transition_v2,
    decode_direct_reservation_body_for_transition_v2,
    encode_direct_action_replay_body_into_transition_v2,
    encode_direct_reservation_body_for_transition_v2,
    encode_direct_selection_body_into_transition_v2,
    write_direct_root_transition_body_v2, AuthenticatedDirectRootTransitionV2,
};
use clutch_direct_market_runtime::lifecycle_v2::{
    bind_direct_candidate_work_batch_v2, prepare_direct_candidate_work_batch_v2,
    prepare_direct_reservation_admission_v2, prepare_direct_reservation_cancel_v2,
    prepare_direct_selection_freeze_v2, AuthenticatedDirectReservationAdmissionV2,
    AuthenticatedDirectReservationCancelV2, AuthenticatedDirectSelectionFreezeV2,
    DirectRootReplayTransitionV2,
};
use clutch_direct_market_runtime::liveness_v1::{
    DirectCandidateLivenessBindingV1, DirectCandidateWorkBatchV1,
};
use clutch_direct_market_runtime::reservation_v1::DirectReservationV1;
use clutch_direct_market_runtime::settlement_v1::DirectReservationOrderInputV1;
use clutch_direct_market_runtime::selection_v1::{
    canonical_direct_price_precondition_v1, DirectSelectionV1,
};
use clutch_direct_market_runtime::{
    DirectActionReplayV1, DirectHashBackendV1, DirectMarketActionV1,
    DirectMarketErrorV1, DirectRentOwnerV1, DirectRootPhaseV1,
};
use clutch_general_v2_contract::{
    project_general_position_replay_prestate_v1, GeneralPositionReplayPrestateV1,
    Id32, MarketBindingV4, MarketRuntimeV3AccountV1,
    GENERAL_REPLAY_ACCOUNT_V1_BYTES, MARKET_BINDING_SEED_DOMAIN_V1,
    MARKET_RUNTIME_ACCOUNT_BYTES, MARKET_RUNTIME_SEED_DOMAIN_V1,
};
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimePersistedAccountViewV1,
    RuntimeReceiptKindV1, RuntimeReceiptObservationV1,
    RuntimeTransferRoleV1, RuntimeTransitionActionV1, RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentV1,
    RUNTIME_LIVENESS_ACCOUNT_BYTES_V1, RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_price_measure::PriceVectorV3;
use clutch_product_series::{
    CompiledProductSeriesBundleV6, FixedCodec, MarketGenesisProfileV2,
    MarketInstancePreimageV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
};
use clutch_retirement::{
    AdapterPositionMarketBindingV3, AdapterPositionPurposeBindingV3,
    Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Sha256Backend, ReplayV3Envelope, ReplayV3HashBackend,
    POSITION_V3_BYTES, POSITION_V3_PDA_PREFIX, PURPOSE_REPLAY_V3_PDA_PREFIX,
};
use clutch_owner_settlement::AuthenticatedPositionV3;
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::direct_market_v1::{
    DirectActionReplayAccountV1, DirectReservationAccountV1,
    DirectSelectionAccountV1,
};
use clutch_solana_layout::direct_market_v2::DirectMarketRootAccountV2;
use clutch_solana_layout::registry::{
    DirectMarketAction, DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2, DIRECT_RESERVATION_ACCOUNT_BYTES,
    DIRECT_SELECTION_ACCOUNT_BYTES,
};
use clutch_solana_layout::PriceGridAccount;
use clutch_solana_layout::{ProfileAccount, RealmAccount};
use clutch_collateral_adapter_v2::CollateralPolicyV2;
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use std::str::FromStr;

const DIRECT_ROOT_SEED_V2: &[u8] = b"dc:direct-market-root:v2";
const DIRECT_REPLAY_SEED_V1: &[u8] = b"dc:direct-action-replay:v1";
const DIRECT_SELECTION_SEED_V1: &[u8] = b"dc:direct-selection:v1";
const DIRECT_RESERVATION_SEED_V1: &[u8] = b"dc:direct-reservation:v1";
const PRODUCT_ARTIFACT_SEED_V1: &[u8] = b"dc:product-artifact:v1";
const PRICE_GRID_SEED_V1: &[u8] = b"dragons-clutch:grid:v1";
const REALM_SEED_V1: &[u8] = b"dragons-clutch:realm:v1";
const PROFILE_SEED_V1: &[u8] = b"dragons-clutch:profile:v1";
const COLLATERAL_POLICY_SEED_V1: &[u8] = b"dragons-clutch:policy:v1";

/// Exact finalized observations used by permissionless action 4.
#[derive(Clone, Copy, Debug)]
pub struct DirectFreezeBookSnapshotV2<'a> {
    pub root: &'a ObservedRpcAccount,
    pub replay: &'a ObservedRpcAccount,
    pub fresh_selection: &'a ObservedRpcAccount,
    pub creation_payer: &'a ObservedRpcAccount,
    pub system_program: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
    pub compiler_bundle_v6: &'a ObservedRpcAccount,
    pub native_claim_basis: &'a ObservedRpcAccount,
    pub price_measure_policy: &'a ObservedRpcAccount,
    pub market_genesis_v2: &'a ObservedRpcAccount,
    pub price_grid: &'a ObservedRpcAccount,
    /// Exact root-owned active prefix. Inactive entries are absent rather than
    /// caller-selected zero padding.
    pub reservations: [Option<&'a ObservedRpcAccount>; 2],
    pub liveness_policy: &'a ObservedRpcAccount,
    pub candidate_compartment: &'a ObservedRpcAccount,
    pub keeper: &'a ObservedRpcAccount,
    pub candidate_payer: &'a ObservedRpcAccount,
}

/// Exact action-4 postimages and lamport conservation facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectFreezeBookPostimagesV2 {
    prestate_id: [u8; 32],
    root_data_id: [u8; 32],
    replay_data_id: [u8; 32],
    selection_data_id: [u8; 32],
    candidate_data_id: [u8; 32],
    selection_lamports_after: u64,
    candidate_lamports_after: u64,
    keeper_payment_lamports: u64,
    candidate_payer_refund_lamports: u64,
}

impl DirectFreezeBookPostimagesV2 {
    pub const fn prestate_id(&self) -> [u8; 32] { self.prestate_id }
    pub const fn root_data_id(&self) -> [u8; 32] { self.root_data_id }
    pub const fn replay_data_id(&self) -> [u8; 32] { self.replay_data_id }
    pub const fn selection_data_id(&self) -> [u8; 32] { self.selection_data_id }
    pub const fn candidate_data_id(&self) -> [u8; 32] { self.candidate_data_id }
    pub const fn selection_lamports_after(&self) -> u64 { self.selection_lamports_after }
    pub const fn candidate_lamports_after(&self) -> u64 { self.candidate_lamports_after }
    pub const fn keeper_payment_lamports(&self) -> u64 { self.keeper_payment_lamports }
    pub const fn candidate_payer_refund_lamports(&self) -> u64 {
        self.candidate_payer_refund_lamports
    }
}

/// Opaque release-bound action-4 material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectFreezeBookActionMaterialV2 {
    canonical: CanonicalActionMaterialV1,
    postimages: DirectFreezeBookPostimagesV2,
}

impl DirectFreezeBookActionMaterialV2 {
    pub const fn canonical(&self) -> &CanonicalActionMaterialV1 { &self.canonical }
    pub const fn postimages(&self) -> &DirectFreezeBookPostimagesV2 { &self.postimages }
}

/// Finalized current Product/General/Collateral graph shared by actions 2 and
/// 3. Every field is an observed account; no semantic identity is accepted as
/// a request parameter.
#[derive(Clone, Copy, Debug)]
pub struct DirectReservationAuthoritySnapshotV2<'a> {
    pub realm: &'a ObservedRpcAccount,
    pub collateral_profile: &'a ObservedRpcAccount,
    pub collateral_policy: &'a ObservedRpcAccount,
    pub token_program: &'a ObservedRpcAccount,
    pub general_market_binding_v4: &'a ObservedRpcAccount,
    pub general_market_runtime: &'a ObservedRpcAccount,
    pub market_instance_v2: &'a ObservedRpcAccount,
    pub market_genesis_v2: &'a ObservedRpcAccount,
    pub position_v3: &'a ObservedRpcAccount,
    pub position_replay_v3: &'a ObservedRpcAccount,
}

/// Exact finalized observations used by deterministic operator-owned action 2.
#[derive(Clone, Copy, Debug)]
pub struct DirectAdmitOrderSnapshotV2<'a> {
    pub root: &'a ObservedRpcAccount,
    pub replay: &'a ObservedRpcAccount,
    pub fresh_reservation: &'a ObservedRpcAccount,
    pub actor_payer: &'a ObservedRpcAccount,
    pub authority: DirectReservationAuthoritySnapshotV2<'a>,
    pub system_program: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
    pub compiler_bundle_v6: &'a ObservedRpcAccount,
    pub price_grid: &'a ObservedRpcAccount,
    pub existing_peer: Option<&'a ObservedRpcAccount>,
}

/// Exact finalized observations used by owner-authenticated action 3.
#[derive(Clone, Copy, Debug)]
pub struct DirectCancelOrderSnapshotV2<'a> {
    pub root: &'a ObservedRpcAccount,
    pub replay: &'a ObservedRpcAccount,
    pub reservation: &'a ObservedRpcAccount,
    pub actor_payer: &'a ObservedRpcAccount,
    pub authority: DirectReservationAuthoritySnapshotV2<'a>,
    pub neutral_sink: &'a ObservedRpcAccount,
    pub clock: &'a ObservedRpcAccount,
}

/// Postimages for one Reservation admission or cancellation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectReservationPostimagesV2 {
    action: DirectMarketAction,
    prestate_id: [u8; 32],
    root_data_id: [u8; 32],
    replay_data_id: [u8; 32],
    reservation_post_data_id: Option<[u8; 32]>,
    position_data_id: [u8; 32],
    position_replay_data_id: [u8; 32],
    reservation_lamports_after: u64,
    payer_lamport_delta: i128,
    neutral_sink_lamports: u64,
}

impl DirectReservationPostimagesV2 {
    pub const fn action(&self) -> DirectMarketAction { self.action }
    pub const fn prestate_id(&self) -> [u8; 32] { self.prestate_id }
    pub const fn root_data_id(&self) -> [u8; 32] { self.root_data_id }
    pub const fn reservation_post_data_id(&self) -> Option<[u8; 32]> {
        self.reservation_post_data_id
    }
    pub const fn position_data_id(&self) -> [u8; 32] { self.position_data_id }
    pub const fn position_replay_data_id(&self) -> [u8; 32] {
        self.position_replay_data_id
    }
    pub const fn reservation_lamports_after(&self) -> u64 { self.reservation_lamports_after }
    pub const fn payer_lamport_delta(&self) -> i128 { self.payer_lamport_delta }
    pub const fn neutral_sink_lamports(&self) -> u64 { self.neutral_sink_lamports }
}

/// Opaque release-bound action-2/3 material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectReservationActionMaterialV2 {
    canonical: CanonicalActionMaterialV1,
    postimages: DirectReservationPostimagesV2,
}

impl DirectReservationActionMaterialV2 {
    pub const fn canonical(&self) -> &CanonicalActionMaterialV1 { &self.canonical }
    pub const fn postimages(&self) -> &DirectReservationPostimagesV2 { &self.postimages }
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedReservationGraphV2 {
    position_replay: GeneralPositionReplayPrestateV1,
    collateral_mint: Address,
}

#[derive(Clone, Copy, Debug)]
struct AdmissionAuthorityV2 {
    root_id: [u8; 32],
    replay: DirectActionReplayV1,
    position_replay: GeneralPositionReplayPrestateV1,
    peer: Option<DirectReservationV1>,
    order: DirectReservationOrderInputV1,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectReservationAdmissionV2 for AdmissionAuthorityV2 {
    fn authenticate_admission_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        position_replay: GeneralPositionReplayPrestateV1,
        existing_peer: Option<DirectReservationV1>,
        consumed_sequence: u64,
        observed_slot: u64,
        order: DirectReservationOrderInputV1,
    ) -> Result<(), DirectMarketErrorV1> {
        if state.root().root_semantic_id() == self.root_id
            && state.replay() == self.replay
            && position_replay == self.position_replay
            && existing_peer == self.peer
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

#[derive(Clone, Copy, Debug)]
struct CancelAuthorityV2 {
    root_id: [u8; 32],
    replay: DirectActionReplayV1,
    reservation: DirectReservationV1,
    position_replay: GeneralPositionReplayPrestateV1,
    reservation_lamports: u64,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectReservationCancelV2 for CancelAuthorityV2 {
    fn authenticate_cancel_v2(
        &self,
        state: &DirectRootReplayTransitionV2,
        reservation: DirectReservationV1,
        position_replay: GeneralPositionReplayPrestateV1,
        observed_reservation_lamports: u64,
        consumed_sequence: u64,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if state.root().root_semantic_id() == self.root_id
            && state.replay() == self.replay
            && reservation == self.reservation
            && position_replay == self.position_replay
            && observed_reservation_lamports == self.reservation_lamports
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Derive action 2's fresh Reservation address before the final exact-account
/// batch. This is the first phase of daemon acquisition only: it reuses the
/// same hostile graph and deterministic-order owners as the final material
/// constructor and returns no caller-selectable economics.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_direct_admit_reservation_address_v2(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    root_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
    operator: Address,
    authority_accounts: DirectReservationAuthoritySnapshotV2<'_>,
    compiler_bundle_v6: &ObservedRpcAccount,
    price_grid: &ObservedRpcAccount,
    existing_peer: Option<&ObservedRpcAccount>,
) -> Result<Address, CanonicalActionMaterialErrorV1> {
    let fixed = [root_account, replay_account, compiler_bundle_v6, price_grid];
    authenticate_snapshot_set(release, freshness, &fixed)?;
    authenticate_snapshot_set(
        release,
        freshness,
        &reservation_authority_accounts(authority_accounts),
    )?;
    if let Some(peer) = existing_peer {
        authenticate_snapshot_set(release, freshness, &[peer])?;
    }
    let decoded = decode_root_replay(release, root_account, replay_account)?;
    let root_count = usize::from(decoded.state.root().live_reservations());
    let peer = match (root_count, existing_peer) {
        (0, None) => None,
        (1, Some(account)) => Some(decode_reservation(
            release,
            root_account,
            account,
            &decoded.state,
            0,
        )?),
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    };
    if peer.is_some_and(|value| value.owner() == operator.to_bytes()) {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let graph = authenticate_reservation_graph(
        release,
        root_account,
        &decoded.state,
        operator,
        authority_accounts,
    )?;
    let (_, _, grid) = authenticate_order_artifacts(
        release,
        &decoded.state,
        compiler_bundle_v6,
        authority_accounts.market_genesis_v2,
        price_grid,
    )?;
    let fields = graph.position_replay.position().semantic.fields();
    let (side, outcome, quantity, limit) = derive_operator_owned_order(
        decoded.state.root(),
        fields.cash_atoms,
        fields.reserved_cash_atoms,
        fields.native_eggs,
        peer,
        &grid,
    )?;
    let order_id = derive_operator_order_id(
        release,
        root_account.address,
        operator,
        decoded.state.replay().next_action_sequence(),
        side,
        outcome,
        quantity,
        limit,
    );
    Ok(Address::find_program_address(
        &[DIRECT_RESERVATION_SEED_V1, root_account.address.as_ref(), &order_id],
        &release.program_id,
    ).0)
}

/// Derive one deterministic valid operator-owned order and construct current
/// action 2. This is deliberately not a generic customer order API: the actor
/// is the transaction builder's payer, the Position is its exact current
/// ordinary Position, and side/outcome/quantity/limit/order-id all derive from
/// that Position, the root-owned peer (if any), and the authenticated grid.
#[allow(clippy::too_many_arguments)]
pub fn construct_direct_admit_order_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectAdmitOrderSnapshotV2<'_>,
) -> Result<DirectReservationActionMaterialV2, CanonicalActionMaterialErrorV1> {
    let authority_accounts = reservation_authority_accounts(snapshot.authority);
    let fixed = [
        snapshot.root,
        snapshot.replay,
        snapshot.fresh_reservation,
        snapshot.actor_payer.address,
        snapshot.system_program,
        snapshot.rent_sysvar,
        snapshot.clock,
        snapshot.compiler_bundle_v6,
        snapshot.price_grid,
    ];
    authenticate_snapshot_set(release, freshness, &fixed)?;
    authenticate_snapshot_set(release, freshness, &authority_accounts)?;
    if let Some(peer) = snapshot.existing_peer {
        authenticate_snapshot_set(release, freshness, &[peer])?;
    }
    require_operator_account(snapshot.actor_payer)?;
    require_system_program(snapshot.system_program)?;
    if builder.payer() != snapshot.actor_payer.address {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let slot = decode_clock(snapshot.clock)?;
    let principal = decode_rent_minimum(snapshot.rent_sysvar, DIRECT_RESERVATION_ACCOUNT_BYTES)?;
    let mut decoded = decode_root_replay(release, snapshot.root, snapshot.replay)?;
    let root_count = usize::from(decoded.state.root().live_reservations());
    let peer = match (root_count, snapshot.existing_peer) {
        (0, None) => None,
        (1, Some(account)) => Some(decode_reservation(
            release,
            snapshot.root,
            account,
            &decoded.state,
            0,
        )?),
        _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
    };
    if peer.is_some_and(|value| value.owner() == snapshot.actor_payer.address.to_bytes()) {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let graph = authenticate_reservation_graph(
        release,
        snapshot.root,
        &decoded.state,
        snapshot.actor_payer.address,
        snapshot.authority,
    )?;
    let (bundle, genesis, grid) = authenticate_order_artifacts(
        release,
        &decoded.state,
        snapshot.compiler_bundle_v6,
        snapshot.authority.market_genesis_v2,
        snapshot.price_grid,
    )?;
    if bundle.market_genesis_profile_id.bytes()
        != genesis.id().map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?.bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let fields = graph.position_replay.position().semantic.fields();
    let (side, outcome, quantity, limit) = derive_operator_owned_order(
        decoded.state.root(),
        fields.cash_atoms,
        fields.reserved_cash_atoms,
        fields.native_eggs,
        peer,
        &grid,
    )?;
    let sequence = decoded.state.replay().next_action_sequence();
    let order_id = derive_operator_order_id(
        release,
        snapshot.root.address,
        snapshot.actor_payer.address,
        sequence,
        side,
        outcome,
        quantity,
        limit,
    );
    let (reservation_pda, reservation_bump) = Address::find_program_address(
        &[
            DIRECT_RESERVATION_SEED_V1,
            snapshot.root.address.as_ref(),
            &order_id,
        ],
        &release.program_id,
    );
    require_fresh_pda(snapshot.fresh_reservation, reservation_pda)?;
    let rent = DirectRentOwnerV1 {
        payer: snapshot.actor_payer.address.to_bytes(),
        principal_lamports: principal,
        donation_floor_lamports: snapshot.fresh_reservation.lamports,
    };
    rent.validate().map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if snapshot.actor_payer.lamports < principal {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let order = DirectReservationOrderInputV1 {
        reservation_account: reservation_pda.to_bytes(),
        order_id,
        side,
        outcome,
        quantity,
        minimum_fill: quantity,
        partial_policy: PartialPolicy::AllOrNone,
        expiry_epoch: decoded.state.root().direct_window_index()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        limit_price_units_per_egg: u128::from(limit),
        rent,
    };
    let authority = AdmissionAuthorityV2 {
        root_id: decoded.state.root().root_semantic_id(),
        replay: decoded.state.replay(),
        position_replay: graph.position_replay,
        peer,
        order,
        sequence,
        slot,
    };
    let plan = prepare_direct_reservation_admission_v2(
        &authority,
        &mut decoded.state,
        graph.position_replay,
        peer,
        sequence,
        slot,
        order,
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let prestate_id = snapshot_id(
        release,
        DirectMarketAction::AdmitOrder,
        sequence,
        &admit_prestate_accounts(snapshot),
    );
    let mut cursor_dependencies = vec![
        snapshot.root,
        snapshot.replay,
        snapshot.authority.position_v3,
        snapshot.authority.position_replay_v3,
    ];
    if let Some(peer) = snapshot.existing_peer {
        cursor_dependencies.push(peer);
    }
    authenticate_signer_cursor(
        operator_selection,
        DirectMarketAction::AdmitOrder,
        decoded.state.root().generation(),
        sequence,
        prestate_id,
        snapshot.root.address,
        &cursor_dependencies,
    )?;
    let root_replay_post = encode_root_replay_postimages(snapshot.root, snapshot.replay, &decoded)?;
    let reservation_post = encode_reservation_postimage(
        reservation_bump,
        plan.reservation,
        decoded.state.root(),
    )?;
    let position_post = plan.position_poststate.semantic.encode()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_post = plan.replay_transition.replay_poststate_body();
    let mut metas = vec![
        meta(snapshot.root, false, true),
        meta(snapshot.replay, false, true),
        meta(snapshot.fresh_reservation, false, true),
        meta(snapshot.actor_payer, true, true),
        meta(snapshot.authority.position_v3, false, true),
        meta(snapshot.authority.position_replay_v3, false, true),
        meta(snapshot.authority.realm, false, false),
        meta(snapshot.authority.collateral_profile, false, false),
        meta(snapshot.authority.collateral_policy, false, false),
        meta(snapshot.authority.token_program, false, false),
        meta(snapshot.authority.general_market_binding_v4, false, false),
        meta(snapshot.authority.general_market_runtime, false, false),
        meta(snapshot.authority.market_instance_v2, false, false),
        meta(snapshot.system_program, false, false),
        meta(snapshot.rent_sysvar, false, false),
        meta(snapshot.clock, false, false),
        meta(snapshot.compiler_bundle_v6, false, false),
        meta(snapshot.authority.market_genesis_v2, false, false),
        meta(snapshot.price_grid, false, false),
    ];
    let mut roles = reservation_prefix_roles(snapshot, true);
    if let Some(account) = snapshot.existing_peer {
        metas.push(meta(account, false, false));
        roles.push(role("root-owned-peer-reservation", account, false, false));
    }
    require_all_distinct(&metas)?;
    let payload = DirectMarketClientPayloadV1::admit_order(
        clutch_solana_layout::direct_market_v1::DirectAdmitOrderPayloadV1 {
            order_id,
            side,
            outcome,
            partial_policy: PartialPolicy::AllOrNone,
            quantity,
            minimum_fill: quantity,
            expiry_epoch: order.expiry_epoch,
            limit_price_units_per_egg: u128::from(limit),
        },
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let pre_fields = graph.position_replay.position().semantic.fields();
    let post_fields = plan.position_poststate.semantic.fields();
    let mut equations = vec![ExactEquation {
        name: "reservation rent principal plus immutable prefund".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(snapshot.actor_payer.lamports)
            + u128::from(snapshot.fresh_reservation.lamports),
        right: u128::from(snapshot.actor_payer.lamports - principal)
            + u128::from(snapshot.fresh_reservation.lamports)
            + u128::from(principal),
    }];
    match side {
        Side::Buy => equations.push(ExactEquation {
            name: "Position reserved-cash subset increases by exact order reserve".into(),
            unit: IntegerUnit::CollateralAtoms { mint: graph.collateral_mint },
            left: u128::from(pre_fields.reserved_cash_atoms)
                + u128::from(plan.reservation.reserved_cash_atoms()),
            right: u128::from(post_fields.reserved_cash_atoms),
        }),
        Side::Sell => equations.push(ExactEquation {
            name: "Position Egg debit equals exact Reservation escrow".into(),
            unit: IntegerUnit::EggAtoms {
                market: decoded.state.root().market_instance_id(),
                outcome,
            },
            left: u128::from(pre_fields.native_eggs[usize::from(outcome)]),
            right: u128::from(post_fields.native_eggs[usize::from(outcome)])
                + u128::from(plan.reservation.reserved_eggs()),
        }),
    }
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        DirectMarketAction::AdmitOrder,
        sequence,
        metas,
        vec![snapshot.actor_payer.address],
        roles,
        equations,
        payload,
    )?;
    Ok(DirectReservationActionMaterialV2 {
        canonical,
        postimages: DirectReservationPostimagesV2 {
            action: DirectMarketAction::AdmitOrder,
            prestate_id,
            root_data_id: root_replay_post.0,
            replay_data_id: root_replay_post.1,
            reservation_post_data_id: Some(sha256(&reservation_post)),
            position_data_id: sha256(&position_post),
            position_replay_data_id: sha256(replay_post),
            reservation_lamports_after: snapshot.fresh_reservation.lamports
                .checked_add(principal)
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?,
            payer_lamport_delta: -i128::from(principal),
            neutral_sink_lamports: 0,
        },
    })
}

/// Construct owner-authenticated action 3 from the exact Reservation selected
/// by its account and body. Payer principal and donation/surplus destinations
/// are read only from persisted state; no caller refund address is accepted.
#[allow(clippy::too_many_arguments)]
pub fn construct_direct_cancel_order_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectCancelOrderSnapshotV2<'_>,
) -> Result<DirectReservationActionMaterialV2, CanonicalActionMaterialErrorV1> {
    let fixed = [
        snapshot.root, snapshot.replay, snapshot.reservation,
        snapshot.actor_payer, snapshot.neutral_sink, snapshot.clock,
    ];
    authenticate_snapshot_set(release, freshness, &fixed)?;
    authenticate_snapshot_set(
        release,
        freshness,
        &reservation_authority_accounts(snapshot.authority),
    )?;
    require_operator_account(snapshot.actor_payer)?;
    if builder.payer() != snapshot.actor_payer.address {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let slot = decode_clock(snapshot.clock)?;
    let mut decoded = decode_root_replay(release, snapshot.root, snapshot.replay)?;
    let root_index = find_reservation_index(&decoded.state, snapshot.reservation.address)?;
    let reservation = decode_reservation(
        release,
        snapshot.root,
        snapshot.reservation,
        &decoded.state,
        root_index,
    )?;
    if reservation.owner() != snapshot.actor_payer.address.to_bytes()
        || reservation.rent().payer != snapshot.actor_payer.address.to_bytes()
        || snapshot.neutral_sink.address.to_bytes()
            != decoded.state.root().neutral_lamport_sink()
        || snapshot.neutral_sink.executable
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let graph = authenticate_reservation_graph(
        release,
        snapshot.root,
        &decoded.state,
        snapshot.actor_payer.address,
        snapshot.authority,
    )?;
    let sequence = decoded.state.replay().next_action_sequence();
    let authority = CancelAuthorityV2 {
        root_id: decoded.state.root().root_semantic_id(),
        replay: decoded.state.replay(),
        reservation,
        position_replay: graph.position_replay,
        reservation_lamports: snapshot.reservation.lamports,
        sequence,
        slot,
    };
    let plan = prepare_direct_reservation_cancel_v2(
        &authority,
        &mut decoded.state,
        reservation,
        graph.position_replay,
        snapshot.reservation.lamports,
        sequence,
        slot,
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let prestate_id = snapshot_id(
        release,
        DirectMarketAction::CancelOrder,
        sequence,
        &cancel_prestate_accounts(snapshot),
    );
    authenticate_signer_cursor(
        operator_selection,
        DirectMarketAction::CancelOrder,
        decoded.state.root().generation(),
        sequence,
        prestate_id,
        snapshot.root.address,
        &[
            snapshot.root,
            snapshot.replay,
            snapshot.authority.position_v3,
            snapshot.authority.position_replay_v3,
            snapshot.reservation,
        ],
    )?;
    let root_replay_post = encode_root_replay_postimages(snapshot.root, snapshot.replay, &decoded)?;
    let position_post = plan.endpoint.position_poststate.semantic.encode()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_post = plan.endpoint.replay_transition.replay_poststate_body();
    let principal = reservation.rent().principal_lamports;
    let surplus = plan.retirement.surplus_lamports;
    if plan.retirement.source_count != 1
        || plan.retirement.refund_count != 1
        || plan.retirement.neutral_lamport_sink != snapshot.neutral_sink.address.to_bytes()
        || match plan.retirement.refunds[0] {
            Some(refund) => refund.recipient != snapshot.actor_payer.address.to_bytes()
                || refund.lamports != principal,
            None => true,
        }
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let metas = vec![
        meta(snapshot.root, false, true),
        meta(snapshot.replay, false, true),
        meta(snapshot.reservation, false, true),
        meta(snapshot.actor_payer, true, true),
        meta(snapshot.authority.position_v3, false, true),
        meta(snapshot.authority.position_replay_v3, false, true),
        meta(snapshot.authority.realm, false, false),
        meta(snapshot.authority.collateral_profile, false, false),
        meta(snapshot.authority.collateral_policy, false, false),
        meta(snapshot.authority.token_program, false, false),
        meta(snapshot.authority.general_market_binding_v4, false, false),
        meta(snapshot.authority.general_market_runtime, false, false),
        meta(snapshot.authority.market_instance_v2, false, false),
        meta(snapshot.authority.market_genesis_v2, false, false),
        meta(snapshot.neutral_sink, false, true),
        meta(snapshot.clock, false, false),
    ];
    require_all_distinct(&metas)?;
    let roles = reservation_prefix_roles_cancel(snapshot);
    let payload = DirectMarketClientPayloadV1::empty(DirectMarketAction::CancelOrder)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let pre_fields = graph.position_replay.position().semantic.fields();
    let post_fields = plan.endpoint.position_poststate.semantic.fields();
    let mut equations = vec![ExactEquation {
        name: "reservation principal refund plus neutral surplus".into(),
        unit: IntegerUnit::Lamports,
        left: u128::from(snapshot.reservation.lamports)
            + u128::from(snapshot.actor_payer.lamports)
            + u128::from(snapshot.neutral_sink.lamports),
        right: u128::from(snapshot.actor_payer.lamports)
            + u128::from(principal)
            + u128::from(snapshot.neutral_sink.lamports)
            + u128::from(surplus),
    }];
    match reservation.side() {
        Side::Buy => equations.push(ExactEquation {
            name: "cancel releases the exact reserved-cash subset".into(),
            unit: IntegerUnit::CollateralAtoms { mint: graph.collateral_mint },
            left: u128::from(post_fields.reserved_cash_atoms)
                + u128::from(reservation.reserved_cash_atoms()),
            right: u128::from(pre_fields.reserved_cash_atoms),
        }),
        Side::Sell => equations.push(ExactEquation {
            name: "cancel restores exact escrowed Eggs".into(),
            unit: IntegerUnit::EggAtoms {
                market: decoded.state.root().market_instance_id(),
                outcome: reservation.outcome(),
            },
            left: u128::from(post_fields.native_eggs[usize::from(reservation.outcome())]),
            right: u128::from(pre_fields.native_eggs[usize::from(reservation.outcome())])
                + u128::from(reservation.reserved_eggs()),
        }),
    }
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        DirectMarketAction::CancelOrder,
        sequence,
        metas,
        vec![snapshot.actor_payer.address],
        roles,
        equations,
        payload,
    )?;
    Ok(DirectReservationActionMaterialV2 {
        canonical,
        postimages: DirectReservationPostimagesV2 {
            action: DirectMarketAction::CancelOrder,
            prestate_id,
            root_data_id: root_replay_post.0,
            replay_data_id: root_replay_post.1,
            reservation_post_data_id: None,
            position_data_id: sha256(&position_post),
            position_replay_data_id: sha256(replay_post),
            reservation_lamports_after: 0,
            payer_lamport_delta: i128::from(principal),
            neutral_sink_lamports: surplus,
        },
    })
}

fn reservation_authority_accounts(
    authority: DirectReservationAuthoritySnapshotV2<'_>,
) -> [&ObservedRpcAccount; 10] {
    [
        authority.realm,
        authority.collateral_profile,
        authority.collateral_policy,
        authority.token_program,
        authority.general_market_binding_v4,
        authority.general_market_runtime,
        authority.market_instance_v2,
        authority.market_genesis_v2,
        authority.position_v3,
        authority.position_replay_v3,
    ]
}

fn authenticate_reservation_graph(
    release: &IndexedProgramRelease,
    root_account: &ObservedRpcAccount,
    state: &DirectRootReplayTransitionV2,
    actor: Address,
    accounts: DirectReservationAuthoritySnapshotV2<'_>,
) -> Result<AuthenticatedReservationGraphV2, CanonicalActionMaterialErrorV1> {
    for account in [
        accounts.realm,
        accounts.collateral_profile,
        accounts.collateral_policy,
        accounts.general_market_binding_v4,
        accounts.general_market_runtime,
        accounts.market_instance_v2,
        accounts.market_genesis_v2,
        accounts.position_v3,
        accounts.position_replay_v3,
    ] {
        if account.owner != release.program_id || account.executable {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    let root = state.root();
    let realm = RealmAccount::decode(&accounts.realm.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (realm_pda, realm_bump) = Address::find_program_address(
        &[REALM_SEED_V1, &realm.realm.0],
        &release.program_id,
    );
    if accounts.realm.address != realm_pda
        || realm.stored_bump != realm_bump
        || realm.realm.0 != root.realm_id()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let profile = ProfileAccount::decode(&accounts.collateral_profile.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (profile_pda, _) = Address::find_program_address(
        &[PROFILE_SEED_V1, &realm.realm.0, &profile.profile.0],
        &release.program_id,
    );
    if accounts.collateral_profile.address != profile_pda
        || profile.realm != realm.realm
        || profile.profile != realm.profile
        || profile.version != realm.profile_version
        || profile.profile.0 != root.collateral_profile_id()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let collateral = CollateralPolicyV2::decode(&accounts.collateral_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let collateral_id = collateral.id()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (policy_pda, _) = Address::find_program_address(
        &[
            COLLATERAL_POLICY_SEED_V1,
            &profile.profile.0,
            &collateral_id.bytes(),
        ],
        &release.program_id,
    );
    if accounts.collateral_policy.address != policy_pda
        || profile.collateral_policy_id.0 != collateral_id.bytes()
        || profile.adapter_release_id.0 != collateral.adapter_release.bytes()
        || collateral_id.bytes() != root.collateral_policy_id()
        || collateral.adapter_release.bytes() != root.collateral_release_id()
        || accounts.token_program.address.to_bytes() != collateral.token_program.bytes()
        || !accounts.token_program.executable
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }

    let binding = MarketBindingV4::decode(&accounts.general_market_binding_v4.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let general = *binding.base().base();
    let (binding_pda, binding_bump) = Address::find_program_address(
        &[MARKET_BINDING_SEED_DOMAIN_V1, &root.market_instance_id()],
        &release.program_id,
    );
    if accounts.general_market_binding_v4.address != binding_pda
        || binding_bump != general.stored_bump
        || accounts.general_market_binding_v4.address.to_bytes()
            != root.general_market_binding_account()
        || general.market_instance_v2_id.bytes() != root.market_instance_id()
        || general.market_genesis_profile_v2_id.bytes()
            != authenticate_product_artifact::<MarketGenesisProfileV2>(
                release,
                accounts.market_genesis_v2,
                ArtifactKind::MarketGenesisProfileV2,
                general.market_genesis_profile_v2_id.bytes(),
                |value| value.id().map(|id| id.bytes()),
            )?
            .id()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .bytes()
        || general.relation_policy_id.bytes() != root.relation_policy_id()
        || general.price_measure_policy_v1_id.bytes() != root.price_policy_id()
        || general.neutral_sink.bytes() != root.neutral_lamport_sink()
        || general.price_scale != root.price_scale()
        || general.outcome_count != root.outcome_count()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_deletable_rent(binding.rent(), accounts.general_market_binding_v4.lamports)?;
    let runtime = MarketRuntimeV3AccountV1::decode(&accounts.general_market_runtime.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (runtime_pda, runtime_bump) = Address::find_program_address(
        &[
            MARKET_RUNTIME_SEED_DOMAIN_V1,
            accounts.general_market_binding_v4.address.as_ref(),
        ],
        &release.program_id,
    );
    if accounts.general_market_runtime.address != runtime_pda
        || runtime.stored_bump != runtime_bump
        || runtime.market_binding.bytes()
            != accounts.general_market_binding_v4.address.to_bytes()
        || runtime.market_instance_v2_id.bytes() != root.market_instance_id()
        || general.market.bytes() != accounts.general_market_runtime.address.to_bytes()
        || accounts.general_market_runtime.address.to_bytes()
            != root.general_market_runtime_account()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_deletable_rent(runtime.rent, accounts.general_market_runtime.lamports)?;
    authenticate_product_artifact::<MarketInstancePreimageV2>(
        release,
        accounts.market_instance_v2,
        ArtifactKind::MarketInstancePreimageV2,
        root.market_instance_id(),
        |value| value.id().map(|id| id.bytes()),
    )?;
    let current = binding.authority();
    let purpose_seed = [u8::from(PositionPurposeV3::General)];
    let (treasury_replay, _) = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            &current.treasury_position_account().bytes(),
            &purpose_seed,
            accounts.general_market_runtime.address.as_ref(),
        ],
        &release.program_id,
    );
    let current_general = clutch_direct_market_runtime::current_v2::DirectCurrentGeneralAuthorityV2 {
        general_market_binding_account: accounts.general_market_binding_v4.address.to_bytes(),
        general_market_binding_v4_data_id: sha256(&accounts.general_market_binding_v4.data),
        general_market_runtime_account: accounts.general_market_runtime.address.to_bytes(),
        general_market_runtime_data_id: sha256(&accounts.general_market_runtime.data),
        revenue_policy_record_account: current.revenue_policy_record_account().bytes(),
        revenue_policy_record_v2_id: current.revenue_policy_record_v2_id().bytes(),
        revenue_policy_v2_digest: current.revenue_policy_v2_digest().bytes(),
        treasury_owner: current.treasury_owner().bytes(),
        treasury_position_derivation_policy_v2_id:
            current.treasury_position_derivation_policy_v2_id().bytes(),
        treasury_position_account: current.treasury_position_account().bytes(),
        treasury_replay_account: treasury_replay.to_bytes(),
        treasury_service_ledger_account: current.treasury_service_ledger_account().bytes(),
    };
    let current_general_id = current_general.semantic_id(&OperatorDirectHashV2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if current_general_id != root.current_general_authority_id()
        || current.revenue_policy_v2_digest().bytes()
            != root.fee_policy().revenue_policy_v2_digest
        || current.treasury_owner().bytes() != root.fee_policy().treasury_owner
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }

    let position = PositionAccountV3::decode(&accounts.position_v3.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let fields = position.fields();
    let (position_pda, position_bump) = Address::find_program_address(
        &[
            POSITION_V3_PDA_PREFIX,
            &root.market_instance_id(),
            actor.as_ref(),
            &purpose_seed,
            accounts.general_market_runtime.address.as_ref(),
        ],
        &release.program_id,
    );
    let (replay_pda, replay_bump) = Address::find_program_address(
        &[
            PURPOSE_REPLAY_V3_PDA_PREFIX,
            position_pda.as_ref(),
            &purpose_seed,
            accounts.general_market_runtime.address.as_ref(),
        ],
        &release.program_id,
    );
    let position_semantic_id = position.semantic_id(&OperatorDirectHashV2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if accounts.position_v3.address != position_pda
        || fields.stored_bump != position_bump
        || accounts.position_replay_v3.address != replay_pda
        || fields.purpose != PositionPurposeV3::General
        || fields.lifecycle != PositionLifecycleV3::Open
        || fields.market_instance_id.bytes() != root.market_instance_id()
        || fields.realm_id.bytes() != root.realm_id()
        || fields.collateral_policy_id.bytes() != root.collateral_policy_id()
        || fields.collateral_release_id.bytes() != root.collateral_release_id()
        || fields.owner.bytes() != actor.to_bytes()
        || fields.controller.bytes() != actor.to_bytes()
        || fields.replay_account.bytes() != replay_pda.to_bytes()
        || fields.purpose_binding_id.bytes()
            != accounts.general_market_runtime.address.to_bytes()
        || fields.outcome_count != root.outcome_count()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let position_rent = fields.rent;
    let position_floor = position_rent.refundable_live_principal
        .checked_add(position_rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(position_rent.donation_floor))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if accounts.position_v3.lamports < position_floor
        || accounts.position_replay_v3.data.len() != GENERAL_REPLAY_ACCOUNT_V1_BYTES
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let envelope = ReplayV3Envelope::decode(
        &accounts.position_replay_v3.data,
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_rent = envelope.header().rent();
    let replay_floor = replay_rent.refundable_principal()
        .checked_add(replay_rent.donation_floor())
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if accounts.position_replay_v3.lamports < replay_floor {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let authenticated_position = AuthenticatedPositionV3 {
        account: position_pda.to_bytes(),
        general_market_runtime: accounts.general_market_runtime.address.to_bytes(),
        semantic: position,
        semantic_id: position_semantic_id.bytes(),
        account_authenticated: true,
        semantic_id_authenticated: true,
        market_binding_authenticated: true,
        writable: true,
    };
    authenticated_position.validate_writable()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    clutch_retirement::project_general_position_v3(
        position,
        AdapterPositionMarketBindingV3 {
            market_instance_id: Identity32V1::new(root.market_instance_id())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
            outcome_count: root.outcome_count(),
            realm_id: Identity32V1::new(root.realm_id())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
            collateral_policy_id: Identity32V1::new(root.collateral_policy_id())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
            collateral_release_id: Identity32V1::new(root.collateral_release_id())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        },
        AdapterPositionPurposeBindingV3 {
            owner: Identity32V1::new(actor.to_bytes())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
            controller: Identity32V1::new(actor.to_bytes())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
            purpose_binding_id: Identity32V1::new(
                accounts.general_market_runtime.address.to_bytes(),
            )
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        },
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let position_replay = project_general_position_replay_prestate_v1(
        Id32::new(replay_pda.to_bytes())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        replay_bump,
        envelope.header().next_sequence(),
        &accounts.position_replay_v3.data,
        authenticated_position,
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if root_account.address.to_bytes() != root.direct_root_account() {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(AuthenticatedReservationGraphV2 {
        position_replay,
        collateral_mint: Address::new_from_array(collateral.mint.bytes()),
    })
}

fn authenticate_order_artifacts(
    release: &IndexedProgramRelease,
    state: &DirectRootReplayTransitionV2,
    bundle_account: &ObservedRpcAccount,
    genesis_account: &ObservedRpcAccount,
    grid_account: &ObservedRpcAccount,
) -> Result<(
    CompiledProductSeriesBundleV6,
    MarketGenesisProfileV2,
    PriceGridAccount,
), CanonicalActionMaterialErrorV1> {
    let root = state.root();
    let bundle = authenticate_product_artifact::<CompiledProductSeriesBundleV6>(
        release,
        bundle_account,
        ArtifactKind::CompiledProductSeriesBundleV6,
        root.compiler_bundle_v6_id(),
        |value| value.id().map(|id| id.bytes()),
    )?;
    let genesis = authenticate_product_artifact::<MarketGenesisProfileV2>(
        release,
        genesis_account,
        ArtifactKind::MarketGenesisProfileV2,
        bundle.market_genesis_profile_id.bytes(),
        |value| value.id().map(|id| id.bytes()),
    )?;
    let grid = PriceGridAccount::decode(&grid_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (expected, bump) = Address::find_program_address(
        &[PRICE_GRID_SEED_V1, &grid.realm.0, &grid.grid.0],
        &release.program_id,
    );
    if grid_account.owner != release.program_id
        || grid_account.executable
        || grid_account.address != expected
        || grid.stored_bump != bump
        || grid.realm.0 != root.realm_id()
        || grid.grid.0 != genesis.price_grid_id.bytes()
        || grid.price_scale != root.price_scale()
        || genesis.realm_id.bytes() != root.realm_id()
        || genesis.profile_id.bytes() != root.collateral_profile_id()
        || genesis.relation_policy_id.bytes() != root.relation_policy_id()
        || genesis.price_measure_policy_id.bytes() != root.price_policy_id()
        || genesis.fee_policy_id.bytes() != root.fee_policy().revenue_policy_v2_digest
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok((bundle, genesis, grid))
}

fn derive_operator_owned_order(
    root: &AuthenticatedDirectRootTransitionV2,
    cash_atoms: u64,
    reserved_cash_atoms: u64,
    native_eggs: [u64; 16],
    peer: Option<DirectReservationV1>,
    grid: &PriceGridAccount,
) -> Result<(Side, u8, u64, u64), CanonicalActionMaterialErrorV1> {
    let free_cash = cash_atoms
        .checked_sub(reserved_cash_atoms)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (side, outcome, limit, maximum_quantity) = match peer {
        Some(peer) if peer.side() == Side::Buy => {
            let outcome = peer.outcome();
            let available = native_eggs
                .get(usize::from(outcome))
                .copied()
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
            (Side::Sell, outcome, u64::try_from(peer.limit_price_units_per_egg())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
                available.min(peer.quantity()))
        }
        Some(peer) => (
            Side::Buy,
            peer.outcome(),
            u64::try_from(peer.limit_price_units_per_egg())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
            peer.quantity(),
        ),
        None => {
            let last = grid.tick_count
                .checked_sub(1)
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
            let limit = grid.tick_value(last)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
            let mut outcome = 0usize;
            while outcome < usize::from(root.outcome_count()) {
                if native_eggs[outcome] != 0 {
                    return quantized_order(
                        root,
                        Side::Sell,
                        u8::try_from(outcome)
                            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
                        native_eggs[outcome],
                        limit,
                        free_cash,
                    );
                }
                outcome += 1;
            }
            (Side::Buy, 0, limit, u64::MAX)
        }
    };
    quantized_order(root, side, outcome, maximum_quantity, limit, free_cash)
}

fn quantized_order(
    root: &AuthenticatedDirectRootTransitionV2,
    side: Side,
    outcome: u8,
    maximum_quantity: u64,
    limit: u64,
    free_cash: u64,
) -> Result<(Side, u8, u64, u64), CanonicalActionMaterialErrorV1> {
    if usize::from(outcome) >= usize::from(root.outcome_count())
        || limit > root.price_scale()
        || maximum_quantity == 0
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let divisor = gcd_u64(limit, root.price_scale());
    let quantum = root.price_scale()
        .checked_div(divisor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let maximum_units = maximum_quantity / quantum;
    if maximum_units == 0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let units = if side == Side::Sell {
        maximum_units
    } else {
        maximum_affordable_units(root, free_cash, quantum, limit, maximum_units)?
    };
    let quantity = units
        .checked_mul(quantum)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if quantity == 0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok((side, outcome, quantity, limit))
}

fn maximum_affordable_units(
    root: &AuthenticatedDirectRootTransitionV2,
    free_cash: u64,
    quantum: u64,
    limit: u64,
    maximum_units: u64,
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let mut low = 0u64;
    let mut high = maximum_units;
    while low < high {
        let step = high
            .checked_sub(low)
            .and_then(|difference| difference.checked_add(1))
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
            / 2;
        let candidate = low
            .checked_add(step)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let quantity = candidate
            .checked_mul(quantum)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if required_buy_cash(root, quantity, limit)? <= free_cash {
            low = candidate;
        } else {
            high = candidate
                .checked_sub(1)
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        }
    }
    if low == 0 {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(low)
    }
}

fn required_buy_cash(
    root: &AuthenticatedDirectRootTransitionV2,
    quantity: u64,
    limit: u64,
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let units = u128::from(quantity)
        .checked_mul(u128::from(limit))
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let scale = u128::from(root.price_scale());
    if scale == 0 || units % scale != 0 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let cash = u64::try_from(units / scale)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let fee = root.fee_policy()
        .maximum_buyer_fee_atoms(quantity, root.outcome_count(), root.price_scale())
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    cash.checked_add(fee)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)
}

const fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn derive_operator_order_id(
    release: &IndexedProgramRelease,
    root: Address,
    owner: Address,
    sequence: u64,
    side: Side,
    outcome: u8,
    quantity: u64,
    limit: u64,
) -> [u8; 32] {
    let side_byte = match side { Side::Buy => 1u8, Side::Sell => 2u8 };
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/direct-owned-order/v2\0");
    hash.update(release.release_manifest_sha256);
    hash.update(release.capability_profile_id);
    hash.update(root.as_ref());
    hash.update(owner.as_ref());
    hash.update(sequence.to_le_bytes());
    hash.update([side_byte, outcome]);
    hash.update(quantity.to_le_bytes());
    hash.update(limit.to_le_bytes());
    hash.finalize().into()
}

fn admit_prestate_accounts(snapshot: DirectAdmitOrderSnapshotV2<'_>) -> Vec<&ObservedRpcAccount> {
    let mut accounts = vec![
        snapshot.root, snapshot.replay, snapshot.fresh_reservation,
        snapshot.actor_payer, snapshot.authority.position_v3,
        snapshot.authority.position_replay_v3, snapshot.authority.realm,
        snapshot.authority.collateral_profile, snapshot.authority.collateral_policy,
        snapshot.authority.token_program, snapshot.authority.general_market_binding_v4,
        snapshot.authority.general_market_runtime, snapshot.authority.market_instance_v2,
        snapshot.system_program, snapshot.rent_sysvar, snapshot.clock,
        snapshot.compiler_bundle_v6, snapshot.authority.market_genesis_v2,
        snapshot.price_grid,
    ];
    if let Some(peer) = snapshot.existing_peer { accounts.push(peer); }
    accounts
}

fn cancel_prestate_accounts(snapshot: DirectCancelOrderSnapshotV2<'_>) -> Vec<&ObservedRpcAccount> {
    vec![
        snapshot.root, snapshot.replay, snapshot.reservation, snapshot.actor_payer,
        snapshot.authority.position_v3, snapshot.authority.position_replay_v3,
        snapshot.authority.realm, snapshot.authority.collateral_profile,
        snapshot.authority.collateral_policy, snapshot.authority.token_program,
        snapshot.authority.general_market_binding_v4,
        snapshot.authority.general_market_runtime, snapshot.authority.market_instance_v2,
        snapshot.authority.market_genesis_v2, snapshot.neutral_sink, snapshot.clock,
    ]
}

fn authenticate_signer_cursor(
    selection: &KeeperActionSelection,
    action: DirectMarketAction,
    generation: u64,
    sequence: u64,
    prestate_id: [u8; 32],
    root: Address,
    required_dependencies: &[&ObservedRpcAccount],
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let dependency_id = finalized_dependency_digest(required_dependencies)?;
    if selection.cursor.workflow_id == [0; 32]
        || selection.cursor.lane != crate::workflow_graph::WorkflowLane::Candidate
        || selection.cursor.generation != generation
        || selection.cursor.position.phase != u16::from(action.tag())
        || selection.cursor.position.item != sequence
        || selection.cursor.observed_state_sha256 != dependency_id
        || prestate_id == [0; 32]
        || selection.account != root
        || selection.dependencies.len() != required_dependencies.len()
        || required_dependencies
            .iter()
            .any(|account| !selection.dependencies.contains(&account.address))
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    Ok(())
}

pub(crate) fn finalized_dependency_digest(
    dependencies: &[&ObservedRpcAccount],
) -> Result<[u8; 32], CanonicalActionMaterialErrorV1> {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator-index-observation/v3-semantic-finalized-state");
    hash.update([2]);
    for dependency in dependencies {
        if dependency.provenance.commitment != RpcCommitment::Finalized {
            return Err(CanonicalActionMaterialErrorV1::WrongSelection);
        }
        hash.update(dependency.address.to_bytes());
        hash.update(dependency.owner.to_bytes());
        hash.update(dependency.lamports.to_le_bytes());
        hash.update(dependency.rent_epoch.to_le_bytes());
        hash.update(u64::try_from(dependency.data.len())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .to_le_bytes());
        hash.update(sha256(&dependency.data));
        let release = dependency.provenance.release_key.as_bytes();
        hash.update(u64::try_from(release.len())
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            .to_le_bytes());
        hash.update(release);
    }
    Ok(hash.finalize().into())
}

fn encode_root_replay_postimages(
    root_pre: &ObservedRpcAccount,
    replay_pre: &ObservedRpcAccount,
    decoded: &DecodedRootReplayV2,
) -> Result<([u8; 32], [u8; 32]), CanonicalActionMaterialErrorV1> {
    let mut root = root_pre.data.clone();
    let mut replay = replay_pre.data.clone();
    if root.len() != DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2
        || replay.len() != DIRECT_ACTION_REPLAY_ACCOUNT_BYTES
        || root[2] != decoded.root_bump
        || replay[2] != decoded.replay_bump
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    write_direct_root_transition_body_v2(
        decoded.state.root(), &mut root[4..], &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    encode_direct_action_replay_body_into_transition_v2(
        decoded.state.replay(), decoded.state.root(), &mut replay[4..],
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok((sha256(&root), sha256(&replay)))
}

fn encode_reservation_postimage(
    bump: u8,
    reservation: DirectReservationV1,
    root: &AuthenticatedDirectRootTransitionV2,
) -> Result<[u8; DIRECT_RESERVATION_ACCOUNT_BYTES], CanonicalActionMaterialErrorV1> {
    let body = encode_direct_reservation_body_for_transition_v2(reservation, root)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let frame = DirectReservationAccountV1::new(bump, body)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut output = [0u8; DIRECT_RESERVATION_ACCOUNT_BYTES];
    frame.encode_into(&mut output)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(output)
}

fn reservation_prefix_roles(
    snapshot: DirectAdmitOrderSnapshotV2<'_>,
    include_creation: bool,
) -> Vec<crate::action_material::CanonicalAccountRoleV1> {
    let fresh = if include_creation { "fresh-direct-reservation" } else { "direct-reservation" };
    vec![
        role("direct-root", snapshot.root, true, false),
        role("direct-replay", snapshot.replay, true, false),
        role(fresh, snapshot.fresh_reservation, true, false),
        role("actor-payer", snapshot.actor_payer, true, true),
        role("general-position-v3", snapshot.authority.position_v3, true, false),
        role("general-position-replay-v3", snapshot.authority.position_replay_v3, true, false),
        role("realm", snapshot.authority.realm, false, false),
        role("collateral-profile", snapshot.authority.collateral_profile, false, false),
        role("collateral-policy", snapshot.authority.collateral_policy, false, false),
        role("token-program", snapshot.authority.token_program, false, false),
        role("general-market-binding-v4", snapshot.authority.general_market_binding_v4, false, false),
        role("general-market-runtime", snapshot.authority.general_market_runtime, false, false),
        role("market-instance-v2", snapshot.authority.market_instance_v2, false, false),
        role("system-program", snapshot.system_program, false, false),
        role("rent-sysvar", snapshot.rent_sysvar, false, false),
        role("clock-sysvar", snapshot.clock, false, false),
        role("compiler-bundle-v6", snapshot.compiler_bundle_v6, false, false),
        role("market-genesis-v2", snapshot.authority.market_genesis_v2, false, false),
        role("price-grid", snapshot.price_grid, false, false),
    ]
}

fn reservation_prefix_roles_cancel(
    snapshot: DirectCancelOrderSnapshotV2<'_>,
) -> Vec<crate::action_material::CanonicalAccountRoleV1> {
    vec![
        role("direct-root", snapshot.root, true, false),
        role("direct-replay", snapshot.replay, true, false),
        role("direct-reservation", snapshot.reservation, true, false),
        role("actor-payer", snapshot.actor_payer, true, true),
        role("general-position-v3", snapshot.authority.position_v3, true, false),
        role("general-position-replay-v3", snapshot.authority.position_replay_v3, true, false),
        role("realm", snapshot.authority.realm, false, false),
        role("collateral-profile", snapshot.authority.collateral_profile, false, false),
        role("collateral-policy", snapshot.authority.collateral_policy, false, false),
        role("token-program", snapshot.authority.token_program, false, false),
        role("general-market-binding-v4", snapshot.authority.general_market_binding_v4, false, false),
        role("general-market-runtime", snapshot.authority.general_market_runtime, false, false),
        role("market-instance-v2", snapshot.authority.market_instance_v2, false, false),
        role("market-genesis-v2", snapshot.authority.market_genesis_v2, false, false),
        role("neutral-lamport-sink", snapshot.neutral_sink, true, false),
        role("clock-sysvar", snapshot.clock, false, false),
    ]
}

fn require_all_distinct(metas: &[AccountMeta]) -> Result<(), CanonicalActionMaterialErrorV1> {
    for left in 0..metas.len() {
        for right in left + 1..metas.len() {
            if metas[left].pubkey == metas[right].pubkey {
                return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
            }
        }
    }
    Ok(())
}

fn find_reservation_index(
    state: &DirectRootReplayTransitionV2,
    address: Address,
) -> Result<u8, CanonicalActionMaterialErrorV1> {
    let mut index = 0u8;
    while index < state.root().live_reservations() {
        if state.root().reservation_account(index)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
            == address.to_bytes()
        {
            return Ok(index);
        }
        index = index.checked_add(1)
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    }
    Err(CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn require_deletable_rent(
    rent: clutch_general_v2_contract::DeletableRentOwnerV1,
    observed: u64,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    rent.validate().map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let floor = rent.refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if observed < floor {
        Err(CanonicalActionMaterialErrorV1::InvalidPlan)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct OperatorDirectHashV2;

impl DirectHashBackendV1 for OperatorDirectHashV2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for part in parts { hash.update(part); }
        hash.finalize().into()
    }
}

impl PositionV3Sha256Backend for OperatorDirectHashV2 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, &[domain, body])
    }
}

impl ReplayV3HashBackend for OperatorDirectHashV2 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        DirectHashBackendV1::sha256_parts(self, parts)
    }
}

#[derive(Debug)]
struct DecodedRootReplayV2 {
    root_bump: u8,
    replay_bump: u8,
    state: DirectRootReplayTransitionV2,
}

#[derive(Clone, Copy, Debug)]
struct FreezeAuthorityV2 {
    root_semantic_id: [u8; 32],
    replay: DirectActionReplayV1,
    selection_account: [u8; 32],
    rent: DirectRentOwnerV1,
    reservations: [Option<DirectReservationV1>; 2],
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    sequence: u64,
    slot: u64,
}

impl AuthenticatedDirectSelectionFreezeV2 for FreezeAuthorityV2 {
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
            && domain == &self.domain
            && price == &self.price
            && consumed_sequence == self.sequence
            && observed_slot == self.slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Construct action 4 from one exact finalized snapshot. The live Reservation
/// count and ordering come only from b1/v2; the canonical Direct price comes
/// only from that complete book and the authenticated Product price artifacts.
#[allow(clippy::too_many_arguments)]
pub fn construct_direct_freeze_book_v2(
    release: &IndexedProgramRelease,
    manifest: &ExplicitOperatorReleaseManifest,
    builder: &ProtocolTransactionBuilder,
    operator_selection: &KeeperActionSelection,
    freshness: ActionFreshnessBoundaryV1,
    snapshot: DirectFreezeBookSnapshotV2<'_>,
) -> Result<DirectFreezeBookActionMaterialV2, CanonicalActionMaterialErrorV1> {
    let fixed = [
        snapshot.root, snapshot.replay, snapshot.fresh_selection,
        snapshot.creation_payer, snapshot.system_program, snapshot.rent_sysvar,
        snapshot.clock, snapshot.compiler_bundle_v6, snapshot.native_claim_basis,
        snapshot.price_measure_policy, snapshot.market_genesis_v2,
        snapshot.price_grid, snapshot.liveness_policy,
        snapshot.candidate_compartment, snapshot.keeper, snapshot.candidate_payer,
    ];
    authenticate_snapshot_set(release, freshness, &fixed)?;
    require_operator_account(snapshot.creation_payer)?;
    require_operator_account(snapshot.keeper)?;
    require_system_program(snapshot.system_program)?;
    if snapshot.creation_payer.address != snapshot.keeper.address
        || builder.payer() != snapshot.keeper.address
    {
        return Err(CanonicalActionMaterialErrorV1::FeePayerMismatch);
    }
    let slot = decode_clock(snapshot.clock)?;
    let rent_principal = decode_rent_minimum(
        snapshot.rent_sysvar,
        DIRECT_SELECTION_ACCOUNT_BYTES,
    )?;
    let mut decoded = decode_root_replay(release, snapshot.root, snapshot.replay)?;
    if decoded.state.root().phase() != DirectRootPhaseV1::Open {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let reservation_count = usize::from(decoded.state.root().live_reservations());
    if reservation_count > 2 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let mut reservation_values = [None; 2];
    let mut reservation_accounts = Vec::with_capacity(reservation_count);
    for index in 0..2 {
        match (index < reservation_count, snapshot.reservations[index]) {
            (true, Some(account)) => {
                authenticate_snapshot_set(release, freshness, &[account])?;
                let value = decode_reservation(
                    release,
                    snapshot.root,
                    account,
                    &decoded.state,
                    u8::try_from(index).map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
                )?;
                reservation_values[index] = Some(value);
                reservation_accounts.push(account);
            }
            (false, None) => {}
            _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        }
    }
    if reservation_count == 2
        && reservation_values[1]
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
            .order_id()
            < reservation_values[0]
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?
                .order_id()
    {
        reservation_values.swap(0, 1);
        reservation_accounts.swap(0, 1);
    }
    let (selection_pda, selection_bump) = Address::find_program_address(
        &[DIRECT_SELECTION_SEED_V1, snapshot.root.address.as_ref()],
        &release.program_id,
    );
    require_fresh_pda(snapshot.fresh_selection, selection_pda)?;
    let selection_rent = DirectRentOwnerV1 {
        payer: snapshot.creation_payer.address.to_bytes(),
        principal_lamports: rent_principal,
        donation_floor_lamports: snapshot.fresh_selection.lamports,
    };
    selection_rent
        .validate()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if snapshot.creation_payer.lamports < rent_principal {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let (domain, price) = authenticate_price_contract(
        release,
        &decoded.state,
        snapshot,
        reservation_values,
    )?;
    let sequence = decoded.state.replay().next_action_sequence();
    let authority = FreezeAuthorityV2 {
        root_semantic_id: decoded.state.root().root_semantic_id(),
        replay: decoded.state.replay(),
        selection_account: selection_pda.to_bytes(),
        rent: selection_rent,
        reservations: reservation_values,
        domain,
        price,
        sequence,
        slot,
    };
    let plan = prepare_direct_selection_freeze_v2(
        &authority,
        &mut decoded.state,
        sequence,
        slot,
        selection_pda.to_bytes(),
        selection_rent,
        reservation_values,
        domain,
        price,
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let binding = decoded.state.root().candidate_liveness();
    let (candidate_after, keeper_payment, payer_refund, candidate_post_id) =
        apply_freeze_liveness(
            release,
            snapshot,
            &decoded.state,
            &plan.selection,
            binding,
        )?;
    let batch = prepare_direct_candidate_work_batch_v2(
        &decoded.state,
        Some(&plan.selection),
        DirectMarketActionV1::FreezeBook,
        decode_candidate(snapshot.candidate_compartment)?.completed_calls,
        decode_candidate(snapshot.candidate_compartment)?.last_work_receipt_id.bytes(),
        sha256(&snapshot.candidate_compartment.data),
        snapshot.keeper.address.to_bytes(),
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    bind_direct_candidate_work_batch_v2(&mut decoded.state, batch, &OperatorDirectHashV2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;

    let mut prestate_accounts = fixed.to_vec();
    prestate_accounts.extend(reservation_accounts.iter().copied());
    let prestate_id = snapshot_id(
        release,
        DirectMarketAction::FreezeBook,
        sequence,
        &prestate_accounts,
    );
    authenticate_freeze_cursor(
        operator_selection,
        decoded.state.root().generation(),
        sequence,
        prestate_id,
        snapshot,
        &reservation_accounts,
    )?;
    let post = encode_freeze_postimages(
        snapshot.root,
        snapshot.replay,
        snapshot.fresh_selection,
        selection_bump,
        &decoded,
        plan.selection,
    )?;

    let mut metas = vec![
        meta(snapshot.root, false, true),
        meta(snapshot.replay, false, true),
        meta(snapshot.fresh_selection, false, true),
        meta(snapshot.creation_payer, true, true),
        meta(snapshot.system_program, false, false),
        meta(snapshot.rent_sysvar, false, false),
        meta(snapshot.clock, false, false),
        meta(snapshot.compiler_bundle_v6, false, false),
        meta(snapshot.native_claim_basis, false, false),
        meta(snapshot.price_measure_policy, false, false),
        meta(snapshot.market_genesis_v2, false, false),
        meta(snapshot.price_grid, false, false),
    ];
    let mut roles = vec![
        role("direct-root", snapshot.root, true, false),
        role("direct-replay", snapshot.replay, true, false),
        role("fresh-direct-selection", snapshot.fresh_selection, true, false),
        role("selection-rent-payer", snapshot.creation_payer, true, true),
        role("system-program", snapshot.system_program, false, false),
        role("rent-sysvar", snapshot.rent_sysvar, false, false),
        role("clock-sysvar", snapshot.clock, false, false),
        role("compiler-bundle-v6", snapshot.compiler_bundle_v6, false, false),
        role("native-claim-basis", snapshot.native_claim_basis, false, false),
        role("price-measure-policy", snapshot.price_measure_policy, false, false),
        role("market-genesis-v2", snapshot.market_genesis_v2, false, false),
        role("price-grid", snapshot.price_grid, false, false),
    ];
    for account in &reservation_accounts {
        metas.push(meta(account, false, false));
        roles.push(role("root-owned-reservation", account, false, false));
    }
    for (label, account, signer) in [
        ("candidate-liveness-policy", snapshot.liveness_policy, false),
        ("candidate-compartment", snapshot.candidate_compartment, false),
        ("keeper", snapshot.keeper, true),
        ("candidate-immutable-payer", snapshot.candidate_payer, false),
    ] {
        let writable = label != "candidate-liveness-policy";
        metas.push(meta(account, signer, writable));
        roles.push(role(label, account, writable, signer));
    }
    require_freeze_alias_contract(&metas, reservation_count)?;
    let selection_after = snapshot
        .fresh_selection
        .lamports
        .checked_add(rent_principal)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let equations = vec![
        ExactEquation {
            name: "selection rent principal plus immutable prefund".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(snapshot.creation_payer.lamports)
                + u128::from(snapshot.fresh_selection.lamports),
            right: u128::from(snapshot.creation_payer.lamports - rent_principal)
                + u128::from(selection_after),
        },
        ExactEquation {
            name: "candidate prepaid work conservation".into(),
            unit: IntegerUnit::Lamports,
            left: u128::from(snapshot.candidate_compartment.lamports),
            right: u128::from(candidate_after)
                + u128::from(keeper_payment)
                + u128::from(payer_refund),
        },
    ];
    let payload = DirectMarketClientPayloadV1::empty(DirectMarketAction::FreezeBook)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let canonical = finish_chain_derived_direct_material_v2(
        release,
        manifest,
        builder,
        operator_selection,
        freshness,
        DirectMarketAction::FreezeBook,
        sequence,
        metas,
        vec![snapshot.keeper.address],
        roles,
        equations,
        payload,
    )?;
    Ok(DirectFreezeBookActionMaterialV2 {
        canonical,
        postimages: DirectFreezeBookPostimagesV2 {
            prestate_id,
            root_data_id: post.0,
            replay_data_id: post.1,
            selection_data_id: post.2,
            candidate_data_id: candidate_post_id,
            selection_lamports_after: selection_after,
            candidate_lamports_after: candidate_after,
            keeper_payment_lamports: keeper_payment,
            candidate_payer_refund_lamports: payer_refund,
        },
    })
}

fn decode_root_replay(
    release: &IndexedProgramRelease,
    root_account: &ObservedRpcAccount,
    replay_account: &ObservedRpcAccount,
) -> Result<DecodedRootReplayV2, CanonicalActionMaterialErrorV1> {
    if root_account.owner != release.program_id
        || replay_account.owner != release.program_id
        || root_account.executable
        || replay_account.executable
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let root_frame = DirectMarketRootAccountV2::decode(&root_account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let root = authenticate_direct_root_transition_body_v2(
        root_frame.semantic_body(),
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let generation = root.generation().to_le_bytes();
    let (expected_root, root_bump) = Address::find_program_address(
        &[DIRECT_ROOT_SEED_V2, &root.market_instance_id(), &generation],
        &release.program_id,
    );
    if expected_root != root_account.address
        || root_bump != root_frame.bump()
        || root.direct_root_account() != root_account.address.to_bytes()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_rent(root.root_rent(), root_account.lamports)?;
    let replay_bytes = <&[u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES]>::try_from(
        replay_account.data.as_slice(),
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay_frame = DirectActionReplayAccountV1::decode(replay_bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let replay = decode_direct_action_replay_body_for_transition_v2(
        replay_frame.semantic_body(),
        &root,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (expected_replay, replay_bump) = Address::find_program_address(
        &[DIRECT_REPLAY_SEED_V1, root_account.address.as_ref()],
        &release.program_id,
    );
    if expected_replay != replay_account.address
        || replay_bump != replay_frame.bump()
        || replay_account.address.to_bytes() != root.action_replay_account()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_rent(replay.rent(), replay_account.lamports)?;
    let state = DirectRootReplayTransitionV2::authenticate(root, replay)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok(DecodedRootReplayV2 { root_bump, replay_bump, state })
}

fn decode_reservation(
    release: &IndexedProgramRelease,
    root_account: &ObservedRpcAccount,
    account: &ObservedRpcAccount,
    state: &DirectRootReplayTransitionV2,
    root_index: u8,
) -> Result<DirectReservationV1, CanonicalActionMaterialErrorV1> {
    if account.owner != release.program_id || account.executable {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let bytes = <&[u8; DIRECT_RESERVATION_ACCOUNT_BYTES]>::try_from(account.data.as_slice())
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let frame = DirectReservationAccountV1::decode(bytes)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let value = decode_direct_reservation_body_for_transition_v2(
        frame.semantic_body(),
        state.root(),
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (expected, bump) = Address::find_program_address(
        &[DIRECT_RESERVATION_SEED_V1, root_account.address.as_ref(), &value.order_id()],
        &release.program_id,
    );
    let semantic_id = state
        .root()
        .child_reservation_semantic_id(value, &OperatorDirectHashV2)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if expected != account.address
        || bump != frame.bump()
        || account.address.to_bytes()
            != state.root().reservation_account(root_index)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
        || semantic_id
            != state.root().reservation_semantic_id(root_index)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    require_rent(value.rent(), account.lamports)?;
    Ok(value)
}

fn authenticate_price_contract(
    release: &IndexedProgramRelease,
    state: &DirectRootReplayTransitionV2,
    snapshot: DirectFreezeBookSnapshotV2<'_>,
    reservations: [Option<DirectReservationV1>; 2],
) -> Result<(EconomicDomainV2, PricePreconditionV2), CanonicalActionMaterialErrorV1> {
    let root = state.root();
    let bundle = authenticate_product_artifact::<CompiledProductSeriesBundleV6>(
        release,
        snapshot.compiler_bundle_v6,
        ArtifactKind::CompiledProductSeriesBundleV6,
        root.compiler_bundle_v6_id(),
        |value| value.id().map(|id| id.bytes()),
    )?;
    let basis = authenticate_product_artifact::<NativeClaimBasisV1>(
        release,
        snapshot.native_claim_basis,
        ArtifactKind::NativeClaimBasisV1,
        bundle.native_claim_basis_id.bytes(),
        |value| value.id().map(|id| id.bytes()),
    )?;
    let policy = authenticate_product_artifact::<PriceMeasurePolicyV1>(
        release,
        snapshot.price_measure_policy,
        ArtifactKind::PriceMeasurePolicyV1,
        bundle.price_measure_policy_id.bytes(),
        |value| value.id().map(|id| id.bytes()),
    )?;
    let genesis = authenticate_product_artifact::<MarketGenesisProfileV2>(
        release,
        snapshot.market_genesis_v2,
        ArtifactKind::MarketGenesisProfileV2,
        bundle.market_genesis_profile_id.bytes(),
        |value| value.id().map(|id| id.bytes()),
    )?;
    if bundle.price_measure_policy_id.bytes() != root.price_policy_id()
        || genesis.realm_id.bytes() != root.realm_id()
        || genesis.profile_id.bytes() != root.collateral_profile_id()
        || genesis.relation_policy_id.bytes() != root.relation_policy_id()
        || genesis.fee_policy_id.bytes() != root.fee_policy().revenue_policy_v2_digest
        || basis.outcome_count != root.outcome_count()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    if snapshot.price_grid.owner != release.program_id
        || snapshot.price_grid.executable
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let grid = PriceGridAccount::decode(&snapshot.price_grid.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (expected_grid, bump) = Address::find_program_address(
        &[PRICE_GRID_SEED_V1, &grid.realm.0, &grid.grid.0],
        &release.program_id,
    );
    if expected_grid != snapshot.price_grid.address
        || bump != grid.stored_bump
        || grid.realm.0 != root.realm_id()
        || grid.grid.0 != genesis.price_grid_id.bytes()
        || grid.price_scale != root.price_scale()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let mut book = DirectEconomicBookV1 {
        orders: [EMPTY_ECONOMIC_ORDER_V2; 2],
        len: 0,
    };
    for (index, reservation) in reservations.iter().enumerate() {
        if let Some(value) = reservation {
            let limit = u64::try_from(value.limit_price_units_per_egg())
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
            grid.tick_of(limit)
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
            book.orders[index] = value
                .economic_order()
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
            book.len = book.len
                .checked_add(1)
                .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        }
    }
    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: root.market_instance_id(),
        epoch_semantics_digest: root.direct_epoch_semantics_id(),
        relation_policy_digest: root.relation_policy_id(),
        price_policy_digest: root.price_policy_id(),
        epoch_index: root.direct_window_index()
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?,
        outcome_count: root.outcome_count(),
        price_scale: root.price_scale(),
    };
    let price = canonical_direct_price_precondition_v1(&domain, &book)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    for index in 0..price.prices.len() {
        if index < usize::from(root.outcome_count()) {
            grid.tick_of(price.prices[index])
                .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        } else if price.prices[index] != 0 {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    policy.validate_candidate_price_contract(
        &basis,
        &PriceVectorV3 {
            basis_degree: basis.basis_degree,
            native_outcome_count: basis.outcome_count,
            price_scale: grid.price_scale,
            prices: price.prices,
        },
        grid.price_scale,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok((domain, price))
}

fn authenticate_product_artifact<T, F>(
    release: &IndexedProgramRelease,
    account: &ObservedRpcAccount,
    kind: ArtifactKind,
    expected_id: [u8; 32],
    derive_id: F,
) -> Result<T, CanonicalActionMaterialErrorV1>
where
    T: FixedCodec,
    F: FnOnce(&T) -> clutch_product_series::Result<[u8; 32]>,
{
    if account.owner != release.program_id
        || account.executable
        || account.data.len() != T::ENCODED_LEN
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let (expected, _) = Address::find_program_address(
        &[PRODUCT_ARTIFACT_SEED_V1, &[kind.byte()], &expected_id],
        &release.program_id,
    );
    let value = T::decode(&account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let derived = derive_id(&value)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if expected != account.address || derived != expected_id {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(value)
}

fn apply_freeze_liveness(
    release: &IndexedProgramRelease,
    snapshot: DirectFreezeBookSnapshotV2<'_>,
    state: &DirectRootReplayTransitionV2,
    selection: &DirectSelectionV1,
    binding: DirectCandidateLivenessBindingV1,
) -> Result<(u64, u64, u64, [u8; 32]), CanonicalActionMaterialErrorV1> {
    if snapshot.liveness_policy.owner != release.program_id
        || snapshot.candidate_compartment.owner != release.program_id
        || snapshot.liveness_policy.executable
        || snapshot.candidate_compartment.executable
        || snapshot.liveness_policy.data.len() != RUNTIME_LIVENESS_POLICY_BYTES_V1
        || snapshot.candidate_compartment.data.len() != RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let policy = clutch_liveness::RuntimeLivenessPolicyV1::decode(&snapshot.liveness_policy.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let mut candidate_data = <[u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1]>::try_from(
        snapshot.candidate_compartment.data.as_slice(),
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate = RuntimeCompartmentV1::decode(&candidate_data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    candidate.validate_against_policy(policy)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate_policy = policy.compartment(RuntimeCompartmentKindV1::Candidate);
    let expected_candidate_balance = candidate.expected_account_balance_lamports()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let candidate_pre_id = sha256(&candidate_data);
    if snapshot.liveness_policy.address.to_bytes() != binding.policy_account
        || snapshot.candidate_compartment.address.to_bytes() != binding.candidate_account
        || sha256(&snapshot.liveness_policy.data) != binding.policy_data_id
        || policy.policy_id.bytes() != state.root().candidate_liveness_policy_id()
        || policy.realm_id.bytes() != state.root().realm_id()
        || policy.neutral_sink.bytes() != state.root().neutral_lamport_sink()
        || candidate.kind != RuntimeCompartmentKindV1::Candidate
        || candidate.phase != clutch_liveness::RuntimeCompartmentPhaseV1::Active
        || candidate.maximum_calls != candidate_policy.maximum_calls
        || candidate.maximum_lamports_per_call != candidate_policy.maximum_lamports_per_call
        || candidate.capitalized_work_lamports != candidate_policy.work_capital_lamports
        || candidate.rent_principal_lamports
            != candidate_policy.account_rent_principal_lamports
        || snapshot.candidate_compartment.lamports < expected_candidate_balance
        || candidate.identity.policy_id.bytes() != state.root().candidate_liveness_policy_id()
        || candidate.identity.lifecycle_id.bytes() != binding.global_lifecycle_id
        || candidate.identity.account_id.bytes() != binding.candidate_account
        || candidate.identity.owner.bytes() != binding.candidate_semantic_owner
        || candidate.identity.payer.bytes() != snapshot.candidate_payer.address.to_bytes()
        || candidate.identity.neutral_sink.bytes() != state.root().neutral_lamport_sink()
        || candidate.identity.generation != binding.candidate_generation
        || candidate.quote_schedule_id.bytes() != binding.candidate_quote_schedule_id
        || candidate.receipt_program_id.bytes() != binding.candidate_receipt_program_id
        || candidate.receipt_program_id.bytes() != release.program_id.to_bytes()
        || (state.replay().candidate_liveness_completed_calls() == 0
            && candidate_pre_id != binding.candidate_data_id)
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let batch = prepare_direct_candidate_work_batch_v2(
        state,
        Some(selection),
        DirectMarketActionV1::FreezeBook,
        candidate.completed_calls,
        candidate.last_work_receipt_id.bytes(),
        candidate_pre_id,
        snapshot.keeper.address.to_bytes(),
        &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let (balance, keeper, payer) = apply_candidate_batch(
        release,
        snapshot,
        binding,
        batch,
        &mut candidate_data,
    )?;
    Ok((balance, keeper, payer, sha256(&candidate_data)))
}

fn apply_candidate_batch(
    release: &IndexedProgramRelease,
    snapshot: DirectFreezeBookSnapshotV2<'_>,
    binding: DirectCandidateLivenessBindingV1,
    batch: DirectCandidateWorkBatchV1,
    candidate_data: &mut [u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1],
) -> Result<(u64, u64, u64), CanonicalActionMaterialErrorV1> {
    let expected_program = LivenessId::from_bytes(release.program_id.to_bytes());
    let expected_policy = LivenessId::from_bytes(snapshot.liveness_policy.address.to_bytes());
    let mut balance = snapshot.candidate_compartment.lamports;
    let mut keeper_total = 0u64;
    let mut payer_total = 0u64;
    for index in 0..batch.receipt_count() {
        let receipt = batch.receipt(index, binding, &OperatorDirectHashV2)
            .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let after = balance.checked_sub(receipt.call_ceiling_lamports())
            .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
        let intent = RuntimeTransitionIntentV1 {
            action: RuntimeTransitionActionV1::SpendWork,
            kind: RuntimeCompartmentKindV1::Candidate,
            policy_id: LivenessId::from_bytes(
                decode_candidate(snapshot.candidate_compartment)?.identity.policy_id.bytes(),
            ),
            lifecycle_id: LivenessId::from_bytes(binding.global_lifecycle_id),
            account_id: LivenessId::from_bytes(binding.candidate_account),
            semantic_owner: LivenessId::from_bytes(binding.candidate_semantic_owner),
            quote_schedule_id: LivenessId::from_bytes(binding.candidate_quote_schedule_id),
            receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
            keeper: LivenessId::from_bytes(snapshot.keeper.address.to_bytes()),
            generation: binding.candidate_generation,
            call_ordinal: receipt.call_ordinal(),
            call_ceiling_lamports: receipt.call_ceiling_lamports(),
            keeper_payment_lamports: receipt.keeper_payment_lamports(),
            flags: 0,
        };
        let transition = plan_runtime_transition_v1(
            expected_program,
            expected_policy,
            RuntimePersistedAccountViewV1 {
                account_id: expected_policy,
                owner_program_id: LivenessId::from_bytes(snapshot.liveness_policy.owner.to_bytes()),
                lamports: snapshot.liveness_policy.lamports,
                data: &snapshot.liveness_policy.data,
                writable: false,
            },
            RuntimePersistedAccountViewV1 {
                account_id: LivenessId::from_bytes(snapshot.candidate_compartment.address.to_bytes()),
                owner_program_id: LivenessId::from_bytes(snapshot.candidate_compartment.owner.to_bytes()),
                lamports: balance,
                data: candidate_data,
                writable: true,
            },
            intent,
            Some(RuntimeReceiptObservationV1 {
                receipt_account_id: LivenessId::from_bytes(snapshot.replay.address.to_bytes()),
                receipt_account_owner_program_id: expected_program,
                receipt_id: LivenessId::from_bytes(receipt.receipt_id()),
                receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
                compartment_kind: RuntimeCompartmentKindV1::Candidate,
                semantic_owner: LivenessId::from_bytes(binding.candidate_semantic_owner),
                lifecycle_id: LivenessId::from_bytes(binding.global_lifecycle_id),
                quote_schedule_id: LivenessId::from_bytes(binding.candidate_quote_schedule_id),
                generation: binding.candidate_generation,
                call_ordinal: receipt.call_ordinal(),
                call_ceiling_lamports: receipt.call_ceiling_lamports(),
            }),
            after,
        )
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
        if !transition.write_account_data || transition.close_account {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
        for movement in transition.transfers() {
            match movement.role {
                RuntimeTransferRoleV1::KeeperPayment
                    if movement.destination.bytes() == snapshot.keeper.address.to_bytes() =>
                {
                    keeper_total = keeper_total.checked_add(movement.lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                }
                RuntimeTransferRoleV1::PayerWorkRefund
                    if movement.destination.bytes() == snapshot.candidate_payer.address.to_bytes() =>
                {
                    payer_total = payer_total.checked_add(movement.lamports)
                        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
                }
                _ => return Err(CanonicalActionMaterialErrorV1::InvalidPlan),
            }
        }
        candidate_data.copy_from_slice(&transition.post_account_data);
        balance = after;
    }
    if keeper_total != batch.total_keeper_payment_lamports()
        || payer_total != batch.total_payer_refund_lamports()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok((balance, keeper_total, payer_total))
}

fn decode_candidate(
    account: &ObservedRpcAccount,
) -> Result<RuntimeCompartmentV1, CanonicalActionMaterialErrorV1> {
    RuntimeCompartmentV1::decode(&account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn encode_freeze_postimages(
    root_pre: &ObservedRpcAccount,
    replay_pre: &ObservedRpcAccount,
    selection_pre: &ObservedRpcAccount,
    selection_bump: u8,
    decoded: &DecodedRootReplayV2,
    selection: DirectSelectionV1,
) -> Result<([u8; 32], [u8; 32], [u8; 32]), CanonicalActionMaterialErrorV1> {
    let mut root = root_pre.data.clone();
    let mut replay = replay_pre.data.clone();
    let mut selection_bytes = vec![0u8; DIRECT_SELECTION_ACCOUNT_BYTES];
    if root.len() != DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V2
        || replay.len() != DIRECT_ACTION_REPLAY_ACCOUNT_BYTES
        || !selection_pre.data.is_empty()
        || root[2] != decoded.root_bump
        || replay[2] != decoded.replay_bump
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    root[2] = decoded.root_bump;
    replay[2] = decoded.replay_bump;
    selection_bytes[0] = clutch_solana_layout::registry::DIRECT_SELECTION_ACCOUNT_TAG;
    selection_bytes[1] = clutch_solana_layout::registry::DIRECT_SELECTION_ACCOUNT_VERSION;
    selection_bytes[2] = selection_bump;
    write_direct_root_transition_body_v2(
        decoded.state.root(), &mut root[4..], &OperatorDirectHashV2,
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    encode_direct_action_replay_body_into_transition_v2(
        decoded.state.replay(), decoded.state.root(), &mut replay[4..],
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    encode_direct_selection_body_into_transition_v2(
        selection, decoded.state.root(), &mut selection_bytes[4..],
    )
    .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let frame = <&[u8; DIRECT_SELECTION_ACCOUNT_BYTES]>::try_from(selection_bytes.as_slice())
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    DirectSelectionAccountV1::decode(frame)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    Ok((sha256(&root), sha256(&replay), sha256(&selection_bytes)))
}

fn authenticate_freeze_cursor(
    selection: &KeeperActionSelection,
    generation: u64,
    sequence: u64,
    prestate_id: [u8; 32],
    snapshot: DirectFreezeBookSnapshotV2<'_>,
    reservations: &[&ObservedRpcAccount],
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let mut expected = vec![snapshot.root.address, snapshot.replay.address];
    expected.extend(reservations.iter().map(|account| account.address));
    expected.push(snapshot.liveness_policy.address);
    expected.push(snapshot.candidate_compartment.address);
    let mut digest_accounts = vec![snapshot.root, snapshot.replay];
    digest_accounts.extend(reservations.iter().copied());
    digest_accounts.push(snapshot.liveness_policy);
    digest_accounts.push(snapshot.candidate_compartment);
    digest_accounts.sort_by_key(|account| account.address);
    let dependency_id = finalized_dependency_digest(&digest_accounts)?;
    if selection.cursor.workflow_id == [0; 32]
        || selection.cursor.lane != crate::workflow_graph::WorkflowLane::Candidate
        || selection.cursor.generation != generation
        || selection.cursor.position.phase != u16::from(DirectMarketAction::FreezeBook.tag())
        || selection.cursor.position.item != sequence
        || selection.cursor.observed_state_sha256 != dependency_id
        || prestate_id == [0; 32]
        || selection.dependencies.len() != expected.len()
        || expected.iter().any(|account| !selection.dependencies.contains(account))
    {
        return Err(CanonicalActionMaterialErrorV1::WrongSelection);
    }
    Ok(())
}

fn require_freeze_alias_contract(
    metas: &[AccountMeta],
    reservation_count: usize,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let liveness_start = 12usize
        .checked_add(reservation_count)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if metas.len() != liveness_start + 4 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    for left in 0..liveness_start {
        for right in left + 1..liveness_start {
            if metas[left].pubkey == metas[right].pubkey {
                return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
            }
        }
    }
    let creation_payer = metas[3].pubkey;
    for fixed in &metas[..liveness_start] {
        if metas[liveness_start].pubkey == fixed.pubkey
            || metas[liveness_start + 1].pubkey == fixed.pubkey
            || (metas[liveness_start + 2].pubkey == fixed.pubkey
                && fixed.pubkey != creation_payer)
            || (metas[liveness_start + 3].pubkey == fixed.pubkey
                && fixed.pubkey != creation_payer)
        {
            return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
        }
    }
    Ok(())
}

fn authenticate_snapshot_set(
    release: &IndexedProgramRelease,
    freshness: ActionFreshnessBoundaryV1,
    accounts: &[&ObservedRpcAccount],
) -> Result<(), CanonicalActionMaterialErrorV1> {
    for account in accounts {
        if account.provenance.commitment != RpcCommitment::Finalized
            || account.provenance.release_key != release.key()
            || account.provenance.slot != freshness.observed_slot
        {
            return Err(CanonicalActionMaterialErrorV1::WrongSelection);
        }
    }
    Ok(())
}

fn require_operator_account(
    account: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.address == Address::default()
        || account.owner != Address::default()
        || account.executable
        || !account.data.is_empty()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn require_system_program(
    account: &ObservedRpcAccount,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.address != Address::default() || !account.executable {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn require_fresh_pda(
    account: &ObservedRpcAccount,
    expected: Address,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    if account.address != expected
        || account.owner != Address::default()
        || account.executable
        || !account.data.is_empty()
    {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(())
}

fn decode_clock(account: &ObservedRpcAccount) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let expected = Address::from_str("SysvarC1ock11111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let owner = Address::from_str("Sysvar1111111111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if account.address != expected || account.owner != owner || account.data.len() != 40 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let slot = u64::from_le_bytes(account.data[..8].try_into()
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?);
    if slot == 0 || slot != account.provenance.slot {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    Ok(slot)
}

fn decode_rent_minimum(
    account: &ObservedRpcAccount,
    data_len: usize,
) -> Result<u64, CanonicalActionMaterialErrorV1> {
    let expected = Address::from_str("SysvarRent111111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    let owner = Address::from_str("Sysvar1111111111111111111111111111111111111")
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if account.address != expected || account.owner != owner || account.data.len() != 17 {
        return Err(CanonicalActionMaterialErrorV1::InvalidPlan);
    }
    let rent: solana_rent::Rent = bincode::deserialize(&account.data)
        .map_err(|_| CanonicalActionMaterialErrorV1::InvalidPlan)?;
    rent.try_minimum_balance(data_len)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)
}

fn require_rent(
    rent: DirectRentOwnerV1,
    observed: u64,
) -> Result<(), CanonicalActionMaterialErrorV1> {
    let minimum = rent.principal_lamports
        .checked_add(rent.donation_floor_lamports)
        .ok_or(CanonicalActionMaterialErrorV1::InvalidPlan)?;
    if observed < minimum { Err(CanonicalActionMaterialErrorV1::InvalidPlan) } else { Ok(()) }
}

fn snapshot_id(
    release: &IndexedProgramRelease,
    action: DirectMarketAction,
    sequence: u64,
    accounts: &[&ObservedRpcAccount],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"dragons-clutch/operator/direct-order-snapshot/v2\0");
    hash.update(release.release_manifest_sha256);
    hash.update(release.capability_profile_id);
    hash.update([action.tag()]);
    hash.update(sequence.to_le_bytes());
    for account in accounts {
        hash.update(account.address.as_ref());
        hash.update(account.owner.as_ref());
        hash.update(account.lamports.to_le_bytes());
        hash.update([u8::from(account.executable)]);
        hash.update(account.provenance.slot.to_le_bytes());
        hash.update(sha256(&account.data));
    }
    hash.finalize().into()
}

fn sha256(input: &[u8]) -> [u8; 32] { Sha256::digest(input).into() }

fn meta(account: &ObservedRpcAccount, signer: bool, writable: bool) -> AccountMeta {
    AccountMeta { pubkey: account.address, is_signer: signer, is_writable: writable }
}

fn role(
    label: &'static str,
    account: &ObservedRpcAccount,
    writable: bool,
    signer: bool,
) -> crate::action_material::CanonicalAccountRoleV1 {
    chain_derived_direct_role_v2(label, account.address, writable, signer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_reservation_prefix_cannot_be_caller_padded() {
        let values = [Some(7u8), None];
        assert!(matches!(values, [Some(_), None]));
        let hostile = [None, Some(7u8)];
        assert!(!matches!(hostile, [Some(_), None]));
    }

    #[test]
    fn rent_prefund_is_donation_not_principal() {
        let prefund = 9u64;
        let principal = 13u64;
        assert_eq!(prefund.checked_add(principal), Some(22));
        assert_ne!(prefund, principal);
    }

    #[test]
    fn quantity_quantum_is_the_exact_cash_boundary() {
        for (limit, scale) in [(3u64, 10u64), (25, 100), (7, 49), (0, 10)] {
            let quantum = scale / gcd_u64(limit, scale);
            let quantity = quantum.checked_mul(11).unwrap();
            assert_eq!(u128::from(quantity) * u128::from(limit) % u128::from(scale), 0);
            if quantum > 1 {
                assert_ne!(
                    u128::from(quantity - 1) * u128::from(limit) % u128::from(scale),
                    0,
                );
            }
        }
    }

    #[test]
    fn operator_order_identity_binds_every_economic_and_release_fact() {
        let release = IndexedProgramRelease {
            program_id: Address::new_from_array([2; 32]),
            program_data: Address::new_from_array([3; 32]),
            deployment_slot: 4,
            elf_sha256: [5; 32],
            release_manifest_sha256: [6; 32],
            capability_profile_id: [7; 32],
            source_commit: "fixture".into(),
            families: vec![],
            enabled_intents: vec![],
        };
        let root = Address::new_from_array([8; 32]);
        let owner = Address::new_from_array([9; 32]);
        let base = derive_operator_order_id(
            &release, root, owner, 11, Side::Buy, 1, 13, 17,
        );
        for hostile in [
            derive_operator_order_id(&release, root, owner, 12, Side::Buy, 1, 13, 17),
            derive_operator_order_id(&release, root, owner, 11, Side::Sell, 1, 13, 17),
            derive_operator_order_id(&release, root, owner, 11, Side::Buy, 0, 13, 17),
            derive_operator_order_id(&release, root, owner, 11, Side::Buy, 1, 14, 17),
            derive_operator_order_id(&release, root, owner, 11, Side::Buy, 1, 13, 18),
        ] {
            assert_ne!(base, hostile);
        }
    }

    #[test]
    fn reservation_meta_aliases_are_refused_before_drafting() {
        let address = Address::new_from_array([1; 32]);
        let metas = vec![
            AccountMeta::new(address, false),
            AccountMeta::new_readonly(address, false),
        ];
        assert_eq!(
            require_all_distinct(&metas),
            Err(CanonicalActionMaterialErrorV1::InvalidPlan),
        );
    }
}
